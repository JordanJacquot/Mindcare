//! Ce que les tests partagent : le vrai catalogue, la lecture des juges exportés par outils\exporter-juges.ps1, et un
//! résultat d'enquête mis sous la même forme que dans les juges.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::OnceLock;

use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{Catalogue, Enquete, Indice, Indices, Objet, titre_verdict};
use serde_json::{Value, json};

/// Le dossier des juges (tests\juges).
pub fn dossier_juges() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("juges")
}

/// Le vrai catalogue du dépôt (Mindcare\fichiers\enquetes.json).
pub fn chemin_catalogue() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("Mindcare")
        .join("fichiers")
        .join("enquetes.json")
}

/// Le vrai catalogue, lu une seule fois.
pub fn catalogue() -> &'static Catalogue {
    static CAT: OnceLock<Catalogue> = OnceLock::new();
    CAT.get_or_init(|| Catalogue::lire_fichier(&chemin_catalogue()).expect("enquetes.json"))
}

/// L'empreinte du catalogue, comme l'export (FNV-1a sur 32 bits, sans les retours chariot).
pub fn empreinte(octets: &[u8]) -> String {
    let mut h: u32 = 2_166_136_261;
    for b in octets.iter().filter(|b| **b != 13) {
        h = (h ^ u32::from(*b)).wrapping_mul(16_777_619);
    }
    format!("{h:08x}")
}

/// Un juge, après avoir vérifié qu'il a été exporté avec ce catalogue-ci.
pub fn lire_juge(nom: &str) -> Value {
    let chemin = dossier_juges().join(nom);
    let texte_juge = std::fs::read_to_string(&chemin).unwrap_or_else(|e| panic!("{} : {e}", chemin.display()));
    let juge: Value = serde_json::from_str(&texte_juge).unwrap_or_else(|e| panic!("{nom} : {e}"));
    let octets = std::fs::read(chemin_catalogue()).expect("enquetes.json");
    let attendue = juge["Catalogue"]["Empreinte"].as_str().unwrap_or_default();
    assert_eq!(
        empreinte(&octets),
        attendue,
        "{nom} a été exporté avec un autre catalogue : relance outils\\exporter-juges.ps1 (le catalogue a changé)"
    );
    juge
}

/// Les juges dont le nom commence par ce début (parcours-000-049.json...), dans l'ordre des noms.
pub fn juges_commencant_par(debut: &str) -> Vec<String> {
    let mut noms: Vec<String> = std::fs::read_dir(dossier_juges())
        .expect("tests\\juges")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .filter(|n| n.starts_with(debut) && n.ends_with(".json"))
        .collect();
    noms.sort();
    noms
}

/// Les réponses d'un juge.
pub fn choix(v: &Value) -> Vec<i64> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default()
}

/// Les indices de Windows d'un juge (une liste de { Id, Vu, Texte, TexteOk, Partiel } ; rien : pas d'indices).
pub fn indices(v: &Value) -> Option<Indices> {
    if v.is_null() {
        return None;
    }
    let liste = v.as_array().unwrap_or_else(|| panic!("indices mal exportés (une liste attendue) : {v}"));
    Some(
        liste
            .iter()
            .map(|i| {
                let ind = Indice {
                    vu: i["Vu"].as_bool().unwrap_or(false),
                    texte: i["Texte"].as_str().unwrap_or_default().to_string(),
                    texte_ok: i["TexteOk"].as_str().unwrap_or_default().to_string(),
                    partiel: i["Partiel"].as_bool().unwrap_or(false),
                };
                (i["Id"].as_str().unwrap_or_default().to_string(), ind)
            })
            .collect(),
    )
}

/// Une panne du vrai catalogue.
pub fn panne(id: &str) -> &'static Objet {
    catalogue().panne(id).unwrap_or_else(|| panic!("panne {id} introuvable"))
}

