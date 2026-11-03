//! llama-server, lancé seulement quand on se sert de l'assistant, et la question : le portage de `Start-Ia`,
//! `Stop-IaSiInutile`, `Stop-IaServeur`, `Stop-Ia`, `Test-IaLancee`, `Get-IaServeurLance`, `Start-IaQuestion` et
//! `Get-IaReponse` (modules\ia.ps1), avec ce que la revue du 25/10 demandait de plus (constat 25m) :
//!
//! - il n'écoute que ce PC (127.0.0.1), sur un port libre gardé (une socket ouverte dessus) jusqu'au lancement ;
//! - une clé neuve à chaque lancement, passée par une variable d'environnement (la ligne de commande se lit dans le
//!   Gestionnaire des tâches) ; avant la première phrase, Mindcare vérifie que c'est bien SON llama-server qui répond
//!   sur ce port : `/props` refusé sans la clé, accepté avec. Sinon, aucune phrase n'est envoyée ;
//! - arrêté après 5 minutes sans question (il rend la mémoire de la carte graphique : un jeu peut en avoir besoin), à
//!   la fermeture, et avec Mindcare même tuée (le Job Object, dans [`crate::windows`]) ;
//! - la vraie cause d'un échec va au journal (son code, ses dernières lignes, l'erreur de la requête).
//!
//! Tout ce qui touche Windows passe par [`Lanceur`] : les tests le remplacent par un faux llama-server en Rust.

use std::collections::VecDeque;
use std::fs;
use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use mindcare_telechargement::Paquet;
use sha2::{Digest, Sha256};

use crate::http;
use crate::requete::{Reponse, corps_question, lire_completion, lire_reponse};

/// Ce qu'il faut pour lancer llama-server
#[derive(Clone, Debug, PartialEq)]
pub struct Ordre {
    pub programme: PathBuf,
    pub arguments: Vec<String>,
    /// Les variables d'environnement données en plus (la clé, le réglage de la réflexion)
    pub environnement: Vec<(String, String)>,
    pub port: u16,
}

impl Ordre {
    /// Une variable d'environnement de l'ordre
    pub fn variable(&self, nom: &str) -> Option<&str> {
        self.environnement.iter().find(|(k, _)| k == nom).map(|(_, v)| v.as_str())
    }
}

/// Un llama-server lancé
pub trait Processus: Send {
    /// Son numéro (pour le journal)
    fn id(&self) -> u32;
    /// `None` : il tourne encore ; `Some(code)` : il s'est arrêté (`Some(None)` : code inconnu)
    fn fin(&mut self) -> Option<Option<i32>>;
    /// L'arrêter (et attendre 3 secondes au plus qu'il soit arrêté)
    fn arreter(&mut self);
    /// Ses dernières lignes (sorties et erreurs, 20 au plus), pour le journal
    fn dernieres_lignes(&mut self) -> Vec<String>;
}

/// Un lancement réussi, et ce qui n'a pas marché à côté (pas relié à Mindcare, par exemple)
pub struct Lance {
    pub processus: Box<dyn Processus>,
    pub avertissement: Option<String>,
}

/// Ce que Windows fournit : lancer un programme sans fenêtre, relié à Mindcare, et une clé aléatoire
pub trait Lanceur: Send + Sync {
    fn lancer(&self, ordre: &Ordre) -> Result<Lance, String>;
    /// Une clé neuve, impossible à deviner (au moins 32 caractères)
    fn cle(&self) -> Result<String, String>;
}

/// L'heure, que les tests avancent à la main (« 5 minutes plus tard »)
pub trait Horloge: Send + Sync {
    fn maintenant(&self) -> Instant;
}

/// La vraie heure
pub struct HorlogeVraie;

impl Horloge for HorlogeVraie {
    fn maintenant(&self) -> Instant {
        Instant::now()
    }
}

/// Une heure simulée, qui n'avance que quand on le dit
pub struct HorlogeSimulee {
    depart: Instant,
    avance: Mutex<Duration>,
}

