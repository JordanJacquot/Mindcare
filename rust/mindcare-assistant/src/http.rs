//! Un tout petit client HTTP vers l'assistant, seulement sur ce PC (127.0.0.1), sans TLS : ce que faisaient
//! `Invoke-WebRequest` et `Invoke-RestMethod` dans modules\ia.ps1.
//!
//! Pourquoi pas WinHTTP (comme mindcare-telechargement) : ici, il n'y a ni https, ni proxy, ni redirection, ni
//! reprise. Une socket de la bibliothèque standard suffit, en Rust sûr (aucun code « unsafe »), avec des délais
//! précis par requête, et elle ne peut parler qu'à 127.0.0.1 : l'adresse n'est même pas un paramètre. Elle marche
//! aussi dans les tests sans Windows, contre le faux llama-server.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// Une réponse : son code (200, 401, 503...) et son corps
#[derive(Clone, Debug, PartialEq)]
pub struct Reponse {
    pub statut: u16,
    pub corps: Vec<u8>,
}

/// Une erreur : la vraie cause, pour le journal
#[derive(Clone, Debug, PartialEq)]
pub enum Erreur {
    /// Personne n'écoute sur ce port, ou la connexion a été refusée ou coupée
    Connexion(String),
    /// Pas de réponse complète dans le délai
    Delai(Duration),
    /// Une réponse qui n'est pas du HTTP lisible
    Illisible(String),
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::Connexion(c) => write!(f, "connexion à l'assistant impossible ({c})"),
            Erreur::Delai(d) => write!(f, "l'assistant n'a pas répondu en {} s", d.as_secs_f64().round()),
            Erreur::Illisible(c) => write!(f, "réponse HTTP illisible ({c})"),
        }
    }
}

/// Une requête vers 127.0.0.1:`port`. `cle` : l'en-tête `Authorization: Bearer ...`. Le délai compte tout : la
/// connexion, l'envoi et la réponse entière.
pub fn requete(
    port: u16,
    methode: &str,
    chemin: &str,
    cle: Option<&str>,
    corps: Option<&[u8]>,
    delai: Duration,
) -> Result<Reponse, Erreur> {
    let fin = Instant::now() + delai;
    let adresse = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut s = TcpStream::connect_timeout(&adresse, delai.max(Duration::from_millis(1)))
        .map_err(|e| if est_delai(&e) { Erreur::Delai(delai) } else { Erreur::Connexion(e.to_string()) })?;
    let _ = s.set_nodelay(true);
    let mut tete = format!("{methode} {chemin} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n");
    if let Some(c) = cle {
        tete.push_str(&format!("Authorization: Bearer {c}\r\n"));
    }
    let corps = corps.unwrap_or_default();
    if !corps.is_empty() || methode == "POST" {
        tete.push_str(&format!("Content-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\n", corps.len()));
    }
    tete.push_str("\r\n");
    let mut envoi = tete.into_bytes();
    envoi.extend_from_slice(corps);
    reste(fin).and_then(|r| s.set_write_timeout(Some(r)).ok()).ok_or(Erreur::Delai(delai))?;
    s.write_all(&envoi)
        .map_err(|e| if est_delai(&e) { Erreur::Delai(delai) } else { Erreur::Connexion(e.to_string()) })?;

    let mut recu: Vec<u8> = Vec::new();
    let mut morceau = [0u8; 16 * 1024];
    loop {
        if let Some(r) = complete(&recu)? {
            return Ok(r);
        }
        let Some(r) = reste(fin) else { return Err(Erreur::Delai(delai)) };
        let _ = s.set_read_timeout(Some(r));
        match s.read(&mut morceau) {
            Ok(0) => break,
            Ok(n) => recu.extend_from_slice(&morceau[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if est_delai(&e) => return Err(Erreur::Delai(delai)),
            // (une connexion coupée après une réponse entière, sans longueur annoncée : la réponse compte)
            Err(e) if recu.is_empty() => return Err(Erreur::Connexion(e.to_string())),
            Err(_) => break,
        }
    }
    // La connexion est fermée : ce qui est arrivé est la réponse entière (sans Content-Length), ou une réponse coupée
    fin_de_connexion(&recu)
}

fn reste(fin: Instant) -> Option<Duration> {
    let r = fin.saturating_duration_since(Instant::now());
    (!r.is_zero()).then_some(r)
}

fn est_delai(e: &std::io::Error) -> bool {
    matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock)
}

/// La tête lue : le code, la longueur annoncée, « chunked » ?, et où commence le corps
struct Tete {
    statut: u16,
    longueur: Option<usize>,
    morceaux: bool,
    debut: usize,
}

fn lire_tete(recu: &[u8]) -> Result<Option<Tete>, Erreur> {
    let Some(i) = recu.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Ok(None);
    };
    let texte = String::from_utf8_lossy(&recu[..i]);
    let mut lignes = texte.split("\r\n");
    let premiere = lignes.next().unwrap_or_default();
    let mut mots = premiere.split(' ');
    let version = mots.next().unwrap_or_default();
    let statut = mots.next().and_then(|c| c.parse::<u16>().ok());
    let Some(statut) = statut.filter(|_| version.starts_with("HTTP/")) else {
        return Err(Erreur::Illisible(premiere.chars().take(80).collect()));
    };
    let mut longueur = None;
    let mut morceaux = false;
    for l in lignes {
        let Some((nom, valeur)) = l.split_once(':') else { continue };
        let (nom, valeur) = (nom.trim(), valeur.trim());
        if nom.eq_ignore_ascii_case("content-length") {
            longueur = Some(valeur.parse::<usize>().map_err(|_| Erreur::Illisible(l.to_string()))?);
        } else if nom.eq_ignore_ascii_case("transfer-encoding") && valeur.to_ascii_lowercase().contains("chunked") {
            morceaux = true;
        }
    }
    Ok(Some(Tete { statut, longueur, morceaux, debut: i + 4 }))
}

