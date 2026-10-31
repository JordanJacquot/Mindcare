//! Les scénarios joués par les vrais modules de Mindcare PowerShell (tests\juges\scenarios-*.json) : historiques v2 et
//! anciens, fichiers abîmés, copies .bak, fichiers pris par un autre programme, reprises d'enquêtes, réglages
//! fusionnés, cartes de Mindset, rapports. Chacun est rejoué dans un dossier d'essai neuf et doit donner exactement
//! les mêmes résultats, le même historique, les mêmes réglages, les mêmes fichiers et le même journal.

mod commun;

use commun::{jouer_tous, juges_commencant_par, lire_juge, verdict_final};
use serde_json::Value;

#[test]
fn scenarios_comme_powershell() {
    let noms = juges_commencant_par("scenarios-");
    assert!(!noms.is_empty(), "aucun juge scenarios-*.json : lance outils\\exporter-juges-donnees.ps1");
    let mut tous: Vec<Value> = Vec::new();
    for nom in &noms {
        let juge = lire_juge(nom);
        tous.extend(juge["Scenarios"].as_array().cloned().unwrap_or_default());
    }
    let ecarts = jouer_tous(&tous, "PowerShell");
    verdict_final("scénarios", tous.len(), &ecarts);
}
