//! Ce que la vie de Mindcare demande au monde extérieur, derrière des traits : Windows ([`Systeme`]), le disque
//! ([`Disque`]) et la fenêtre ([`Fenetre`]). Le programme les remplit avec Windows et Tauri (étape 4.4), les tests
//! avec des faux qui ne touchent jamais au vrai PC.

use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};

use mindcare_donnees::dossier::Suite;

/// La date d'un fichier (`LastWriteTimeUtc`) ; `None` : le fichier n'existe pas (`[datetime]::MinValue`,
/// systeme.ps1:89).
pub type Date = Option<SystemTime>;

/// Les noms du verrou et du signal d'affichage (apps.json : `mutex` et `afficher_signal`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Noms {
    /// `Local\Mindcare-Instance` (Mindcare.ps1:42)
    pub verrou: String,
    /// `Local\Mindcare-Afficher` (Mindcare.ps1:50, :54)
    pub signal: String,
}

impl Noms {
    /// Les vrais noms, ceux que Mindset connaît par apps.json.
    pub fn mindcare() -> Noms {
        Noms { verrou: r"Local\Mindcare-Instance".into(), signal: r"Local\Mindcare-Afficher".into() }
    }
}

/// Ce qu'a donné l'attente d'un verrou déjà tenu (`WaitOne(5000)`, Mindcare.ps1:47).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttenteVerrou {
    /// L'autre copie l'a rendu : il est à nous
    Libre,
    /// L'autre copie s'est arrêtée sans le rendre (`AbandonedMutexException`) : il est à nous aussi
    Abandonne,
    /// Toujours tenu après l'attente
    Tenu,
}

/// Windows, sauf la fenêtre et les fichiers : verrou, signal, registre, lancement d'un programme.
pub trait Systeme {
    /// Crée le verrou nommé et le prend (`New-Object Mutex($true, nom, [ref]$cree)`, Mindcare.ps1:42) : `true` s'il
    /// vient d'être créé, `false` s'il existait déjà (tenu par une autre copie). Gardé jusqu'à la fin du processus.
    fn creer_verrou(&mut self, nom: &str) -> bool;
    /// Attend le verrou déjà tenu (Mindcare.ps1:47).
    fn attendre_verrou(&mut self, delai: Duration) -> AttenteVerrou;
    /// Rend le verrou et le ferme (Mindcare.ps1:92, avant une relance).
    fn liberer_verrou(&mut self);
    /// Ouvre le signal d'une autre copie et le lève ; toute erreur est ignorée (Mindcare.ps1:50).
    fn lever_signal(&mut self, nom: &str);
    /// Crée le signal d'affichage, à remise automatique (`EventWaitHandle(false, AutoReset, nom)`, Mindcare.ps1:54).
    fn creer_signal(&mut self, nom: &str);
    /// Le signal a été levé depuis la dernière fois (`WaitOne(0)`, systeme.ps1:113) ; il est alors baissé.
    fn signal_recu(&mut self) -> bool;
    /// Ferme le signal (Mindcare.ps1:93, avant une relance).
    fn fermer_signal(&mut self);
    /// `InstallLocation` de `HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\Mindset`, tel quel, s'il se lit
    /// (mindset.ps1:79). Seul un administrateur peut l'écrire.
    fn emplacement_mindset(&mut self) -> Option<String>;
    /// Lance un programme sans l'attendre (`Start-Process`), dans ce dossier de travail s'il est donné.
    fn lancer(&mut self, exe: &Path, arguments: &[&str], dossier: Option<&Path>) -> io::Result<()>;
}

/// Le disque : fichiers du dossier de données, suite.json, journal.
pub trait Disque {
    /// `Test-Path` : un fichier ou un dossier existe.
    fn existe(&self, chemin: &Path) -> bool;
    /// `[IO.File]::Exists`, `Test-Path -PathType Leaf` : un fichier existe.
    fn est_fichier(&self, chemin: &Path) -> bool;
    /// `[IO.File]::Delete`.
    fn effacer(&mut self, chemin: &Path) -> io::Result<()>;
    /// La date d'écriture d'un fichier, `None` s'il n'existe pas (systeme.ps1:88-89).
    fn date(&self, chemin: &Path) -> Date;
    /// `Read-Suite` (mindset.ps1:33-56), `essais` lectures s'il est pris.
    fn lire_suite(&mut self, fichier: &Path, essais: u32) -> Suite;
    /// `New-Item -ItemType Directory -Force`, `[IO.Directory]::CreateDirectory`.
    fn creer_dossier(&mut self, chemin: &Path) -> io::Result<()>;
    /// `[IO.File]::WriteAllText(chemin, texte, UTF8Encoding($false))` : UTF-8 sans BOM.
    fn ecrire_texte(&mut self, chemin: &Path, texte: &str) -> io::Result<()>;
    /// Ouvre le dossier de données (le journal, etat.json), une fois qu'il est connu (donnees.ps1:18-25).
    fn ouvrir_donnees(&mut self, dossier: &Path);
    /// `Write-EtatMindset` sans sa condition (systeme.ps1:72-80) : la carte de Mindset écrite dans etat.json, seulement
    /// si elle a changé depuis la dernière écriture.
    fn ecrire_etat(&mut self);
    /// `Write-Log` (donnees.ps1:35-37) : une ligne au journal.
    fn journal(&mut self, message: &str);
}

/// Une taille de fenêtre, en pixels logiques.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Taille {
    pub largeur: f64,
    pub hauteur: f64,
}

/// La fenêtre ou l'un de ses parents, pour la veille de la page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Niveau {
    /// `IsWindowVisible`
    pub visible: bool,
    /// `IsIconic`
    pub reduite: bool,
}

/// La fenêtre principale (une seule, créée au démarrage, jamais recréée : contrat-appli partie 7).
pub trait Fenetre {
    /// La fenêtre a son handle (systeme.ps1:21-22).
    fn existe(&self) -> bool;
    /// Accrochée dans Mindset : `WS_CHILD` ou un parent (`EstEnfant`, natif.cs:79-82).
    fn est_accrochee(&self) -> bool;
    /// `$Window.IsVisible`.
    fn est_visible(&self) -> bool;
    /// Se montre sans prendre la main ni bouger, là où Mindset l'a mise (systeme.ps1:29-34 ; `SW_SHOWNA`).
    fn montrer_sans_activer(&mut self);
    /// Se montre au premier plan (systeme.ps1:57-60) : montrée, rendue si réduite, puis activée.
    fn montrer_et_activer(&mut self);
    /// La taille minimale (`MinWidth`, `MinHeight`) ; `None` : aucune (0, systeme.ps1:28).
    fn poser_taille_minimale(&mut self, taille: Option<Taille>);
    /// La fenêtre, puis chacun de ses parents jusqu'au premier niveau (`GetParent`).
    fn niveaux(&self) -> Vec<Niveau>;
    /// La page est-elle visible pour WebView2 (`SetIsVisible` : cachée, elle se met en veille).
    fn page_visible(&mut self, visible: bool);
}

/// Les trois ensemble, pour un tour de surveillance.
pub struct Monde<'a> {
    pub systeme: &'a mut dyn Systeme,
    pub disque: &'a mut dyn Disque,
    pub fenetre: &'a mut dyn Fenetre,
}
