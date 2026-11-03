//! Les juges de la dictée : les cas de outils\tester-dictee.ps1 (Mindcare PowerShell), rejoués sur la crate.
//! - un faux micro (le test règle ce qu'il « entend ») : le vrai micro n'est JAMAIS ouvert ;
//! - un faux whisper-cli (le programme faux-whisper de cette crate, copié dans un dossier d'essai), lancé par le vrai
//!   lanceur (sans fenêtre, relié par le Job Object) ;
//! - tout se passe dans un dossier d'essai du dossier temporaire, effacé à la fin : ni les vraies données de Mindcare,
//!   ni le vrai dossier de la dictée, ni le vrai suite.json.
//!
//! Le fichier son est vérifié effacé dans tous les cas (phrase écrite, rien compris, whisper bloqué, annulation,
//! erreur), et les fichiers de voix orphelins vidés au démarrage.

#![cfg(windows)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use mindcare_dictee::windows::{LanceurWindows, processus_vivant};
use mindcare_dictee::{
    Appui, Dictee, DicteeEnFond, Etat, Lance, Lanceur, Materiel, Mesures, Micro, Reglages, messages, whisper,
};
use mindcare_telechargement::Paquet;

/// Le numéro de processus donné à la dictée dans les tests (pas le nôtre : les noms des fichiers restent à l'essai)
const PID: u32 = 4_000_000_001;
const WINDOWS: u32 = u32::MAX;

// ---------------------------------------------------------------------------------------------------------------
// Le dossier d'essai

struct Essai {
    racine: PathBuf,
}

impl Essai {
    fn nouveau(nom: &str) -> Essai {
        static N: AtomicUsize = AtomicUsize::new(0);
        let racine = std::env::temp_dir().join(format!(
            "MindcareDictee-{}-{nom}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&racine);
        std::fs::create_dir_all(racine.join("temp")).unwrap();
        std::fs::create_dir_all(racine.join("donnees")).unwrap();
        Essai { racine }
    }
    fn temp(&self) -> PathBuf {
        self.racine.join("temp")
    }
    fn donnees(&self) -> PathBuf {
        self.racine.join("donnees")
    }
    fn dossier_dictee(&self) -> PathBuf {
        self.donnees().join("dictee")
    }
    /// Le faux whisper installé comme la vraie dictée : whisper-cli.exe, ses trois .dll, le modèle
    fn installer(&self, consigne: &str) {
        installer_faux_whisper(&self.dossier_dictee(), consigne);
    }
    fn arguments(&self) -> String {
        std::fs::read_to_string(self.dossier_dictee().join("arguments.txt")).unwrap_or_default()
    }
    /// Les fichiers de voix encore là dans le dossier temporaire de l'essai
    fn fichiers_de_voix(&self) -> Vec<String> {
        std::fs::read_dir(self.temp())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("mindcare-"))
            .collect()
    }
    fn reglages(&self, suite_json: Option<PathBuf>) -> Reglages {
        Reglages { paquet: Paquet::dictee(&self.donnees()), suite_json, temp: self.temp(), pid: PID, fils: 2 }
    }
}

impl Drop for Essai {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.racine);
    }
}

