//! Get-Barrettes : les mêmes barrettes (Win32_PhysicalMemory) s'écrivent exactement pareil (tests\juges\barrettes.json).
//! WMI illisible : aucune barrette, comme PowerShell.

mod commun;

use commun::{FauxWindows, lire_juge, verdict_final};
use mindcare_windows::barrettes;
use serde_json::{Value, json};

#[test]
fn barrettes_comme_powershell() {
    let juge = lire_juge("barrettes.json");
    let cas = juge["Cas"].as_array().expect("Cas");
    let mut ecarts = Vec::new();
    for c in cas {
        let liste: Vec<Value> = c["Barrettes"].as_array().cloned().unwrap_or_default();
        let mut donnees = json!({ "Maintenant": "2026-10-31T00:00:00", "Sonde": {}, "Erreurs": {}, "Demarrage": "2026-10-30T00:00:00" });
        if let Some(e) = c["Erreur"].as_str().filter(|e| !e.is_empty()) {
            donnees["Erreurs"]["Barrettes"] = json!(e);
        }
        let faux = FauxWindows::depuis(&donnees, &liste);
        let obtenu = barrettes(&faux).unwrap_or_default();
        let attendu: Vec<String> = c["Textes"]
            .as_array()
            .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        if obtenu != attendu {
            ecarts.push(format!("« {} » :\n    PowerShell {attendu:?}\n    Rust       {obtenu:?}", c["Nom"]));
        }
    }
    verdict_final("barrettes", cas.len(), &ecarts);
}
