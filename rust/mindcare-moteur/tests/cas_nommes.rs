//! Les enquêtes dont on connaît la bonne réponse (les cas nommés de outils\tester.ps1 : le PC de Jordan, celui de son
//! cousin...), l'indice de Windows ignoré quand Mindcare est ouvert sur un autre appareil, et l'avancement
//! « étape N sur M ». Chaque résultat doit être exactement celui de PowerShell, et la réponse celle que tester.ps1 attend.

mod commun;

use commun::{catalogue, choix, ecart, indices, lire_juge, panne, resultat, verdict_final};
use mindcare_moteur::enquete;
use serde_json::Value;

#[test]
fn cas_nommes_de_tester() {
    let juge = lire_juge("cas-nommes.json");
    let cat = catalogue();
    let cas = juge["Cas"].as_array().expect("Cas");
    let mut ecarts = Vec::new();
    for c in cas {
        let nom = c["Nom"].as_str().unwrap_or_default();
        let ind = indices(&c["Indices"]);
        let e = enquete(cat, panne(c["Panne"].as_str().unwrap_or_default()), &choix(&c["Entree"]), ind.as_ref());
        let obtenu = resultat(cat, &e);
        if let Some(d) = ecart(&c["R"], &obtenu) {
            ecarts.push(format!("« {nom} » :\n    {d}"));
        }
        // La réponse attendue par tester.ps1
        let attendu = &c["Attendu"];
        let bon = e.verdict.as_ref().is_some_and(|v| {
            let suspect = attendu["Suspect"].as_str().unwrap_or_default();
            let mut restants: Vec<String> = choix_textes(&attendu["Restants"]);
            restants.sort();
            let mut vrais = v.restants.clone();
            vrais.sort();
            v.genre.nom() == attendu["Type"].as_str().unwrap_or_default()
                && (suspect.is_empty() || v.suspect.as_deref() == Some(suspect))
                && (restants.is_empty() || restants == vrais)
        });
        if !bon {
            ecarts.push(format!("« {nom} » : pas la réponse attendue par tester.ps1 ({:?})", e.verdict));
        }
    }
    assert!(cas.len() >= 44, "il manque des cas nommés ({})", cas.len());
    verdict_final("cas nommés", cas.len(), &ecarts);
}

fn choix_textes(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

#[test]
fn indices_ignores_sur_un_autre_appareil() {
    let juge = lire_juge("cas-nommes.json");
    let cat = catalogue();
    let c = &juge["Ailleurs"];
    let ind = indices(&c["Indices"]);
    let e = enquete(cat, panne("redemarre"), &choix(&c["Entree"]), ind.as_ref());
    assert_eq!(ecart(&c["R"], &resultat(cat, &e)), None);
    assert!(e.etape.is_some() && e.auto.is_empty(), "sans « cepc », les indices de Windows ne servent pas");
}

#[test]
fn avancement_etape_n_sur_m() {
    let juge = lire_juge("cas-nommes.json");
    let cat = catalogue();
    let a = &juge["Avancement"];
    let mut morceaux = Vec::new();
    for c in a["Cas"].as_array().expect("Cas") {
        let e = enquete(cat, panne("songresille"), &choix(&c["Entree"]), None);
        assert_eq!(ecart(&c["R"], &resultat(cat, &e)), None);
        morceaux.push(format!("{}/{}", e.faits.len() + 1, e.faits.len() + 1 + e.a_venir));
    }
    assert_eq!(morceaux.join(" "), a["Attendu"].as_str().unwrap_or_default());
}
