//! Les scénarios de la vie de Mindcare avec Mindset : un par point du contrat (D:\Projets\Mindset\docs\
//! contrat-appli.md, parties 3, 5, 6, 7 et 8), sur des faux Windows, disque et fenêtre. Chacun a sa contre-épreuve
//! (la règle cassée exprès fait échouer le test) : elles sont notées dans le compte rendu de l'étape 4.3.

mod faux;

use std::path::{Path, PathBuf};
use std::time::Duration;

use faux::{Banc, Lance};
use mindcare_donnees::dossier::Suite;
use mindcare_vie::demarrage::{ATTENTE_VERROU, ESSAIS_SUITE};
use mindcare_vie::{
    AttenteVerrou, Depart, Environnement, Fin, Lancement, Niveau, Noms, Reglages, TOUR, Taille, Vie, demarrage_demande,
    demarrer, echelle, theme_choisi,
};

// Des chemins inventés : rien n'est lu ni écrit sur le disque, tout reste dans les faux.
const LOCAL: &str = r"C:\Users\Essai\AppData\Local";
const GEREE: &str = r"E:\Mindset\Programmes\Mindcare";
const DONNEES_GEREE: &str = r"E:\Mindset\Donnees\Mindcare";
const PF: &str = r"C:\Program Files";
const MINDSET: &str = r"C:\Program Files\Mindset\Mindset";
const SUITE: &str = r"C:\Users\Essai\AppData\Local\Mindset\suite.json";
const PAGE: &str = r"C:\Users\Essai\AppData\Local\Mindset\afficher-page.txt";
const MIN: Taille = Taille { largeur: 900.0, hauteur: 640.0 };

fn env(racine: &str) -> Environnement {
    Environnement {
        racine: Some(racine.to_string()),
        localappdata: LOCAL.to_string(),
        program_files: Some(PF.to_string()),
        program_w6432: Some(PF.to_string()),
        version: "0.16.0".to_string(),
    }
}

/// suite.json lisible, qui gère ce dossier, avec ce thème.
fn suite_geree(dossier: &str, theme: &str) -> Suite {
    Suite {
        existe: true,
        lisible: true,
        pris: false,
        theme: Some(theme.to_string()),
        dossier: Some(dossier.to_string()),
        micro: String::new(),
    }
}

/// Un banc où Mindcare est gérée par Mindset (E:\Mindset\Programmes\Mindcare), Mindset installé.
fn banc_gere() -> Banc {
    let mut b = Banc::default();
    b.disque.poser(SUITE, "{}", 0);
    b.disque.suite = suite_geree(GEREE, "neon");
    b.systeme.emplacement = Some(format!("  \"{MINDSET}\\\"  "));
    b.disque.poser(&format!(r"{MINDSET}\Mindset.exe"), "", 0);
    b.disque.poser(&format!(r"{GEREE}\Mindset.exe"), "", 0);
    b
}

fn demarrer_banc(b: &mut Banc, demarrage: bool, racine: &str) -> Depart {
    demarrer(demarrage, &env(racine), &Noms::mindcare(), &mut b.systeme, &mut b.disque)
}

fn lancement(d: Depart) -> Lancement {
    match d {
        Depart::Lancer(l) => *l,
        autre => panic!("l'appli devait démarrer : {autre:?}"),
    }
}

/// Démarre une copie gérée lancée cachée, puis sa vie (surveillances commencées).
fn vie_geree(b: &mut Banc) -> Vie {
    let l = lancement(demarrer_banc(b, true, GEREE));
    let theme = theme_choisi(l.gere, l.suite.theme.as_deref(), "neon");
    let mut v = Vie::nouvelle(Reglages::depuis(&l, &theme, MIN));
    v.commencer(&mut b.monde());
    v
}

fn s(secondes: f64) -> Duration {
    Duration::from_secs_f64(secondes)
}

fn quitter_demande() -> String {
    format!(r"{DONNEES_GEREE}\quitter.demande")
}

