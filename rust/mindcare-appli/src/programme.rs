//! Le programme Mindset.exe de Mindcare (étape 4.4 du plan « Mindcare en Rust ») : tout est branché ici.
//!
//! 1. L'identité `JordanJacquot.Mindcare` (barre des tâches), puis les arguments (`-Demarrage`) et le démarrage de
//!    mindcare-vie : copie bloquée, copie gérée lancée à la main (Mindset l'ouvre dans son onglet), une seule copie
//!    à la fois (verrou `Local\Mindcare-Instance`, signal `Local\Mindcare-Afficher`), dossier de données.
//! 2. Les données (journal, réglages, etat.json) et le thème (suite.json pour une copie gérée, sinon le réglage).
//! 3. Tauri : la page et ses deux commandes, le protocole `pack` (les images du thème), la fenêtre « principale »
//!    créée une seule fois, cachée et sans prendre la main (même lancée sans `-Demarrage` : elle se montre ensuite),
//!    titre exact « Mindcare », jamais recréée. Les réglages de WebView2 que Tauri ne transmet pas sont coupés.
//! 4. Les surveillances : un pas de 100 ms sur le fil de la fenêtre, qui fait avancer la vie de l'appli (ses tours de
//!    500 ms : accrochage et taille minimale, signal d'affichage, veille de la page, quitter.demande, suite.json,
//!    etat.json). La croix et `WM_CLOSE` (le bouton « Fermer » de Mindset) quittent.
//! 5. La fin : la relance si le thème a changé (comme aujourd'hui), puis `std::process::exit` (aucun fil ne garde
//!    le processus en vie sans fenêtre, contrat-appli partie 3).
//!
//! Tout l'état vit sur le fil de la fenêtre (celui de Tauri) : la vie, les données, le verrou (qui appartient au fil
//! qui l'a pris) et la fenêtre. La minuterie, sur un autre fil, ne fait que demander un tour à ce fil-là.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mindcare_donnees::{Donnees, Journal};
use mindcare_vie::{
    Depart, DisqueReel, Environnement, Fin, Lancement, Monde, Reglages, Taille, Vie, demarrage_demande, demarrer,
    theme_choisi,
};
use serde_json::{Value, json};
use suite_commun::themes;
use tauri::{App, AppHandle, Manager, Window, WindowEvent};

use crate::commandes::etat_page;
use crate::fenetre::{LABEL, TAILLE_MIN, creer_principale};
use crate::windows::fenetre::FenetreTauri;
use crate::windows::systeme::SystemeWindows;
use crate::windows::{heure, identite, webview2};
use crate::{Page, VERSION, pack};

/// Le pas de la minuterie : les tours de la vie de l'appli (toutes les 500 ms) tombent à ce pas près.
const PAS: Duration = Duration::from_millis(100);

/// Tout ce que l'appli garde pendant qu'elle tourne, sur le fil de la fenêtre.
struct Etat {
    vie: Vie,
    systeme: SystemeWindows,
    disque: DisqueReel,
    fenetre: Option<FenetreTauri>,
    /// Le début des surveillances
    debut: Instant,
    /// La fin décidée (quitter ou relancer), une seule fois
    fin: Option<Fin>,
    journal: Journal,
    /// Le dossier de données (construction d'essai : la demande de capture)
    #[cfg(feature = "essai")]
    dossier: PathBuf,
}

thread_local! {
    static ETAT: RefCell<Option<Etat>> = const { RefCell::new(None) };
}

/// Le dossier du programme (celui de Mindset.exe et Mindcare.exe).
fn dossier_programme() -> Option<String> {
    std::env::current_exe().ok()?.parent()?.to_str().map(str::to_string)
}

/// Ce que Windows donne au démarrage (Mindcare.ps1 lit les mêmes variables).
#[cfg(not(feature = "essai"))]
fn environnement() -> Environnement {
    let v = |n: &str| std::env::var(n).ok().filter(|x| !x.is_empty());
    Environnement {
        racine: dossier_programme(),
        localappdata: v("LOCALAPPDATA").unwrap_or_default(),
        program_files: v("ProgramFiles"),
        program_w6432: v("ProgramW6432"),
        version: VERSION.to_string(),
        donnees_essai: None,
    }
}

/// Construction d'essai : tout vient du dossier d'essai (crate::essai), rien des vraies données.
#[cfg(feature = "essai")]
fn environnement_essai(e: &crate::essai::Essai) -> Environnement {
    Environnement {
        racine: dossier_programme(),
        localappdata: e.local.clone(),
        program_files: None,
        program_w6432: None,
        version: VERSION.to_string(),
        donnees_essai: Some(e.donnees.to_string_lossy().into_owned()),
    }
}

