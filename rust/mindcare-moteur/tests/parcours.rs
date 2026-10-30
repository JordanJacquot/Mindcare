//! Des milliers d'enquêtes au hasard sur les 300 pannes (au moins 40 par panne), jouées par le moteur PowerShell : des
//! « Je ne sais pas », des réponses en trop ou hors de la liste, des indices de Windows inventés (vus, pas vus, partiels,
//! absents). Pour chacune : le résultat complet à la fin, et l'étape suivante (avec « encore N étapes ») à chaque pas.

mod commun;

use commun::{
    catalogue, choix, ecart, indices, juges_commencant_par, lire_juge, panne, resoudre, resultat, verdict_final,
};
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{Indices, enquete};

/// Rejoue un fichier de parcours ; renvoie le nombre de cas, de pas et les écarts.
fn rejouer(nom: &str) -> (usize, usize, Vec<String>) {
    let juge = lire_juge(nom);
    let cat = catalogue();
    let textes: Vec<String> =
        juge["Textes"].as_array().expect("Textes").iter().map(|t| t.as_str().unwrap_or_default().to_string()).collect();
    let tables: Vec<Option<Indices>> = juge["Indices"].as_array().expect("Indices").iter().map(indices).collect();
    let cas = juge["Parcours"].as_array().expect("Parcours");
    let mut ecarts = Vec::new();
    let mut pas_total = 0;
    for c in cas {
        let p = panne(c["Panne"].as_str().unwrap_or_default());
        let ind = match c["Indices"].as_i64() {
            Some(i) if i >= 0 => tables[usize::try_from(i).expect("numéro")].as_ref(),
            _ => None,
        };
        let entree = choix(&c["Entree"]);
        let e = enquete(cat, p, &entree, ind);
        if let Some(d) = ecart(&resoudre(&c["R"], &textes), &resultat(cat, &e)) {
            ecarts.push(format!("{nom}, {} {:?} (indices {}) :\n    {d}", c["Panne"], entree, c["Indices"]));
        }
        // Chaque pas du parcours : l'étape posée et ce qui peut encore venir
        for (i, pas) in c["Pas"].as_array().map(Vec::as_slice).unwrap_or_default().iter().enumerate() {
            pas_total += 1;
            let e = enquete(cat, p, &entree[..i], ind);
            let etape = e.etape.as_ref().map(|x| texte(champ(x, "Id"))).unwrap_or_default();
            let attendu = (pas[0].as_str().unwrap_or_default(), pas[1].as_u64().unwrap_or_default());
            if (etape.as_str(), e.a_venir as u64) != attendu {
                ecarts.push(format!(
                    "{nom}, {} {:?}, pas {i} : PowerShell {attendu:?} / Rust ({etape}, {})",
                    c["Panne"],
                    &entree[..i],
                    e.a_venir
                ));
            }
        }
    }
    (cas.len(), pas_total, ecarts)
}

#[test]
fn parcours_au_hasard() {
    let noms = juges_commencant_par("parcours-");
    assert!(!noms.is_empty(), "aucun juge parcours-*.json");
    // (un fil par fichier : les fichiers sont indépendants)
    let resultats: Vec<(usize, usize, Vec<String>)> = std::thread::scope(|s| {
        let fils: Vec<_> = noms.iter().map(|n| s.spawn(move || rejouer(n))).collect();
        fils.into_iter().map(|f| f.join().expect("fil")).collect()
    });
    let total: usize = resultats.iter().map(|r| r.0).sum();
    let pas: usize = resultats.iter().map(|r| r.1).sum();
    let ecarts: Vec<String> = resultats.into_iter().flat_map(|r| r.2).collect();
    println!("{pas} pas vérifiés");
    verdict_final("parcours au hasard", total, &ecarts);
}

#[test]
fn chaque_panne_a_ses_parcours() {
    // Au moins 40 parcours par panne, dont certains avec des « Je ne sais pas » et des indices de Windows
    let mut par_panne: std::collections::HashMap<String, (usize, usize, usize)> = Default::default();
    for nom in juges_commencant_par("parcours-") {
        let juge = lire_juge(&nom);
        for c in juge["Parcours"].as_array().expect("Parcours") {
            let entree = choix(&c["Entree"]);
            let n = par_panne.entry(c["Panne"].as_str().unwrap_or_default().to_string()).or_default();
            n.0 += usize::from(c["Pas"].as_array().is_some_and(|p| !p.is_empty()));
            n.1 += usize::from(entree.contains(&-1));
            n.2 += usize::from(c["Indices"].as_i64().is_some_and(|i| i >= 0));
        }
    }
    let cat = catalogue();
    for p in cat.pannes() {
        let id = texte(champ(p, "Id"));
        let n = par_panne.get(&id).copied().unwrap_or_default();
        assert!(
            n.0 >= 40 && n.1 >= 1 && n.2 >= 1,
            "panne {id} : {n:?} (parcours, avec « Je ne sais pas », avec indices)"
        );
    }
}