// ---------------------------------------------------------------------------------------------------------------
// Partie 6 : -Demarrage
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn partie6_arguments_demarrage() {
    // lanceur.cs:36 : -Demarrage sans tenir compte des majuscules, où qu'il soit ; plan 4.3 : aussi /Demarrage
    for oui in [&["-Demarrage"][..], &["/Demarrage"], &["-DEMARRAGE"], &["/demarrage"], &["x", "-demarrage", "y"]] {
        assert!(demarrage_demande(oui), "{oui:?}");
    }
    for non in
        [&[][..], &["Demarrage"], &["--Demarrage"], &["-Demarrage2"], &["-Dem"], &["-Démarrage"], &["\\Demarrage"]]
    {
        assert!(!demarrage_demande(non), "{non:?}");
    }
}

#[test]
fn partie6_lancee_cachee_fenetre_creee_cachee_sans_ouvrir_mindset() {
    let mut b = banc_gere();
    let l = lancement(demarrer_banc(&mut b, true, GEREE));
    // contrat 6 : la fenêtre est créée cachée, le verrou pris et les demandes surveillées
    assert!(!l.fenetre_montree());
    assert!(l.gere);
    assert_eq!(b.systeme.verrous_crees, [r"Local\Mindcare-Instance"]);
    assert_eq!(b.systeme.signal_cree.as_deref(), Some(r"Local\Mindcare-Afficher"));
    // Mindcare.ps1:35 : jamais la page de Mindset avec -Demarrage
    assert!(b.systeme.lances.is_empty());
    assert!(!b.disque.a(PAGE));
    assert!(b.disque.dans_journal("lancée cachée"));
}

#[test]
fn partie6_lancement_normal_fenetre_montree() {
    // (dossier de travail : pas géré, jamais bloqué)
    let mut b = Banc::default();
    b.disque.dossiers.push(r"c:\code\mindcare\.git".into());
    let l = lancement(demarrer_banc(&mut b, false, r"C:\code\mindcare\Mindcare"));
    assert!(l.fenetre_montree());
    assert!(!b.disque.dans_journal("lancée cachée"));
}

// ---------------------------------------------------------------------------------------------------------------
// Démarrage : copie bloquée, copie gérée lancée à la main (mindset.ps1, Mindcare.ps1:34-35)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn demarrage_copie_non_geree_bloquee() {
    let mut b = Banc::default();
    b.systeme.emplacement = Some(format!("\"{MINDSET}\""));
    b.disque.poser(&format!(r"{MINDSET}\Mindset.exe"), "", 0);
    // suite.json gère un autre dossier
    b.disque.poser(SUITE, "{}", 0);
    b.disque.suite = suite_geree(GEREE, "neon");
    let d = demarrer_banc(&mut b, false, r"C:\Users\Essai\Downloads\Mindcare");
    match d {
        Depart::Bloquee { mindset_exe } => {
            assert_eq!(mindset_exe, Some(PathBuf::from(format!(r"{MINDSET}\Mindset.exe"))))
        }
        autre => panic!("{autre:?}"),
    }
    // rien d'autre : ni verrou, ni signal, ni dossier de données, ni programme lancé
    assert!(b.systeme.verrous_crees.is_empty());
    assert!(b.systeme.signal_cree.is_none());
    assert!(b.disque.donnees_ouvertes.is_none());
    assert!(b.systeme.lances.is_empty());
    // même avec -Demarrage
    let mut b2 = Banc::default();
    assert!(matches!(demarrer_banc(&mut b2, true, r"C:\Users\Essai\Downloads\Mindcare"), Depart::Bloquee { .. }));
}

#[test]
fn demarrage_bloquee_sans_mindset_installe() {
    // mindset.ps1:80 : InstallLocation sans Mindset.exe dedans ne compte pas (« Télécharger Mindset »)
    let mut b = Banc::default();
    b.systeme.emplacement = Some(MINDSET.to_string());
    match demarrer_banc(&mut b, false, r"C:\Temp\Mindcare") {
        Depart::Bloquee { mindset_exe } => assert_eq!(mindset_exe, None),
        autre => panic!("{autre:?}"),
    }
    // un chemin sans lettre de disque non plus (Get-CheminNormal)
    let mut b = Banc::default();
    b.systeme.emplacement = Some(r"\\serveur\Mindset".to_string());
    b.disque.poser(r"\\serveur\Mindset\Mindset.exe", "", 0);
    assert!(matches!(demarrer_banc(&mut b, false, r"C:\Temp\Mindcare"), Depart::Bloquee { mindset_exe: None }));
}

