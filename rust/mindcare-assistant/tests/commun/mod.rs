//! Ce que les tests partagent : un faux llama-server en Rust (std seulement, sur 127.0.0.1), un faux lanceur qui le
//! démarre dans le test lui-même, un faux assistant installé dans un dossier d'essai, le journal en mémoire.
//! Rien n'est téléchargé, rien n'est lancé sur le vrai PC, aucune fenêtre.

#![allow(dead_code)]

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use mindcare_assistant::{Assistant, Delais, Horloge, HorlogeSimulee, Lance, Lanceur, Ordre, Processus, Reglages};
use mindcare_telechargement::Paquet;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------------------------------
// Le faux llama-server
// ---------------------------------------------------------------------------------------------------------------

/// Comment répond le faux llama-server (comme le vrai : /health ouvert, le reste fermé sans la clé)
#[derive(Clone, Debug, Default)]
pub struct Comportement {
    /// La clé attendue ; `None` : il accepte tout (un llama-server lancé sans clé)
    pub cle: Option<String>,
    /// Il charge son modèle pendant ce temps (503 partout)
    pub chargement: Duration,
    /// Le texte de sa réponse (`choices[0].message.content`)
    pub contenu: String,
    /// Les probabilités des jetons (`logprobs.content`) ; `None` : une liste vide
    pub jetons: Option<Value>,
    /// Une réponse entière à la place (code, corps)
    pub brut: Option<(u16, String)>,
    /// Il attend ce temps avant de répondre à la question
    pub lenteur: Duration,
    /// Il arrête d'écouter dès que sa clé a été vérifiée (la connexion de la question est refusée)
    pub fermer_apres_verification: bool,
    /// Sa réponse en morceaux (Transfer-Encoding: chunked)
    pub morceaux: bool,
}

/// Une requête reçue
#[derive(Clone, Debug)]
pub struct Recu {
    pub methode: String,
    pub chemin: String,
    pub autorisation: Option<String>,
    pub corps: String,
}

pub struct FauxServeur {
    pub port: u16,
    arret: Arc<AtomicBool>,
    pub recus: Arc<Mutex<Vec<Recu>>>,
    fil: Option<thread::JoinHandle<()>>,
}