/// Une enquête rejouée, sous la forme des juges.
pub fn resultat(cat: &Catalogue, e: &Enquete) -> Value {
    let id = |o: &Objet| texte(champ(o, "Id"));
    json!({
        "Etape": e.etape.as_ref().map(id).unwrap_or_default(),
        "Choix": e.choix,
        "Restants": e.restants,
        "Ecartes": e.ecartes.iter().map(|x| json!([x.id, x.raison])).collect::<Vec<_>>(),
        "Notes": e.notes,
        "Preuves": e.preuves,
        "Faits": e.faits.iter().map(|f| json!([id(&f.etape), f.court, f.reponse, f.texte])).collect::<Vec<_>>(),
        "Auto": e.auto.iter().map(|a| json!([a.texte, a.vu])).collect::<Vec<_>>(),
        "Verdict": e.verdict.as_ref().map(|v| json!([v.genre.nom(), v.suspect.clone().unwrap_or_default(), v.detail, v.restants])),
        "Titre": titre_verdict(cat, e.verdict.as_ref()),
        "AVenir": e.a_venir,
    })
}

/// Un texte d'un juge : tel quel, ou son numéro dans la table « Textes ».
fn t(v: &Value, textes: &[String]) -> Value {
    match v {
        Value::Number(n) => {
            let i = usize::try_from(n.as_u64().expect("numéro de texte")).expect("numéro de texte");
            Value::String(textes[i].clone())
        }
        autre => autre.clone(),
    }
}

/// Un résultat d'un juge, ses textes remis à la place de leurs numéros.
pub fn resoudre(r: &Value, textes: &[String]) -> Value {
    let liste = |nom: &str| r[nom].as_array().cloned().unwrap_or_default();
    let ecartes: Vec<Value> = liste("Ecartes").iter().map(|x| json!([x[0], t(&x[1], textes)])).collect();
    let preuves: Vec<Value> = liste("Preuves").iter().map(|x| t(x, textes)).collect();
    let faits: Vec<Value> =
        liste("Faits").iter().map(|x| json!([x[0], t(&x[1], textes), t(&x[2], textes), t(&x[3], textes)])).collect();
    let auto: Vec<Value> = liste("Auto").iter().map(|x| json!([t(&x[0], textes), x[1]])).collect();
    let verdict = match &r["Verdict"] {
        Value::Array(v) => json!([v[0], v[1], t(&v[2], textes), v[3]]),
        _ => Value::Null,
    };
    json!({
        "Etape": r["Etape"],
        "Choix": r["Choix"],
        "Restants": r["Restants"],
        "Ecartes": ecartes,
        "Notes": r["Notes"],
        "Preuves": preuves,
        "Faits": faits,
        "Auto": auto,
        "Verdict": verdict,
        "Titre": t(&r["Titre"], textes),
        "AVenir": r["AVenir"],
    })
}

/// Ce qui diffère entre deux résultats (champ par champ), ou rien.
pub fn ecart(attendu: &Value, obtenu: &Value) -> Option<String> {
    let champs =
        ["Etape", "Choix", "Restants", "Ecartes", "Notes", "Preuves", "Faits", "Auto", "Verdict", "Titre", "AVenir"];
    let diff: Vec<String> = champs
        .iter()
        .filter(|c| attendu[**c] != obtenu[**c])
        .map(|c| format!("{c} : PowerShell {} / Rust {}", attendu[*c], obtenu[*c]))
        .collect();
    if diff.is_empty() { None } else { Some(diff.join("\n    ")) }
}

/// Échoue en montrant les premiers écarts (et leur nombre).
pub fn verdict_final(quoi: &str, total: usize, ecarts: &[String]) {
    assert!(total > 0, "{quoi} : aucun cas dans le juge");
    if !ecarts.is_empty() {
        let premiers: Vec<&String> = ecarts.iter().take(15).collect();
        panic!(
            "{quoi} : {} écart(s) sur {total}\n{}",
            ecarts.len(),
            premiers.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
        );
    }
    println!("{quoi} : {total} cas, tous pareils");
}
