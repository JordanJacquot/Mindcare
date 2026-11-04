//! Les juges de l'assistant et de la dictée CÔTÉ ÉCRANS (étape 6.4 de « Mindcare en Rust ») : ce que
//! outils\tester-assistant.ps1 et outils\tester-dictee.ps1 vérifient de ce qui s'affiche et de ce qui se passe, rejoué
//! par les pages branchées (`effets::Branchement` et ses `Outils`), comme le programme le fait, avec des faux :
//! - le faux llama-server des tests de mindcare-assistant (dans ce test lui-même, sur 127.0.0.1 : aucun programme
//!   lancé) ;
//! - le faux serveur de téléchargement des tests de mindcare-telechargement (sur 127.0.0.1, rien sur Internet), avec
//!   ses zips faits à la main, par le vrai WinHTTP sans proxy ;
//! - un faux micro (le test règle ce qu'il « entend » ; le vrai micro n'est jamais ouvert) et un faux whisper (aucun
//!   programme lancé : il écrit la phrase demandée là où whisper l'écrirait).
//!
//! Chaque juge rend `Err` avec ce qu'il a vu ; les contre-épreuves (en bas) montrent qu'un juge voit bien une appli qui
//! se trompe. Tout se passe dans un dossier d'essai neuf (dans le dossier temporaire), effacé à la fin.

#![cfg(windows)]

#[allow(dead_code, clippy::all)]
#[path = "../../mindcare-assistant/tests/commun/mod.rs"]
mod ca;
#[allow(dead_code, clippy::all)]
#[path = "../../mindcare-telechargement/tests/commun/mod.rs"]
mod ct;

use std::cell::RefCell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use mindcare_appli::effets::{Branchement, Exterieur, LectureWindows, Monde, Outils, VersLaPage, charger_catalogue};
use mindcare_appli::pages::{Effet, POUR_ETRE_SUR, ecrans, ids, ids_aides, textes_aides};
use mindcare_appli::pont::MessagePage;
use mindcare_assistant::{Assistant, HorlogeVraie, MESSAGE_ECHEC, Reglages};
use mindcare_dictee::{Dictee, DicteeEnFond, Lance, Lanceur, Materiel, Mesures, Micro, Processus, messages};
use mindcare_donnees::{Donnees, Moment as MomentDonnees, horloge_figee};
use mindcare_moteur::Catalogue;
use mindcare_moteur::ps::{champ, texte};
use mindcare_telechargement::{Installation, Paquet, Transport};
use mindcare_windows::{
    Barrette, Batterie, Disque, DisquePhysique, Evenement, Filtre, Horloge, InfosPc, Mesure, Moment, Partition, Place,
    SourceWindows,
};
use serde_json::{Value, json};

type Juge = Result<(), String>;

macro_rules! exiger {
    ($c:expr, $($t:tt)*) => {
        if !$c {
            return Err(format!($($t)*));
        }
    };
}

// ------------------------------------------------------------------------------------------------ le catalogue

fn cat() -> Arc<Catalogue> {
    static CAT: OnceLock<Arc<Catalogue>> = OnceLock::new();
    CAT.get_or_init(|| {
        let f = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("Mindcare").join("fichiers");
        charger_catalogue(&f.join("enquetes.json")).expect("enquetes.json")
    })
    .clone()
}

// ------------------------------------------------------------------------------------------------ un Windows muet

/// Windows n'est jamais lu ici (les juges ne vont pas jusqu'aux étapes « ce PC »).
struct WindowsMuet;

impl SourceWindows for WindowsMuet {
    fn plus_ancien(&self) -> Mesure<Option<Moment>> {
        Err("essai".into())
    }
    fn evenements(&self, _: &Filtre) -> Mesure<Vec<Evenement>> {
        Err("essai".into())
    }
    fn disques(&self) -> Mesure<Vec<Disque>> {
        Err("essai".into())
    }
    fn partitions(&self) -> Mesure<Vec<Partition>> {
        Err("essai".into())
    }
    fn disques_physiques(&self) -> Mesure<Vec<DisquePhysique>> {
        Err("essai".into())
    }
    fn lecteur_systeme(&self) -> String {
        "C:".into()
    }
    fn place(&self, _: &str) -> Mesure<Place> {
        Err("essai".into())
    }
    fn batterie(&self) -> Mesure<Option<Batterie>> {
        Err("essai".into())
    }
    fn demarrage(&self) -> Mesure<Moment> {
        Err("essai".into())
    }
    fn barrettes(&self) -> Mesure<Vec<Barrette>> {
        Err("essai".into())
    }
    fn infos_pc(&self) -> Mesure<InfosPc> {
        Err("essai".into())
    }
}

struct HorlogeMuette;

impl Horloge for HorlogeMuette {
    fn maintenant(&self) -> Moment {
        Moment::nouveau(2026, 11, 3, 10, 15, 0, 0).unwrap()
    }
}

// ------------------------------------------------------------------------------------------------ page et extérieur

#[derive(Default)]
struct FaussePage {
    etats: RefCell<Vec<(Value, Option<String>)>>,
    ordres: RefCell<Vec<Value>>,
}

impl VersLaPage for FaussePage {
    fn etat(&self, etat: Value, ecran: Option<&str>) {
        self.etats.borrow_mut().push((etat, ecran.map(str::to_string)));
    }
    fn ordre(&self, ordre: Value) {
        self.ordres.borrow_mut().push(ordre);
    }
}

struct SansExterieur;

impl Exterieur for SansExterieur {
    fn bureau(&self) -> Result<PathBuf, String> {
        Err("pas de Bureau dans ce test".into())
    }
    fn copier(&mut self, _: &str) -> Result<(), String> {
        Err("pas de presse-papiers dans ce test".into())
    }
}

// ------------------------------------------------------------------------------------------------ le faux micro

/// Ce que le faux micro « entend », réglé par le test ; et ce qu'on lui a demandé.
#[derive(Default)]
struct Oreille {
    millis: u32,
    entendu: bool,
    depuis_voix: u32,
    /// Le code de Windows au démarrage (0 : il marche ; 4 : refusé ou déjà pris)
    code: u32,
    demarre: Option<u32>,
    arrete: bool,
}

struct FauxMicro(Arc<Mutex<Oreille>>);

