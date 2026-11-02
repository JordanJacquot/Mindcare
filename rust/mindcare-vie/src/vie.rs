//! Les surveillances de Mindcare (modules\systeme.ps1, `Start-Surveillances`) : une seule minuterie, toutes les 500 ms
//! (systeme.ps1:106-126), comme une machine d'état pilotée par le temps que donne l'appelant ([`Vie::avancer`]).
//!
//! À chaque tour, dans l'ordre du PowerShell :
//! 1. accrochage dans Mindset (`Update-DansMindset`, systeme.ps1:20-39) ;
//! 2. signal d'affichage d'un second lancement (`Show-MainWindow`, systeme.ps1:53-62) ;
//! 3. veille de la page (nouveau, sans PowerShell : contrat-appli partie 7, « Reste à faire », et plan 4.3) ;
//! 4. quitter.demande (systeme.ps1:114-119) ;
//! 5. un tour sur cinq (toutes les 2,5 s) : suite.json sur sa date, relance au changement de thème
//!    (`Update-SuiteSurveillance`, systeme.ps1:86-101) ;
//! 6. un tour sur 120 (toutes les minutes) : etat.json si l'appli est gérée (`Write-EtatMindset`, systeme.ps1:121).
//!
//! La croix de la fenêtre (et `WM_CLOSE`, le bouton « Fermer » de la barre de Mindset) quitte (evenements.ps1:10).

use std::path::{Path, PathBuf};
use std::time::Duration;

use mindcare_moteur::ps::egal;

use crate::demarrage::{Lancement, QUITTER};
use crate::mindset::{PAGE_MINDCARE, ouvrir_page_mindset};
use crate::monde::{Date, Disque, Monde, Systeme, Taille};

/// L'intervalle de la minuterie (systeme.ps1:109).
pub const TOUR: Duration = Duration::from_millis(500);
/// suite.json est regardé un tour sur cinq, au 1er, 6e, 11e... (systeme.ps1:120).
pub const TOURS_SUITE: u64 = 5;
/// etat.json est réécrit (s'il a changé) un tour sur 120, toutes les minutes (systeme.ps1:121).
pub const TOURS_ETAT: u64 = 120;
/// L'argument d'une relance cachée (Mindcare.ps1:94).
pub const ARG_DEMARRAGE: &str = "-Demarrage";

/// Comment l'appli finit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fin {
    /// Quitter pour de bon (`Exit-App`, systeme.ps1:133-141)
    Quitter,
    /// Se relancer avec le nouveau thème de la suite (`Restart-App`, systeme.ps1:131), cachée si elle l'était
    /// (systeme.ps1:98)
    Relancer { cachee: bool },
}

/// Ce que la vie de l'appli a besoin de savoir, venu du démarrage.
#[derive(Debug, Clone)]
pub struct Reglages {
    /// Gérée par Mindset (mindset.ps1:74)
    pub gere: bool,
    /// suite.json (mindset.ps1:12)
    pub fichier_suite: Option<PathBuf>,
    /// suite.json était encore pris au démarrage (`$script:Suite.Pris`, systeme.ps1:91)
    pub suite_prise: bool,
    /// Le thème de l'appli (`$ThemeSetting`, themes.ps1:104)
    pub theme: String,
    /// `%LOCALAPPDATA%\Mindset` (afficher-page.txt)
    pub dossier_mindset: PathBuf,
    /// Le dossier de données (quitter.demande)
    pub dossier_donnees: PathBuf,
    /// La taille minimale de la fenêtre quand elle n'est pas dans Mindset (`$script:TailleMin`, systeme.ps1:25)
    pub taille_min: Taille,
    /// Le programme qui relance l'appli (Mindcare.ps1:22-23)
    pub programme: Option<PathBuf>,
}

impl Reglages {
    /// D'après le démarrage, le thème choisi ([`crate::theme_choisi`]) et la taille minimale de la fenêtre.
    pub fn depuis(l: &Lancement, theme: &str, taille_min: Taille) -> Reglages {
        Reglages {
            gere: l.gere,
            fichier_suite: Some(l.fichier_suite.clone()),
            suite_prise: l.suite.pris,
            theme: theme.to_string(),
            dossier_mindset: l.dossier_mindset.clone(),
            dossier_donnees: l.dossier_donnees.clone(),
            taille_min,
            programme: l.programme.clone(),
        }
    }
}

/// `Update-EchelleMindset` (systeme.ps1:42-48) : dans Mindset, plus petite que la taille minimale, tout le contenu
/// est réduit à l'échelle (rien n'est coupé ni tassé). 1 sinon.
pub fn echelle(dans_mindset: bool, largeur: f64, hauteur: f64, min: Taille) -> f64 {
    if dans_mindset && largeur > 0.0 && hauteur > 0.0 && min.largeur > 0.0 && min.hauteur > 0.0 {
        (largeur / min.largeur).min(hauteur / min.hauteur).min(1.0)
    } else {
        1.0
    }
}

