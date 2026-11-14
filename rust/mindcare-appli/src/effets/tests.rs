//! Les effets des pages exécutés, sans fenêtre et sans rien du vrai PC : un faux Bureau et un faux presse-papiers
//! (dans le dossier d'essai), un faux Windows rapide ou lent, une fausse page qui garde ce qu'elle reçoit.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use mindcare_donnees::rapport::reparateur;
use mindcare_donnees::{Donnees, Moment as MomentDonnees, horloge_figee};
use mindcare_moteur::Catalogue;
use mindcare_moteur::ps::{champ, texte, vrai};
use mindcare_windows::{
    Barrette, Batterie, Disque, DisquePhysique, Evenement, Filtre, Horloge, InfosPc, Mesure, Moment, Partition, Place,
    SourceWindows,
};
use serde_json::{Value, json};

use super::*;
use crate::pages::{Vue, ecrans, ids};

/// « Le faire pour moi » exécuté, avec un faux exécuteur (jamais rien de lancé).
mod reparer;

// ------------------------------------------------------------------------------------------------ le catalogue

fn fichier_catalogue() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("Mindcare")
        .join("fichiers")
        .join("enquetes.json")
}

fn cat() -> Arc<Catalogue> {
    static CAT: OnceLock<Arc<Catalogue>> = OnceLock::new();
    CAT.get_or_init(|| charger_catalogue(&fichier_catalogue()).expect("enquetes.json")).clone()
}

// ------------------------------------------------------------------------------------------------ un faux Windows

/// Un faux Windows : rien de grave noté, une barrette, un PC connu. Lent : sa lecture du journal attend d'être
/// libérée (au plus 20 secondes).
struct FauxWindows {
    lent: (Mutex<bool>, Condvar),
    lectures: AtomicUsize,
    /// Le matériel du PC (rien : pas lu, comme les faux Windows des autres tests)
    materiel: Option<mindcare_windows::PcBrut>,
}

impl FauxWindows {
    fn avec(lent: bool, materiel: Option<mindcare_windows::PcBrut>) -> Arc<FauxWindows> {
        Arc::new(FauxWindows { lent: (Mutex::new(lent), Condvar::new()), lectures: AtomicUsize::new(0), materiel })
    }

    fn liberer(&self) {
        let (m, c) = &self.lent;
        *m.lock().unwrap_or_else(|e| e.into_inner()) = false;
        c.notify_all();
    }
}

fn maintenant_windows() -> Moment {
    Moment::nouveau(2026, 11, 3, 10, 15, 0, 0).unwrap()
}

impl SourceWindows for FauxWindows {
    fn plus_ancien(&self) -> Mesure<Option<Moment>> {
        self.lectures.fetch_add(1, Ordering::SeqCst);
        let (m, c) = &self.lent;
        let g = m.lock().unwrap_or_else(|e| e.into_inner());
        let _ = c.wait_timeout_while(g, Duration::from_secs(20), |lent| *lent);
        Ok(Some(maintenant_windows().plus_jours(-400)))
    }
    fn evenements(&self, _: &Filtre) -> Mesure<Vec<Evenement>> {
        Ok(Vec::new())
    }
    fn disques(&self) -> Mesure<Vec<Disque>> {
        Ok(Vec::new())
    }
    fn partitions(&self) -> Mesure<Vec<Partition>> {
        Ok(Vec::new())
    }
    fn disques_physiques(&self) -> Mesure<Vec<DisquePhysique>> {
        Ok(Vec::new())
    }
    fn lecteur_systeme(&self) -> String {
        "C:".into()
    }
    fn place(&self, _: &str) -> Mesure<Place> {
        Ok(Place { libre: 200 << 30, taille: 500 << 30 })
    }
    fn batterie(&self) -> Mesure<Option<Batterie>> {
        Ok(None)
    }
    fn demarrage(&self) -> Mesure<Moment> {
        Ok(maintenant_windows().plus_jours(-1))
    }
    fn barrettes(&self) -> Mesure<Vec<Barrette>> {
        Ok(vec![Barrette {
            capacite: Some(8 << 30),
            fabricant: "Kingston".into(),
            reference: "KF3200".into(),
            emplacement: "DIMM1".into(),
        }])
    }
    fn infos_pc(&self) -> Mesure<InfosPc> {
        Ok(InfosPc { fabricant: "Dell".into(), modele: "XPS 13".into(), systeme: "Windows 11 Famille".into() })
    }
    fn materiel(&self) -> Mesure<mindcare_windows::PcBrut> {
        self.materiel.clone().ok_or_else(|| "matériel non lu".to_string())
    }
}

/// L'horloge de la lecture de Windows : arrêtée, sauf quand un essai l'avance (en minutes).
#[derive(Default)]
struct FausseHorloge {
    minutes: AtomicI64,
}

impl FausseHorloge {
    fn avancer(&self, minutes: i64) {
        self.minutes.fetch_add(minutes, Ordering::SeqCst);
    }
}

impl Horloge for FausseHorloge {
    fn maintenant(&self) -> Moment {
        maintenant_windows().plus_minutes(self.minutes.load(Ordering::SeqCst))
    }
}

// ------------------------------------------------------------------------------------------------ page et extérieur

#[derive(Debug, Clone, PartialEq)]
enum Envoi {
    Etat(Value, Option<String>),
    Ordre(Value),
}

#[derive(Default)]
struct FaussePage {
    envois: RefCell<Vec<Envoi>>,
}

impl VersLaPage for FaussePage {
    fn etat(&self, etat: Value, ecran: Option<&str>) {
        self.envois.borrow_mut().push(Envoi::Etat(etat, ecran.map(str::to_string)));
    }
    fn ordre(&self, ordre: Value) {
        self.envois.borrow_mut().push(Envoi::Ordre(ordre));
    }
}