#[test]
fn demarrage_dossier_de_travail_jamais_bloque() {
    let mut b = Banc::default();
    // mindset.ps1:106 : .git dans le dossier ou juste au-dessus
    b.disque.dossiers.push(r"c:\code\mindcare\.git".into());
    let l = lancement(demarrer_banc(&mut b, false, r"C:\code\mindcare\Mindcare"));
    assert!(l.travail && !l.gere);
    // donnees.ps1:20 : un dossier à part, pour ne jamais mélanger avec la vraie
    assert_eq!(l.dossier_donnees, PathBuf::from(format!(r"{LOCAL}\Mindset\Mindcare (travail)")));
}

#[test]
fn demarrage_copie_geree_lancee_a_la_main_s_ouvre_dans_mindset() {
    let mut b = banc_gere();
    let d = demarrer_banc(&mut b, false, GEREE);
    assert!(matches!(d, Depart::OuverteDansMindset), "{d:?}");
    // mindset.ps1:94-95 : la page dans afficher-page.txt, puis Mindset.exe de HKLM, dans son dossier
    assert_eq!(b.disque.texte(PAGE).as_deref(), Some("appli:mindcare"));
    assert_eq!(
        b.systeme.lances,
        [Lance {
            exe: PathBuf::from(format!(r"{MINDSET}\Mindset.exe")),
            arguments: vec![],
            dossier: Some(PathBuf::from(MINDSET))
        }]
    );
    // et rien d'autre : pas de verrou pris
    assert!(b.systeme.verrous_crees.is_empty());
}

#[test]
fn demarrage_mindset_jamais_lance_par_suite_json() {
    // suite.json ne sert qu'à comparer : Mindset absent de HKLM, l'appli continue dans sa propre fenêtre
    let mut b = banc_gere();
    b.systeme.emplacement = None;
    let l = lancement(demarrer_banc(&mut b, false, GEREE));
    assert!(l.fenetre_montree());
    assert!(b.systeme.lances.is_empty());
    assert!(!b.disque.a(PAGE));
}

#[test]
fn partie8_dossier_des_donnees_d_une_copie_geree() {
    let mut b = banc_gere();
    let l = lancement(demarrer_banc(&mut b, true, GEREE));
    // donnees.ps1:15 : <X>:\Mindset\Programmes\Mindcare donne <X>:\Mindset\Donnees\Mindcare
    assert_eq!(l.dossier_donnees, PathBuf::from(DONNEES_GEREE));
    assert_eq!(b.disque.donnees_ouvertes.as_deref(), Some(Path::new(DONNEES_GEREE)));
    assert!(b.disque.dossiers.contains(&DONNEES_GEREE.to_lowercase()));
    // Program Files\Mindset\Mindcare donne %LOCALAPPDATA%\Mindset\Mindcare
    let mut b = banc_gere();
    let pf = r"C:\Program Files\Mindset\Mindcare";
    b.disque.suite = suite_geree(pf, "neon");
    let l = lancement(demarrer_banc(&mut b, true, pf));
    assert_eq!(l.dossier_donnees, PathBuf::from(format!(r"{LOCAL}\Mindset\Mindcare")));
    // mindset.ps1:109 : une copie gérée n'est jamais un dossier de travail, même avec un .git à côté
    let mut b = banc_gere();
    b.disque.dossiers.push(r"e:\mindset\programmes\.git".into());
    let l = lancement(demarrer_banc(&mut b, true, GEREE));
    assert!(l.gere && !l.travail);
}

#[test]
fn demarrage_suite_json_pris_copie_dans_les_programmes_de_mindset() {
    // mindset.ps1:61-72 : suite.json encore pris, une copie dans Program Files\Mindset est gérée
    let mut b = banc_gere();
    b.disque.suite = Suite { existe: true, pris: true, ..Suite::default() };
    let l = lancement(demarrer_banc(&mut b, true, r"C:\Program Files\Mindset\Mindcare"));
    assert!(l.gere);
    assert!(l.suite.pris);
    assert_eq!(b.disque.lectures_suite, [ESSAIS_SUITE]);
}

