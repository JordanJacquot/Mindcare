//! Ce qui sert à tous les tests : un faux serveur sur ce PC (127.0.0.1, rien sur Internet), comme celui de
//! outils\tester-assistant.ps1, des zips faits à la main, un dossier d'essai effacé à la fin.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flate2::Crc;
use flate2::write::DeflateEncoder;
use mindcare_telechargement::{Disque, Extraire, Fichier};
use sha2::{Digest, Sha256};

/// Une demande reçue par le faux serveur
#[derive(Clone, Debug, PartialEq)]
pub struct Demande {
    pub chemin: String,
    /// L'en-tête « Range: bytes=N- », s'il y en a un
    pub depuis: Option<u64>,
}

/// Ce que le faux serveur répond
pub enum Action {
    /// Le fichier. `reprise` : il répond 206 avec la suite quand on la demande (sinon 200 et tout le fichier) ;
    /// `couper_apres` : la connexion se coupe après ce nombre d'octets ; `lent` : une pause tous les 4 Ko.
    Donner { octets: Arc<Vec<u8>>, reprise: bool, couper_apres: Option<usize>, lent: Option<Duration> },
    /// Une réponse sans fichier (404...)
    Statut(u16),
}

impl Action {
    pub fn fichier(octets: &Arc<Vec<u8>>) -> Action {
        Action::Donner { octets: octets.clone(), reprise: true, couper_apres: None, lent: None }
    }
}

type Regle = dyn Fn(&Demande, usize) -> Action + Send + Sync;

/// Le faux serveur : chaque demande est notée, puis la règle dit quoi répondre (elle reçoit aussi le numéro de la
/// demande, à partir de 0)
pub struct FauxServeur {
    pub port: u16,
    pub demandes: Arc<Mutex<Vec<Demande>>>,
    arret: Arc<AtomicBool>,
}

impl FauxServeur {
    pub fn lancer(regle: impl Fn(&Demande, usize) -> Action + Send + Sync + 'static) -> FauxServeur {
        let ecoute = TcpListener::bind("127.0.0.1:0").expect("faux serveur");
        let port = ecoute.local_addr().unwrap().port();
        let demandes = Arc::new(Mutex::new(Vec::new()));
        let arret = Arc::new(AtomicBool::new(false));
        let regle: Arc<Regle> = Arc::new(regle);
        let (d, a) = (demandes.clone(), arret.clone());
        let compte = Arc::new(AtomicUsize::new(0));
        std::thread::spawn(move || {
            for c in ecoute.incoming() {
                if a.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(c) = c else { continue };
                let (d, regle, compte) = (d.clone(), regle.clone(), compte.clone());
                std::thread::spawn(move || servir(c, &d, &*regle, &compte));
            }
        });
        FauxServeur { port, demandes, arret }
    }

    pub fn url(&self, nom: &str) -> String {
        format!("http://127.0.0.1:{}/{nom}", self.port)
    }

    pub fn demandes(&self) -> Vec<Demande> {
        self.demandes.lock().unwrap().clone()
    }
}

