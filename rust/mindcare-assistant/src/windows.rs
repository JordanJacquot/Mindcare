//! Le seul module qui parle à Windows (et le seul où le code « unsafe » est permis) :
//! - [`LanceurWindows`] lance llama-server SANS fenêtre (CREATE_NO_WINDOW), ses sorties lues sur des fils à part
//!   (sinon ses messages le bloqueraient), et le met dans un Job Object « kill on close » : quand le processus de
//!   Mindcare disparaît, même tué, Windows ferme le job et arrête llama-server (`ArreterAvecMindcare`, natif.cs) ;
//! - la clé : 32 octets de `BCryptGenRandom` (le générateur du système), écrits en hexadécimal.

#![allow(unsafe_code)]

use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows::core::PCWSTR;

use crate::serveur::{DernieresLignes, Lance, Lanceur, Ordre, Processus};

/// CREATE_NO_WINDOW : aucune fenêtre de console, même un instant
pub const SANS_FENETRE: u32 = 0x0800_0000;

/// Le job de Mindcare, créé une fois (la poignée, gardée comme un nombre : HANDLE ne passe pas d'un fil à l'autre).
/// Jamais fermé à la main : Windows le ferme quand Mindcare s'arrête, et arrête alors tout ce qu'il contient.
static JOB: OnceLock<Result<isize, String>> = OnceLock::new();

fn job() -> Result<HANDLE, String> {
    let j = JOB.get_or_init(|| {
        // SAFETY : aucun attribut de sécurité (la poignée n'est donc pas héritable : llama-server ne la garde pas, sinon
        // le job ne se fermerait jamais) et aucun nom (un job à nous seuls). La poignée rendue est valide ou une erreur.
        let j = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(|e| format!("création du job ({e})"))?;
        let mut l = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        l.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY : `j` est le job valide créé juste avant ; le pointeur et la taille sont ceux d'une structure
        // JOBOBJECT_EXTENDED_LIMIT_INFORMATION vivante pendant tout l'appel, la classe annoncée est la bonne.
        unsafe {
            SetInformationJobObject(
                j,
                JobObjectExtendedLimitInformation,
                (&raw const l).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(|e| format!("réglage du job ({e})"))?;
        Ok(j.0 as isize)
    });
    j.clone().map(|h| HANDLE(h as *mut _))
}

/// Met ce programme dans le job de Mindcare : il s'arrêtera avec elle
fn relier_a_mindcare(enfant: &Child) -> Result<(), String> {
    let j = job()?;
    // SAFETY : `j` est le job valide (jamais fermé tant que Mindcare tourne) ; la poignée du processus appartient à
    // `enfant`, vivant pendant tout l'appel (std l'ouvre avec PROCESS_ALL_ACCESS, dont PROCESS_SET_QUOTA et
    // PROCESS_TERMINATE). Un processus déjà dans un autre job (Windows 8 et plus) peut être mis dans le nôtre.
    unsafe { AssignProcessToJobObject(j, HANDLE(enfant.as_raw_handle())) }.map_err(|e| format!("ajout au job ({e})"))
}

/// Une clé neuve : 32 octets aléatoires du système, en hexadécimal (64 caractères)
pub fn cle_aleatoire() -> Result<String, String> {
    let mut octets = [0u8; 32];
    // SAFETY : aucun algorithme (le générateur préféré du système, BCRYPT_USE_SYSTEM_PREFERRED_RNG) ; le tampon est
    // un tableau à nous, valide et modifiable pendant tout l'appel, sa taille est passée par la crate windows.
    let r = unsafe { BCryptGenRandom(None, &mut octets, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if r.is_err() {
        return Err(format!("pas de clé aléatoire (code {:#x})", r.0));
    }
    Ok(octets.iter().map(|o| format!("{o:02x}")).collect())
}

/// Lance llama-server comme Start-Ia : sans fenêtre, ses sorties lues, relié à Mindcare
pub struct LanceurWindows;

impl Lanceur for LanceurWindows {
    fn lancer(&self, ordre: &Ordre) -> Result<Lance, String> {
        let mut c = Command::new(&ordre.programme);
        c.args(&ordre.arguments)
            .envs(ordre.environnement.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(SANS_FENETRE);
        if let Some(d) = ordre.programme.parent() {
            c.current_dir(d);
        }
        let mut enfant = c.spawn().map_err(|e| e.to_string())?;
        // (arrêté avec Mindcare, même si elle est fermée de force : sinon il resterait lancé, sans fenêtre, jusqu'au
        // redémarrage)
        let avertissement = relier_a_mindcare(&enfant).err().map(|e| format!("pas relié à Mindcare ({e})"));
        let lignes = DernieresLignes::default();
        let mut lecteurs = Vec::new();
        if let Some(s) = enfant.stdout.take() {
            lecteurs.extend(lignes.lire(s));
        }
        if let Some(s) = enfant.stderr.take() {
            lecteurs.extend(lignes.lire(s));
        }
        Ok(Lance { processus: Box::new(ProcessusWindows { enfant, lignes, lecteurs, code: None }), avertissement })
    }

    fn cle(&self) -> Result<String, String> {
        cle_aleatoire()
    }
}

struct ProcessusWindows {
    enfant: Child,
    lignes: DernieresLignes,
    lecteurs: Vec<JoinHandle<()>>,
    code: Option<Option<i32>>,
}

impl ProcessusWindows {
    /// Ses sorties lues jusqu'au bout (2 secondes au plus : un programme qu'il aurait lancé peut les garder ouvertes)
    fn finir_lecture(&mut self) {
        let fin = Instant::now() + Duration::from_secs(2);
        while Instant::now() < fin && self.lecteurs.iter().any(|l| !l.is_finished()) {
            std::thread::sleep(Duration::from_millis(10));
        }
        for l in std::mem::take(&mut self.lecteurs) {
            if l.is_finished() {
                let _ = l.join();
            }
        }
    }
}

impl Processus for ProcessusWindows {
    fn id(&self) -> u32 {
        self.enfant.id()
    }

    fn fin(&mut self) -> Option<Option<i32>> {
        if self.code.is_none()
            && let Ok(Some(s)) = self.enfant.try_wait()
        {
            self.finir_lecture();
            self.code = Some(s.code());
        }
        self.code
    }

    fn arreter(&mut self) {
        if self.fin().is_some() {
            return;
        }
        let _ = self.enfant.kill();
        let fin = Instant::now() + Duration::from_secs(3);
        while Instant::now() < fin {
            if let Ok(Some(s)) = self.enfant.try_wait() {
                self.code = Some(s.code());
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        self.finir_lecture();
    }

    fn dernieres_lignes(&mut self) -> Vec<String> {
        self.lignes.toutes()
    }
}

impl Drop for ProcessusWindows {
    fn drop(&mut self) {
        // (jamais laissé tourner sans personne pour l'arrêter)
        self.arreter();
    }
}
