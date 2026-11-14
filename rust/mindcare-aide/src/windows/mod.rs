//! Le seul module de l'Aide qui parle à Windows (et le seul où le code « unsafe » est permis, chaque bloc avec son
//! commentaire SAFETY). Il remplit les traits `Systeme` et `Hote` avec les vrais appels ; les règles et les actions
//! n'en savent rien. Le cadre (canal, verrou, processus, outils) est copié de l'Aide de Nevermind
//! (D:\Projets\Nevermind\rust\nevermind-aide\src\windows).
//!
//! Rien n'est lu chez l'utilisateur : ni variables d'environnement (les dossiers viennent de Windows), ni profil. Le
//! seul programme lancé (powercfg) l'est par son chemin dans System32, sans fenêtre, avec un environnement neuf fait
//! ici.

#![allow(unsafe_code)]

mod canal;
mod energie;
mod fichiers;
mod ip;
mod outils;
mod processus;
mod services;
mod verrou;
mod wifi;

use crate::systeme::{CarteWifi, Entree, ErreurCanal, Hote, Identite, Systeme, Verrouillage};
use mindcare_aide_regles::cartes::Carte;
use std::time::Duration;
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, LOAD_LIBRARY_SEARCH_SYSTEM32, SetDefaultDllDirectories,
};
use windows::Win32::System::SystemInformation::{GetLocalTime, GetSystemDirectoryW, GetSystemWindowsDirectoryW};

/// Prépare le processus avant toute chose : les DLL ne sont cherchées que dans System32, et le dossier courant devient
/// System32 (comme l'Aide de Nevermind).
pub fn preparer_processus() {
    // SAFETY : réglage du processus, avant tout autre appel
    unsafe {
        let _ = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
    }
    let _ = std::env::set_current_dir(system32());
}

/// Un dossier de Windows (System32 ou Windows), sans « \ » final
fn dossier(systeme: bool) -> String {
    let mut b = vec![0u16; 1024];
    // SAFETY : tampon et taille cohérents
    let n =
        unsafe { if systeme { GetSystemDirectoryW(Some(&mut b)) } else { GetSystemWindowsDirectoryW(Some(&mut b)) } }
            as usize;
    String::from_utf16_lossy(&b[..n.min(b.len())]).trim_end_matches('\\').to_string()
}

fn system32() -> String {
    dossier(true)
}

/// Le PC, tel que Windows le décrit
pub struct Windows {
    windows: String,
    system32: String,
    racine: String,
}

impl Windows {
    pub fn new() -> Windows {
        let mut e = vec![0u16; 32768];
        // SAFETY : tampon et taille cohérents (le chemin de cet exe)
        let n = unsafe { GetModuleFileNameW(None, &mut e) } as usize;
        let exe = String::from_utf16_lossy(&e[..n.min(e.len())]);
        let racine = match exe.rfind('\\') {
            Some(i) => exe[..i].to_string(),
            None => exe,
        };
        Windows { windows: dossier(false), system32: system32(), racine }
    }

    /// L'environnement du programme lancé par l'Aide (powercfg) : neuf, fait ici (rien de l'utilisateur)
    fn environnement(&self) -> Vec<(String, String)> {
        let w = &self.windows;
        vec![
            ("SystemRoot".to_string(), w.clone()),
            ("windir".to_string(), w.clone()),
            ("SystemDrive".to_string(), w.get(..2).unwrap_or("C:").to_string()),
            ("PATH".to_string(), format!("{0};{w};{0}\\Wbem", self.system32)),
        ]
    }
}

impl Default for Windows {
    fn default() -> Self {
        Windows::new()
    }
}

impl Systeme for Windows {
    fn identite(&self) -> Identite {
        let (compte, admin) = processus::moi();
        Identite { compte, admin, pid: std::process::id() }
    }

    fn racine(&self) -> String {
        self.racine.clone()
    }

    fn heure_locale(&self) -> String {
        // SAFETY : simple lecture de l'horloge
        let t = unsafe { GetLocalTime() };
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
    }

    fn pause(&self, d: Duration) {
        std::thread::sleep(d);
    }

    fn processus(&self) -> Vec<i64> {
        processus::liste()
    }

    fn chemin_processus(&self, pid: i64) -> String {
        processus::chemin(pid)
    }

    fn compte_processus(&self, pid: i64) -> String {
        processus::compte(pid)
    }

    fn wifi(&self) -> Result<Option<Vec<CarteWifi>>, String> {
        wifi::lire()
    }

    fn wifi_oublier(&self, carte: &str, profil: &str) -> Result<(), String> {
        wifi::oublier(carte, profil)
    }

    fn cartes(&self) -> Result<Vec<Carte>, String> {
        ip::cartes()
    }

    fn ip_rendre(&self, index: u32) -> Result<(), String> {
        ip::rendre(index)
    }

    fn ip_redemander(&self, index: u32, limite: Duration) -> Result<(), String> {
        ip::redemander(index, limite)
    }

    fn powercfg_par_defaut(&self) -> Result<i32, String> {
        energie::par_defaut(&self.system32, &self.environnement())
    }

    fn plan_actif(&self) -> Option<String> {
        energie::actif()
    }

    fn dossier_windows(&self) -> String {
        self.windows.clone()
    }

    fn service_tourne(&self, nom: &str) -> Result<bool, String> {
        services::tourne(nom)
    }

    fn service_arreter(&self, nom: &str) -> Result<(), String> {
        services::arreter(nom)
    }

    fn service_demarrer(&self, nom: &str) -> Result<(), String> {
        services::demarrer(nom)
    }

    fn entree(&self, chemin: &str) -> Entree {
        fichiers::entree(chemin)
    }

    fn supprimer_dossier(&self, chemin: &str) -> Result<(), String> {
        fichiers::supprimer(chemin)
    }

    fn renommer(&self, de: &str, vers: &str) -> Result<(), String> {
        fichiers::renommer(de, vers)
    }
}

impl Hote for Windows {
    type E = canal::EcouteWindows;

    fn verrou(&self, nom: &str) -> Verrouillage {
        verrou::verrou_aide(nom)
    }

    fn canal(&self, nom: &str, sddl: &str, premier: bool, proprietaire: Option<&str>) -> Result<Self::E, ErreurCanal> {
        canal::creer(nom, sddl, premier, proprietaire)
    }
}
