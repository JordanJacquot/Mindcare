//! Une étape de l'enquête (Show-EtapeEnquete et Add-PanneauEnquete), et l'attente de ce que Windows a noté
//! (Wait-Journaux). Le design A2 : le fil d'Ariane (famille › panne) avec « Étape X sur N », l'avancée en segments, la
//! question en grand sur toute la largeur, les réponses en grands boutons, « Je ne sais pas » et « Étape précédente »
//! en liens, puis les suspects juste en dessous (ils suivent la hauteur de la question).

use serde_json::{Value, json};

use mindcare_moteur::ps::{champ, champ_v, egal, tableau, texte, vrai};
use mindcare_moteur::{Catalogue, Enquete, Objet};

use super::Etat;
use super::actions::ids;
use super::outils::{gestes_materiel, image, suspects_montres, t, textes, titre_suspect};

/// Les textes des étapes.
pub mod textes {
    pub const SUSPECTS: &str = "Les suspects";
    pub const A_VERIFIER: &str = "À vérifier";
    pub const ECARTE: &str = "Écarté";
    pub const NSP: &str = "Je ne sais pas";
    pub const PRECEDENT: &str = "Étape précédente";
    pub const PLUS_TARD: &str = "Je le ferai plus tard";
    pub const ACCUEIL: &str = "Accueil";
}

/// Le fil d'Ariane d'une panne : sa famille, puis son titre.
pub fn fil_panne(cat: &Catalogue, panne: &Objet) -> Vec<String> {
    let famille = cat.famille(&t(panne, "Famille")).map(|f| t(f, "Titre")).unwrap_or_default();
    [famille, t(panne, "Titre")].into_iter().filter(|x| !x.trim().is_empty()).collect()
}

/// Les suspects (Add-AnneauSuspects) : une petite carte par suspect à montrer, « À vérifier » ou « Écarté », avec le
/// compte (« 1 écarté, encore 5 à vérifier ») ; rien quand la panne n'en montre aucun. Un suspect qui n'a rien à voir
/// avec ce PC (le bloc d'alimentation d'un portable) n'y est pas du tout, ni barré ni compté.
pub fn bloc_suspects(cat: &Catalogue, panne: &Objet, e: &Enquete) -> Option<Value> {
    let tous: Vec<String> = suspects_montres(cat, panne).into_iter().filter(|s| !e.hors_sujet(cat, s)).collect();
    if tous.is_empty() {
        return None;
    }
    let reste = |id: &str| e.restants.iter().any(|r| egal(r, id));
    let ecartes = tous.iter().filter(|s| !reste(s)).count();
    let restent = tous.len() - ecartes;
    let encore = match restent {
        0 => "tous vérifiés".to_string(),
        1 => "encore 1 à vérifier".to_string(),
        n => format!("encore {n} à vérifier"),
    };
    let ecartes_texte = if ecartes <= 1 { format!("{ecartes} écarté") } else { format!("{ecartes} écartés") };
    let elements: Vec<Value> = tous
        .iter()
        .map(|s| {
            let r = reste(s);
            json!({ "nom": titre_suspect(cat, s), "etat": if r { textes::A_VERIFIER } else { textes::ECARTE },
                    "ton": if r { "" } else { "ok" } })
        })
        .collect();
    Some(json!({ "type": "suspects", "titre": textes::SUSPECTS, "droite": format!("{ecartes_texte}, {encore}"),
                 "elements": elements }))
}

/// Les commandes à taper dans le Terminal (Add-Commandes) : chacune avec son bouton « Copier ».
fn commandes(etat: &Etat, commande: &Objet) -> Vec<Value> {
    let ou = if vrai(champ(commande, "Admin")) {
        "À taper dans le Terminal (administrateur)"
    } else {
        "À taper dans le Terminal"
    };
    textes(commande, "Lignes")
        .into_iter()
        .enumerate()
        .map(|(k, ligne)| {
            let bouton = match etat.commande {
                Some((n, true)) if n == k => "Copiée",
                Some((n, false)) if n == k => "Copie impossible",
                _ => "Copier",
            };
            json!({ "type": "ligne", "icone": "windows", "couleur": "S", "texte": ligne, "note": ou, "brut": true,
                    "bouton": { "id": ids::COMMANDE, "texte": bouton, "valeur": k.to_string() } })
        })
        .collect()
}

