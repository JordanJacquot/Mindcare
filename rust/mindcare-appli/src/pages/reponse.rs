//! La réponse finale (Show-Verdict) : trouvée, tout le reste écarté, urgente, ou ce qui reste pour le réparateur ;
//! comment on le sait, ce qui est écarté, ce qu'il faut faire, la garantie avant d'acheter une pièce, la sécurité.

use serde_json::{Value, json};

use mindcare_donnees::{TitreReponse, titre_reponse};
use mindcare_moteur::ps::{champ, egal, parcours, texte, vrai};
use mindcare_moteur::verdict::minuscule;
use mindcare_moteur::{Catalogue, Enquete, TypeVerdict};

use super::Etat;
use super::actions::ids;
use super::outils::{t, textes, titre_suspect, urgent};

/// Le badge de la réponse, sa couleur et son icône.
pub struct Badge {
    pub texte: &'static str,
    pub couleur: &'static str,
    pub icone: &'static str,
}

/// Le badge de la réponse (les étiquettes de Show-Verdict).
pub fn badge(genre: TypeVerdict, urgent: bool, supposee: bool) -> Badge {
    let b = |texte, couleur, icone| Badge { texte, couleur, icone };
    match genre {
        TypeVerdict::Trouve if urgent => b("Urgent", "rouge", "alerte"),
        TypeVerdict::Trouve => b("Trouvé", "vert", "coche"),
        TypeVerdict::Elimination if supposee => b("La piste la plus probable", "orange", "aide"),
        TypeVerdict::Elimination => b("Tout le reste est écarté", "vert", "coche"),
        TypeVerdict::Reparateur => b("À faire vérifier par un réparateur", "orange", "aide"),
        TypeVerdict::Rien => b("Aucune piste ne correspond", "orange", "aide"),
    }
}

/// Avant d'acheter une pièce : confirmée (la garantie), ou seulement restée par élimination (pas testée).
pub const PIECE_TITRE: &str = "Avant d'acheter une pièce";
pub const PIECE_SUPPOSEE: &str = "Mindcare n'a pas pu tester cette pièce : elle reste parce que tout le reste est écarté. Avant d'en acheter une neuve, fais-la vérifier par un réparateur, ou essaie avec celle d'un proche.";
pub const PIECE_GARANTIE: &str = "Si le PC a moins de 2 ans, il est encore sous garantie : contacte d'abord le magasin ou le fabricant, la réparation peut être gratuite. Dans le doute, un réparateur peut confirmer avant que tu achètes.";

