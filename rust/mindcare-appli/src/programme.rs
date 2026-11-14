//! Le programme Mindset.exe de Mindcare (étape 4.4 du plan « Mindcare en Rust ») : tout est branché ici.
//!
//! 1. L'identité `JordanJacquot.Mindcare` (barre des tâches), puis les arguments (`-Demarrage`) et le démarrage de
//!    mindcare-vie : copie bloquée, copie gérée lancée à la main (Mindset l'ouvre dans son onglet), une seule copie
//!    à la fois (verrou `Local\Mindcare-Instance`, signal `Local\Mindcare-Afficher`), dossier de données.
//! 2. Les données (journal, réglages, etat.json) et le thème (suite.json pour une copie gérée, sinon le réglage).
//! 3. Tauri : la page et ses deux commandes, le protocole `pack` (les images du thème), la fenêtre « principale »
//!    créée une seule fois, cachée et sans prendre la main (même lancée sans `-Demarrage` : elle se montre ensuite),
//!    titre exact « Mindcare », jamais recréée. Les réglages de WebView2 que Tauri ne transmet pas sont coupés.
//!    Les pages (étape 5.3, src\effets.rs) : le catalogue des enquêtes (enquetes.json, à côté du programme, comme
//!    Mindcare.ps1), l'état des pages envoyé au kit, chaque message du kit appliqué aux pages et leurs effets exécutés
//!    (enquête gardée, rapport sur le Bureau, presse-papiers, retour au menu), et ce que Windows a noté, lu dès le
//!    démarrage sur un fil à part.
//! 4. Les surveillances : un pas de 100 ms sur le fil de la fenêtre, qui fait avancer la vie de l'appli (ses tours de
//!    500 ms : accrochage et taille minimale, signal d'affichage, veille de la page, quitter.demande, suite.json,
//!    etat.json, avec la vraie situation des enquêtes), et la lecture de Windows attendue par une enquête. L'échelle
//!    de la page dans un panneau de Mindset plus petit que la fenêtre (`Update-EchelleMindset`) suit chaque nouvelle
//!    taille (`Resized`), et chaque pas. La croix et `WM_CLOSE` (le bouton « Fermer » de Mindset) quittent.
//! 5. La fin : la relance si le thème a changé (comme aujourd'hui), puis `std::process::exit` (aucun fil ne garde
//!    le processus en vie sans fenêtre, contrat-appli partie 3), sans rien lâcher de l'état du fil de la fenêtre
//!    ([`quitter_sans_rien_lacher`]).
//!
//! Tout l'état vit sur le fil de la fenêtre (celui de Tauri) : la vie, les données, le verrou (qui appartient au fil
//! qui l'a pris) et la fenêtre. La minuterie, sur un autre fil, ne fait que demander un tour à ce fil-là.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::LocalKey;
use std::time::{Duration, Instant};

use mindcare_donnees::{Donnees, Journal};
use mindcare_vie::{
    Depart, DisqueReel, Environnement, Fin, Lancement, Monde, Reglages, Taille, Vie, demarrage_demande, demarrer,
    theme_choisi,
};
use serde_json::{Value, json};
use suite_commun::themes;
use tauri::{App, AppHandle, Manager, Window, WindowEvent};

use crate::effets::{ATTENTE_WINDOWS_MAX, Branchement, Exterieur, LectureWindows, Monde as MondePages};
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
    /// Les données (aussi écrites par la vie de l'appli : etat.json)
    donnees: Rc<RefCell<Option<Donnees>>>,
    /// Les pages, le catalogue, la lecture de Windows
    pages: Branchement,
    /// Le dossier de Mindset (retour à sa console)
    dossier_mindset: PathBuf,
    /// Tauri, une fois la fenêtre créée (la page partagée)
    app: Option<AppHandle>,
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