/// Le faux Bureau (un dossier d'essai, ou aucun) et le faux presse-papiers (ce qui y a été copié).
struct FauxExterieur {
    bureau: Option<PathBuf>,
    copies: Rc<RefCell<Vec<String>>>,
    copie_possible: bool,
}

impl Exterieur for FauxExterieur {
    fn bureau(&self) -> Result<PathBuf, String> {
        self.bureau.clone().ok_or_else(|| "Bureau introuvable".to_string())
    }
    fn copier(&mut self, texte: &str) -> Result<(), String> {
        if !self.copie_possible {
            return Err("presse-papiers pris".into());
        }
        self.copies.borrow_mut().push(texte.to_string());
        Ok(())
    }
    /// (jamais les vrais Paramètres : un fichier à côté du faux Bureau)
    fn ouvrir_windows_update(&mut self) -> Result<(), String> {
        let b = self.bureau.as_ref().and_then(|b| b.parent()).ok_or("pas de dossier d'essai")?;
        std::fs::write(b.join("windows-update.txt"), "ms-settings:windowsupdate").map_err(|e| e.to_string())
    }
}

// ------------------------------------------------------------------------------------------------ le banc

static DOSSIERS: AtomicUsize = AtomicUsize::new(0);

fn dossier_neuf() -> PathBuf {
    let n = DOSSIERS.fetch_add(1, Ordering::SeqCst);
    let racine = std::env::temp_dir().join(format!("mindcare-effets-essai-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&racine);
    racine
}

/// L'appli sans fenêtre : les pages branchées, leurs effets exécutés pour de vrai dans un dossier d'essai.
struct Banc {
    b: Branchement,
    donnees: Donnees,
    page: FaussePage,
    racine: PathBuf,
    copies: Rc<RefCell<Vec<String>>>,
    windows: Arc<FauxWindows>,
    menus: usize,
    /// L'horloge de la lecture de Windows (avancée par un essai)
    horloge: Arc<FausseHorloge>,
    /// Effacer le dossier d'essai à la fin (pas avant une relance)
    effacer: bool,
}

fn moment() -> MomentDonnees {
    MomentDonnees::lire("2026-11-03T10:15:00").unwrap()
}

impl Banc {
    fn nouveau(cat: Result<Arc<Catalogue>, String>, lent: bool) -> Banc {
        Banc::dans(dossier_neuf(), cat, lent, true)
    }

    /// Les pages sur ce dossier d'essai (une relance de l'appli : le même dossier).
    fn dans(racine: PathBuf, cat: Result<Arc<Catalogue>, String>, lent: bool, copie_possible: bool) -> Banc {
        Banc::avec_pc(racine, cat, lent, copie_possible, None)
    }

    /// Les pages sur ce dossier d'essai, avec ce matériel du PC (rien : pas lu).
    fn avec_pc(
        racine: PathBuf,
        cat: Result<Arc<Catalogue>, String>,
        lent: bool,
        copie_possible: bool,
        materiel: Option<mindcare_windows::PcBrut>,
    ) -> Banc {
        let bureau = racine.join("Bureau");
        std::fs::create_dir_all(&bureau).unwrap();
        let mut donnees = Donnees::ouvrir(&racine.join("Donnees"), horloge_figee(moment()), true);
        donnees.lire_historique();
        let windows = FauxWindows::avec(lent, materiel);
        let horloge = Arc::new(FausseHorloge::default());
        let mut lecture = LectureWindows::nouvelle(
            Arc::clone(&windows) as Arc<dyn SourceWindows>,
            Arc::clone(&horloge) as Arc<dyn Horloge>,
            ATTENTE_WINDOWS_MAX,
        );
        lecture.lancer();
        let copies = Rc::new(RefCell::new(Vec::new()));
        let exterieur = FauxExterieur { bureau: Some(bureau), copies: Rc::clone(&copies), copie_possible };
        let b = Branchement::nouveau("0.1.0", "neon", cat, &donnees, lecture, Box::new(exterieur));
        Banc { b, donnees, page: FaussePage::default(), racine, copies, windows, horloge, menus: 0, effacer: true }
    }

    fn envoyer(&mut self, m: MessagePage) {
        let menus = &mut self.menus;
        let mut menu = || {
            *menus += 1;
            "Retour au menu de Mindset demandé".to_string()
        };
        self.b.recevoir(&m, &mut Monde { page: &self.page, donnees: &mut self.donnees, menu: &mut menu });
    }

    fn action(&mut self, id: &str, valeur: Value, ecran: &str) {
        self.envoyer(MessagePage::Action { id: id.into(), valeur, ecran: ecran.into() });
    }

    fn tour(&mut self, t: Instant) {
        let mut menu = String::new;
        self.b.tour(t, &mut Monde { page: &self.page, donnees: &mut self.donnees, menu: &mut menu });
    }

    fn executer(&mut self, effets: Vec<Effet>) {
        let mut menu = String::new;
        self.b.executer(effets, &mut Monde { page: &self.page, donnees: &mut self.donnees, menu: &mut menu });
    }

    /// Le dernier état envoyé.
    fn dernier_etat(&self) -> Value {
        self.page
            .envois
            .borrow()
            .iter()
            .rev()
            .find_map(|e| if let Envoi::Etat(v, _) = e { Some(v.clone()) } else { None })
            .expect("un état envoyé")
    }

    /// Le dernier écran demandé.
    fn dernier_ecran(&self) -> Option<String> {
        self.page
            .envois
            .borrow()
            .iter()
            .rev()
            .find_map(|e| if let Envoi::Etat(_, Some(x)) = e { Some(x.clone()) } else { None })
    }

    fn messages(&self) -> Vec<(String, bool)> {
        self.page
            .envois
            .borrow()
            .iter()
            .filter_map(|e| match e {
                Envoi::Ordre(o) if o["kit"] == "message" => {
                    Some((o["texte"].as_str().unwrap_or_default().to_string(), o["bravo"] == json!(true)))
                }
                _ => None,
            })
            .collect()
    }

    fn journal(&self) -> String {
        std::fs::read_to_string(self.racine.join("Donnees").join("journal.txt")).unwrap_or_default()
    }

    fn vue(&self) -> Option<Vue> {
        self.b.pages.vue(self.b.cat.as_deref().unwrap())
    }

    /// L'Id de l'étape montrée.
    fn etape(&self) -> Option<String> {
        match self.vue()? {
            Vue::Etape(e) => e.etape.as_ref().map(|x| texte(champ(x, "Id"))),
            _ => None,
        }
    }

    /// Jusqu'à la question « Sur ce PC ? » : non au danger, « Je ne sais pas » au reste.
    fn jusqu_a_cepc(&mut self) {
        for _ in 0..20 {
            match self.etape().as_deref() {
                Some("cepc") => return,
                Some("danger") => self.action(ids::REP, json!("1"), ecrans::ENQUETE),
                Some(_) => self.action(ids::NSP, Value::Null, ecrans::ENQUETE),
                None => break,
            }
        }
        panic!("pas de question « Sur ce PC ? »");
    }

    /// Répond jusqu'à la réponse finale : non au danger, `cepc` à « Sur ce PC ? », sinon « Je ne sais pas ».
    fn jusqu_a_la_reponse(&mut self, cepc: &str) {
        for _ in 0..100 {
            match self.etape().as_deref() {
                Some("danger") => self.action(ids::REP, json!("1"), ecrans::ENQUETE),
                Some("cepc") => self.action(ids::REP, json!(cepc), ecrans::ENQUETE),
                Some(_) => self.action(ids::NSP, Value::Null, ecrans::ENQUETE),
                None => break,
            }
        }
        assert!(matches!(self.vue(), Some(Vue::Reponse(_))), "pas de réponse finale");
    }

    /// Les fichiers du faux Bureau.
    fn bureau(&self) -> Vec<PathBuf> {
        let mut l: Vec<PathBuf> =
            std::fs::read_dir(self.racine.join("Bureau")).unwrap().map(|e| e.unwrap().path()).collect();
        l.sort();
        l
    }
}

impl Drop for Banc {
    fn drop(&mut self) {
        self.windows.liberer();
        if self.effacer {
            let _ = std::fs::remove_dir_all(&self.racine);
        }
    }
}

/// Attend (au plus 5 s) que la lecture du faux Windows soit faite, pas de minuterie après pas de minuterie.
fn attendre_windows(b: &mut Banc) {
    let debut = Instant::now();
    while b.b.windows.attendue() && debut.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
        b.tour(Instant::now());
    }
    assert!(!b.b.windows.attendue(), "la lecture n'arrive pas");
}

// ------------------------------------------------------------------------------------------------ les essais

/// Une vraie enquête jouée jusqu'à la réponse, puis le rapport sur le (faux) Bureau et dans le (faux)
/// presse-papiers : l'enquête est gardée, la carte de Mindset dit la vraie situation.
#[test]
fn enquete_de_bout_en_bout() {
    let mut b = Banc::nouveau(Ok(cat()), false);
    b.envoyer(MessagePage::Pret);
    let e = b.dernier_etat();
    assert_eq!(e["app"]["nom"], "Mindcare");
    assert!(e["pages"].get(ecrans::RACONTE).is_some());
    assert!(b.dernier_ecran().is_none(), "pret : on reste où l'on est");
    assert!(b.journal().contains("Écran prêt"));

    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    assert_eq!(b.dernier_ecran().as_deref(), Some(ecrans::ENQUETE));
    assert!(b.journal().contains("Enquête : ecranbleu"));
    assert!(b.etape().is_some(), "la première question");
    b.jusqu_a_la_reponse("1");
    let e = b.dernier_etat();
    assert!(e["pages"].get(ecrans::REPARATEUR).is_some());
    // L'enquête finie est gardée, et la carte de Mindset la montre (avec le catalogue)
    let gardee = b.donnees.historique.liste.first().expect("enquête gardée").clone();
    assert!(vrai(champ(&gardee, "Fini")));
    assert_eq!(b.b.pages.historique, b.donnees.historique.liste);
    let carte = std::fs::read_to_string(b.racine.join("Donnees").join("etat.json")).unwrap();
    let carte: Value = serde_json::from_str(carte.trim_start_matches('\u{feff}')).unwrap();
    let lignes = carte["Lignes"].to_string();
    assert!(!lignes.contains("Aucun diagnostic"), "{carte}");
    let titre = texte(champ(cat().panne("ecranbleu").unwrap(), "Titre"));
    assert!(lignes.contains(&titre), "{carte}");

    // Pour le réparateur, puis le rapport sur le Bureau
    b.action(ids::REPARATEUR, Value::Null, ecrans::ENQUETE);
    assert_eq!(b.dernier_ecran().as_deref(), Some(ecrans::REPARATEUR));
    b.action(ids::BUREAU, Value::Null, ecrans::REPARATEUR);
    let fichiers = b.bureau();
    assert_eq!(fichiers.len(), 1, "{fichiers:?}");
    let nom = fichiers[0].file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(nom, "Mindcare, diagnostic du 3-11-2026 10h15.txt");
    let octets = std::fs::read(&fichiers[0]).unwrap();
    assert_eq!(&octets[..3], &[0xEF, 0xBB, 0xBF], "UTF-8 avec BOM");
    let rapport = String::from_utf8(octets[3..].to_vec()).unwrap();
    assert!(rapport.starts_with("Mindcare : rapport de l'enquête\r\n"));
    assert!(rapport.contains(&format!("Panne : {titre}")));
    assert!(rapport.contains("Fait avec Mindcare 0.1.0."));
    // (une enquête faite sur un autre appareil : rien du PC)
    assert!(!rapport.contains("Ordinateur :"));
    // (dit dans la page seulement, pas en bulle ; la page montre le texte du fichier, tel quel)
    assert!(!b.messages().iter().any(|(m, _)| *m == reparateur::enregistre(&nom)), "{:?}", b.messages());
    assert!(b.dernier_etat()["pages"][ecrans::REPARATEUR].to_string().contains(&reparateur::enregistre(&nom)));
    assert_eq!(b.b.pages.rapport.as_deref(), Some(rapport.as_str()));
    assert!(b.dernier_etat()["pages"][ecrans::REPARATEUR].to_string().contains(&format!("Panne : {titre}")));
    assert!(b.journal().contains("Rapport enregistré : "));
    // Copier : le même texte
    b.action(ids::COPIER, Value::Null, ecrans::REPARATEUR);
    assert_eq!(b.copies.borrow().as_slice(), std::slice::from_ref(&rapport));
    assert!(!b.messages().iter().any(|(m, _)| m == reparateur::COPIE), "{:?}", b.messages());
    assert!(b.dernier_etat()["pages"][ecrans::REPARATEUR].to_string().contains(reparateur::COPIE));
    assert!(b.b.pages.copie);
    // Un second rapport dans la même minute : le premier est gardé
    b.action(ids::BUREAU, Value::Null, ecrans::REPARATEUR);
    assert_eq!(b.bureau().len(), 2);
    // Retour : l'accueil, puis Retour depuis l'accueil (le menu de Mindset)
    b.action(ids::ACCUEIL, Value::Null, ecrans::REPARATEUR);
    assert_eq!(b.dernier_ecran().as_deref(), Some(ecrans::ACCUEIL));
    b.envoyer(MessagePage::Menu);
    assert_eq!(b.menus, 1);
    assert!(b.journal().contains("Retour au menu de Mindset demandé"));
}

/// Sur le PC en panne, Windows rapide : l'enquête avance avec ce qu'il a noté, et le rapport dit le PC.
#[test]
fn windows_rapide() {
    let mut b = Banc::nouveau(Ok(cat()), false);
    b.envoyer(MessagePage::Pret);
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    b.jusqu_a_cepc();
    b.action(ids::REP, json!("0"), ecrans::ENQUETE);
    attendre_windows(&mut b);
    let jour = b.b.pages.indices_du_jour.clone().expect("Windows lu");
    assert!(jour.get("placelibre").is_some(), "{jour:?}");
    assert!(!matches!(b.vue(), Some(Vue::Attente)));
    // (les barrettes sont lues sur leur propre fil : elles rejoignent l'état au pas de minuterie qui suit leur lecture)
    let debut = Instant::now();
    while b.b.pages.barrettes.is_empty() && debut.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
        b.tour(Instant::now());
    }
    assert_eq!(b.b.pages.barrettes, ["8 Go Kingston KF3200 (DIMM1)"]);
    assert!(b.journal().contains("Journaux de Windows : "));
    assert_eq!(b.windows.lectures.load(Ordering::SeqCst), 1, "une seule lecture");
    b.jusqu_a_la_reponse("0");
    // L'enquête finie garde ce que Windows a noté
    let gardee = b.donnees.historique.liste.first().unwrap().clone();
    assert!(champ(&gardee, "Indices").is_some_and(|i| i.get("placelibre").is_some()), "{gardee:?}");
    b.action(ids::COPIER, Value::Null, ecrans::REPARATEUR);
    let rapport = b.copies.borrow().first().cloned().expect("rapport copié");
    assert!(rapport.contains("Ordinateur : Dell XPS 13, Windows 11 Famille"), "{rapport}");
    assert!(rapport.contains("Mémoire : 8 Go Kingston KF3200 (DIMM1)"), "{rapport}");
}

