//! Get-IndicesWindows : les mêmes données brutes, données à un faux Windows, rendent exactement les indices du vrai
//! journaux.ps1 (tests\juges\windows-cas.json : les cas nommés ; windows-hasard.json : des cas tirés au hasard, graine
//! écrite dans le juge).

mod commun;

use commun::{FauxWindows, ecart, indices_juge, lire_juge, verdict_final};
use mindcare_windows::{JOURS, indices_windows};
use serde_json::Value;

fn rejouer(juge: &Value) -> (usize, Vec<String>) {
    let cas = juge["Cas"].as_array().expect("Cas");
    let mut ecarts = Vec::new();
    for c in cas {
        let faux = FauxWindows::depuis(&c["Donnees"], &[]);
        let obtenu = indices_windows(&faux, faux.maintenant, JOURS);
        if let Some(d) = ecart(&indices_juge(&c["Indices"]), &obtenu) {
            ecarts.push(format!("« {} » :\n    {d}", c["Nom"].as_str().unwrap_or_default()));
        }
    }
    (cas.len(), ecarts)
}

#[test]
fn cas_nommes() {
    let juge = lire_juge("windows-cas.json");
    let (n, ecarts) = rejouer(&juge);
    assert!(n >= 60, "trop peu de cas nommés ({n})");
    verdict_final("cas nommés", n, &ecarts);
}

#[test]
fn cas_au_hasard() {
    let juge = lire_juge("windows-hasard.json");
    assert!(juge["Graine"].is_u64(), "la graine doit être écrite dans le juge");
    let (n, ecarts) = rejouer(&juge);
    assert!(n >= 500, "trop peu de cas au hasard ({n})");
    verdict_final("cas au hasard", n, &ecarts);
}

/// Les juges couvrent bien chaque indice, vu et pas vu (sinon un cas manque à l'exporteur).
#[test]
fn chaque_indice_vu_et_pas_vu() {
    let mut vus = std::collections::BTreeMap::<String, (usize, usize)>::new();
    for nom in ["windows-cas.json", "windows-hasard.json"] {
        let juge = lire_juge(nom);
        for c in juge["Cas"].as_array().expect("Cas") {
            for (k, i) in indices_juge(&c["Indices"]) {
                let e = vus.entry(k).or_default();
                if i.vu { e.0 += 1 } else { e.1 += 1 }
            }
        }
    }
    for k in mindcare_moteur::INDICES_CONNUS {
        let (oui, non) = vus.get(k).copied().unwrap_or_default();
        assert!(oui >= 3 && non >= 3, "{k} : {oui} fois vu, {non} fois pas vu dans les juges");
    }
}
