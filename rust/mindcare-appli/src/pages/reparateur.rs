//! « Pour le réparateur » (Add-Reparateur) : le rapport de l'enquête, montré dans la page (il ne s'ouvre jamais tout
//! seul dans un autre programme : décision de Jordan du 03/11), à enregistrer sur le Bureau ou à copier.
//!
//! Le texte montré est celui du fichier (`Get-ResumeTexte`, préparé par programme.rs dans [`Etat::rapport`]), rangé
//! comme dans le design A2 : en haut la panne, la réponse et la date ; à gauche ce qui est sûr, ce qui est écarté (et
//! pourquoi), ce qui reste à vérifier, les questions et les réponses ; à droite le PC et ce que Windows a noté. Chaque
//! ligne du texte est dans la page, et seulement elles (le texte copié ou enregistré ne change pas) : l'enquête
//! gardée ne sert qu'à séparer un nom de sa raison (« L'alimentation (le PC s'allume) »), et à savoir si Windows a
//! vu quelque chose.

use serde_json::{Value, json};

use mindcare_donnees::rapport::reparateur as textes;
use mindcare_donnees::titre_reponse;
use mindcare_moteur::ps::{champ, vrai};
use mindcare_moteur::{Catalogue, Enquete, HORS_SUJET, TypeVerdict};

use super::actions::ids;
use super::outils::{t, titre_suspect, urgent};
use super::reponse::badge;
use super::{Etat, ecrans};

/// Les textes de la page (ceux de la maquette validée).
pub mod titres {
    pub const SOUS: &str =
        "Tout ce que Mindcare a vu et vérifié, pour qu'un réparateur gagne du temps (et toi de l'argent).";
    pub const COPIER: &str = "Copier";
    pub const COPIE: &str = "Copié";
    pub const ENREGISTRER: &str = "Enregistrer sur le Bureau";
    pub const SUR: &str = "Ce qui est sûr";
    pub const ECARTE: &str = "Ce qui est écarté, et pourquoi";
    pub const RESTE: &str = "Ce qui reste à vérifier";
    pub const QUESTIONS: &str = "Les questions de Mindcare et tes réponses";
    pub const PC: &str = "Le PC";
    pub const WINDOWS: &str = "Ce que Windows a noté";
    pub const PC_RIEN: &str = "Mindcare lit ce PC seulement quand la panne est sur lui : rien de lu cette fois.";
    pub const QUESTIONS_RIEN: &str = "Aucune question n'a été posée.";
    pub const RIEN_ENVOYE: &str = "Rien n'est envoyé tout seul : c'est toi qui choisis à qui le montrer.";
}

/// Les débuts des lignes du texte (`texte_rapport` de mindcare-donnees).
mod lignes {
    pub const DATE: &str = "Date";
    pub const PANNE: &str = "Panne";
    pub const REPONSE: &str = "Réponse";
    pub const IL_RESTE: &str = "Il reste";
    pub const A_SAVOIR: &str = "À savoir";
    pub const PREUVES: &str = "Comment on le sait";
    pub const ECARTES: &str = "Écartés";
    pub const VERIFIE: &str = "Ce que Mindcare a vérifié";
    pub const REPONSES: &str = "Les réponses";
    pub const FAIT: &str = "Fait avec Mindcare";
}