/// Jusqu'à « Sur ce PC ? », oui, puis : Windows est relu (`relu`, une lecture de plus) ou pas (les indices gardés).
fn cepc_et_lecture(b: &mut Banc, relu: bool, quoi: &str) {
    let avant = b.windows.lectures.load(Ordering::SeqCst);
    b.jusqu_a_cepc();
    b.action(ids::REP, json!("0"), ecrans::ENQUETE);
    assert_eq!(b.b.windows.attendue(), relu, "{quoi} : lecture de Windows attendue ?");
    attendre_windows(b);
    assert_eq!(b.windows.lectures.load(Ordering::SeqCst), avant + usize::from(relu), "{quoi} : lectures de Windows");
    assert!(!matches!(b.vue(), Some(Vue::Attente) | None), "{quoi}");
}

/// Constat 21 (M13 de l'audit Rust) : Mindcare reste ouverte. Plus de 10 minutes après la dernière lecture de ce que
/// Windows a noté, toute enquête neuve la relance, quelle que soit la façon dont elle part : par mots, par
/// l'assistant sûr de lui, par « Pour être sûr », par « Reprendre l'enquête ». Avant 10 minutes : rien de relu.
#[test]
fn indices_relus_pour_toute_enquete_neuve() {
    use crate::pages::ids_aides;
    use mindcare_assistant::{Choix, Decision};
    let mut b = Banc::nouveau(Ok(cat()), false);
    b.envoyer(MessagePage::Pret);
    let journal = b.donnees.journal().clone();
    // Le matin : l'enquête par mots se sert de la lecture du lancement
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    b.jusqu_a_cepc();
    b.action(ids::REP, json!("0"), ecrans::ENQUETE);
    attendre_windows(&mut b);
    assert!(b.b.pages.indices_du_jour.is_some());
    assert_eq!(b.windows.lectures.load(Ordering::SeqCst), 1);
    // 5 minutes plus tard : rien de relu
    b.horloge.avancer(5);
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    cepc_et_lecture(&mut b, false, "par mots, 5 minutes après");
    // Par l'assistant, sûr de lui, 11 minutes plus tard
    b.horloge.avancer(11);
    b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
    b.b.pages.aides.lit = true;
    let d = Decision::Enquete { panne: "ecranbleu".into(), reponses: Vec::new() };
    let e = b.b.decision_assistant(&cat(), d, &journal);
    b.executer(e);
    assert_eq!(b.dernier_ecran().as_deref(), Some(ecrans::ENQUETE));
    cepc_et_lecture(&mut b, true, "par l'assistant");
    // Par « Pour être sûr », 11 minutes plus tard
    b.horloge.avancer(11);
    b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
    b.b.pages.aides.lit = true;
    let p = cat().panne("ecranbleu").unwrap().clone();
    let choix = Choix {
        panne: "ecranbleu".into(),
        titre: texte(champ(&p, "Titre")),
        phrase: texte(champ(&p, "Phrase")),
        reponses: Vec::new(),
    };
    let d = Decision::PourEtreSur { phrase: "mon écran devient bleu".into(), choix: vec![choix] };
    let e = b.b.decision_assistant(&cat(), d, &journal);
    b.executer(e);
    b.action(ids_aides::IDEE, json!("0"), crate::pages::POUR_ETRE_SUR);
    assert_eq!(b.dernier_ecran().as_deref(), Some(ecrans::ENQUETE));
    cepc_et_lecture(&mut b, true, "par « Pour être sûr »");
    // « Reprendre l'enquête » (pas finie, « Sur ce PC ? » déjà répondu), 11 minutes plus tard : relue avant de
    // continuer
    b.action(ids::PLUS_TARD, Value::Null, ecrans::ENQUETE);
    b.horloge.avancer(11);
    let avant = b.windows.lectures.load(Ordering::SeqCst);
    b.action(crate::pages::REPRENDRE, Value::Null, ecrans::ACCUEIL);
    assert!(b.b.windows.attendue(), "« Reprendre » : lecture de Windows attendue");
    attendre_windows(&mut b);
    assert_eq!(b.windows.lectures.load(Ordering::SeqCst), avant + 1, "« Reprendre » : lectures de Windows");
    assert!(!matches!(b.vue(), Some(Vue::Attente) | None));
    // (une réponse dans l'enquête reprise ne relit rien de plus)
    b.action(ids::NSP, Value::Null, ecrans::ENQUETE);
    assert_eq!(b.windows.lectures.load(Ordering::SeqCst), avant + 1);
}

