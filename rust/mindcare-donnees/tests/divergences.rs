//! La divergence voulue avec Mindcare PowerShell (tests\juges\divergences.json) : constat 23 de l'audit, un fichier
//! vide n'est jamais rangé en copie de secours.
//!
//! PowerShell copiait un fichier vide (ou abîmé, ou qui n'est pas un objet JSON) à côté, mais le laissait en place :
//! l'enregistrement suivant le poussait dans « .bak » et la bonne copie était perdue. Le Rust passe par `json_sur` de
//! suite-commun (le fichier est déplacé de côté, sa copie remise en place) et retire un fichier vidé entre la lecture
//! et l'écriture. Chaque cas garde les deux réponses : celle de PowerShell, et la réponse voulue (jouée par
//! l'exporteur avec la lecture et l'écriture du Rust), que le Rust doit donner.

mod commun;

use commun::{base64, ecart, jouer_tous, lire_juge, s, verdict_final};
use mindcare_donnees::fichiers::{analyser, decoder};

#[test]
fn constat_23_fichier_vide_jamais_en_copie() {
    let juge = lire_juge("divergences.json");
    assert_eq!(juge["Constat"], 23);
    let cas = juge["Scenarios"].as_array().cloned().unwrap_or_default();
    let mut ecarts = jouer_tous(&cas, "Voulu");
    for c in &cas {
        let nom = s(&c["Nom"]);
        // (une vraie divergence : les deux réponses diffèrent)
        if ecart(&c["PowerShell"], &c["Voulu"]).is_none() {
            ecarts.push(format!("« {nom} » : PowerShell et la réponse voulue sont pareilles"));
        }
        // (et elle vient d'un fichier vide, abîmé ou qui n'est pas un objet : au départ, ou vidé en route)
        let mauvais_depart = c["Fichiers"]
            .as_object()
            .into_iter()
            .flatten()
            .any(|(n, b)| n.contains(".json") && analyser(&decoder(&base64(s(b)))).is_err());
        let vide_en_route = c["Ops"].as_array().into_iter().flatten().any(|o| s(&o["Op"]) == "Ecrire");
        if !mauvais_depart && !vide_en_route {
            ecarts.push(format!("« {nom} » : diverge sans fichier vide ni abîmé"));
        }
    }
    verdict_final("divergences (constat 23)", cas.len(), &ecarts);
}

/// La règle elle-même, sans juge : un fichier vide en place, une bonne copie ; après lecture et enregistrement, la
/// copie est toujours la bonne.
#[test]
fn copie_gardee_apres_un_fichier_vide() {
    use mindcare_donnees::{Fichiers, Journal, Moment, horloge_figee};
    use serde_json::json;
    let d = commun::DossierEssai::nouveau("constat23");
    let f = d.0.join("diagnostics.json");
    let bak = d.0.join("diagnostics.json.bak");
    std::fs::write(&f, "").unwrap();
    std::fs::write(&bak, r#"{"Version":2,"Diagnostics":[{"Id":"a","Panne":"redemarre"}]}"#).unwrap();
    let journal =
        Journal::ouvrir(&d.0.join("journal.txt"), horloge_figee(Moment::lire("2026-10-30T14:05:00").unwrap()));
    let mut fichiers = Fichiers::nouveau(journal);
    let lu = fichiers.lire(&f).expect("la copie est lue");
    assert_eq!(lu["Diagnostics"][0]["Id"], "a");
    fichiers.sauver(&f, &json!({ "Version": 2, "Diagnostics": [] })).unwrap();
    let copie: serde_json::Value = serde_json::from_slice(&std::fs::read(&bak).unwrap()).unwrap();
    assert_eq!(copie["Diagnostics"][0]["Id"], "a", "la bonne copie ne doit jamais être remplacée par le fichier vide");
    // Vidé entre la lecture et l'écriture : retiré avant d'écrire, la copie reste la même
    std::fs::write(&f, "  ").unwrap();
    fichiers.sauver(&f, &json!({ "Version": 2, "Diagnostics": [] })).unwrap();
    let copie: serde_json::Value = serde_json::from_slice(&std::fs::read(&bak).unwrap()).unwrap();
    assert_eq!(copie["Diagnostics"][0]["Id"], "a");
}
