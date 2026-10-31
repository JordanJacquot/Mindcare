//! Les fichiers JSON de Mindcare (reglages.json, diagnostics.json, etat.json) : `Save-JsonFile`, `Read-JsonFile`,
//! `Test-ErreurFichierPris` et `Get-FichierRelu` (modules\donnees.ps1, Mindcare 0.15.1).
//!
//! - Écriture : `json_sur::ecrire` de suite-commun (à côté, puis remplacé d'un coup ; l'ancien reste dans « .bak »).
//!   Un fichier noté « pris » n'est jamais écrit : on n'en a peut-être que la sauvegarde en mémoire.
//! - Lecture : un fichier qu'un autre programme garde ouvert un instant (antivirus, OneDrive au démarrage de Windows)
//!   n'est pas abîmé : 5 essais à 300 ms, puis il est noté « pris » ; on lit sa sauvegarde pour montrer quelque chose,
//!   mais rien n'est enregistré tant qu'il n'a pas été relu. Il n'est jamais mis de côté.
//! - Un fichier abîmé (vide, coupé, pas un objet JSON) passe par `json_sur::lire` : il est mis de côté
//!   (« .abime-<date> », jamais effacé) et sa copie « .bak » est remise à sa place.
//!
//! Divergence voulue avec Mindcare PowerShell (constat 23 de l'audit, tests\juges\divergences.json) : un fichier vide
//! n'est jamais rangé en copie de secours. PowerShell copiait le fichier vide (ou abîmé) à côté mais le laissait en
//! place : l'enregistrement suivant le poussait dans « .bak » et la bonne copie était perdue. Ici il est déplacé par
//! `json_sur::lire`, et un fichier devenu vide entre la lecture et l'écriture est retiré avant d'écrire (rien n'y est
//! perdu : il ne contient rien).

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use suite_commun::json_sur;

use crate::journal::Journal;
use mindcare_moteur::Objet;

/// Les essais de lecture d'un fichier pris un instant.
pub const ESSAIS: u32 = 5;
/// L'attente entre deux essais.
pub const ATTENTE: Duration = Duration::from_millis(300);

/// Le texte d'un fichier comme `[IO.File]::ReadAllText($f, [Text.Encoding]::UTF8)` : sa marque d'encodage (UTF-8,
/// UTF-16, UTF-32) est suivie, sinon UTF-8 ; les octets invalides deviennent « � ».
pub fn decoder(octets: &[u8]) -> String {
    fn utf16(o: &[u8], lire: fn([u8; 2]) -> u16) -> String {
        let unites: Vec<u16> = o.as_chunks::<2>().0.iter().map(|c| lire(*c)).collect();
        String::from_utf16_lossy(&unites)
    }
    fn utf32(o: &[u8], lire: fn([u8; 4]) -> u32) -> String {
        o.as_chunks::<4>().0.iter().map(|c| char::from_u32(lire(*c)).unwrap_or('\u{FFFD}')).collect()
    }
    if let Some(r) = octets.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(r).into_owned();
    }
    if let Some(r) = octets.strip_prefix(&[0xFF, 0xFE, 0x00, 0x00]) {
        return utf32(r, u32::from_le_bytes);
    }
    if let Some(r) = octets.strip_prefix(&[0x00, 0x00, 0xFE, 0xFF]) {
        return utf32(r, u32::from_be_bytes);
    }
    if let Some(r) = octets.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(r, u16::from_le_bytes);
    }
    if let Some(r) = octets.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(r, u16::from_be_bytes);
    }
    String::from_utf8_lossy(octets).into_owned()
}

/// `Test-ErreurFichierPris` : l'erreur d'un fichier qu'un autre programme garde ouvert (partage refusé : 32,
/// verrou : 33).
pub fn erreur_fichier_pris(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(32 | 33))
}

/// Un fichier « vide » au sens de Mindcare : rien, des espaces, ou « null ».
pub fn texte_vide(t: &str) -> bool {
    t.trim().is_empty() || t.trim() == "null"
}

