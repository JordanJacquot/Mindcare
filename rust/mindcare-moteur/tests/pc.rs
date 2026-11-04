//! Les pannes qui concernent ce PC (« PourPc ») : le tri par profil, la règle « ce qu'on ne sait pas ne cache rien »,
//! la validation d'un « PourPc » mal écrit, et les juges de PowerShell rejoués avec le profil « je ne sais pas ».

mod commun;

use commun::{catalogue, lire_juge, verdict_final};
use mindcare_moteur::pc::{CHAMP, problemes, trier};
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{Catalogue, MARQUES, Objet, ProfilPc, TypePc, chercher, chercher_pour, concerne, verifier};
use serde_json::{Value, json};

fn objet(v: Value) -> Objet {
    v.as_object().cloned().expect("un objet")
}

fn profil(genre: Option<TypePc>, marque: Option<&str>) -> ProfilPc {
    ProfilPc { genre, marque: marque.map(str::to_string) }
}

fn ids<'a>(l: impl IntoIterator<Item = &'a Objet>) -> Vec<String> {
    l.into_iter().map(|p| texte(champ(p, "Id"))).collect()
}

#[test]
fn une_panne_et_un_profil() {
    let portable = objet(json!({ "Id": "a", "PourPc": { "Type": "portable" } }));
    let fixe = objet(json!({ "Id": "b", "PourPc": { "Type": "fixe" } }));
    let hp = objet(json!({ "Id": "c", "PourPc": { "Marque": ["HP"] } }));
    let hp_dell_portable = objet(json!({ "Id": "d", "PourPc": { "Type": "portable", "Marque": ["HP", "Dell"] } }));
    let tous = objet(json!({ "Id": "e" }));

    let p_hp = profil(Some(TypePc::Portable), Some("HP"));
    assert!(concerne(&portable, &p_hp));
    assert!(!concerne(&fixe, &p_hp));
    assert!(concerne(&hp, &p_hp));
    assert!(concerne(&hp_dell_portable, &p_hp));
    assert!(concerne(&tous, &p_hp));

    let f_asus = profil(Some(TypePc::Fixe), Some("ASUS"));
    assert!(!concerne(&portable, &f_asus));
    assert!(concerne(&fixe, &f_asus));
    assert!(!concerne(&hp, &f_asus));
    assert!(!concerne(&hp_dell_portable, &f_asus));
    assert!(concerne(&tous, &f_asus));

    // (la marque se compare sans tenir compte des majuscules)
    assert!(concerne(&hp, &profil(None, Some("hp"))));
    let p_dell = profil(Some(TypePc::Portable), Some("Dell"));
    assert!(concerne(&hp_dell_portable, &p_dell));
    assert!(!concerne(&hp_dell_portable, &profil(Some(TypePc::Fixe), Some("Dell"))));
}

#[test]
fn ce_qu_on_ne_sait_pas_ne_cache_rien() {
    let pannes = [
        objet(json!({ "Id": "a", "PourPc": { "Type": "portable" } })),
        objet(json!({ "Id": "b", "PourPc": { "Type": "fixe" } })),
        objet(json!({ "Id": "c", "PourPc": { "Marque": ["HP"] } })),
        objet(json!({ "Id": "d", "PourPc": { "Type": "portable", "Marque": ["Dell"] } })),
    ];
    // Rien de connu : tout
    let t = trier(&pannes, &ProfilPc::inconnu());
    assert_eq!((t.visibles.len(), t.cachees), (4, 0));
    // Le type seul : les marques ne cachent rien
    let t = trier(&pannes, &profil(Some(TypePc::Portable), None));
    assert_eq!(ids(t.visibles), ["a", "c", "d"]);
    assert_eq!(t.cachees, 1);
    // La marque seule (une marque vide compte pour rien) : le type ne cache rien
    let t = trier(&pannes, &profil(None, Some("Dell")));
    assert_eq!(ids(t.visibles), ["a", "b", "d"]);
    let t = trier(&pannes, &profil(None, Some("  ")));
    assert_eq!(t.cachees, 0);
    // Un « PourPc » mal écrit (refusé par la validation) ne cache rien non plus
    let abimes = [
        objet(json!({ "PourPc": "portable" })),
        objet(json!({ "PourPc": { "Type": "tablette" } })),
        objet(json!({ "PourPc": { "Marque": [] } })),
        objet(json!({ "PourPc": { "Marque": "HP" } })),
    ];
    for a in &abimes {
        assert!(concerne(a, &profil(Some(TypePc::Fixe), Some("ASUS"))), "{a:?}");
    }
}

#[test]
fn le_libelle_court() {
    assert_eq!(profil(Some(TypePc::Portable), Some("HP")).libelle_court(), "Portable HP");
    assert_eq!(profil(Some(TypePc::Fixe), Some("ASUS")).libelle_court(), "PC fixe ASUS");
    assert_eq!(profil(Some(TypePc::Fixe), None).libelle_court(), "PC fixe");
    assert_eq!(profil(None, Some("Dell")).libelle_court(), "PC Dell");
    assert_eq!(ProfilPc::inconnu().libelle_court(), "Tous les PC");
    assert!(ProfilPc::inconnu().est_inconnu());
    assert!(profil(None, Some("")).est_inconnu());
    assert!(!profil(None, Some("HP")).est_inconnu());
    assert_eq!(TypePc::depuis("Tour"), Some(TypePc::Fixe));
    assert_eq!(TypePc::depuis("PORTABLE"), Some(TypePc::Portable));
    assert_eq!(TypePc::depuis("tablette"), None);
}

