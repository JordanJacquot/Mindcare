//! « Le faire pour moi » exécuté par le programme : l'action part sur un fil à part avec un FAUX exécuteur (il note ce
//! qu'on lui demande, attend d'être libéré, et rend le code voulu ; jamais rien de lancé), la page suit la jauge,
//! un clic répété ne relance rien, puis fini ou raté, avec le journal.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use mindcare_donnees::IndicesGardes;
use mindcare_moteur::ps::{champ, egal, tableau, texte};
use mindcare_moteur::{Enquete, enquete};
use serde_json::{Value, json};

use super::{Banc, cat};
use crate::pages::Effet;
use crate::pages::{ecrans, ids, textes_reparer as x};
use crate::reparer::{Executeur, Geste, Programme};

/// Le faux exécuteur : ce qu'on lui a demandé ; il attend d'être libéré (au plus 10 s), puis rend `code`.
struct FauxExecuteur {
    lances: Mutex<Vec<Programme>>,
    retenu: (Mutex<bool>, Condvar),
    code: i32,
}

impl FauxExecuteur {
    fn nouveau(code: i32) -> Arc<FauxExecuteur> {
        Arc::new(FauxExecuteur { lances: Mutex::new(Vec::new()), retenu: (Mutex::new(true), Condvar::new()), code })
    }
    fn liberer(&self) {
        *self.retenu.0.lock().unwrap() = false;
        self.retenu.1.notify_all();
    }
    fn lances(&self) -> Vec<Programme> {
        self.lances.lock().unwrap().clone()
    }
}

impl Executeur for FauxExecuteur {
    fn lancer(&self, g: &Geste) -> Result<i32, String> {
        self.lances.lock().unwrap().push(g.programme);
        let garde = self.retenu.0.lock().unwrap();
        let _ = self.retenu.1.wait_timeout_while(garde, Duration::from_secs(10), |r| *r);
        Ok(self.code)
    }
}

fn cepc(e: &Enquete) -> bool {
    e.notes.iter().any(|n| egal(n, "cepc"))
}

/// Une suite de réponses jusqu'à ce test (Windows lu, rien de noté).
fn chemin_vers(test: &str) -> (String, Vec<i64>) {
    let rien = IndicesGardes::default().moteur();
    let cat = cat();
    for p in cat.pannes() {
        let mut file: VecDeque<Vec<i64>> = VecDeque::from([Vec::new()]);
        let mut n = 0;
        while let Some(choix) = file.pop_front() {
            n += 1;
            if n > 3000 {
                break;
            }
            let e = enquete(&cat, p, &choix, None);
            let e = if cepc(&e) { enquete(&cat, p, &choix, Some(&rien)) } else { e };
            let Some(et) = &e.etape else { continue };
            if egal(&texte(champ(et, "Id")), test) {
                return (texte(champ(p, "Id")), choix);
            }
            for k in 0..tableau(champ(et, "Reponses")).len() {
                let mut suite = choix.clone();
                suite.push(k as i64);
                file.push_back(suite);
            }
        }
    }
    panic!("{test} introuvable");
}

/// Le banc arrivé sur ce test, avec cet exécuteur (aucun : celui de départ, qui ne lance rien).
fn banc_sur(test: &str, executeur: Option<Arc<FauxExecuteur>>) -> Banc {
    let mut b = Banc::nouveau(Ok(cat()), false);
    if let Some(e) = executeur {
        b.b.brancher_executeur(e);
    }
    // (Windows déjà lu, rien de noté : le parcours est celui du moteur)
    b.b.pages.indices_du_jour = Some(IndicesGardes::default());
    let (panne, choix) = chemin_vers(test);
    b.action(&format!("panne:{panne}"), Value::Null, ecrans::RACONTE);
    for c in choix {
        if c < 0 {
            b.action(ids::NSP, Value::Null, ecrans::ENQUETE);
        } else {
            b.action(ids::REP, json!(c.to_string()), ecrans::ENQUETE);
        }
    }
    assert_eq!(b.etape().as_deref(), Some(test));
    b
}

