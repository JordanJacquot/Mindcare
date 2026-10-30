//! « Chaque suspect peut être identifié » : pour chaque panne, les suspects trouvés en essayant les suites de réponses
//! des plus courtes aux plus longues (sans indices, puis avec tous les indices de Windows vus s'il en manque) sont ceux
//! de PowerShell, et aucun suspect n'est jamais introuvable.

mod commun;

use commun::{catalogue, lire_juge, panne, verdict_final};
use mindcare_moteur::identifiables::{introuvables, suspects_trouvables, tous_vus};
use serde_json::Value;

fn rangee(mut l: Vec<String>) -> Vec<String> {
    l.sort_by_key(|s| s.to_lowercase());
    l
}

fn textes(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

#[test]
fn suspects_identifiables() {
    let juge = lire_juge("identifiables.json");
    let cat = catalogue();
    let vus = tous_vus();
    let cas = juge["Pannes"].as_array().expect("Pannes");
    let mut ecarts = Vec::new();
    for c in cas {
        let id = c["Panne"].as_str().unwrap_or_default();
        let p = panne(id);
        let sans = rangee(suspects_trouvables(cat, p, None));
        if sans != rangee(textes(&c["SansIndices"])) {
            ecarts.push(format!("{id}, sans indices : PowerShell {:?} / Rust {sans:?}", textes(&c["SansIndices"])));
        }
        if !c["TousVus"].is_null() {
            let avec = rangee(suspects_trouvables(cat, p, Some(&vus)));
            if avec != rangee(textes(&c["TousVus"])) {
                ecarts.push(format!("{id}, tous vus : PowerShell {:?} / Rust {avec:?}", textes(&c["TousVus"])));
            }
        }
    }
    verdict_final("suspects identifiables", cas.len(), &ecarts);
}

#[test]
fn aucun_suspect_introuvable() {
    let juge = lire_juge("identifiables.json");
    let attendu: Vec<String> = juge["Pannes"]
        .as_array()
        .expect("Pannes")
        .iter()
        .flat_map(|c| {
            textes(&c["Manque"]).into_iter().map(move |m| format!("{} : {m}", c["Panne"].as_str().unwrap_or_default()))
        })
        .collect();
    assert_eq!(introuvables(catalogue()), attendu);
    assert!(attendu.is_empty(), "jamais identifiés : {attendu:?}");
}