/// Sur le PC en panne, Windows lent : la page attend sans jamais bloquer, puis l'enquête continue sans lui au bout
/// de 30 secondes (et ne s'en sert plus ensuite).
#[test]
fn windows_lent_abandonne_au_bout_de_30_secondes() {
    let mut b = Banc::nouveau(Ok(cat()), true);
    b.envoyer(MessagePage::Pret);
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    b.jusqu_a_cepc();
    let avant = Instant::now();
    b.action(ids::REP, json!("0"), ecrans::ENQUETE);
    assert!(avant.elapsed() < Duration::from_secs(2), "la réponse ne doit pas attendre Windows");
    assert!(matches!(b.vue(), Some(Vue::Attente)));
    assert_eq!(b.dernier_etat()["pages"][ecrans::ENQUETE]["titre"], "Mindcare regarde ce que Windows a noté...");
    assert!(b.b.windows.attendue());
    let debut = Instant::now();
    // Les pas de la minuterie, avant 30 secondes : on attend toujours, et chaque pas rend la main tout de suite
    for s in [0, 1, 10, 29] {
        let t = Instant::now();
        b.tour(debut + Duration::from_secs(s));
        assert!(t.elapsed() < Duration::from_millis(500), "un pas de la minuterie a bloqué");
        assert!(matches!(b.vue(), Some(Vue::Attente)), "{s} s");
    }
    // Au bout de 30 secondes : l'enquête continue sans Windows
    b.tour(debut + Duration::from_secs(31));
    assert!(!b.b.windows.attendue());
    assert!(!matches!(b.vue(), Some(Vue::Attente) | None));
    assert_eq!(b.b.pages.indices_du_jour.as_ref().map(|i| i.len()), Some(0));
    let j = b.journal();
    assert!(j.contains("Journaux de Windows non lus : trop longs à lire, enquête continuée sans eux"), "{j}");
    assert!(j.contains("Journaux de Windows : attendus"), "{j}");
    // La lecture finit plus tard : l'enquête ne s'en sert pas
    b.windows.liberer();
    std::thread::sleep(Duration::from_millis(200));
    b.tour(Instant::now());
    assert_eq!(b.b.pages.indices_du_jour.as_ref().map(|i| i.len()), Some(0));
}

