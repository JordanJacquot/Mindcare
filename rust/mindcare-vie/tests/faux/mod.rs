//! Les faux Windows, disque et fenêtre des scénarios : tout se passe en mémoire, rien ne touche au vrai PC (ni
//! verrou, ni signal, ni registre, ni fichier, ni programme lancé).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use mindcare_donnees::dossier::Suite;
use mindcare_vie::{AttenteVerrou, Date, Disque, Fenetre, Monde, Niveau, Systeme, Taille};

/// Un programme lancé : son chemin, ses arguments, son dossier de travail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lance {
    pub exe: PathBuf,
    pub arguments: Vec<String>,
    pub dossier: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct FauxSysteme {
    /// Une autre copie tient déjà le verrou
    pub verrou_deja_tenu: bool,
    /// Ce que donne l'attente du verrou tenu
    pub attente: Option<AttenteVerrou>,
    pub verrous_crees: Vec<String>,
    pub attentes: Vec<Duration>,
    pub verrou_libere: bool,
    pub signaux_leves: Vec<String>,
    pub signal_cree: Option<String>,
    pub signal_ferme: bool,
    /// Un second lancement a levé le signal
    pub signal_leve: bool,
    /// InstallLocation de Mindset dans HKLM
    pub emplacement: Option<String>,
    pub lances: Vec<Lance>,
    pub lancer_echoue: bool,
}

impl Systeme for FauxSysteme {
    fn creer_verrou(&mut self, nom: &str) -> bool {
        self.verrous_crees.push(nom.to_string());
        !self.verrou_deja_tenu
    }
    fn attendre_verrou(&mut self, delai: Duration) -> AttenteVerrou {
        self.attentes.push(delai);
        self.attente.unwrap_or(AttenteVerrou::Tenu)
    }
    fn liberer_verrou(&mut self) {
        self.verrou_libere = true;
    }
    fn lever_signal(&mut self, nom: &str) {
        self.signaux_leves.push(nom.to_string());
    }
    fn creer_signal(&mut self, nom: &str) {
        self.signal_cree = Some(nom.to_string());
    }
    fn signal_recu(&mut self) -> bool {
        // (remise automatique : lu une fois, il est baissé)
        std::mem::take(&mut self.signal_leve)
    }
    fn fermer_signal(&mut self) {
        self.signal_ferme = true;
    }
    fn emplacement_mindset(&mut self) -> Option<String> {
        self.emplacement.clone()
    }
    fn lancer(&mut self, exe: &Path, arguments: &[&str], dossier: Option<&Path>) -> io::Result<()> {
        if self.lancer_echoue {
            return Err(io::Error::other("refusé"));
        }
        self.lances.push(Lance {
            exe: exe.to_path_buf(),
            arguments: arguments.iter().map(|a| a.to_string()).collect(),
            dossier: dossier.map(Path::to_path_buf),
        });
        Ok(())
    }
}

/// Un fichier du faux disque.
#[derive(Debug, Clone)]
pub struct FauxFichier {
    pub texte: String,
    pub date: SystemTime,
}

#[derive(Debug, Default)]
pub struct FauxDisque {
    pub fichiers: BTreeMap<String, FauxFichier>,
    pub dossiers: Vec<String>,
    /// Ce que donne la lecture de suite.json quand il existe
    pub suite: Suite,
    /// Les lectures de suite.json (nombre d'essais de chacune)
    pub lectures_suite: Vec<u32>,
    pub donnees_ouvertes: Option<PathBuf>,
    pub etats_ecrits: u32,
    pub journal: Vec<String>,
    pub effacer_echoue: bool,
}

/// La clé d'un chemin : sans tenir compte des majuscules, comme Windows.
pub fn cle(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

impl FauxDisque {
    /// Pose un fichier à cette date (en secondes).
    pub fn poser(&mut self, p: &str, texte: &str, secondes: u64) {
        let date = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + secondes);
        self.fichiers.insert(cle(Path::new(p)), FauxFichier { texte: texte.to_string(), date });
    }
    pub fn a(&self, p: &str) -> bool {
        self.fichiers.contains_key(&cle(Path::new(p)))
    }
    pub fn texte(&self, p: &str) -> Option<String> {
        self.fichiers.get(&cle(Path::new(p))).map(|f| f.texte.clone())
    }
    pub fn dans_journal(&self, morceau: &str) -> bool {
        self.journal.iter().any(|l| l.contains(morceau))
    }
}