fn installer_faux_whisper(dossier: &Path, consigne: &str) {
    std::fs::create_dir_all(dossier).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_faux-whisper"), dossier.join("whisper-cli.exe")).unwrap();
    for d in ["whisper.dll", "ggml.dll", "ggml-base.dll", "ggml-small-q5_1.bin"] {
        std::fs::write(dossier.join(d), "faux").unwrap();
    }
    std::fs::write(dossier.join("consigne.txt"), consigne).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------
// Le faux micro

#[derive(Default)]
struct FauxEtat {
    mesures: Mesures,
    demarre: Option<u32>,
    arrete: bool,
}

#[derive(Clone)]
struct FauxMicro(Arc<Mutex<FauxEtat>>, u32);

impl Micro for FauxMicro {
    fn demarrer(&mut self, numero: u32) -> Result<(), u32> {
        self.0.lock().unwrap().demarre = Some(numero);
        if self.1 == 0 { Ok(()) } else { Err(self.1) }
    }
    fn mesures(&self) -> Mesures {
        self.0.lock().unwrap().mesures
    }
    fn arreter(&mut self) {
        self.0.lock().unwrap().arrete = true;
    }
    fn son(&self) -> Vec<u8> {
        vec![1; 3200]
    }
}

/// Les faux micros du PC : leurs noms, présents ou non, le code rendu au démarrage, et le dernier micro créé
#[derive(Clone)]
struct FauxMateriel {
    present: bool,
    noms: Vec<String>,
    code: u32,
    dernier: Arc<Mutex<Option<FauxMicro>>>,
}

impl FauxMateriel {
    fn nouveau() -> FauxMateriel {
        FauxMateriel { present: true, noms: vec!["Micro du casque".into()], code: 0, dernier: Default::default() }
    }
    fn micro(&self) -> Arc<Mutex<FauxEtat>> {
        self.dernier.lock().unwrap().as_ref().expect("un micro créé").0.clone()
    }
    fn entendre(&self, millis: u32, entendu: bool, depuis: u32) {
        let m = self.micro();
        let mut m = m.lock().unwrap();
        m.mesures.millis = millis;
        m.mesures.entendu = entendu;
        m.mesures.millis_depuis_voix = depuis;
        m.mesures.niveau = 0.2;
    }
}

impl Materiel for FauxMateriel {
    fn micro_present(&self) -> bool {
        self.present
    }
    fn noms(&self) -> Vec<String> {
        self.noms.clone()
    }
    fn nouveau_micro(&self) -> Box<dyn Micro> {
        let m = FauxMicro(Default::default(), self.code);
        *self.dernier.lock().unwrap() = Some(m.clone());
        Box::new(m)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Le lanceur : le vrai (sans fenêtre, Job Object), qui note ce qu'il lance ; ou un qui échoue

#[derive(Clone, Default)]
struct Notes {
    lances: Arc<AtomicUsize>,
    pids: Arc<Mutex<Vec<u32>>>,
    relies: Arc<Mutex<Vec<bool>>>,
}

struct LanceurNote {
    notes: Notes,
    echec: bool,
    vivants: Vec<u32>,
}

impl Lanceur for LanceurNote {
    fn lancer(&self, programme: &Path, arguments: &[OsString], dossier: &Path) -> Result<Lance, String> {
        self.notes.lances.fetch_add(1, Ordering::SeqCst);
        if self.echec {
            return Err("faux échec du lancement".into());
        }
        let l = LanceurWindows.lancer(programme, arguments, dossier)?;
        self.notes.pids.lock().unwrap().push(l.processus.numero());
        self.notes.relies.lock().unwrap().push(l.relie.is_ok());
        Ok(l)
    }
    fn vivant(&self, pid: u32) -> bool {
        self.vivants.contains(&pid)
    }
}

fn dictee(essai: &Essai, materiel: &FauxMateriel, suite_json: Option<PathBuf>) -> (Dictee, Notes) {
    let notes = Notes::default();
    let lanceur = LanceurNote { notes: notes.clone(), echec: false, vivants: Vec::new() };
    (Dictee::nouvelle(essai.reglages(suite_json), Box::new(materiel.clone()), Box::new(lanceur)), notes)
}

/// Fait avancer la dictée jusqu'à ce que whisper ait fini (15 secondes au plus)
fn attendre_whisper(d: &mut Dictee) {
    let limite = Instant::now() + Duration::from_secs(15);
    while d.occupee() && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(30));
        d.etape();
    }
    assert!(!d.occupee(), "whisper n'a pas fini");
}

/// Attend qu'un processus soit parti (5 secondes au plus)
fn parti(pid: u32) -> bool {
    let limite = Instant::now() + Duration::from_secs(5);
    while processus_vivant(pid) && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(50));
    }
    !processus_vivant(pid)
}

// ---------------------------------------------------------------------------------------------------------------
// 1. Pas installée