#[test]
fn demarrage_vieille_demande_de_fermeture_effacee() {
    // systeme.ps1:14 : une vieille demande refermerait l'appli tout de suite
    let mut b = banc_gere();
    b.disque.poser(&quitter_demande(), "1234", 0);
    let mut v = vie_geree(&mut b);
    assert!(!b.disque.a(&quitter_demande()));
    assert_eq!(v.avancer(s(10.0), &mut b.monde()), None);
}

#[test]
fn theme_de_la_suite_ou_des_reglages() {
    // themes.ps1:104
    assert_eq!(theme_choisi(true, Some("rubis"), "neon"), "rubis");
    assert_eq!(theme_choisi(true, None, "arcade"), "arcade");
    assert_eq!(theme_choisi(false, Some("rubis"), "arcade"), "arcade");
}

// ---------------------------------------------------------------------------------------------------------------
// Partie 3 : le verrou
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn partie3_verrou_tenu_lancement_normal_leve_le_signal_et_quitte() {
    let mut b = Banc::default();
    b.disque.dossiers.push(r"c:\code\mindcare\.git".into());
    b.systeme.verrou_deja_tenu = true;
    let d = demarrer_banc(&mut b, false, r"C:\code\mindcare\Mindcare");
    assert!(matches!(d, Depart::DejaOuverte), "{d:?}");
    // Mindcare.ps1:50 : la fenêtre de l'autre copie est ramenée, sans attendre
    assert_eq!(b.systeme.signaux_leves, [r"Local\Mindcare-Afficher"]);
    assert!(b.systeme.attentes.is_empty());
    // et rien d'autre : ni signal à soi, ni dossier de données, ni journal
    assert!(b.systeme.signal_cree.is_none());
    assert!(b.disque.donnees_ouvertes.is_none());
}

#[test]
fn partie3_verrou_tenu_avec_demarrage_attend_5_s_puis_quitte_sans_rien_deposer() {
    let mut b = banc_gere();
    b.systeme.verrou_deja_tenu = true;
    b.systeme.attente = Some(AttenteVerrou::Tenu);
    let d = demarrer_banc(&mut b, true, GEREE);
    assert!(matches!(d, Depart::DejaOuverte), "{d:?}");
    assert_eq!(b.systeme.attentes, [ATTENTE_VERROU]);
    assert_eq!(ATTENTE_VERROU, Duration::from_secs(5));
    // décision du 29/10 : un lancement caché ne dépose rien
    assert!(b.systeme.signaux_leves.is_empty());
    assert!(b.disque.donnees_ouvertes.is_none());
    assert!(b.systeme.signal_cree.is_none());
}

