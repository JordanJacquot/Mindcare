//! Les divergences voulues avec la lecture de Windows de PowerShell (tests\juges\windows-divergences.json, audit du
//! 25/10).
//!
//! Constat 20 : une mesure ratée qui n'a rien vu ne donne aucun indice (le moteur n'écarte rien) ; PowerShell rendait
//! « pas vu ». Chaque cas garde les deux réponses : celle de PowerShell, et la voulue (celle de PowerShell sans les
//! indices « Absents »), que le Rust doit rendre exactement. Les cas du moteur (mindcare-moteur,
//! tests\juges\divergences.json) partent des mêmes indices ratés : on vérifie qu'ils restent d'accord.
//!
//! Constat 21 : au début d'une enquête, une lecture finie lancée il y a plus de 10 minutes est relancée ; PowerShell ne
//! relit jamais.

mod commun;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use commun::{FauxWindows, HorlogeFixe, ecart, indices_juge, lire_juge, noms, verdict_final};
use mindcare_windows::{
    Barrette, Batterie, Disque, DisquePhysique, Evenement, Filtre, InfosPc, JOURS, LectureJournaux, Mesure, Moment,
    Partition, Place, SourceWindows, indices_windows,
};
use serde_json::{Value, json};

#[test]
fn constat_20_mesure_ratee_sans_indice() {
    let juge = lire_juge("windows-divergences.json");
    let cas = juge["Constat20"].as_array().expect("Constat20");
    let mut ecarts = Vec::new();
    for c in cas {
        let nom = c["Nom"].as_str().unwrap_or_default();
        let faux = FauxWindows::depuis(&c["Donnees"], &[]);
        let obtenu = indices_windows(&faux, faux.maintenant, JOURS);
        let voulus = indices_juge(&c["IndicesVoulus"]);
        let powershell = indices_juge(&c["IndicesPowerShell"]);
        let absents: Vec<&str> = c["Absents"].as_array().expect("Absents").iter().filter_map(Value::as_str).collect();
        if let Some(d) = ecart(&voulus, &obtenu) {
            ecarts.push(format!("« {nom} » (réponse voulue) :\n    {d}"));
        }
        // (les absents sont des indices que PowerShell donnait « pas vu », et seulement eux diffèrent)
        for a in &absents {
            match powershell.get(*a) {
                Some(i) if !i.vu => {}
                autre => ecarts.push(format!("« {nom} » : {a} devrait être « pas vu » chez PowerShell ({autre:?})")),
            }
            if obtenu.contains_key(*a) {
                ecarts.push(format!("« {nom} » : {a} ne devrait pas être donné"));
            }
        }
        let mut reste = powershell.clone();
        reste.retain(|k, _| !absents.contains(&k.as_str()));
        if reste != voulus {
            ecarts.push(format!("« {nom} » : les indices voulus ne sont pas ceux de PowerShell moins les absents"));
        }
    }
    let avec_absents = cas.iter().filter(|c| c["Absents"].as_array().is_some_and(|a| !a.is_empty())).count();
    assert!(avec_absents >= 15, "trop peu de mesures ratées dans le juge ({avec_absents})");
    verdict_final("divergences (constat 20)", cas.len(), &ecarts);
}

/// Les 5 cas du moteur (mindcare-moteur) supposent qu'une mesure ratée de « allume », « placelibre » et
/// « usurebatterie » donnait chez PowerShell { Vu faux, textes vides, pas partiel }, et que la réponse voulue n'a pas
/// d'indice : c'est bien ce que disent les juges de la lecture de Windows.
#[test]
fn constat_20_d_accord_avec_le_moteur() {
    let moteur = lire_juge("../../../mindcare-moteur/tests/juges/divergences.json");
    let juge = lire_juge("windows-divergences.json");
    let cas = juge["Constat20"].as_array().expect("Constat20");
    let mut vus = 0;
    for m in moteur["Divergences"].as_array().expect("Divergences") {
        let mesure = m["Mesure"].as_str().expect("Mesure");
        assert_eq!(m["IndicesVoulus"], json!([]), "moteur : réponse voulue sans indice pour {mesure}");
        let attendu = m["IndicesPowerShell"].as_array().and_then(|l| l.first()).expect("IndicesPowerShell");
        let c = cas
            .iter()
            .find(|c| c["Absents"] == json!([mesure]))
            .unwrap_or_else(|| panic!("aucune mesure ratée de « {mesure} » seule dans windows-divergences.json"));
        let ps = indices_juge(&c["IndicesPowerShell"]);
        let i = ps.get(mesure).expect("indice de PowerShell");
        assert_eq!(
            (i.vu, i.texte.as_str(), i.texte_ok.as_str(), i.partiel),
            (
                attendu["Vu"].as_bool().unwrap_or(true),
                attendu["Texte"].as_str().unwrap_or("?"),
                attendu["TexteOk"].as_str().unwrap_or("?"),
                attendu["Partiel"].as_bool().unwrap_or(true)
            ),
            "{mesure} : PowerShell ne donne pas ce que le moteur suppose"
        );
        let faux = FauxWindows::depuis(&c["Donnees"], &[]);
        let obtenu = indices_windows(&faux, faux.maintenant, JOURS);
        assert!(!obtenu.contains_key(mesure), "{mesure} donné par le Rust ({:?})", noms(&obtenu));
        vus += 1;
    }
    assert_eq!(vus, 5);
}