/// La page d'une étape de l'enquête.
pub fn page(etat: &Etat, cat: &Catalogue, e: &Enquete) -> Value {
    let (Some(panne), Some(et)) = (etat.en_cours.panne.as_ref(), e.etape.as_ref()) else {
        return page_attente(etat, cat);
    };
    let id_etape = t(et, "Id");
    let test = egal(&t(et, "Type"), "test");
    // « Étape 2 sur 5 » (le total peut baisser : des étapes deviennent inutiles en route)
    let n = e.faits.len() + 1;
    let total = (n + e.a_venir).max(n);

    let mut blocs = vec![json!({ "type": "segments", "valeur": n, "sur": total })];
    let aide = t(et, "Aide");
    if !aide.is_empty() {
        blocs.push(json!({ "type": "texte", "texte": aide }));
    }
    let img = t(et, "Image");
    if !img.is_empty() {
        blocs.push(json!({ "type": "image", "url": image(&img), "hauteur": 160 }));
    }
    if test {
        let gestes = textes(et, "Gestes");
        let commande = champ(et, "Commande").and_then(Value::as_object).filter(|c| vrai(champ(c, "Lignes")));
        let mut posee = false;
        blocs.push(json!({ "type": "titre", "texte": "Coche chaque geste une fois fait" }));
        for (k, g) in gestes.iter().enumerate() {
            let oui = etat.gestes.iter().any(|(id, x)| *x == k && egal(id, &id_etape));
            blocs.push(json!({ "type": "case", "id": ids::GESTE, "valeur": k.to_string(),
                               "texte": format!("{}. {g}", k + 1), "oui": oui }));
            // (la commande juste sous le geste qui dit de la copier)
            if let Some(c) = commande
                && !posee
                && g.to_lowercase().contains("copier")
            {
                blocs.extend(commandes(etat, c));
                posee = true;
            }
        }
        if let Some(c) = commande
            && !posee
        {
            blocs.extend(commandes(etat, c));
        }
        // (la consigne de sécurité seulement pour les gestes sur le matériel, pas pour une manipulation dans Windows)
        if gestes_materiel(&gestes) {
            let securite = texte(cat.json().get("Securite").and_then(|s| s.as_array()).and_then(|a| a.first()));
            if !securite.is_empty() {
                blocs.push(json!({ "type": "note", "texte": securite }));
            }
        }
        blocs.push(json!({ "type": "titre", "texte": "Ensuite, dis à Mindcare ce que ça a donné" }));
    }
    let options: Vec<Value> = tableau(champ(et, "Reponses"))
        .into_iter()
        .enumerate()
        .map(|(k, r)| json!({ "id": k.to_string(), "titre": texte(champ_v(Some(r), "Texte")), "texte": "" }))
        .collect();
    blocs.push(json!({ "type": "cartes", "id": ids::REP, "options": options }));
    // (les liens sous les réponses : « Je ne sais pas » en couleur, les autres en gris)
    blocs.push(json!({ "type": "bouton", "id": ids::NSP, "texte": textes::NSP, "style": "lien" }));
    blocs.push(json!({ "type": "bouton", "id": ids::PRECEDENT, "texte": textes::PRECEDENT, "style": "discret" }));
    blocs.push(if test {
        json!({ "type": "bouton", "id": ids::PLUS_TARD, "texte": textes::PLUS_TARD, "style": "discret" })
    } else {
        json!({ "type": "bouton", "id": ids::ACCUEIL, "texte": textes::ACCUEIL, "style": "discret" })
    });
    blocs.extend(bloc_suspects(cat, panne, e));
    if !e.auto.is_empty() {
        blocs.push(json!({ "type": "titre", "texte": "Ce que Mindcare a vérifié" }));
        for a in &e.auto {
            blocs.push(json!({ "type": "ligne", "icone": if a.vu { "alerte" } else { "ok" },
                               "couleur": if a.vu { "orange" } else { "vert" }, "texte": a.texte }));
        }
    }
    if !e.faits.is_empty() {
        blocs.push(json!({ "type": "titre", "texte": "Tes réponses", "droite": "Clique pour changer" }));
        for (i, f) in e.faits.iter().enumerate() {
            let question = if f.court.is_empty() { f.texte.clone() } else { f.court.clone() };
            blocs.push(json!({ "type": "ligne", "id": format!("{}{i}", ids::CHANGER), "texte": question,
                               "note": f.reponse, "valeur": "Changer" }));
        }
    }
    let mut page = json!({
        "titre": t(et, "Texte"),
        "forme": "question",
        "chemin": fil_panne(cat, panne),
        "etape": format!("Étape {n} sur {total}"),
        "retour": "appli",
        "blocs": blocs,
    });
    if test {
        let duree = t(et, "Duree");
        page["sous"] = json!(if duree.is_empty() {
            "Une manipulation à faire".to_string()
        } else {
            format!("Une manipulation à faire ({duree})")
        });
    }
    page
}

/// La page d'attente : sur le PC en panne, Mindcare lit ce que Windows a noté avant de continuer.
pub fn page_attente(etat: &Etat, cat: &Catalogue) -> Value {
    let chemin = etat.en_cours.panne.as_ref().map(|p| fil_panne(cat, p)).unwrap_or_default();
    json!({
        "titre": "Mindcare regarde ce que Windows a noté...",
        "sous": "Je regarde ce que Windows a noté, ça ne prend qu'un instant.",
        "chemin": chemin,
        "retour": "appli",
        "blocs": [
            { "type": "texte",
              "texte": "Quelques secondes : ce que Windows a noté sur ce PC permet d'écarter des pistes sans rien te demander." },
            { "type": "bouton", "id": ids::PRECEDENT, "texte": textes::PRECEDENT, "style": "second" },
            { "type": "bouton", "id": ids::ACCUEIL, "texte": textes::ACCUEIL, "style": "second" },
        ],
    })
}
