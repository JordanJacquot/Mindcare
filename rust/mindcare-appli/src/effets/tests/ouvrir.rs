//! « Ouvrir Nevermind » exécuté par le programme, dans un dossier d'essai (jamais le vrai dossier de Mindset) : un
//! faux suite.json dit si Nevermind est installé, la demande est écrite sur un fil à part dans le dossier de données
//! d'essai, un faux Mindset la lit (ou personne : reprise au bout de 3 s, avec la bulle), un clic répété pendant
//! l'attente ne refait rien, et une ligne au journal.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mindcare_donnees::IndicesGardes;
use serde_json::{Value, json};

use super::{Banc, cat};
use crate::ouvrir::{ATTENTE, DEMANDE, Suite};
use crate::pages::{ecrans, ids, textes_ouvrir as x};

/// Le dossier de données d'essai où Mindset lirait la demande, et le faux suite.json.
fn suite(b: &Banc) -> Suite {
    Suite { dossier_donnees: b.racine.join("Donnees"), fichier_suite: b.racine.join("suite.json") }
}

/// Le faux suite.json : Nevermind installé (son programme dans un dossier d'essai), ou pas.
fn ecrire_suite(racine: &Path, installe: bool) {
    let nm = racine.join("Programmes").join("Nevermind");
    std::fs::create_dir_all(&nm).unwrap();
    std::fs::write(nm.join("Nevermind.exe"), b"").unwrap();
    let applis = if installe { json!({ "nevermind": nm.to_string_lossy() }) } else { json!({}) };
    std::fs::write(racine.join("suite.json"), json!({ "theme": "neon", "applis": applis }).to_string()).unwrap();
}

/// Le banc sur la réponse de « Les jeux saccadent », dans Mindset (ou pas), géré par Mindset (ou pas).
fn banc(gere: bool, dans_mindset: bool, installe: bool, attente: Duration) -> Banc {
    let mut b = Banc::nouveau(Ok(cat()), false);
    ecrire_suite(&b.racine, installe);
    if gere {
        let s = suite(&b);
        b.b.brancher_mindset(s, attente);
    }
    b.b.pages.indices_du_jour = Some(IndicesGardes::default());
    b.action("panne:saccades", Value::Null, ecrans::RACONTE);
    b.jusqu_a_la_reponse("0");
    b.b.noter_dans_mindset(dans_mindset);
    b
}

/// Le texte du bouton dans la dernière page envoyée.
fn bouton(b: &Banc) -> Option<String> {
    let p = b.dernier_etat()["pages"][ecrans::ENQUETE].clone();
    let blocs = p["blocs"].as_array().cloned().unwrap_or_default();
    blocs
        .iter()
        .find(|l| l["bouton"]["id"] == ids::OUVRIR)
        .map(|l| l["bouton"]["texte"].as_str().unwrap_or_default().to_string())
}