/// Pendant l'attente, « Accueil » : la lecture finie ne ramène pas sur l'enquête.
#[test]
fn windows_lu_apres_etre_parti() {
    let mut b = Banc::nouveau(Ok(cat()), true);
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    b.jusqu_a_cepc();
    b.action(ids::REP, json!("0"), ecrans::ENQUETE);
    assert!(matches!(b.vue(), Some(Vue::Attente)));
    b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
    let envois = b.page.envois.borrow().len();
    b.windows.liberer();
    attendre_windows(&mut b);
    assert!(b.vue().is_none());
    // (seulement l'état rafraîchi, sans changer d'écran)
    let apres: Vec<Envoi> = b.page.envois.borrow()[envois..].to_vec();
    assert!(!apres.is_empty() && apres.iter().all(|e| matches!(e, Envoi::Etat(_, None))), "{apres:?}");
}

/// Une enquête laissée en cours se reprend après une relance de l'appli, à la même question.
#[test]
fn reprise_apres_relance() {
    let mut b = Banc::nouveau(Ok(cat()), false);
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    b.action(ids::NSP, Value::Null, ecrans::ENQUETE);
    b.action(ids::NSP, Value::Null, ecrans::ENQUETE);
    let choix = b.b.pages.en_cours.choix.clone();
    assert_eq!(choix.len(), 2);
    let etape = b.etape();
    b.action(ids::PLUS_TARD, Value::Null, ecrans::ENQUETE);
    assert_eq!(b.dernier_ecran().as_deref(), Some(ecrans::ACCUEIL));
    assert!(b.messages().iter().any(|(t, _)| t == crate::pages::PLUS_TARD_MESSAGE));
    // La relance : le même dossier, de nouvelles pages
    b.effacer = false;
    let racine = b.racine.clone();
    drop(b);
    let mut b = Banc::dans(racine, Ok(cat()), false, true);
    b.envoyer(MessagePage::Pret);
    let accueil = b.dernier_etat()["accueil"].to_string();
    assert!(accueil.contains(&format!("\"{}\"", crate::pages::REPRENDRE)), "{accueil}");
    let carte = std::fs::read_to_string(b.racine.join("Donnees").join("etat.json")).unwrap();
    assert!(carte.contains("Pas finie"), "{carte}");
    b.action(crate::pages::REPRENDRE, Value::Null, ecrans::ACCUEIL);
    assert_eq!(b.dernier_ecran().as_deref(), Some(ecrans::ENQUETE));
    assert_eq!(b.b.pages.en_cours.choix, choix);
    assert_eq!(b.etape(), etape);
    assert!(b.journal().contains("Reprise de l'enquête"));
}

