//! La construction d'essai (option cargo `essai`), jamais livrée : elle tourne seulement sur un dossier d'essai et
//! sur un bureau Windows à part (outils\sur-bureau.ps1, outils\essai-fenetre.ps1). L'exe livré ne contient pas ce
//! module : la CI cherche ses noms de variables dans l'exe de publication.
//!
//! Trois variables, toutes obligatoires (sinon le programme s'arrête aussitôt, sans rien faire) :
//! - `MINDCARE_DONNEES` : le dossier de données de Mindcare (donnees.ps1:18) ; le dossier WebView2 se met à côté,
//!   dans son dossier parent (comme `<données de la suite>\Mindcare`) ;
//! - `MINDCARE_ESSAI_LOCAL` : le faux `%LOCALAPPDATA%` (suite.json, afficher-page.txt) ;
//! - `MINDCARE_ESSAI_NOM` : le nom des verrou et signal d'essai, `Local\MindcareEssai-<nom>-Instance` et
//!   `-Afficher` (jamais ceux de la vraie Mindcare, qui tourne peut-être sur le PC).
//!
//! En plus, dans cette construction : HKLM n'est jamais lu et seul ce programme peut être lancé
//! (windows\systeme.rs) ; un fichier `capture.demande` dans le dossier de données fait écrire l'image de la
//! fenêtre dans `capture.png`, à côté ; un fichier `actions.demande` y fait jouer des messages de la page (chaque
//! écran atteint sans clavier, pour les captures).
//!
//! L'assistant et la dictée de l'essai (étape 6.4, [`outils`]) : les téléchargements viennent SEULEMENT d'un faux
//! serveur sur ce PC (127.0.0.1), décrit par le fichier `MINDCARE_ESSAI_DEPOT` (sans lui, ou pour une adresse qui
//! n'est pas sur ce PC : une adresse morte, jamais Internet) ; le micro est un FAUX micro (le vrai n'est jamais
//! ouvert) ; le son vit dans le `Temp` du dossier d'essai. llama-server et whisper sont ceux que le faux serveur a
//! donnés (les faux des tests des crates), lancés comme les vrais (sans fenêtre, arrêtés avec Mindcare).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mindcare_dictee::{Materiel, Mesures, Micro};
use mindcare_telechargement::Paquet;
use mindcare_vie::Noms;
use serde_json::Value;

/// Le fichier qui décrit le faux serveur de l'essai : `{ "assistant": [{ url, sha256, taille }, ...], "dictee": [...] }`,
/// dans l'ordre des fichiers de chaque paquet.
pub const VARIABLE_DEPOT: &str = "MINDCARE_ESSAI_DEPOT";

/// Le début de toute adresse permise dans l'essai : ce PC.
const ICI: &str = "http://127.0.0.1:";

/// Les fichiers d'un paquet pris sur le faux serveur (adresse, empreinte, taille), jamais sur Internet.
pub fn paquet_essai(mut p: Paquet, cle: &str, depot: Option<&Value>) -> Paquet {
    for (i, f) in p.fichiers.iter_mut().enumerate() {
        let e = depot.and_then(|d| d.get(cle)).and_then(|l| l.get(i));
        let url = e.and_then(|e| e.get("url")).and_then(Value::as_str).filter(|u| u.starts_with(ICI));
        match (url, e) {
            (Some(u), Some(e)) => {
                f.url = u.to_string();
                f.sha256 = e.get("sha256").and_then(Value::as_str).unwrap_or_default().to_string();
                f.taille = e.get("taille").and_then(Value::as_u64).unwrap_or(0);
            }
            _ => {
                let nom = f.url.rsplit('/').next().unwrap_or("fichier").to_string();
                f.url = format!("{ICI}9/essai-sans-depot/{nom}");
            }
        }
    }
    p
}

/// Le faux micro de l'essai : il « entend » une voix de 0,6 à 2,6 secondes après l'appui, puis le silence (la dictée
/// s'arrête une seconde après). Aucun vrai micro n'est ouvert.
pub struct FauxMateriel;

