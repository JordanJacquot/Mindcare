//! Les fichiers JSON de Mindcare (reglages.json, diagnostics.json, etat.json) : `Save-JsonFile`, `Read-JsonFile`,
//! `Test-ErreurFichierPris` et `Get-FichierRelu` (modules\donnees.ps1, Mindcare 0.15.1).
//!
//! L'écriture, la lecture, les essais et les mises de côté sont ceux de `json_sur` (suite-commun), avec les essais de
//! Mindcare ([`Fichiers::options`] : 5 essais à 300 ms, seulement pour un fichier pris) et l'heure de son journal.
//! Ici ne restent que ce qui est propre à Mindcare : la table des fichiers « pris », et les lignes de son journal
//! (les mêmes que Mindcare PowerShell, suivies de celles de json_sur quand il met un fichier de côté).
//!
//! - Écriture : `json_sur::ecrire_avec` (à côté, puis remplacé d'un coup ; l'ancien reste dans « .bak »).
//!   Un fichier noté « pris » n'est jamais écrit : on n'en a peut-être que la sauvegarde en mémoire.
//! - Lecture : un fichier qu'un autre programme garde ouvert un instant (antivirus, OneDrive au démarrage de Windows)
//!   n'est pas abîmé : 5 essais à 300 ms, puis il est noté « pris » ; on lit sa sauvegarde pour montrer quelque chose,
//!   mais rien n'est enregistré tant qu'il n'a pas été relu. Il n'est jamais mis de côté.
//! - Un fichier abîmé (vide, coupé, pas un objet JSON) passe par `json_sur::lire_avec` : il est mis de côté
//!   (« .abime-<date> », jamais effacé) et sa copie « .bak » est remise à sa place.
//!
//! Divergences voulues avec Mindcare PowerShell (tests\juges\divergences.json) :
//! - constat 23 de l'audit : un fichier vide n'est jamais rangé en copie de secours. PowerShell copiait le fichier vide
//!   (ou abîmé) à côté mais le laissait en place : l'enregistrement suivant le poussait dans « .bak » et la bonne copie
//!   était perdue. Ici il est déplacé par `json_sur::lire_avec`, et à l'écriture `json_sur::ecrire_avec` ne range
//!   dans « .bak » qu'un fichier qui contient un objet JSON (vide, il est simplement remplacé ; abîmé, il est d'abord
//!   copié de côté) ;
//! - une copie « .bak » abîmée est déplacée de côté (« .bak.abime-<date> ») par json_sur, et non plus copiée :
//!   PowerShell la laissait en place et en refaisait une copie à chaque lecture ;
//! - deux mises de côté dans la même seconde ne s'écrasent plus : la seconde prend le nom suivant (« -2 », « -3 »).

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use suite_commun::json_sur::{self, HeureLocale, Options};

use crate::journal::Journal;
use mindcare_moteur::Objet;

/// Les essais de lecture d'un fichier pris un instant.
pub const ESSAIS: u32 = 5;
/// L'attente entre deux essais.
pub const ATTENTE: Duration = Duration::from_millis(300);

/// Ce qu'a donné la lecture d'un fichier, essais compris.
enum Lu {
    Absent,
    Pris,
    Erreur(io::Error),
    Texte(String),
}

/// Lit le texte d'un fichier avec les essais de Mindcare, sans rien déplacer : la raison d'un fichier abîmé est
/// notée avant que json_sur le mette de côté (sa [`json_sur::Lecture`] ne la garde pas quand la copie est reprise).
fn lire_texte(p: &Path, o: &Options) -> Lu {
    if !p.is_file() {
        return Lu::Absent;
    }
    for essai in 1..=o.essais {
        match std::fs::read(p) {
            Ok(octets) => return Lu::Texte(json_sur::texte_du_fichier(&octets)),
            Err(e) if json_sur::erreur_fichier_pris(&e) => {
                if essai < o.essais {
                    std::thread::sleep(o.attente);
                }
            }
            Err(e) => return Lu::Erreur(e),
        }
    }
    Lu::Pris
}