/// Le catalogue absent ou abîmé : la page du problème, une ligne au journal, et jamais un plantage.
#[test]
fn catalogue_absent_ou_abime() {
    let dossier = dossier_neuf();
    std::fs::create_dir_all(&dossier).unwrap();
    let abime = dossier.join("enquetes.json");
    std::fs::write(&abime, "{ \"Pannes\": [ abîmé").unwrap();
    for cat in [charger_catalogue(&dossier.join("absent.json")), charger_catalogue(&abime)] {
        let raison = cat.as_ref().err().cloned().expect("refusé");
        let mut b = Banc::nouveau(cat, false);
        assert!(b.journal().contains(&format!("Enquêtes illisibles : {raison}")), "{}", b.journal());
        b.envoyer(MessagePage::Pret);
        let e = b.dernier_etat();
        assert_eq!(e["accueil"]["tuiles"][0]["id"], ecrans::PROBLEME);
        assert!(e["pages"].get(ecrans::PROBLEME).is_some());
        // Rien d'autre ne fait planter, ni ne change
        let n = b.page.envois.borrow().len();
        b.action("panne:ecranbleu", Value::Null, ecrans::ACCUEIL);
        b.action(ids::BUREAU, Value::Null, ecrans::REPARATEUR);
        b.envoyer(MessagePage::Ecran { id: "probleme".into() });
        b.envoyer(MessagePage::Reponse { id: "q".into(), valeur: "ok".into() });
        b.tour(Instant::now() + Duration::from_secs(60));
        b.executer(vec![Effet::RapportBureau, Effet::CopierRapport, Effet::EnregistrerEnquete]);
        assert_eq!(b.page.envois.borrow().len(), n);
        assert!(b.bureau().is_empty());
        b.envoyer(MessagePage::Menu);
        assert_eq!(b.menus, 1);
    }
    let _ = std::fs::remove_dir_all(&dossier);
}

/// Bureau introuvable, presse-papiers pris : un message simple, une ligne au journal.
#[test]
fn rapport_et_copie_impossibles() {
    let mut b = Banc::dans(dossier_neuf(), Ok(cat()), false, false);
    b.b.exterieur = Box::new(FauxExterieur { bureau: None, copies: Rc::clone(&b.copies), copie_possible: false });
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    b.jusqu_a_la_reponse("1");
    b.action(ids::BUREAU, Value::Null, ecrans::REPARATEUR);
    assert!(b.dernier_etat()["pages"][ecrans::REPARATEUR].to_string().contains(reparateur::ECHEC_ENREGISTRER));
    assert!(b.journal().contains("Rapport non enregistré : Bureau introuvable"));
    b.action(ids::COPIER, Value::Null, ecrans::REPARATEUR);
    assert!(b.dernier_etat()["pages"][ecrans::REPARATEUR].to_string().contains(reparateur::ECHEC_COPIE));
    assert!(b.messages().is_empty(), "{:?}", b.messages());
    assert!(b.journal().contains("Rapport non copié : presse-papiers pris"));
    assert!(!b.b.pages.copie);
    // Une commande à copier : « Copie impossible », et le journal
    b.executer(vec![Effet::CopierCommande { numero: 1, texte: "sfc /scannow".into() }]);
    assert_eq!(b.b.pages.commande, Some((1, false)));
    assert!(b.journal().contains("Commande non copiée : presse-papiers pris"));
    // Le presse-papiers revient : la commande est copiée
    b.b.exterieur = Box::new(FauxExterieur { bureau: None, copies: Rc::clone(&b.copies), copie_possible: true });
    b.executer(vec![Effet::CopierCommande { numero: 0, texte: "sfc /scannow".into() }]);
    assert_eq!(b.b.pages.commande, Some((0, true)));
    assert_eq!(b.copies.borrow().as_slice(), ["sfc /scannow"]);
}

/// Les messages que les pages ne connaissent pas : au journal, rien d'autre.
#[test]
fn messages_inconnus_au_journal() {
    let mut b = Banc::nouveau(Ok(cat()), false);
    b.envoyer(MessagePage::Reponse { id: "q".into(), valeur: "oui".into() });
    b.action("bouton-inconnu", Value::Null, ecrans::ACCUEIL);
    let j = b.journal();
    assert!(j.contains("Écran : inconnu (message « reponse »)"), "{j}");
    assert!(j.contains("Écran : inconnu (bouton-inconnu)"), "{j}");
    assert!(b.page.envois.borrow().is_empty());
}