pub fn page(etat: &Etat, cat: &Catalogue) -> Value {
    let mut blocs = Vec::new();
    // (le dernier essai d'enregistrer ou de copier, en haut : il se voit sans rien faire défiler)
    if let Some(info) = &etat.info_rapport {
        blocs.push(json!({ "type": "ligne", "icone": if info.reussi { "ok" } else { "alerte" },
                           "couleur": if info.reussi { "vert" } else { "orange" }, "texte": info.texte }));
    }
    let enquete = etat.en_cours.resume_de.as_ref().map(|r| &r.enquete);
    match etat.rapport.as_deref().filter(|t| !t.trim().is_empty()) {
        Some(t) => blocs.extend(blocs_du_rapport(t, cat, enquete)),
        None => {
            // (le rapport n'est pas encore prêt : ce qu'il contiendra)
            let panne = etat.en_cours.panne.as_ref().map(|p| t(p, "Titre")).unwrap_or_default();
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
    blocs.push(json!({ "type": "petit", "texte": titres::RIEN_ENVOYE }));
    let panne = etat.en_cours.panne.as_ref().map(|p| t(p, "Titre")).unwrap_or_default();
    let chemin: Vec<&str> = [panne.as_str(), textes::TITRE].into_iter().filter(|c| !c.is_empty()).collect();
    json!({
        "titre": textes::TITRE,
        "parent": ecrans::ENQUETE,
        "chemin": chemin,
        "sous": titres::SOUS,
        "actions": [
            { "id": ids::COPIER, "texte": if etat.copie { titres::COPIE } else { titres::COPIER }, "style": "second" },
            { "id": ids::BUREAU, "texte": titres::ENREGISTRER, "style": "principal" },
        ],
        "focus": format!("o:{}", ids::BUREAU),
        "blocs": blocs,
    })
}

/// Le texte du rapport, lu partie par partie (les parties sont séparées par une ligne vide).
#[derive(Debug, Default)]
struct Lu {
    date: String,
    panne: String,
    reponse: String,
    /// « Ordinateur : ... », « Mémoire : ... » (et ce que la détection du PC y ajoutera)
    pc: Vec<(String, String)>,
    surs: Vec<String>,
    ecartes: Vec<String>,
    reste: Option<String>,
    windows: Vec<String>,
    a_savoir: Vec<String>,
    reponses: Vec<(String, String)>,
    fin: Vec<String>,
    /// Une partie que la page ne connaît pas : son titre (s'il y en a un) et ses lignes, montrées telles quelles
    autres: Vec<(Option<String>, Vec<String>)>,
}

/// « Clé : valeur » (l'espace avant les deux-points, comme le texte les écrit).
fn cle_valeur(l: &str) -> Option<(&str, &str)> {
    l.split_once(" : ").map(|(c, v)| (c.trim(), v.trim()))
}

fn lire(rapport: &str) -> Lu {
    let lignes: Vec<&str> = rapport.lines().collect();
    // (la première ligne est le titre du texte, « Mindcare : rapport de l'enquête » : la page a le sien)
    let reste = match lignes.split_first() {
        Some((t, r)) if !t.trim().is_empty() => r,
        _ => lignes.as_slice(),
    };
    let mut lu = Lu::default();
    for partie in reste.split(|l| l.trim().is_empty()).filter(|p| !p.is_empty()) {
        let (entete, corps) = partie.split_first().expect("une partie non vide");
        let entete_t = entete.trim();
        // Une partie qui commence par son titre (« Comment on le sait : ») puis sa liste
        if entete_t.ends_with(':') && corps.iter().all(|l| l.starts_with("- ") || l.starts_with("  ")) {
            let nom = entete_t.trim_end_matches(':').trim_end();
            let items = || corps.iter().filter_map(|l| l.strip_prefix("- ")).map(|x| x.trim().to_string());
            if nom.starts_with(lignes::PREUVES) {
                lu.surs.extend(items());
            } else if nom.starts_with(lignes::ECARTES) {
                lu.ecartes.extend(items());
            } else if nom.starts_with(lignes::VERIFIE) {
                lu.windows.extend(items());
            } else if nom.starts_with(lignes::REPONSES) {
                for l in corps.iter() {
                    if let Some(q) = l.strip_prefix("- ") {
                        lu.reponses.push((q.trim().to_string(), String::new()));
                    } else if let Some(r) = lu.reponses.last_mut() {
                        let r = &mut r.1;
                        if !r.is_empty() {
                            r.push(' ');
                        }
                        r.push_str(l.trim());
                    }
                }
            } else {
                let l: Vec<String> =
                    corps.iter().map(|x| x.strip_prefix("- ").unwrap_or(x).trim().to_string()).collect();
                lu.autres.push((Some(nom.to_string()), l));
            }
            continue;
        }
        for l in partie.iter().map(|l| l.trim()) {
            match cle_valeur(l) {
                Some((lignes::DATE, v)) => lu.date = v.to_string(),
                Some((lignes::PANNE, v)) => lu.panne = v.to_string(),
                Some((lignes::REPONSE, v)) => lu.reponse = v.to_string(),
                Some((lignes::IL_RESTE, v)) => lu.reste = Some(v.to_string()),
                Some((lignes::A_SAVOIR, _)) => lu.a_savoir.push(l.to_string()),
                _ if l.starts_with(lignes::FAIT) => lu.fin.push(l.to_string()),
                Some((c, v)) => lu.pc.push((c.to_string(), v.to_string())),
                None => lu.autres.push((None, vec![l.to_string()])),
            }
        }
    }
    lu
}

/// Une carte de lignes (rien quand elle est vide, sauf avec une phrase pour le dire).
fn rangs(titre: &str, forme: &str, rangs: Vec<Value>, note: Option<&str>) -> Option<Value> {
    if rangs.is_empty() && note.is_none() {
        return None;
    }
    let mut b = json!({ "type": "rangs", "titre": titre, "forme": forme, "rangs": rangs });
    if let Some(n) = note {
        b["note"] = json!(n);
    }
    Some(b)
}

/// Les blocs du kit pour le texte du rapport (avec l'enquête gardée, pour séparer les noms de leurs raisons).
pub fn blocs_du_rapport(rapport: &str, cat: &Catalogue, enquete: Option<&Enquete>) -> Vec<Value> {
    let lu = lire(rapport);
    let mut blocs = Vec::new();

    // En haut : la panne, la réponse, la date (l'icône et la couleur de la réponse)
    let (icone, couleur) = match enquete.and_then(|e| e.verdict.as_ref().map(|v| (e, v))) {
        Some((e, v)) => {
            let s = v.suspect.as_deref().and_then(|s| cat.suspect(s));
            let b = badge(v.genre, s.is_some_and(|s| vrai(champ(s, "Urgent"))), titre_reponse(cat, e).supposee);
            (b.icone, b.couleur)
        }
        None => ("rapport", "P"),
    };
    let mut haut = json!({ "type": "verdict", "compact": true, "icone": icone, "couleur": couleur,
                           "titre": if lu.reponse.is_empty() { textes::TITRE } else { lu.reponse.as_str() } });
    if !lu.panne.is_empty() {
        haut["petit"] = json!(format!("{} : {}", lignes::PANNE, lu.panne));
    }
    if !lu.date.is_empty() {
        haut["droite"] = json!(format!("Enquête du {}", lu.date));
    }
    blocs.push(haut);

    // Les noms des écartés (avec leur raison) et de ce qui reste, d'après l'enquête gardée
    let ligne_ecarte = |id: &str, raison: &str| {
        let nom = titre_suspect(cat, id);
        let ligne = if raison.is_empty() { nom.clone() } else { format!("{nom} ({raison})") };
        (ligne, nom)
    };
    let ecartes_connus: Vec<(String, String, String)> = enquete
        .map(|e| {
            e.ecartes_montres(cat)
                .filter(|x| !urgent(cat, &x.id))
                .map(|x| {
                    let (ligne, nom) = ligne_ecarte(&x.id, &x.raison);
                    (ligne, nom, x.raison.clone())
                })
                .collect()
        })
        .unwrap_or_default();
    // Ce qui n'a rien à voir avec ce PC (le bloc d'alimentation d'un portable) n'est pas montré, même dans un rapport
    // écrit par une version d'avant qui le mettait dans « Écartés »
    let hors_sujet: Vec<String> = enquete
        .map(|e| {
            e.ecartes.iter().filter(|x| e.hors_sujet(cat, &x.id)).map(|x| ligne_ecarte(&x.id, &x.raison).0).collect()
        })
        .unwrap_or_default();
    let a_montrer = |l: &&String| !hors_sujet.contains(l) && !l.ends_with(&format!("({HORS_SUJET})"));
    let restants: Vec<String> = enquete
        .and_then(|e| e.verdict.as_ref())
        .filter(|v| v.genre == TypeVerdict::Reparateur)
        .map(|v| v.restants.iter().map(|id| titre_suspect(cat, id)).collect())
        .unwrap_or_default();

    let mut gauche = Vec::new();
    let surs = lu.surs.iter().map(|x| json!({ "puce": "coche", "texte": x })).collect();
    gauche.extend(rangs(titres::SUR, "liste", surs, None));
    let ecartes = lu
        .ecartes
        .iter()
        .filter(a_montrer)
        .map(|l| match ecartes_connus.iter().find(|(ligne, _, _)| ligne == l) {
            Some((_, nom, raison)) => json!({ "puce": "croix", "gras": nom, "texte": raison }),
            None => json!({ "puce": "croix", "texte": l }),
        })
        .collect();
    gauche.extend(rangs(titres::ECARTE, "liste", ecartes, None));
    if let Some(r) = &lu.reste {
        let rs: Vec<Value> = if !restants.is_empty() && restants.join(", ") == *r {
            restants.iter().map(|n| json!({ "puce": "point", "gras": n })).collect()
        } else {
            vec![json!({ "puce": "point", "texte": r })]
        };
        gauche.extend(rangs(titres::RESTE, "liste", rs, None));
    }
    let qr = lu.reponses.iter().map(|(q, r)| json!({ "texte": q, "valeur": r })).collect();
    gauche.extend(rangs(titres::QUESTIONS, "qr", qr, Some(titres::QUESTIONS_RIEN)));
    for (titre, l) in &lu.autres {
        let rs = l.iter().map(|x| json!({ "texte": x })).collect();
        gauche.extend(rangs(titre.as_deref().unwrap_or_default(), "liste", rs, None));
    }

    let mut droite = Vec::new();
    let pc = lu.pc.iter().map(|(c, v)| json!({ "cle": c, "valeur": v })).collect();
    droite.extend(rangs(titres::PC, "kv", pc, Some(titres::PC_RIEN)));
    let vu = |texte: &str| enquete.and_then(|e| e.auto.iter().find(|a| a.texte == texte)).map(|a| a.vu);
    let mut windows: Vec<Value> = lu
        .windows
        .iter()
        .map(|l| {
            let puce = match vu(l) {
                Some(true) => "alerte",
                Some(false) => "ok",
                None => "point",
            };
            json!({ "puce": puce, "texte": l })
        })
        .collect();
    windows.extend(lu.a_savoir.iter().map(|l| json!({ "puce": "alerte", "texte": l })));
    droite.extend(rangs(titres::WINDOWS, "liste", windows, None));

    blocs.push(json!({ "type": "colonnes", "elements": [{ "blocs": gauche }, { "blocs": droite }] }));
    for l in &lu.fin {
        blocs.push(json!({ "type": "petit", "texte": l }));
    }
    blocs
}
