//! « Ça concerne quoi ? » (les familles de pannes) et une famille (ses pannes).

use serde_json::{Value, json};

use mindcare_moteur::ps::egal;
use mindcare_moteur::{Catalogue, Objet};

use super::ecrans;
use super::outils::{image, nombre, t};
use super::raconte::PANNE;

/// Le début de l'id d'une famille à ouvrir (« famille:plantages »).
pub const FAMILLE: &str = "famille:";

/// Les pannes d'une famille, dans l'ordre du catalogue.
pub fn pannes_de<'a>(cat: &'a Catalogue, famille: &str) -> Vec<&'a Objet> {
    cat.pannes().iter().filter(|p| egal(&t(p, "Famille"), famille)).collect()
}

pub fn page_familles(cat: &Catalogue) -> Value {
    const COULEURS: [&str; 3] = ["P", "S", "T"];
    let elements: Vec<Value> = cat
        .familles()
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let id = t(f, "Id");
            let sous = if pannes_de(cat, &id).is_empty() { "Arrive bientôt".to_string() } else { t(f, "Phrase") };
            json!({ "id": format!("{FAMILLE}{id}"), "titre": t(f, "Titre"), "sous": sous, "image": image(&t(f, "Icone")),
                    "couleur": COULEURS[i % COULEURS.len()] })
        })
        .collect();
    json!({
        "titre": "Ça concerne quoi ?",
        "sous": format!("{}, {} : choisis ce qui ressemble le plus à ce qui se passe.",
            nombre(cat.familles().len(), "famille", "familles"), nombre(cat.pannes().len(), "panne", "pannes")),
        "icone": "liste", "couleur": "S",
        "blocs": [{ "type": "grille", "elements": elements }],
    })
}

/// Une famille et ses pannes (rien pour une famille inconnue).
pub fn page_famille(cat: &Catalogue, id: &str) -> Option<Value> {
    let f = cat.famille(id)?;
    let pannes = pannes_de(cat, &t(f, "Id"));
    let mut blocs = Vec::new();
    if pannes.is_empty() {
        blocs
            .push(json!({ "type": "note", "texte": "Arrive bientôt : cette famille n'a pas encore de panne connue." }));
    } else {
        blocs.push(json!({ "type": "titre", "texte": "Choisis la phrase qui ressemble le plus",
                           "droite": nombre(pannes.len(), "panne", "pannes") }));
        for p in &pannes {
            blocs.push(json!({ "type": "ligne", "id": format!("{PANNE}{}", t(p, "Id")), "texte": t(p, "Titre"),
                               "note": t(p, "Phrase"), "valeur": "Commencer" }));
        }
    }
    blocs.push(json!({ "type": "note",
        "texte": "Rien ne ressemble ? Raconte ton problème avec tes mots, ou change de famille. Ton cas pourra être ajouté." }));
    Some(json!({
        "titre": t(f, "Titre"), "parent": ecrans::FAMILLES, "sous": "Qu'est-ce qui se passe ?",
        "icone": "liste", "couleur": "S",
        "module": { "image": image(&t(f, "Icone")), "titre": t(f, "Titre"), "texte": t(f, "Phrase"),
                    "resume": [{ "nom": "Pannes connues", "valeur": pannes.len().to_string() }] },
        "blocs": blocs,
    }))
}
