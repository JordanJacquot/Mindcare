//! Le garde-fou de « Le faire pour moi » : dans le vrai catalogue, exactement les 10 tests choisis avec Jordan ont une
//! action, test-sfc et test-gaming-services n'en ont jamais, chaque action est connue du code ; la validation refuse
//! un champ mal écrit ou mal placé.

mod commun;

use commun::catalogue;
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{Catalogue, Reparation, verifier, verifier_reparer};
use serde_json::json;

/// Les tests choisis, avec leur action (6 sans droits administrateur, puis 4 qui les demandent).
const ATTENDUS: [(&str, Reparation); 10] = [
    ("test-dns", Reparation::Dns),
    ("test-wsreset", Reparation::CacheStore),
    ("test-cache-icones", Reparation::CacheIcones),
    ("test-vcredist", Reparation::VisualCpp),
    ("test-java", Reparation::Java),
    ("test-reset-parametres", Reparation::Parametres),
    ("test-profil-wifi", Reparation::ProfilWifi),
    ("test-ipconfig", Reparation::AdresseIp),
    ("test-alim-defaut", Reparation::Alimentation),
    ("test-depanneur-maj", Reparation::MisesAJour),
];

#[test]
fn exactement_les_dix_tests_choisis() {
    let cat = catalogue();
    assert_eq!(verifier_reparer(cat), Vec::<String>::new());
    let mut avec: Vec<(String, Reparation)> = Vec::new();
    for (nom, t) in cat.tests() {
        let o = t.as_object().expect("un test est un objet");
        if let Some(v) = champ(o, "Reparer") {
            let r = Reparation::depuis_id(v.as_str().unwrap_or_default()).unwrap_or_else(|| panic!("{nom} : {v}"));
            avec.push((nom.clone(), r));
        }
    }
    avec.sort_by(|a, b| a.0.cmp(&b.0));
    let mut attendus: Vec<(String, Reparation)> = ATTENDUS.iter().map(|(n, r)| (n.to_string(), *r)).collect();
    attendus.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(avec, attendus);
    // (chaque action du code sert une seule fois, et les droits administrateur sont ceux décidés)
    for r in Reparation::TOUTES {
        assert_eq!(ATTENDUS.iter().filter(|(_, x)| *x == r).count(), 1, "{r:?}");
    }
    let admin: Vec<&str> = ATTENDUS.iter().filter(|(_, r)| r.admin()).map(|(n, _)| *n).collect();
    assert_eq!(admin, ["test-profil-wifi", "test-ipconfig", "test-alim-defaut", "test-depanneur-maj"]);
}

#[test]
fn jamais_pour_sfc_ni_les_services_de_jeu() {
    let cat = catalogue();
    for nom in ["test-sfc", "test-gaming-services"] {
        let t = cat.test(nom).and_then(|t| t.as_object()).unwrap_or_else(|| panic!("{nom} absent du catalogue"));
        assert!(champ(t, "Reparer").is_none(), "{nom} a une action");
    }
    // Même écrit à la main dans une étape prête, test-sfc n'a pas d'action
    let mut sfc = cat.test("test-sfc").and_then(|t| t.as_object()).cloned().unwrap();
    sfc.insert("Id".into(), json!("test-sfc"));
    sfc.insert("Reparer".into(), json!("dns"));
    assert_eq!(Reparation::de_l_etape(&sfc), None);
}

#[test]
fn chaque_etape_prete_garde_son_action() {
    let cat = catalogue();
    let mut vus = Vec::new();
    for p in cat.pannes() {
        for e in cat.etapes(p) {
            let id = texte(champ(&e, "Id"));
            match ATTENDUS.iter().find(|(n, _)| *n == id) {
                Some((_, r)) => {
                    assert_eq!(Reparation::de_l_etape(&e), Some(*r), "{id} dans {}", texte(champ(p, "Id")));
                    vus.push(id);
                }
                None => assert_eq!(Reparation::de_l_etape(&e), None, "{id} dans {}", texte(champ(p, "Id"))),
            }
        }
    }
    for (n, _) in ATTENDUS {
        assert!(vus.iter().any(|v| v == n), "{n} n'est utilisé par aucune panne");
    }
}

/// Un petit catalogue : un test partagé, et une panne qui le cite.
fn petit(test: serde_json::Value, propre: serde_json::Value) -> Catalogue {
    let mut etape = json!({ "Ref": "test-x" });
    for (k, v) in propre.as_object().unwrap() {
        etape[k] = v.clone();
    }
    let j = json!({
        "Familles": [],
        "Suspects": {},
        "Tests": { "test-x": test, "test-sfc": { "Type": "test", "Reparer": "dns", "Commande": { "Lignes": ["sfc"] } } },
        "Pannes": [{ "Id": "p", "Etapes": [etape] }],
    });
    Catalogue::lire(&j.to_string()).unwrap()
}

#[test]
fn la_validation_refuse_un_champ_mal_ecrit() {
    let bon = json!({ "Type": "test", "Reparer": "java", "Commande": { "Lignes": ["x"] } });
    let cat = petit(bon.clone(), json!({}));
    assert_eq!(verifier_reparer(&cat), ["Test test-sfc : jamais d'action pour ce test"]);
    let cat =
        petit(json!({ "Type": "test", "Reparer": "formater-le-disque", "Commande": { "Lignes": ["x"] } }), json!({}));
    assert!(verifier_reparer(&cat).contains(&"Test test-x : action inconnue « formater-le-disque »".to_string()));
    let cat = petit(json!({ "Type": "test", "Reparer": "Java", "Commande": { "Lignes": ["x"] } }), json!({}));
    assert!(verifier_reparer(&cat).contains(&"Test test-x : action inconnue « Java »".to_string()));
    let cat = petit(json!({ "Type": "test", "Reparer": ["dns"], "Commande": { "Lignes": ["x"] } }), json!({}));
    assert!(verifier_reparer(&cat).iter().any(|m| m.starts_with("Test test-x : action inconnue")));
    let cat = petit(json!({ "Type": "question", "Reparer": "dns", "Commande": { "Lignes": ["x"] } }), json!({}));
    assert!(verifier_reparer(&cat).contains(&"Test test-x : une action seulement sur un test".to_string()));
    let cat = petit(json!({ "Type": "test", "Reparer": "dns" }), json!({}));
    assert!(verifier_reparer(&cat).contains(&"Test test-x : une action sans commande à montrer".to_string()));
    let cat = petit(bon, json!({ "Reparer": "dns" }));
    assert!(
        verifier_reparer(&cat)
            .contains(&"Panne p, étape test-x : Reparer n'est lu que sur un test partagé".to_string())
    );
}

#[test]
fn powershell_ne_voit_pas_le_champ() {
    // La validation de PowerShell (reprise exactement par `verifier`) ne dit rien du champ : le vrai catalogue reste
    // sans problème, avec ou sans lui.
    let texte_fichier =
        String::from_utf8(std::fs::read(commun::chemin_catalogue()).expect("enquetes.json")).expect("UTF-8");
    let texte_fichier = texte_fichier.strip_prefix('\u{feff}').unwrap_or(&texte_fichier);
    assert_eq!(verifier(catalogue(), texte_fichier), Vec::<String>::new());
    let sans: String =
        texte_fichier.lines().filter(|l| !l.trim_start().starts_with("\"Reparer\":")).collect::<Vec<_>>().join("\n");
    let cat_sans = Catalogue::lire(&sans).expect("catalogue sans le champ");
    assert_eq!(verifier(&cat_sans, &sans), Vec::<String>::new());
    assert_eq!(cat_sans.tests().len(), catalogue().tests().len());
}
