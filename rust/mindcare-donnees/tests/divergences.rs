//! Les divergences voulues avec Mindcare PowerShell (tests\juges\divergences.json) : constat 23 de l'audit, un fichier
//! vide n'est jamais rangé en copie de secours ; et une copie « .bak » abîmée est déplacée de côté.
//!
//! PowerShell copiait un fichier vide (ou abîmé, ou qui n'est pas un objet JSON) à côté, mais le laissait en place :
//! l'enregistrement suivant le poussait dans « .bak » et la bonne copie était perdue. Le Rust passe par `json_sur` de
//! suite-commun : le fichier est déplacé de côté, sa copie remise en place, et à l'écriture seul un fichier qui
//! contient un objet JSON va dans « .bak » (un fichier vidé entre la lecture et l'écriture est simplement remplacé).
//! Une copie « .bak » abîmée est elle aussi déplacée de côté par json_sur (PowerShell la laissait en place et en
//! refaisait une copie à chaque lecture). Chaque cas garde les deux réponses : celle de PowerShell, et la réponse
//! voulue (jouée par l'exporteur avec la lecture et l'écriture du Rust), que le Rust doit donner.

mod commun;

use commun::{base64, ecart, jouer_tous, lire_juge, s, verdict_final};
use suite_commun::json_sur::{analyser, texte_du_fichier};

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
            .any(|(n, b)| n.contains(".json") && analyser(&texte_du_fichier(&base64(s(b)))).is_err());
        let vide_en_route = c["Ops"].as_array().into_iter().flatten().any(|o| s(&o["Op"]) == "Ecrire");
        if !mauvais_depart && !vide_en_route {
            ecarts.push(format!("« {nom} » : diverge sans fichier vide ni abîmé"));
        }
    }
    verdict_final("divergences (constat 23, copies abîmées)", cas.len(), &ecarts);
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
    // Vidé entre la lecture et l'écriture : simplement remplacé, la copie reste la même
    std::fs::write(&f, "  ").unwrap();
    fichiers.sauver(&f, &json!({ "Version": 2, "Diagnostics": [] })).unwrap();
    let copie: serde_json::Value = serde_json::from_slice(&std::fs::read(&bak).unwrap()).unwrap();
    assert_eq!(copie["Diagnostics"][0]["Id"], "a");
    // Abîmé entre la lecture et l'écriture : copié de côté, jamais rangé dans « .bak »
    std::fs::write(&f, "{\"Version\":2,\"Diag").unwrap();
    fichiers.sauver(&f, &json!({ "Version": 2, "Diagnostics": [] })).unwrap();
    let copie: serde_json::Value = serde_json::from_slice(&std::fs::read(&bak).unwrap()).unwrap();
    assert_eq!(copie["Diagnostics"][0]["Id"], "a");
    // (avec le fichier vide mis de côté à la lecture, dans la même seconde : le second prend le nom suivant)
    assert_eq!(mis_de_cote(&d.0, "diagnostics.json.abime-"), vec![String::new(), "{\"Version\":2,\"Diag".to_string()]);
    assert!(d.0.join("diagnostics.json.abime-20261030-140500-2").is_file());
}

/// Les textes des fichiers mis de côté dont le nom commence par ce début.
fn mis_de_cote(dossier: &std::path::Path, debut: &str) -> Vec<String> {
    let mut l: Vec<String> = std::fs::read_dir(dossier)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(debut))
        .map(|e| String::from_utf8_lossy(&std::fs::read(e.path()).unwrap()).into_owned())
        .collect();
    l.sort();
    l
}

/// La règle elle-même, sans juge : une copie « .bak » abîmée (le fichier manque) est déplacée de côté, une seule fois,
/// avec les lignes de Mindcare PowerShell suivies de celles de json_sur.
#[test]
fn copie_abimee_deplacee_une_seule_fois() {
    use mindcare_donnees::{Fichiers, Journal, Moment, horloge_figee};
    let d = commun::DossierEssai::nouveau("copie-abimee");
    let f = d.0.join("diagnostics.json");
    let bak = d.0.join("diagnostics.json.bak");
    std::fs::write(&bak, "{\"Version\":2,\"Diag").unwrap();
    let journal =
        Journal::ouvrir(&d.0.join("journal.txt"), horloge_figee(Moment::lire("2026-10-30T14:05:00").unwrap()));
    let mut fichiers = Fichiers::nouveau(journal);
    assert_eq!(fichiers.lire(&f), None);
    assert!(!bak.exists(), "la copie abîmée doit être déplacée, pas copiée");
    assert_eq!(
        mis_de_cote(&d.0, "diagnostics.json.bak.abime-20261030-140500"),
        vec!["{\"Version\":2,\"Diag".to_string()]
    );
    // (relue : plus rien à mettre de côté)
    assert_eq!(fichiers.lire(&f), None);
    assert_eq!(mis_de_cote(&d.0, "diagnostics.json.bak.abime-").len(), 1);
    let texte = std::fs::read_to_string(d.0.join("journal.txt")).unwrap();
    let lignes: Vec<&str> = texte.lines().map(|l| l.split_once(' ').map_or(l, |(_, m)| m)).collect();
    let dossier = d.0.display().to_string();
    assert_eq!(lignes.len(), 4, "{texte}");
    assert_eq!(lignes[0], format!("Fichier repris de sa sauvegarde : {dossier}\\diagnostics.json.bak"));
    assert!(lignes[1].starts_with(&format!("Fichier illisible ({dossier}\\diagnostics.json.bak) : ")), "{texte}");
    assert_eq!(lignes[2], "diagnostics.json : sa copie (diagnostics.json.bak) est illisible aussi");
    assert_eq!(lignes[3], "diagnostics.json.bak mis de côté dans diagnostics.json.bak.abime-20261030-140500");
}