#[test]
fn pas_installee_le_bouton_propose_l_installation() {
    let essai = Essai::nouveau("pas-installee");
    let materiel = FauxMateriel::nouveau();
    let (mut d, notes) = dictee(&essai, &materiel, None);
    assert_eq!(d.etat(), Etat::PasInstallee);
    assert_eq!(d.appui(), Appui::Installer);
    assert!(materiel.dernier.lock().unwrap().is_none(), "aucun micro ouvert");
    // Installation coupée (programme sans ses .dll) : toujours pas installée (« Réparer »)
    std::fs::create_dir_all(essai.dossier_dictee()).unwrap();
    std::fs::write(essai.dossier_dictee().join("whisper-cli.exe"), "faux").unwrap();
    std::fs::write(essai.dossier_dictee().join("ggml-small-q5_1.bin"), "faux").unwrap();
    assert!(Paquet::dictee(&essai.donnees()).incomplet());
    assert_eq!(d.appui(), Appui::Installer);
    assert_eq!(notes.lances.load(Ordering::SeqCst), 0);
    // Installée : prête
    essai.installer("");
    assert!(d.installee());
    assert_eq!(d.etat(), Etat::Prete);
}

// ---------------------------------------------------------------------------------------------------------------
// 2. On parle puis on se tait

#[test]
fn on_parle_puis_on_se_tait_la_phrase_est_ecrite_et_le_son_efface() {
    let essai = Essai::nouveau("phrase");
    essai.installer(" [Musique] Mon PC s'éteint quand je joue. \r\n");
    let materiel = FauxMateriel::nouveau();
    let (mut d, notes) = dictee(&essai, &materiel, None);
    let t0 = Instant::now();
    assert_eq!(d.appui_a(t0), Appui::Ecoute);
    assert_eq!(materiel.micro().lock().unwrap().demarre, Some(WINDOWS), "le micro de Windows si rien n'est choisi");
    assert!(matches!(d.etat(), Etat::Ecoute { .. }));

    materiel.entendre(2000, true, 400);
    d.etape_a(t0 + Duration::from_millis(2000));
    assert!(d.ecoute(), "on attend une seconde de silence");
    assert!(matches!(d.etat(), Etat::Ecoute { niveau } if niveau == 0.2));
    materiel.entendre(2600, true, 1100);
    d.etape_a(t0 + Duration::from_millis(2600));
    assert!(!d.ecoute());
    assert!(materiel.micro().lock().unwrap().arrete, "le micro est arrêté");
    assert_eq!(d.etat(), Etat::Ecrit);
    // Le son est là le temps que whisper le lise : un .wav de 16 kHz
    let son = std::fs::read(d.fichier_son()).expect("le son pour whisper");
    assert_eq!(&son[..4], b"RIFF");
    assert_eq!(son.len(), 44 + 3200);

    attendre_whisper(&mut d);
    assert_eq!(d.etat(), Etat::PhrasePrete("Mon PC s'éteint quand je joue.".into()));
    assert!(essai.fichiers_de_voix().is_empty(), "son et texte effacés : {:?}", essai.fichiers_de_voix());

    let a = essai.arguments();
    let a: Vec<&str> = a.lines().collect();
    let modele = essai.dossier_dictee().join("ggml-small-q5_1.bin");
    assert!(a.windows(2).any(|w| w == ["-l", "fr"]), "{a:?}");
    assert!(a.windows(2).any(|w| w[0] == "-m" && Path::new(w[1]) == modele), "{a:?}");
    assert!(a.contains(&"-nt") && a.contains(&"-np") && a.contains(&"-otxt"), "{a:?}");
    assert!(a.windows(2).any(|w| w == ["-t", "2"]), "{a:?}");
    assert!(a.windows(2).any(|w| w == ["--prompt", whisper::ATTENDU]), "{a:?}");
    assert!(a.windows(2).any(|w| w[0] == "-of" && Path::new(w[1]) == whisper::sortie(&essai.temp(), PID)), "{a:?}");
    assert_eq!(*notes.relies.lock().unwrap(), [true], "whisper relié à Mindcare (Job Object)");

    let journal = d.prendre_journal().join(" | ");
    assert!(!journal.contains("éteint"), "la phrase n'est pas dans le journal : {journal}");
    assert!(journal.contains(" s, 30 caractères"), "{journal}");
    assert!(!journal.contains("code"), "{journal}");

    assert_eq!(d.prendre_phrase().as_deref(), Some("Mon PC s'éteint quand je joue."));
    assert_eq!(d.prendre_phrase(), None, "une seule fois");
    assert_eq!(d.etat(), Etat::Prete);
}

