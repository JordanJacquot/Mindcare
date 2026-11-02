//! Le démarrage, dans l'ordre de Mindcare.ps1 : suite.json lu, copie gérée ou non (modules\mindset.ps1), copie
//! bloquée, copie gérée lancée à la main, une seule copie à la fois (verrou et signal), dossier de données
//! (modules\donnees.ps1), vieille demande de fermeture effacée (modules\systeme.ps1).
//!
//! Écarts avec le contrat (contrat-appli, partie 3), où le PowerShell est suivi :
//! - verrou déjà tenu SANS `-Demarrage` : le PowerShell lève tout de suite le signal `Local\Mindcare-Afficher` puis
//!   quitte (Mindcare.ps1:48-52) ; le contrat parle d'attendre quelques secondes puis de déposer `afficher.demande`.
//!   Mindcare n'a pas d'`afficher_fichier` dans apps.json : seul le signal la ramène.
//! - l'attente de 5 s n'a lieu qu'avec `-Demarrage` (Mindcare.ps1:45-47), comme dans le plan de l'étape 4.

use std::path::{Path, PathBuf};
use std::time::Duration;

use mindcare_donnees::dossier::APP_NOM;
use mindcare_donnees::dossier::{Suite, chemin_normal, dossier_donnees, gere_par_mindset};
use suite_commun::chemins::{joindre, parent};

use crate::mindset::{PAGE_MINDCARE, mindset_exe, ouvrir_page_mindset};
use crate::monde::{AttenteVerrou, Disque, Noms, Systeme};

/// Les lectures de suite.json au démarrage, 300 ms entre deux s'il est pris (`Read-Suite`, mindset.ps1:33 et :59).
pub const ESSAIS_SUITE: u32 = 10;
/// L'attente du verrou tenu par une copie qui se ferme (Mindcare.ps1:47).
pub const ATTENTE_VERROU: Duration = Duration::from_millis(5000);
/// La demande de fermeture de Mindset, dans le dossier de données (systeme.ps1:12).
pub const QUITTER: &str = "quitter.demande";

/// Ce que Windows donne au démarrage.
#[derive(Debug, Clone, Default)]
pub struct Environnement {
    /// Le dossier du programme (celui de Mindset.exe et Mindcare.exe), tel quel (`Split-Path $AppDir -Parent`)
    pub racine: Option<String>,
    /// `%LOCALAPPDATA%`
    pub localappdata: String,
    /// `$env:ProgramFiles`
    pub program_files: Option<String>,
    /// `$env:ProgramW6432`
    pub program_w6432: Option<String>,
    /// La version de Mindcare (pour le journal)
    pub version: String,
    /// Le dossier de données d'un essai (`MINDCARE_DONNEES`, donnees.ps1:18) : rempli seulement par la construction
    /// d'essai de mindcare-appli (option cargo `essai`), jamais par l'exe livré
    pub donnees_essai: Option<String>,
}

impl Environnement {
    /// `%LOCALAPPDATA%\Mindset` : suite.json et afficher-page.txt.
    pub fn dossier_mindset(&self) -> PathBuf {
        PathBuf::from(joindre(&self.localappdata, "Mindset"))
    }

    /// `%LOCALAPPDATA%\Mindset\suite.json`, où que soit installée la suite (mindset.ps1:12).
    pub fn fichier_suite(&self) -> PathBuf {
        PathBuf::from(joindre(&self.localappdata, r"Mindset\suite.json"))
    }
}

/// Ce que le démarrage a trouvé, quand l'appli continue.
#[derive(Debug, Clone)]
pub struct Lancement {
    /// Lancée cachée (`-Demarrage`)
    pub demarrage: bool,
    /// Gérée par Mindset (mindset.ps1:74)
    pub gere: bool,
    /// Dossier de travail (code source), jamais bloqué (mindset.ps1:109)
    pub travail: bool,
    /// Le dossier de cette copie, normalisé (mindset.ps1:58)
    pub racine: Option<String>,
    /// suite.json au démarrage (mindset.ps1:59)
    pub suite: Suite,
    pub fichier_suite: PathBuf,
    pub dossier_mindset: PathBuf,
    /// `$DataDir` (donnees.ps1:18-20)
    pub dossier_donnees: PathBuf,
    /// Le programme qui relance Mindcare après un changement de thème (Mindcare.ps1:22-23)
    pub programme: Option<PathBuf>,
}

impl Lancement {
    /// La fenêtre se montre dès le départ, sauf lancée cachée : elle existe quand même (Mindset la cherche par son
    /// titre) et ne se montre qu'une fois accrochée (Mindcare.ps1:75-79).
    pub fn fenetre_montree(&self) -> bool {
        !self.demarrage
    }
}

/// Comment le démarrage finit.
#[derive(Debug, Clone)]
pub enum Depart {
    /// Copie que Mindset ne gère pas (ni dossier de travail) : la fenêtre « Mindcare s'utilise avec Mindset », puis
    /// rien d'autre (Mindcare.ps1:34). `mindset_exe` : « Ouvrir Mindset » s'il est installé, sinon
    /// « Télécharger Mindset » (mindset.ps1:122, :161).
    Bloquee { mindset_exe: Option<PathBuf> },
    /// Copie gérée lancée à la main : Mindset l'ouvre dans son onglet (Mindcare.ps1:35)
    OuverteDansMindset,
    /// Une autre copie tourne déjà (Mindcare.ps1:52) : quitter sans fenêtre
    DejaOuverte,
    /// L'appli démarre : verrou pris, signal créé, dossier de données prêt
    Lancer(Box<Lancement>),
}

