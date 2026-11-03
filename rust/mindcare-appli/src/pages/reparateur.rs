//! « Pour le réparateur » (Add-Reparateur) : le rapport de l'enquête, montré dans la page (il ne s'ouvre jamais tout
//! seul dans un autre programme : décision de Jordan du 03/11), à enregistrer sur le Bureau ou à copier.
//!
//! Le texte montré est celui du fichier (`Get-ResumeTexte`, préparé par programme.rs dans [`Etat::rapport`]) : chaque
//! partie du texte (séparée par une ligne vide) devient une carte, les listes « - ... » des lignes de la carte, et
//! chaque réponse (« - la question » puis « la réponse » en retrait) une ligne avec sa réponse dessous.

use serde_json::{Value, json};

use mindcare_donnees::rapport::reparateur as textes;

use super::actions::ids;
use super::{Etat, ecrans};

/// Le titre du rapport dans la page quand son texte n'a pas de première ligne.
pub const TITRE_RAPPORT: &str = "Le rapport";

pub fn page(etat: &Etat) -> Value {
    let mut blocs = Vec::new();
    // (le dernier essai d'enregistrer ou de copier, en haut : il se voit sans rien faire défiler)
    if let Some(info) = &etat.info_rapport {
        blocs.push(json!({ "type": "ligne", "icone": if info.reussi { "ok" } else { "alerte" },
                           "couleur": if info.reussi { "vert" } else { "orange" }, "texte": info.texte }));
    }
    match etat.rapport.as_deref().filter(|t| !t.trim().is_empty()) {
        Some(t) => blocs.extend(blocs_du_rapport(t)),
        None => {
            // (le rapport n'est pas encore prêt : ce qu'il contiendra)
            let panne = etat.en_cours.panne.as_ref().map(|p| super::outils::t(p, "Titre")).unwrap_or_default();
            blocs.push(json!({ "type": "titre", "texte": "Ce qu'il contient" }));
            blocs.push(json!({ "type": "liste", "elements": [
                format!("La panne : {panne}"),
                "La réponse de Mindcare et comment on le sait",
                "Tes réponses, une par une",
                "Ce que Windows a noté (si l'enquête était faite sur ce PC)",
                "Le modèle du PC et sa version de Windows",
            ] }));
        }
    }
    blocs.push(
        json!({ "type": "note", "texte": "Rien n'est envoyé tout seul : c'est toi qui choisis à qui le montrer." }),
    );
    json!({
        "titre": textes::TITRE,
        "parent": ecrans::ENQUETE,
        "sous": textes::EXPLICATION,
        "icone": "rapport", "couleur": "P",
        "module": {
            "icone": "rapport", "titre": "Le rapport",
            "texte": "Le voici, tel qu'il sera enregistré ou copié.",
            "boutons": [
                { "id": ids::BUREAU, "texte": textes::BOUTON_ENREGISTRER },
                { "id": ids::COPIER, "texte": if etat.copie { "Copié" } else { textes::BOUTON_COPIER } },
            ],
        },
        "blocs": blocs,
    })
}

/// Une ligne de carte : son texte, et ce qui vient dessous (la réponse à une question).
struct Rang {
    texte: String,
    note: Vec<String>,
}

/// Les blocs du kit pour le texte du rapport : sa première ligne en titre, puis une carte par partie.
pub fn blocs_du_rapport(rapport: &str) -> Vec<Value> {
    let lignes: Vec<&str> = rapport.lines().collect();
    let mut blocs = Vec::new();
    let (titre, reste) = match lignes.split_first() {
        Some((t, r)) if !t.trim().is_empty() => (t.trim(), r),
        _ => (TITRE_RAPPORT, lignes.as_slice()),
    };
    blocs.push(json!({ "type": "titre", "texte": titre }));
    for partie in reste.split(|l| l.trim().is_empty()).filter(|p| !p.is_empty()) {
        // Une partie qui commence par son titre (« Comment on le sait : ») puis sa liste
        let (entete, corps) = match partie.split_first() {
            Some((e, c)) if !c.is_empty() && e.trim_end().ends_with(':') && c[0].starts_with("- ") => (Some(*e), c),
            _ => (None, partie),
        };
        if let Some(e) = entete {
            blocs.push(json!({ "type": "titre", "texte": e.trim_end().trim_end_matches(':').trim_end() }));
        }
        let mut rangs: Vec<Rang> = Vec::new();
        for l in corps {
            if let Some(x) = l.strip_prefix("- ") {
                rangs.push(Rang { texte: x.trim().to_string(), note: Vec::new() });
            } else if l.starts_with("  ")
                && let Some(r) = rangs.last_mut()
            {
                r.note.push(l.trim().to_string());
            } else {
                rangs.push(Rang { texte: l.trim().to_string(), note: Vec::new() });
            }
        }
        let rangs: Vec<Value> = rangs
            .into_iter()
            .map(|r| {
                if r.note.is_empty() {
                    json!({ "texte": r.texte })
                } else {
                    json!({ "texte": r.texte, "note": r.note.join(" ") })
                }
            })
            .collect();
        blocs.push(json!({ "type": "reglages", "rangs": rangs }));
    }
    blocs
}