// ---------------------------------------------------------------------------------------------------------------
// 3. Deuxième appui

#[test]
fn deuxieme_appui_arrete_l_ecoute_et_la_phrase_arrive() {
    let essai = Essai::nouveau("deuxieme-appui");
    essai.installer("le wifi coupe");
    let materiel = FauxMateriel::nouveau();
    let (mut d, _) = dictee(&essai, &materiel, None);
    assert_eq!(d.appui(), Appui::Ecoute);
    materiel.entendre(1500, true, 0);
    assert_eq!(d.appui(), Appui::Fin);
    assert!(!d.ecoute());
    assert_eq!(d.etat(), Etat::Ecrit);
    attendre_whisper(&mut d);
    assert_eq!(d.prendre_phrase().as_deref(), Some("le wifi coupe"));
    assert!(essai.fichiers_de_voix().is_empty());
}

// ---------------------------------------------------------------------------------------------------------------
// 4. Rien entendu, micro débranché, micro refusé, pas de micro

#[test]
fn rien_entendu_en_7_secondes_whisper_n_est_pas_lance() {
    let essai = Essai::nouveau("rien-entendu");
    essai.installer("ne doit pas servir");
    let materiel = FauxMateriel::nouveau();
    let (mut d, notes) = dictee(&essai, &materiel, None);
    let t0 = Instant::now();
    d.appui_a(t0);
    materiel.entendre(6900, false, 0);
    d.etape_a(t0 + Duration::from_millis(6900));
    assert!(d.ecoute());
    materiel.entendre(7100, false, 0);
    d.etape_a(t0 + Duration::from_millis(7100));
    assert!(!d.occupee());
    assert_eq!(d.etat(), Etat::Erreur(messages::RIEN_ENTENDU.into()));
    assert_eq!(notes.lances.load(Ordering::SeqCst), 0);
    assert!(essai.fichiers_de_voix().is_empty());
    assert!(d.prendre_journal().contains(&"Dictée : rien entendu".to_string()));
    d.effacer_message();
    assert_eq!(d.etat(), Etat::Prete);
}

#[test]
fn micro_debranche_pendant_l_ecoute() {
    let essai = Essai::nouveau("micro-debranche");
    essai.installer("");
    let materiel = FauxMateriel::nouveau();
    let (mut d, notes) = dictee(&essai, &materiel, None);
    let t0 = Instant::now();
    d.appui_a(t0);
    d.etape_a(t0 + Duration::from_millis(2900));
    assert!(d.ecoute(), "pas encore 3 secondes sans son");
    d.etape_a(t0 + Duration::from_millis(3200));
    assert!(!d.occupee());
    assert_eq!(d.etat(), Etat::Erreur(messages::MICRO_PERDU.into()));
    assert!(messages::MICRO_PERDU.contains("entends plus le micro"));
    assert_eq!(notes.lances.load(Ordering::SeqCst), 0);
}

#[test]
fn micro_refuse_par_windows() {
    let essai = Essai::nouveau("micro-refuse");
    essai.installer("");
    let mut materiel = FauxMateriel::nouveau();
    materiel.code = 4; // (le code de Windows quand le micro est refusé ou occupé)
    let (mut d, _) = dictee(&essai, &materiel, None);
    let r = d.appui();
    assert_eq!(r, Appui::Erreur(messages::MICRO_REFUSE.into()));
    assert!(messages::MICRO_REFUSE.contains("Confidentialité, Microphone"));
    assert!(!d.ecoute());
    assert!(materiel.micro().lock().unwrap().arrete, "le micro est relâché");
    assert_eq!(d.etat(), Etat::Erreur(messages::MICRO_REFUSE.into()));
    assert!(d.prendre_journal().contains(&"Dictée : micro indisponible (code 4)".to_string()));
}