struct FauxMicro {
    debut: Option<Instant>,
    duree: Option<Duration>,
}

impl FauxMicro {
    fn ecoule(&self) -> Duration {
        self.duree.or_else(|| self.debut.map(|d| d.elapsed())).unwrap_or_default()
    }
}

impl Micro for FauxMicro {
    fn demarrer(&mut self, _numero: u32) -> Result<(), u32> {
        self.debut = Some(Instant::now());
        Ok(())
    }
    fn mesures(&self) -> Mesures {
        let ms = u32::try_from(self.ecoule().as_millis()).unwrap_or(u32::MAX);
        let voix = (600..2600).contains(&ms);
        Mesures {
            niveau: if voix { 0.5 } else { 0.02 },
            millis: ms,
            entendu: ms >= 600,
            millis_depuis_voix: ms.saturating_sub(2600),
        }
    }
    fn arreter(&mut self) {
        if self.duree.is_none() {
            self.duree = Some(self.ecoule());
        }
    }
    fn son(&self) -> Vec<u8> {
        vec![0; 3 * 16_000 * 2]
    }
}

impl Materiel for FauxMateriel {
    fn micro_present(&self) -> bool {
        true
    }
    fn noms(&self) -> Vec<String> {
        vec!["Micro d'essai".to_string()]
    }
    fn nouveau_micro(&self) -> Box<dyn Micro> {
        Box::new(FauxMicro { debut: None, duree: None })
    }
}

/// L'assistant et la dictée de l'essai : le faux serveur (sans proxy, sur ce PC), le faux micro, le `Temp` de l'essai.
pub fn outils(
    donnees: &Path,
    suite_json: &Path,
    local: &Path,
    journal: &mindcare_donnees::Journal,
) -> crate::effets::Outils {
    use mindcare_telechargement::{Installation, Transport, windows};
    let depot: Option<Value> = std::env::var(VARIABLE_DEPOT)
        .ok()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok());
    let temp = local.join("Temp");
    let _ = std::fs::create_dir_all(&temp);
    let reglages = mindcare_dictee::Reglages {
        paquet: Paquet::dictee(donnees),
        suite_json: Some(suite_json.to_path_buf()),
        temp,
        pid: std::process::id(),
        fils: 2,
    };
    let dictee = mindcare_dictee::DicteeEnFond::lancer(mindcare_dictee::Dictee::nouvelle(
        reglages,
        Box::new(FauxMateriel),
        Box::new(mindcare_dictee::windows::LanceurWindows),
    ));
    let j = journal.clone();
    crate::effets::Outils::nouveaux(
        mindcare_assistant::assistant(donnees, Arc::new(move |t: &str| j.ecrire(t))),
        Installation::nouvelle(paquet_essai(Paquet::assistant(donnees), "assistant", depot.as_ref())),
        dictee,
        Installation::nouvelle(paquet_essai(Paquet::dictee(donnees), "dictee", depot.as_ref())),
        windows::WinHttp::sans_proxy().map(|t| Arc::new(t) as Arc<dyn Transport>),
        Arc::new(windows::PlaceWindows),
    )
}

/// La demande de capture et l'image, dans le dossier de données d'essai.
pub const CAPTURE_DEMANDE: &str = "capture.demande";
pub const CAPTURE: &str = "capture.png";
/// Les messages demandés par le script d'essai (outils\essai-captures.ps1), un JSON par ligne : comme s'ils venaient
/// de la page, ou un ordre au kit (`{ "kit": "aller", "ecran": ... }`).
pub const ACTIONS_DEMANDE: &str = "actions.demande";

/// Ce que l'essai donne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Essai {
    pub donnees: PathBuf,
    pub local: String,
    pub nom: String,
}

impl Essai {
    /// Les noms du verrou et du signal d'essai.
    pub fn noms(&self) -> Noms {
        Noms {
            verrou: format!(r"Local\MindcareEssai-{}-Instance", self.nom),
            signal: format!(r"Local\MindcareEssai-{}-Afficher", self.nom),
        }
    }
}