/// Les messages vérifiés du kit redeviennent ceux que les pages lisent.
#[test]
fn message_du_kit() {
    let m = MessagePage::Action { id: "rep".into(), valeur: json!("2"), ecran: "enquete".into() };
    assert_eq!(message_kit(&m), json!({ "type": "action", "id": "rep", "valeur": "2", "ecran": "enquete" }));
    assert_eq!(message_kit(&MessagePage::Pret), json!({ "type": "pret" }));
    assert_eq!(message_kit(&MessagePage::Ecran { id: "aide".into() }), json!({ "type": "ecran", "id": "aide" }));
}

/// La conversion de ce que Windows a noté (mindcare-windows) en indices gardés (mindcare-donnees).
#[test]
fn indices_windows_gardes() {
    let mut i = mindcare_windows::IndicesWindows::new();
    let indice = |vu, texte: &str, ok: &str, affiche, partiel| mindcare_windows::IndiceWindows {
        vu,
        texte: texte.into(),
        texte_ok: ok.into(),
        affiche,
        partiel,
    };
    i.insert("coupures".into(), indice(true, "2 coupures", "", true, false));
    i.insert("journal".into(), indice(false, "", "le journal commence le 2 octobre", false, true));
    let g = indices_gardes(&i);
    assert_eq!(g.len(), 2);
    let c = g.get("Coupures").unwrap();
    assert!(c.vu && c.affiche && !c.partiel && c.texte == "2 coupures");
    let j = g.get("journal").unwrap();
    assert!(!j.vu && !j.affiche && j.partiel && j.texte_ok == "le journal commence le 2 octobre");
    // Le moteur reçoit les mêmes indices que par vers_moteur
    assert_eq!(g.moteur(), mindcare_windows::vers_moteur(&i));
}

// ------------------------------------------------------------------------------------------------ le PC reconnu

/// Un portable HP, tel que WMI le dirait.
fn portable_hp() -> mindcare_windows::PcBrut {
    mindcare_windows::PcBrut {
        boitiers: vec![10],
        genre_systeme: Some(2),
        batterie: Some(true),
        fabricant: "HP".into(),
        modele: "HP Pavilion Laptop 15-eh1xxx".into(),
        carte_fabricant: "HP".into(),
        carte_modele: "87B2".into(),
        processeur: "AMD Ryzen 5 5500U with Radeon Graphics".into(),
        barrettes: vec![Some(8 << 30)],
        systeme: "Microsoft Windows 11 Famille".into(),
        build: "22631".into(),
        ..Default::default()
    }
}

/// Attend (au plus 5 s) que la détection du PC arrive aux pages, pas de minuterie après pas de minuterie.
fn attendre_pc(b: &mut Banc) {
    let debut = Instant::now();
    while b.b.pages.pc.is_none() && debut.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
        b.tour(Instant::now());
    }
    assert!(b.b.pages.pc.is_some(), "la détection du PC n'arrive pas");
}

/// Les clés des lignes de la carte « Le PC » de « Pour le réparateur » (vide : la carte dit « rien de lu »).
fn cles_le_pc(e: &Value) -> Vec<String> {
    fn chercher(v: &Value, l: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                if o.get("type").and_then(Value::as_str) == Some("rangs")
                    && o.get("titre").and_then(Value::as_str) == Some("Le PC")
                {
                    for r in o["rangs"].as_array().into_iter().flatten() {
                        l.push(r["cle"].as_str().unwrap_or_default().to_string());
                    }
                }
                o.values().for_each(|x| chercher(x, l));
            }
            Value::Array(a) => a.iter().for_each(|x| chercher(x, l)),
            _ => {}
        }
    }
    let mut l = Vec::new();
    chercher(&e["pages"][ecrans::REPARATEUR], &mut l);
    l
}

/// Le texte de la bande des familles (rien sans bande).
fn bande_familles(e: &Value) -> String {
    let b = &e["pages"][ecrans::FAMILLES]["blocs"][0];
    if b["type"] == "bande" { b["texte"].as_str().unwrap_or_default().to_string() } else { String::new() }
}

/// La détection lancée au démarrage sur son fil : la fenêtre ne l'attend pas, les pages la reçoivent au pas de la
/// minuterie qui suit, une seule fois ; la pastille et le tri la suivent.
#[test]
fn detection_du_pc_au_demarrage() {
    let mut b = Banc::avec_pc(dossier_neuf(), Ok(cat()), false, true, Some(portable_hp()));
    b.envoyer(MessagePage::Pret);
    attendre_pc(&mut b);
    let pc = b.b.pages.pc.clone().unwrap();
    assert_eq!(pc.profil.libelle_court(), "Portable HP");
    assert_eq!(pc.fiche.genre.as_deref(), Some("Portable"));
    assert!(b.journal().contains("PC reconnu : Portable HP"), "{}", b.journal());
    // (la page redessinée avec la pastille et la bande)
    let e = b.dernier_etat();
    assert_eq!(e["pc"]["texte"], "Portable HP");
    assert!(bande_familles(&e).starts_with("Adapté à ton portable HP : "), "{}", bande_familles(&e));
    // (une seule fois : les pas suivants ne redessinent rien pour elle)
    let envois = b.page.envois.borrow().len();
    b.tour(Instant::now());
    b.tour(Instant::now());
    assert_eq!(b.page.envois.borrow().len(), envois);
    assert_eq!(b.journal().matches("PC reconnu").count(), 1);
}