#[test]
fn aucun_micro_sur_le_pc() {
    let essai = Essai::nouveau("aucun-micro");
    essai.installer("");
    let mut materiel = FauxMateriel::nouveau();
    materiel.present = false;
    let (mut d, _) = dictee(&essai, &materiel, None);
    assert_eq!(d.appui(), Appui::Erreur(messages::PAS_DE_MICRO.into()));
    assert!(materiel.dernier.lock().unwrap().as_ref().is_none_or(|m| m.0.lock().unwrap().demarre.is_none()));
    assert!(!d.ecoute());
}

#[test]
fn au_bout_de_25_secondes_de_son_ou_30_en_tout() {
    let essai = Essai::nouveau("duree-max");
    essai.installer("la souris");
    let materiel = FauxMateriel::nouveau();
    let (mut d, _) = dictee(&essai, &materiel, None);
    let t0 = Instant::now();
    d.appui_a(t0);
    materiel.entendre(25_000, true, 0);
    d.etape_a(t0 + Duration::from_millis(25_000));
    assert!(d.ecoute());
    materiel.entendre(25_100, true, 0);
    d.etape_a(t0 + Duration::from_millis(25_100));
    assert!(!d.ecoute(), "25 secondes de son");
    attendre_whisper(&mut d);
    assert_eq!(d.prendre_phrase().as_deref(), Some("la souris"));
    // (la limite de 30 secondes en tout ne peut jouer seule : au-delà de 3 secondes sans son, le micro est perdu)
}

// ---------------------------------------------------------------------------------------------------------------
// 5. Le micro choisi dans Mindset