/// Le catalogue des enquêtes : enquetes.json à côté du programme (`$EnquetesFichier`, enquete.ps1:10).
fn charger_catalogue() -> Result<Arc<mindcare_moteur::Catalogue>, String> {
    let dossier = dossier_programme().ok_or("dossier du programme introuvable")?;
    crate::effets::charger_catalogue(&Path::new(&dossier).join("enquetes.json"))
}

/// Le vrai Bureau et le vrai presse-papiers (l'exe livré seulement).
#[cfg(not(feature = "essai"))]
struct ExterieurWindows;

#[cfg(not(feature = "essai"))]
impl Exterieur for ExterieurWindows {
    fn bureau(&self) -> Result<PathBuf, String> {
        crate::windows::presse_papiers::bureau()
    }
    fn copier(&mut self, texte: &str) -> Result<(), String> {
        crate::windows::presse_papiers::copier(texte)
    }
    fn ouvrir_windows_update(&mut self) -> Result<(), String> {
        crate::windows::reparer::ouvrir_windows_update()
    }
}

/// Construction d'essai : un faux Bureau (`<local d'essai>\Bureau`) et un faux presse-papiers
/// (`<local d'essai>\presse-papiers.txt`), jamais ceux de la personne.
#[cfg(feature = "essai")]
struct ExterieurEssai {
    local: PathBuf,
}

#[cfg(feature = "essai")]
impl Exterieur for ExterieurEssai {
    fn bureau(&self) -> Result<PathBuf, String> {
        let b = self.local.join("Bureau");
        std::fs::create_dir_all(&b).map_err(|e| e.to_string())?;
        Ok(b)
    }
    fn copier(&mut self, texte: &str) -> Result<(), String> {
        std::fs::write(self.local.join("presse-papiers.txt"), texte).map_err(|e| e.to_string())
    }
    /// (jamais les vrais Paramètres : une ligne dans `<local d'essai>\windows-update.txt`)
    fn ouvrir_windows_update(&mut self) -> Result<(), String> {
        std::fs::write(self.local.join("windows-update.txt"), "ms-settings:windowsupdate").map_err(|e| e.to_string())
    }
}

/// Construction d'essai : « Le faire pour moi » ne lance jamais rien. Chaque geste attend un peu (pour voir la jauge),
/// puis réussit ; il rate si le fichier `<local d'essai>\reparer-rate.txt` existe.
#[cfg(feature = "essai")]
struct ExecuteurEssai {
    local: PathBuf,
}

#[cfg(feature = "essai")]
impl crate::reparer::Executeur for ExecuteurEssai {
    fn lancer(&self, _: &crate::reparer::Geste) -> Result<i32, String> {
        std::thread::sleep(Duration::from_millis(1500));
        if self.local.join("reparer-rate.txt").exists() { Ok(1) } else { Ok(0) }
    }
}

