//! « Ouvrir Nevermind » : Mindcare, affiché dans Mindset, demande à Mindset d'ouvrir une autre appli de la suite
//! (docs\contrat-appli.md du dépôt Mindset, « ouvrir.demande »). Mindcare n'ouvre et n'installe jamais rien lui-même :
//! il écrit l'id de l'appli (`nevermind`) dans `ouvrir.demande` de SON dossier de données, à côté puis renommé ;
//! Mindset le lit toutes les 400 ms, l'efface, et fait ce que fait un clic sur sa carte (ouverte, ou sa page avec
//! « Installe-le depuis l'accueil »). Toujours là au bout de 3 s (Mindset trop ancien) : Mindcare le reprend et dit
//! quoi faire.
//!
//! Tout se passe sur un fil à part ([`Demande`]) : l'écriture et l'attente ne figent jamais la page. Savoir si
//! Nevermind est installé aussi ([`Installee`] : suite.json et son programme, seulement regardés).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use mindcare_moteur::Appli;

/// Le fichier de la demande, dans le dossier de données de Mindcare.
pub const DEMANDE: &str = "ouvrir.demande";
/// Mindset le lit en 400 ms : passé ce temps, personne ne le lira.
pub const ATTENTE: Duration = Duration::from_secs(3);
/// Le fil regarde si la demande est partie à ce rythme.
const PAS_ATTENTE: Duration = Duration::from_millis(100);
/// Le contrat : moins de 64 octets.
const TAILLE_MAX: usize = 64;

/// Où Mindset lit la demande, et où il dit ce qui est installé : seulement pour une copie gérée par Mindset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suite {
    /// Le dossier de données de Mindcare (celui où Mindset lit ses `.demande`)
    pub dossier_donnees: PathBuf,
    /// `%LOCALAPPDATA%\Mindset\suite.json`
    pub fichier_suite: PathBuf,
}

/// Le programme d'une appli de la suite, dans son dossier (celui que Mindset exige, apps.json `exe`).
pub fn programme(a: Appli) -> &'static str {
    match a {
        Appli::Nevermind => "Nevermind.exe",
    }
}

/// L'appli est installée par Mindset : suite.json donne son dossier (`applis.<id>`), et son programme y est.
pub fn installee(fichier_suite: &Path, a: Appli) -> bool {
    mindcare_donnees::dossier::dossier_appli_suite(fichier_suite, a.id())
        .is_some_and(|d| Path::new(&d).join(programme(a)).is_file())
}

/// Écrit la demande : l'id seul, sans retour à la ligne, en UTF-8 sans marque, à côté puis renommé (Mindset ne lit
/// jamais un fichier à moitié écrit).
pub fn ecrire(dossier: &Path, a: Appli) -> Result<(), String> {
    let contenu = a.id().as_bytes();
    debug_assert!(contenu.len() < TAILLE_MAX);
    std::fs::create_dir_all(dossier).map_err(|e| e.to_string())?;
    let provisoire = dossier.join(format!("{DEMANDE}.{}.tmp", std::process::id()));
    std::fs::write(&provisoire, contenu).map_err(|e| e.to_string())?;
    std::fs::rename(&provisoire, dossier.join(DEMANDE)).map_err(|e| {
        let _ = std::fs::remove_file(&provisoire);
        e.to_string()
    })
}

/// Comment une demande s'est finie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fin {
    /// Mindset l'a lue (et effacée)
    Lue,
    /// Personne ne l'a lue en 3 s : Mindcare l'a reprise (Mindset trop ancien)
    Reprise,
    /// Elle n'a pas pu être écrite (la raison)
    Ratee(String),
}

/// Une demande en cours, sur son fil : écrite, puis attendue au plus `attente`.
pub struct Demande {
    pub appli: Appli,
    fin: Arc<Mutex<Option<Fin>>>,
}

impl Demande {
    /// Écrit la demande et l'attend, sur un fil à part (jamais sur celui de la page).
    pub fn lancer(dossier: PathBuf, a: Appli, attente: Duration) -> Demande {
        let fin = Arc::new(Mutex::new(None));
        let pour_fil = Arc::clone(&fin);
        let fil = std::thread::Builder::new().name("mindcare-ouvrir".into()).spawn(move || {
            let f = attendre(&dossier, a, attente);
            *pour_fil.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
        });
        if let Err(e) = fil {
            *fin.lock().unwrap_or_else(|e| e.into_inner()) = Some(Fin::Ratee(e.to_string()));
        }
        Demande { appli: a, fin }
    }

