//! Le vrai disque ([`DisqueReel`]) sur un dossier d'essai sous %TEMP% (jamais les vraies données), et un démarrage
//! complet avec lui (Windows reste faux : ni verrou, ni signal, ni registre, ni programme lancé).

mod faux;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mindcare_donnees::{Donnees, Moment, horloge_figee};
use mindcare_vie::{Depart, Disque, DisqueReel, Environnement, Noms, Reglages, Taille, Vie, demarrer};

/// Un dossier d'essai neuf sous %TEMP%, effacé à la fin.
struct Essai(PathBuf);

impl Essai {
    fn nouveau(nom: &str) -> Essai {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let d = std::env::temp_dir().join(format!("MindcareEssai-{}-{nom}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        Essai(d)
    }
}

impl Drop for Essai {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn horloge() -> mindcare_donnees::Horloge {
    horloge_figee(Moment::nouveau(2026, 11, 2, 14, 5, 0).unwrap())
}

#[test]
fn fichiers_journal_et_suite() {
    let e = Essai::nouveau("disque");
    let mut d = DisqueReel::nouveau(horloge(), Box::new(|| {}));
    let f = e.0.join("a.txt");
    assert!(!d.existe(&f) && !d.est_fichier(&f) && d.date(&f).is_none());
    // UTF-8 sans BOM (mindset.ps1:94)
    d.ecrire_texte(&f, "appli:mindcare").unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), b"appli:mindcare");
    assert!(d.existe(&f) && d.est_fichier(&f) && d.date(&f).is_some());
    // un dossier existe (Test-Path) mais n'est pas un fichier, et n'a pas de date de fichier
    assert!(d.existe(&e.0) && !d.est_fichier(&e.0) && d.date(&e.0).is_none());
    d.effacer(&f).unwrap();
    assert!(!d.existe(&f));
    // le journal, une fois le dossier de données ouvert
    d.journal("avant"); // (pas encore de journal : rien)
    d.ouvrir_donnees(&e.0);
    d.journal("Fermeture");
    let j = std::fs::read_to_string(e.0.join("journal.txt")).unwrap();
    assert_eq!(j, "2026-11-02T14:05:00 Fermeture\r\n");
    // suite.json lu par dossier.rs
    let s = e.0.join("suite.json");
    std::fs::write(&s, r#"{"theme":"Rubis","applis":{"mindcare":"E:\\Mindset\\Programmes\\Mindcare\\"}}"#).unwrap();
    let suite = d.lire_suite(&s, 1);
    assert!(suite.existe && suite.lisible);
    assert_eq!(suite.theme.as_deref(), Some("rubis"));
    assert_eq!(suite.dossier.as_deref(), Some(r"E:\Mindset\Programmes\Mindcare"));
}

#[test]
fn etat_json_par_la_carte() {
    let e = Essai::nouveau("etat");
    let donnees = Rc::new(RefCell::new(Donnees::ouvrir(&e.0, horloge(), true)));
    let pour_etat = donnees.clone();
    let mut d = DisqueReel::nouveau(horloge(), Box::new(move || pour_etat.borrow_mut().ecrire_etat(None)));
    d.ecrire_etat();
    let etat = e.0.join("etat.json");
    let texte = std::fs::read_to_string(&etat).unwrap();
    assert!(texte.contains("Aucun diagnostic pour l'instant."), "{texte}");
    assert!(texte.contains("\"Maj\""));
    // la même carte n'est pas réécrite (carte.rs, systeme.ps1:76)
    std::fs::remove_file(&etat).unwrap();
    d.ecrire_etat();
    assert!(!etat.exists());
}

#[test]
fn demarrage_complet_sur_le_disque() {
    let e = Essai::nouveau("depart");
    let racine = e.0.join(r"X\Mindset\Programmes\Mindcare");
    let local = e.0.join("Local");
    std::fs::create_dir_all(&racine).unwrap();
    std::fs::create_dir_all(local.join("Mindset")).unwrap();
    std::fs::write(racine.join("Mindset.exe"), b"").unwrap();
    let racine_texte = racine.to_string_lossy().to_string();
    std::fs::write(local.join(r"Mindset\suite.json"), serde_json_texte(&racine_texte)).unwrap();
    // une vieille demande de fermeture dans le dossier de données
    let donnees = e.0.join(r"X\Mindset\Donnees\Mindcare");
    std::fs::create_dir_all(&donnees).unwrap();
    std::fs::write(donnees.join("quitter.demande"), b"999").unwrap();

    let env = Environnement {
        racine: Some(racine_texte),
        localappdata: local.to_string_lossy().to_string(),
        program_files: None,
        program_w6432: None,
        version: "0.16.0".into(),
    };
    let mut systeme = faux::FauxSysteme::default();
    let mut disque = DisqueReel::nouveau(horloge(), Box::new(|| {}));
    let d = demarrer(true, &env, &Noms::mindcare(), &mut systeme, &mut disque);
    let Depart::Lancer(l) = d else { panic!("{d:?}") };
    assert!(l.gere);
    assert_eq!(l.suite.theme.as_deref(), Some("crepuscule"));
    assert_eq!(l.dossier_donnees, donnees);
    assert!(!donnees.join("quitter.demande").exists(), "vieille demande effacée");
    assert_eq!(l.programme.as_deref(), Some(racine.join("Mindset.exe").as_path()));
    let journal = std::fs::read_to_string(donnees.join("journal.txt")).unwrap();
    assert!(journal.contains("Démarrage Mindcare 0.16.0 (Rust), lancée cachée"), "{journal}");

    // puis quitter.demande déposé par Mindset, lu sur le vrai disque
    let mut fenetre = faux::FausseFenetre::default();
    let mut v = Vie::nouvelle(Reglages::depuis(&l, "crepuscule", Taille { largeur: 900.0, hauteur: 640.0 }));
    let mut m = mindcare_vie::Monde { systeme: &mut systeme, disque: &mut disque, fenetre: &mut fenetre };
    v.commencer(&mut m);
    assert_eq!(v.avancer(std::time::Duration::from_secs(2), &mut m), None);
    std::fs::write(donnees.join("quitter.demande"), b"1234").unwrap();
    assert_eq!(v.avancer(std::time::Duration::from_millis(2500), &mut m), Some(mindcare_vie::Fin::Quitter));
    assert!(!Path::new(&donnees.join("quitter.demande")).exists());
}

/// suite.json qui gère ce dossier, au thème Crépuscule.
fn serde_json_texte(racine: &str) -> String {
    format!(
        r#"{{"theme":"crepuscule","applis":{{"mindcare":"{}"}},"modifie":"2026-11-02T14:00:00"}}"#,
        racine.replace('\\', "\\\\")
    )
}