impl HorlogeSimulee {
    pub fn nouvelle() -> HorlogeSimulee {
        HorlogeSimulee { depart: Instant::now(), avance: Mutex::new(Duration::ZERO) }
    }
    pub fn avancer(&self, d: Duration) {
        *self.avance.lock().unwrap_or_else(|e| e.into_inner()) += d;
    }
}

impl Horloge for HorlogeSimulee {
    fn maintenant(&self) -> Instant {
        self.depart + *self.avance.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Les délais (ceux du PowerShell par défaut ; les tests les raccourcissent)
#[derive(Clone, Debug, PartialEq)]
pub struct Delais {
    /// Sans question pendant ce temps, il est arrêté (`$IaRepos`, 5 minutes)
    pub repos: Duration,
    /// Tous les combien Mindcare regarde s'il ne sert plus (la minuterie, 30 secondes)
    pub veille: Duration,
    /// Arrêté tout seul : pas de nouvel essai avant ce temps (1 minute)
    pub relance: Duration,
    /// Lancé depuis moins de ce temps : il charge encore son modèle (60 secondes)
    pub recent: Duration,
    /// L'attente d'une réponse juste après le lancement (100 secondes) et ensuite (40 secondes)
    pub attente_premiere: Duration,
    pub attente: Duration,
    /// Le temps gardé pour la réponse elle-même (32 secondes) : l'attente qu'il soit prêt s'arrête avant
    pub marge_reponse: Duration,
    /// Le délai de la requête de la question (30 secondes)
    pub requete: Duration,
    /// Entre deux coups d'œil à `/health` pendant le chargement (500 ms), et leur délai (2 secondes)
    pub sonde: Duration,
    pub delai_sonde: Duration,
}

impl Default for Delais {
    fn default() -> Delais {
        Delais {
            repos: Duration::from_secs(5 * 60),
            veille: Duration::from_secs(30),
            relance: Duration::from_secs(60),
            recent: Duration::from_secs(60),
            attente_premiere: Duration::from_secs(100),
            attente: Duration::from_secs(40),
            marge_reponse: Duration::from_secs(32),
            requete: Duration::from_secs(30),
            sonde: Duration::from_millis(500),
            delai_sonde: Duration::from_secs(2),
        }
    }
}

/// Ce que voient les pages
#[derive(Clone, Debug, PartialEq)]
pub enum Etat {
    /// L'assistant n'est pas installé (la page propose de l'installer : mindcare-telechargement)
    PasInstalle,
    /// Installé, pas lancé : il démarrera à la première phrase
    Repos,
    /// Lancé, il charge son modèle
    Demarre,
    /// Prêt à lire une phrase
    Pret,
    /// Il lit une phrase
    Reflechit,
    /// Le dernier essai a échoué (la vraie cause est au journal)
    Erreur { message: String },
}

impl Etat {
    /// Une phrase simple pour la ligne « L'assistant »
    pub fn texte(&self) -> String {
        match self {
            Etat::PasInstalle => "L'assistant n'est pas installé.".to_string(),
            Etat::Repos | Etat::Pret => {
                "L'assistant est prêt : raconte ton problème avec tes mots, puis appuie sur Entrée.".to_string()
            }
            Etat::Demarre => "L'assistant se prépare, quelques secondes...".to_string(),
            Etat::Reflechit => "Je lis ta phrase...".to_string(),
            Etat::Erreur { message } => message.clone(),
        }
    }
}

/// Ce que donne une phrase posée ([`Assistant::poser`])
pub enum Question {
    /// Pas d'assistant installé : la recherche par mots reste, sans rien dire
    SansAssistant,
    /// Installé, mais il ne démarre pas (la cause est au journal) : la page montre le message d'échec
    NeDemarrePas { cause: String },
    /// La phrase est en route : [`Tache::reponse`] dit quand la réponse est là
    EnCours(Tache),
}

/// Une question en cours (la tâche de Start-IaQuestion)
pub struct Tache {
    resultat: Arc<Mutex<Option<Reponse>>>,
    debut: Instant,
    attente: Duration,
    annulee: Arc<AtomicBool>,
    partage: Weak<Partage>,
}

impl Tache {
    /// La réponse si elle est arrivée (`Get-IaReponse`) : `None` pas encore ; après l'attente, « trop long »
    pub fn reponse(&self) -> Option<Reponse> {
        if let Some(p) = self.partage.upgrade() {
            // (une question en cours : il sert)
            p.verrou().derniere = p.horloge.maintenant();
        }
        if let Some(r) = verrou(&self.resultat).clone() {
            return Some(r);
        }
        let ecoule = self.debut.elapsed();
        if ecoule < self.attente {
            return None;
        }
        self.annulee.store(true, Ordering::SeqCst);
        Some(Reponse::vide(format!("trop long ({} s)", ecoule.as_secs_f64().round())))
    }

    /// Attend la réponse (les tests, et les pages qui veulent attendre sur un fil à part)
    pub fn attendre(&self) -> Reponse {
        loop {
            if let Some(r) = self.reponse() {
                return r;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Les réglages : où est l'assistant, et les délais
#[derive(Clone, Debug)]
pub struct Reglages {
    pub paquet: Paquet,
    pub delais: Delais,
}

/// L'état interne (`$script:Ia`)
struct Interne {
    processus: Option<Box<dyn Processus>>,
    port: u16,
    cle: String,
    /// Il répond sur `/health` et c'est bien lui (la clé)
    pret: bool,
    /// Arrêté par Mindcare (relancé dès la prochaine question), pas tout seul
    arrete: bool,
    /// Son arrêt tout seul est déjà au journal
    fin_notee: bool,
    lance: Instant,
    derniere: Instant,
    /// La cause du dernier échec (au journal) : l'état « erreur » des pages
    erreur: Option<String>,
    /// Les questions en cours
    questions: usize,
    /// Chaque lancement a son numéro : les fils d'un ancien lancement s'arrêtent d'eux-mêmes
    generation: u64,
    /// llama-server.exe et sa copie Mindset.exe vérifiés (taille et date) : pas de nouvelle empreinte
    copie_vue: Option<(u64, Option<SystemTime>)>,
    empreintes: usize,
    veilleur: bool,
}

struct Partage {
    reglages: Reglages,
    lanceur: Arc<dyn Lanceur>,
    horloge: Arc<dyn Horloge>,
    journal: Arc<dyn Fn(&str) + Send + Sync>,
    interne: Mutex<Interne>,
}

fn verrou<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Partage {
    fn verrou(&self) -> MutexGuard<'_, Interne> {
        verrou(&self.interne)
    }
    fn noter(&self, t: &str) {
        (self.journal)(t)
    }
}

/// L'assistant de Mindcare. Une seule instance, gardée par l'appli ; quand elle disparaît, llama-server est arrêté.
pub struct Assistant {
    partage: Arc<Partage>,
}

impl Assistant {
    /// L'assistant (pas encore lancé : il ne l'est qu'à la première phrase, jamais à l'ouverture de Mindcare)
    pub fn nouveau(
        reglages: Reglages,
        lanceur: Arc<dyn Lanceur>,
        horloge: Arc<dyn Horloge>,
        journal: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Assistant {
        let maintenant = horloge.maintenant();
        let interne = Interne {
            processus: None,
            port: 0,
            cle: String::new(),
            pret: false,
            arrete: false,
            fin_notee: false,
            lance: maintenant,
            derniere: maintenant,
            erreur: None,
            questions: 0,
            generation: 0,
            copie_vue: None,
            empreintes: 0,
            veilleur: false,
        };
        Assistant { partage: Arc::new(Partage { reglages, lanceur, horloge, journal, interne: Mutex::new(interne) }) }
    }

    /// Le moteur et un modèle sont là (`Test-IaInstallee`)
    pub fn installe(&self) -> bool {
        self.partage.reglages.paquet.installe()
    }

    /// llama-server tourne (`Test-IaLancee`)
    pub fn lance(&self) -> bool {
        lance(&mut self.partage.verrou())
    }

    /// Le port et la clé du lancement en cours (le journal, les tests)
    pub fn port_et_cle(&self) -> (u16, String) {
        let i = self.partage.verrou();
        (i.port, i.cle.clone())
    }

    /// Le nombre d'empreintes SHA-256 calculées (les tests : une seule par session, tant que rien ne change)
    pub fn empreintes_calculees(&self) -> usize {
        self.partage.verrou().empreintes
    }

    /// Ce que voient les pages
    pub fn etat(&self) -> Etat {
        if !self.installe() {
            return Etat::PasInstalle;
        }
        let mut i = self.partage.verrou();
        if i.erreur.is_some() {
            return Etat::Erreur { message: crate::decision::MESSAGE_ECHEC.to_string() };
        }
        if !lance(&mut i) {
            return Etat::Repos;
        }
        if !i.pret {
            Etat::Demarre
        } else if i.questions > 0 {
            Etat::Reflechit
        } else {
            Etat::Pret
        }
    }

    /// Lance llama-server s'il ne tourne pas (`Start-Ia`) : appelé quand on se sert de l'assistant (on écrit dans la
    /// recherche, ou Entrée). Ça compte aussi comme « on s'en sert » (voir [`Assistant::arreter_si_inutile`]).
    pub fn demarrer(&self) {
        demarrer(&self.partage)
    }

    /// Pose la phrase (`Start-IaQuestion`) : lance llama-server s'il le faut, puis la question part sur un fil à
    /// part. `permises` : les Id des pannes du catalogue (la grammaire : il ne peut choisir qu'elles).
    pub fn poser(&self, phrase: &str, permises: &[String]) -> Question {
        let p = &self.partage;
        demarrer(p);
        let mut i = p.verrou();
        if !lance(&mut i) {
            if !p.reglages.paquet.installe() {
                return Question::SansAssistant;
            }
            if i.processus.is_some() && !i.arrete {
                noter_fin(p, &mut i);
            }
            let cause = i.erreur.clone().unwrap_or_else(|| "il ne démarre pas".to_string());
            drop(i);
            p.noter("Assistant : pas de réponse : il ne démarre pas");
            return Question::NeDemarrePas { cause };
        }
        let d = &p.reglages.delais;
        // (juste lancé : il charge encore son modèle, plus long la première fois ou sur un disque lent)
        let recent = p.horloge.maintenant().saturating_duration_since(i.lance) < d.recent;
        let attente = if recent { d.attente_premiere } else { d.attente };
        i.questions += 1;
        let generation = i.generation;
        drop(i);
        let resultat = Arc::new(Mutex::new(None));
        let annulee = Arc::new(AtomicBool::new(false));
        let debut = Instant::now();
        let (r, a, pp) = (resultat.clone(), annulee.clone(), p.clone());
        let (phrase, permises) = (phrase.to_string(), permises.to_vec());
        let lance_fil = thread::Builder::new().name("mindcare-assistant-question".into()).spawn(move || {
            let rep = question(&pp, generation, &phrase, &permises, debut, attente, &a);
            let mut i = pp.verrou();
            i.questions = i.questions.saturating_sub(1);
            i.derniere = pp.horloge.maintenant();
            if rep.erreur.is_empty() {
                i.erreur = None;
            }
            drop(i);
            *verrou(&r) = Some(rep);
        });
        if let Err(e) = lance_fil {
            let mut i = p.verrou();
            i.questions = i.questions.saturating_sub(1);
            drop(i);
            *verrou(&resultat) = Some(Reponse::vide(format!("pas de réponse : {e}")));
        }
        Question::EnCours(Tache { resultat, debut, attente, annulee, partage: Arc::downgrade(p) })
    }

    /// Arrête llama-server s'il n'a servi à rien depuis 5 minutes (`Stop-IaSiInutile`) : il rend la mémoire de la
    /// carte graphique et du PC. Relancé tout seul à la prochaine question. Vrai s'il vient d'être arrêté.
    pub fn arreter_si_inutile(&self, maintenant: Instant) -> bool {
        arreter_si_inutile(&self.partage, maintenant)
    }

    /// Arrête llama-server (`Stop-IaServeur`) : il sera relancé à la prochaine question
    pub fn arreter_serveur(&self) {
        arreter_serveur(&self.partage)
    }
}

impl Drop for Assistant {
    /// La fermeture de Mindcare (`Stop-Ia`). Si Mindcare est tuée, c'est le Job Object qui l'arrête.
    fn drop(&mut self) {
        arreter_serveur(&self.partage);
    }
}

fn lance(i: &mut Interne) -> bool {
    match i.processus.as_mut() {
        Some(p) => p.fin().is_none(),
        None => false,
    }
}

fn demarrer(p: &Arc<Partage>) {
    let maintenant = p.horloge.maintenant();
    let mut i = p.verrou();
    i.derniere = maintenant;
    if lance(&mut i) {
        return;
    }
    if i.processus.is_some() && !i.arrete {
        // Arrêté tout seul (pas par Mindcare) : la cause au journal une seule fois (son code, ses dernières lignes),
        // et pas de nouvel essai avant une minute (sinon un assistant qui plante serait relancé à chaque lettre)
        noter_fin(p, &mut i);
        if maintenant.saturating_duration_since(i.lance) < p.reglages.delais.relance {
            return;
        }
    }
    if !p.reglages.paquet.installe() {
        return;
    }
    match lancer(p, &mut i) {
        Ok((port, modele, programme, avertissement)) => {
            i.lance = maintenant;
            drop(i);
            if let Some(a) = avertissement {
                p.noter(&format!("Assistant : {a}"));
            }
            p.noter(&format!("Assistant lancé (port {port}, {modele}, {programme})"));
            surveiller(p);
            veiller(p);
        }
        Err(e) => {
            i.processus = None;
            i.erreur = Some(format!("lancement impossible ({e})"));
            drop(i);
            p.noter(&format!("Assistant : lancement impossible ({e})"));
        }
    }
}

/// Son arrêt tout seul, au journal une seule fois
fn noter_fin(p: &Partage, i: &mut Interne) {
    if i.fin_notee {
        return;
    }
    i.fin_notee = true;
    let Some(pr) = i.processus.as_mut() else { return };
    let code = match pr.fin() {
        Some(Some(c)) => c.to_string(),
        _ => "?".to_string(),
    };
    let lignes = pr.dernieres_lignes();
    let suite =
        if lignes.is_empty() { String::new() } else { format!(" ; ses dernières lignes : {}", lignes.join(" | ")) };
    i.erreur = Some(format!("arrêté tout seul (code {code})"));
    p.noter(&format!("Assistant arrêté tout seul (code {code}){suite}"));
}

fn nom(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Le lancement lui-même : port, programme, clé. Rend (port, modèle, programme, avertissement).
fn lancer(p: &Partage, i: &mut Interne) -> Result<(u16, String, String, Option<String>), String> {
    let paquet = &p.reglages.paquet;
    let modele = paquet.modele_lance().ok_or("pas de modèle")?;
    let programme = serveur_lance(p, i);
    let cle = p.lanceur.cle()?;
    // (une clé neuve à chaque lancement : un autre programme du PC ne peut pas questionner l'assistant ; par une
    // variable d'environnement, car la ligne de commande se lit dans le Gestionnaire des tâches ; /health reste ouvert)
    if cle.len() < 32 {
        return Err("clé trop courte".to_string());
    }
    // Le port : libre, et gardé (une socket ouverte dessus) jusqu'au tout dernier moment avant le lancement
    let reserve = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| format!("pas de port libre ({e})"))?;
    let port = reserve.local_addr().map_err(|e| format!("pas de port libre ({e})"))?.port();
    let ordre = Ordre {
        programme: programme.clone(),
        arguments: vec![
            "-m".to_string(),
            modele.to_string_lossy().into_owned(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            port.to_string(),
            "-ngl".to_string(),
            "99".to_string(),
            "-c".to_string(),
            "2048".to_string(),
            "-np".to_string(),
            "1".to_string(),
            "--no-webui".to_string(),
        ],
        environnement: vec![
            ("LLAMA_ARG_CHAT_TEMPLATE_KWARGS".to_string(), r#"{"enable_thinking":false}"#.to_string()),
            ("LLAMA_API_KEY".to_string(), cle.clone()),
        ],
        port,
    };
    drop(reserve);
    let l = p.lanceur.lancer(&ordre)?;
    i.processus = Some(l.processus);
    i.port = port;
    i.cle = cle;
    i.pret = false;
    i.arrete = false;
    i.fin_notee = false;
    i.erreur = None;
    i.generation += 1;
    Ok((port, nom(&modele), nom(&programme), l.avertissement))
}

/// Le programme à lancer (`Get-IaServeurLance`). Le Gestionnaire des tâches range sur une seule ligne « Mindset » les
/// programmes qui ont le même nom : l'assistant est donc lancé depuis une copie de llama-server.exe nommée
/// Mindset.exe, dans le même dossier (il y trouve ses .dll). La copie est refaite dès qu'elle n'est plus identique ;
/// vérifiée une fois par session (ensuite, tant que les deux fichiers n'ont pas changé, pas de nouvelle empreinte).
/// Si la copie est impossible : llama-server.exe, comme avant.
fn serveur_lance(p: &Partage, i: &mut Interne) -> PathBuf {
    let serveur = p.reglages.paquet.programme.clone();
    let copie = serveur.with_file_name("Mindset.exe");
    let r = (|| -> Result<PathBuf, String> {
        let s = fs::metadata(&serveur).map_err(|e| e.to_string())?;
        let vu = (s.len(), s.modified().ok());
        let taille_copie = fs::metadata(&copie).ok().map(|c| c.len());
        if i.copie_vue == Some(vu) && taille_copie == Some(s.len()) {
            return Ok(copie.clone());
        }
        let attendue = empreinte(&serveur, &mut i.empreintes)?;
        if taille_copie != Some(s.len()) || empreinte(&copie, &mut i.empreintes)? != attendue {
            fs::copy(&serveur, &copie).map_err(|e| e.to_string())?;
            if empreinte(&copie, &mut i.empreintes)? != attendue {
                return Err("copie différente de llama-server.exe".to_string());
            }
        }
        i.copie_vue = Some(vu);
        Ok(copie.clone())
    })();
    r.unwrap_or_else(|e| {
        p.noter(&format!("Assistant : copie Mindset.exe impossible, llama-server.exe lancé ({e})"));
        serveur
    })
}

fn empreinte(f: &Path, compte: &mut usize) -> Result<[u8; 32], String> {
    *compte += 1;
    let mut h = Sha256::new();
    let mut lu = fs::File::open(f).map_err(|e| e.to_string())?;
    let mut tampon = vec![0u8; 64 * 1024];
    loop {
        let n = lu.read(&mut tampon).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&tampon[..n]);
    }
    Ok(h.finalize().into())
}

/// Le fil qui attend qu'il soit prêt (`/health`), puis vérifie que c'est bien lui (la clé)
fn surveiller(p: &Arc<Partage>) {
    let faible = Arc::downgrade(p);
    let (generation, port, cle) = {
        let i = p.verrou();
        (i.generation, i.port, i.cle.clone())
    };
    let d = p.reglages.delais.clone();
    let _ = thread::Builder::new().name("mindcare-assistant-pret".into()).spawn(move || {
        loop {
            let Some(p) = faible.upgrade() else { return };
            {
                let mut i = p.verrou();
                if i.generation != generation || i.arrete {
                    return;
                }
                if !lance(&mut i) {
                    noter_fin(&p, &mut i);
                    return;
                }
            }
            drop(p);
            let sante = http::requete(port, "GET", "/health", None, None, d.delai_sonde);
            if matches!(sante, Ok(ref r) if r.statut == 200) {
                let verdict = verifier_cle(port, &cle, d.delai_sonde);
                let Some(p) = faible.upgrade() else { return };
                let mut i = p.verrou();
                if i.generation != generation || i.arrete {
                    return;
                }
                match verdict {
                    Ok(()) => {
                        i.pret = true;
                        return;
                    }
                    Err(cause) => {
                        // (notre llama-server a pu s'arrêter : le port pris par un autre programme, par exemple)
                        if !lance(&mut i) {
                            noter_fin(&p, &mut i);
                        }
                        i.erreur = Some(cause.clone());
                        let pr = i.processus.take();
                        i.arrete = true;
                        drop(i);
                        p.noter(&format!("Assistant : {cause} ; arrêté, aucune phrase envoyée"));
                        if let Some(mut pr) = pr {
                            pr.arreter();
                        }
                        return;
                    }
                }
            }
            thread::sleep(d.sonde);
        }
    });
}

/// Est-ce bien NOTRE llama-server qui répond sur ce port ? `/props` doit être refusé sans la clé (401) et accepté
/// avec (200). Un autre llama-server (une autre clé, ou aucune) ou un autre programme échoue.
fn verifier_cle(port: u16, cle: &str, delai: Duration) -> Result<(), String> {
    let sans = http::requete(port, "GET", "/props", None, None, delai).map_err(|e| e.to_string())?;
    let avec = http::requete(port, "GET", "/props", Some(cle), None, delai).map_err(|e| e.to_string())?;
    if sans.statut == 401 && avec.statut == 200 {
        Ok(())
    } else {
        Err(format!(
            "un autre programme répond sur le port {port}, ce n'est pas le nôtre (sans la clé : {}, avec : {})",
            sans.statut, avec.statut
        ))
    }
}

/// La question elle-même, sur son fil (le bloc de script de Start-IaQuestion)
fn question(
    p: &Arc<Partage>,
    generation: u64,
    phrase: &str,
    permises: &[String],
    debut: Instant,
    attente: Duration,
    annulee: &AtomicBool,
) -> Reponse {
    let d = &p.reglages.delais;
    // (l'assistant peut encore charger son modèle : on attend qu'il soit prêt, le temps de la réponse en moins)
    let fin_pret = debut + attente.saturating_sub(d.marge_reponse);
    let (port, cle) = loop {
        {
            let mut i = p.verrou();
            if i.generation != generation || i.processus.is_none() {
                let cause = i.erreur.clone().unwrap_or_else(|| "arrêté".to_string());
                return Reponse::vide(format!("pas de réponse : l'assistant s'est arrêté ({cause})"));
            }
            if i.pret {
                break (i.port, i.cle.clone());
            }
            if !lance(&mut i) {
                noter_fin(p, &mut i);
                let cause = i.erreur.clone().unwrap_or_default();
                return Reponse::vide(format!("pas de réponse : l'assistant s'est arrêté ({cause})"));
            }
        }
        if annulee.load(Ordering::SeqCst) || Instant::now() >= fin_pret {
            return Reponse::vide(format!(
                "pas de réponse : l'assistant n'était pas prêt après {} s",
                debut.elapsed().as_secs_f64().round()
            ));
        }
        thread::sleep(Duration::from_millis(50));
    };
    let corps = corps_question(phrase, permises);
    let r = http::requete(port, "POST", "/v1/chat/completions", Some(&cle), Some(&corps), d.requete);
    let echec = |cause: String| -> Reponse {
        // (la vraie cause : l'erreur de la requête, ou l'assistant arrêté)
        let mut i = p.verrou();
        let cause = if lance(&mut i) || i.generation != generation {
            cause
        } else {
            noter_fin(p, &mut i);
            format!("l'assistant s'est arrêté ({cause})")
        };
        Reponse::vide(format!("pas de réponse : {cause}"))
    };
    let r = match r {
        Ok(r) => r,
        Err(e) => return echec(e.to_string()),
    };
    if r.statut == 401 {
        return echec("l'assistant a refusé la clé : ce n'est pas le nôtre".to_string());
    }
    if r.statut != 200 {
        let corps = String::from_utf8_lossy(&r.corps);
        let extrait: String = corps.chars().take(200).collect();
        return echec(format!("erreur {} de l'assistant ({extrait})", r.statut));
    }
    match lire_completion(&r.corps) {
        Ok((c, conf)) if !c.is_empty() => lire_reponse(&c, conf),
        Ok(_) => echec("réponse vide".to_string()),
        Err(e) => Reponse::vide(e),
    }
}

fn arreter_si_inutile(p: &Arc<Partage>, maintenant: Instant) -> bool {
    let mut i = p.verrou();
    if !lance(&mut i) || i.questions > 0 {
        return false;
    }
    if maintenant.saturating_duration_since(i.derniere) < p.reglages.delais.repos {
        return false;
    }
    drop(i);
    let minutes = p.reglages.delais.repos.as_secs() / 60;
    p.noter(&format!("Assistant arrêté : pas de question depuis {minutes} minutes (relancé à la prochaine)"));
    arreter_serveur(p);
    true
}

fn arreter_serveur(p: &Partage) {
    let mut i = p.verrou();
    // (arrêté par Mindcare : relancé dès la prochaine question)
    i.arrete = true;
    i.pret = false;
    i.generation += 1;
    let pr = i.processus.take();
    drop(i);
    if let Some(mut pr) = pr {
        pr.arreter();
    }
}

/// La minuterie : toutes les 30 secondes, arrêté s'il ne sert plus. Un seul fil, qui s'arrête avec l'assistant.
fn veiller(p: &Arc<Partage>) {
    {
        let mut i = p.verrou();
        if i.veilleur {
            return;
        }
        i.veilleur = true;
    }
    let faible = Arc::downgrade(p);
    let periode = p.reglages.delais.veille;
    let lance_fil = thread::Builder::new().name("mindcare-assistant-veille".into()).spawn(move || {
        loop {
            // (attendre par petits bouts : l'assistant fermé, le fil s'arrête vite)
            let fin = Instant::now() + periode;
            while Instant::now() < fin {
                if faible.strong_count() == 0 {
                    return;
                }
                thread::sleep(periode.min(Duration::from_millis(200)));
            }
            let Some(p) = faible.upgrade() else { return };
            let maintenant = p.horloge.maintenant();
            arreter_si_inutile(&p, maintenant);
        }
    });
    if lance_fil.is_err() {
        p.verrou().veilleur = false;
    }
}

/// Les dernières lignes d'un programme (20 au plus), remplies depuis ses sorties sur d'autres fils
#[derive(Clone, Default)]
pub struct DernieresLignes {
    lignes: Arc<Mutex<VecDeque<String>>>,
}

impl DernieresLignes {
    pub const MAX: usize = 20;

    pub fn ajouter(&self, l: String) {
        let mut v = verrou(&self.lignes);
        v.push_back(l);
        while v.len() > Self::MAX {
            v.pop_front();
        }
    }

    pub fn toutes(&self) -> Vec<String> {
        verrou(&self.lignes).iter().cloned().collect()
    }

    /// Lit un flux ligne par ligne sur un fil à part, jusqu'à sa fin ; rend le fil (fini quand le flux l'est)
    pub fn lire<R: Read + Send + 'static>(&self, flux: R) -> Option<thread::JoinHandle<()>> {
        let moi = self.clone();
        thread::Builder::new()
            .name("mindcare-assistant-sorties".into())
            .spawn(move || {
                let mut l = std::io::BufReader::new(flux);
                let mut tampon = Vec::new();
                loop {
                    tampon.clear();
                    match std::io::BufRead::read_until(&mut l, b'\n', &mut tampon) {
                        Ok(0) | Err(_) => return,
                        Ok(_) => {
                            let t = String::from_utf8_lossy(&tampon);
                            let t = t.trim_end_matches(['\r', '\n']);
                            if !t.is_empty() {
                                moi.ajouter(t.to_string());
                            }
                        }
                    }
                }
            })
            .ok()
    }
}