/// L'assistant et la dictée de Mindcare (l'exe livré) : le vrai llama-server et le vrai whisper (sans fenêtre, arrêtés
/// avec Mindcare), le micro choisi dans Mindset, les vrais téléchargements (WinHTTP, avec le proxy du PC), dans le
/// dossier de données de Mindcare.
#[cfg(not(feature = "essai"))]
fn outils(donnees: &Path, suite_json: &Path, journal: &mindcare_donnees::Journal) -> crate::effets::Outils {
    use mindcare_telechargement::{Installation, Paquet, Transport, windows};
    let j = journal.clone();
    let transport = windows::WinHttp::nouveau().map(|t| Arc::new(t) as Arc<dyn Transport>);
    crate::effets::Outils::nouveaux(
        mindcare_assistant::assistant(donnees, Arc::new(move |t: &str| j.ecrire(t))),
        Installation::nouvelle(Paquet::assistant(donnees)),
        mindcare_dictee::dictee_de_mindcare(donnees, Some(suite_json.to_path_buf())),
        Installation::nouvelle(Paquet::dictee(donnees)),
        transport,
        Arc::new(windows::PlaceWindows),
    )
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
    let (env, noms, essai) = match crate::essai::lire() {
        Ok(e) => (environnement_essai(&e), e.noms(), e),
        // (sans son dossier d'essai, la construction d'essai ne fait rien du tout)
        Err(_) => std::process::exit(4),
    };
    #[cfg(not(feature = "essai"))]
    let (env, noms) = (environnement(), mindcare_vie::Noms::mindcare());

    // Le catalogue des enquêtes (illisible : la page du problème, et la carte de Mindset sans lui)
    let catalogue = charger_catalogue();
    let cat_etat = catalogue.as_ref().ok().cloned();

    // Le disque de la vie de l'appli : etat.json passe par les données, ouvertes une fois le dossier connu, avec le
    // catalogue (la carte de Mindcare dans Mindset : enquête en cours, dernière enquête)
    let horloge = heure::horloge();
    let donnees: Rc<RefCell<Option<Donnees>>> = Rc::new(RefCell::new(None));
    let pour_etat = Rc::clone(&donnees);
    let mut disque = DisqueReel::nouveau(
        horloge.clone(),
        Box::new(move || {
            if let Ok(mut d) = pour_etat.try_borrow_mut()
                && let Some(d) = d.as_mut()
            {
                d.ecrire_etat(cat_etat.as_deref());
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
    let mut d = Donnees::ouvrir(&l.dossier_donnees, horloge, l.gere);
    let journal = d.journal().clone();
    // (Read-Historique : les enquêtes gardées, pour l'accueil et la carte de Mindset)
    d.lire_historique();
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
    // Les pages : ce que Windows a noté est lu dès maintenant, sur un fil à part (Start-LectureJournaux)
    #[cfg(not(feature = "essai"))]
    let (source, exterieur): (_, Box<dyn Exterieur>) =
        (mindcare_windows::windows::WindowsReel::default(), Box::new(ExterieurWindows));
    #[cfg(feature = "essai")]
    let (source, exterieur): (_, Box<dyn Exterieur>) = {
        let local = PathBuf::from(&essai.local);
        let temp = local.join("Temp");
        let _ = std::fs::create_dir_all(&temp);
        // (le PC inventé de l'essai : le vrai matériel n'est jamais lu)
        let reel = mindcare_windows::windows::WindowsReel { dossier_temp: temp };
        (crate::essai::SourceEssai { reel }, Box::new(ExterieurEssai { local }))
    };
    let mut lecture = LectureWindows::nouvelle(
        Arc::new(source),
        Arc::new(mindcare_windows::windows::HorlogeWindows),
        ATTENTE_WINDOWS_MAX,
    );
    lecture.lancer();
    let mut pages = Branchement::nouveau(VERSION, theme.theme_ecran(), catalogue, &d, lecture, exterieur);
    // L'assistant et la dictée (étape 6.4) : rien n'est lancé avant qu'on écrive ou qu'on appuie sur Parler
    #[cfg(not(feature = "essai"))]
    pages.brancher(outils(&l.dossier_donnees, &l.fichier_suite, &journal));
    #[cfg(feature = "essai")]
    pages.brancher(crate::essai::outils(&l.dossier_donnees, &l.fichier_suite, Path::new(&essai.local), &journal));
    // « Le faire pour moi » : rien n'est lancé avant le clic de la personne (la construction d'essai ne lance rien)
    #[cfg(not(feature = "essai"))]
    pages.brancher_executeur(Arc::new(crate::windows::reparer::ExecuteurWindows));
    #[cfg(feature = "essai")]
    pages.brancher_executeur(Arc::new(ExecuteurEssai { local: PathBuf::from(&essai.local) }));
    *donnees.borrow_mut() = Some(d);

    let pour_page = journal.clone();
    let page = Page::new(pages.etat_kit(), move |t| pour_page.ecrire(t));
    // (les couleurs du thème et d'un pack viennent de l'habillage, comme Get-PackVue de vue.ps1)
    page.habiller(crate::pont::Habillage::depuis(&theme, crate::pont::soir(heure::maintenant().heure)));
    let pour_receveur = journal.clone();
    page.recevoir(move |m| {
        ETAT.with(|e| {
            let Ok(mut e) = e.try_borrow_mut() else {
                pour_receveur.ecrire(&format!("Écran : message laissé, appli occupée ({})", ligne_courte(&m)));
                return;
            };
            if let Some(etat) = e.as_mut() {
                etat.recevoir(&m);
            }
        })
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
            donnees: Rc::clone(&donnees),
            pages,
            dossier_mindset: l.dossier_mindset.clone(),
            app: None,
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
        Ok(app) => boucle_protegee(&journal, move || {
            let _ = app.run_return(|_, _| {});
        }),
        Err(e) => journal.ecrire(&format!("Écran : Tauri n'a pas pu démarrer ({e})")),
    }
    // (dans tous les cas, même après un plantage sur le fil de la fenêtre : llama-server, whisper et le micro arrêtés)
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
        etat.app = Some(app.handle().clone());
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
        if fin.is_none() {
            // (SizeChanged, systeme.ps1:125 : une taille manquée par `Resized`, la fenêtre occupée, est vue ici)
            vie.ajuster_echelle(&mut Monde { systeme, disque, fenetre });
            etat.tour_pages();
        }
        #[cfg(feature = "essai")]
        if fin.is_none() {
            actions_demandees(etat);
        }
        #[cfg(feature = "essai")]
        capture_demandee(etat);
        etat.fin = fin;
        fin
    });
    if fin.is_some() {
        app.exit(0);
    }
}

/// Un message de la page, en court pour le journal.
fn ligne_courte(m: &crate::pont::MessagePage) -> String {
    court(&crate::pont::ligne_journal(m))
}

impl Etat {
    /// Ce dont les pages ont besoin pour un message ou un tour : la page partagée de Tauri, les données, et le retour
    /// à la console de Mindset (seulement dans Mindset ; sinon la demande est seulement notée).
    fn avec_pages(&mut self, f: impl FnOnce(&mut Branchement, &mut MondePages)) {
        let Some(app) = self.app.clone() else { return };
        let page = app.state::<Page>();
        let donnees = Rc::clone(&self.donnees);
        let Ok(mut d) = donnees.try_borrow_mut() else { return };
        let Some(d) = d.as_mut() else { return };
        let Etat { vie, systeme, disque, pages, dossier_mindset, .. } = self;
        let mut menu = || {
            if vie.dans_mindset() {
                crate::pont::demander_menu(dossier_mindset, systeme, disque)
            } else {
                crate::pont::ligne_journal(&crate::pont::MessagePage::Menu)
            }
        };
        f(pages, &mut MondePages { page: page.inner(), donnees: d, menu: &mut menu });
    }

    /// Un message du kit, donné aux pages.
    fn recevoir(&mut self, m: &crate::pont::MessagePage) {
        self.avec_pages(|pages, monde| pages.recevoir(m, monde));
    }

    /// Le pas de la minuterie pour les pages (la lecture de Windows attendue).
    fn tour_pages(&mut self) {
        self.avec_pages(|pages, monde| pages.tour(Instant::now(), monde));
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
        // Mindset change la taille du panneau (`SizeChanged`, systeme.ps1:125) : l'échelle suit tout de suite
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => echelle_tout_de_suite(),
        _ => {}
    }
}

/// `Update-EchelleMindset` sur une nouvelle taille. La fenêtre occupée (un tour en cours) : au prochain pas de la
/// minuterie.
fn echelle_tout_de_suite() {
    ETAT.with(|e| {
        let Ok(mut e) = e.try_borrow_mut() else { return };
        let Some(etat) = e.as_mut() else { return };
        if etat.fin.is_some() {
            return;
        }
        let Etat { vie, systeme, disque, fenetre, .. } = &mut *etat;
        if let Some(fenetre) = fenetre.as_mut() {
            vie.ajuster_echelle(&mut Monde { systeme, disque, fenetre });
        }
    });
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

/// Construction d'essai : les messages demandés (`actions.demande`, un JSON par ligne), traités comme s'ils venaient
/// de la page (`{ "type": "action", ... }`, lus par le pont comme ceux du kit), ou envoyés au kit tels quels
/// (`{ "kit": "aller", ... }`). Le fichier est effacé une fois lu (le script d'essai l'écrit d'un coup, par un
/// renommage).
#[cfg(feature = "essai")]
fn actions_demandees(etat: &mut Etat) {
    let demande = etat.dossier.join(crate::essai::ACTIONS_DEMANDE);
    let Ok(texte) = std::fs::read_to_string(&demande) else { return };
    let _ = std::fs::remove_file(&demande);
    for ligne in texte.trim_start_matches('\u{feff}').lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Ok(v) = serde_json::from_str::<Value>(ligne) else {
            etat.journal.ecrire("Essai : demande illisible");
            continue;
        };
        if v.get("kit").is_some() {
            if let Some(app) = etat.app.clone() {
                app.state::<Page>().ordre(v);
            }
            continue;
        }
        match crate::pont::lire_message(&v) {
            Ok(m) => etat.recevoir(&m),
            Err(r) => etat.journal.ecrire(&format!("Essai : demande refusée ({})", r.raison())),
        }
    }
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

/// La boucle de la fenêtre (`run_return`), protégée : tao attrape une panique d'un rappel de la fenêtre (une page, un
/// effet, une commande) et la relance à la sortie de sa boucle. Sans cette garde, elle remonterait hors de
/// `lancer()` et sauterait la fin propre ([`finir`]) : llama-server et whisper resteraient en mémoire tant que le
/// processus vit. Ici, elle est notée au journal et la fin propre suit.
pub fn boucle_protegee(journal: &Journal, boucle: impl FnOnce()) {
    if let Err(p) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(boucle)) {
        let raison = p
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| p.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "raison inconnue".to_string());
        journal.ecrire(&format!("Fermeture : plantage ({raison})"));
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
            // (Stop-Contenu : llama-server, whisper et le micro arrêtés, les téléchargements aussi ; si Mindcare est
            // tuée, les Job Objects arrêtent llama-server et whisper avec elle)
            etat.pages.arreter();
            let fin = etat.fin.unwrap_or(Fin::Quitter);
            etat.vie.terminer(fin, &mut etat.systeme, &mut etat.disque);
        }
    });
    quitter_sans_rien_lacher(&ETAT, 0)
}

/// Quitte le processus (`std::process::exit`) sans lâcher ce que garde `etat`, le `thread_local` du fil qui quitte.
///
/// `std::process::exit`, c'est `ExitProcess` : Windows arrête d'abord tous les autres fils, puis Rust lâche les
/// `thread_local` du fil qui quitte. Lâcher là les outils de l'étape 6.4 bloquait la fin pour toujours : la dictée
/// attend son fil, déjà arrêté par Windows (la jointure panique), puis `WinHttpCloseHandle` attend un fil réseau de
/// WinHTTP lui aussi arrêté (vu sur la pile d'une copie restée accrochée après un changement de thème : la nouvelle
/// copie était lancée, l'ancienne ne quittait jamais). Ici, rien n'est lâché : tout a été arrêté avant
/// (`Branchement::arreter`, `Vie::terminer`) et Windows ferme lui-même les poignées du processus qui se termine.
pub fn quitter_sans_rien_lacher<T: 'static>(etat: &'static LocalKey<RefCell<Option<T>>>, code: i32) -> ! {
    let reste = etat.with(|e| e.try_borrow_mut().ok().and_then(|mut e| e.take()));
    std::mem::forget(reste);
    std::process::exit(code)
}
