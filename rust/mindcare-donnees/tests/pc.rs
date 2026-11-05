//! Le PC dont on regarde les pannes (« mon PC » ou un autre choisi à la main), gardé dans les réglages d'un dossier
//! d'essai (jamais le vrai dossier de données), et la fiche du PC dans le rapport du réparateur.

mod commun;

use commun::{DossierEssai, catalogue, indices, lire_juge, moment, s, sans_hors_sujet, verdict_final};
use mindcare_donnees::choix_pc::REGLAGE;
use mindcare_donnees::{ChoixPc, Donnees, PcDuRapport, SurLePc, horloge_figee, lignes_fiche, texte_rapport_pc};
use mindcare_moteur::pc::FichePc;
use mindcare_moteur::{ProfilPc, TypePc, enquete};
use serde_json::{Value, json};

fn ouvrir(d: &DossierEssai) -> Donnees {
    Donnees::ouvrir(&d.0, horloge_figee(moment("2026-11-04T10:00:00")), false)
}

fn portable_hp() -> ProfilPc {
    ProfilPc { genre: Some(TypePc::Portable), marque: Some("HP".into()) }
}

#[test]
fn le_choix_est_garde_jusqu_a_revenir_a_mon_pc() {
    let d = DossierEssai::nouveau("choix-pc");
    let mut donnees = ouvrir(&d);
    assert_eq!(donnees.choix_pc(), ChoixPc::MonPc);
    donnees.reglages.changer(&mut donnees.fichiers, "Theme", json!("clair"));

    // Un portable HP pour un ami : gardé d'un lancement à l'autre
    donnees.choisir_pc(&ChoixPc::Autre(portable_hp()));
    let mut donnees = ouvrir(&d);
    assert_eq!(donnees.choix_pc(), ChoixPc::Autre(portable_hp()));
    assert!(donnees.choix_pc().pour_un_autre());
    assert_eq!(donnees.choix_pc().pastille(None), "Pour un autre PC : Portable HP");

    // « Je ne sais pas » : gardé aussi, et tout est montré
    donnees.choisir_pc(&ChoixPc::Autre(ProfilPc::inconnu()));
    let mut donnees = ouvrir(&d);
    let choix = donnees.choix_pc();
    assert_eq!(choix, ChoixPc::Autre(ProfilPc::inconnu()));
    let mon_pc = ProfilPc { genre: Some(TypePc::Fixe), marque: Some("ASUS".into()) };
    assert!(choix.profil(Some(&mon_pc)).est_inconnu());
    assert_eq!(catalogue().pannes_pour(&choix.profil(Some(&mon_pc))).cachees, 0);

    // Une marque seule (une carte mère ASUS pour un ami)
    donnees.choisir_pc(&ChoixPc::Autre(ProfilPc { genre: None, marque: Some("ASUS".into()) }));
    assert_eq!(ouvrir(&d).choix_pc().pastille(None), "Pour un autre PC : PC ASUS");

    // Revenir à mon PC : le profil reconnu (pas encore reconnu : tout est montré)
    donnees.choisir_pc(&ChoixPc::MonPc);
    let donnees = ouvrir(&d);
    assert_eq!(donnees.choix_pc(), ChoixPc::MonPc);
    assert_eq!(donnees.choix_pc().profil(Some(&mon_pc)), mon_pc);
    assert_eq!(donnees.choix_pc().pastille(Some(&mon_pc)), "Ton PC : PC fixe ASUS");
    assert!(donnees.choix_pc().profil(None).est_inconnu());
    // (les autres réglages sont gardés)
    assert_eq!(donnees.reglages.lire("Theme", Value::Null), json!("clair"));
}