/// Contre-épreuve : un faux Windows qui ne donne pas le matériel (comme tous les autres tests, et jamais le vrai PC) :
/// le PC n'est pas reconnu, rien n'est caché.
#[test]
fn detection_ratee_rien_de_cache() {
    let mut b = Banc::nouveau(Ok(cat()), false);
    b.envoyer(MessagePage::Pret);
    attendre_pc(&mut b);
    assert!(b.b.pages.pc.as_ref().unwrap().profil.est_inconnu());
    assert!(b.journal().contains("PC non reconnu : matériel non lu"), "{}", b.journal());
    let e = b.dernier_etat();
    assert_eq!(e["pc"]["texte"], "PC pas reconnu");
    assert_eq!(bande_familles(&e), "");
}

/// Un autre PC choisi est gardé après une relance de l'appli ; « Tout afficher » ne l'est pas.
#[test]
fn choix_garde_apres_relance() {
    let racine = dossier_neuf();
    let mut b = Banc::avec_pc(racine.clone(), Ok(cat()), false, true, Some(portable_hp()));
    b.envoyer(MessagePage::Pret);
    attendre_pc(&mut b);
    b.action(ids::TON_PC, Value::Null, ecrans::ACCUEIL);
    b.action("pc-genre:fixe", Value::Null, ecrans::TON_PC);
    b.action("pc-marque:Dell", Value::Null, ecrans::TON_PC_MARQUE);
    b.action(ids::TOUT_AFFICHER, Value::Null, ecrans::FAMILLES);
    assert!(b.journal().contains("PC choisi : un autre PC (PC fixe Dell)"), "{}", b.journal());
    assert!(b.b.pages.tout_afficher);
    b.effacer = false;
    drop(b);
    let mut b = Banc::avec_pc(racine, Ok(cat()), false, true, Some(portable_hp()));
    b.envoyer(MessagePage::Pret);
    // (avant même la détection : l'autre PC choisi trie déjà)
    let e = b.dernier_etat();
    assert_eq!(e["pc"]["texte"], "Autre : PC fixe Dell");
    assert!(!b.b.pages.tout_afficher, "« Tout afficher » est pour une fois seulement");
    assert!(bande_familles(&e).starts_with("Adapté au PC choisi (PC fixe Dell) : "), "{}", bande_familles(&e));
    attendre_pc(&mut b);
    assert_eq!(b.dernier_etat()["pc"]["texte"], "Autre : PC fixe Dell");
}

/// Le rapport pour le réparateur : sur ce PC, la fiche du PC reconnu (et la partie « Le PC » de la page remplie) ;
/// pas sur ce PC, rien du PC, ou le PC choisi pour un autre.
#[test]
fn rapport_avec_la_fiche_du_pc() {
    let mut b = Banc::avec_pc(dossier_neuf(), Ok(cat()), false, true, Some(portable_hp()));
    b.envoyer(MessagePage::Pret);
    attendre_pc(&mut b);
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    b.jusqu_a_cepc();
    b.action(ids::REP, json!("0"), ecrans::ENQUETE);
    attendre_windows(&mut b);
    b.jusqu_a_la_reponse("0");
    b.action(ids::REPARATEUR, Value::Null, ecrans::ENQUETE);
    b.action(ids::COPIER, Value::Null, ecrans::REPARATEUR);
    let rapport = b.copies.borrow().last().cloned().expect("rapport copié");
    for l in [
        "Ordinateur : Dell XPS 13, Windows 11 Famille",
        "Type : Portable",
        "Processeur : AMD Ryzen 5 5500U with Radeon Graphics",
        "Mémoire installée : 8 Go",
        "Windows : Windows 11, version 23H2",
    ] {
        assert!(rapport.contains(l), "« {l} » manque : {rapport}");
    }
    // (le modèle est déjà dans « Ordinateur : »)
    assert!(!rapport.contains("Modèle : "), "{rapport}");
    assert!(!rapport.contains("PC choisi"), "{rapport}");
    // (la carte « Le PC » de la page : ses lignes, pas la phrase « rien de lu cette fois »)
    let cles = cles_le_pc(&b.dernier_etat());
    for c in ["Ordinateur", "Type", "Processeur", "Mémoire installée", "Windows"] {
        assert!(cles.iter().any(|x| x == c), "« {c} » manque dans « Le PC » : {cles:?}");
    }

    // Pas sur ce PC, avec un autre PC choisi : seulement le PC choisi
    b.action(ids::ACCUEIL, Value::Null, ecrans::REPARATEUR);
    b.action(ids::TON_PC, Value::Null, ecrans::ACCUEIL);
    b.action("pc-genre:fixe", Value::Null, ecrans::TON_PC);
    b.action("pc-marque:ASUS", Value::Null, ecrans::TON_PC_MARQUE);
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    b.jusqu_a_la_reponse("1");
    b.action(ids::COPIER, Value::Null, ecrans::REPARATEUR);
    let rapport = b.copies.borrow().last().cloned().unwrap();
    assert!(rapport.contains("PC choisi : PC fixe ASUS"), "{rapport}");
    assert!(!rapport.contains("Processeur"), "la fiche de ce PC pour une panne d'un autre : {rapport}");
    assert!(!rapport.contains("Ordinateur :"), "{rapport}");
    assert_eq!(cles_le_pc(&b.dernier_etat()), ["PC choisi"]);
    // (contre-épreuve : revenu à mon PC, pas sur ce PC : rien du PC)
    b.action(ids::MON_PC, Value::Null, ecrans::TON_PC);
    b.action("panne:ecranbleu", Value::Null, ecrans::RACONTE);
    b.jusqu_a_la_reponse("1");
    b.action(ids::COPIER, Value::Null, ecrans::REPARATEUR);
    let rapport = b.copies.borrow().last().cloned().unwrap();
    assert!(!rapport.contains("PC choisi") && !rapport.contains("Processeur"), "{rapport}");
    assert!(cles_le_pc(&b.dernier_etat()).is_empty(), "« Le PC » : rien de lu cette fois");
}
