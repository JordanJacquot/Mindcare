//! Le [`Disque`] du programme : les vrais fichiers (std::fs), suite.json lu par dossier.rs (`Read-Suite`), le journal
//! de mindcare-donnees (`Write-Log`), et etat.json écrit par la fonction que donne l'appelant (en pratique
//! `Donnees::ecrire_etat`, qui passe par carte.rs : `Get-EtatCarte` et `Write-EtatMindset`).

use std::io;
use std::path::Path;

use mindcare_donnees::donnees::JOURNAL;
use mindcare_donnees::dossier::{Suite, lire_suite};
use mindcare_donnees::{Horloge, Journal};

use crate::monde::{Date, Disque};

/// Les vrais fichiers.
pub struct DisqueReel {
    horloge: Horloge,
    journal: Option<Journal>,
    etat: Box<dyn FnMut()>,
}

impl DisqueReel {
    /// `horloge` : l'heure locale du journal ; `etat` : écrit la carte de Mindset si elle a changé.
    pub fn nouveau(horloge: Horloge, etat: Box<dyn FnMut()>) -> DisqueReel {
        DisqueReel { horloge, journal: None, etat }
    }
}

impl Disque for DisqueReel {
    fn existe(&self, chemin: &Path) -> bool {
        chemin.exists()
    }

    fn est_fichier(&self, chemin: &Path) -> bool {
        chemin.is_file()
    }

    fn effacer(&mut self, chemin: &Path) -> io::Result<()> {
        std::fs::remove_file(chemin)
    }

    fn date(&self, chemin: &Path) -> Date {
        // (FileInfo.Exists : seulement un fichier)
        std::fs::metadata(chemin).ok().filter(|m| m.is_file()).and_then(|m| m.modified().ok())
    }

    fn lire_suite(&mut self, fichier: &Path, essais: u32) -> Suite {
        lire_suite(Some(fichier), essais)
    }

    fn creer_dossier(&mut self, chemin: &Path) -> io::Result<()> {
        std::fs::create_dir_all(chemin)
    }

    fn ecrire_texte(&mut self, chemin: &Path, texte: &str) -> io::Result<()> {
        std::fs::write(chemin, texte.as_bytes())
    }

    fn ouvrir_donnees(&mut self, dossier: &Path) {
        // donnees.ps1:22, :29-34 : le journal, raccourci s'il est trop gros
        self.journal = Some(Journal::ouvrir(&dossier.join(JOURNAL), self.horloge.clone()));
    }

    fn ecrire_etat(&mut self) {
        (self.etat)();
    }

    fn journal(&mut self, message: &str) {
        if let Some(j) = &self.journal {
            j.ecrire(message);
        }
    }
}
