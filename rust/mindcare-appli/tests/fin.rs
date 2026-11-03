//! La fin du programme avec l'assistant et la dictée branchés (étape 6.4) : le processus quitte pour de bon.
//!
//! `std::process::exit` arrête d'abord les autres fils, puis Rust lâche les `thread_local` du fil qui quitte. Le
//! programme garde tout dans un `thread_local` : lâcher là la dictée (son fil) et le réseau de WinHTTP (avec le proxy
//! du PC) bloquait la fin pour toujours, et l'ancienne copie de Mindcare ne quittait plus après un changement de
//! thème (le contrat de la suite restait accroché). Ce test rejoue cette fin dans un processus à part (ce même
//! programme de test, relancé sans fenêtre) avec les vrais outils de l'exe livré, sauf le micro (un faux : aucun vrai
//! micro n'est ouvert, rien n'est téléchargé, llama-server et whisper ne sont jamais lancés).

#![cfg(windows)]

use std::cell::RefCell;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mindcare_appli::effets::Outils;
use mindcare_appli::programme::quitter_sans_rien_lacher;
use mindcare_dictee::{Dictee, DicteeEnFond, Materiel, Mesures, Micro, Reglages};
use mindcare_telechargement::{Disque, Installation, Paquet, Transport, windows};

/// Aucune fenêtre de console pour le processus à part
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// La variable qui dit au programme de test relancé qu'il est le processus à part (et son dossier d'essai)
const VARIABLE: &str = "MINDCARE_ESSAI_FIN";
/// Le nom du test joué par le processus à part
const ENFANT: &str = "processus_a_part_qui_quitte_avec_ses_outils";
/// Le temps laissé au processus à part pour quitter (il quitte en moins d'une seconde)
const DELAI: Duration = Duration::from_secs(30);

thread_local! {
    static OUTILS: RefCell<Option<Outils>> = const { RefCell::new(None) };
}

/// Un micro qui n'existe pas : aucun vrai micro n'est ouvert
struct SansMicro;
impl Micro for SansMicro {
    fn demarrer(&mut self, _numero: u32) -> Result<(), u32> {
        Err(1)
    }
    fn mesures(&self) -> Mesures {
        Mesures { niveau: 0.0, millis: 0, entendu: false, millis_depuis_voix: 0 }
    }
    fn arreter(&mut self) {}
    fn son(&self) -> Vec<u8> {
        Vec::new()
    }
}
struct SansMateriel;
impl Materiel for SansMateriel {
    fn micro_present(&self) -> bool {
        false
    }
    fn noms(&self) -> Vec<String> {
        Vec::new()
    }
    fn nouveau_micro(&self) -> Box<dyn Micro> {
        Box::new(SansMicro)
    }
}

/// Le processus à part (seulement quand la variable est là) : les outils de l'exe livré (WinHTTP avec le proxy du
/// PC, la dictée et son fil, l'assistant, les installations) gardés dans un `thread_local` comme le programme, arrêtés
/// comme à la fermeture, puis la fin du programme. Il ne revient jamais.
#[test]
fn processus_a_part_qui_quitte_avec_ses_outils() {
    let Some(dossier) = std::env::var_os(VARIABLE).map(PathBuf::from) else { return };
    let temp = dossier.join("Temp");
    std::fs::create_dir_all(&temp).expect("dossier d'essai");
    let dictee = DicteeEnFond::lancer(Dictee::nouvelle(
        Reglages { paquet: Paquet::dictee(&dossier), suite_json: None, temp, pid: std::process::id(), fils: 1 },
        Box::new(SansMateriel),
        Box::new(mindcare_dictee::windows::LanceurWindows),
    ));
    let transport = windows::WinHttp::nouveau().map(|t| Arc::new(t) as Arc<dyn Transport>);
    assert!(transport.is_ok(), "WinHTTP : {:?}", transport.as_ref().err());
    let mut outils = Outils::nouveaux(
        mindcare_assistant::assistant(&dossier, Arc::new(|_: &str| {})),
        Installation::nouvelle(Paquet::assistant(&dossier)),
        dictee,
        Installation::nouvelle(Paquet::dictee(&dossier)),
        transport,
        Arc::new(windows::PlaceWindows) as Arc<dyn Disque>,
    );
    // (la fermeture : Branchement::arreter)
    outils.arreter();
    OUTILS.with(|o| *o.borrow_mut() = Some(outils));
    quitter_sans_rien_lacher(&OUTILS, 0)
}

/// La fin du programme avec ses outils quitte vite, et sans erreur (code 0).
#[test]
fn la_fin_avec_l_assistant_et_la_dictee_quitte_pour_de_bon() {
    if std::env::var_os(VARIABLE).is_some() {
        return;
    }
    let dossier = std::env::temp_dir().join(format!("mindcare-fin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    std::fs::create_dir_all(&dossier).expect("dossier d'essai");
    let mut enfant = Command::new(std::env::current_exe().expect("programme de test"))
        .args([ENFANT, "--exact", "--test-threads=1", "--nocapture"])
        .env(VARIABLE, &dossier)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .expect("processus à part");
    let debut = Instant::now();
    let fin = loop {
        if let Some(s) = enfant.try_wait().expect("attente") {
            break Some(s);
        }
        if debut.elapsed() > DELAI {
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if fin.is_none() {
        let _ = enfant.kill();
        let _ = enfant.wait();
    }
    let _ = std::fs::remove_dir_all(&dossier);
    let fin =
        fin.unwrap_or_else(|| panic!("le processus ne quitte pas en {} s (la fin reste bloquée)", DELAI.as_secs()));
    assert_eq!(fin.code(), Some(0), "le processus quitte, mais pas proprement : {fin:?}");
}
