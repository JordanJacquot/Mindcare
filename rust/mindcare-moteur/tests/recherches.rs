//! La recherche par mots : les recherches de outils\tester.ps1 (la bonne panne en premier), puis des milliers d'autres
//! (les noms, phrases et mots du catalogue, des fautes, des accents, des majuscules, des petits mots, des caractères
//! rares), avec le classement complet de PowerShell.

mod commun;

use commun::{catalogue, lire_juge, verdict_final};
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{chercher, simple};
use serde_json::Value;

fn classement(q: &str) -> Vec<String> {
    chercher(catalogue(), q).into_iter().map(|p| texte(champ(p, "Id"))).collect()
}

fn ids(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

#[test]
fn recherches_de_tester() {
    let juge = lire_juge("recherches.json");
    let cas = juge["Tester"].as_array().expect("Tester");
    let mut ecarts = Vec::new();
    for c in cas {
        let q = c["Q"].as_str().unwrap_or_default();
        let obtenu = classement(q);
        if obtenu != ids(&c["P"]) {
            ecarts.push(format!("« {q} » : PowerShell {:?} / Rust {obtenu:?}", ids(&c["P"])));
        }
        let bonnes: Vec<&str> = c["Attendu"].as_str().unwrap_or_default().split('|').collect();
        if !obtenu.first().is_some_and(|p| bonnes.contains(&p.as_str())) {
            ecarts.push(format!("« {q} » : {:?} en premier, tester.ps1 attend {bonnes:?}", obtenu.first()));
        }
    }
    assert!(cas.len() >= 22, "il manque des recherches de tester.ps1 ({})", cas.len());
    verdict_final("recherches de tester.ps1", cas.len(), &ecarts);
}

#[test]
fn milliers_de_recherches() {
    let juge = lire_juge("recherches.json");
    let cas = juge["Recherches"].as_array().expect("Recherches");
    let mut ecarts = Vec::new();
    for c in cas {
        let q = c["Q"].as_str().unwrap_or_default();
        let obtenu = classement(q);
        if obtenu != ids(&c["P"]) {
            ecarts.push(format!(
                "« {q} » (simple : « {} ») : PowerShell {:?} / Rust {obtenu:?}",
                simple(q),
                ids(&c["P"])
            ));
        }
    }
    assert!(cas.len() >= 2000, "trop peu de recherches ({})", cas.len());
    verdict_final("recherches", cas.len(), &ecarts);
}

#[test]
fn texte_simple() {
    assert_eq!(simple("  Le PC  ne   démarre PAS !"), "le pc ne demarre pas");
    assert_eq!(simple("e\u{301}cran"), "ecran");
    assert_eq!(simple("œil"), "il");
    assert_eq!(simple(""), "");
}