/// Le dossier des données de la suite, où va `WebView2-Tauri\Mindcare` : pour une copie gérée, celui qui contient
/// le dossier de Mindcare (`%LOCALAPPDATA%\Mindset`, `X:\Mindset\Donnees`) ; pour une copie de travail, son propre
/// dossier (jamais mélangé avec la vraie).
fn donnees_suite(l: &Lancement) -> PathBuf {
    match l.dossier_donnees.parent() {
        Some(p) if l.gere => p.to_path_buf(),
        _ => l.dossier_donnees.clone(),
    }
}

/// Un texte venu de la page, coupé pour le journal.
fn court(t: &str) -> String {
    t.chars().take(120).collect()
}

/// Une copie que Mindset ne gère pas (Mindcare.ps1:34, `Show-BlocageMindset`, mindset.ps1:121-161) : un message,
/// avec « Ouvrir Mindset » s'il est installé, sinon l'adresse de sa page.
#[cfg(not(feature = "essai"))]
fn bloquee(mindset_exe: Option<&Path>, systeme: &mut SystemeWindows) {
    use crate::windows::message;
    use mindcare_vie::Systeme;
    let texte = "Mindcare s'utilise avec Mindset.\n\nInstalle Mindset, le portail de la suite : il installe Mindcare, \
                 le garde à jour et l'affiche dans sa fenêtre.";
    match mindset_exe {
        Some(exe) => {
            if message::demander(&format!("{texte}\n\nOuvrir Mindset maintenant ?"), crate::NOM_APPLI) {
                let _ = systeme.lancer(exe, &[], exe.parent());
            }
        }
        None => message::informer(
            &format!("{texte}\n\nTélécharge Mindset sur sa page :\n{}", mindcare_vie::mindset::MINDSET_PAGE),
            crate::NOM_APPLI,
        ),
    }
}

/// Construction d'essai : aucune boîte (le script d'essai le voit au code de sortie 3).
#[cfg(feature = "essai")]
fn bloquee(_mindset_exe: Option<&Path>, _systeme: &mut SystemeWindows) {
    std::process::exit(3);
}

/// Le programme : ne revient jamais (`std::process::exit`).
pub fn lancer() -> ! {
    let _ = identite::poser();
    let demarrage = demarrage_demande(std::env::args_os().skip(1));
    #[cfg(feature = "essai")]
    let (env, noms) = match crate::essai::lire() {
        Ok(e) => (environnement_essai(&e), e.noms()),
        // (sans son dossier d'essai, la construction d'essai ne fait rien du tout)
        Err(_) => std::process::exit(4),
    };
    #[cfg(not(feature = "essai"))]
    let (env, noms) = (environnement(), mindcare_vie::Noms::mindcare());

    // Le disque de la vie de l'appli : etat.json passe par les données, ouvertes une fois le dossier connu
    let horloge = heure::horloge();
    let donnees: Rc<RefCell<Option<Donnees>>> = Rc::new(RefCell::new(None));
    let pour_etat = Rc::clone(&donnees);
    let mut disque = DisqueReel::nouveau(
        horloge.clone(),
        Box::new(move || {
            if let Some(d) = pour_etat.borrow_mut().as_mut() {
                // (le catalogue des enquêtes arrive à l'étape 5 : en attendant, la carte de départ)
                d.ecrire_etat(None);
            }
        }),
    );
    let mut systeme = SystemeWindows::nouveau();
    let l = match demarrer(demarrage, &env, &noms, &mut systeme, &mut disque) {
        Depart::Lancer(l) => *l,
        Depart::Bloquee { mindset_exe } => {
            bloquee(mindset_exe.as_deref(), &mut systeme);
            std::process::exit(0)
        }
        Depart::OuverteDansMindset | Depart::DejaOuverte => std::process::exit(0),
    };

    // Les données et le thème (themes.ps1:104 : celui de la suite pour une copie gérée, sinon le réglage)
    let d = Donnees::ouvrir(&l.dossier_donnees, horloge, l.gere);
    let journal = d.journal().clone();
    let reglage = match d.reglages.lire("Theme", json!("neon")) {
        Value::String(t) => t,
        _ => "neon".to_string(),
    };
    let choix = theme_choisi(l.gere, l.suite.theme.as_deref(), &reglage);
    let suite = donnees_suite(&l);
    let mut packs = vec![l.dossier_donnees.join("packs")];
    if l.gere {
        packs.push(suite.join("Mindset").join("packs"));
    }
    let theme = themes::charger(Some(&choix), &packs);
    for r in &theme.raisons {
        journal.ecrire(&format!("Thème : {r}"));
    }
    *donnees.borrow_mut() = Some(d);

    let pour_page = journal.clone();
    let page = Page::new(etat_page(VERSION, l.gere, &[]), move |t| pour_page.ecrire(t));
    // (les couleurs du thème et d'un pack viennent de l'habillage, comme Get-PackVue de vue.ps1)
    page.habiller(crate::pont::Habillage::depuis(&theme, crate::pont::soir(heure::maintenant().heure)));
    let dossier_mindset = l.dossier_mindset.clone();
    let pour_receveur = journal.clone();
    page.recevoir(move |m| match m {
        crate::pont::MessagePage::Menu => ETAT.with(|e| {
            let Ok(mut e) = e.try_borrow_mut() else { return };
            let Some(etat) = e.as_mut() else { return };
            let ligne = if etat.vie.dans_mindset() {
                crate::pont::demander_menu(&dossier_mindset, &mut etat.systeme, &mut etat.disque)
            } else {
                // (hors de Mindset : rien à ramener, la demande est seulement notée)
                crate::pont::ligne_journal(&crate::pont::MessagePage::Menu)
            };
            etat.journal.ecrire(&ligne);
        }),
        // (étape 5.3 : vers les pages)
        autre => pour_receveur.ecrire(&crate::pont::ligne_journal(&autre)),
    });
    let dossier_pack = theme.pack.as_ref().map(|p| p.dossier.clone());
    let taille_min = Taille { largeur: TAILLE_MIN.0, hauteur: TAILLE_MIN.1 };
    ETAT.with(|e| {
        *e.borrow_mut() = Some(Etat {
            vie: Vie::nouvelle(Reglages::depuis(&l, &choix, taille_min)),
            systeme,
            disque,
            fenetre: None,
            debut: Instant::now(),
            fin: None,
            journal: journal.clone(),
            #[cfg(feature = "essai")]
            dossier: l.dossier_donnees.clone(),
        })
    });

    let pour_pack = journal.clone();
    let pour_fenetre = journal.clone();
    let montree = l.fenetre_montree();
    let donnees_appli = l.dossier_donnees.clone();
    let constructeur = crate::preparer(tauri::Builder::default(), page)
        .register_uri_scheme_protocol("pack", move |_, demande| {
            let chemin = demande.uri().path();
            let (reponse, refus) = pack::repondre(dossier_pack.as_deref(), chemin);
            if let Some(r) = refus {
                pour_pack.ecrire(&format!("Thème : fichier refusé ({}) : {}", r.raison(), court(chemin)));
            }
            reponse
        })
        .setup(move |app| {
            ouvrir_fenetre(app, &suite, &donnees_appli, montree, &pour_fenetre);
            Ok(())
        })
        .on_window_event(evenement_fenetre);
    match constructeur.build(crate::contexte()) {
        Ok(app) => {
            let _ = app.run_return(|_, _| {});
        }
        Err(e) => journal.ecrire(&format!("Écran : Tauri n'a pas pu démarrer ({e})")),
    }
    finir()
}

