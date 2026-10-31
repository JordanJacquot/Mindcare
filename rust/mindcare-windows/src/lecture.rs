//! La lecture en arrière-plan (Start-LectureJournaux, Test-JournauxLus, Stop-AttenteJournaux, Get-Indices,
//! Stop-LectureJournaux) : un fil qui lit ce que Windows a noté, pendant que la fenêtre reste libre. L'enquête
//! l'attend sans figer la fenêtre ; si la lecture prend trop de temps, l'enquête continue sans elle et ne s'en sert
//! plus ensuite (sinon les suspects changeraient en cours de route quand elle finirait).
//!
//! **Divergence voulue (constat 21)** : PowerShell lit les indices une seule fois, au démarrage ; une enquête
//! commencée des heures plus tard se sert d'indices périmés. Ici, au début d'une enquête
//! ([`LectureJournaux::debut_enquete`]), une lecture finie depuis plus de [`RELIRE_APRES_MINUTES`] minutes (comptées
//! depuis son lancement) est relancée.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use crate::indices::{IndicesWindows, JOURS, lire};
use crate::moment::{Moment, TICKS_PAR_MINUTE};
use crate::source::{Horloge, SourceWindows};

/// Les indices sont relus au début d'une enquête s'ils ont plus de 10 minutes (constat 21).
pub const RELIRE_APRES_MINUTES: i64 = 10;

/// Faut-il relire des indices lus à `lus_a` ? Plus de 10 minutes, ou une horloge revenue en arrière (heure changée) :
/// oui.
pub fn a_relire(lus_a: Moment, maintenant: Moment) -> bool {
    maintenant.0 - lus_a.0 > RELIRE_APRES_MINUTES * TICKS_PAR_MINUTE || maintenant < lus_a
}

/// Où en est un fil de lecture.
#[derive(Debug, Clone)]
enum Resultat {
    EnCours,
    Lus(Arc<IndicesWindows>),
    /// La lecture a échoué (`$script:Journaux.Erreur`).
    Erreur(String),
    /// Arrêtée par Stop-LectureJournaux.
    Arretee,
}

struct Partage {
    resultat: Mutex<Resultat>,
    fini: Condvar,
    arret: AtomicBool,
}

impl Partage {
    fn resultat(&self) -> MutexGuard<'_, Resultat> {
        // (un fil qui a paniqué en tenant le verrou : le résultat reste lisible)
        self.resultat.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn finir(&self, r: Resultat) {
        *self.resultat() = r;
        self.fini.notify_all();
    }
}

struct Tache {
    lancee_a: Moment,
    partage: Arc<Partage>,
}

/// La lecture de ce que Windows a noté, en arrière-plan.
pub struct LectureJournaux {
    source: Arc<dyn SourceWindows>,
    horloge: Arc<dyn Horloge>,
    tache: Option<Tache>,
    /// L'enquête a continué sans la lecture (`$script:JournauxAbandon`).
    abandon: bool,
    /// Une erreur avant même de lancer le fil.
    erreur: Option<String>,
}

impl LectureJournaux {
    /// Une lecture pas encore lancée.
    pub fn nouvelle(source: Arc<dyn SourceWindows>, horloge: Arc<dyn Horloge>) -> LectureJournaux {
        LectureJournaux { source, horloge, tache: None, abandon: false, erreur: None }
    }

    /// Start-LectureJournaux : lance la lecture dans un fil à part (une lecture déjà en cours est arrêtée).
    pub fn lancer(&mut self) {
        self.arreter();
        self.abandon = false;
        self.erreur = None;
        let partage = Arc::new(Partage {
            resultat: Mutex::new(Resultat::EnCours),
            fini: Condvar::new(),
            arret: AtomicBool::new(false),
        });
        let maintenant = self.horloge.maintenant();
        let (source, p) = (Arc::clone(&self.source), Arc::clone(&partage));
        let fil = std::thread::Builder::new().name("mindcare-journaux".to_string()).spawn(move || {
            let lu = catch_unwind(AssertUnwindSafe(|| lire(source.as_ref(), maintenant, JOURS, Some(&p.arret))));
            p.finir(match lu {
                Ok(Ok(i)) => Resultat::Lus(Arc::new(i)),
                Ok(Err(_)) => Resultat::Arretee,
                Err(_) => Resultat::Erreur("la lecture s'est arrêtée sur une erreur".to_string()),
            });
        });
        match fil {
            Ok(_) => self.tache = Some(Tache { lancee_a: maintenant, partage }),
            Err(e) => self.erreur = Some(format!("lecture impossible ({e})")),
        }
    }

