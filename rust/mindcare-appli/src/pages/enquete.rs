//! Une étape de l'enquête (Show-EtapeEnquete et Add-PanneauEnquete), et l'attente de ce que Windows a noté
//! (Wait-Journaux).

use serde_json::{Value, json};

use mindcare_moteur::ps::{champ, champ_v, egal, tableau, texte, vrai};
use mindcare_moteur::verdict::minuscule;
use mindcare_moteur::{Catalogue, Enquete, Objet};

use super::Etat;
use super::actions::ids;
use super::outils::{gestes_materiel, image, suspects_montres, t, textes, titre_suspect, urgent};

/// L'anneau des suspects (Add-AnneauSuspects) : combien sont déjà écartés, et la liste de ceux à montrer.
pub fn module_suspects(cat: &Catalogue, panne: &Objet, e: &Enquete, label: &str) -> Value {
    let tous = suspects_montres(cat, panne);
    if tous.is_empty() {
        return json!({ "icone": "loupe", "couleur": "P", "titre": t(panne, "Titre"), "texte": label });
    }
    let reste = |id: &str| e.restants.iter().any(|r| egal(r, id));
    let ecartes = tous.iter().filter(|s| !reste(s)).count();
    let restent = tous.len() - ecartes;
    let encore = match restent {
        0 => "tous vérifiés".to_string(),
        1 => "encore 1 à vérifier".to_string(),
        n => format!("encore {n} à vérifier"),
    };
    let ecartes_texte = if ecartes == 1 { "1 écarté".to_string() } else { format!("{ecartes} écartés") };
    let resume: Vec<Value> = tous
        .iter()
        .map(|s| {
            let r = reste(s);
            json!({ "nom": titre_suspect(cat, s), "valeur": if r { "À vérifier" } else { "Écarté" },
                    "ton": if r { "" } else { "ok" } })
        })
        .collect();
    json!({
        "anneau": { "valeur": ecartes, "sur": tous.len(), "texte": format!("{ecartes}/{}", tous.len()), "petit": "" },
        "titre": "Les suspects",
        "texte": format!("{ecartes_texte}, {encore}"),
        "resume": resume,
    })
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
        return page_attente(etat);
    };
    let id_etape = t(et, "Id");
    let test = egal(&t(et, "Type"), "test");
    let suspects = e.restants.iter().filter(|r| !urgent(cat, r)).count();
    let label = if test {
        let duree = t(et, "Duree");
        if duree.is_empty() {
            "Une manipulation à faire".to_string()
        } else {
            format!("Une manipulation à faire ({duree})")
        }
    } else if suspects > 1 {
        format!("Il reste {suspects} suspects")
    } else {
        "Une dernière vérification".to_string()
    };
    // « Étape 2 sur 5 » (le total peut baisser : des étapes deviennent inutiles en route)
    let n = e.faits.len() + 1;
    let total = (n + e.a_venir).max(n);

    let mut blocs = vec![json!({ "type": "jauge", "texte": format!("Étape {n} sur {total}"),
                                 "valeur": (100 * n) as f64 / total as f64 })];
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
        .map(|(k, r)| {
            json!({ "id": k.to_string(), "titre": texte(champ_v(Some(r), "Texte")),
                              "texte": "" })
        })
        .collect();
    blocs.push(json!({ "type": "cartes", "id": ids::REP, "options": options }));
    blocs.push(json!({ "type": "bouton", "id": ids::PRECEDENT, "texte": "Étape précédente", "style": "second" }));
    blocs.push(json!({ "type": "bouton", "id": ids::NSP, "texte": "Je ne sais pas", "style": "second" }));
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
    let action = if test {
        json!({ "id": ids::PLUS_TARD, "texte": "Je le ferai plus tard", "icone": "horloge" })
    } else {
        json!({ "id": ids::ACCUEIL, "texte": "Accueil", "icone": "maison" })
    };
    json!({
        "titre": t(et, "Texte"),
        "sous": format!("{} · {}", t(panne, "Titre"), minuscule(&label)),
        "icone": if test { "test" } else { "aide" },
        "couleur": if test { "T" } else { "P" },
        "retour": "appli",
        "actions": [action],
        "module": module_suspects(cat, panne, e, &label),
        "blocs": blocs,
    })
}

/// La page d'attente : sur le PC en panne, Mindcare lit ce que Windows a noté avant de continuer.
pub fn page_attente(etat: &Etat) -> Value {
    let panne = etat.en_cours.panne.as_ref().map(|p| t(p, "Titre")).unwrap_or_default();
    json!({
        "titre": "Mindcare regarde ce que Windows a noté...",
        "sous": if panne.is_empty() { "Un instant".to_string() } else { panne },
        "icone": "windows", "couleur": "S", "retour": "appli",
        "module": { "icone": "windows", "couleur": "S", "titre": "Un instant",
                    "texte": "Je regarde ce que Windows a noté, ça ne prend qu'un instant." },
        "blocs": [
            { "type": "texte",
              "texte": "Quelques secondes : ce que Windows a noté sur ce PC permet d'écarter des pistes sans rien te demander." },
            { "type": "bouton", "id": ids::PRECEDENT, "texte": "Étape précédente", "style": "second" },
            { "type": "bouton", "id": ids::ACCUEIL, "texte": "Accueil", "style": "second" },
        ],
    })
}
