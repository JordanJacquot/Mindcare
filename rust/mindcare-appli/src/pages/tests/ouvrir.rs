//! « Ouvrir Nevermind » dans les pages, avec le vrai catalogue : la ligne « Nevermind règle ça pour toi » dans la
//! réponse des 6 pannes choisies (« Ouvrir Nevermind » s'il est installé, sinon « L'installer avec Mindset »), jamais
//! pour les autres pannes, jamais sur une étape, jamais quand Mindcare n'est pas affiché dans Mindset ou que l'on ne
//! sait pas encore si Nevermind est installé. Le clic rend la demande une seule fois pendant l'attente ; la fin est
//! donnée ici à la main (rien n'est écrit : c'est le travail d'effets.rs, testé dans effets\tests\ouvrir.rs).

use serde_json::Value;

use mindcare_donnees::IndicesGardes;
use mindcare_moteur::Appli;
use mindcare_moteur::ps::{champ, texte};

use super::{Banc, actions_montrees, blocs, cat, toutes_acceptees};
use crate::ouvrir::Fin;
use crate::pages::{Effet, Vue, ecrans, ids, textes_ouvrir as x};

/// Les pannes choisies avec Jordan.
const PANNES: [&str; 6] = ["saccades", "saccadesdebut", "inputlag", "laggame", "frequence", "processeurbride"];

/// Les pages dans Mindset (ou pas), Nevermind installé, pas installé, ou pas encore lu.
fn banc(dans_mindset: bool, installe: Option<bool>) -> Banc {
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    b.etat.dans_mindset = dans_mindset;
    b.etat.nevermind_installe = installe;
    b
}

/// Commence l'enquête de cette panne, et répond « Je ne sais pas » jusqu'à la réponse finale.
fn jusqu_a_la_reponse(b: &mut Banc, panne: &str) {
    b.commencer(panne);
    for _ in 0..80 {
        match b.vue() {
            Some(Vue::Reponse(_)) => return,
            Some(Vue::Etape(_)) => {
                b.action(ids::NSP, Value::Null, ecrans::ENQUETE);
            }
            _ => panic!("{panne} : ni étape ni réponse"),
        }
    }
    panic!("{panne} : pas de réponse finale");
}

/// Le bouton « Ouvrir Nevermind » de la page de l'enquête, s'il est montré (son texte).
fn bouton(b: &Banc) -> Option<String> {
    let montre = actions_montrees(&b.json).iter().any(|(id, _, e)| id == ids::OUVRIR && e == ecrans::ENQUETE);
    let ligne = blocs(b.enquete(), "ligne").into_iter().find(|l| l["bouton"]["id"] == ids::OUVRIR).cloned();
    assert_eq!(montre, ligne.is_some());
    ligne.map(|l| l["bouton"]["texte"].as_str().unwrap_or_default().to_string())
}

#[test]
fn la_ligne_pour_les_six_pannes_installe_ou_pas() {
    for panne in PANNES {
        for installe in [true, false] {
            let mut b = banc(true, Some(installe));
            // (pas sur une étape : seulement dans la réponse)
            assert!(matches!(b.vue(), None | Some(Vue::Etape(_))));
            b.commencer(panne);
            if matches!(b.vue(), Some(Vue::Etape(_))) {
                assert_eq!(bouton(&b), None, "{panne} : sur une étape");
            }
            jusqu_a_la_reponse(&mut b, panne);
            b.propre();
            let attendu = if installe { x::OUVRIR } else { x::INSTALLER };
            assert_eq!(bouton(&b).as_deref(), Some(attendu), "{panne}");
            // La ligne de la maquette, juste sous « Ce qu'il faut faire »
            let page = b.enquete().clone();
            let liste = page["blocs"].as_array().unwrap();
            let k = liste.iter().position(|l| l["bouton"]["id"] == ids::OUVRIR).unwrap();
            assert_eq!(liste[k - 1]["type"], "titre", "{panne}");
            assert_eq!(liste[k - 1]["texte"], "Ce qu'il faut faire", "{panne}");
            assert_eq!(liste[k]["texte"], x::TITRE);
            assert_eq!(liste[k]["note"], x::NOTE);
            assert_eq!(liste[k]["icone"], "jauge");
            assert!(toutes_acceptees(&b.etat, &b.json).is_empty(), "{panne}");
            // Le clic : la demande, une seule fois pendant l'attente (installé ou pas : la même)
            assert_eq!(b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE), [Effet::OuvrirAppli(Appli::Nevermind)]);
            assert_eq!(b.etat.ouverture, Some(Appli::Nevermind));
            for _ in 0..3 {
                let effets = b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE);
                assert!(!effets.iter().any(|e| matches!(e, Effet::OuvrirAppli(_))), "{panne} : {effets:?}");
            }
            assert_eq!(b.ouvertures, [Appli::Nevermind], "{panne}");
            // Mindset ne l'a pas lue : la bulle, et un nouveau clic redemande
            let effets = b.etat.ouverture_finie(&Fin::Reprise);
            assert_eq!(effets, [Effet::Message { texte: x::DEPUIS_L_ACCUEIL.into(), bravo: false }]);
            assert_eq!(b.etat.ouverture, None);
            assert_eq!(b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE), [Effet::OuvrirAppli(Appli::Nevermind)]);
            // Lue par Mindset : rien à dire (il ouvre Nevermind)
            assert_eq!(b.etat.ouverture_finie(&Fin::Lue), Vec::<Effet>::new());
            let ratee = b.etat.ouverture_finie(&Fin::Ratee("disque plein".into()));
            assert_eq!(ratee, [Effet::Message { texte: x::DEPUIS_L_ACCUEIL.into(), bravo: false }]);
            assert_eq!(b.ouvertures.len(), 2);
            b.propre();
        }
    }
}

