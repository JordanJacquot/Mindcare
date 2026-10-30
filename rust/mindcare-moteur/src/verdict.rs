//! La phrase de la réponse finale (Get-VerdictTitre) : « Une barrette de mémoire (RAM) est abîmée », « Il chauffe trop :
//! un ventilateur ne tourne plus », « Il reste la mémoire ou l'alimentation : un réparateur départagera ».

use crate::catalogue::Catalogue;
use crate::enquete::{TypeVerdict, Verdict};
use crate::ps::{champ, texte};

/// Un nom en minuscule au début (« la mémoire (RAM) »), sans toucher aux noms propres (Windows, RAM).
pub fn minuscule(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(premier) => premier.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

/// La phrase de la réponse finale (vide tant que l'enquête n'est pas finie).
pub fn titre_verdict(cat: &Catalogue, verdict: Option<&Verdict>) -> String {
    let Some(v) = verdict else { return String::new() };
    let champ_suspect =
        |id: Option<&String>, nom: &str| texte(id.and_then(|id| cat.suspect(id)).and_then(|s| champ(s, nom)));
    match v.genre {
        TypeVerdict::Trouve => {
            let trouve = champ_suspect(v.suspect.as_ref(), "Trouve");
            if v.detail.is_empty() { trouve } else { format!("{trouve} : {}", v.detail) }
        }
        TypeVerdict::Elimination => champ_suspect(v.suspect.as_ref(), "Trouve"),
        TypeVerdict::Reparateur => {
            let noms: Vec<String> = v.restants.iter().map(|id| minuscule(&champ_suspect(Some(id), "Titre"))).collect();
            format!("Il reste {} : un réparateur départagera", noms.join(" ou "))
        }
        TypeVerdict::Rien => "Aucune piste ne correspond : montre ce rapport à un réparateur".to_string(),
    }
}