/// Crée la fenêtre principale (une seule fois), coupe les réglages de WebView2, la montre si elle n'est pas lancée
/// cachée, puis commence les surveillances.
fn ouvrir_fenetre(app: &App, suite: &Path, donnees: &Path, montree: bool, journal: &Journal) {
    // (une seule fenêtre, jamais recréée)
    if app.get_webview_window(LABEL).is_some() {
        return;
    }
    let pour_page = journal.clone();
    let (f, choix) = match creer_principale(app, suite, donnees, std::sync::Arc::new(move |t| pour_page.ecrire(t))) {
        Ok(x) => x,
        Err(e) => {
            journal.ecrire(&format!("Écran : la fenêtre n'a pas pu être créée ({e})"));
            app.handle().exit(1);
            return;
        }
    };
    journal.ecrire(&format!("Écran : dossier WebView2 {}", choix.dossier.display()));
    if let Some(r) = &choix.raison {
        journal.ecrire(&format!("Écran : dossier de la suite non pris, repli ({r})"));
    }
    let h = f.hwnd().map(|h| h.0 as usize).unwrap_or(0);
    // (contrat-appli, partie 7 : la taille minimale revient une fois libérée, même quand Mindset change la taille)
    if !crate::windows::fenetre::garder_taille_minimale(h) {
        journal.ecrire("Écran : taille minimale non gardée (sous-classe refusée)");
    }
    let pour_reglages = journal.clone();
    if let Err(e) = webview2::couper_reglages(&f, move |t| pour_reglages.ecrire(t)) {
        journal.ecrire(&format!("Écran : réglages de WebView2 non posés ({e})"));
    }
    // Lancée à la main (copie de travail, ou gérée quand Mindset est introuvable) : montrée au premier plan
    // (Mindcare.ps1:79). Lancée cachée : elle ne se montre qu'une fois accrochée dans Mindset.
    if montree {
        let _ = f.show();
        let _ = f.set_focus();
    }
    let pour_fenetre = journal.clone();
    let fenetre = FenetreTauri::nouvelle(f, h, move |t| pour_fenetre.ecrire(t));
    ETAT.with(|e| {
        let mut e = e.borrow_mut();
        let Some(etat) = e.as_mut() else { return };
        etat.fenetre = Some(fenetre);
        etat.debut = Instant::now();
        let Etat { vie, systeme, disque, fenetre, .. } = etat;
        if let Some(fenetre) = fenetre.as_mut() {
            // systeme.ps1:123-124 : la carte de Mindset écrite tout de suite
            vie.commencer(&mut Monde { systeme, disque, fenetre });
        }
    });
    minuterie(app.handle().clone());
}

