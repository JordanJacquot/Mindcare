//! La réponse finale (Show-Verdict) : trouvée, tout le reste écarté, urgente, ou ce qui reste pour le réparateur.
//! Le design A2 (Jordan trouvait l'ancienne trop chargée) : en haut la carte « verdict » (la pastille et la couleur du
//! cas, le sur-titre, la conclusion en une phrase, une ligne d'explication) ; puis « Ce qu'il faut faire » en liste
//! numérotée, avec en dernier le conseil pour le réparateur (et la garantie avant d'acheter une pièce) ; « Comment
//! Mindcare le sait » replié (une ligne avec son résumé) ; la sécurité en une petite ligne en bas ; les boutons dans
//! le pied de la page, toujours visibles.

use serde_json::{Value, json};

use mindcare_donnees::{TitreReponse, titre_reponse};
use mindcare_moteur::ps::{champ, egal, parcours, texte, vrai};
use mindcare_moteur::verdict::minuscule;
use mindcare_moteur::{Catalogue, Enquete, TypeVerdict};

use super::Etat;
use super::actions::ids;
use super::outils::{nombre, t, textes, titre_suspect, urgent};

/// Le badge de la réponse, sa couleur et son icône.
pub struct Badge {
    pub texte: &'static str,
    pub couleur: &'static str,
    pub icone: &'static str,
}

/// Le badge de la réponse (les étiquettes de Show-Verdict) : jamais une coche sur « Urgent ».
pub fn badge(genre: TypeVerdict, urgent: bool, supposee: bool) -> Badge {
    let b = |texte, couleur, icone| Badge { texte, couleur, icone };
    match genre {
        TypeVerdict::Trouve if urgent => b("Urgent", "rouge", "alerte"),
        TypeVerdict::Trouve => b("Trouvé", "vert", "coche"),
        TypeVerdict::Elimination if supposee => b("La piste la plus probable", "orange", "question"),
        TypeVerdict::Elimination => b("Tout le reste est écarté", "vert", "coche"),
        TypeVerdict::Reparateur => b("À faire vérifier par un réparateur", "orange", "question"),
        TypeVerdict::Rien => b("Aucune piste ne correspond", "orange", "question"),
    }
}

/// Avant d'acheter une pièce : confirmée (la garantie), ou seulement restée par élimination (pas testée).
pub const PIECE_TITRE: &str = "Avant d'acheter une pièce";
pub const PIECE_SUPPOSEE: &str = "Mindcare n'a pas pu tester cette pièce : elle reste parce que tout le reste est écarté. Avant d'en acheter une neuve, fais-la vérifier par un réparateur, ou essaie avec celle d'un proche.";
pub const PIECE_GARANTIE: &str = "Si le PC a moins de 2 ans, il est encore sous garantie : contacte d'abord le magasin ou le fabricant, la réparation peut être gratuite. Dans le doute, un réparateur peut confirmer avant que tu achètes.";

/// Les textes de la page.
pub mod textes_reponse {
    pub const A_FAIRE: &str = "Ce qu'il faut faire";
    pub const CHEZ_REPARATEUR: &str = "Ensuite, chez un réparateur :";
    pub const COMMENT: &str = "Comment Mindcare le sait";
    pub const RESTE: &str = "Ce qui reste à vérifier";
    pub const RIEN_ECARTE: &str = "Tes réponses ne permettaient pas d'écarter de piste.";
    pub const ACCUEIL: &str = "Retour à l'accueil";
    pub const REPARATEUR: &str = "Pour le réparateur";
    pub const CHANGER: &str = "Changer ma dernière réponse";
    /// L'explication sous la conclusion, sans suspect trouvé
    pub const EXPLICATION_REPARATEUR: &str = "Ces pistes se départagent avec un outil de réparateur.";
    pub const EXPLICATION_RIEN: &str = "Aucun suspect ne tient après tes réponses : la panne est peut-être ailleurs.";
    pub const CONSEIL_REPARATEUR: &str =
        "montre-lui le rapport (bouton « Pour le réparateur ») : il saura par où commencer.";
    pub const AUTRE_PANNE: &str = "Essaie une autre panne qui ressemble à ce qui se passe.";
    pub const CONSEIL_RIEN: &str = "montre-lui le rapport (bouton « Pour le réparateur »).";
}
use textes_reponse as x;

