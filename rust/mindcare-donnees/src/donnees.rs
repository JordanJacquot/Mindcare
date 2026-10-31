//! Les données de Mindcare réunies, comme au chargement de donnees.ps1, historique.ps1 et systeme.ps1 : le dossier de
//! données, le journal, les fichiers pris, les réglages, l'historique et la carte de Mindset.

use std::path::{Path, PathBuf};

use crate::carte::{EtatMindset, etat_carte};
use crate::fichiers::Fichiers;
use crate::historique::{Enregistrement, Historique};
use crate::journal::{Horloge, Journal};
use crate::reglages::Reglages;
use mindcare_moteur::Catalogue;

/// Les noms des fichiers du dossier de données.
pub const JOURNAL: &str = "journal.txt";
pub const REGLAGES: &str = "reglages.json";
pub const HISTORIQUE: &str = "diagnostics.json";
pub const ETAT: &str = "etat.json";

/// Tout ce que Mindcare garde dans son dossier de données.
#[derive(Clone)]
pub struct Donnees {
    dossier: PathBuf,
    pub fichiers: Fichiers,
    pub reglages: Reglages,
    pub historique: Historique,
    pub etat: EtatMindset,
    /// La carte de Mindset est écrite (gérée par Mindset, ou en test)
    pub carte_active: bool,
}

impl Donnees {
    /// Ouvre le dossier de données (créé s'il n'existe pas), raccourcit le journal et lit les réglages, comme le
    /// chargement de donnees.ps1. L'historique est lu ensuite (`Historique::lire`, au début de l'appli).
    pub fn ouvrir(dossier: &Path, horloge: Horloge, carte_active: bool) -> Donnees {
        let _ = std::fs::create_dir_all(dossier);
        let journal = Journal::ouvrir(&dossier.join(JOURNAL), horloge);
        let mut fichiers = Fichiers::nouveau(journal);
        let reglages = Reglages::charger(&mut fichiers, &dossier.join(REGLAGES));
        Donnees {
            dossier: dossier.to_path_buf(),
            fichiers,
            reglages,
            historique: Historique::nouveau(&dossier.join(HISTORIQUE)),
            etat: EtatMindset::nouveau(&dossier.join(ETAT)),
            carte_active,
        }
    }

    /// Le dossier de données.
    pub fn dossier(&self) -> &Path {
        &self.dossier
    }

    /// Le journal.
    pub fn journal(&self) -> &Journal {
        &self.fichiers.journal
    }

    /// `Read-Historique`.
    pub fn lire_historique(&mut self) {
        self.historique.lire(&mut self.fichiers);
    }

    /// `Save-Diagnostic`, puis la carte de Mindset réécrite (si elle a changé).
    pub fn sauver_diagnostic(&mut self, cat: Option<&Catalogue>, e: &Enregistrement) {
        if self.historique.sauver(&mut self.fichiers, e) {
            self.ecrire_etat(cat);
        }
    }

    /// `Write-EtatMindset` : la carte de Mindset, écrite seulement si elle change.
    pub fn ecrire_etat(&mut self, cat: Option<&Catalogue>) {
        if !self.carte_active {
            return;
        }
        let carte = etat_carte(cat, &self.historique.liste);
        self.etat.ecrire(&mut self.fichiers, &carte);
    }
}