/// Un nom d'essai : lettres, chiffres et tirets, 40 au plus.
fn nom_valable(n: &str) -> bool {
    (1..=40).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Les réglages de l'essai, d'après les trois valeurs (lues dans l'environnement par [`lire`]).
pub fn depuis(donnees: Option<String>, local: Option<String>, nom: Option<String>) -> Result<Essai, String> {
    let donnees = donnees.filter(|d| Path::new(d).is_absolute()).ok_or("MINDCARE_DONNEES manque ou incomplet")?;
    let local = local.filter(|d| Path::new(d).is_absolute()).ok_or("MINDCARE_ESSAI_LOCAL manque ou incomplet")?;
    let nom = nom.filter(|n| nom_valable(n)).ok_or("MINDCARE_ESSAI_NOM manque ou mal écrit")?;
    Ok(Essai { donnees: PathBuf::from(donnees), local, nom })
}

/// Les réglages de l'essai, lus dans l'environnement.
pub fn lire() -> Result<Essai, String> {
    let v = |n: &str| std::env::var(n).ok();
    depuis(v("MINDCARE_DONNEES"), v("MINDCARE_ESSAI_LOCAL"), v("MINDCARE_ESSAI_NOM"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reglages_d_essai() {
        let s = |t: &str| Some(t.to_string());
        let e = depuis(s(r"C:\Temp\essai\Donnees\Mindcare"), s(r"C:\Temp\essai\Local"), s("essai-42")).unwrap();
        assert_eq!(e.noms().verrou, r"Local\MindcareEssai-essai-42-Instance");
        assert_eq!(e.noms().signal, r"Local\MindcareEssai-essai-42-Afficher");
        // Jamais les vrais noms
        assert!(!e.noms().verrou.starts_with(r"Local\Mindcare-"));
        // Chaque valeur manquante ou mal écrite arrête l'essai
        assert!(depuis(None, s(r"C:\L"), s("a")).is_err());
        assert!(depuis(s(r"Donnees\Mindcare"), s(r"C:\L"), s("a")).is_err());
        assert!(depuis(s(r"C:\D"), None, s("a")).is_err());
        assert!(depuis(s(r"C:\D"), s(r"C:\L"), None).is_err());
        assert!(depuis(s(r"C:\D"), s(r"C:\L"), s(r"a\..\Mindcare")).is_err());
        assert!(depuis(s(r"C:\D"), s(r"C:\L"), s("")).is_err());
    }

    #[test]
    fn jamais_internet_dans_l_essai() {
        let d = Path::new(r"C:\Temp\essai\Donnees\Mindcare");
        let depot = serde_json::json!({
            "assistant": [
                { "url": "http://127.0.0.1:5123/moteur.zip", "sha256": "AB", "taille": 12 },
                { "url": "https://github.com/x/y.gguf", "sha256": "CD", "taille": 34 },
            ],
        });
        let a = paquet_essai(Paquet::assistant(d), "assistant", Some(&depot));
        assert_eq!(
            (a.fichiers[0].url.as_str(), a.fichiers[0].sha256.as_str(), a.fichiers[0].taille),
            ("http://127.0.0.1:5123/moteur.zip", "AB", 12)
        );
        // (une adresse qui n'est pas sur ce PC, ou rien dans le dépôt : une adresse morte sur ce PC)
        assert!(a.fichiers[1].url.starts_with("http://127.0.0.1:9/"), "{}", a.fichiers[1].url);
        for f in paquet_essai(Paquet::dictee(d), "dictee", None).fichiers {
            assert!(f.url.starts_with("http://127.0.0.1:9/"), "{}", f.url);
        }
    }

    #[test]
    fn faux_micro_entend_puis_se_tait() {
        let mut m = FauxMateriel.nouveau_micro();
        assert!(m.demarrer(0).is_ok());
        assert!(!m.mesures().entendu);
        m.arreter();
        assert_eq!(m.son().len(), 96_000);
    }
}