/// Les pas de la minuterie pendant au plus `max`, jusqu'à ce que `fini` dise oui.
fn tours_jusqu_a(b: &mut Banc, max: Duration, fini: impl Fn(&Banc) -> bool) -> bool {
    let debut = Instant::now();
    while debut.elapsed() < max {
        b.tour(Instant::now());
        if fini(b) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

fn demande(b: &Banc) -> PathBuf {
    b.racine.join("Donnees").join(DEMANDE)
}

fn lignes_ouvrir(b: &Banc) -> Vec<String> {
    b.journal().lines().filter(|l| l.contains("Ouvrir Nevermind :")).map(str::to_string).collect()
}

#[test]
fn installe_ou_pas_le_bouton_suit_suite_json() {
    for installe in [true, false] {
        let mut b = banc(true, true, installe, ATTENTE);
        let attendu = if installe { x::OUVRIR } else { x::INSTALLER };
        assert!(tours_jusqu_a(&mut b, Duration::from_secs(5), |b| bouton(b).as_deref() == Some(attendu)), "{installe}");
        let j = b.journal();
        assert!(j.contains(if installe { "Nevermind : installé par Mindset" } else { "Nevermind : pas installé" }));
        // Nevermind installé ou désinstallé pendant que Mindcare tourne : le bouton suit (relu toutes les 5 s)
        ecrire_suite(&b.racine, !installe);
        let autre = if installe { x::INSTALLER } else { x::OUVRIR };
        assert!(tours_jusqu_a(&mut b, Duration::from_secs(8), |b| bouton(b).as_deref() == Some(autre)), "{installe}");
        // Sorti de Mindset : plus de bouton
        b.b.noter_dans_mindset(false);
        b.tour(Instant::now());
        assert_eq!(bouton(&b), None);
    }
}

#[test]
fn pas_de_bouton_sans_mindset() {
    // Mindcare seul (copie de travail, ou gérée mais dans sa propre fenêtre) : jamais de bouton, rien d'écrit
    for (gere, dans) in [(false, true), (false, false), (true, false)] {
        let mut b = banc(gere, dans, true, ATTENTE);
        assert!(!tours_jusqu_a(&mut b, Duration::from_millis(400), |b| bouton(b).is_some()), "{gere} {dans}");
        b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE);
        b.tour(Instant::now());
        assert!(!demande(&b).exists());
        assert!(lignes_ouvrir(&b).is_empty(), "{:?}", lignes_ouvrir(&b));
        assert!(b.messages().is_empty());
    }
}

#[test]
fn lue_par_un_faux_mindset() {
    let mut b = banc(true, true, true, ATTENTE);
    assert!(tours_jusqu_a(&mut b, Duration::from_secs(5), |b| bouton(b).is_some()));
    b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE);
    // (écrite sur un fil à part : le faux Mindset la trouve, la lit et l'efface)
    let f = demande(&b);
    let debut = Instant::now();
    let mut lu = None;
    while lu.is_none() && debut.elapsed() < Duration::from_secs(2) {
        if let Ok(o) = std::fs::read(&f) {
            lu = Some(o);
            // un clic de plus pendant l'attente : rien de plus
            b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE);
            std::fs::remove_file(&f).unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(lu.as_deref(), Some(&b"nevermind"[..]), "exactement l'id, sans retour à la ligne ni marque");
    assert!(tours_jusqu_a(&mut b, Duration::from_secs(2), |b| !lignes_ouvrir(b).is_empty()));
    assert_eq!(lignes_ouvrir(&b).len(), 1);
    assert!(lignes_ouvrir(&b)[0].ends_with("Ouvrir Nevermind : demande lue par Mindset"));
    assert!(b.messages().is_empty(), "{:?}", b.messages());
    // Rien d'autre dans le dossier de données (le fichier provisoire est renommé)
    let restes: Vec<String> = std::fs::read_dir(b.racine.join("Donnees"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("demande"))
        .collect();
    assert!(restes.is_empty(), "{restes:?}");
    // Et l'on peut redemander
    assert!(!demande(&b).exists());
    b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE);
    assert!(tours_jusqu_a(&mut b, Duration::from_secs(2), |b| demande(b).exists()));
}

#[test]
fn personne_ne_la_lit_reprise_au_bout_de_3_s() {
    let mut b = banc(true, true, false, ATTENTE);
    assert!(tours_jusqu_a(&mut b, Duration::from_secs(5), |b| bouton(b).as_deref() == Some(x::INSTALLER)));
    let debut = Instant::now();
    b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE);
    assert!(tours_jusqu_a(&mut b, Duration::from_secs(2), |b| demande(b).exists()));
    assert_eq!(std::fs::read(demande(&b)).unwrap(), b"nevermind", "pas installé : la même demande");
    // Des clics pendant l'attente : ignorés (une seule demande, une seule bulle)
    for _ in 0..3 {
        b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE);
        b.tour(Instant::now());
    }
    assert!(b.messages().is_empty(), "pas de bulle avant 3 s");
    assert!(tours_jusqu_a(&mut b, Duration::from_secs(6), |b| !b.messages().is_empty()));
    let ecoule = debut.elapsed();
    assert!(ecoule >= Duration::from_secs(3), "{ecoule:?}");
    assert!(ecoule < Duration::from_secs(5), "{ecoule:?}");
    assert_eq!(b.messages(), [(x::DEPUIS_L_ACCUEIL.to_string(), false)]);
    assert!(!demande(&b).exists(), "reprise par Mindcare");
    let l = lignes_ouvrir(&b);
    assert_eq!(l.len(), 1, "{l:?}");
    assert!(l[0].contains("demande pas lue en 3 s, reprise"), "{l:?}");
    // (et plus rien ensuite)
    std::thread::sleep(Duration::from_millis(200));
    b.tour(Instant::now());
    assert_eq!(b.messages().len(), 1);
}
