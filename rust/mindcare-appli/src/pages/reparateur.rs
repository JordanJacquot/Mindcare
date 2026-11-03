//! « Pour le réparateur » (Add-Reparateur) : le rapport de l'enquête, à enregistrer sur le Bureau ou à copier.

use serde_json::{Value, json};

use mindcare_donnees::rapport::reparateur as textes;

use super::actions::ids;
use super::{Etat, ecrans};

pub fn page(etat: &Etat) -> Value {
    let panne = etat.en_cours.panne.as_ref().map(|p| super::outils::t(p, "Titre")).unwrap_or_default();
    let mut blocs = vec![
        json!({ "type": "titre", "texte": "Ce qu'il contient" }),
        json!({ "type": "liste", "elements": [
            format!("La panne : {panne}"),
            "La réponse de Mindcare et comment on le sait",
            "Tes réponses, une par une",
            "Ce que Windows a noté (si l'enquête était faite sur ce PC)",
            "Le modèle du PC et sa version de Windows",
        ] }),
    ];
    if let Some(info) = &etat.info_rapport {
        blocs.push(json!({ "type": "ligne", "icone": if info.reussi { "ok" } else { "alerte" },
                           "couleur": if info.reussi { "vert" } else { "orange" }, "texte": info.texte }));
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
            "texte": "Un petit fichier texte, à montrer ou à envoyer au réparateur.",
            "boutons": [
                { "id": ids::BUREAU, "texte": textes::BOUTON_ENREGISTRER },
                { "id": ids::COPIER, "texte": if etat.copie { "Copié" } else { textes::BOUTON_COPIER } },
            ],
        },
        "blocs": blocs,
    })
}
