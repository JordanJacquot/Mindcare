//! La carte vivante dans l'accueil de Mindset (etat.json, format commun) : la dernière enquête. `Get-Court`,
//! `Get-PhraseCarte`, `Get-EtatCarte` (modules\historique.ps1) et `Write-EtatMindset` (modules\systeme.ps1).
//!
//! etat.json : { Version, Titre, Scores : [ { Nom, Valeur 0 à 100 } ] (2 au plus), Lignes : [ { Texte, Etat } ]
//! (3 au plus), Maj }. Etat : ok (vert), info (blanc), attention (orange), urgent (rouge). Mindset coupe chaque ligne à
//! 80 caractères : chaque ligne tient dans 80, coupée au dernier mot entier.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::fichiers::Fichiers;
use crate::historique::{InfoDiagnostic, info_diagnostic};
use crate::moment::format_date;
use mindcare_moteur::ps::{champ, egal, texte, vrai};
use mindcare_moteur::{Catalogue, Objet, TypeVerdict, Verdict, titre_verdict};

/// La longueur d'une ligne de la carte (en caractères .NET : unités UTF-16).
pub const LIGNE_MAX: usize = 80;

/// La longueur d'un texte comme `.Length` de .NET.
pub fn longueur(t: &str) -> usize {
    t.encode_utf16().count()
}

/// `Get-Court` : un texte qui tient dans `max` caractères, coupé au dernier mot entier (s'il n'est pas trop tôt),
/// suivi de « ... ».
pub fn court(t: &str, max: usize) -> String {
    let u: Vec<u16> = t.encode_utf16().collect();
    if u.len() <= max {
        return t.to_string();
    }
    let mut c = &u[..max.saturating_sub(3)];
    if let Some(i) = c.iter().rposition(|x| *x == u16::from(b' '))
        && (i as f64) > max as f64 / 2.0
    {
        c = &c[..i];
    }
    let fin = c.iter().rposition(|x| !b" ,:;(".iter().any(|b| u16::from(*b) == *x));
    let c = match fin {
        Some(i) => &c[..=i],
        None => &c[..0],
    };
    format!("{}...", String::from_utf16_lossy(c))
}

/// `Get-PhraseCarte` : la réponse d'une enquête finie, sur une ligne de la carte (la consigne utile n'est jamais
/// coupée).
pub fn phrase_carte(cat: &Catalogue, info: &InfoDiagnostic) -> String {
    let p = info.phrase.as_str();
    let rien = Verdict { genre: TypeVerdict::Rien, suspect: None, detail: String::new(), restants: Vec::new() };
    if info.suspect.is_none() && egal(p, &titre_verdict(cat, Some(&rien))) {
        return "Aucune piste trouvée : montre le rapport à un réparateur".to_string();
    }
    if longueur(p) <= LIGNE_MAX {
        return p.to_string();
    }
    if info.suspect.is_none() {
        return "Plusieurs pistes : un réparateur départagera".to_string();
    }
    // (« ce qui a été trouvé : le détail » : ce qui a été trouvé)
    court(p.split_once(" : ").map_or(p, |(a, _)| a), LIGNE_MAX)
}

/// Une ligne de la carte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ligne {
    pub texte: String,
    /// ok, info, attention, urgent
    pub etat: String,
}

/// La carte de Mindset.
#[derive(Debug, Clone, PartialEq)]
pub struct Carte {
    pub titre: String,
    pub scores: Vec<Value>,
    pub lignes: Vec<Ligne>,
}

impl Carte {
    /// Sous la forme de etat.json (sans Version ni Maj).
    pub fn json(&self) -> Value {
        json!({
            "Titre": self.titre,
            "Scores": self.scores,
            "Lignes": self.lignes.iter().map(|l| json!({ "Texte": l.texte, "Etat": l.etat })).collect::<Vec<_>>(),
        })
    }
}

fn ligne(texte: String, etat: &str) -> Ligne {
    Ligne { texte, etat: etat.to_string() }
}

/// `Get-EtatCarte` : ce que la carte montre de la dernière enquête (`historique` : de la plus récente à la plus
/// ancienne ; `cat` : le catalogue, s'il a pu être lu).
pub fn etat_carte(cat: Option<&Catalogue>, historique: &[Objet]) -> Carte {
    let mut carte = Carte {
        titre: "Dernier diagnostic".to_string(),
        scores: Vec::new(),
        lignes: vec![ligne("Aucun diagnostic pour l'instant.".to_string(), "info")],
    };
    let (Some(d), Some(c)) = (historique.first(), cat) else { return carte };
    let i = info_diagnostic(Some(c), d);
    let Some(panne) = i.panne else { return carte };
    let quand = format_date(&texte(champ(d, "Debut")), false);
    let titre_panne = court(&texte(champ(panne, "Titre")), LIGNE_MAX);
    // (la panne, puis la réponse sur sa propre ligne : une seule phrase « panne : réponse » coupait la réponse)
    carte.lignes = if !vrai(champ(d, "Fini")) {
        vec![
            ligne(titre_panne, "info"),
            ligne(court(&format!("Pas finie (commencée le {quand}), à reprendre dans Mindcare."), LIGNE_MAX), "info"),
        ]
    } else {
        let urgent = i.suspect.is_some_and(|s| vrai(champ(s, "Urgent")));
        vec![
            ligne(titre_panne, "info"),
            ligne(phrase_carte(c, &i), if urgent { "urgent" } else { "attention" }),
            ligne(format!("Le {quand}"), "info"),
        ]
    };
    carte
}

/// L'écriture de etat.json (`Write-EtatMindset`) : seulement quand la carte change.
#[derive(Debug, Clone)]
pub struct EtatMindset {
    fichier: PathBuf,
    /// (la dernière carte écrite : rien de neuf, pas d'écriture)
    ecrit: Option<String>,
}

impl EtatMindset {
    pub fn nouveau(fichier: &Path) -> EtatMindset {
        EtatMindset { fichier: fichier.to_path_buf(), ecrit: None }
    }

    pub fn fichier(&self) -> &Path {
        &self.fichier
    }

    /// Écrit la carte dans etat.json si elle a changé depuis la dernière écriture.
    pub fn ecrire(&mut self, fichiers: &mut Fichiers, carte: &Carte) {
        let mut etat = serde_json::Map::new();
        etat.insert("Version".into(), json!(1));
        etat.insert("Titre".into(), json!(carte.titre));
        etat.insert("Scores".into(), Value::Array(carte.scores.iter().take(2).cloned().collect()));
        etat.insert(
            "Lignes".into(),
            Value::Array(carte.lignes.iter().take(3).map(|l| json!({ "Texte": l.texte, "Etat": l.etat })).collect()),
        );
        let cle = Value::Object(etat.clone()).to_string();
        // (comparé comme `-eq` de PowerShell : sans tenir compte des majuscules ; une carte qui ne change que de
        // majuscules n'est pas réécrite)
        if self.ecrit.as_deref().is_some_and(|e| egal(e, &cle)) {
            return;
        }
        etat.insert("Maj".into(), json!(fichiers.journal.maintenant().iso()));
        match fichiers.sauver(&self.fichier, &Value::Object(etat)) {
            Ok(()) => self.ecrit = Some(cle),
            Err(e) => fichiers.journal.ecrire(&format!("etat.json pour Mindset : {e}")),
        }
    }
}
