//! Le serveur de l'Aide avec le FAUX PC et un faux canal (src\essais.rs) : les réponses, la liste fermée, une seule
//! réparation à la fois, l'appelant vérifié, le journal, l'arrêt quand Mindcare est fermé. Aucune vraie Aide lancée,
//! aucun vrai canal, rien touché sur le PC.

use super::*;
use crate::essais::{Appel, BRUNO, ELODIE, FauxHote, FauxPc, RACINE, Reglage};
use crate::systeme::Identite;

fn etat() -> EtatAide {
    EtatAide { admin: true, compte: "S-1-5-18".into(), pid: 4242 }
}

fn reponse(pc: &FauxPc, occupee: &AtomicBool, journal: &Journal, ligne: &str) -> Value {
    let (texte, _) = repondre(ligne, pc, &etat(), occupee, journal);
    serde_json::from_str(&texte).unwrap()
}

#[test]
fn droits_du_canal_comme_ceux_de_nevermind() {
    let u = ELODIE;
    assert_eq!(droits_canal(u, true, true), format!("O:SYD:P(D;;0x1f019f;;;NU)(A;;0x12019b;;;{u})(A;;0x1f019f;;;SY)"));
    assert_eq!(droits_canal(u, true, false), format!("O:BAD:P(D;;0x1f019f;;;NU)(A;;0x12019b;;;{u})(A;;0x1f019f;;;BA)"));
    assert_eq!(droits_canal(u, false, false), format!("D:P(D;;0x1f019f;;;NU)(A;;0x12019f;;;{u})(A;;0x1f019f;;;BA)"));
}