#[test]
fn partie3_verrou_rendu_pendant_l_attente_apres_une_mise_a_jour() {
    for attente in [AttenteVerrou::Libre, AttenteVerrou::Abandonne] {
        let mut b = banc_gere();
        b.systeme.verrou_deja_tenu = true;
        b.systeme.attente = Some(attente);
        let l = lancement(demarrer_banc(&mut b, true, GEREE));
        assert!(!l.fenetre_montree());
        assert_eq!(b.systeme.signal_cree.as_deref(), Some(r"Local\Mindcare-Afficher"));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Partie 5 : quitter.demande, signal d'affichage, etat.json, suite.json
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn partie5_quitter_demande_lu_efface_et_quitte_en_moins_de_6_s() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    assert_eq!(v.avancer(s(3.0), &mut b.monde()), None);
    // Mindset dépose sa demande à 3,2 s
    b.disque.poser(&quitter_demande(), "4321", 10);
    let mut t = 3.0;
    let mut fin = None;
    while fin.is_none() && t < 20.0 {
        t += 0.1;
        fin = v.avancer(s(t), &mut b.monde());
    }
    assert_eq!(fin, Some(Fin::Quitter));
    // systeme.ps1:109 : lue au tour suivant, toutes les 500 ms (bien moins que les 6 s de Mindset)
    assert!(t - 3.2 <= 0.5 + 1e-9, "quitté à {t} s");
    assert!(!b.disque.a(&quitter_demande()), "la demande est effacée");
    assert!(b.disque.dans_journal("Fermeture demandée par Mindset"));
    // une fois finie, plus rien ne tourne
    let tours = v.tours();
    assert_eq!(v.avancer(s(60.0), &mut b.monde()), None);
    assert_eq!(v.tours(), tours);
}

#[test]
fn partie5_quitter_demande_meme_si_elle_ne_s_efface_pas() {
    // systeme.ps1:115 : `try { Delete } catch {}`, l'appli quitte quand même
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    b.disque.poser(&quitter_demande(), "1", 10);
    b.disque.effacer_echoue = true;
    assert_eq!(v.avancer(s(0.5), &mut b.monde()), Some(Fin::Quitter));
}

#[test]
fn partie5_signal_d_affichage_copie_geree_ouvre_mindset() {
    let mut b = banc_gere();
    b.systeme.emplacement = None;
    let l = lancement(demarrer_banc(&mut b, false, GEREE)); // (Mindset introuvable au démarrage)
    let mut v = Vie::nouvelle(Reglages::depuis(&l, "neon", MIN));
    v.commencer(&mut b.monde());
    b.fenetre.visible = true;
    // Mindset est installé entre-temps ; un second lancement lève le signal
    b.systeme.emplacement = Some(MINDSET.to_string());
    b.systeme.signal_leve = true;
    v.avancer(s(0.5), &mut b.monde());
    // systeme.ps1:55 : gérée, elle ne s'ouvre que dans Mindset
    assert_eq!(b.disque.texte(PAGE).as_deref(), Some("appli:mindcare"));
    assert_eq!(b.systeme.lances.len(), 1);
    assert_eq!(b.fenetre.activee, 0);
}

#[test]
fn partie5_signal_d_affichage_sans_mindset_montre_la_fenetre() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    b.systeme.emplacement = None;
    b.fenetre.reduite = true;
    b.systeme.signal_leve = true;
    v.avancer(s(0.5), &mut b.monde());
    // systeme.ps1:57-60 : montrée, rendue, activée
    assert_eq!(b.fenetre.activee, 1);
    assert!(b.fenetre.visible && !b.fenetre.reduite);
    // le signal est à remise automatique : une seule fois
    v.avancer(s(5.0), &mut b.monde());
    assert_eq!(b.fenetre.activee, 1);
}

#[test]
fn partie5_signal_d_affichage_copie_de_travail() {
    let mut b = Banc::default();
    b.disque.dossiers.push(r"c:\code\mindcare\.git".into());
    b.systeme.emplacement = Some(MINDSET.to_string());
    b.disque.poser(&format!(r"{MINDSET}\Mindset.exe"), "", 0);
    let l = lancement(demarrer_banc(&mut b, false, r"C:\code\mindcare\Mindcare"));
    let mut v = Vie::nouvelle(Reglages::depuis(&l, "neon", MIN));
    v.commencer(&mut b.monde());
    b.systeme.signal_leve = true;
    v.avancer(s(0.5), &mut b.monde());
    // pas gérée : sa propre fenêtre, jamais Mindset
    assert_eq!(b.fenetre.activee, 1);
    assert!(b.systeme.lances.is_empty());
}

#[test]
fn partie5_etat_json_au_depart_puis_toutes_les_minutes() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    // systeme.ps1:124 : dès le démarrage
    assert_eq!(b.disque.etats_ecrits, 1);
    v.avancer(s(59.5), &mut b.monde());
    assert_eq!(b.disque.etats_ecrits, 1);
    // systeme.ps1:121 : au 120e tour, 60 s
    v.avancer(s(60.0), &mut b.monde());
    assert_eq!(b.disque.etats_ecrits, 2);
    v.avancer(s(180.0), &mut b.monde());
    assert_eq!(b.disque.etats_ecrits, 4);
}