/// La réponse, si elle est déjà entière (longueur annoncée atteinte, ou dernier morceau reçu)
fn complete(recu: &[u8]) -> Result<Option<Reponse>, Erreur> {
    let Some(t) = lire_tete(recu)? else { return Ok(None) };
    let corps = &recu[t.debut..];
    if t.morceaux {
        return Ok(recoller(corps).map(|c| Reponse { statut: t.statut, corps: c }));
    }
    match t.longueur {
        Some(n) if corps.len() >= n => Ok(Some(Reponse { statut: t.statut, corps: corps[..n].to_vec() })),
        _ => Ok(None),
    }
}

fn fin_de_connexion(recu: &[u8]) -> Result<Reponse, Erreur> {
    if recu.is_empty() {
        return Err(Erreur::Connexion("connexion fermée sans réponse".to_string()));
    }
    let Some(t) = lire_tete(recu)? else {
        return Err(Erreur::Illisible("réponse coupée avant la fin de l'en-tête".to_string()));
    };
    if t.morceaux || t.longueur.is_some() {
        return Err(Erreur::Connexion("réponse coupée avant la fin".to_string()));
    }
    Ok(Reponse { statut: t.statut, corps: recu[t.debut..].to_vec() })
}

/// Un corps « chunked » recollé, s'il est entier (le morceau de taille 0 est arrivé)
fn recoller(mut b: &[u8]) -> Option<Vec<u8>> {
    let mut sortie = Vec::new();
    loop {
        let i = b.windows(2).position(|w| w == b"\r\n")?;
        let taille_txt = String::from_utf8_lossy(&b[..i]);
        let taille = usize::from_str_radix(taille_txt.split(';').next().unwrap_or_default().trim(), 16).ok()?;
        b = &b[i + 2..];
        if taille == 0 {
            return Some(sortie);
        }
        if b.len() < taille + 2 {
            return None;
        }
        sortie.extend_from_slice(&b[..taille]);
        b = &b[taille + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longueur_annoncee() {
        let r = complete(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nokEN TROP").unwrap().unwrap();
        assert_eq!((r.statut, r.corps.as_slice()), (200, b"ok".as_slice()));
        assert!(complete(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nok").unwrap().is_none());
    }

    #[test]
    fn morceaux() {
        let r = complete(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nok\r\n3\r\n !!\r\n0\r\n\r\n")
            .unwrap()
            .unwrap();
        assert_eq!(r.corps, b"ok !!");
        assert!(complete(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nok\r\n").unwrap().is_none());
    }

    #[test]
    fn pas_du_http() {
        assert!(matches!(complete(b"bonjour\r\n\r\n"), Err(Erreur::Illisible(_))));
        assert!(fin_de_connexion(b"").is_err());
        assert_eq!(fin_de_connexion(b"HTTP/1.0 200 OK\r\n\r\nfin").unwrap().corps, b"fin");
    }
}