#[test]
fn reponses_et_liste_fermee() {
    let pc = FauxPc::default();
    let (o, j) = (AtomicBool::new(false), Journal::default());
    let r = reponse(&pc, &o, &j, r#"{"a":"aide.bonjour"}"#);
    assert_eq!(r["ok"], true);
    assert_eq!(r["r"]["Version"], 1);
    for ligne in [
        r#"{"a":"reg.ecrire","p":{"Cle":"HKLM:\\SOFTWARE\\X","Nom":"Y"}}"#,
        r#"{"a":"maj.vider","p":{"Dossier":"C:\\Users"}}"#,
        r#"{"a":"MAJ.VIDER"}"#,
        "pas du json",
    ] {
        let r = reponse(&pc, &o, &j, ligne);
        assert_eq!(r["ok"], false, "{ligne}");
        assert!(r["e"].as_str().unwrap().starts_with("Demande refusée"), "{ligne}");
    }
    // Aucun refus n'a rien fait sur le PC
    assert!(pc.appels().is_empty(), "{:?}", pc.appels());
    // Le journal : chaque demande, refusée ou faite (pas « aide.journal » elle-même)
    let r = reponse(&pc, &o, &j, r#"{"a":"aide.journal"}"#);
    let lignes: Vec<String> = serde_json::from_value(r["r"].clone()).unwrap();
    assert_eq!(lignes.len(), 0, "le journal est noté par traiter, pas par repondre");
    let (_, resume) = repondre(r#"{"a":"reg.ecrire"}"#, &pc, &etat(), &o, &j);
    assert!(resume.unwrap().starts_with("reg.ecrire : refusé ou raté (Demande refusée"));
}

#[test]
fn une_seule_reparation_a_la_fois() {
    let pc = FauxPc::default();
    let (o, j) = (AtomicBool::new(true), Journal::default());
    let r = reponse(&pc, &o, &j, r#"{"a":"energie.par-defaut"}"#);
    assert_eq!((r["ok"].clone(), r["e"].clone()), (json!(false), json!(demandes::OCCUPEE)));
    assert!(pc.appels().is_empty());
    // (« aide.bonjour » et « aide.journal » passent pendant une réparation)
    assert_eq!(reponse(&pc, &o, &j, r#"{"a":"aide.bonjour"}"#)["ok"], true);
    assert_eq!(reponse(&pc, &o, &j, r#"{"a":"aide.journal"}"#)["ok"], true);
    // Libre : elle part, et la place est rendue après, même si l'Aide a paniqué
    o.store(false, Ordering::SeqCst);
    assert_eq!(reponse(&pc, &o, &j, r#"{"a":"energie.par-defaut"}"#)["ok"], true);
    assert!(!o.load(Ordering::SeqCst));
    let pc = FauxPc::nouveau(Reglage { powercfg_panique: true, ..Reglage::default() });
    let r = reponse(&pc, &o, &j, r#"{"a":"energie.par-defaut"}"#);
    assert_eq!(r["e"], actions::phrases::INTERNE);
    assert!(!o.load(Ordering::SeqCst), "la place est rendue après une panique");
    assert!(j.lignes().iter().any(|l| l.ends_with("energie.par-defaut : commencé")));
}

#[test]
fn traiter_une_connexion() {
    let pc = FauxPc::default();
    let (o, j) = (AtomicBool::new(false), Journal::default());
    let a = Appel::nouveau(77, r#"{"a":"aide.bonjour"}"#);
    traiter(crate::essais::connexion(a.clone()), &pc, &etat(), &o, &j);
    assert!(a.reponse().ends_with("\r\n"));
    assert!(a.reponse().starts_with(r#"{"ok":true"#));
    assert!(j.lignes()[0].ends_with("aide.bonjour : fait"));
    // Trop long : coupé sans réponse
    let mut long = Appel::nouveau(77, "");
    long.envoi = vec![b'x'; 70_000];
    traiter(crate::essais::connexion(long.clone()), &pc, &etat(), &o, &j);
    assert_eq!(long.reponse(), "");
    assert!(j.lignes().iter().any(|l| l.contains("demande trop longue")));
}

#[test]
fn seul_mindset_exe_de_mindcare_du_compte_servi() {
    let pc = FauxPc::nouveau(Reglage {
        processus: vec![
            (10, format!("{RACINE}\\Mindset.exe"), ELODIE.into()),
            (11, format!("{RACINE}\\Mindset.exe"), BRUNO.into()),
            (12, "C:\\Users\\Elodie\\Downloads\\Mindset.exe".into(), ELODIE.into()),
            (13, format!("{RACINE}\\Mindcare.exe"), ELODIE.into()),
        ],
        ..Reglage::default()
    });
    let ok = |pid: i64, admin: bool| client_servi(&*pc, pid, &pc.chemin_processus(pid), RACINE, admin, ELODIE);
    assert!(ok(10, true));
    assert!(!ok(11, true), "un autre compte, même programme");
    assert!(!ok(12, true) && !ok(13, true), "un autre programme");
    // (sans droits administrateur, l'Aide ne peut rien de plus que l'utilisateur : le compte n'est pas regardé)
    assert!(ok(11, false));
}

/// L'Aide entière, avec le faux hôte : des appels donnés à l'avance, puis Mindcare fermé
#[test]
fn l_aide_entiere_refuse_les_etrangers_puis_s_arrete_seule() {
    let pc = FauxPc::nouveau(Reglage {
        processus: vec![
            (10, format!("{RACINE}\\Mindset.exe"), ELODIE.into()),
            (11, format!("{RACINE}\\Mindset.exe"), BRUNO.into()),
            (12, "C:\\Users\\Elodie\\Downloads\\outil.exe".into(), ELODIE.into()),
        ],
        ..Reglage::default()
    });
    let etranger = Appel::nouveau(12, r#"{"a":"aide.bonjour"}"#);
    let bruno = Appel::nouveau(11, r#"{"a":"maj.vider"}"#);
    let elodie = Appel::nouveau(10, r#"{"a":"aide.bonjour"}"#);
    pc.file.lock().unwrap().extend([etranger.clone(), bruno.clone(), elodie.clone()]);
    let delais = Delais { recherche: Duration::from_millis(20), seule_au_plus: Duration::from_millis(200) };
    let h = Arc::new(FauxHote(Arc::clone(&pc)));
    let fin = std::thread::spawn(move || lancer(h, Some(ELODIE), delais));
    // (Élodie a sa réponse)
    let debut = Instant::now();
    while elodie.reponse().is_empty() && debut.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(elodie.reponse().starts_with(r#"{"ok":true"#), "{}", elodie.reponse());
    assert_eq!((etranger.reponse(), bruno.reponse()), (String::new(), String::new()), "un étranger a eu une réponse");
    // Le journal, demandé par Élodie : les refus y sont
    let journal = Appel::nouveau(10, r#"{"a":"aide.journal"}"#);
    pc.file.lock().unwrap().push_back(journal.clone());
    while journal.reponse().is_empty() && debut.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let j = journal.reponse();
    assert!(j.contains("Refusé : demande venant de C:\\\\Users\\\\Elodie\\\\Downloads\\\\outil.exe"), "{j}");
    assert!(j.contains(&format!("(compte {BRUNO})")), "{j}");
    assert!(j.contains("démarrée en SYSTEM pour"), "{j}");
    assert!(pc.appels().is_empty(), "maj.vider de Bruno a touché au PC : {:?}", pc.appels());
    // Le canal : celui de Mindcare, pour Élodie, propriétaire le système
    let canaux = pc.canaux.lock().unwrap().clone();
    let s = mindcare_aide_regles::canal::suffixe(RACINE).unwrap();
    assert_eq!(canaux[0].0, format!("Mindcare-Aide-{ELODIE}-{s}"));
    assert!(canaux[0].2 && canaux[0].1.starts_with("O:SYD:P"));
    assert!(canaux[1..].iter().all(|c| !c.2 && c.3.as_deref() == Some("S-1-5-18")));
    // Mindcare fermé : l'Aide s'arrête seule
    pc.r.lock().unwrap().processus.retain(|p| p.0 != 10);
    let debut = Instant::now();
    while !fin.is_finished() && debut.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(fin.is_finished(), "l'Aide ne s'arrête pas une fois Mindcare fermé");
}

#[test]
fn en_systeme_sans_sid_l_aide_ne_demarre_pas() {
    for sid in [None, Some("S-1-5-18"), Some("pas-un-sid")] {
        let pc = FauxPc::nouveau(Reglage::default());
        lancer(Arc::new(FauxHote(Arc::clone(&pc))), sid, Delais::NORMAUX);
        assert!(pc.canaux.lock().unwrap().is_empty(), "{sid:?}");
    }
    // Sous le compte de Bruno, pour Élodie : non plus
    let pc = FauxPc::nouveau(Reglage {
        identite: Identite { compte: BRUNO.into(), admin: true, pid: 1 },
        ..Reglage::default()
    });
    lancer(Arc::new(FauxHote(Arc::clone(&pc))), Some(ELODIE), Delais::NORMAUX);
    assert!(pc.canaux.lock().unwrap().is_empty());
}
