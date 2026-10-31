//! Les réglages (reglages.json) : `Get-Setting` et `Set-Setting` (modules\donnees.ps1).
//!
//! Fichier pris au lancement : un réglage changé est gardé en mémoire, et enregistré avec ceux du fichier dès qu'il est
//! libre (le fichier est relu, ses réglages pris, sauf ceux changés depuis le lancement).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::fichiers::Fichiers;
use mindcare_moteur::Objet;
use mindcare_moteur::ps::{champ, cle, mettre};

/// Les réglages de Mindcare.
#[derive(Debug, Clone)]
pub struct Reglages {
    fichier: PathBuf,
    valeurs: Objet,
    /// (les réglages changés depuis le lancement : gardés si le fichier était pris)
    changes: HashSet<String>,
}

impl Reglages {
    /// Lit reglages.json au lancement (un fichier absent ou abîmé : aucun réglage).
    pub fn charger(fichiers: &mut Fichiers, fichier: &Path) -> Reglages {
        let mut valeurs = Objet::new();
        if let Some(j) = fichiers.lire(fichier) {
            for (k, v) in j {
                mettre(&mut valeurs, &k, v);
            }
        }
        Reglages { fichier: fichier.to_path_buf(), valeurs, changes: HashSet::new() }
    }

    /// Le fichier des réglages.
    pub fn fichier(&self) -> &Path {
        &self.fichier
    }

    /// Tous les réglages en mémoire.
    pub fn valeurs(&self) -> &Objet {
        &self.valeurs
    }

    /// `Get-Setting` : un réglage, ou `defaut` s'il n'existe pas (le nom sans tenir compte des majuscules).
    pub fn lire(&self, nom: &str, defaut: Value) -> Value {
        champ(&self.valeurs, nom).cloned().unwrap_or(defaut)
    }

    /// `Set-Setting` : change un réglage et l'enregistre (ou le garde en mémoire si le fichier est pris).
    pub fn changer(&mut self, fichiers: &mut Fichiers, nom: &str, valeur: Value) {
        mettre(&mut self.valeurs, nom, valeur);
        self.changes.insert(cle(nom));
        let f = fichiers.relire(&self.fichier);
        if !f.libre {
            fichiers.journal.ecrire(&format!(
                "Réglage {nom} gardé en mémoire : le fichier des réglages est pris, il sera enregistré plus tard"
            ));
            return;
        }
        if f.relu
            && let Some(contenu) = f.contenu
        {
            for (k, v) in contenu {
                if !self.changes.contains(&cle(&k)) {
                    mettre(&mut self.valeurs, &k, v);
                }
            }
        }
        if let Err(e) = fichiers.sauver(&self.fichier, &Value::Object(self.valeurs.clone())) {
            fichiers.journal.ecrire(&format!("Réglages non enregistrés : {e}"));
        }
    }
}