impl Drop for FauxServeur {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn servir(mut c: TcpStream, demandes: &Mutex<Vec<Demande>>, regle: &Regle, compte: &AtomicUsize) {
    let mut lecteur = BufReader::new(c.try_clone().unwrap());
    let mut ligne = String::new();
    if lecteur.read_line(&mut ligne).unwrap_or(0) == 0 {
        return;
    }
    let chemin = ligne.split(' ').nth(1).unwrap_or("/").trim_start_matches('/').to_string();
    let mut depuis = None;
    loop {
        let mut l = String::new();
        if lecteur.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
            break;
        }
        let bas = l.to_ascii_lowercase();
        if let Some(r) = bas.strip_prefix("range: bytes=") {
            depuis = r.trim().trim_end_matches('-').parse::<u64>().ok();
        }
    }
    let demande = Demande { chemin, depuis };
    demandes.lock().unwrap().push(demande.clone());
    let n = compte.fetch_add(1, Ordering::SeqCst);
    match regle(&demande, n) {
        Action::Statut(s) => {
            let _ = write!(c, "HTTP/1.1 {s} Non\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
        Action::Donner { octets, reprise, couper_apres, lent } => {
            let longueur = octets.len();
            let d = demande.depuis.map(|d| d as usize).filter(|&d| reprise && d < longueur);
            let corps = &octets[d.unwrap_or(0)..];
            let tete = match d {
                Some(d) => format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {d}-{}/{longueur}\r\nConnection: close\r\n\r\n",
                    corps.len(),
                    longueur - 1
                ),
                None => format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", corps.len()),
            };
            if c.write_all(tete.as_bytes()).is_err() {
                return;
            }
            let corps = &corps[..couper_apres.unwrap_or(corps.len()).min(corps.len())];
            for morceau in corps.chunks(4096) {
                if c.write_all(morceau).is_err() {
                    return;
                }
                if let Some(p) = lent {
                    let _ = c.flush();
                    std::thread::sleep(p);
                }
            }
        }
    }
    let _ = c.flush();
    let _ = c.shutdown(Shutdown::Both);
}

/// Un dossier d'essai, effacé à la fin
pub struct Essai(pub PathBuf);

impl Essai {
    pub fn nouveau(nom: &str) -> Essai {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!(
            "MindcareTelechargement-{}-{nom}-{}",
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

/// Une fausse place libre
pub struct Place(pub Option<u64>);

impl Disque for Place {
    fn place_libre(&self, _dossier: &Path) -> Option<u64> {
        self.0
    }
}

/// Des octets qui ne se répètent pas trop (pour que deflate ait du travail et qu'un décalage se voie)
pub fn octets(n: usize, graine: u32) -> Vec<u8> {
    let mut x = graine.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..n)
        .map(|i| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            if i % 3 == 0 { b'a' + (x % 7) as u8 } else { x as u8 }
        })
        .collect()
}

pub fn sha256(o: &[u8]) -> String {
    Sha256::digest(o).iter().map(|b| format!("{b:02X}")).collect()
}

/// Un fichier à télécharger depuis le faux serveur
pub fn fichier(serveur: &FauxServeur, nom_url: &str, o: &[u8], garder: &Path, extraire: Option<Extraire>) -> Fichier {
    Fichier {
        nom: format!("fichier {nom_url}"),
        url: serveur.url(nom_url),
        sha256: sha256(o),
        taille: o.len() as u64,
        extraire,
        garder: garder.to_path_buf(),
    }
}

/// Un zip fait à la main : (chemin, contenu, compressé par deflate ?). `zip64` : les tailles, les positions et le
/// nombre de fichiers écrits en 64 bits, comme un très gros zip.
pub fn zip(entrees: &[(&str, &[u8], bool)], zip64: bool) -> Vec<u8> {
    let mut z = Vec::new();
    let mut sommaire = Vec::new();
    for (chemin, contenu, deflate) in entrees {
        let mut crc = Crc::new();
        crc.update(contenu);
        let donnees = if *deflate {
            let mut e = DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(contenu).unwrap();
            e.finish().unwrap()
        } else {
            contenu.to_vec()
        };
        let methode: u16 = if *deflate { 8 } else { 0 };
        let decalage = z.len() as u64;
        let nom = chemin.as_bytes();
        // en-tête local
        z.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        z.extend_from_slice(&20u16.to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(&methode.to_le_bytes());
        z.extend_from_slice(&[0; 4]);
        z.extend_from_slice(&crc.sum().to_le_bytes());
        z.extend_from_slice(&(donnees.len() as u32).to_le_bytes());
        z.extend_from_slice(&(contenu.len() as u32).to_le_bytes());
        z.extend_from_slice(&(nom.len() as u16).to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(nom);
        z.extend_from_slice(&donnees);
        // ligne du sommaire
        let mut extra = Vec::new();
        let (t, tc, dec) = if zip64 {
            extra.extend_from_slice(&1u16.to_le_bytes());
            extra.extend_from_slice(&24u16.to_le_bytes());
            extra.extend_from_slice(&(contenu.len() as u64).to_le_bytes());
            extra.extend_from_slice(&(donnees.len() as u64).to_le_bytes());
            extra.extend_from_slice(&decalage.to_le_bytes());
            (u32::MAX, u32::MAX, u32::MAX)
        } else {
            (contenu.len() as u32, donnees.len() as u32, decalage as u32)
        };
        sommaire.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        sommaire.extend_from_slice(&45u16.to_le_bytes());
        sommaire.extend_from_slice(&20u16.to_le_bytes());
        sommaire.extend_from_slice(&0u16.to_le_bytes());
        sommaire.extend_from_slice(&methode.to_le_bytes());
        sommaire.extend_from_slice(&[0; 4]);
        sommaire.extend_from_slice(&crc.sum().to_le_bytes());
        sommaire.extend_from_slice(&tc.to_le_bytes());
        sommaire.extend_from_slice(&t.to_le_bytes());
        sommaire.extend_from_slice(&(nom.len() as u16).to_le_bytes());
        sommaire.extend_from_slice(&(extra.len() as u16).to_le_bytes());
        // (commentaire, disque, attributs : 10 octets)
        sommaire.extend_from_slice(&[0; 10]);
        sommaire.extend_from_slice(&dec.to_le_bytes());
        sommaire.extend_from_slice(nom);
        sommaire.extend_from_slice(&extra);
    }
    let debut = z.len() as u64;
    z.extend_from_slice(&sommaire);
    let nombre = entrees.len() as u64;
    if zip64 {
        let ou = z.len() as u64;
        z.extend_from_slice(&[0x50, 0x4b, 0x06, 0x06]);
        z.extend_from_slice(&44u64.to_le_bytes());
        z.extend_from_slice(&45u16.to_le_bytes());
        z.extend_from_slice(&45u16.to_le_bytes());
        z.extend_from_slice(&[0; 8]);
        z.extend_from_slice(&nombre.to_le_bytes());
        z.extend_from_slice(&nombre.to_le_bytes());
        z.extend_from_slice(&(sommaire.len() as u64).to_le_bytes());
        z.extend_from_slice(&debut.to_le_bytes());
        z.extend_from_slice(&[0x50, 0x4b, 0x06, 0x07]);
        z.extend_from_slice(&0u32.to_le_bytes());
        z.extend_from_slice(&ou.to_le_bytes());
        z.extend_from_slice(&1u32.to_le_bytes());
    }
    z.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
    z.extend_from_slice(&[0; 4]);
    let (n16, taille32, debut32) =
        if zip64 { (u16::MAX, u32::MAX, u32::MAX) } else { (nombre as u16, sommaire.len() as u32, debut as u32) };
    z.extend_from_slice(&n16.to_le_bytes());
    z.extend_from_slice(&n16.to_le_bytes());
    z.extend_from_slice(&taille32.to_le_bytes());
    z.extend_from_slice(&debut32.to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z
}

/// Tous les fichiers sous un dossier (chemins complets)
pub fn tous_les_fichiers(d: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(l) = std::fs::read_dir(d) {
        for e in l.flatten() {
            let p = e.path();
            if p.is_dir() { v.extend(tous_les_fichiers(&p)) } else { v.push(p) }
        }
    }
    v.sort();
    v
}