    /// Test-JournauxLus : la lecture est-elle finie (ou abandonnée) ? Jamais d'attente ici. Pas lancée : rien à
    /// attendre.
    pub fn lus(&self) -> bool {
        if self.abandon || self.erreur.is_some() {
            return true;
        }
        match &self.tache {
            Some(t) => !matches!(*t.partage.resultat(), Resultat::EnCours),
            None => true,
        }
    }

    /// Attend la fin de la lecture, au plus `delai` ; vrai si elle est finie.
    pub fn attendre(&self, delai: Duration) -> bool {
        let Some(t) = &self.tache else { return self.lus() };
        let r = t.partage.resultat();
        let (r, _) = t
            .partage
            .fini
            .wait_timeout_while(r, delai, |r| matches!(r, Resultat::EnCours))
            .unwrap_or_else(|e| e.into_inner());
        drop(r);
        self.lus()
    }

    /// Stop-AttenteJournaux : la lecture prend trop de temps, l'enquête continue sans elle et ne s'en servira plus.
    pub fn arreter_attente(&mut self) {
        if !self.lus() {
            self.abandon = true;
        }
    }

    /// Get-Indices : les indices, si la lecture est finie et a réussi (sinon aucun : rien, l'équivalent du `@{}` de
    /// PowerShell).
    pub fn indices(&self) -> Option<Arc<IndicesWindows>> {
        if !self.lus() || self.abandon {
            return None;
        }
        match self.tache.as_ref().map(|t| t.partage.resultat().clone()) {
            Some(Resultat::Lus(i)) => Some(i),
            _ => None,
        }
    }

    /// Pourquoi il n'y a pas d'indices (le texte du journal de Mindcare : « Journaux de Windows non lus : ... »).
    pub fn cause_absence(&self) -> Option<String> {
        if !self.lus() {
            return None;
        }
        if self.abandon {
            return Some("trop longs à lire, enquête continuée sans eux".to_string());
        }
        if let Some(e) = &self.erreur {
            return Some(e.clone());
        }
        match self.tache.as_ref().map(|t| t.partage.resultat().clone()) {
            None => Some("lecture pas lancée".to_string()),
            Some(Resultat::Erreur(e)) => Some(e),
            Some(Resultat::Lus(_)) => None,
            Some(Resultat::EnCours | Resultat::Arretee) => Some("lecture finie sans résultat".to_string()),
        }
    }

    /// Stop-LectureJournaux : arrête la lecture (le fil s'arrête entre deux mesures, son résultat est jeté).
    pub fn arreter(&mut self) {
        if let Some(t) = &self.tache {
            t.partage.arret.store(true, Ordering::SeqCst);
        }
    }

    /// Quand la lecture actuelle a été lancée.
    pub fn lancee_a(&self) -> Option<Moment> {
        self.tache.as_ref().map(|t| t.lancee_a)
    }

    /// Le début d'une enquête (constat 21) : une lecture finie (lue, ratée ou abandonnée) lancée il y a plus de
    /// 10 minutes est relancée. Une lecture en cours ou jamais lancée n'est pas touchée. Vrai si elle est relancée.
    pub fn debut_enquete(&mut self) -> bool {
        let Some(t) = &self.tache else { return false };
        let finie = !matches!(*t.partage.resultat(), Resultat::EnCours);
        if finie && a_relire(t.lancee_a, self.horloge.maintenant()) {
            self.lancer();
            return true;
        }
        false
    }
}

impl Drop for LectureJournaux {
    fn drop(&mut self) {
        self.arreter();
    }
}
