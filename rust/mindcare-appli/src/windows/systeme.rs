//! Le trait `Systeme` de mindcare-vie rempli avec Windows : le verrou et le signal ([`super::synchro`]), le
//! dossier de Mindset dans HKLM ([`super::registre`]), et le lancement d'un programme.
//!
//! Construction d'essai (option cargo `essai`) : HKLM n'est jamais lu (Mindset est « introuvable ») et seul ce
//! programme-ci peut être lancé (une relance après un changement de thème). Une copie d'essai ne peut donc jamais
//! ouvrir le vrai Mindset du PC.

use std::io;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use mindcare_vie::{AttenteVerrou, Systeme};

use super::synchro::{self, Signal, Verrou};

/// Windows, pour la vie de l'appli. Le verrou et le signal sont gardés jusqu'à la fin du processus (ou jusqu'à une
/// relance).
#[derive(Debug, Default)]
pub struct SystemeWindows {
    verrou: Option<Verrou>,
    signal: Option<Signal>,
}

impl SystemeWindows {
    pub fn nouveau() -> SystemeWindows {
        SystemeWindows::default()
    }
}

/// Construction d'essai : `exe` est-il ce programme-ci ?
#[cfg(feature = "essai")]
fn est_ce_programme(exe: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).ok();
    let moi = std::env::current_exe().ok().and_then(|e| canon(&e));
    moi.is_some() && canon(exe) == moi
}

impl Systeme for SystemeWindows {
    fn creer_verrou(&mut self, nom: &str) -> bool {
        match Verrou::creer(nom) {
            Some((v, cree)) => {
                self.verrou = Some(v);
                cree
            }
            // (Windows refuse : tenu par un programme que nous ne pouvons pas ouvrir, donc par un autre)
            None => false,
        }
    }

    fn attendre_verrou(&mut self, delai: Duration) -> AttenteVerrou {
        self.verrou.as_ref().map_or(AttenteVerrou::Tenu, |v| v.attendre(delai))
    }

    fn liberer_verrou(&mut self) {
        if let Some(v) = self.verrou.take() {
            v.liberer();
        }
    }

    fn lever_signal(&mut self, nom: &str) {
        let _ = synchro::lever(nom);
    }

    fn creer_signal(&mut self, nom: &str) {
        self.signal = Signal::creer(nom);
    }

    fn signal_recu(&mut self) -> bool {
        self.signal.as_ref().is_some_and(Signal::recu)
    }

    fn fermer_signal(&mut self) {
        self.signal = None;
    }

    #[cfg(not(feature = "essai"))]
    fn emplacement_mindset(&mut self) -> Option<String> {
        super::registre::lire_hklm(super::registre::CLE_MINDSET, super::registre::VALEUR_EMPLACEMENT)
    }

    /// Construction d'essai : jamais le vrai Mindset du PC.
    #[cfg(feature = "essai")]
    fn emplacement_mindset(&mut self) -> Option<String> {
        None
    }

    fn lancer(&mut self, exe: &Path, arguments: &[&str], dossier: Option<&Path>) -> io::Result<()> {
        #[cfg(feature = "essai")]
        if !est_ce_programme(exe) {
            return Err(io::Error::other("construction d'essai : seul ce programme peut être relancé"));
        }
        // fenêtre voulue : Mindset (son Mindset.exe) ou cette appli relancée, des programmes à fenêtre pour
        // l'utilisateur, lancés comme Start-Process (mindset.ps1:95, Mindcare.ps1:95), sans option de création.
        let mut c = Command::new(exe);
        c.args(arguments).creation_flags(0);
        if let Some(d) = dossier {
            c.current_dir(d);
        }
        // (pas attendu : il continue seul)
        c.spawn().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le verrou et le signal par le trait, avec des noms d'essai (jamais `Local\Mindcare-*`).
    #[test]
    fn verrou_et_signal_par_le_trait() {
        let base = format!(r"Local\MindcareEssai-{}-systeme", std::process::id());
        let (verrou, signal) = (format!("{base}-Instance"), format!("{base}-Afficher"));
        let mut s = SystemeWindows::nouveau();
        assert!(s.creer_verrou(&verrou));
        s.creer_signal(&signal);
        assert!(!s.signal_recu());
        // Un second lancement, dans un autre fil : le verrou est tenu, il lève le signal
        let (v2, s2) = (verrou.clone(), signal.clone());
        let tenu = std::thread::spawn(move || {
            let mut autre = SystemeWindows::nouveau();
            let cree = autre.creer_verrou(&v2);
            let attente = autre.attendre_verrou(Duration::from_millis(50));
            autre.lever_signal(&s2);
            (cree, attente)
        })
        .join()
        .unwrap();
        assert_eq!(tenu, (false, AttenteVerrou::Tenu));
        assert!(s.signal_recu());
        assert!(!s.signal_recu());
        // Relance : la place est libérée
        s.liberer_verrou();
        s.fermer_signal();
        let mut apres = SystemeWindows::nouveau();
        assert!(apres.creer_verrou(&verrou), "rendu et fermé : le suivant le crée");
    }
}