/// La minuterie : toutes les 100 ms, un tour demandé au fil de la fenêtre. Elle s'arrête quand Tauri ne répond
/// plus (la fin) ; de toute façon, `std::process::exit` arrête tous les fils.
fn minuterie(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(PAS);
            let a = app.clone();
            if app.run_on_main_thread(move || tour(&a)).is_err() {
                break;
            }
        }
    });
}

/// Un pas des surveillances, sur le fil de la fenêtre : la vie de l'appli fait les tours dus.
fn tour(app: &AppHandle) {
    let fin = ETAT.with(|e| {
        // (déjà occupé : au prochain pas)
        let mut e = e.try_borrow_mut().ok()?;
        let etat = e.as_mut()?;
        if etat.fin.is_some() {
            return None;
        }
        let maintenant = etat.debut.elapsed();
        let Etat { vie, systeme, disque, fenetre, .. } = &mut *etat;
        let fenetre = fenetre.as_mut()?;
        let fin = vie.avancer(maintenant, &mut Monde { systeme, disque, fenetre });
        #[cfg(feature = "essai")]
        capture_demandee(etat);
        etat.fin = fin;
        fin
    });
    if fin.is_some() {
        app.exit(0);
    }
}

/// La croix, ou `WM_CLOSE` venu de Mindset (evenements.ps1:10) : la fenêtre n'est pas détruite par Tauri, l'appli
/// quitte (sa fenêtre part avec elle).
fn evenement_fenetre(w: &Window, ev: &WindowEvent) {
    match ev {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            croix(w.app_handle());
        }
        // Détruite quand même (elle n'est jamais recréée) : l'appli quitte
        WindowEvent::Destroyed => w.app_handle().exit(0),
        _ => {}
    }
}

/// La croix, traitée sur le fil de la fenêtre dès qu'il est libre.
fn croix(app: &AppHandle) {
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let r = ETAT.with(|e| {
            let Ok(mut e) = e.try_borrow_mut() else { return Err(()) };
            let Some(etat) = e.as_mut() else { return Ok(None) };
            let Etat { vie, systeme, disque, fenetre, .. } = &mut *etat;
            let Some(fenetre) = fenetre.as_mut() else { return Ok(None) };
            let fin = vie.croix(&mut Monde { systeme, disque, fenetre });
            if fin.is_some() {
                etat.fin = fin;
            }
            Ok(fin)
        });
        match r {
            Ok(Some(_)) => a.exit(0),
            Ok(None) => {}
            // (un tour en cours : réessayé juste après)
            Err(()) => croix(&a),
        }
    });
}

/// Construction d'essai : une capture demandée (`capture.demande`) écrit l'image de la fenêtre dans `capture.png`.
#[cfg(feature = "essai")]
fn capture_demandee(etat: &Etat) {
    use crate::essai::{CAPTURE, CAPTURE_DEMANDE};
    let demande = etat.dossier.join(CAPTURE_DEMANDE);
    if !demande.is_file() {
        return;
    }
    let _ = std::fs::remove_file(&demande);
    let Some(f) = etat.fenetre.as_ref() else { return };
    let png = crate::windows::capture::capturer(f.hwnd())
        .and_then(|(l, h, px)| crate::png::encoder(l, h, &px).map(|p| (l, h, p)));
    match png {
        Some((l, h, p)) => match std::fs::write(etat.dossier.join(CAPTURE), p) {
            Ok(()) => etat.journal.ecrire(&format!("Essai : capture de {l} x {h}")),
            Err(e) => etat.journal.ecrire(&format!("Essai : capture non écrite ({e})")),
        },
        None => etat.journal.ecrire("Essai : capture impossible"),
    }
}

/// La fin (Mindcare.ps1:90-97) : la relance si le thème a changé (verrou et signal rendus d'abord), puis
/// `std::process::exit`.
fn finir() -> ! {
    ETAT.with(|e| {
        if let Ok(mut e) = e.try_borrow_mut()
            && let Some(etat) = e.as_mut()
        {
            if etat.fin.is_none() {
                etat.journal.ecrire("Fermeture : la fenêtre a disparu");
            }
            let fin = etat.fin.unwrap_or(Fin::Quitter);
            etat.vie.terminer(fin, &mut etat.systeme, &mut etat.disque);
        }
    });
    std::process::exit(0)
}