impl Micro for FauxMicro {
    fn demarrer(&mut self, numero: u32) -> Result<(), u32> {
        let mut o = self.0.lock().unwrap();
        o.demarre = Some(numero);
        if o.code == 0 { Ok(()) } else { Err(o.code) }
    }
    fn mesures(&self) -> Mesures {
        let o = self.0.lock().unwrap();
        Mesures { niveau: 0.3, millis: o.millis, entendu: o.entendu, millis_depuis_voix: o.depuis_voix }
    }
    fn arreter(&mut self) {
        self.0.lock().unwrap().arrete = true;
    }
    fn son(&self) -> Vec<u8> {
        vec![1; 3200]
    }
}

struct FauxMateriel(Arc<Mutex<Oreille>>);

impl Materiel for FauxMateriel {
    fn micro_present(&self) -> bool {
        true
    }
    fn noms(&self) -> Vec<String> {
        vec!["Micro du casque".into()]
    }
    fn nouveau_micro(&self) -> Box<dyn Micro> {
        Box::new(FauxMicro(self.0.clone()))
    }
}

// ------------------------------------------------------------------------------------------------ le faux whisper

/// Ce que le faux whisper écrit (comme FAUX_WHISPER de tester-dictee.ps1) ; « attendre » : il ne finit jamais seul.
#[derive(Default)]
struct Plume {
    phrase: String,
    attendre: bool,
    lances: usize,
    tue: bool,
}

struct FauxProcessus(Arc<Mutex<Plume>>, bool);

impl Processus for FauxProcessus {
    fn fini(&mut self) -> Option<Option<i32>> {
        let p = self.0.lock().unwrap();
        if p.tue {
            Some(None)
        } else if self.1 {
            Some(Some(0))
        } else {
            None
        }
    }
    fn tuer(&mut self) {
        self.0.lock().unwrap().tue = true;
    }
    fn numero(&self) -> u32 {
        4242
    }
}

/// Le faux lancement de whisper : aucun programme, la phrase écrite tout de suite dans le fichier -of + « .txt ».
struct FauxWhisper(Arc<Mutex<Plume>>);

impl Lanceur for FauxWhisper {
    fn lancer(&self, _programme: &Path, arguments: &[OsString], _dossier: &Path) -> Result<Lance, String> {
        let mut p = self.0.lock().unwrap();
        p.lances += 1;
        p.tue = false;
        let i = arguments.iter().position(|a| a == "-of").ok_or("pas de -of")?;
        let sortie = arguments.get(i + 1).ok_or("pas de sortie")?.to_string_lossy().into_owned();
        if !p.attendre {
            std::fs::write(format!("{sortie}.txt"), p.phrase.as_bytes()).map_err(|e| e.to_string())?;
        }
        let fini = !p.attendre;
        Ok(Lance { processus: Box::new(FauxProcessus(self.0.clone(), fini)), relie: Ok(()) })
    }
    fn vivant(&self, _pid: u32) -> bool {
        false
    }
}

// ------------------------------------------------------------------------------------------------ le banc

/// Ce qui est déjà installé avant d'ouvrir Mindcare.
#[derive(Clone, Copy, PartialEq)]
enum Deja {
    Rien,
    Installe,
    /// Le programme sans toutes ses .dll (une installation coupée)
    Coupe,
    /// Le moteur et un ANCIEN modèle (la mise à jour est proposée)
    Ancien,
}

#[derive(Clone)]
struct Reglage {
    scenario: ca::Scenario,
    ia: Deja,
    dictee: Deja,
    place: Option<u64>,
    /// Le faux serveur donne ses fichiers lentement (l'avancement se voit)
    lent: bool,
}

impl Default for Reglage {
    fn default() -> Reglage {
        Reglage {
            scenario: ca::Scenario::Normal(ca::comportement_bruit()),
            ia: Deja::Installe,
            dictee: Deja::Installe,
            place: None,
            lent: false,
        }
    }
}

/// Une réponse du faux llama-server : la panne, ses réponses, et la confiance de son nom (par ses jetons).
fn comportement(panne: &str, reponses: Value, confiance: f64) -> ca::Comportement {
    let contenu = json!({ "panne": panne, "reponses": reponses }).to_string();
    let debut = r#"{"panne":""#;
    let fin = &contenu[debut.len() + panne.len()..];
    let jetons = json!([
        { "token": debut, "logprob": 0.0 },
        { "token": panne, "logprob": confiance.ln() },
        { "token": fin, "logprob": 0.0 },
    ]);
    ca::Comportement { contenu, jetons: Some(jetons), ..ca::Comportement::default() }
}

const PHRASE_SURE: &str = "mon pc portable s'éteint d'un coup, et les lumières de la maison vacillent";
const PHRASE_HESITANTE: &str = "mon pc s'éteint d'un coup";

fn sure() -> ca::Comportement {
    comportement("seteint", json!({ "type": 0, "maison": 0, "surbatterie": 1 }), 0.97)
}
fn hesitante() -> ca::Comportement {
    comportement("seteint", json!({ "maison": 0 }), 0.6)
}

/// Les fichiers du faux serveur : le moteur (un zip fait à la main) et le modèle, de l'assistant puis de la dictée.
struct Depot {
    serveur: ct::FauxServeur,
    fichiers: Arc<Vec<(String, Arc<Vec<u8>>)>>,
}

fn depot(lent: bool) -> Depot {
    let moteur = ct::zip(
        &[
            ("llama/llama-server.exe", b"faux llama-server", true),
            ("llama/llama.dll", b"faux llama", true),
            ("llama/ggml.dll", b"faux ggml", true),
            ("llama/autre.txt", b"pas voulu", true),
        ],
        false,
    );
    let whisper = ct::zip(
        &[
            ("Release/whisper-cli.exe", b"faux whisper", true),
            ("Release/whisper.dll", b"faux whisper", true),
            ("Release/ggml.dll", b"faux ggml", true),
            ("Release/ggml-base.dll", b"faux ggml-base", true),
        ],
        false,
    );
    let fichiers: Arc<Vec<(String, Arc<Vec<u8>>)>> = Arc::new(vec![
        ("moteur.zip".into(), Arc::new(moteur)),
        ("mindcare-0.6b-v3-q8_0.gguf".into(), Arc::new(ct::octets(300_000, 3))),
        ("whisper.zip".into(), Arc::new(whisper)),
        ("ggml-small-q5_1.bin".into(), Arc::new(ct::octets(200_000, 4))),
    ]);
    let f = fichiers.clone();
    let serveur = ct::FauxServeur::lancer(move |d, _| match f.iter().find(|(n, _)| *n == d.chemin) {
        Some((_, o)) => ct::Action::Donner {
            octets: o.clone(),
            reprise: true,
            couper_apres: None,
            lent: lent.then_some(Duration::from_millis(10)),
        },
        None => ct::Action::Statut(404),
    });
    Depot { serveur, fichiers }
}