impl Disque for FauxDisque {
    fn existe(&self, chemin: &Path) -> bool {
        let c = cle(chemin);
        self.fichiers.contains_key(&c) || self.dossiers.contains(&c)
    }
    fn est_fichier(&self, chemin: &Path) -> bool {
        self.fichiers.contains_key(&cle(chemin))
    }
    fn effacer(&mut self, chemin: &Path) -> io::Result<()> {
        if self.effacer_echoue {
            return Err(io::Error::other("pris"));
        }
        self.fichiers.remove(&cle(chemin)).map(|_| ()).ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
    fn date(&self, chemin: &Path) -> Date {
        self.fichiers.get(&cle(chemin)).map(|f| f.date)
    }
    fn lire_suite(&mut self, fichier: &Path, essais: u32) -> Suite {
        self.lectures_suite.push(essais);
        if self.est_fichier(fichier) { self.suite.clone() } else { Suite::default() }
    }
    fn creer_dossier(&mut self, chemin: &Path) -> io::Result<()> {
        self.dossiers.push(cle(chemin));
        Ok(())
    }
    fn ecrire_texte(&mut self, chemin: &Path, texte: &str) -> io::Result<()> {
        self.fichiers.insert(cle(chemin), FauxFichier { texte: texte.to_string(), date: SystemTime::now() });
        Ok(())
    }
    fn ouvrir_donnees(&mut self, dossier: &Path) {
        self.donnees_ouvertes = Some(dossier.to_path_buf());
    }
    fn ecrire_etat(&mut self) {
        self.etats_ecrits += 1;
    }
    fn journal(&mut self, message: &str) {
        self.journal.push(message.to_string());
    }
}

#[derive(Debug, Default)]
pub struct FausseFenetre {
    pub sans_handle: bool,
    pub accrochee: bool,
    pub visible: bool,
    pub reduite: bool,
    /// Les parents (le panneau de Mindset, puis sa fenêtre)
    pub parents: Vec<Niveau>,
    /// Chaque taille minimale posée
    pub tailles_min: Vec<Option<Taille>>,
    pub montree_sans_activer: u32,
    pub activee: u32,
    /// Chaque visibilité donnée à la page
    pub page: Vec<bool>,
    /// La place de la page (pixels logiques) ; `None` : illisible
    pub taille: Option<Taille>,
    /// Chaque échelle donnée à la fenêtre
    pub echelles: Vec<f64>,
}

impl Fenetre for FausseFenetre {
    fn existe(&self) -> bool {
        !self.sans_handle
    }
    fn est_accrochee(&self) -> bool {
        self.accrochee
    }
    fn est_visible(&self) -> bool {
        self.visible
    }
    fn montrer_sans_activer(&mut self) {
        self.visible = true;
        self.montree_sans_activer += 1;
    }
    fn montrer_et_activer(&mut self) {
        self.visible = true;
        self.reduite = false;
        self.activee += 1;
    }
    fn poser_taille_minimale(&mut self, taille: Option<Taille>) {
        self.tailles_min.push(taille);
    }
    fn niveaux(&self) -> Vec<Niveau> {
        let mut n = vec![Niveau { visible: self.visible, reduite: self.reduite }];
        n.extend(self.parents.iter().copied());
        n
    }
    fn page_visible(&mut self, visible: bool) {
        self.page.push(visible);
    }
    fn taille(&self) -> Option<Taille> {
        self.taille
    }
    fn poser_echelle(&mut self, echelle: f64) {
        self.echelles.push(echelle);
    }
}

/// Le banc d'essai : les trois faux.
#[derive(Debug, Default)]
pub struct Banc {
    pub systeme: FauxSysteme,
    pub disque: FauxDisque,
    pub fenetre: FausseFenetre,
}

impl Banc {
    pub fn monde(&mut self) -> Monde<'_> {
        Monde { systeme: &mut self.systeme, disque: &mut self.disque, fenetre: &mut self.fenetre }
    }
}