pub fn page(etat: &Etat, cat: &Catalogue, e: &Enquete) -> Value {
    let p = etat.en_cours.panne.as_ref();
    let panne = p.map(|p| t(p, "Titre")).unwrap_or_default();
    let Some(v) = e.verdict.as_ref() else {
        return super::enquete::page_attente(etat, cat);
    };
    let TitreReponse { titre, piece, supposee } = titre_reponse(cat, e);
    let suspect = v.suspect.as_deref().and_then(|s| cat.suspect(s));
    let est_urgent = suspect.is_some_and(|s| vrai(champ(s, "Urgent")));
    let b = badge(v.genre, est_urgent, supposee);

    // L'explication sous la conclusion : pourquoi ce suspect, ou ce que veut dire la réponse
    let explication = match suspect {
        Some(s) => t(s, "Pourquoi"),
        None if v.genre == TypeVerdict::Reparateur => x::EXPLICATION_REPARATEUR.to_string(),
        None => x::EXPLICATION_RIEN.to_string(),
    };
    let mut verdict = json!({ "type": "verdict", "icone": b.icone, "couleur": b.couleur, "surtitre": b.texte,
                              "titre": titre });
    if !explication.is_empty() {
        verdict["texte"] = json!(explication);
    }
    let mut blocs = vec![verdict];

    // Ce qui reste à vérifier (sans suspect trouvé, pour le réparateur)
    if suspect.is_none() && v.genre == TypeVerdict::Reparateur && !v.restants.is_empty() {
        blocs.push(json!({ "type": "titre", "texte": x::RESTE }));
        for id in &v.restants {
            let r = cat.suspect(id);
            blocs.push(json!({ "type": "ligne", "icone": "aide", "couleur": "orange",
                               "texte": r.map(|r| t(r, "Titre")).unwrap_or_else(|| id.clone()),
                               "note": r.map(|r| t(r, "Pourquoi")).unwrap_or_default() }));
        }
    }

    // Ce qu'il faut faire : la liste numérotée, puis le conseil pour le réparateur (et avant d'acheter une pièce)
    let mut a_faire: Vec<Value> = Vec::new();
    if let Some(s) = suspect {
        a_faire.extend(textes(s, "AFaire").into_iter().map(|x| json!({ "texte": x })));
        let cepc = e.notes.iter().any(|n| egal(n, "cepc"));
        if v.suspect.as_deref().is_some_and(|x| egal(x, "memoire")) && cepc && !etat.barrettes.is_empty() {
            a_faire.push(json!({ "icone": "memoire",
                "texte": format!("Mindcare a lu les barrettes de ce PC : {}.", etat.barrettes.join(" ; ")) }));
        }
        let pro = t(s, "Pro");
        if !pro.is_empty() {
            a_faire.push(json!({ "icone": "cle", "titre": x::CHEZ_REPARATEUR, "texte": pro }));
        }
    } else if v.genre == TypeVerdict::Reparateur {
        a_faire.push(json!({ "icone": "cle", "titre": x::CHEZ_REPARATEUR, "texte": x::CONSEIL_REPARATEUR }));
    } else {
        a_faire.push(json!({ "texte": x::AUTRE_PANNE }));
        a_faire.push(json!({ "icone": "cle", "titre": x::CHEZ_REPARATEUR, "texte": x::CONSEIL_RIEN }));
    }
    if piece {
        a_faire.push(json!({ "icone": "euro", "titre": format!("{PIECE_TITRE} :"),
                             "texte": if supposee { PIECE_SUPPOSEE } else { PIECE_GARANTIE } }));
    }
    if !a_faire.is_empty() {
        blocs.push(json!({ "type": "titre", "texte": x::A_FAIRE }));
        blocs.push(json!({ "type": "etapes", "elements": a_faire }));
    }

    // Comment Mindcare le sait, replié : les preuves, ce que Windows a noté, ce qui est écarté
    let mut preuves = Vec::new();
    if !e.preuves.is_empty() {
        preuves.push(json!({ "type": "liste", "ton": "ok", "elements": e.preuves }));
    }
    let autres: Vec<_> = e.auto.iter().filter(|a| !e.preuves.iter().any(|p| p == &a.texte)).collect();
    for a in &autres {
        preuves.push(json!({ "type": "ligne", "icone": if a.vu { "alerte" } else { "ok" },
                             "couleur": if a.vu { "orange" } else { "vert" }, "texte": a.texte }));
    }
    if e.preuves.is_empty() && e.auto.is_empty() {
        preuves.push(json!({ "type": "texte", "texte": x::RIEN_ECARTE }));
    }
    let mut ecartes: Vec<String> = Vec::new();
    for x in e.ecartes.iter().filter(|x| !urgent(cat, &x.id)) {
        let nom = minuscule(&titre_suspect(cat, &x.id));
        if !ecartes.contains(&nom) {
            ecartes.push(nom);
        }
    }
    if !ecartes.is_empty() {
        preuves.push(json!({ "type": "texte", "texte": format!("Écartés : {}.", ecartes.join(", ")) }));
    }
    let indices = e.preuves.len() + autres.iter().filter(|a| a.vu).count();
    let mut resume = Vec::new();
    if indices > 0 {
        resume.push(nombre(indices, "indice", "indices"));
    }
    if !ecartes.is_empty() {
        resume.push(nombre(ecartes.len(), "piste écartée", "pistes écartées"));
    }
    let resume = if resume.is_empty() { "Rien d'écarté".to_string() } else { resume.join(", ") };
    blocs.push(json!({ "type": "repli", "titre": x::COMMENT, "resume": resume, "blocs": preuves }));

    // Les boutons, dans le pied de la page (toujours visibles, même quand la réponse est longue) : l'accueil d'abord
    // (choisi à l'ouverture), le rapport, changer la dernière réponse
    let pied = json!([
        { "type": "bouton", "id": ids::ACCUEIL, "texte": x::ACCUEIL },
        { "type": "bouton", "id": ids::REPARATEUR, "texte": x::REPARATEUR, "icone": "rapport", "style": "second" },
        { "type": "bouton", "id": ids::PRECEDENT, "texte": x::CHANGER, "style": "discret" },
    ]);

    let securite: Vec<String> =
        parcours(cat.json().get("Securite")).into_iter().map(|x| texte(Some(x))).filter(|x| !x.is_empty()).collect();
    if !securite.is_empty() {
        blocs.push(json!({ "type": "petit", "texte": securite.join(" ") }));
    }

    let chemin: Vec<&str> = [panne.as_str(), "La réponse"].into_iter().filter(|c| !c.is_empty()).collect();
    json!({
        "titre": panne,
        "sous": "La réponse de Mindcare",
        "forme": "carte",
        "chemin": chemin,
        "retour": "appli",
        "focus": format!("o:{}", ids::ACCUEIL),
        "blocs": blocs,
        "pied": pied,
    })
}