impl FauxServeur {
    /// Démarre sur ce port (0 : un port libre)
    pub fn demarrer(port: u16, c: Comportement) -> std::io::Result<FauxServeur> {
        let ecoute = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let port = ecoute.local_addr()?.port();
        ecoute.set_nonblocking(true)?;
        let arret = Arc::new(AtomicBool::new(false));
        let recus = Arc::new(Mutex::new(Vec::new()));
        let (a, r) = (arret.clone(), recus.clone());
        let debut = Instant::now();
        let fil = thread::spawn(move || {
            while !a.load(Ordering::SeqCst) {
                match ecoute.accept() {
                    Ok((s, _)) => {
                        let (c, a, r) = (c.clone(), a.clone(), r.clone());
                        thread::spawn(move || servir(s, &c, debut, &a, &r));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
            // (l'écoute fermée en sortant : le port ne répond plus)
        });
        Ok(FauxServeur { port, arret, recus, fil: Some(fil) })
    }

    pub fn arreter(&mut self) {
        self.arret.store(true, Ordering::SeqCst);
        if let Some(f) = self.fil.take() {
            let _ = f.join();
        }
    }

    /// Les phrases reçues (les questions)
    pub fn questions(&self) -> Vec<Recu> {
        self.recus.lock().unwrap().iter().filter(|r| r.chemin == "/v1/chat/completions").cloned().collect()
    }
}

impl Drop for FauxServeur {
    fn drop(&mut self) {
        self.arreter();
    }
}

/// Une requête lue : la méthode, le chemin, les en-têtes (noms en minuscules), le corps
type Lue = (String, String, Vec<(String, String)>, Vec<u8>);

fn lire_requete(s: &mut TcpStream) -> Option<Lue> {
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut recu = Vec::new();
    let mut m = [0u8; 4096];
    let fin_tete = loop {
        if let Some(i) = recu.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        let n = s.read(&mut m).ok()?;
        if n == 0 {
            return None;
        }
        recu.extend_from_slice(&m[..n]);
    };
    let tete = String::from_utf8_lossy(&recu[..fin_tete]).into_owned();
    let mut lignes = tete.split("\r\n");
    let premiere = lignes.next()?.to_string();
    let mut mots = premiere.split(' ');
    let methode = mots.next()?.to_string();
    let chemin = mots.next()?.to_string();
    let entetes: Vec<(String, String)> = lignes
        .filter_map(|l| l.split_once(':').map(|(a, b)| (a.trim().to_ascii_lowercase(), b.trim().to_string())))
        .collect();
    let longueur: usize =
        entetes.iter().find(|(k, _)| k == "content-length").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    let mut corps = recu[fin_tete + 4..].to_vec();
    while corps.len() < longueur {
        let n = s.read(&mut m).ok()?;
        if n == 0 {
            break;
        }
        corps.extend_from_slice(&m[..n]);
    }
    Some((methode, chemin, entetes, corps))
}

fn repondre(s: &mut TcpStream, statut: u16, corps: &str, morceaux: bool) {
    let mot = match statut {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        503 => "Service Unavailable",
        _ => "Erreur",
    };
    let o = corps.as_bytes();
    let mut t =
        format!("HTTP/1.1 {statut} {mot}\r\nContent-Type: application/json; charset=utf-8\r\nConnection: close\r\n");
    if morceaux {
        t.push_str("Transfer-Encoding: chunked\r\n\r\n");
        let mut b = t.into_bytes();
        for m in o.chunks(7) {
            b.extend_from_slice(format!("{:x}\r\n", m.len()).as_bytes());
            b.extend_from_slice(m);
            b.extend_from_slice(b"\r\n");
        }
        b.extend_from_slice(b"0\r\n\r\n");
        let _ = s.write_all(&b);
    } else {
        t.push_str(&format!("Content-Length: {}\r\n\r\n", o.len()));
        let mut b = t.into_bytes();
        b.extend_from_slice(o);
        let _ = s.write_all(&b);
    }
    let _ = s.flush();
}

fn servir(mut s: TcpStream, c: &Comportement, debut: Instant, arret: &AtomicBool, recus: &Mutex<Vec<Recu>>) {
    let _ = s.set_nonblocking(false);
    let Some((methode, chemin, entetes, corps)) = lire_requete(&mut s) else { return };
    let autorisation = entetes.iter().find(|(k, _)| k == "authorization").map(|(_, v)| v.clone());
    recus.lock().unwrap().push(Recu {
        methode: methode.clone(),
        chemin: chemin.clone(),
        autorisation: autorisation.clone(),
        corps: String::from_utf8_lossy(&corps).into_owned(),
    });
    let charge = debut.elapsed() < c.chargement;
    if chemin == "/health" || chemin == "/v1/health" {
        if charge {
            repondre(
                &mut s,
                503,
                r#"{"error":{"code":503,"message":"Loading model","type":"unavailable_error"}}"#,
                false,
            );
        } else {
            repondre(&mut s, 200, r#"{"status":"ok"}"#, false);
        }
        return;
    }
    if charge {
        repondre(&mut s, 503, r#"{"error":{"code":503,"message":"Loading model","type":"unavailable_error"}}"#, false);
        return;
    }
    if let Some(cle) = &c.cle {
        let x_api = entetes.iter().find(|(k, _)| k == "x-api-key").map(|(_, v)| v.clone());
        let bonne = autorisation.as_deref() == Some(&format!("Bearer {cle}")) || x_api.as_deref() == Some(cle);
        if !bonne {
            repondre(
                &mut s,
                401,
                r#"{"error":{"code":401,"message":"Invalid API Key","type":"authentication_error"}}"#,
                false,
            );
            return;
        }
    }
    match (methode.as_str(), chemin.as_str()) {
        ("GET", "/props") => {
            // (il arrête d'écouter avant même de répondre : la connexion de la question sera refusée)
            if c.fermer_apres_verification && autorisation.is_some() {
                arret.store(true, Ordering::SeqCst);
            }
            repondre(&mut s, 200, r#"{"default_generation_settings":{},"total_slots":1}"#, false);
        }
        ("POST", "/v1/chat/completions") => {
            thread::sleep(c.lenteur);
            if let Some((statut, b)) = &c.brut {
                repondre(&mut s, *statut, b, c.morceaux);
                return;
            }
            let r = json!({
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": c.contenu },
                    "logprobs": { "content": c.jetons.clone().unwrap_or(json!([])) },
                    "finish_reason": "stop",
                }],
                "object": "chat.completion",
            });
            repondre(&mut s, 200, &r.to_string(), c.morceaux);
        }
        _ => repondre(
            &mut s,
            404,
            r#"{"error":{"code":404,"message":"File Not Found","type":"not_found_error"}}"#,
            false,
        ),
    }
}

/// Le port répond-il encore ?
pub fn repond(port: u16) -> bool {
    TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_millis(300)).is_ok()
}

/// Attend qu'une condition soit vraie (ou le délai passé)
pub fn attendre(delai: Duration, mut f: impl FnMut() -> bool) -> bool {
    let fin = Instant::now() + delai;
    while Instant::now() < fin {
        if f() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    f()
}

// ---------------------------------------------------------------------------------------------------------------
// Le faux lanceur : le faux llama-server dans le test lui-même
// ---------------------------------------------------------------------------------------------------------------

/// Ce que fait le « programme » lancé
#[derive(Clone, Debug)]
pub enum Scenario {
    /// Un llama-server qui marche, avec ce comportement (sa clé : celle de l'ordre)
    Normal(Comportement),
    /// Il écrit 25 lignes d'erreur (dont sa clé et ses arguments), puis s'arrête avec le code 3
    Plante,
    /// Le fichier n'est pas un programme : le lancement échoue
    PasUnProgramme,
    /// Il tourne, mais n'écoute jamais (il ne démarre pas)
    Muet,
    /// Un autre programme a pris le port juste avant : notre llama-server s'arrête (code 1)
    PortVole(Comportement),
    /// Il tourne sans écouter, et un autre programme répond sur son port
    Etranger(Comportement),
}

pub struct FauxProcessus {
    id: u32,
    serveur: Option<FauxServeur>,
    code: Option<i32>,
    lignes: Vec<String>,
    pub tue: Arc<AtomicBool>,
}

impl Processus for FauxProcessus {
    fn id(&self) -> u32 {
        self.id
    }
    fn fin(&mut self) -> Option<Option<i32>> {
        if self.tue.load(Ordering::SeqCst) {
            return Some(Some(1));
        }
        self.code.map(Some)
    }
    fn arreter(&mut self) {
        self.tue.store(true, Ordering::SeqCst);
        if let Some(mut s) = self.serveur.take() {
            s.arreter();
        }
    }
    fn dernieres_lignes(&mut self) -> Vec<String> {
        let n = self.lignes.len();
        self.lignes[n.saturating_sub(20)..].to_vec()
    }
}

pub struct FauxLanceur {
    pub scenario: Mutex<Scenario>,
    pub ordres: Mutex<Vec<Ordre>>,
    /// Les faux llama-server lancés (leur port) et s'ils ont été arrêtés
    pub lances: Mutex<Vec<(u16, Arc<AtomicBool>)>>,
    /// Les autres programmes (pas les nôtres) et ce qu'ils ont reçu
    pub etrangers: Mutex<Vec<FauxServeur>>,
    /// Ce qu'ont reçu nos faux llama-server
    pub recus: Mutex<Vec<Arc<Mutex<Vec<Recu>>>>>,
    n: AtomicU32,
}

impl FauxLanceur {
    pub fn nouveau(s: Scenario) -> Arc<FauxLanceur> {
        Arc::new(FauxLanceur {
            scenario: Mutex::new(s),
            ordres: Mutex::new(Vec::new()),
            lances: Mutex::new(Vec::new()),
            etrangers: Mutex::new(Vec::new()),
            recus: Mutex::new(Vec::new()),
            n: AtomicU32::new(0),
        })
    }
    pub fn nb_lancements(&self) -> usize {
        self.ordres.lock().unwrap().len()
    }
    pub fn dernier_ordre(&self) -> Ordre {
        self.ordres.lock().unwrap().last().cloned().expect("aucun lancement")
    }
    /// Les phrases reçues par les autres programmes (jamais aucune : elles ne doivent pas sortir)
    /// Les questions reçues par nos faux llama-server
    pub fn questions(&self) -> Vec<Recu> {
        let r = self.recus.lock().unwrap();
        r.iter().flat_map(|x| x.lock().unwrap().clone()).filter(|q| q.chemin == "/v1/chat/completions").collect()
    }
    pub fn phrases_aux_etrangers(&self) -> usize {
        self.etrangers.lock().unwrap().iter().map(|e| e.questions().len()).sum()
    }
}

/// Les lignes d'erreur du faux llama-server qui plante (comme tester-assistant.ps1, `New-FauxAssistant -Plante`)
pub fn lignes_plante(cle: Option<&str>, arguments: &[String]) -> Vec<String> {
    let mut l: Vec<String> = (1..=23).map(|i| format!("ligne {i}")).collect();
    l.push(format!("cle={}", cle.unwrap_or("absente")));
    l.push(format!("arguments={}", arguments.join(" ")));
    l
}

impl Lanceur for FauxLanceur {
    fn lancer(&self, ordre: &Ordre) -> Result<Lance, String> {
        self.ordres.lock().unwrap().push(ordre.clone());
        let id = 1000 + self.n.fetch_add(1, Ordering::SeqCst);
        let tue = Arc::new(AtomicBool::new(false));
        let cle = ordre.variable("LLAMA_API_KEY").map(str::to_string);
        let mut p = FauxProcessus { id, serveur: None, code: None, lignes: Vec::new(), tue: tue.clone() };
        match self.scenario.lock().unwrap().clone() {
            Scenario::Normal(mut c) => {
                c.cle = cle;
                let s = FauxServeur::demarrer(ordre.port, c).map_err(|e| e.to_string())?;
                self.recus.lock().unwrap().push(s.recus.clone());
                p.serveur = Some(s);
            }
            Scenario::Plante => {
                p.lignes = lignes_plante(cle.as_deref(), &ordre.arguments);
                p.code = Some(3);
            }
            Scenario::PasUnProgramme => {
                return Err("%1 n’est pas une application Win32 valide. (os error 193)".to_string());
            }
            Scenario::Muet => {}
            Scenario::PortVole(c) => {
                self.etrangers.lock().unwrap().push(FauxServeur::demarrer(ordre.port, c).map_err(|e| e.to_string())?);
                p.lignes = vec![format!("couldn't bind HTTP server socket, hostname: 127.0.0.1, port: {}", ordre.port)];
                p.code = Some(1);
            }
            Scenario::Etranger(c) => {
                self.etrangers.lock().unwrap().push(FauxServeur::demarrer(ordre.port, c).map_err(|e| e.to_string())?);
            }
        }
        self.lances.lock().unwrap().push((ordre.port, tue));
        Ok(Lance { processus: Box::new(p), avertissement: None })
    }

    fn cle(&self) -> Result<String, String> {
        // (les tests : une clé neuve à chaque fois, la vraie vient de BCryptGenRandom)
        let mut h = RandomState::new().build_hasher();
        h.write_u32(self.n.load(Ordering::SeqCst));
        let a = h.finish();
        let mut h = RandomState::new().build_hasher();
        h.write_u64(a);
        Ok(format!("{a:016x}{:016x}{a:016x}{:016x}", h.finish(), h.finish().rotate_left(7)))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Le faux assistant installé, le journal
// ---------------------------------------------------------------------------------------------------------------

/// Un dossier d'essai (dans le dossier de construction, sur D:), effacé à la fin
pub struct Essai(pub PathBuf);

impl Essai {
    pub fn nouveau(nom: &str) -> Essai {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "assistant-{}-{nom}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Essai(d)
    }
}

impl Drop for Essai {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Un faux assistant installé dans `donnees\ia` : llama-server.exe (`programme`), ses .dll et le modèle
pub fn installer(donnees: &Path, programme: &[u8]) -> Paquet {
    let paquet = Paquet::assistant(donnees);
    std::fs::create_dir_all(&paquet.dossier).unwrap();
    std::fs::write(&paquet.programme, programme).unwrap();
    for d in &paquet.dll {
        std::fs::write(paquet.dossier.join(d), b"faux").unwrap();
    }
    std::fs::write(&paquet.modele, b"faux").unwrap();
    paquet
}

/// Le journal en mémoire
#[derive(Clone, Default)]
pub struct Journal(pub Arc<Mutex<Vec<String>>>);

impl Journal {
    pub fn fonction(&self) -> Arc<dyn Fn(&str) + Send + Sync> {
        let j = self.0.clone();
        Arc::new(move |t: &str| j.lock().unwrap().push(t.to_string()))
    }
    pub fn texte(&self) -> String {
        self.0.lock().unwrap().join("\n")
    }
    pub fn compte(&self, motif: &str) -> usize {
        self.0.lock().unwrap().iter().map(|l| l.matches(motif).count()).sum()
    }
}

/// Des délais courts pour les tests (5 minutes de repos quand même : l'horloge est simulée)
pub fn delais_courts() -> Delais {
    Delais {
        repos: Duration::from_secs(5 * 60),
        veille: Duration::from_secs(30),
        relance: Duration::from_secs(60),
        recent: Duration::from_secs(60),
        attente_premiere: Duration::from_secs(6),
        attente: Duration::from_secs(5),
        marge_reponse: Duration::from_secs(2),
        requete: Duration::from_secs(3),
        sonde: Duration::from_millis(20),
        delai_sonde: Duration::from_secs(1),
    }
}

/// Un assistant d'essai : un faux assistant installé, le faux lanceur, l'horloge simulée
pub struct Banc {
    pub essai: Essai,
    pub lanceur: Arc<FauxLanceur>,
    pub horloge: Arc<HorlogeSimulee>,
    pub journal: Journal,
    pub assistant: Assistant,
}

impl Banc {
    pub fn nouveau(nom: &str, scenario: Scenario, delais: Delais) -> Banc {
        let essai = Essai::nouveau(nom);
        let paquet = installer(&essai.0, b"faux llama-server");
        let lanceur = FauxLanceur::nouveau(scenario);
        let horloge = Arc::new(HorlogeSimulee::nouvelle());
        let journal = Journal::default();
        let assistant = Assistant::nouveau(
            Reglages { paquet, delais },
            lanceur.clone(),
            horloge.clone() as Arc<dyn Horloge>,
            journal.fonction(),
        );
        Banc { essai, lanceur, horloge, journal, assistant }
    }
}

/// La réponse du faux llama-server de tester-assistant.ps1 (« la question est envoyée avec la clé du lancement »)
pub fn comportement_bruit() -> Comportement {
    Comportement { contenu: r#"{"panne":"bruit","reponses":{}}"#.to_string(), ..Comportement::default() }
}