#[test]
fn micro_choisi_dans_mindset_par_son_nom_sinon_celui_de_windows() {
    let essai = Essai::nouveau("micro-choisi");
    essai.installer("");
    let suite = essai.racine.join("suite.json");
    let mut materiel = FauxMateriel::nouveau();
    materiel.noms = vec!["Réseau de microphones".into(), "Micro USB".into()];
    let (mut d, _) = dictee(&essai, &materiel, Some(suite.clone()));
    // suite.json absent : celui de Windows
    assert_eq!(d.micro_choisi(), WINDOWS);
    // Pris par son nom
    std::fs::write(&suite, r#"{"theme":"neon","micro":"Micro USB"}"#).unwrap();
    assert_eq!(d.micro_choisi(), 1);
    assert_eq!(d.appui(), Appui::Ecoute);
    assert_eq!(materiel.micro().lock().unwrap().demarre, Some(1));
    d.annuler();
    // Choisi mais débranché : celui de Windows, et une ligne au journal
    std::fs::write(&suite, r#"{"micro":"Micro qui n'existe pas"}"#).unwrap();
    d.prendre_journal();
    assert_eq!(d.micro_choisi(), WINDOWS);
    assert!(d.prendre_journal().iter().any(|l| l.contains("« Micro qui n'existe pas »") && l.contains("pas branché")));
    // Celui de Windows choisi exprès, ou un suite.json abîmé
    std::fs::write(&suite, r#"{"micro":""}"#).unwrap();
    assert_eq!(d.micro_choisi(), WINDOWS);
    std::fs::write(&suite, "{abîmé").unwrap();
    assert_eq!(d.micro_choisi(), WINDOWS);
    // (le nom exact, mêmes majuscules)
    std::fs::write(&suite, r#"{"micro":"micro usb"}"#).unwrap();
    assert_eq!(d.micro_choisi(), WINDOWS);
}

// ---------------------------------------------------------------------------------------------------------------
// 6. whisper n'écrit rien de sensé, sort avec une erreur, est bloqué, ne démarre pas

#[test]
fn whisper_n_ecrit_rien_de_sense() {
    let essai = Essai::nouveau("rien-de-sense");
    essai.installer("Sous-titrage ST' 501");
    let materiel = FauxMateriel::nouveau();
    let (mut d, _) = dictee(&essai, &materiel, None);
    d.appui();
    materiel.entendre(1500, true, 1200);
    d.etape();
    attendre_whisper(&mut d);
    assert_eq!(d.etat(), Etat::Erreur(messages::PAS_COMPRIS.into()));
    assert!(essai.fichiers_de_voix().is_empty());
    assert!(d.prendre_journal().iter().any(|l| l.contains(" s, 0 caractères")));
}

#[test]
fn whisper_sort_avec_un_code_d_erreur() {
    let essai = Essai::nouveau("code");
    essai.installer("code 3 la souris");
    let materiel = FauxMateriel::nouveau();
    let (mut d, _) = dictee(&essai, &materiel, None);
    d.appui();
    materiel.entendre(1500, true, 1200);
    d.etape();
    attendre_whisper(&mut d);
    assert_eq!(d.prendre_phrase().as_deref(), Some("la souris"));
    assert!(d.prendre_journal().iter().any(|l| l.ends_with(" s, 9 caractères (code 3)")));
}

#[test]
fn whisper_en_cours_un_nouvel_appui_ne_relance_rien_et_mindcare_qui_se_ferme_l_arrete() {
    let essai = Essai::nouveau("whisper-bloque");
    essai.installer("attendre");
    let materiel = FauxMateriel::nouveau();
    let (mut d, notes) = dictee(&essai, &materiel, None);
    d.appui();
    materiel.entendre(1500, true, 1200);
    d.etape();
    assert_eq!(d.etat(), Etat::Ecrit);
    assert_eq!(d.appui(), Appui::Occupee);
    assert_eq!(notes.lances.load(Ordering::SeqCst), 1);
    let pid = notes.pids.lock().unwrap()[0];
    assert!(processus_vivant(pid));
    assert!(!essai.fichiers_de_voix().is_empty(), "le son attend whisper");
    drop(d); // (Mindcare se ferme : Stop-Dictees)
    assert!(parti(pid), "whisper arrêté");
    assert!(essai.fichiers_de_voix().is_empty(), "son effacé : {:?}", essai.fichiers_de_voix());
}

#[test]
fn quitter_l_accueil_pendant_que_whisper_ecrit() {
    let essai = Essai::nouveau("annuler-ecriture");
    essai.installer("attendre");
    let materiel = FauxMateriel::nouveau();
    let (mut d, notes) = dictee(&essai, &materiel, None);
    d.appui();
    materiel.entendre(1500, true, 1200);
    d.etape();
    let pid = notes.pids.lock().unwrap()[0];
    d.annuler();
    assert!(parti(pid), "whisper arrêté");
    assert!(essai.fichiers_de_voix().is_empty());
    assert_eq!(d.etat(), Etat::Prete);
    assert_eq!(d.prendre_phrase(), None, "aucune phrase n'arrive après");
}

#[test]
fn quitter_l_accueil_pendant_l_ecoute() {
    let essai = Essai::nouveau("annuler-ecoute");
    essai.installer("");
    let materiel = FauxMateriel::nouveau();
    let (mut d, notes) = dictee(&essai, &materiel, None);
    d.appui();
    materiel.entendre(1500, true, 300);
    d.annuler();
    assert!(!d.occupee());
    assert!(materiel.micro().lock().unwrap().arrete);
    assert_eq!(notes.lances.load(Ordering::SeqCst), 0, "whisper n'est pas lancé");
    assert!(essai.fichiers_de_voix().is_empty());
    assert_eq!(d.etat(), Etat::Prete);
}

#[test]
fn whisper_trop_long_arrete_au_bout_de_60_secondes() {
    let essai = Essai::nouveau("trop-long");
    essai.installer("attendre");
    let materiel = FauxMateriel::nouveau();
    let (mut d, notes) = dictee(&essai, &materiel, None);
    let t0 = Instant::now();
    d.appui_a(t0);
    materiel.entendre(1500, true, 1200);
    d.etape_a(t0);
    d.etape_a(t0 + Duration::from_secs(59));
    assert!(d.occupee());
    d.etape_a(t0 + Duration::from_secs(61));
    assert!(!d.occupee());
    let pid = notes.pids.lock().unwrap()[0];
    assert!(parti(pid));
    assert_eq!(d.etat(), Etat::Erreur(messages::PAS_COMPRIS.into()));
    assert!(essai.fichiers_de_voix().is_empty());
    let journal = d.prendre_journal();
    assert!(journal.contains(&"Dictée : Whisper trop long, arrêté".to_string()), "{journal:?}");
}

#[test]
fn whisper_ne_demarre_pas() {
    let essai = Essai::nouveau("ne-demarre-pas");
    essai.installer("");
    let materiel = FauxMateriel::nouveau();
    let notes = Notes::default();
    let lanceur = LanceurNote { notes: notes.clone(), echec: true, vivants: Vec::new() };
    let mut d = Dictee::nouvelle(essai.reglages(None), Box::new(materiel.clone()), Box::new(lanceur));
    d.appui();
    materiel.entendre(1500, true, 1200);
    d.etape();
    assert_eq!(notes.lances.load(Ordering::SeqCst), 1);
    assert!(!d.occupee());
    assert_eq!(d.etat(), Etat::Erreur(messages::PAS_DEMARRE.into()));
    assert!(essai.fichiers_de_voix().is_empty(), "son effacé");
    assert!(d.prendre_journal().iter().any(|l| l.starts_with("Dictée : Whisper ne démarre pas (")));
}

#[test]
fn le_vrai_lanceur_sans_programme_rend_une_erreur() {
    let essai = Essai::nouveau("sans-programme");
    let r = LanceurWindows.lancer(&essai.racine.join("absent.exe"), &[], &essai.racine);
    assert!(r.is_err());
}

#[test]
fn son_impossible_a_garder() {
    let essai = Essai::nouveau("son-pas-garde");
    essai.installer("");
    let materiel = FauxMateriel::nouveau();
    let mut reglages = essai.reglages(None);
    reglages.temp = essai.racine.join("dossier-absent");
    let notes = Notes::default();
    let lanceur = LanceurNote { notes: notes.clone(), echec: false, vivants: Vec::new() };
    let mut d = Dictee::nouvelle(reglages, Box::new(materiel.clone()), Box::new(lanceur));
    d.appui();
    materiel.entendre(1500, true, 1200);
    d.etape();
    assert_eq!(d.etat(), Etat::Erreur(messages::PAS_MARCHE.into()));
    assert_eq!(notes.lances.load(Ordering::SeqCst), 0);
    assert!(d.prendre_journal().iter().any(|l| l.starts_with("Dictée : son non gardé (")));
}

// ---------------------------------------------------------------------------------------------------------------
// 7. Arrêté avec Mindcare, même quand elle s'arrête d'un coup (Job Object)

#[test]
fn whisper_s_arrete_avec_mindcare() {
    let essai = Essai::nouveau("job");
    // « Mindcare » : un faux whisper qui lance un autre faux whisper (bloqué) comme la dictée, puis s'arrête d'un coup
    let mindcare = essai.racine.join("mindcare");
    let enfant = essai.racine.join("enfant");
    installer_faux_whisper(&enfant, "attendre");
    installer_faux_whisper(&mindcare, &format!("lancer {}", enfant.display()));
    let mut m = LanceurWindows.lancer(&mindcare.join("whisper-cli.exe"), &[], &mindcare).unwrap();
    let limite = Instant::now() + Duration::from_secs(15);
    while m.processus.fini().is_none() && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(m.processus.fini().is_some(), "« Mindcare » s'est arrêtée");
    let note = std::fs::read_to_string(mindcare.join("pid.txt")).unwrap();
    let (pid, relie) = note.split_once(' ').unwrap();
    assert_eq!(relie, "relie", "{note}");
    let pid: u32 = pid.parse().unwrap();
    assert!(pid != 0);
    assert!(parti(pid), "le whisper lancé par « Mindcare » s'est arrêté avec elle");
}

// ---------------------------------------------------------------------------------------------------------------
// 8. Fichiers de voix orphelins (constat 25n)

#[test]
fn fichiers_de_voix_orphelins_effaces_au_demarrage() {
    let essai = Essai::nouveau("orphelins");
    essai.installer("");
    let t = essai.temp();
    let ecrire = |nom: &str| std::fs::write(t.join(nom), "voix").unwrap();
    ecrire(&format!("mindcare-{PID}-dictee.wav")); // le nôtre : forcément d'avant
    ecrire("mindcare-123-dictee.wav"); // processus parti
    ecrire("mindcare-123-dictee.txt");
    ecrire("mindcare-456-dictee.wav"); // une autre Mindcare encore ouverte, récent : laissé
    ecrire("mindcare-789-dictee.wav"); // processus vivant mais fichier de plus de 10 minutes
    let vieux = SystemTime::now() - Duration::from_secs(11 * 60);
    std::fs::File::options().write(true).open(t.join("mindcare-789-dictee.wav")).unwrap().set_modified(vieux).unwrap();
    ecrire("mindcare-batterie-123.xml"); // pas un fichier de voix
    ecrire("autre-123-dictee.wav");
    std::fs::create_dir_all(t.join("mindcare-321-dictee.wav")).unwrap(); // un dossier : laissé

    let lanceur = LanceurNote { notes: Notes::default(), echec: false, vivants: vec![PID, 456, 789] };
    // (notre propre numéro est toujours vivant : c'est nous ; son fichier est pourtant d'avant)
    let mut d = Dictee::nouvelle(essai.reglages(None), Box::new(FauxMateriel::nouveau()), Box::new(lanceur));
    let mut restent = essai.fichiers_de_voix();
    restent.sort();
    assert_eq!(restent, ["mindcare-321-dictee.wav", "mindcare-456-dictee.wav", "mindcare-batterie-123.xml"]);
    assert!(t.join("autre-123-dictee.wav").exists());
    let journal = d.prendre_journal();
    assert!(journal.iter().any(|l| l.starts_with("Dictée : 4 fichier(s) de voix")), "{journal:?}");
}

#[test]
fn processus_vivant_lit_seulement() {
    assert!(processus_vivant(std::process::id()));
    assert!(!processus_vivant(4_000_000_001));
}

// ---------------------------------------------------------------------------------------------------------------
// 9. La dictée sur son fil, comme la verront les pages

#[test]
fn dictee_en_fond_de_l_appui_a_la_phrase() {
    let essai = Essai::nouveau("en-fond");
    essai.installer("l'écran reste noir");
    let materiel = FauxMateriel::nouveau();
    let (d, _) = dictee(&essai, &materiel, None);
    let fond = DicteeEnFond::lancer(d);
    assert_eq!(fond.etat(), Etat::Prete);
    assert_eq!(fond.appui(), Appui::Ecoute);
    materiel.entendre(2000, true, 1200); // (le fil voit la fin de la phrase à sa prochaine étape)
    let limite = Instant::now() + Duration::from_secs(15);
    while !matches!(fond.etat(), Etat::PhrasePrete(_)) && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(fond.prendre_phrase().as_deref(), Some("l'écran reste noir"));
    assert_eq!(fond.etat(), Etat::Prete);
    assert!(fond.prendre_journal().iter().any(|l| l == "Dictée : écoute"));
    assert!(essai.fichiers_de_voix().is_empty());
}

#[test]
fn dictee_en_fond_lachee_pendant_que_whisper_ecrit() {
    let essai = Essai::nouveau("en-fond-lachee");
    essai.installer("attendre");
    let materiel = FauxMateriel::nouveau();
    let (d, notes) = dictee(&essai, &materiel, None);
    let fond = DicteeEnFond::lancer(d);
    fond.appui();
    materiel.entendre(2000, true, 1200);
    let limite = Instant::now() + Duration::from_secs(15);
    while fond.etat() != Etat::Ecrit && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(fond.etat(), Etat::Ecrit);
    let pid = notes.pids.lock().unwrap()[0];
    drop(fond);
    assert!(parti(pid));
    assert!(essai.fichiers_de_voix().is_empty());
}