/// `Test-DossierTravail` (mindset.ps1:103-108) : un `.git` dans le dossier de la copie ou juste au-dessus.
fn dossier_de_travail(racine: Option<&str>, disque: &dyn Disque) -> bool {
    let Some(r) = racine else { return false };
    [r, parent(r)].iter().any(|d| !d.is_empty() && disque.existe(Path::new(&joindre(d, ".git"))))
}

/// `$AppExe` (Mindcare.ps1:22-23) : `Mindset.exe` du dossier de la copie, sinon le relais `Mindcare.exe`.
fn programme(racine: Option<&str>, disque: &dyn Disque) -> Option<PathBuf> {
    let r = racine?;
    let exe = PathBuf::from(joindre(r, "Mindset.exe"));
    Some(if disque.existe(&exe) { exe } else { PathBuf::from(joindre(r, "Mindcare.exe")) })
}

/// Le thème de l'appli (`$ThemeSetting`, themes.ps1:104) : celui de la suite pour une copie gérée, sinon celui des
/// réglages (`Get-Setting 'Theme' 'neon'`, donné par l'appelant).
pub fn theme_choisi(gere: bool, theme_suite: Option<&str>, theme_reglages: &str) -> String {
    match theme_suite {
        Some(t) if gere && !t.is_empty() => t.to_string(),
        _ => theme_reglages.to_string(),
    }
}

/// Le démarrage de Mindcare (Mindcare.ps1:33-55, puis donnees.ps1:18-21 et systeme.ps1:14). `demarrage` : lancée
/// avec `-Demarrage` ([`crate::demarrage_demande`]).
pub fn demarrer(
    demarrage: bool,
    env: &Environnement,
    noms: &Noms,
    systeme: &mut dyn Systeme,
    disque: &mut dyn Disque,
) -> Depart {
    // mindset.ps1:58-59 : le dossier de cette copie, et suite.json (10 essais s'il est pris)
    let racine = env.racine.as_deref().and_then(chemin_normal);
    let fichier_suite = env.fichier_suite();
    let suite = disque.lire_suite(&fichier_suite, ESSAIS_SUITE);
    // mindset.ps1:74 : gérée par Mindset
    let gere =
        gere_par_mindset(&suite, racine.as_deref(), &[env.program_files.as_deref(), env.program_w6432.as_deref()]);
    // mindset.ps1:109 : dossier de travail, seulement s'il n'est pas géré
    let travail = !gere && dossier_de_travail(racine.as_deref(), disque);

    // Mindcare.ps1:34 et mindset.ps1:113-117 : ni gérée ni dossier de travail, bloquée
    if !gere && !travail {
        return Depart::Bloquee { mindset_exe: mindset_exe(systeme, disque) };
    }
    // Mindcare.ps1:35 : gérée et lancée à la main, elle s'ouvre dans Mindset (s'il est introuvable, elle continue)
    let dossier_mindset = env.dossier_mindset();
    if gere && !demarrage && ouvrir_page_mindset(PAGE_MINDCARE, &dossier_mindset, systeme, disque) {
        return Depart::OuverteDansMindset;
    }

    // Mindcare.ps1:40-53 : une seule à la fois
    if !systeme.creer_verrou(&noms.verrou) {
        let libre = if demarrage {
            // Mindcare.ps1:46-47 : Mindset la relance juste après une fermeture, l'ancienne libère la place dans
            // quelques secondes (un verrou abandonné est libre aussi)
            matches!(systeme.attendre_verrou(ATTENTE_VERROU), AttenteVerrou::Libre | AttenteVerrou::Abandonne)
        } else {
            // Mindcare.ps1:49-50 : un clic alors qu'elle tourne déjà, sa fenêtre est ramenée
            systeme.lever_signal(&noms.signal);
            false
        };
        // Mindcare.ps1:52
        if !libre {
            return Depart::DejaOuverte;
        }
    }
    // Mindcare.ps1:54 : le signal que lèvera un second lancement
    systeme.creer_signal(&noms.signal);

    // donnees.ps1:18-21 : le dossier de données, créé s'il manque (celui d'un essai seulement dans la construction
    // d'essai)
    let dossier =
        PathBuf::from(dossier_donnees(env.donnees_essai.as_deref(), gere, racine.as_deref(), &env.localappdata));
    let _ = disque.creer_dossier(&dossier);
    disque.ouvrir_donnees(&dossier);
    // systeme.ps1:14 : une vieille demande refermerait l'appli tout de suite
    let quitter = dossier.join(QUITTER);
    if disque.est_fichier(&quitter) {
        let _ = disque.effacer(&quitter);
    }
    // Mindcare.ps1:73
    disque.journal(&format!(
        "Démarrage {APP_NOM} {} (Rust){}",
        env.version,
        if demarrage { ", lancée cachée" } else { "" }
    ));

    Depart::Lancer(Box::new(Lancement {
        demarrage,
        gere,
        travail,
        programme: programme(env.racine.as_deref(), disque),
        racine,
        suite,
        fichier_suite,
        dossier_mindset,
        dossier_donnees: dossier,
    }))
}