/// Un faux Windows qu'on peut retenir (la lecture attend qu'on le libère), qui compte ses lectures, et peut planter.
struct Retenu {
    faux: FauxWindows,
    ouvert: Mutex<bool>,
    signal: Condvar,
    lectures: AtomicUsize,
    plante: bool,
}

impl Retenu {
    fn nouveau(ouvert: bool, plante: bool) -> Arc<Retenu> {
        let d = json!({ "Maintenant": "2026-10-31T14:30:00", "Sonde": { "Debut": "2026-08-01T00:00:00" }, "Erreurs": {},
            "Lecteur": "C:", "Place": { "Libre": 1, "Taille": 2 }, "Demarrage": "2026-10-30T00:00:00" });
        Arc::new(Retenu {
            faux: FauxWindows::depuis(&d, &[]),
            ouvert: Mutex::new(ouvert),
            signal: Condvar::new(),
            lectures: AtomicUsize::new(0),
            plante,
        })
    }

    fn ouvrir(&self) {
        *self.ouvert.lock().expect("verrou") = true;
        self.signal.notify_all();
    }
}

impl SourceWindows for Retenu {
    fn plus_ancien(&self) -> Mesure<Option<Moment>> {
        self.lectures.fetch_add(1, Ordering::SeqCst);
        {
            let g = self.ouvert.lock().expect("verrou");
            let _g = self.signal.wait_while(g, |o| !*o).expect("verrou");
        }
        assert!(!self.plante, "faux Windows qui plante (voulu par le test)");
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

#[test]
fn constat_21_relecture_au_debut_d_une_enquete() {
    let juge = lire_juge("windows-divergences.json");
    let cas = juge["Constat21"].as_array().expect("Constat21");
    let t0 = Moment::lire("2026-10-31T14:30:00").expect("date");
    let attente = Duration::from_secs(10);
    let mut ecarts = Vec::new();
    for c in cas {
        let nom = c["Nom"].as_str().unwrap_or_default();
        assert_eq!(c["PowerShell"], json!(false), "« {nom} » : PowerShell ne relit jamais");
        let etat = c["Etat"].as_str().expect("Etat");
        let source = Retenu::nouveau(etat != "en cours" && etat != "abandon", etat == "erreur");
        let horloge = Arc::new(HorlogeFixe::nouvelle(t0));
        let mut l = LectureJournaux::nouvelle(source.clone(), horloge.clone());
        if etat != "jamais" {
            l.lancer();
        }
        match etat {
            "lue" => assert!(l.attendre(attente) && l.indices().is_some(), "« {nom} » : pas lue"),
            "erreur" => {
                assert!(l.attendre(attente) && l.indices().is_none(), "« {nom} » : pas ratée");
                assert!(l.cause_absence().is_some_and(|c| c.contains("erreur")), "« {nom} » : {:?}", l.cause_absence());
            }
            "abandon" => {
                assert!(!l.lus(), "« {nom} » : finie trop tôt");
                l.arreter_attente();
                assert!(l.lus() && l.indices().is_none(), "« {nom} » : pas abandonnée");
                source.ouvrir();
                assert!(l.attendre(attente));
                assert!(l.indices().is_none(), "« {nom} » : une lecture abandonnée ne sert plus");
            }
            _ => {}
        }
        horloge.regler(t0.plus_minutes(c["Lancee"].as_i64().expect("Lancee")));
        let relue = l.debut_enquete();
        if relue != c["Relue"].as_bool().expect("Relue") {
            ecarts.push(format!("« {nom} » : relue {relue}, attendu {}", c["Relue"]));
        }
        if relue {
            source.ouvrir();
            if source.plante {
                // (la relecture plante aussi : elle a bien été relancée)
                assert!(l.attendre(attente));
            } else {
                assert!(l.attendre(attente) && l.indices().is_some(), "« {nom} » : relue sans indices");
            }
            assert_eq!(source.lectures.load(Ordering::SeqCst), 2, "« {nom} » : deux lectures attendues");
            assert_eq!(l.lancee_a(), Some(horloge.0.lock().map(|m| *m).expect("horloge")));
            // (relue à l'instant : pas relue une seconde fois)
            assert!(!l.debut_enquete(), "« {nom} » : relue deux fois");
        } else {
            let attendues = usize::from(etat != "jamais");
            // (le fil peut ne pas avoir encore commencé sa lecture : on lui en laisse le temps)
            let debut = std::time::Instant::now();
            while source.lectures.load(Ordering::SeqCst) < attendues && debut.elapsed() < attente {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(source.lectures.load(Ordering::SeqCst), attendues, "« {nom} »");
        }
        source.ouvrir();
    }
    verdict_final("divergences (constat 21)", cas.len(), &ecarts);
}
