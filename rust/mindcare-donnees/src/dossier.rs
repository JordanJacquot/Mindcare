//! Où sont les données de Mindcare : `Get-CheminNormal`, `Read-Suite`, `Test-GereeParMindset` (modules\mindset.ps1)
//! et `Get-DonneesMindset` (modules\donnees.ps1). Rien n'est lu dans Windows : les variables d'environnement
//! (LOCALAPPDATA, ProgramFiles) sont données par l'appelant.
//!
//! - Copie gérée par Mindset : déduit de l'emplacement du programme (jamais d'un fichier modifiable) :
//!   `<X>:\Mindset\Programmes\Mindcare` donne `<X>:\Mindset\Donnees\Mindcare` ; sinon
//!   `%LOCALAPPDATA%\Mindset\Mindcare`.
//! - Dossier de travail (code source) : `%LOCALAPPDATA%\Mindset\Mindcare (travail)`, pour ne jamais mélanger avec la
//!   vraie.
//! - Tests : MINDCARE_DONNEES.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use suite_commun::chemins::{chemin_complet, commence_par_disque, egal, feuille, joindre, parent};

use crate::carte::longueur;
use crate::fichiers::{decoder, erreur_fichier_pris};
use mindcare_moteur::ps::{champ, vrai};

/// Le nom de l'appli (ses dossiers).
pub const APP_NOM: &str = "Mindcare";
/// Son Id dans la suite (suite.json, apps.json).
pub const APP_ID: &str = "mindcare";
/// Les thèmes de la suite.
pub const THEMES_SUITE: [&str; 5] = ["neon", "crepuscule", "terminal", "arcade", "rubis"];

/// `Get-CheminNormal` : le chemin complet, sans « \ » à la fin (pour comparer deux dossiers), ou rien s'il n'est pas
/// valable (il doit commencer par une lettre de disque).
pub fn chemin_normal(chemin: &str) -> Option<String> {
    if !commence_par_disque(chemin) {
        return None;
    }
    chemin_complet(chemin).ok().map(|c| c.trim_end_matches('\\').to_string())
}

/// `Get-DonneesMindset` : le dossier des données d'une copie gérée par Mindset, d'après le dossier de cette copie.
pub fn donnees_mindset(racine: Option<&str>, localappdata: &str) -> String {
    let p = racine.map(parent).unwrap_or_default();
    let m = if p.is_empty() { "" } else { parent(p) };
    if !m.is_empty() && egal(feuille(p), "Programmes") && egal(feuille(m), "Mindset") {
        return joindre(&joindre(m, "Donnees"), APP_NOM);
    }
    joindre(localappdata, &format!(r"Mindset\{APP_NOM}"))
}

/// Le dossier de données (`$DataDir`) : celui des tests (MINDCARE_DONNEES, seulement en test), celui de Mindset pour une
/// copie gérée, sinon le dossier de travail.
pub fn dossier_donnees(en_test: Option<&str>, gere: bool, racine: Option<&str>, localappdata: &str) -> String {
    match en_test {
        Some(d) if !d.is_empty() => d.to_string(),
        _ if gere => donnees_mindset(racine, localappdata),
        _ => joindre(localappdata, &format!(r"Mindset\{APP_NOM} (travail)")),
    }
}

/// Ce qu'on garde de suite.json (`Read-Suite`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suite {
    pub existe: bool,
    pub lisible: bool,
    /// Tenu par un autre programme après tous les essais : on ne sait pas encore ce qu'il dit
    pub pris: bool,
    pub theme: Option<String>,
    /// Le dossier de Mindcare selon Mindset (seulement pour comparer avec le nôtre)
    pub dossier: Option<String>,
    /// Le micro choisi dans Mindset (vide : celui de Windows)
    pub micro: String,
}