#[test]
fn le_vrai_catalogue() {
    let cat = catalogue();
    // Rien de connu : aucune panne cachée
    assert_eq!(cat.pannes_pour(&ProfilPc::inconnu()).cachees, 0);
    let avec: Vec<&Objet> = cat.pannes().iter().filter(|p| champ(p, CHAMP).is_some()).collect();
    assert!(avec.len() >= 20, "trop peu de pannes avec PourPc ({})", avec.len());
    for p in &avec {
        assert!(problemes(champ(p, CHAMP).expect("PourPc")).is_empty(), "{}", texte(champ(p, "Id")));
    }

    let tour_asus = profil(Some(TypePc::Fixe), Some("ASUS"));
    let portable_hp = profil(Some(TypePc::Portable), Some("HP"));
    let visible = |id: &str, pr: &ProfilPc| concerne(cat.panne(id).expect(id), pr);
    for id in ["necharge", "tientpeu", "gonfle", "capot", "portablecasse", "capslockhp", "voyantdell"] {
        assert!(!visible(id, &tour_asus), "{id} montré à une tour ASUS");
    }
    for id in ["ledcartemere", "qcode", "ledgpu", "microjack", "voyantdell"] {
        assert!(!visible(id, &portable_hp), "{id} montré à un portable HP");
    }
    for id in ["necharge", "capslockhp", "redemarre", "plusinternet", "etincelles", "logobloque"] {
        assert!(visible(id, &portable_hp), "{id} caché à un portable HP");
    }
    for id in ["ledcartemere", "qcode", "redemarre", "logobloque", "etincelles"] {
        assert!(visible(id, &tour_asus), "{id} caché à une tour ASUS");
    }
    // Une famille entière de portable : presque tout caché pour une tour, mais le chargeur qui fait des étincelles reste
    // (la prise du mur peut aussi être en cause)
    let t = cat.pannes_famille_pour("batterie", &tour_asus);
    assert_eq!(ids(t.visibles), ["etincelles"]);
    assert_eq!(t.cachees + 1, cat.pannes_famille("batterie").len());
    // La recherche dit combien de pannes trouvées sont cachées
    let t = chercher_pour(cat, "batterie", &tour_asus);
    assert!(t.cachees >= 5, "{}", t.cachees);
    assert!(t.visibles.iter().all(|p| concerne(p, &tour_asus)));
    assert_eq!(t.visibles.len() + t.cachees, chercher(cat, "batterie").len());
}

#[test]
fn les_recherches_de_powershell_avec_je_ne_sais_pas() {
    let juge = lire_juge("recherches.json");
    let mut cas: Vec<&Value> = juge["Recherches"].as_array().expect("Recherches").iter().collect();
    cas.extend(juge["Tester"].as_array().expect("Tester"));
    let mut ecarts = Vec::new();
    for c in &cas {
        let q = c["Q"].as_str().unwrap_or_default();
        let attendu: Vec<String> = c["P"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let t = chercher_pour(catalogue(), q, &ProfilPc::inconnu());
        let obtenu = ids(t.visibles);
        if obtenu != attendu || t.cachees != 0 {
            ecarts.push(format!("« {q} » : PowerShell {attendu:?} / Rust {obtenu:?} ({} cachées)", t.cachees));
        }
    }
    verdict_final("recherches avec « je ne sais pas »", cas.len(), &ecarts);
}

#[test]
fn la_validation_refuse_un_pour_pc_mal_ecrit() {
    let base = |pour: Value| {
        json!({
            "Familles": [{ "Id": "f", "Titre": "F", "Phrase": "P", "Icone": "i" }],
            "Suspects": { "s": { "Titre": "S", "Trouve": "T", "Pourquoi": "P", "AFaire": "A", "Pro": "R" } },
            "Tests": {},
            "Pannes": [{
                "Id": "p", "Famille": "f", "Titre": "T", "Phrase": "P", "Suspects": ["s"], "PourPc": pour,
                "Etapes": [{ "Id": "e", "Type": "question", "Texte": "T", "Court": "C",
                    "Reponses": [{ "Texte": "a" }, { "Texte": "b" }] }]
            }]
        })
        .to_string()
    };
    let pb = |pour: Value| {
        let j = base(pour);
        verifier(&Catalogue::lire(&j).expect("lisible"), &j)
    };
    assert!(pb(json!({ "Type": "portable" })).is_empty());
    assert!(pb(json!({ "Type": "fixe", "Marque": ["HP", "Dell"] })).is_empty());
    for m in MARQUES {
        assert!(pb(json!({ "Marque": [m] })).is_empty(), "{m}");
    }
    let mal = [
        json!("portable"),
        json!({}),
        json!({ "Type": "tour" }),
        json!({ "Type": "Portable" }),
        json!({ "Type": ["portable"] }),
        json!({ "Marque": "HP" }),
        json!({ "Marque": [] }),
        json!({ "Marque": ["Hewlett-Packard"] }),
        json!({ "Marque": [3] }),
        json!({ "Typ": "portable" }),
    ];
    for m in mal {
        let p = pb(m.clone());
        assert!(!p.is_empty() && p.iter().all(|x| x.starts_with("Panne p : PourPc")), "{m} : {p:?}");
    }
    // Sur une étape, il est refusé (il changerait le parcours des enquêtes gardées)
    let mut v: Value = serde_json::from_str(&base(json!({ "Type": "portable" }))).expect("json");
    v["Pannes"][0]["Etapes"][0]["PourPc"] = json!({ "Type": "portable" });
    let j = v.to_string();
    let p = verifier(&Catalogue::lire(&j).expect("lisible"), &j);
    assert_eq!(p.len(), 1, "{p:?}");
    assert!(p[0].starts_with("Panne p, étape e : PourPc"), "{p:?}");
}
