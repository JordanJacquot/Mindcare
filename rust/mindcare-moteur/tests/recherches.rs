//! La recherche par mots : les recherches de outils\tester.ps1 (la bonne panne en premier), puis des milliers d'autres
//! (les noms, phrases et mots du catalogue, des fautes, des accents, des majuscules, des petits mots, des caractères
//! rares), avec le classement complet de PowerShell.
//!
//! Depuis le 05/11/2026, la recherche classe autrement que PowerShell (écart voulu, tests\divergences.rs) : elle doit
//! trouver exactement les mêmes pannes, et l'ancien ordre ([`chercher_comme_powershell`]) reste celui de PowerShell.

mod commun;

use commun::{catalogue, lire_juge, verdict_final};
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{Objet, chercher, chercher_comme_powershell, simple};
use serde_json::Value;

fn noms(pannes: Vec<&Objet>) -> Vec<String> {
    pannes.into_iter().map(|p| texte(champ(p, "Id"))).collect()
}

fn classement(q: &str) -> Vec<String> {
    noms(chercher(catalogue(), q))
}

fn classement_powershell(q: &str) -> Vec<String> {
    noms(chercher_comme_powershell(catalogue(), q))
}

fn ids(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn tries(mut l: Vec<String>) -> Vec<String> {
    l.sort();
    l
}

#[test]
fn recherches_de_tester() {
    let juge = lire_juge("recherches.json");
    let cas = juge["Tester"].as_array().expect("Tester");
    let mut ecarts = Vec::new();
    for c in cas {
        let q = c["Q"].as_str().unwrap_or_default();
        let ancien = classement_powershell(q);
        if ancien != ids(&c["P"]) {
            ecarts.push(format!("« {q} » : PowerShell {:?} / Rust (ancien ordre) {ancien:?}", ids(&c["P"])));
        }
        let obtenu = classement(q);
        if tries(obtenu.clone()) != tries(ids(&c["P"])) {
            ecarts.push(format!("« {q} » : pas les mêmes pannes que PowerShell {:?} / Rust {obtenu:?}", ids(&c["P"])));
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
        let ancien = classement_powershell(q);
        if ancien != ids(&c["P"]) {
            ecarts.push(format!(
                "« {q} » (simple : « {} ») : PowerShell {:?} / Rust (ancien ordre) {ancien:?}",
                simple(q),
                ids(&c["P"])
            ));
        }
        let obtenu = classement(q);
        if tries(obtenu.clone()) != tries(ancien.clone()) {
            ecarts.push(format!("« {q} » : pas les mêmes pannes, ancien ordre {ancien:?} / nouveau {obtenu:?}"));
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
