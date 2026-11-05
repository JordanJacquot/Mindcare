//! Les écartés « hors sujet » (le bloc d'alimentation d'un portable) : `Enquete::hors_sujet` reconnaît chaque suspect
//! que l'enquête écarte parce qu'il ne concerne pas ce type de PC, jamais un suspect qui reste, et `ecartes_montres`
//! garde tous les autres. L'enquête elle-même ne change pas : les juges (parcours.rs, cas_nommes.rs) le vérifient.

mod commun;

use std::collections::VecDeque;

use commun::{catalogue, panne};
use mindcare_moteur::ps::{champ, tableau};
use mindcare_moteur::{HORS_SUJET, enquete};

#[test]
fn hors_sujet_reconnu_sur_toutes_les_pannes() {
    let cat = catalogue();
    let (mut hors_sujet, mut vrais) = (0, 0);
    for p in cat.pannes() {
        let mut file: VecDeque<Vec<i64>> = VecDeque::from([Vec::new()]);
        let mut n = 0;
        while let Some(choix) = file.pop_front() {
            n += 1;
            if n > 400 {
                break;
            }
            let e = enquete(cat, p, &choix, None);
            for x in &e.ecartes {
                if x.raison == HORS_SUJET {
                    assert!(e.hors_sujet(cat, &x.id), "{} {choix:?} : {} pas reconnu", x.id, x.raison);
                    hors_sujet += 1;
                }
            }
            for id in &e.restants {
                assert!(!e.hors_sujet(cat, id), "{id} {choix:?} : un suspect qui reste n'est jamais hors sujet");
            }
            let montres: Vec<_> = e.ecartes_montres(cat).collect();
            assert!(montres.iter().all(|x| x.raison != HORS_SUJET), "{choix:?} : un hors sujet montré");
            vrais += montres.len();
            if let Some(et) = &e.etape {
                for k in 0..tableau(champ(et, "Reponses")).len() {
                    let mut suite = choix.clone();
                    suite.push(k as i64);
                    file.push_back(suite);
                }
            }
        }
    }
    assert!(hors_sujet > 0 && vrais > 0, "cas : {hors_sujet} hors sujet, {vrais} vrais écartés");
}

/// Un portable qui s'éteint d'un coup : le bloc d'alimentation est écarté d'office, il est hors sujet ; une piste
/// écartée par une vraie réponse reste montrée.
#[test]
fn le_bloc_d_alimentation_d_un_portable() {
    let cat = catalogue();
    let p = panne("seteint");
    let mut file: VecDeque<Vec<i64>> = VecDeque::from([Vec::new()]);
    let mut trouve = false;
    while let Some(choix) = file.pop_front() {
        let e = enquete(cat, p, &choix, None);
        if e.notes.iter().any(|n| n == "portable") && e.ecartes.iter().any(|x| x.id == "alimentation") {
            assert!(e.hors_sujet(cat, "alimentation"), "{choix:?}");
            assert!(e.ecartes_montres(cat).all(|x| x.id != "alimentation"), "{choix:?}");
            if e.ecartes_montres(cat).any(|x| !x.raison.is_empty()) {
                trouve = true;
                break;
            }
        }
        if let Some(et) = &e.etape {
            for k in 0..tableau(champ(et, "Reponses")).len() {
                let mut suite = choix.clone();
                suite.push(k as i64);
                file.push_back(suite);
            }
        }
    }
    assert!(trouve, "aucun parcours de « seteint » sur un portable avec une vraie piste écartée");
}
