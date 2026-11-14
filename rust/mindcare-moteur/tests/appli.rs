//! Le garde-fou de « Ouvrir Nevermind » : dans le vrai catalogue, exactement les 6 pannes choisies avec Jordan
//! proposent Nevermind ; la validation refuse un champ mal écrit ou mal placé ; PowerShell ne voit pas le champ.

mod commun;

use commun::catalogue;
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{Appli, Catalogue, verifier, verifier_appli};
use serde_json::json;

/// Les pannes choisies (les jeux qui saccadent ou laguent, l'écran à 60 Hz, le processeur bridé).
const ATTENDUES: [&str; 6] = ["saccades", "saccadesdebut", "inputlag", "laggame", "frequence", "processeurbride"];

#[test]
fn exactement_les_six_pannes_choisies() {
    let cat = catalogue();
    assert_eq!(verifier_appli(cat), Vec::<String>::new());
    let mut avec: Vec<String> = cat
        .pannes()
        .iter()
        .filter(|p| champ(p, "Appli").is_some())
        .map(|p| {
            assert_eq!(Appli::de_la_panne(p), Some(Appli::Nevermind), "{}", texte(champ(p, "Id")));
            texte(champ(p, "Id"))
        })
        .collect();
    avec.sort();
    let mut attendues: Vec<String> = ATTENDUES.iter().map(|s| s.to_string()).collect();
    attendues.sort();
    assert_eq!(avec, attendues);
    // (pas les ventilateurs ni la mémoire vidéo : décidé avec Jordan)
    for id in ["ventilosfond", "memoirevideojeu", "jeu"] {
        let p = cat.panne(id).unwrap_or_else(|| panic!("{id} absente du catalogue"));
        assert_eq!(Appli::de_la_panne(p), None, "{id}");
    }
}

/// Un petit catalogue : une panne (avec ce champ en plus), son étape, un test partagé et un suspect.
fn petit(panne: serde_json::Value, etape: serde_json::Value, test: serde_json::Value) -> Catalogue {
    let mut p = json!({ "Id": "p", "Etapes": [etape] });
    for (k, v) in panne.as_object().unwrap() {
        p[k] = v.clone();
    }
    let j = json!({
        "Familles": [{ "Id": "jeux" }],
        "Suspects": { "s": { "Titre": "S" } },
        "Tests": { "test-x": test },
        "Pannes": [p],
    });
    Catalogue::lire(&j.to_string()).unwrap()
}

#[test]
fn la_validation_refuse_un_champ_mal_ecrit_ou_mal_place() {
    let test = json!({ "Type": "test" });
    let etape = json!({ "Ref": "test-x" });
    let cat = petit(json!({ "Appli": "nevermind" }), etape.clone(), test.clone());
    assert_eq!(verifier_appli(&cat), Vec::<String>::new());
    for (valeur, montre) in [
        (json!("Nevermind"), "Nevermind"),
        (json!("mindset"), "mindset"),
        (json!("C:\\Nevermind.exe"), "C:\\Nevermind.exe"),
        (json!(""), ""),
    ] {
        let cat = petit(json!({ "Appli": valeur }), etape.clone(), test.clone());
        assert_eq!(verifier_appli(&cat), [format!("Panne p : appli inconnue « {montre} »")]);
    }
    let cat = petit(json!({ "Appli": ["nevermind"] }), etape.clone(), test.clone());
    assert!(verifier_appli(&cat).iter().any(|m| m.starts_with("Panne p : appli inconnue")));
    let cat = petit(json!({}), json!({ "Ref": "test-x", "Appli": "nevermind" }), test.clone());
    assert_eq!(verifier_appli(&cat), ["Panne p, étape test-x : Appli n'est lu que sur une panne"]);
    let cat = petit(json!({}), etape.clone(), json!({ "Type": "test", "Appli": "nevermind" }));
    assert_eq!(verifier_appli(&cat), ["Test test-x : Appli n'est lu que sur une panne"]);
    let j = json!({
        "Familles": [{ "Id": "jeux", "Appli": "nevermind" }],
        "Suspects": { "s": { "Titre": "S", "Appli": "nevermind" } },
        "Tests": {},
        "Pannes": [],
    });
    let cat = Catalogue::lire(&j.to_string()).unwrap();
    assert_eq!(
        verifier_appli(&cat),
        ["Suspect s : Appli n'est lu que sur une panne", "Famille jeux : Appli n'est lu que sur une panne"]
    );
}

#[test]
fn powershell_ne_voit_pas_le_champ() {
    // La validation de PowerShell (reprise exactement par `verifier`) ne dit rien du champ : le vrai catalogue reste
    // sans problème, avec ou sans lui, avec les mêmes pannes.
    let texte_fichier =
        String::from_utf8(std::fs::read(commun::chemin_catalogue()).expect("enquetes.json")).expect("UTF-8");
    let texte_fichier = texte_fichier.strip_prefix('\u{feff}').unwrap_or(&texte_fichier);
    assert_eq!(verifier(catalogue(), texte_fichier), Vec::<String>::new());
    let sans: String =
        texte_fichier.lines().filter(|l| !l.trim_start().starts_with("\"Appli\":")).collect::<Vec<_>>().join("\n");
    assert_eq!(texte_fichier.lines().count() - sans.lines().count(), ATTENDUES.len(), "une ligne par panne");
    let cat_sans = Catalogue::lire(&sans).expect("catalogue sans le champ");
    assert_eq!(verifier(&cat_sans, &sans), Vec::<String>::new());
    assert_eq!(cat_sans.pannes().len(), catalogue().pannes().len());
    assert!(cat_sans.pannes().iter().all(|p| Appli::de_la_panne(p).is_none()));
}
