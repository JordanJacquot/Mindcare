//! La validation du catalogue (Test-Enquetes) : le vrai catalogue n'a aucun problème, et un petit catalogue abîmé de
//! toutes les façons donne exactement les messages de PowerShell, dans le même ordre.

mod commun;

use commun::{catalogue, chemin_catalogue, lire_juge, verdict_final};
use mindcare_moteur::{Catalogue, verifier};
use serde_json::Value;

fn textes(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

#[test]
fn le_vrai_catalogue_est_bon() {
    let juge = lire_juge("validation.json");
    let texte = String::from_utf8(std::fs::read(chemin_catalogue()).expect("enquetes.json")).expect("UTF-8");
    let texte = texte.strip_prefix('\u{feff}').unwrap_or(&texte);
    assert_eq!(verifier(catalogue(), texte), textes(&juge["Reel"]));
    assert!(textes(&juge["Reel"]).is_empty(), "enquetes.json a des problèmes");
    let avec_tiret = format!("{texte}\u{2014}");
    assert_eq!(verifier(catalogue(), &avec_tiret), textes(&juge["ReelTiret"]));
}

#[test]
fn catalogues_abimes() {
    let juge = lire_juge("validation.json");
    let cas = juge["Cas"].as_array().expect("Cas");
    let mut ecarts = Vec::new();
    for c in cas {
        let json = c["Json"].as_str().unwrap_or_default();
        let cat = Catalogue::lire(json).expect("catalogue abîmé lisible");
        let obtenu = verifier(&cat, json);
        let attendu = textes(&c["Problemes"]);
        if obtenu != attendu {
            ecarts.push(format!("« {} » :\n    PowerShell {attendu:?}\n    Rust       {obtenu:?}", c["Nom"]));
        }
    }
    let avec_problemes = cas.iter().filter(|c| !textes(&c["Problemes"]).is_empty()).count();
    assert!(avec_problemes >= 30, "trop peu de catalogues abîmés ({avec_problemes})");
    verdict_final("catalogues abîmés", cas.len(), &ecarts);
}