#[test]
fn partie5_copie_non_geree_n_ecrit_ni_ne_lit_rien_pour_mindset() {
    let mut b = Banc::default();
    b.disque.dossiers.push(r"c:\code\mindcare\.git".into());
    b.disque.poser(SUITE, "{}", 0);
    b.disque.suite = suite_geree(GEREE, "neon");
    let l = lancement(demarrer_banc(&mut b, false, r"C:\code\mindcare\Mindcare"));
    let mut v = Vie::nouvelle(Reglages::depuis(&l, "neon", MIN));
    v.commencer(&mut b.monde());
    assert_eq!(v.avancer(s(1.0), &mut b.monde()), None);
    b.disque.poser(SUITE, "{}", 50);
    b.disque.suite = suite_geree(GEREE, "rubis");
    assert_eq!(v.avancer(s(120.0), &mut b.monde()), None);
    // systeme.ps1:71 et :87
    assert_eq!(b.disque.etats_ecrits, 0);
    assert_eq!(b.disque.lectures_suite.len(), 1, "seulement au démarrage");
}

#[test]
fn partie5_suite_json_regarde_sur_sa_date_toutes_les_2_5_s() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    let au_depart = b.disque.lectures_suite.len();
    // date inchangée : jamais relu
    assert_eq!(v.avancer(s(30.0), &mut b.monde()), None);
    assert_eq!(b.disque.lectures_suite.len(), au_depart);
    // nouvelle date, même thème (autre casse) : relu une fois (un seul essai), pas de relance
    b.disque.poser(SUITE, "{}", 100);
    b.disque.suite = suite_geree(GEREE, "NEON");
    assert_eq!(v.avancer(s(33.0), &mut b.monde()), None);
    assert_eq!(&b.disque.lectures_suite[au_depart..], [1]);
    assert_eq!(v.avancer(s(40.0), &mut b.monde()), None);
    assert_eq!(b.disque.lectures_suite.len(), au_depart + 1);
}

#[test]
fn partie5_suite_json_nouveau_theme_relance_cachee() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    assert_eq!(v.avancer(s(1.0), &mut b.monde()), None); // (1er tour : la date est notée)
    // Mindset change le thème à 1,2 s ; regardé aux tours 6, 11... (3 s, 5,5 s)
    b.disque.poser(SUITE, "{}", 100);
    b.disque.suite = suite_geree(GEREE, "rubis");
    assert_eq!(v.avancer(s(2.9), &mut b.monde()), None);
    assert_eq!(v.avancer(s(3.0), &mut b.monde()), Some(Fin::Relancer { cachee: true }));
    assert!(b.disque.dans_journal("Thème de la suite changé : rubis, relance"));
    // la relance : verrou et signal rendus, Mindset.exe de son dossier avec -Demarrage
    v.terminer(Fin::Relancer { cachee: true }, &mut b.systeme, &mut b.disque);
    assert!(b.systeme.verrou_libere && b.systeme.signal_ferme);
    assert_eq!(
        b.systeme.lances,
        [Lance {
            exe: PathBuf::from(format!(r"{GEREE}\Mindset.exe")),
            arguments: vec!["-Demarrage".into()],
            dossier: None
        }]
    );
}

#[test]
fn partie5_suite_json_nouveau_theme_relance_visible() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    b.fenetre.visible = true;
    v.avancer(s(1.0), &mut b.monde());
    b.disque.poser(SUITE, "{}", 100);
    b.disque.suite = suite_geree(GEREE, "pack:mon-pack");
    let fin = v.avancer(s(3.0), &mut b.monde());
    assert_eq!(fin, Some(Fin::Relancer { cachee: false }));
    v.terminer(fin.unwrap(), &mut b.systeme, &mut b.disque);
    assert_eq!(b.systeme.lances[0].arguments, Vec::<String>::new());
}

#[test]
fn partie5_suite_json_illisible_relu_au_tour_suivant() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    v.avancer(s(1.0), &mut b.monde());
    // systeme.ps1:94 : en train d'être remplacé
    b.disque.poser(SUITE, "{", 100);
    b.disque.suite = Suite { existe: true, lisible: false, ..Suite::default() };
    assert_eq!(v.avancer(s(3.0), &mut b.monde()), None);
    // au tour suivant (5,5 s) il est lisible, avec un nouveau thème : la date n'avait pas été notée
    b.disque.suite = suite_geree(GEREE, "arcade");
    assert_eq!(v.avancer(s(5.5), &mut b.monde()), Some(Fin::Relancer { cachee: true }));
}