/// La vie de l'appli, de la fin du démarrage à sa fin.
#[derive(Debug, Clone)]
pub struct Vie {
    reglages: Reglages,
    /// `$script:Tours` (systeme.ps1:107)
    tours: u64,
    /// L'heure du prochain tour (depuis le début des surveillances)
    prochain: Duration,
    /// `$script:DansMindset` (systeme.ps1:15)
    dans_mindset: bool,
    /// Ce que la page sait de sa visibilité (rien avant le premier tour)
    page: Option<bool>,
    /// `$script:SuiteDate` : `None` tant qu'elle n'a pas été notée (systeme.ps1:91)
    suite_date: Option<Date>,
    /// `$script:Quitting` (systeme.ps1:134-135)
    quitte: bool,
}

impl Vie {
    pub fn nouvelle(reglages: Reglages) -> Vie {
        Vie { reglages, tours: 0, prochain: TOUR, dans_mindset: false, page: None, suite_date: None, quitte: false }
    }

    /// Affichée dans Mindset (accrochée).
    pub fn dans_mindset(&self) -> bool {
        self.dans_mindset
    }

    /// L'appli est en train de finir.
    pub fn quitte(&self) -> bool {
        self.quitte
    }

    /// Les tours faits depuis le début.
    pub fn tours(&self) -> u64 {
        self.tours
    }

    /// Le début des surveillances (systeme.ps1:123-124) : la carte de Mindset écrite tout de suite.
    pub fn commencer(&mut self, m: &mut Monde) {
        self.ecrire_etat(m.disque);
    }

    /// Fait tous les tours dus jusqu'à `maintenant` (le temps depuis [`Vie::commencer`]) ; le premier a lieu après
    /// 500 ms, comme une minuterie WPF. Rend la fin de l'appli dès qu'un tour la décide.
    pub fn avancer(&mut self, maintenant: Duration, m: &mut Monde) -> Option<Fin> {
        while !self.quitte && maintenant >= self.prochain {
            self.prochain += TOUR;
            if let Some(f) = self.tour(m) {
                return Some(f);
            }
        }
        None
    }

    /// Un tour de la minuterie (systeme.ps1:110-122).
    pub fn tour(&mut self, m: &mut Monde) -> Option<Fin> {
        if self.quitte {
            return None; // (systeme.ps1:137 : la minuterie est arrêtée)
        }
        // systeme.ps1:111
        self.tours += 1;
        // systeme.ps1:112
        self.accrochage(m);
        // systeme.ps1:113 : le signal d'un second lancement
        if m.systeme.signal_recu() {
            self.montrer(m);
        }
        // (nouveau) la page en veille quand on ne peut pas la voir
        self.veille(m);
        // systeme.ps1:114-119 : Mindset demande la fermeture
        let quitter = self.reglages.dossier_donnees.join(QUITTER);
        if m.disque.est_fichier(&quitter) {
            let _ = m.disque.effacer(&quitter); // systeme.ps1:115
            m.disque.journal("Fermeture demandée par Mindset"); // systeme.ps1:116
            return self.finir(Fin::Quitter, m.disque); // systeme.ps1:117
        }
        // systeme.ps1:120 : un tour sur cinq
        if self.tours % TOURS_SUITE == 1
            && let Some(f) = self.surveiller_suite(m)
        {
            return Some(f);
        }
        // systeme.ps1:121 : toutes les minutes
        if self.tours.is_multiple_of(TOURS_ETAT) {
            self.ecrire_etat(m.disque);
        }
        None
    }

    /// La croix de la fenêtre, ou `WM_CLOSE` venu de Mindset (evenements.ps1:10) : l'appli quitte proprement.
    pub fn croix(&mut self, m: &mut Monde) -> Option<Fin> {
        // evenements.ps1:10 : `if (-not $script:Quitting) { Exit-App }`
        self.finir(Fin::Quitter, m.disque)
    }

    /// `Exit-App` (systeme.ps1:133-136) : une seule fois.
    fn finir(&mut self, fin: Fin, disque: &mut dyn Disque) -> Option<Fin> {
        if self.quitte {
            return None; // systeme.ps1:134
        }
        self.quitte = true; // systeme.ps1:135
        disque.journal("Fermeture"); // systeme.ps1:136
        Some(fin)
    }

    /// `Update-DansMindset` (systeme.ps1:20-39).
    fn accrochage(&mut self, m: &mut Monde) {
        // systeme.ps1:21-22 : pas encore de fenêtre
        if !m.fenetre.existe() {
            return;
        }
        // systeme.ps1:23-24 : rien de changé
        let dans = m.fenetre.est_accrochee();
        if dans == self.dans_mindset {
            return;
        }
        // systeme.ps1:26
        self.dans_mindset = dans;
        if dans {
            // systeme.ps1:28 : Mindset donne la place qu'il a
            m.fenetre.poser_taille_minimale(None);
            // systeme.ps1:29-34 : lancée cachée, elle se montre sans s'activer, là où Mindset l'a mise
            if !m.fenetre.est_visible() {
                m.fenetre.montrer_sans_activer();
            }
        } else {
            // systeme.ps1:36 : décrochée, la taille minimale est rendue
            m.fenetre.poser_taille_minimale(Some(self.reglages.taille_min));
        }
        // systeme.ps1:37
        m.disque.journal(if dans { "Affichée dans Mindset" } else { "Plus affichée dans Mindset" });
    }

