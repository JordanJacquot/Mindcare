//! Le faux llama-server de la construction d'essai de Mindcare (outils\essai-aides.ps1), comme celui des tests de cette
//! crate (tests\commun) : jamais livré, jamais téléchargé ailleurs que depuis le faux serveur de l'essai, sur ce PC.
//! Mindcare le lance comme le vrai (copié en Mindset.exe, sans fenêtre, relié à Mindcare par un Job Object, sa clé
//! dans `LLAMA_API_KEY`). Il ne lance rien et n'écrit rien.
//!
//! - il écoute seulement sur 127.0.0.1, au port donné par `--port` ;
//! - `/health` répond « Loading model » (503) pendant 1,5 seconde, puis « ok » : l'assistant « se prépare » ;
//! - le reste demande la clé (401 sans elle), comme le vrai : `/props`, `/v1/chat/completions` ;
//! - la réponse suit `faux-reponses.json`, à côté de lui (posé par l'essai) : la première règle dont le mot
//!   (`contient`) est dans la phrase donne la panne, ses réponses et la confiance du nom de la panne (par ses
//!   jetons) ; `brut` : ce texte tel quel (une réponse illisible) ; `lenteur` : en millisecondes (1200 sinon). Sans
//!   règle : la première panne permise, à 30 %. Il s'arrête tout seul au bout de 10 minutes.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const CHARGEMENT: Duration = Duration::from_millis(1500);
const VIE: Duration = Duration::from_secs(10 * 60);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port =
        args.iter().position(|a| a == "--port").and_then(|i| args.get(i + 1)).and_then(|p| p.parse::<u16>().ok());
    let Some(port) = port else {
        eprintln!("faux llama-server : --port manque");
        std::process::exit(2);
    };
    let cle = std::env::var("LLAMA_API_KEY").ok().filter(|c| !c.is_empty());
    let ici = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)).unwrap_or_default();
    let regles: Vec<Value> = std::fs::read_to_string(ici.join("faux-reponses.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let ecoute = match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
        Ok(e) => e,
        Err(_) => {
            eprintln!("couldn't bind HTTP server socket, hostname: 127.0.0.1, port: {port}");
            std::process::exit(1);
        }
    };
    let _ = ecoute.set_nonblocking(true);
    println!("faux llama-server prêt sur le port {port}");
    let debut = Instant::now();
    while debut.elapsed() < VIE {
        match ecoute.accept() {
            Ok((s, _)) => {
                let (cle, regles) = (cle.clone(), regles.clone());
                std::thread::spawn(move || servir(s, cle.as_deref(), &regles, debut));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// Une demande : la méthode, le chemin, les en-têtes (noms en minuscules), le corps.
type Demande = (String, String, Vec<(String, String)>, Vec<u8>);

fn lire(s: &mut TcpStream) -> Option<Demande> {
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut recu = Vec::new();
    let mut m = [0u8; 4096];
    let fin = loop {
        if let Some(i) = recu.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        let n = s.read(&mut m).ok()?;
        if n == 0 {
            return None;
        }
        recu.extend_from_slice(&m[..n]);
    };
    let tete = String::from_utf8_lossy(&recu[..fin]).into_owned();
    let mut lignes = tete.split("\r\n");
    let mut premiere = lignes.next()?.split(' ');
    let (methode, chemin) = (premiere.next()?.to_string(), premiere.next()?.to_string());
    let entetes: Vec<(String, String)> = lignes
        .filter_map(|l| l.split_once(':').map(|(a, b)| (a.trim().to_ascii_lowercase(), b.trim().to_string())))
        .collect();
    let longueur: usize =
        entetes.iter().find(|(k, _)| k == "content-length").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    let mut corps = recu[fin + 4..].to_vec();
    while corps.len() < longueur {
        let n = s.read(&mut m).ok()?;
        if n == 0 {
            break;
        }
        corps.extend_from_slice(&m[..n]);
    }
    Some((methode, chemin, entetes, corps))
}

fn repondre(s: &mut TcpStream, statut: u16, corps: &str) {
    let mot = match statut {
        200 => "OK",
        401 => "Unauthorized",
        503 => "Service Unavailable",
        _ => "Not Found",
    };
    let t = format!(
        "HTTP/1.1 {statut} {mot}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corps}",
        corps.len()
    );
    let _ = s.write_all(t.as_bytes());
    let _ = s.flush();
}

fn servir(mut s: TcpStream, cle: Option<&str>, regles: &[Value], debut: Instant) {
    let _ = s.set_nonblocking(false);
    let Some((methode, chemin, entetes, corps)) = lire(&mut s) else { return };
    let charge = debut.elapsed() < CHARGEMENT;
    let chargement = r#"{"error":{"code":503,"message":"Loading model","type":"unavailable_error"}}"#;
    if chemin == "/health" {
        if charge {
            repondre(&mut s, 503, chargement)
        } else {
            repondre(&mut s, 200, r#"{"status":"ok"}"#)
        }
        return;
    }
    if charge {
        repondre(&mut s, 503, chargement);
        return;
    }
    if let Some(cle) = cle {
        let bonne = entetes.iter().any(|(k, v)| k == "authorization" && *v == format!("Bearer {cle}"));
        if !bonne {
            repondre(
                &mut s,
                401,
                r#"{"error":{"code":401,"message":"Invalid API Key","type":"authentication_error"}}"#,
            );
            return;
        }
    }
    match (methode.as_str(), chemin.as_str()) {
        ("GET", "/props") => repondre(&mut s, 200, r#"{"default_generation_settings":{},"total_slots":1}"#),
        ("POST", "/v1/chat/completions") => {
            let q: Value = serde_json::from_slice(&corps).unwrap_or(Value::Null);
            repondre(&mut s, 200, &reponse(&q, regles).to_string());
        }
        _ => repondre(&mut s, 404, r#"{"error":{"code":404,"message":"File Not Found","type":"not_found_error"}}"#),
    }
}

/// La réponse à une question, d'après les règles.
fn reponse(q: &Value, regles: &[Value]) -> Value {
    let phrase = q["messages"]
        .as_array()
        .and_then(|m| m.iter().rev().find(|x| x["role"] == "user"))
        .and_then(|x| x["content"].as_str())
        .unwrap_or_default()
        .to_lowercase();
    let permises: Vec<String> = q["response_format"]["json_schema"]["schema"]["properties"]["panne"]["enum"]
        .as_array()
        .map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let regle = regles.iter().find(|r| phrase.contains(&r["contient"].as_str().unwrap_or_default().to_lowercase()));
    let lenteur = regle.and_then(|r| r["lenteur"].as_u64()).unwrap_or(1200);
    std::thread::sleep(Duration::from_millis(lenteur));
    if let Some(brut) = regle.and_then(|r| r["brut"].as_str()) {
        return json!({ "choices": [{ "index": 0, "message": { "role": "assistant", "content": brut },
                       "logprobs": { "content": [] }, "finish_reason": "stop" }] });
    }
    let panne = regle
        .and_then(|r| r["panne"].as_str())
        .map(str::to_string)
        .or_else(|| permises.first().cloned())
        .unwrap_or_default();
    let reponses = regle.map(|r| r["reponses"].clone()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    let confiance = regle.and_then(|r| r["confiance"].as_f64()).unwrap_or(0.3).clamp(1e-6, 1.0);
    let contenu = json!({ "panne": panne, "reponses": reponses }).to_string();
    let debut = r#"{"panne":""#;
    let fin = contenu.get(debut.len() + panne.len()..).unwrap_or_default().to_string();
    json!({
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": contenu },
            "logprobs": { "content": [
                { "token": debut, "logprob": 0.0 },
                { "token": panne, "logprob": confiance.ln() },
                { "token": fin, "logprob": 0.0 },
            ] },
            "finish_reason": "stop",
        }],
        "object": "chat.completion",
    })
}