impl Depot {
    /// Le paquet, ses fichiers pris sur le faux serveur (leurs adresses, empreintes et tailles).
    fn paquet(&self, mut p: Paquet, premier: usize) -> Paquet {
        for (i, f) in p.fichiers.iter_mut().enumerate() {
            let (nom, o) = &self.fichiers[premier + i];
            f.url = self.serveur.url(nom);
            f.sha256 = ct::sha256(o);
            f.taille = o.len() as u64;
        }
        p
    }
}

/// Pose dans `dossier` ce qui est déjà installé.
fn poser(p: &Paquet, deja: Deja) {
    if deja == Deja::Rien {
        return;
    }
    std::fs::create_dir_all(&p.dossier).unwrap();
    std::fs::write(&p.programme, b"faux").unwrap();
    let dll: Vec<&String> = if deja == Deja::Coupe { p.dll.iter().take(1).collect() } else { p.dll.iter().collect() };
    for d in dll {
        std::fs::write(p.dossier.join(d), b"faux").unwrap();
    }
    match deja {
        Deja::Installe => std::fs::write(&p.modele, b"faux").unwrap(),
        Deja::Ancien => std::fs::write(p.dossier.join("mindcare-0.6b-v2-q8_0.gguf"), b"ancien").unwrap(),
        Deja::Rien | Deja::Coupe => {}
    }
}

struct Banc {
    b: Branchement,
    donnees: Donnees,
    page: FaussePage,
    dossier: PathBuf,
    lanceur: Arc<ca::FauxLanceur>,
    oreille: Arc<Mutex<Oreille>>,
    plume: Arc<Mutex<Plume>>,
    /// Les changements d'écran ne sont pas dits aux pages (une appli qui se tromperait : les contre-épreuves)
    muet: bool,
    _depot: Depot,
    _essai: ct::Essai,
}

impl Banc {
    fn nouveau(nom: &str, r: Reglage) -> Banc {
        let essai = ct::Essai::nouveau(&format!("aides-{nom}"));
        let dossier = essai.0.join("Donnees");
        let mut donnees =
            Donnees::ouvrir(&dossier, horloge_figee(MomentDonnees::lire("2026-11-03T10:15:00").unwrap()), true);
        donnees.lire_historique();
        let lecture = LectureWindows::nouvelle(Arc::new(WindowsMuet), Arc::new(HorlogeMuette), Duration::from_secs(1));
        let mut b = Branchement::nouveau("0.1.0", "neon", Ok(cat()), &donnees, lecture, Box::new(SansExterieur));
        let depot = depot(r.lent);
        let pa = Paquet::assistant(&dossier);
        let pd = Paquet::dictee(&dossier);
        poser(&pa, r.ia);
        poser(&pd, r.dictee);
        let lanceur = ca::FauxLanceur::nouveau(r.scenario);
        let j = donnees.journal().clone();
        let assistant = Assistant::nouveau(
            Reglages { paquet: pa.clone(), delais: ca::delais_courts() },
            lanceur.clone(),
            Arc::new(HorlogeVraie),
            Arc::new(move |t: &str| j.ecrire(t)),
        );
        let oreille = Arc::new(Mutex::new(Oreille::default()));
        let plume = Arc::new(Mutex::new(Plume::default()));
        let temp = essai.0.join("Temp");
        std::fs::create_dir_all(&temp).unwrap();
        let reglages = mindcare_dictee::Reglages { paquet: pd.clone(), suite_json: None, temp, pid: 4242, fils: 2 };
        let dictee = DicteeEnFond::lancer(Dictee::nouvelle(
            reglages,
            Box::new(FauxMateriel(oreille.clone())),
            Box::new(FauxWhisper(plume.clone())),
        ));
        let mut ia = Installation::nouvelle(depot.paquet(pa, 0));
        ia.pause = Duration::ZERO;
        let mut di = Installation::nouvelle(depot.paquet(pd, 2));
        di.pause = Duration::ZERO;
        let transport =
            mindcare_telechargement::windows::WinHttp::sans_proxy().map(|t| Arc::new(t) as Arc<dyn Transport>);
        b.brancher(Outils::nouveaux(assistant, ia, dictee, di, transport, Arc::new(ct::Place(r.place))));
        let mut banc = Banc {
            b,
            donnees,
            page: FaussePage::default(),
            dossier,
            lanceur,
            oreille,
            plume,
            muet: false,
            _depot: depot,
            _essai: essai,
        };
        banc.envoyer(MessagePage::Pret);
        banc
    }

    fn envoyer(&mut self, m: MessagePage) {
        let mut menu = String::new;
        self.b.recevoir(&m, &mut Monde { page: &self.page, donnees: &mut self.donnees, menu: &mut menu });
    }

    fn action(&mut self, id: &str, valeur: Value, ecran: &str) {
        self.envoyer(MessagePage::Action { id: id.into(), valeur, ecran: ecran.into() });
    }

    /// Le kit va sur cet écran (`{ type: 'ecran', id }`).
    fn ecran(&mut self, id: &str) {
        if !self.muet {
            self.envoyer(MessagePage::Ecran { id: id.into() });
        }
    }

    /// Va sur « Raconte ton problème », comme une personne (la tuile, puis le kit dit l'écran).
    fn raconte(&mut self) {
        self.action(ecrans::RACONTE, Value::Null, ecrans::ACCUEIL);
        self.ecran(ecrans::RACONTE);
    }

    /// Écrit dans la recherche (le kit l'envoie un court instant après la dernière touche).
    fn ecrire(&mut self, t: &str) {
        self.action("recherche", json!(t), ecrans::RACONTE);
    }

    /// Entrée dans la recherche : le kit renvoie la phrase déjà reçue.
    fn entree(&mut self) {
        let t = self.b.pages.recherche.clone();
        self.action("recherche", json!(t), ecrans::RACONTE);
    }

    fn tour(&mut self) {
        let mut menu = String::new;
        self.b.tour(Instant::now(), &mut Monde { page: &self.page, donnees: &mut self.donnees, menu: &mut menu });
    }

