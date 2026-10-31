//! La lecture en arrière-plan répond comme Start-LectureJournaux, Test-JournauxLus, Stop-AttenteJournaux, Get-Indices
//! et Stop-LectureJournaux : pas lancée, rien à attendre et aucun indice ; en cours, pas lue et aucun indice ; finie,
//! les indices de Get-IndicesWindows ; abandonnée, plus jamais d'indices (jusqu'à la relecture du constat 21) ;
//! plantée ou arrêtée, aucun indice et la raison pour le journal de Mindcare.

mod commun;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use commun::{FauxWindows, HorlogeFixe, lire_juge};
use mindcare_windows::{
    Barrette, Batterie, Disque, DisquePhysique, Evenement, Filtre, InfosPc, JOURS, LectureJournaux, Mesure, Moment,
    Partition, Place, SourceWindows, indices_windows, lecture,
};

/// Le faux Windows du cas « PC en mauvais état, tout à la fois », qu'on peut retenir ou faire planter.
struct Porte {
    faux: FauxWindows,
    ouverte: Mutex<bool>,
    signal: Condvar,
    plante: AtomicBool,
}

impl Porte {
    fn nouvelle(ouverte: bool) -> Arc<Porte> {
        let juge = lire_juge("windows-cas.json");
        let c = juge["Cas"]
            .as_array()
            .and_then(|l| l.iter().find(|c| c["Nom"] == "PC en mauvais état, tout à la fois"))
            .expect("cas « tout à la fois »");
        Arc::new(Porte {
            faux: FauxWindows::depuis(&c["Donnees"], &[]),
            ouverte: Mutex::new(ouverte),
            signal: Condvar::new(),
            plante: AtomicBool::new(false),
        })
    }

    fn ouvrir(&self) {
        *self.ouverte.lock().expect("verrou") = true;
        self.signal.notify_all();
    }
}

impl SourceWindows for Porte {
    fn plus_ancien(&self) -> Mesure<Option<Moment>> {
        {
            let g = self.ouverte.lock().expect("verrou");
            let _g = self.signal.wait_while(g, |o| !*o).expect("verrou");
        }
        assert!(!self.plante.load(Ordering::SeqCst), "faux Windows qui plante (voulu par le test)");
        self.faux.plus_ancien()
    }
    fn evenements(&self, f: &Filtre) -> Mesure<Vec<Evenement>> {
        self.faux.evenements(f)
    }
    fn disques(&self) -> Mesure<Vec<Disque>> {
        self.faux.disques()
    }
    fn partitions(&self) -> Mesure<Vec<Partition>> {
        self.faux.partitions()
    }
    fn disques_physiques(&self) -> Mesure<Vec<DisquePhysique>> {
        self.faux.disques_physiques()
    }
    fn lecteur_systeme(&self) -> String {
        self.faux.lecteur_systeme()
    }
    fn place(&self, l: &str) -> Mesure<Place> {
        self.faux.place(l)
    }
    fn batterie(&self) -> Mesure<Option<Batterie>> {
        self.faux.batterie()
    }
    fn demarrage(&self) -> Mesure<Moment> {
        self.faux.demarrage()
    }
    fn barrettes(&self) -> Mesure<Vec<Barrette>> {
        self.faux.barrettes()
    }
    fn infos_pc(&self) -> Mesure<InfosPc> {
        self.faux.infos_pc()
    }
}

const ATTENTE: Duration = Duration::from_secs(10);

fn lecture_de(p: &Arc<Porte>) -> LectureJournaux {
    LectureJournaux::nouvelle(p.clone(), Arc::new(HorlogeFixe::nouvelle(p.faux.maintenant)))
}

#[test]
fn pas_lancee() {
    let p = Porte::nouvelle(true);
    let l = lecture_de(&p);
    assert!(l.lus(), "rien à attendre");
    assert!(l.attendre(Duration::ZERO));
    assert!(l.indices().is_none());
    assert_eq!(l.cause_absence().as_deref(), Some("lecture pas lancée"));
}

#[test]
fn finie_rend_les_indices_de_get_indiceswindows() {
    let p = Porte::nouvelle(true);
    let mut l = lecture_de(&p);
    l.lancer();
    assert!(l.attendre(ATTENTE));
    let attendu = indices_windows(&p.faux, p.faux.maintenant, JOURS);
    assert_eq!(l.indices().as_deref(), Some(&attendu));
    assert!(l.indices().is_some_and(|i| i["coupures"].vu), "le cas a des redémarrages brutaux");
    assert_eq!(l.cause_absence(), None);
}

#[test]
fn en_cours_puis_abandonnee() {
    let p = Porte::nouvelle(false);
    let mut l = lecture_de(&p);
    l.lancer();
    assert!(!l.lus(), "la lecture est retenue");
    assert!(!l.attendre(Duration::from_millis(50)));
    assert!(l.indices().is_none(), "pas encore lue : aucun indice");
    assert_eq!(l.cause_absence(), None, "pas encore lue : rien à noter");
    l.arreter_attente();
    assert!(l.lus(), "abandonnée : plus rien à attendre");
    assert!(l.indices().is_none());
    assert_eq!(l.cause_absence().as_deref(), Some("trop longs à lire, enquête continuée sans eux"));
    // (la lecture finit ensuite : l'enquête ne s'en sert toujours pas, les suspects ne changent pas en route)
    p.ouvrir();
    assert!(l.attendre(ATTENTE));
    assert!(l.indices().is_none());
    // (finie avant l'abandon : arreter_attente ne fait rien)
    let mut l2 = lecture_de(&p);
    l2.lancer();
    assert!(l2.attendre(ATTENTE));
    l2.arreter_attente();
    assert!(l2.indices().is_some());
}

#[test]
fn plantee() {
    let p = Porte::nouvelle(true);
    p.plante.store(true, Ordering::SeqCst);
    let mut l = lecture_de(&p);
    l.lancer();
    assert!(l.attendre(ATTENTE), "une lecture plantée est finie");
    assert!(l.indices().is_none());
    assert!(l.cause_absence().is_some_and(|c| c.contains("erreur")));
}

#[test]
fn arretee() {
    let p = Porte::nouvelle(false);
    let mut l = lecture_de(&p);
    l.lancer();
    l.arreter();
    p.ouvrir();
    assert!(l.attendre(ATTENTE));
    assert!(l.indices().is_none(), "arrêtée : son résultat est jeté");
    assert_eq!(l.cause_absence().as_deref(), Some("lecture finie sans résultat"));
}

#[test]
fn regle_des_dix_minutes() {
    let t = Moment::lire("2026-10-31T12:00:00").expect("date");
    assert!(!lecture::a_relire(t, t));
    assert!(!lecture::a_relire(t, t.plus_minutes(10)));
    assert!(lecture::a_relire(t, Moment(t.plus_minutes(10).0 + 1)));
    assert!(lecture::a_relire(t, t.plus_minutes(-1)), "horloge revenue en arrière");
}