pub fn page(etat: &Etat, cat: &Catalogue, e: &Enquete) -> Value {
    let panne = etat.en_cours.panne.as_ref().map(|p| t(p, "Titre")).unwrap_or_default();
    let Some(v) = e.verdict.as_ref() else {
        return super::enquete::page_attente(etat);
    };
    let TitreReponse { titre, piece, supposee } = titre_reponse(cat, e);
    let suspect = v.suspect.as_deref().and_then(|s| cat.suspect(s));
    let est_urgent = suspect.is_some_and(|s| vrai(champ(s, "Urgent")));
    let b = badge(v.genre, est_urgent, supposee);

    let mut blocs = Vec::new();
    if piece {
        blocs.push(json!({ "type": "titre", "texte": PIECE_TITRE }));
        blocs.push(json!({ "type": "note", "texte": if supposee { PIECE_SUPPOSEE } else { PIECE_GARANTIE } }));
    }

    // Comment on le sait : les preuves, ce que Windows a noté, ce qui est écarté
    blocs.push(json!({ "type": "titre", "texte": "Comment on le sait" }));
    if !e.preuves.is_empty() {
        blocs.push(json!({ "type": "liste", "ton": "ok", "elements": e.preuves }));
    }
    for a in e.auto.iter().filter(|a| !e.preuves.iter().any(|p| p == &a.texte)) {
        blocs.push(json!({ "type": "ligne", "icone": if a.vu { "alerte" } else { "ok" },
                           "couleur": if a.vu { "orange" } else { "vert" }, "texte": a.texte }));
    }
    if e.preuves.is_empty() && e.auto.is_empty() {
        blocs.push(json!({ "type": "texte", "texte": "Tes réponses ne permettaient pas d'écarter de piste." }));
    }
    let mut ecartes: Vec<String> = Vec::new();
    for x in e.ecartes.iter().filter(|x| !urgent(cat, &x.id)) {
        let nom = minuscule(&titre_suspect(cat, &x.id));
        if !ecartes.contains(&nom) {
            ecartes.push(nom);
        }
    }
    if !ecartes.is_empty() {
        blocs.push(json!({ "type": "texte", "texte": format!("Écartés : {}.", ecartes.join(", ")) }));
    }

    // Ce qu'il faut faire
    blocs.push(json!({ "type": "titre", "texte": "Ce qu'il faut faire" }));
    if let Some(s) = suspect {
        let pourquoi = t(s, "Pourquoi");
        if !pourquoi.is_empty() {
            blocs.push(json!({ "type": "texte", "texte": pourquoi }));
        }
        let a_faire: Vec<Value> = textes(s, "AFaire").into_iter().map(|x| json!({ "texte": x })).collect();
        if !a_faire.is_empty() {
            blocs.push(json!({ "type": "etapes", "elements": a_faire }));
        }
        let cepc = e.notes.iter().any(|n| egal(n, "cepc"));
        if v.suspect.as_deref().is_some_and(|x| egal(x, "memoire")) && cepc && !etat.barrettes.is_empty() {
            blocs.push(json!({ "type": "texte",
                               "texte": format!("Mindcare a lu les barrettes de ce PC : {}.", etat.barrettes.join(" ; ")) }));
        }
        let pro = t(s, "Pro");
        if !pro.is_empty() {
            blocs.push(json!({ "type": "titre", "texte": "Quand aller chez un réparateur" }));
            blocs.push(json!({ "type": "texte", "texte": pro }));
        }
    } else if v.genre == TypeVerdict::Reparateur {
        blocs.push(json!({ "type": "texte",
            "texte": "Ces pistes se départagent avec un outil de réparateur. Montre-lui le rapport (bouton « Pour le réparateur ») : il saura par où commencer." }));
        for id in &v.restants {
            let r = cat.suspect(id);
            blocs.push(json!({ "type": "ligne", "icone": "aide", "couleur": "orange",
                               "texte": r.map(|r| t(r, "Titre")).unwrap_or_else(|| id.clone()),
                               "note": r.map(|r| t(r, "Pourquoi")).unwrap_or_default() }));
        }
    } else {
        blocs.push(json!({ "type": "texte",
            "texte": "Aucun suspect ne tient après tes réponses. La panne est peut-être ailleurs : essaie une autre panne qui ressemble, ou montre le rapport à un réparateur." }));
    }
    let securite: Vec<String> =
        parcours(cat.json().get("Securite")).into_iter().map(|x| texte(Some(x))).filter(|x| !x.is_empty()).collect();
    if !securite.is_empty() {
        blocs.push(json!({ "type": "note", "texte": securite.join(" ") }));
    }

    json!({
        "titre": panne,
        "sous": "La réponse de Mindcare",
        "icone": "ok",
        "couleur": b.couleur,
        "retour": "appli",
        "actions": [
            { "id": ids::PRECEDENT, "texte": "Changer ma dernière réponse" },
            { "id": ids::REPARATEUR, "texte": "Pour le réparateur", "icone": "rapport" },
        ],
        "module": { "icone": b.icone, "couleur": b.couleur, "titre": b.texte, "texte": titre,
                    "boutons": [{ "id": ids::ACCUEIL, "texte": "Retour à l'accueil" }] },
        "blocs": blocs,
    })
}