    /// `Show-MainWindow` (systeme.ps1:53-62).
    fn montrer(&mut self, m: &mut Monde) {
        // systeme.ps1:54 : affichée dans Mindset, rien à faire
        if self.dans_mindset {
            return;
        }
        // systeme.ps1:55 : gérée, elle ne s'ouvre que dans Mindset (s'il est introuvable : sa propre fenêtre)
        if self.reglages.gere && ouvrir_page_mindset(PAGE_MINDCARE, &self.reglages.dossier_mindset, m.systeme, m.disque)
        {
            return;
        }
        // systeme.ps1:57-60
        m.fenetre.montrer_et_activer();
    }

    /// La veille de la page (sans PowerShell : contrat-appli partie 7, plan 4.3) : la page n'est visible que si la
    /// fenêtre et tous ses parents sont visibles et non réduits. Dit à la page seulement quand cela change.
    fn veille(&mut self, m: &mut Monde) {
        let niveaux = m.fenetre.niveaux();
        let visible = !niveaux.is_empty() && niveaux.iter().all(|n| n.visible && !n.reduite);
        if self.page != Some(visible) {
            self.page = Some(visible);
            m.fenetre.page_visible(visible);
        }
    }

    /// `Update-SuiteSurveillance` (systeme.ps1:86-101) : suite.json n'est relu que si sa date a changé ; un nouveau
    /// thème relance l'appli (cachée si elle l'était), comme aujourd'hui (décision de Jordan du 02/11).
    fn surveiller_suite(&mut self, m: &mut Monde) -> Option<Fin> {
        // systeme.ps1:87 : seulement pour une copie gérée
        if !self.reglages.gere {
            return None;
        }
        let fichier = self.reglages.fichier_suite.clone()?;
        // systeme.ps1:88-89 : sa date (absent : la plus petite)
        let date = m.disque.date(&fichier);
        // systeme.ps1:91 : le premier tour note la date, sauf si suite.json était pris au démarrage (son thème n'a
        // pas pu être lu : il l'est au premier tour où il est libre)
        if self.suite_date.is_none() && !self.reglages.suite_prise {
            self.suite_date = Some(date);
            return None;
        }
        // systeme.ps1:92
        if self.suite_date == Some(date) {
            return None;
        }
        // systeme.ps1:93 : un seul essai (le tour suivant réessaie)
        let s = m.disque.lire_suite(&fichier, 1);
        // systeme.ps1:94 : en train d'être remplacé, ou pris : relu au prochain tour
        if s.existe && !s.lisible {
            return None;
        }
        // systeme.ps1:95
        self.suite_date = Some(date);
        // systeme.ps1:96 : `-ne`, sans tenir compte des majuscules
        let theme = s.theme.filter(|t| !t.is_empty() && !egal(t, &self.reglages.theme))?;
        // systeme.ps1:97-99
        m.disque.journal(&format!("Thème de la suite changé : {theme}, relance"));
        let cachee = !m.fenetre.est_visible();
        self.finir(Fin::Relancer { cachee }, m.disque)
    }

    /// `Write-EtatMindset` (systeme.ps1:70-81) : seulement pour une copie gérée (systeme.ps1:71) ; la carte n'est
    /// réécrite que si elle a changé (carte.rs).
    fn ecrire_etat(&mut self, disque: &mut dyn Disque) {
        if self.reglages.gere {
            disque.ecrire_etat();
        }
    }

    /// Après la fin (Mindcare.ps1:90-97) : pour une relance, le verrou et le signal sont rendus, puis la nouvelle
    /// copie est lancée (avec `-Demarrage` si l'ancienne était cachée). Le programme quitte ensuite pour de bon
    /// (lanceur.cs:68-70 : aucun fil de fond ne le garde ouvert sans fenêtre).
    pub fn terminer(&self, fin: Fin, systeme: &mut dyn Systeme, disque: &mut dyn Disque) {
        let Fin::Relancer { cachee } = fin else { return };
        // Mindcare.ps1:92-93 : la place est libérée
        systeme.liberer_verrou();
        systeme.fermer_signal();
        // Mindcare.ps1:94-95
        let arguments: &[&str] = if cachee { &[ARG_DEMARRAGE] } else { &[] };
        match self.reglages.programme.as_deref().filter(|p| disque.existe(p)) {
            Some(exe) => {
                if let Err(e) = systeme.lancer(exe, arguments, None) {
                    disque.journal(&format!("Relance : {e}"));
                }
            }
            // (Mindcare.ps1:96 relançait le script par powershell.exe : il n'y a plus de script)
            None => disque.journal("Relance : programme introuvable"),
        }
    }

    /// Le dossier de données (pour l'appelant).
    pub fn dossier_donnees(&self) -> &Path {
        &self.reglages.dossier_donnees
    }
}