#[test]
fn jamais_pour_les_autres_pannes() {
    for p in cat().pannes() {
        let id = texte(champ(p, "Id"));
        if PANNES.contains(&id.as_str()) {
            continue;
        }
        let mut b = banc(true, Some(true));
        jusqu_a_la_reponse(&mut b, &id);
        assert_eq!(bouton(&b), None, "{id}");
        // (même cliqué quand même : rien n'est demandé)
        let effets = b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE);
        assert!(!effets.iter().any(|e| matches!(e, Effet::OuvrirAppli(_))), "{id} : {effets:?}");
        assert_eq!(b.etat.ouverture, None);
        b.propre();
    }
}

#[test]
fn jamais_hors_de_mindset_ni_avant_de_savoir() {
    for (dans, installe) in [(false, Some(true)), (false, Some(false)), (false, None), (true, None)] {
        for panne in PANNES {
            let mut b = banc(dans, installe);
            jusqu_a_la_reponse(&mut b, panne);
            assert_eq!(bouton(&b), None, "{panne} ({dans}, {installe:?})");
            let effets = b.action(ids::OUVRIR, Value::Null, ecrans::ENQUETE);
            assert!(!effets.iter().any(|e| matches!(e, Effet::OuvrirAppli(_))), "{panne} : {effets:?}");
            assert!(b.ouvertures.is_empty());
            b.propre();
        }
    }
    // Un état neuf (Mindcare seul, ou Mindcare Mobile qui ne lit pas le champ) : rien
    let b = Banc::nouveau(None);
    assert!(!b.etat.dans_mindset && b.etat.nevermind_installe.is_none() && b.etat.ouverture.is_none());
}

#[test]
fn la_page_suit_mindset_et_l_installation() {
    let mut b = banc(false, None);
    jusqu_a_la_reponse(&mut b, "saccades");
    assert_eq!(bouton(&b), None);
    // (redessiner seulement quand la ligne change)
    assert!(!b.etat.noter_nevermind(true), "hors de Mindset, rien ne change à l'écran");
    assert!(b.etat.noter_dans_mindset(true));
    b.executer(vec![Effet::Afficher { ecran: None }]);
    assert_eq!(bouton(&b).as_deref(), Some(x::OUVRIR));
    assert!(!b.etat.noter_dans_mindset(true));
    assert!(!b.etat.noter_nevermind(true));
    assert!(b.etat.noter_nevermind(false));
    b.executer(vec![Effet::Afficher { ecran: None }]);
    assert_eq!(bouton(&b).as_deref(), Some(x::INSTALLER));
    assert!(b.etat.noter_dans_mindset(false));
    b.executer(vec![Effet::Afficher { ecran: None }]);
    assert_eq!(bouton(&b), None);
}

#[test]
fn textes_sans_tiret_long() {
    for s in [x::TITRE, x::NOTE, x::OUVRIR, x::INSTALLER, x::DEPUIS_L_ACCUEIL] {
        assert!(!s.contains(['\u{2013}', '\u{2014}']), "{s}");
    }
}