#[test]
fn partie5_suite_json_pris_au_demarrage_lu_au_premier_tour() {
    // systeme.ps1:91 : pris au lancement, son thème n'a pas pu être lu ; il l'est au premier tour où il est libre
    let mut b = banc_gere();
    b.disque.suite = Suite { existe: true, pris: true, ..Suite::default() };
    let l = lancement(demarrer_banc(&mut b, true, r"C:\Program Files\Mindset\Mindcare"));
    let mut v = Vie::nouvelle(Reglages::depuis(&l, "neon", MIN));
    v.commencer(&mut b.monde());
    b.disque.suite = suite_geree(r"C:\Program Files\Mindset\Mindcare", "terminal");
    assert_eq!(v.avancer(s(0.5), &mut b.monde()), Some(Fin::Relancer { cachee: true }));
}

#[test]
fn relance_sans_programme_notee_au_journal() {
    let mut b = banc_gere();
    b.disque.fichiers.remove(&faux::cle(Path::new(&format!(r"{GEREE}\Mindset.exe"))));
    let v = vie_geree(&mut b);
    v.terminer(Fin::Relancer { cachee: true }, &mut b.systeme, &mut b.disque);
    // Mindcare.ps1:23 : le relais Mindcare.exe, s'il existe ; sinon rien n'est lancé
    assert!(b.systeme.lances.is_empty());
    assert!(b.disque.dans_journal("Relance : programme introuvable"));
    let mut b = banc_gere();
    b.disque.fichiers.remove(&faux::cle(Path::new(&format!(r"{GEREE}\Mindset.exe"))));
    b.disque.poser(&format!(r"{GEREE}\Mindcare.exe"), "", 0);
    let v = vie_geree(&mut b);
    v.terminer(Fin::Relancer { cachee: false }, &mut b.systeme, &mut b.disque);
    assert_eq!(b.systeme.lances[0].exe, PathBuf::from(format!(r"{GEREE}\Mindcare.exe")));
    // Quitter : rien n'est relancé
    let mut b = banc_gere();
    let v = vie_geree(&mut b);
    v.terminer(Fin::Quitter, &mut b.systeme, &mut b.disque);
    assert!(b.systeme.lances.is_empty() && !b.systeme.verrou_libere);
}

// ---------------------------------------------------------------------------------------------------------------
// Partie 7 : l'accrochage, la croix, la veille de la page
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn partie7_accrochee_taille_minimale_0_et_se_montre_sans_s_activer() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    v.avancer(s(2.0), &mut b.monde());
    assert!(!b.fenetre.visible, "cachée tant qu'elle n'est pas accrochée");
    assert!(b.fenetre.tailles_min.is_empty());
    // Mindset l'accroche (WS_CHILD, SetParent)
    b.fenetre.accrochee = true;
    v.avancer(s(2.5), &mut b.monde());
    assert!(v.dans_mindset());
    assert_eq!(b.fenetre.tailles_min, [None]);
    assert_eq!(b.fenetre.montree_sans_activer, 1);
    assert_eq!(b.fenetre.activee, 0, "jamais au premier plan");
    assert!(b.disque.dans_journal("Affichée dans Mindset"));
    // rien ne change tant qu'elle reste accrochée
    v.avancer(s(20.0), &mut b.monde());
    assert_eq!(b.fenetre.tailles_min.len(), 1);
    assert_eq!(b.fenetre.montree_sans_activer, 1);
    // décrochée (Mindset quitte) : la taille minimale est rendue
    b.fenetre.accrochee = false;
    v.avancer(s(20.5), &mut b.monde());
    assert_eq!(b.fenetre.tailles_min, [None, Some(MIN)]);
    assert!(b.disque.dans_journal("Plus affichée dans Mindset"));
}

#[test]
fn partie7_accrochee_deja_visible_ne_se_remontre_pas() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    b.fenetre.visible = true;
    b.fenetre.accrochee = true;
    v.avancer(s(0.5), &mut b.monde());
    assert_eq!(b.fenetre.montree_sans_activer, 0);
    assert_eq!(b.fenetre.tailles_min, [None]);
}