/// `$j.nom` : le champ d'un objet ; sur une liste, celui de chacun de ses objets (un seul : lui-même).
fn membre(v: Option<&Value>, nom: &str) -> Option<Value> {
    match v {
        Some(Value::Object(o)) => champ(o, nom).cloned(),
        Some(Value::Array(a)) => {
            let l: Vec<Value> = a.iter().filter_map(|x| x.as_object().and_then(|o| champ(o, nom)).cloned()).collect();
            match l.len() {
                0 => None,
                1 => l.into_iter().next(),
                _ => Some(Value::Array(l)),
            }
        }
        _ => None,
    }
}

/// `-cmatch '^pack:[a-z0-9-]{1,40}$'` (« $ » de .NET permet un retour à la ligne final).
fn nom_de_pack(t: &str) -> bool {
    let t = t.strip_suffix('\n').unwrap_or(t);
    t.strip_prefix("pack:").is_some_and(|n| {
        (1..=40).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}

/// `Read-Suite` : lit suite.json (`essais` lectures espacées de 300 ms s'il est pris). Rien n'y est jamais lancé ni
/// ouvert : un nom de thème connu, un dossier seulement comparé au nôtre, un nom de micro.
pub fn lire_suite(fichier: Option<&Path>, essais: u32) -> Suite {
    let mut s = Suite::default();
    let Some(f) = fichier.filter(|f| f.is_file()) else { return s };
    s.existe = true;
    let mut texte = None;
    for i in 1..=essais {
        match std::fs::read(f) {
            Ok(o) => {
                texte = Some(decoder(&o));
                s.pris = false;
                break;
            }
            Err(e) => {
                s.pris = erreur_fichier_pris(&e);
                if !s.pris {
                    break;
                }
                if i < essais {
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
        }
    }
    let Some(t) = texte else { return s };
    // (un texte vide est lu comme « rien » par ConvertFrom-Json : lisible, sans rien dedans)
    let j: Option<Value> = if t.trim().is_empty() {
        None
    } else {
        match serde_json::from_str(&t) {
            Ok(v) => Some(v),
            Err(_) => return s, // (illisible, ou en train d'être remplacé par Mindset)
        }
    };
    let j = j.as_ref();
    if let Some(Value::String(theme)) = membre(j, "theme") {
        if THEMES_SUITE.iter().any(|x| mindcare_moteur::ps::egal(x, &theme)) {
            s.theme = Some(theme.to_lowercase());
        } else if nom_de_pack(&theme) {
            s.theme = Some(theme);
        }
    }
    let applis = membre(j, "applis");
    if vrai(applis.as_ref())
        && let Some(Value::String(d)) = membre(applis.as_ref(), APP_ID)
    {
        s.dossier = chemin_normal(&d);
    }
    if let Some(Value::String(m)) = membre(j, "micro")
        && longueur(&m) <= 64
        && !m.chars().any(|c| (c as u32) < 0x20)
    {
        s.micro = m;
    }
    s.lisible = true;
    s
}

/// `Test-GereeParMindset` : suite.json le dit ; encore pris après 3 secondes (démarrage de Windows), une copie dans un
/// dossier de programmes de Mindset (`<X>:\Mindset\Programmes`, ou `Program Files\Mindset`) est gérée.
/// `programmes` : ProgramFiles et ProgramW6432.
pub fn gere_par_mindset(suite: &Suite, racine: Option<&str>, programmes: &[Option<&str>]) -> bool {
    let Some(racine) = racine.filter(|r| !r.is_empty()) else { return false };
    if suite.dossier.as_deref().is_some_and(|d| !d.is_empty() && egal(d, racine)) {
        return true;
    }
    if !suite.pris {
        return false;
    }
    let p = parent(racine);
    let m = if p.is_empty() { "" } else { parent(p) };
    if !m.is_empty() && egal(feuille(p), "Programmes") && egal(feuille(m), "Mindset") {
        return true;
    }
    programmes.iter().flatten().any(|pf| !pf.is_empty() && egal(p, &joindre(pf, "Mindset")))
}