/// La page de l'enquête du dernier état envoyé.
fn page(b: &Banc) -> Value {
    b.dernier_etat()["pages"][ecrans::ENQUETE].clone()
}

fn a(page: &Value, quoi: &str) -> bool {
    page.to_string().contains(quoi)
}

/// Les pas de la minuterie jusqu'à ce que la page montre ce texte (au plus 5 s).
fn attendre_page(b: &mut Banc, quoi: &str) {
    let debut = Instant::now();
    while !a(&page(b), quoi) && debut.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
        b.tour(Instant::now());
    }
    assert!(a(&page(b), quoi), "la page ne montre pas « {quoi} »");
}

fn attendre_lancement(f: &FauxExecuteur, n: usize) {
    let debut = Instant::now();
    while f.lances().len() < n && debut.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(f.lances().len(), n);
}

#[test]
fn fait_par_mindcare_avec_une_fausse_execution() {
    let f = FauxExecuteur::nouveau(0);
    let mut b = banc_sur("test-dns", Some(Arc::clone(&f)));
    assert!(a(&page(&b), x::BOUTON));
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    attendre_lancement(&f, 1);
    assert_eq!(f.lances(), [Programme::Systeme("ipconfig.exe")]);
    // Pendant : la jauge, pas de réponses ; d'autres clics et des pas de minuterie ne relancent rien
    let p = page(&b);
    assert!(a(&p, x::PENDANT) && !a(&p, x::BOUTON) && !a(&p, "\"cartes\""));
    for _ in 0..3 {
        b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
        b.tour(Instant::now());
    }
    b.executer(vec![Effet::Reparer(crate::reparer::Reparation::Dns)]);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(f.lances().len(), 1, "relancé");
    assert!(b.journal().contains("Le faire pour moi : dns pas lancé, dns tourne encore"));
    // Libéré : fini
    f.liberer();
    attendre_page(&mut b, x::FAIT);
    let p = page(&b);
    assert!(a(&p, "\"cartes\"") && !a(&p, x::PENDANT));
    let j = b.journal();
    assert!(j.contains("Le faire pour moi : dns (test-dns)"), "{j}");
    assert!(j.contains("Le faire pour moi (dns) : ipconfig.exe, fini (code 0)"), "{j}");
    assert!(j.contains("Le faire pour moi : fait"), "{j}");
    assert_eq!(f.lances().len(), 1);
}

#[test]
fn rate_avec_une_fausse_execution() {
    let f = FauxExecuteur::nouveau(1);
    f.liberer();
    let mut b = banc_sur("test-vcredist", Some(Arc::clone(&f)));
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    attendre_page(&mut b, x::RATE);
    // (le premier raté arrête tout : le second winget n'est jamais demandé)
    assert_eq!(f.lances(), [Programme::Winget]);
    let p = page(&b);
    assert!(a(&p, x::RATE_RIEN) && a(&p, "\"case\"") && a(&p, "\"cartes\""));
    let j = b.journal();
    assert!(j.contains("Le faire pour moi (visual-cpp) : winget.exe, raté (code 1, 0x00000001)"), "{j}");
    assert!(j.contains("Le faire pour moi : raté (winget.exe : code 1)"), "{j}");
}

#[test]
fn sans_executeur_rate_proprement() {
    let mut b = banc_sur("test-cache-icones", None);
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    attendre_page(&mut b, x::RATE);
    assert!(b.journal().contains("rien pour lancer les gestes"));
}

#[test]
fn ouvrir_windows_update_par_le_faux_exterieur() {
    let mut b = Banc::nouveau(Ok(cat()), false);
    b.executer(vec![Effet::OuvrirWindowsUpdate]);
    assert!(b.racine.join("windows-update.txt").is_file());
    assert!(b.journal().contains("Windows Update ouvert"));
}