#[test]
fn partie7_sans_handle_rien_n_est_touche() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    b.fenetre.sans_handle = true;
    b.fenetre.accrochee = true;
    v.avancer(s(2.0), &mut b.monde());
    assert!(!v.dans_mindset());
    assert!(b.fenetre.tailles_min.is_empty());
}

#[test]
fn partie7_accrochee_le_signal_ne_touche_pas_a_la_fenetre() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    b.fenetre.accrochee = true;
    v.avancer(s(0.5), &mut b.monde());
    b.systeme.signal_leve = true;
    v.avancer(s(1.0), &mut b.monde());
    // systeme.ps1:54 : affichée dans Mindset, ni Mindset relancé, ni fenêtre activée
    assert_eq!(b.fenetre.activee, 0);
    assert!(b.systeme.lances.is_empty());
    assert!(!b.systeme.signal_leve, "le signal a bien été lu");
}

#[test]
fn partie7_croix_et_wm_close_quittent_une_seule_fois() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    v.avancer(s(1.0), &mut b.monde());
    assert_eq!(v.croix(&mut b.monde()), Some(Fin::Quitter));
    assert!(v.quitte());
    // evenements.ps1:10 et systeme.ps1:134 : une seule fois
    assert_eq!(v.croix(&mut b.monde()), None);
    assert_eq!(b.disque.journal.iter().filter(|l| *l == "Fermeture").count(), 1);
    // et la minuterie s'arrête : une demande déposée ensuite ne fait plus rien
    b.disque.poser(&quitter_demande(), "1", 10);
    assert_eq!(v.avancer(s(10.0), &mut b.monde()), None);
    assert!(b.disque.a(&quitter_demande()));
}

#[test]
fn partie7_veille_page_visible_seulement_si_tout_est_visible() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    // cachée : la page dort
    v.avancer(s(0.5), &mut b.monde());
    assert_eq!(b.fenetre.page, [false]);
    // accrochée dans un panneau visible d'un Mindset visible : elle se montre, la page se réveille
    b.fenetre.parents = vec![Niveau { visible: true, reduite: false }, Niveau { visible: true, reduite: false }];
    b.fenetre.accrochee = true;
    v.avancer(s(1.0), &mut b.monde());
    assert_eq!(b.fenetre.page, [false, true]);
    // rien de neuf : rien n'est redit
    v.avancer(s(5.0), &mut b.monde());
    assert_eq!(b.fenetre.page.len(), 2);
    // Mindset réduit : la page dort
    b.fenetre.parents[1].reduite = true;
    v.avancer(s(5.5), &mut b.monde());
    assert_eq!(b.fenetre.page, [false, true, false]);
    // Mindset rendu, mais l'onglet (le panneau) caché : elle dort encore
    b.fenetre.parents[1].reduite = false;
    b.fenetre.parents[0].visible = false;
    v.avancer(s(6.0), &mut b.monde());
    assert_eq!(b.fenetre.page.len(), 3);
    // l'onglet revient
    b.fenetre.parents[0].visible = true;
    v.avancer(s(6.5), &mut b.monde());
    assert_eq!(b.fenetre.page, [false, true, false, true]);
    // la fenêtre elle-même réduite (hors de Mindset)
    b.fenetre.reduite = true;
    v.avancer(s(7.0), &mut b.monde());
    assert_eq!(b.fenetre.page.last(), Some(&false));
}

#[test]
fn partie7_echelle_dans_mindset() {
    // systeme.ps1:42-48
    assert_eq!(echelle(true, 450.0, 640.0, MIN), 0.5);
    assert_eq!(echelle(true, 900.0, 320.0, MIN), 0.5);
    assert_eq!(echelle(true, 1800.0, 1280.0, MIN), 1.0);
    assert_eq!(echelle(false, 450.0, 320.0, MIN), 1.0);
    assert_eq!(echelle(true, 0.0, 320.0, MIN), 1.0);
}

#[test]
fn tours_toutes_les_500_ms() {
    let mut b = banc_gere();
    let mut v = vie_geree(&mut b);
    assert_eq!(TOUR, Duration::from_millis(500));
    v.avancer(s(0.49), &mut b.monde());
    assert_eq!(v.tours(), 0);
    v.avancer(s(10.0), &mut b.monde());
    assert_eq!(v.tours(), 20);
}