/// `<fichier>.bak`.
fn copie_de(fichier: &Path) -> PathBuf {
    let mut s = fichier.as_os_str().to_owned();
    s.push(".bak");
    PathBuf::from(s)
}

/// Le nom d'un fichier (pour les messages).
fn nom(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Un fichier noté « pris » et relu avant d'être enregistré (`Get-FichierRelu`).
#[derive(Debug, Clone, PartialEq)]
pub struct FichierRelu {
    /// On peut l'enregistrer.
    pub libre: bool,
    /// Il vient d'être relu (son contenu est à fusionner avec ce qui a changé pendant qu'il était pris).
    pub relu: bool,
    pub contenu: Option<Objet>,
}

/// Les fichiers JSON de Mindcare : le journal, et ceux qu'un autre programme gardait ouverts à la lecture.
#[derive(Clone)]
pub struct Fichiers {
    pub journal: Journal,
    /// (les chemins sans tenir compte des majuscules, comme les clés d'une table PowerShell)
    pris: HashSet<String>,
}

/// La clé d'un chemin dans la table des fichiers pris.
fn cle(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

impl Fichiers {
    pub fn nouveau(journal: Journal) -> Fichiers {
        Fichiers { journal, pris: HashSet::new() }
    }

    /// Les options de json_sur pour Mindcare : 5 essais à 300 ms, seulement pour un fichier pris (une autre erreur, ou
    /// un texte abîmé, n'est pas réessayée) ; les fichiers mis de côté prennent l'heure du journal.
    pub fn options(&self) -> Options {
        let journal = self.journal.clone();
        Options {
            essais: ESSAIS,
            attente: ATTENTE,
            seulement_partage: true,
            horloge: Arc::new(move || {
                let m = journal.maintenant();
                HeureLocale {
                    annee: m.annee,
                    mois: m.mois,
                    jour: m.jour,
                    heure: m.heure,
                    minute: m.minute,
                    seconde: m.seconde,
                }
            }),
        }
    }

    /// Ce fichier est-il noté « pris » (en lecture seule pour l'instant) ?
    pub fn est_pris(&self, p: &Path) -> bool {
        self.pris.contains(&cle(p))
    }

    /// Les fichiers notés « pris » (leurs chemins, en minuscules).
    pub fn fichiers_pris(&self) -> Vec<String> {
        let mut l: Vec<String> = self.pris.iter().cloned().collect();
        l.sort();
        l
    }

    /// Recopie dans le journal ce qu'a noté json_sur.
    fn recopier(&self, lignes: &[String]) {
        for l in lignes {
            self.journal.ecrire(l);
        }
    }

    /// La ligne de Mindcare PowerShell pour un fichier pris pendant tous les essais.
    fn noter_pris(&self, p: &Path) {
        self.journal
            .ecrire(&format!("Fichier pris par un autre programme, en lecture seule pour l'instant : {}", p.display()));
    }

    /// `Read-JsonFile` : le contenu du fichier, ou de sa sauvegarde ; rien s'il n'y a rien de bon à lire.
    pub fn lire(&mut self, fichier: &Path) -> Option<Objet> {
        self.pris.remove(&cle(fichier));
        let o = self.options();
        match lire_texte(fichier, &o) {
            Lu::Absent => {}
            Lu::Pris => {
                self.noter_pris(fichier);
                self.pris.insert(cle(fichier));
            }
            // (une autre erreur : rien ne peut être mis de côté, sa sauvegarde est lue)
            Lu::Erreur(e) => self.journal.ecrire(&format!("Fichier illisible ({}) : {e}", fichier.display())),
            Lu::Texte(t) => match json_sur::analyser(&t) {
                Ok(v) => return Some(v),
                Err(raison) => {
                    self.journal.ecrire(&format!("Fichier illisible ({}) : {raison}", fichier.display()));
                    // json_sur le met de côté et reprend sa copie (remise à sa place)
                    match json_sur::lire_avec(fichier, true, &o) {
                        Ok(l) => {
                            self.recopier(&l.journal);
                            return l.valeur;
                        }
                        Err(e) => {
                            self.recopier(&e.journal);
                            if e.fichier != fichier {
                                // (sa copie, prise par un autre programme)
                                self.noter_pris(&e.fichier);
                                return None;
                            }
                            // (pris ou illisible entre-temps : sa sauvegarde est lue comme pour un fichier pris)
                            if e.pris {
                                self.noter_pris(fichier);
                                self.pris.insert(cle(fichier));
                            } else {
                                self.journal.ecrire(&e.to_string());
                            }
                        }
                    }
                }
            },
        }
        self.lire_sauvegarde(fichier, &o)
    }

    /// La sauvegarde (« .bak ») d'un fichier absent, pris ou illisible, lue comme lui (5 essais). Abîmée, json_sur la
    /// met de côté (« .bak.abime-<date> »).
    fn lire_sauvegarde(&mut self, fichier: &Path, o: &Options) -> Option<Objet> {
        let p = copie_de(fichier);
        let raison = match lire_texte(&p, o) {
            Lu::Absent => return None,
            Lu::Pris => {
                self.noter_pris(&p);
                return None;
            }
            Lu::Erreur(e) => {
                self.journal.ecrire(&format!("Fichier illisible ({}) : {e}", p.display()));
                return None;
            }
            Lu::Texte(t) => {
                // (noté avant de lire le JSON, comme Mindcare PowerShell : une copie abîmée donne « repris » puis
                // « illisible » ; une copie vide, seulement « illisible »)
                if !json_sur::texte_vide(&t) {
                    self.journal.ecrire(&format!("Fichier repris de sa sauvegarde : {}", p.display()));
                }
                match json_sur::analyser(&t) {
                    Ok(v) => return Some(v),
                    Err(r) => r,
                }
            }
        };
        self.journal.ecrire(&format!("Fichier illisible ({}) : {raison}", p.display()));
        match json_sur::lire_copie(fichier, true, o) {
            // (bonne entre-temps : elle est reprise)
            Ok(l) => {
                self.recopier(&l.journal);
                l.valeur
            }
            Err(e) => {
                self.recopier(&e.journal);
                self.noter_pris(&e.fichier);
                None
            }
        }
    }

    /// `Save-JsonFile` : écrit le fichier sans risque de le perdre (`json_sur::ecrire_avec`). Un fichier noté « pris »
    /// n'est jamais écrit. Seul un fichier qui contient un objet JSON est gardé dans « .bak » (constat 23).
    pub fn sauver(&mut self, fichier: &Path, valeur: &Value) -> Result<(), String> {
        if self.est_pris(fichier) {
            return Err(format!(
                "{} est en lecture seule pour l'instant (un autre programme le garde ouvert)",
                nom(fichier)
            ));
        }
        let texte = serde_json::to_string_pretty(valeur).map_err(|e| e.to_string())?;
        json_sur::ecrire_avec(fichier, &texte, &self.options()).map_err(|e| e.to_string())
    }

    /// `Get-FichierRelu` : un fichier noté « pris » est relu avant d'être enregistré.
    pub fn relire(&mut self, fichier: &Path) -> FichierRelu {
        if !self.est_pris(fichier) {
            return FichierRelu { libre: true, relu: false, contenu: None };
        }
        let j = self.lire(fichier);
        if self.est_pris(fichier) {
            return FichierRelu { libre: false, relu: false, contenu: None };
        }
        self.journal.ecrire(&format!("Fichier de nouveau libre, relu avant d'enregistrer : {}", fichier.display()));
        FichierRelu { libre: true, relu: true, contenu: j }
    }
}
