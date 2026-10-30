//! Le garde-fou de l'assistant (Select-PreRempli) : les cas de outils\tester.ps1, puis plus d'une centaine d'autres
//! (réponses hors de la liste, Id en majuscules, étapes inconnues, phrases vides, majuscules, sans accents).

mod commun;

use commun::{catalogue, lire_juge, verdict_final};
use mindcare_moteur::pre_rempli;
use serde_json::Value;

fn paires(v: &Value) -> Vec<(String, i64)> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|x| (x[0].as_str().unwrap_or_default().to_string(), x[1].as_i64().unwrap_or_default()))
                .collect()
        })
        .unwrap_or_default()
}

/// Rangées par Id (sans tenir compte des majuscules) : PowerShell rend une table, sans ordre.
fn rangees(mut l: Vec<(String, i64)>) -> Vec<(String, i64)> {
    l.sort_by_key(|(k, _)| k.to_lowercase());
    l
}

#[test]
fn garde_fou_de_l_assistant() {
    let juge = lire_juge("garde-fou.json");
    let cat = catalogue();
    let cas = juge["Cas"].as_array().expect("Cas");
    let mut ecarts = Vec::new();
    let mut de_tester = 0;
    for c in cas {
        let id = c["Panne"].as_str().unwrap_or_default();
        let panne = if id.is_empty() { None } else { cat.panne(id) };
        let phrase = c["Phrase"].as_str().unwrap_or_default();
        let obtenu = rangees(pre_rempli(cat, panne, &paires(&c["Reponses"]), phrase));
        let attendu = rangees(paires(&c["Garde"]));
        if obtenu != attendu {
            ecarts.push(format!("{id} « {phrase} » {} : PowerShell {attendu:?} / Rust {obtenu:?}", c["Reponses"]));
        }
        // La réponse attendue par tester.ps1 : les Id gardés, rangés, séparés par des virgules
        if let Some(a) = c["Attendu"].as_str() {
            de_tester += 1;
            let ids: Vec<String> = obtenu.iter().map(|(k, _)| k.clone()).collect();
            if ids.join(",") != a {
                ecarts.push(format!("« {phrase} » : {} au lieu de {a} (tester.ps1)", ids.join(",")));
            }
        }
    }
    assert_eq!(de_tester, 4, "les 4 cas de tester.ps1");
    assert!(cas.len() >= 104, "trop peu de cas ({})", cas.len());
    verdict_final("garde-fou", cas.len(), &ecarts);
}