    /// La fin, une fois arrivée (rendue une seule fois).
    pub fn fin(&self) -> Option<Fin> {
        self.fin.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

/// Le travail du fil : écrire, puis attendre que Mindset efface le fichier ; sinon le reprendre.
fn attendre(dossier: &Path, a: Appli, attente: Duration) -> Fin {
    if let Err(e) = ecrire(dossier, a) {
        return Fin::Ratee(e);
    }
    let f = dossier.join(DEMANDE);
    let debut = Instant::now();
    loop {
        if !f.exists() {
            return Fin::Lue;
        }
        if debut.elapsed() >= attente {
            break;
        }
        std::thread::sleep(PAS_ATTENTE);
    }
    match std::fs::remove_file(&f) {
        Ok(()) => Fin::Reprise,
        // (effacé entre-temps : Mindset l'a lu au dernier moment)
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Fin::Lue,
        Err(_) => Fin::Reprise,
    }
}

/// Savoir si l'appli est installée, regardé sur un fil à part (suite.json et le disque), à refaire de temps en temps
/// (Nevermind peut être installé ou désinstallé pendant que Mindcare tourne).
pub struct Installee {
    lu: Arc<Mutex<Option<bool>>>,
    en_cours: Arc<Mutex<bool>>,
    derniere: Option<Instant>,
}

/// Le dossier de Nevermind est regardé de nouveau à ce rythme.
pub const RELIRE: Duration = Duration::from_secs(5);

impl Default for Installee {
    fn default() -> Self {
        Installee { lu: Arc::new(Mutex::new(None)), en_cours: Arc::new(Mutex::new(false)), derniere: None }
    }
}

impl Installee {
    /// Regarde de nouveau, sur un fil à part, si la dernière fois date de plus de [`RELIRE`] (rien si un fil regarde
    /// encore).
    pub fn relire_si_besoin(&mut self, fichier_suite: &Path, a: Appli, maintenant: Instant) {
        if self.derniere.is_some_and(|d| maintenant.saturating_duration_since(d) < RELIRE) {
            return;
        }
        {
            let mut occupe = self.en_cours.lock().unwrap_or_else(|e| e.into_inner());
            if *occupe {
                return;
            }
            *occupe = true;
        }
        self.derniere = Some(maintenant);
        let (lu, en_cours, fichier) = (Arc::clone(&self.lu), Arc::clone(&self.en_cours), fichier_suite.to_path_buf());
        let fil = std::thread::Builder::new().name("mindcare-nevermind".into()).spawn(move || {
            let oui = installee(&fichier, a);
            *lu.lock().unwrap_or_else(|e| e.into_inner()) = Some(oui);
            *en_cours.lock().unwrap_or_else(|e| e.into_inner()) = false;
        });
        if fil.is_err() {
            *self.en_cours.lock().unwrap_or_else(|e| e.into_inner()) = false;
        }
    }

    /// Ce qui a été lu depuis la dernière fois (rendu une seule fois).
    pub fn lu(&self) -> Option<bool> {
        self.lu.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier_neuf(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mindcare-ouvrir-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn la_demande_contient_l_id_seul() {
        let d = dossier_neuf("ecrire");
        ecrire(&d, Appli::Nevermind).unwrap();
        let o = std::fs::read(d.join(DEMANDE)).unwrap();
        assert_eq!(o, b"nevermind", "sans retour à la ligne ni marque UTF-8");
        assert!(o.len() < 64);
        // (rien d'autre de laissé : le fichier provisoire est renommé)
        let noms: Vec<String> =
            std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(noms, [DEMANDE]);
        // Une deuxième demande remplace la première
        ecrire(&d, Appli::Nevermind).unwrap();
        assert_eq!(std::fs::read(d.join(DEMANDE)).unwrap(), b"nevermind");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nevermind_installe_selon_suite_json_et_son_programme() {
        let d = dossier_neuf("installe");
        let suite = d.join("suite.json");
        assert!(!installee(&suite, Appli::Nevermind), "sans suite.json");
        let nm = d.join("Programmes").join("Nevermind");
        std::fs::create_dir_all(&nm).unwrap();
        let ecrire_suite = |j: serde_json::Value| std::fs::write(&suite, j.to_string()).unwrap();
        ecrire_suite(serde_json::json!({ "applis": { "mindcare": "C:\\x" } }));
        assert!(!installee(&suite, Appli::Nevermind), "pas dans suite.json");
        ecrire_suite(serde_json::json!({ "applis": { "nevermind": nm.to_string_lossy() } }));
        assert!(!installee(&suite, Appli::Nevermind), "son programme manque");
        std::fs::write(nm.join("Nevermind.exe"), b"").unwrap();
        assert!(installee(&suite, Appli::Nevermind));
        // (avec la marque UTF-8 que PowerShell écrit)
        let mut o = vec![0xEF, 0xBB, 0xBF];
        o.extend(serde_json::json!({ "applis": { "nevermind": nm.to_string_lossy() } }).to_string().into_bytes());
        std::fs::write(&suite, o).unwrap();
        assert!(installee(&suite, Appli::Nevermind));
        std::fs::write(&suite, b"{ abime").unwrap();
        assert!(!installee(&suite, Appli::Nevermind), "illisible");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lue_par_mindset_ou_reprise_au_bout_de_l_attente() {
        // Personne ne la lit : reprise, le fichier effacé
        let d = dossier_neuf("reprise");
        let debut = Instant::now();
        let dem = Demande::lancer(d.clone(), Appli::Nevermind, Duration::from_millis(400));
        let fin = loop {
            if let Some(f) = dem.fin() {
                break f;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(fin, Fin::Reprise);
        assert!(debut.elapsed() >= Duration::from_millis(400));
        assert!(!d.join(DEMANDE).exists());
        assert_eq!(dem.fin(), None, "rendue une seule fois");
        // Un faux Mindset l'efface : lue
        let dem = Demande::lancer(d.clone(), Appli::Nevermind, Duration::from_secs(10));
        let f = d.join(DEMANDE);
        let mut lu = None;
        for _ in 0..200 {
            if let Ok(o) = std::fs::read(&f) {
                lu = Some(o);
                std::fs::remove_file(&f).unwrap();
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(lu.as_deref(), Some(&b"nevermind"[..]));
        let fin = loop {
            if let Some(f) = dem.fin() {
                break f;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(fin, Fin::Lue);
        // Le dossier ne peut pas être écrit (un fichier à sa place) : ratée
        let bloque = d.join("bloque");
        std::fs::write(&bloque, b"").unwrap();
        let dem = Demande::lancer(bloque, Appli::Nevermind, Duration::from_millis(100));
        let fin = loop {
            if let Some(f) = dem.fin() {
                break f;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(matches!(fin, Fin::Ratee(_)), "{fin:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