/// Le contenu d'un fichier JSON : un objet, ou pourquoi il est abîmé.
pub fn analyser(t: &str) -> Result<Objet, String> {
    if texte_vide(t) {
        return Err("fichier vide".to_string());
    }
    match serde_json::from_str::<Value>(t) {
        Ok(Value::Object(o)) => Ok(o),
        Ok(_) => Err("ce n'est pas un objet JSON".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Ce qu'a donné la lecture d'un fichier, essais compris.
enum Lu {
    Absent,
    Pris,
    Erreur(io::Error),
    Texte(String),
}

/// Lit un fichier : 5 essais à 300 ms s'il est pris, aucun autre essai pour une autre erreur.
fn lire_texte(p: &Path) -> Lu {
    if !p.is_file() {
        return Lu::Absent;
    }
    for essai in 1..=ESSAIS {
        match std::fs::read(p) {
            Ok(o) => return Lu::Texte(decoder(&o)),
            Err(e) if erreur_fichier_pris(&e) => {
                if essai < ESSAIS {
                    std::thread::sleep(ATTENTE);
                }
            }
            Err(e) => return Lu::Erreur(e),
        }
    }
    Lu::Pris
}

/// `<fichier><fin>`.
pub fn a_cote(fichier: &Path, fin: &str) -> PathBuf {
    let mut s = fichier.as_os_str().to_owned();
    s.push(fin);
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

    /// `Read-JsonFile` : le contenu du fichier, ou de sa sauvegarde ; rien s'il n'y a rien de bon à lire.
    pub fn lire(&mut self, fichier: &Path) -> Option<Objet> {
        self.pris.remove(&cle(fichier));
        let affiche = fichier.display().to_string();
        match lire_texte(fichier) {
            Lu::Absent => {}
            Lu::Pris => {
                self.journal.ecrire(&format!(
                    "Fichier pris par un autre programme, en lecture seule pour l'instant : {affiche}"
                ));
                self.pris.insert(cle(fichier));
            }
            Lu::Erreur(e) => {
                if let Some(v) = self.abime(fichier, &e.to_string()) {
                    return v;
                }
            }
            Lu::Texte(t) => match analyser(&t) {
                Ok(o) => return Some(o),
                Err(raison) => {
                    if let Some(v) = self.abime(fichier, &raison) {
                        return v;
                    }
                }
            },
        }
        self.lire_sauvegarde(fichier)
    }

    /// Le fichier lui-même est abîmé : `json_sur::lire` le met de côté et reprend sa copie « .bak » (remise à sa
    /// place). `None` : json_sur n'a pas pu le lire du tout (pris entre-temps) ; la sauvegarde est alors lue comme
    /// pour un fichier pris.
    fn abime(&mut self, fichier: &Path, raison: &str) -> Option<Option<Objet>> {
        self.journal.ecrire(&format!("Fichier illisible ({}) : {raison}", fichier.display()));
        match json_sur::lire(fichier, true) {
            Ok(l) => {
                for ligne in &l.journal {
                    self.journal.ecrire(ligne);
                }
                Some(l.valeur)
            }
            Err(e) => {
                self.journal.ecrire(&e);
                None
            }
        }
    }

    /// La sauvegarde (« .bak ») d'un fichier absent ou pris, lue comme lui (5 essais) ; abîmée, elle est copiée à
    /// côté (« .bak.abime-<date> »).
    fn lire_sauvegarde(&mut self, fichier: &Path) -> Option<Objet> {
        let p = a_cote(fichier, ".bak");
        let affiche = p.display().to_string();
        let raison = match lire_texte(&p) {
            Lu::Absent => return None,
            Lu::Pris => {
                self.journal.ecrire(&format!(
                    "Fichier pris par un autre programme, en lecture seule pour l'instant : {affiche}"
                ));
                return None;
            }
            Lu::Erreur(e) => e.to_string(),
            Lu::Texte(t) if texte_vide(&t) => "fichier vide".to_string(),
            Lu::Texte(t) => {
                // (noté avant de lire le JSON, comme Mindcare PowerShell : une copie abîmée donne « repris » puis
                // « illisible »)
                self.journal.ecrire(&format!("Fichier repris de sa sauvegarde : {affiche}"));
                match analyser(&t) {
                    Ok(o) => return Some(o),
                    Err(r) => r,
                }
            }
        };
        self.journal.ecrire(&format!("Fichier illisible ({affiche}) : {raison}"));
        let de = a_cote(&p, &format!(".abime-{}", self.journal.maintenant().compact()));
        let _ = std::fs::copy(&p, de);
        None
    }

    /// `Save-JsonFile` : écrit le fichier sans risque de le perdre (json_sur). Un fichier noté « pris » n'est jamais
    /// écrit. Un fichier vide en place n'est jamais poussé dans « .bak » (constat 23) : il est retiré d'abord.
    pub fn sauver(&mut self, fichier: &Path, valeur: &Value) -> Result<(), String> {
        if self.est_pris(fichier) {
            return Err(format!(
                "{} est en lecture seule pour l'instant (un autre programme le garde ouvert)",
                nom(fichier)
            ));
        }
        let texte = serde_json::to_string_pretty(valeur).map_err(|e| e.to_string())?;
        retirer_si_vide(fichier);
        json_sur::ecrire(fichier, &texte).map_err(|e| e.to_string())
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

/// Constat 23 : un fichier vide (rien, des espaces, « null ») n'a rien à sauvegarder ; il est retiré avant l'écriture
/// pour ne jamais remplacer la bonne copie « .bak ».
fn retirer_si_vide(fichier: &Path) {
    let Ok(m) = std::fs::metadata(fichier) else { return };
    if !m.is_file() || m.len() > 4096 {
        return;
    }
    if let Ok(o) = std::fs::read(fichier)
        && texte_vide(&decoder(&o))
    {
        let _ = std::fs::remove_file(fichier);
    }
}