    fn executer(&mut self, effets: Vec<Effet>) {
        let mut menu = String::new;
        self.b.executer(effets, &mut Monde { page: &self.page, donnees: &mut self.donnees, menu: &mut menu });
    }

    /// Fait tourner la minuterie jusqu'à ce que la condition soit vraie (15 secondes au plus).
    fn attendre(&mut self, quoi: &str, cond: impl Fn(&Banc) -> bool) -> Juge {
        let fin = Instant::now() + Duration::from_secs(15);
        while Instant::now() < fin {
            self.tour();
            if cond(self) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(format!("{quoi} : pas vu en 15 secondes (journal : {})", self.journal()))
    }

    fn journal(&self) -> String {
        std::fs::read_to_string(self.dossier.join("journal.txt")).unwrap_or_default()
    }

    fn dernier_etat(&self) -> Value {
        self.page.etats.borrow().last().map(|(e, _)| e.clone()).unwrap_or(Value::Null)
    }

    fn dernier_ecran(&self) -> Option<String> {
        self.page.etats.borrow().iter().rev().find_map(|(_, e)| e.clone())
    }

    fn page_kit(&self, id: &str) -> Value {
        self.dernier_etat()["pages"][id].clone()
    }

    /// Une ligne de « Raconte ton problème » (« La dictée », « L'assistant »).
    fn ligne(&self, nom: &str) -> Value {
        self.page_kit(ecrans::RACONTE)["blocs"]
            .as_array()
            .and_then(|b| b.iter().find(|x| x["type"] == "ligne" && x["texte"] == nom).cloned())
            .unwrap_or(Value::Null)
    }

    fn note(&self, nom: &str) -> String {
        self.ligne(nom)["note"].as_str().unwrap_or_default().to_string()
    }

    fn bouton(&self, nom: &str) -> String {
        self.ligne(nom)["bouton"]["texte"].as_str().unwrap_or_default().to_string()
    }

    fn bouton_demander(&self) -> bool {
        self.page_kit(ecrans::RACONTE)["blocs"]
            .as_array()
            .is_some_and(|b| b.iter().any(|x| x["type"] == "bouton" && x["id"] == ids_aides::DEMANDER))
    }

    fn texte_parler(&self) -> String {
        self.page_kit(ecrans::RACONTE)["blocs"][0]["bouton"]["texte"].as_str().unwrap_or_default().to_string()
    }

    fn messages(&self) -> Vec<String> {
        self.page
            .ordres
            .borrow()
            .iter()
            .filter(|o| o["kit"] == "message")
            .map(|o| o["texte"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    fn occupe(&self) -> Vec<String> {
        self.page
            .ordres
            .borrow()
            .iter()
            .filter(|o| o["kit"] == "occupe")
            .map(|o| o["texte"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// L'enquête montrée (la panne, et les réponses déjà données par la phrase), s'il y en a une.
    fn enquete(&self) -> Option<(String, Vec<(String, i64)>)> {
        let e = &self.b.pages.en_cours;
        let p = e.panne.as_ref()?;
        Some((texte(champ(p, "Id")), e.pre_rempli.as_ref().map(|r| r.0.clone()).unwrap_or_default()))
    }

    fn lancements(&self) -> usize {
        self.lanceur.nb_lancements()
    }

    /// Les fichiers de voix laissés dans le Temp de l'essai.
    fn sons(&self) -> usize {
        std::fs::read_dir(self._essai.0.join("Temp"))
            .map(|l| l.flatten().filter(|e| e.file_name().to_string_lossy().contains("dictee")).count())
            .unwrap_or(0)
    }

    fn entendre(&self, millis: u32, entendu: bool, depuis_voix: u32) {
        let mut o = self.oreille.lock().unwrap();
        (o.millis, o.entendu, o.depuis_voix) = (millis, entendu, depuis_voix);
    }
}

impl Drop for Banc {
    fn drop(&mut self) {
        self.b.arreter();
    }
}

// ================================================================================================ l'assistant

/// Bug 6 : l'assistant n'est pas lancé à l'ouverture de Mindcare, seulement dès qu'on écrit.
fn juge_pas_lance_a_l_ouverture(b: &mut Banc) -> Juge {
    b.raconte();
    for _ in 0..6 {
        b.tour();
    }
    exiger!(b.lancements() == 0, "lancé à l'ouverture : {} lancement(s)", b.lancements());
    b.ecrire("mon pc fait du bruit");
    exiger!(b.lancements() == 1, "pas lancé quand on écrit : {} lancement(s)", b.lancements());
    Ok(())
}

/// Bug 4 : réponse illisible, la personne voit un message (pas une ligne vide), la recherche reste.
fn juge_reponse_illisible(b: &mut Banc) -> Juge {
    b.raconte();
    b.ecrire("mon pc s'éteint quand je joue");
    b.entree();
    b.attendre("la réponse lue", |b| !b.b.pages.aides.lit)?;
    let note = b.note(textes_aides::LIGNE_ASSISTANT);
    exiger!(note == MESSAGE_ECHEC, "ligne de l'assistant : « {note} »");
    exiger!(b.enquete().is_none() && b.dernier_ecran().as_deref() != Some(ecrans::ENQUETE), "une enquête a commencé");
    exiger!(b.journal().contains("Assistant : réponse illisible"), "la cause n'est pas au journal");
    Ok(())
}

/// Bug 4 : installé mais il ne démarre pas, la personne voit un message (et le journal dit pourquoi).
fn juge_ne_demarre_pas(b: &mut Banc) -> Juge {
    b.raconte();
    b.ecrire("mon pc s'éteint quand je joue");
    b.entree();
    let note = b.note(textes_aides::LIGNE_ASSISTANT);
    exiger!(note == MESSAGE_ECHEC, "ligne de l'assistant : « {note} »");
    exiger!(b.journal().contains("lancement impossible"), "journal : {}", b.journal());
    Ok(())
}

/// Bug 5 : pas assez de place, dit avant de télécharger, avec « Réessayer ».
fn juge_pas_assez_de_place(b: &mut Banc) -> Juge {
    b.raconte();
    b.action(ids_aides::INSTALLER_IA, Value::Null, ecrans::RACONTE);
    b.attendre("l'erreur", |b| b.note(textes_aides::LIGNE_ASSISTANT).contains("n'a pas pu s'installer"))?;
    let note = b.note(textes_aides::LIGNE_ASSISTANT);
    exiger!(note.contains("pas assez de place sur le disque"), "ligne : « {note} »");
    exiger!(b.bouton(textes_aides::LIGNE_ASSISTANT) == textes_aides::REESSAYER, "bouton : {}", b.bouton("L'assistant"));
    exiger!(!Paquet::assistant(&b.dossier).modele.exists(), "le modèle est là quand même");
    b.attendre("la cause au journal", |b| {
        b.journal().contains("Assistant non installé : il n'y a pas assez de place")
    })?;
    Ok(())
}

/// Bug 11 : une installation coupée se voit (« Réparer »), et « Réparer » la finit.
fn juge_installation_coupee(b: &mut Banc) -> Juge {
    b.raconte();
    let note = b.note(textes_aides::LIGNE_ASSISTANT);
    exiger!(note.contains("pas allée jusqu'au bout"), "ligne : « {note} »");
    exiger!(b.bouton(textes_aides::LIGNE_ASSISTANT) == textes_aides::REPARER, "bouton : {}", b.bouton("L'assistant"));
    b.action(ids_aides::INSTALLER_IA, Value::Null, ecrans::RACONTE);
    b.attendre("l'assistant installé", |b| b.journal().contains("Assistant installé"))?;
    b.attendre("la ligne « prêt »", |b| b.bouton(textes_aides::LIGNE_ASSISTANT).is_empty())?;
    let note = b.note(textes_aides::LIGNE_ASSISTANT);
    exiger!(note.starts_with("L'assistant est prêt"), "ligne : « {note} »");
    exiger!(Paquet::assistant(&b.dossier).moteur_present(), "moteur incomplet après Réparer");
    Ok(())
}

/// Constat 25 : un assistant qui plante n'est pas relancé à chaque lettre, et on sait pourquoi (une fois).
fn juge_qui_plante(b: &mut Banc) -> Juge {
    b.raconte();
    for t in ["m", "mo", "mon", "mon ", "mon p", "mon pc"] {
        b.ecrire(t);
        std::thread::sleep(Duration::from_millis(20));
    }
    exiger!(b.lancements() == 1, "{} lancement(s) pour 6 lettres", b.lancements());
    let j = b.journal();
    exiger!(j.matches("Assistant arrêté tout seul (code 3)").count() == 1, "journal : {j}");
    Ok(())
}

/// L'installation de bout en bout : la taille, l'avancement (barre), puis prêt et « Demander à l'assistant ».
fn juge_installer_l_assistant(b: &mut Banc) -> Juge {
    b.raconte();
    let note = b.note(textes_aides::LIGNE_ASSISTANT);
    exiger!(note.contains("installe son assistant, environ 1 Mo"), "ligne : « {note} »");
    exiger!(b.bouton(textes_aides::LIGNE_ASSISTANT) == textes_aides::INSTALLER, "bouton : {}", b.bouton("L'assistant"));
    exiger!(!b.bouton_demander(), "« Demander » sans assistant");
    b.action(ids_aides::INSTALLER_IA, Value::Null, ecrans::RACONTE);
    // (un deuxième appui pendant le téléchargement ne lance rien de plus)
    b.action(ids_aides::INSTALLER_IA, Value::Null, ecrans::RACONTE);
    b.attendre("l'avancement", |b| b.ligne(textes_aides::LIGNE_ASSISTANT)["barre"].is_u64())?;
    let note = b.note(textes_aides::LIGNE_ASSISTANT);
    exiger!(note.starts_with("Installation de l'assistant : ") && note.contains("Tu peux continuer"), "« {note} »");
    b.attendre("l'assistant installé", |b| b.journal().contains("Assistant installé"))?;
    b.attendre("la ligne « prêt »", |b| b.note(textes_aides::LIGNE_ASSISTANT).starts_with("L'assistant est prêt"))?;
    let j = b.journal();
    exiger!(j.matches("Installation de l'assistant (").count() == 1, "journal : {j}");
    b.ecrire("mon pc s'éteint");
    exiger!(b.bouton_demander(), "pas de « Demander à l'assistant »");
    Ok(())
}

/// Une phrase sûre : l'enquête commence tout de suite, avec les réponses que la phrase donne vraiment.
fn juge_phrase_sure(b: &mut Banc) -> Juge {
    b.raconte();
    b.ecrire(PHRASE_SURE);
    b.action(ids_aides::DEMANDER, Value::Null, ecrans::RACONTE);
    exiger!(b.note(textes_aides::LIGNE_ASSISTANT) == textes_aides::LIT, "pendant : « {} »", b.note("L'assistant"));
    exiger!(!b.bouton_demander(), "« Demander » pendant qu'il lit");
    b.attendre("l'enquête", |b| b.enquete().is_some())?;
    let (panne, reponses) = b.enquete().unwrap_or_default();
    exiger!(panne == "seteint", "enquête : {panne}");
    exiger!(
        reponses == vec![("type".to_string(), 0), ("maison".to_string(), 0)],
        "réponses déjà données : {reponses:?}"
    );
    exiger!(b.dernier_ecran().as_deref() == Some(ecrans::ENQUETE), "écran : {:?}", b.dernier_ecran());
    exiger!(b.occupe() == vec![textes_aides::LIT.to_string(), String::new()], "pastille : {:?}", b.occupe());
    let j = b.journal();
    exiger!(j.contains("(sûr à 97 %)") && j.contains("Enquête : seteint (déjà su : type, maison)"), "journal : {j}");
    exiger!(!j.contains("éteint d'un coup"), "la phrase est au journal");
    Ok(())
}

/// Une phrase hésitante : « Pour être sûr », sur ce qu'on voit ; une réponse lance son enquête.
fn juge_phrase_hesitante(b: &mut Banc, choix: Option<usize>) -> Juge {
    b.raconte();
    b.ecrire(PHRASE_HESITANTE);
    b.entree();
    b.attendre("« Pour être sûr »", |b| b.dernier_ecran().as_deref() == Some(POUR_ETRE_SUR))?;
    let p = b.page_kit(POUR_ETRE_SUR);
    exiger!(p["blocs"][0]["texte"] == format!("« {PHRASE_HESITANTE} »"), "ce que tu as écrit : {}", p["blocs"][0]);
    let cartes = p["blocs"].as_array().and_then(|l| l.iter().find(|x| x["type"] == "cartes").cloned());
    let options = cartes.as_ref().and_then(|c| c["options"].as_array().cloned()).unwrap_or_default();
    exiger!((2..=3).contains(&options.len()), "{} réponse(s)", options.len());
    let pannes: Vec<String> =
        b.b.pages
            .aides
            .pour_etre_sur
            .as_ref()
            .map(|p| p.choix.iter().map(|c| c.panne.clone()).collect())
            .unwrap_or_default();
    exiger!(pannes.first().map(String::as_str) == Some("seteint"), "idées : {pannes:?}");
    for (o, id) in options.iter().zip(&pannes) {
        let titre = cat().panne(id).map(|x| texte(champ(x, "Titre"))).unwrap_or_default();
        exiger!(o["titre"] == titre.as_str(), "carte {o} pour {id}");
    }
    let aucune = p["blocs"].as_array().is_some_and(|l| l.iter().any(|x| x["id"] == ids_aides::AUCUNE));
    exiger!(aucune, "pas de « Aucune de ces phrases »");
    b.ecran(POUR_ETRE_SUR);
    match choix {
        Some(n) => {
            b.action(ids_aides::IDEE, json!(n.to_string()), POUR_ETRE_SUR);
            let (panne, _) = b.enquete().unwrap_or_default();
            exiger!(panne == pannes[n], "enquête {panne}, choisie {}", pannes[n]);
            exiger!(b.dernier_ecran().as_deref() == Some(ecrans::ENQUETE), "écran : {:?}", b.dernier_ecran());
            exiger!(b.journal().contains(&format!("Pour être sûr : {}", pannes[n])), "journal : {}", b.journal());
        }
        None => {
            b.action(ids_aides::AUCUNE, Value::Null, POUR_ETRE_SUR);
            exiger!(b.enquete().is_none(), "une enquête a commencé");
            exiger!(b.dernier_ecran().as_deref() == Some(ecrans::FAMILLES), "écran : {:?}", b.dernier_ecran());
            exiger!(b.journal().contains("Pour être sûr : aucune de ces phrases"), "journal : {}", b.journal());
        }
    }
    Ok(())
}

// ================================================================================================ la dictée

/// Pas installée : Parler propose l'installation (sa taille), sans rien télécharger.
fn juge_dictee_pas_installee(b: &mut Banc) -> Juge {
    b.raconte();
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    exiger!(b.messages().contains(&textes_aides::INSTALLE_D_ABORD.to_string()), "messages : {:?}", b.messages());
    exiger!(b.journal().contains("Dictée : installation proposée"), "journal : {}", b.journal());
    let note = b.note(textes_aides::LIGNE_DICTEE);
    exiger!(note.contains("installe la dictée : environ 1 Mo"), "ligne : « {note} »");
    exiger!(b.bouton(textes_aides::LIGNE_DICTEE) == textes_aides::INSTALLER, "bouton : {}", b.bouton("La dictée"));
    exiger!(b.oreille.lock().unwrap().demarre.is_none(), "le micro a été ouvert");
    Ok(())
}

/// On parle puis on se tait : whisper écrit, la phrase va dans la recherche (sans les bruits notés), le son est effacé,
/// la phrase n'est pas au journal.
fn juge_dictee_parle_puis_se_tait(b: &mut Banc) -> Juge {
    b.plume.lock().unwrap().phrase = " [Musique] Mon PC s'éteint quand je joue. \r\n".into();
    b.raconte();
    exiger!(b.texte_parler() == textes_aides::PARLER, "bouton : {}", b.texte_parler());
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    exiger!(b.texte_parler() == textes_aides::ARRETER, "pendant l'écoute : {}", b.texte_parler());
    exiger!(b.note(textes_aides::LIGNE_DICTEE) == textes_aides::DICTEE_ECOUTE, "« {} »", b.note("La dictée"));
    exiger!(b.oreille.lock().unwrap().demarre == Some(u32::MAX), "pas le micro de Windows");
    b.entendre(2000, true, 400);
    std::thread::sleep(Duration::from_millis(250));
    exiger!(
        b.b.pages.aides.dictee_ecoute() || b.plume.lock().unwrap().lances == 0,
        "arrêtée avant la fin de la phrase"
    );
    b.entendre(2600, true, 1100);
    b.attendre("la phrase", |b| b.b.pages.recherche == "Mon PC s'éteint quand je joue.")?;
    exiger!(b.note(textes_aides::LIGNE_DICTEE) == textes_aides::DICTEE_COMPRIS, "« {} »", b.note("La dictée"));
    exiger!(b.page_kit(ecrans::RACONTE)["blocs"][0]["valeur"] == "Mon PC s'éteint quand je joue.", "le champ");
    exiger!(b.oreille.lock().unwrap().arrete, "le micro n'est pas arrêté");
    exiger!(b.sons() == 0, "{} fichier(s) de voix restent", b.sons());
    let j = b.journal();
    exiger!(!j.contains("éteint") && j.contains("30 caractères"), "journal : {j}");
    exiger!(b.texte_parler() == textes_aides::PARLER, "après : {}", b.texte_parler());
    Ok(())
}

/// Deuxième appui sur Parler : l'écoute s'arrête tout de suite, la phrase est écrite.
fn juge_dictee_deuxieme_appui(b: &mut Banc) -> Juge {
    b.plume.lock().unwrap().phrase = "le wifi coupe".into();
    b.raconte();
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    b.entendre(1500, true, 0);
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    b.attendre("la phrase", |b| b.b.pages.recherche == "le wifi coupe")?;
    Ok(())
}

/// Rien entendu en 7 secondes ; micro refusé par Windows.
fn juge_dictee_rien_entendu_et_micro_refuse(b: &mut Banc) -> Juge {
    b.raconte();
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    b.entendre(7100, false, 0);
    b.attendre("rien entendu", |b| b.note(textes_aides::LIGNE_DICTEE) == messages::RIEN_ENTENDU)?;
    exiger!(b.plume.lock().unwrap().lances == 0, "whisper lancé quand même");
    b.oreille.lock().unwrap().code = 4;
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    let note = b.note(textes_aides::LIGNE_DICTEE);
    exiger!(note.contains("Confidentialité, Microphone"), "« {note} »");
    exiger!(b.texte_parler() == textes_aides::PARLER, "bouton : {}", b.texte_parler());
    Ok(())
}

/// whisper écrit encore : un appui ne relance rien ; on quitte « Raconte ton problème » : il est arrêté, le son effacé,
/// et la phrase ne va pas dans une recherche qui n'est plus à l'écran.
fn juge_dictee_quitter_pendant_l_ecriture(b: &mut Banc) -> Juge {
    b.plume.lock().unwrap().attendre = true;
    b.raconte();
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    b.entendre(1500, true, 1200);
    b.attendre("whisper écrit", |b| b.note(textes_aides::LIGNE_DICTEE) == textes_aides::DICTEE_ECRIT)?;
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    exiger!(b.plume.lock().unwrap().lances == 1, "relancé : {} lancement(s)", b.plume.lock().unwrap().lances);
    b.action(ids::ACCUEIL, Value::Null, ecrans::RACONTE);
    b.ecran(ecrans::ACCUEIL);
    exiger!(b.plume.lock().unwrap().tue, "whisper n'est pas arrêté");
    exiger!(b.sons() == 0, "{} fichier(s) de voix restent", b.sons());
    for _ in 0..10 {
        b.tour();
    }
    exiger!(b.b.pages.recherche.is_empty(), "recherche : « {} »", b.b.pages.recherche);
    Ok(())
}

/// Mindcare se ferme pendant que whisper écrit : il est arrêté, le son effacé ; llama-server aussi.
fn juge_fermeture(b: &mut Banc) -> Juge {
    b.plume.lock().unwrap().attendre = true;
    b.raconte();
    b.ecrire("mon pc fait du bruit");
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    b.entendre(1500, true, 1200);
    b.attendre("whisper écrit", |b| b.note(textes_aides::LIGNE_DICTEE) == textes_aides::DICTEE_ECRIT)?;
    b.b.arreter();
    exiger!(b.plume.lock().unwrap().tue, "whisper n'est pas arrêté");
    exiger!(b.sons() == 0, "{} fichier(s) de voix restent", b.sons());
    let arretes = b.lanceur.lances.lock().unwrap().iter().all(|(_, tue)| tue.load(Ordering::SeqCst));
    exiger!(arretes, "llama-server tourne encore");
    Ok(())
}

/// La phrase dictée se corrige, puis Entrée la donne à l'assistant (une seule fois, même appuyée deux fois).
fn juge_dictee_puis_entree(b: &mut Banc) -> Juge {
    b.plume.lock().unwrap().phrase = PHRASE_SURE.into();
    b.raconte();
    b.action(ids_aides::PARLER, Value::Null, ecrans::RACONTE);
    b.entendre(2600, true, 1100);
    b.attendre("la phrase", |b| b.b.pages.recherche == PHRASE_SURE)?;
    exiger!(b.lancements() == 1, "l'assistant ne se prépare pas avec la phrase dictée ({})", b.lancements());
    b.entree();
    b.entree();
    exiger!(b.note(textes_aides::LIGNE_DICTEE) == textes_aides::DICTEE_PRETE, "« {} »", b.note("La dictée"));
    b.attendre("l'enquête", |b| b.enquete().is_some())?;
    let n = b.lanceur.questions().len();
    exiger!(n == 1, "{n} question(s) envoyée(s)");
    Ok(())
}

/// La dictée s'installe depuis le faux serveur, puis elle est prête.
fn juge_installer_la_dictee(b: &mut Banc) -> Juge {
    b.raconte();
    b.action(ids_aides::INSTALLER_DICTEE, Value::Null, ecrans::RACONTE);
    b.attendre("l'avancement", |b| b.note(textes_aides::LIGNE_DICTEE).starts_with("Installation de la dictée : "))?;
    b.attendre("la dictée installée", |b| b.journal().contains("Dictée installée"))?;
    b.attendre("la dictée prête", |b| b.note(textes_aides::LIGNE_DICTEE) == textes_aides::DICTEE_PRETE)?;
    exiger!(b.bouton(textes_aides::LIGNE_DICTEE).is_empty(), "bouton : {}", b.bouton("La dictée"));
    exiger!(Paquet::dictee(&b.dossier).installe(), "pas installée");
    Ok(())
}

/// Un ancien assistant marche en attendant la mise à jour ; une fois à jour, llama-server est arrêté AVANT que
/// l'ancien modèle soit effacé.
fn juge_mise_a_jour(b: &mut Banc) -> Juge {
    b.raconte();
    let note = b.note(textes_aides::LIGNE_ASSISTANT);
    exiger!(note.contains("mets-la à jour"), "ligne : « {note} »");
    exiger!(
        b.bouton(textes_aides::LIGNE_ASSISTANT) == textes_aides::METTRE_A_JOUR,
        "bouton : {}",
        b.bouton("L'assistant")
    );
    b.ecrire("mon pc fait du bruit");
    exiger!(b.lancements() == 1 && b.bouton_demander(), "l'ancien ne marche pas en attendant");
    b.action(ids_aides::INSTALLER_IA, Value::Null, ecrans::RACONTE);
    b.attendre("l'ancien modèle effacé", |b| {
        b.journal().contains("Ancien modèle effacé : mindcare-0.6b-v2-q8_0.gguf")
    })?;
    let arrete = b.lanceur.lances.lock().unwrap().first().is_some_and(|(_, tue)| tue.load(Ordering::SeqCst));
    exiger!(arrete, "llama-server n'a pas été arrêté");
    let j = b.journal();
    exiger!(j.find("Assistant installé") < j.find("Ancien modèle effacé"), "ordre : {j}");
    exiger!(j.matches("Assistant : une nouvelle version est proposée").count() == 1, "journal : {j}");
    Ok(())
}

/// On écrit autre chose pendant qu'il lit : sa réponse ne nous emmène nulle part.
fn juge_autre_phrase_pendant_qu_il_lit(b: &mut Banc) -> Juge {
    b.raconte();
    b.ecrire(PHRASE_SURE);
    b.entree();
    b.ecrire("en fait non");
    std::thread::sleep(Duration::from_millis(1500));
    for _ in 0..10 {
        b.tour();
    }
    exiger!(b.enquete().is_none(), "une enquête a commencé");
    exiger!(b.occupe().last().map(String::as_str) == Some(""), "la pastille reste : {:?}", b.occupe());
    Ok(())
}

// ================================================================================================ les tests

fn juger(nom: &str, r: Reglage, juge: impl FnOnce(&mut Banc) -> Juge) {
    let mut b = Banc::nouveau(nom, r);
    if let Err(e) = juge(&mut b) {
        panic!("{nom} : {e}");
    }
}

fn reglage(scenario: ca::Scenario) -> Reglage {
    Reglage { scenario, ..Reglage::default() }
}

#[test]
fn assistant_pas_lance_a_l_ouverture() {
    juger("ouverture", Reglage::default(), juge_pas_lance_a_l_ouverture);
}

#[test]
fn assistant_reponse_illisible() {
    let c = ca::Comportement { contenu: "pas du json".into(), ..ca::Comportement::default() };
    juger("illisible", reglage(ca::Scenario::Normal(c)), juge_reponse_illisible);
}

#[test]
fn assistant_ne_demarre_pas() {
    juger("casse", reglage(ca::Scenario::PasUnProgramme), juge_ne_demarre_pas);
}

#[test]
fn assistant_pas_assez_de_place() {
    juger("place", Reglage { ia: Deja::Rien, place: Some(1024 * 1024), ..Reglage::default() }, juge_pas_assez_de_place);
}

#[test]
fn assistant_installation_coupee() {
    juger("coupee", Reglage { ia: Deja::Coupe, ..Reglage::default() }, juge_installation_coupee);
}

#[test]
fn assistant_qui_plante() {
    juger("plante", reglage(ca::Scenario::Plante), juge_qui_plante);
}

#[test]
fn assistant_installe_depuis_le_faux_serveur() {
    juger("installer", Reglage { ia: Deja::Rien, lent: true, ..Reglage::default() }, juge_installer_l_assistant);
}

#[test]
fn assistant_phrase_sure() {
    juger("sure", reglage(ca::Scenario::Normal(sure())), juge_phrase_sure);
}

#[test]
fn assistant_phrase_hesitante_choisie() {
    juger("hesite", reglage(ca::Scenario::Normal(hesitante())), |b| juge_phrase_hesitante(b, Some(1)));
}

#[test]
fn assistant_phrase_hesitante_aucune() {
    juger("aucune", reglage(ca::Scenario::Normal(hesitante())), |b| juge_phrase_hesitante(b, None));
}

#[test]
fn assistant_mise_a_jour() {
    juger("maj", Reglage { ia: Deja::Ancien, ..Reglage::default() }, juge_mise_a_jour);
}

#[test]
fn assistant_autre_phrase_pendant_qu_il_lit() {
    let c = ca::Comportement { lenteur: Duration::from_millis(800), ..sure() };
    juger("oubli", reglage(ca::Scenario::Normal(c)), juge_autre_phrase_pendant_qu_il_lit);
}

#[test]
fn dictee_pas_installee() {
    juger("dictee-absente", Reglage { dictee: Deja::Rien, ..Reglage::default() }, juge_dictee_pas_installee);
}

#[test]
fn dictee_installation_coupee() {
    juger("dictee-coupee", Reglage { dictee: Deja::Coupe, ..Reglage::default() }, |b| {
        b.raconte();
        exiger!(b.bouton(textes_aides::LIGNE_DICTEE) == textes_aides::REPARER, "bouton : {}", b.bouton("La dictée"));
        Ok(())
    });
}

#[test]
fn dictee_installee_depuis_le_faux_serveur() {
    juger(
        "dictee-installer",
        Reglage { dictee: Deja::Rien, lent: true, ..Reglage::default() },
        juge_installer_la_dictee,
    );
}

#[test]
fn dictee_parle_puis_se_tait() {
    juger("parle", Reglage::default(), juge_dictee_parle_puis_se_tait);
}

#[test]
fn dictee_deuxieme_appui() {
    juger("deux-appuis", Reglage::default(), juge_dictee_deuxieme_appui);
}

#[test]
fn dictee_rien_entendu_et_micro_refuse() {
    juger("rien", Reglage::default(), juge_dictee_rien_entendu_et_micro_refuse);
}

#[test]
fn dictee_quitter_pendant_l_ecriture() {
    juger("quitter", Reglage::default(), juge_dictee_quitter_pendant_l_ecriture);
}

#[test]
fn dictee_fermeture_pendant_l_ecriture() {
    juger("fermeture", Reglage::default(), juge_fermeture);
}

#[test]
fn dictee_puis_entree() {
    juger("dictee-entree", reglage(ca::Scenario::Normal(sure())), juge_dictee_puis_entree);
}

// ================================================================================================ contre-épreuves

/// Contre-épreuve 1 : une appli qui lancerait l'assistant à l'ouverture est vue.
#[test]
fn contre_epreuve_lance_a_l_ouverture() {
    let mut b = Banc::nouveau("ce-ouverture", Reglage::default());
    b.executer(vec![Effet::DemarrerAssistant]);
    assert!(juge_pas_lance_a_l_ouverture(&mut b).is_err(), "le juge n'a rien vu");
}

/// Contre-épreuve 2 : sûr de lui, l'assistant ne montre pas « Pour être sûr » : le juge le voit.
#[test]
fn contre_epreuve_pas_pour_etre_sur() {
    let mut b = Banc::nouveau("ce-sure", reglage(ca::Scenario::Normal(sure())));
    let r = juge_phrase_hesitante(&mut b, Some(0));
    assert!(r.is_err(), "le juge n'a rien vu");
}

/// Contre-épreuve 3 : une appli qui ne saurait pas qu'on a quitté la page (whisper laissé, son gardé) est vue.
#[test]
fn contre_epreuve_page_quittee_sans_le_savoir() {
    let mut b = Banc::nouveau("ce-quitter", Reglage::default());
    b.muet = true;
    b.raconte();
    b.envoyer(MessagePage::Ecran { id: ecrans::RACONTE.into() });
    assert!(juge_dictee_quitter_pendant_l_ecriture(&mut b).is_err(), "le juge n'a rien vu");
}

/// Contre-épreuve 4 : rien de coupé, pas de « Réparer » : le juge de l'installation coupée le voit.
#[test]
fn contre_epreuve_rien_a_reparer() {
    let mut b = Banc::nouveau("ce-coupee", Reglage::default());
    assert!(juge_installation_coupee(&mut b).is_err(), "le juge n'a rien vu");
}

/// Contre-épreuve 5 : une réponse lisible n'est pas un échec : le juge de la réponse illisible le voit.
#[test]
fn contre_epreuve_reponse_lisible() {
    let mut b = Banc::nouveau("ce-lisible", reglage(ca::Scenario::Normal(sure())));
    assert!(juge_reponse_illisible(&mut b).is_err(), "le juge n'a rien vu");
}