#[test]
fn un_reglage_abime_revient_a_mon_pc() {
    for abime in [
        json!("portable"),
        json!(3),
        json!([]),
        json!(true),
        json!({ "Type": "tablette" }),
        json!({ "Type": 1 }),
        json!({ "Marque": "Hewlett" }),
        json!({ "Marque": ["HP"] }),
        json!({ "Type": "portable", "Marque": "" }),
    ] {
        assert_eq!(ChoixPc::depuis_json(Some(&abime)), ChoixPc::MonPc, "{abime}");
    }
    // (les majuscules et « tour » sont compris)
    assert_eq!(
        ChoixPc::depuis_json(Some(&json!({ "type": "Tour", "marque": "asus" }))),
        ChoixPc::Autre(ProfilPc { genre: Some(TypePc::Fixe), marque: Some("ASUS".into()) })
    );

    // Un fichier de réglages abîmé, ou un réglage abîmé dedans : « mon PC », rien ne casse
    let d = DossierEssai::nouveau("choix-pc-abime");
    std::fs::write(d.0.join("reglages.json"), b"{ \"PcChoisi\": { \"Type\": ").expect("fichier");
    assert_eq!(ouvrir(&d).choix_pc(), ChoixPc::MonPc);
    std::fs::write(d.0.join("reglages.json"), br#"{ "Theme": "clair", "PcChoisi": { "Type": "valise" } }"#)
        .expect("fichier");
    let mut donnees = ouvrir(&d);
    assert_eq!(donnees.choix_pc(), ChoixPc::MonPc);
    // et on peut encore choisir
    donnees.choisir_pc(&ChoixPc::Autre(portable_hp()));
    assert_eq!(ouvrir(&d).choix_pc(), ChoixPc::Autre(portable_hp()));
    assert_eq!(ouvrir(&d).reglages.lire("Theme", Value::Null), json!("clair"));
    // (l'écrit : { "Type": "portable", "Marque": "HP" }, « mon PC » : rien)
    assert_eq!(ChoixPc::Autre(portable_hp()).vers_json(), json!({ "Type": "portable", "Marque": "HP" }));
    assert_eq!(ChoixPc::MonPc.vers_json(), Value::Null);
    assert_eq!(REGLAGE, "PcChoisi");
}

fn fiche() -> FichePc {
    FichePc {
        genre: Some("Portable".into()),
        modele: Some("HP Pavilion 15".into()),
        carte_mere: Some("HP 87B2".into()),
        processeur: Some("AMD Ryzen 5 5500U".into()),
        memoire: Some("16 Go (2 barrettes)".into()),
        disque: Some("SSD NVMe de 476 Go, 120 Go libres".into()),
        windows: Some("Windows 11, version 24H2".into()),
    }
}

#[test]
fn la_fiche_du_pc_dans_le_rapport() {
    let cat = catalogue();
    let juge = lire_juge("rapports.json");
    let l = juge["Cas"].as_array().cloned().unwrap_or_default();
    let f = fiche();
    let choisi = portable_hp();
    let (mut avec_cepc, mut sans_cepc) = (0, 0);
    let mut ecarts = Vec::new();
    for c in &l {
        let p = cat.panne(s(&c["Panne"])).expect("panne");
        let entree: Vec<i64> = c["Entree"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
        let ind = if c["Indices"].is_null() { None } else { Some(indices(&c["Indices"]).moteur()) };
        let e = enquete(cat, p, &entree, ind.as_ref());
        let jour = indices(&c["IndicesJour"]);
        let barrettes: Vec<String> =
            c["Barrettes"].as_array().into_iter().flatten().map(|b| s(b).to_string()).collect();
        let pc = SurLePc { infos: s(&c["InfosPc"]), barrettes: &barrettes, indices_du_jour: Some(&jour) };
        let rapport = |detail: &PcDuRapport| {
            texte_rapport_pc(cat, p, &e, s(&c["Titre"]), &moment(s(&c["Maintenant"])), &pc, detail, s(&c["Version"]))
        };
        // (sans les écartés hors sujet : différence voulue, commun::sans_hors_sujet)
        let attendu = sans_hors_sujet(s(&c["R"]));
        // Sans rien en plus : exactement PowerShell
        let r = rapport(&PcDuRapport::default());
        if r != attendu {
            ecarts.push(format!("{} {entree:?} : sans fiche, différent de PowerShell", s(&c["Panne"])));
        }
        let r = rapport(&PcDuRapport { fiche: Some(&f), choisi: Some(&choisi) });
        let lignes: Vec<&str> = r.split("\r\n").collect();
        let cepc = e.notes.iter().any(|n| n.eq_ignore_ascii_case("cepc"));
        let ajout: Vec<String> =
            if cepc { lignes_fiche(&f, !pc.infos.is_empty()) } else { vec!["PC choisi : Portable HP".to_string()] };
        if cepc {
            avec_cepc += 1;
        } else {
            sans_cepc += 1;
        }
        // Les lignes ajoutées, juste après celles du PC (ou de la panne), et rien d'autre de changé
        let restes: Vec<&str> = lignes.iter().copied().filter(|x| !ajout.iter().any(|a| a == x)).collect();
        if restes.join("\r\n") != attendu {
            ecarts.push(format!("{} {entree:?} : le reste du rapport a changé\n{r}", s(&c["Panne"])));
        }
        let ou = lignes.iter().position(|x| *x == ajout[0]);
        let avant = ou.and_then(|i| lignes.get(i.wrapping_sub(1)).copied()).unwrap_or_default();
        if !(avant.starts_with("Panne : ") || avant.starts_with("Ordinateur : ") || avant.starts_with("Mémoire : ")) {
            ecarts.push(format!("{} {entree:?} : lignes du PC mal placées\n{r}", s(&c["Panne"])));
        }
        // Le modèle n'est pas répété quand la ligne « Ordinateur » le dit déjà
        if cepc && !pc.infos.is_empty() && r.contains("Modèle : ") {
            ecarts.push(format!("{} {entree:?} : modèle écrit deux fois", s(&c["Panne"])));
        }
        if !cepc && r.contains("Processeur : ") {
            ecarts.push(format!("{} {entree:?} : la fiche de ce PC dans une enquête pour un autre", s(&c["Panne"])));
        }
    }
    assert!(avec_cepc >= 3 && sans_cepc >= 3, "cas : {avec_cepc} sur ce PC, {sans_cepc} sur un autre");
    verdict_final("rapports avec la fiche du PC", l.len(), &ecarts);
}

#[test]
fn un_pc_monte_sans_nom_n_ecrit_pas_de_ligne_ordinateur_vide() {
    let cat = catalogue();
    let juge = lire_juge("rapports.json");
    let l = juge["Cas"].as_array().cloned().unwrap_or_default();
    let f = fiche();
    let mut essayes = 0;
    for c in &l {
        let p = cat.panne(s(&c["Panne"])).expect("panne");
        let entree: Vec<i64> = c["Entree"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
        let ind = if c["Indices"].is_null() { None } else { Some(indices(&c["Indices"]).moteur()) };
        let e = enquete(cat, p, &entree, ind.as_ref());
        if !e.notes.iter().any(|n| n.eq_ignore_ascii_case("cepc")) {
            continue;
        }
        essayes += 1;
        let jour = indices(&c["IndicesJour"]);
        let pc = SurLePc {
            infos: "System manufacturer System Product Name, Windows 11 Famille",
            barrettes: &[],
            indices_du_jour: Some(&jour),
        };
        let rapport = |detail: &PcDuRapport| {
            texte_rapport_pc(cat, p, &e, s(&c["Titre"]), &moment(s(&c["Maintenant"])), &pc, detail, s(&c["Version"]))
        };
        // Avec la fiche : la ligne sans nom part, et le modèle de la fiche est écrit à la place
        let r = rapport(&PcDuRapport { fiche: Some(&f), choisi: None });
        assert!(!r.contains("Ordinateur : "), "{r}");
        assert!(r.contains("Modèle : HP Pavilion 15") && r.contains("Carte mère : HP 87B2"), "{r}");
        // Sans fiche : rien d'autre à dire, la ligne reste comme dans PowerShell
        let r = rapport(&PcDuRapport::default());
        assert!(r.contains("Ordinateur : System manufacturer"), "{r}");
    }
    assert!(essayes >= 1, "aucun cas sur ce PC");
}

#[test]
fn les_lignes_de_la_fiche() {
    assert_eq!(
        lignes_fiche(&fiche(), false),
        [
            "Type : Portable",
            "Modèle : HP Pavilion 15",
            "Carte mère : HP 87B2",
            "Processeur : AMD Ryzen 5 5500U",
            "Mémoire installée : 16 Go (2 barrettes)",
            "Disque de Windows : SSD NVMe de 476 Go, 120 Go libres",
            "Windows : Windows 11, version 24H2",
        ]
    );
    assert_eq!(lignes_fiche(&fiche(), true).len(), 6);
    let presque_vide =
        FichePc { processeur: Some("  ".into()), windows: Some("Windows 10".into()), ..FichePc::default() };
    assert_eq!(lignes_fiche(&presque_vide, false), ["Windows : Windows 10"]);
    assert!(lignes_fiche(&FichePc::default(), false).is_empty());
}
