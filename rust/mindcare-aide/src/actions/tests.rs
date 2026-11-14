//! Les 4 réparations avec le FAUX PC (src\essais.rs) : rien n'est touché sur le vrai PC. Les cas difficiles à voir en
//! vrai sont ici : pas de Wi-Fi, Wi-Fi gardé par Windows, box qui ne répond pas, service qui ne s'arrête pas, renommage
//! refusé, `.old` d'avant, dossier qui est un lien, services toujours remis comme avant.

use super::*;
use crate::essais::{FauxPc, OLD, Reglage, SD, carte};
use crate::systeme::CarteWifi;
use std::cell::RefCell;

fn etat() -> EtatAide {
    EtatAide { admin: true, compte: "S-1-5-18".into(), pid: 4242 }
}

/// Fait la demande sur ce faux PC : la réponse et le journal
fn faire_sur(pc: &FauxPc, d: Demande) -> (Result<Value, String>, Vec<String>) {
    let journal = RefCell::new(Vec::new());
    let r = faire(d, pc, &etat(), &|l: &str| journal.borrow_mut().push(l.to_string()));
    (r, journal.into_inner())
}

#[test]
fn bonjour() {
    let pc = FauxPc::default();
    let (r, _) = faire_sur(&pc, Demande::Bonjour);
    assert_eq!(r.unwrap(), json!({ "Version": 1, "Admin": true, "Pid": 4242, "Compte": "S-1-5-18" }));
    assert!(pc.appels().is_empty());
}

#[test]
fn wifi_oublie_tout_sans_copie() {
    let pc = FauxPc::nouveau(Reglage {
        wifi: Ok(Some(vec![
            CarteWifi { id: "a".into(), profils: vec!["Box-1234".into(), "Café".into()] },
            CarteWifi { id: "b".into(), profils: vec!["Box-1234".into()] },
        ])),
        ..Reglage::default()
    });
    let (r, j) = faire_sur(&pc, Demande::WifiOublier);
    assert_eq!(r.unwrap(), json!({ "supprimes": 3, "restes": [], "wifi": true }));
    assert_eq!(pc.appels(), ["wifi", "oublier a Box-1234", "oublier a Café", "oublier b Box-1234"]);
    assert!(j.iter().any(|l| l == "wifi.oublier : « Café » oublié"), "{j:?}");
}

#[test]
fn wifi_les_echecs_sont_clairs() {
    // Pas de Wi-Fi (PC fixe, ou Windows Server comme la machine de GitHub)
    for w in [Ok(None), Ok(Some(Vec::new()))] {
        let pc = FauxPc::nouveau(Reglage { wifi: w, ..Reglage::default() });
        let (r, j) = faire_sur(&pc, Demande::WifiOublier);
        assert_eq!(r, Err(phrases::SANS_WIFI.into()));
        assert!(j[0].contains("pas de Wi-Fi"));
        assert_eq!(pc.appels(), ["wifi"]);
    }
    let pc = FauxPc::nouveau(Reglage { wifi: Err("WlanOpenHandle : erreur 5".into()), ..Reglage::default() });
    let (r, j) = faire_sur(&pc, Demande::WifiOublier);
    assert_eq!(r, Err(phrases::WIFI_ILLISIBLE.into()));
    assert!(j[0].contains("erreur 5"));
    // Un Wi-Fi réglé par l'entreprise : les autres sont oubliés quand même, la réponse le dit
    let pc = FauxPc::nouveau(Reglage {
        wifi: Ok(Some(vec![CarteWifi { id: "a".into(), profils: vec!["Bureau".into(), "Maison".into()] }])),
        wifi_gardes: vec!["Bureau".into()],
        ..Reglage::default()
    });
    let (r, _) = faire_sur(&pc, Demande::WifiOublier);
    assert_eq!(r, Err(phrases::WIFI_RESTES.into()));
    assert!(pc.appels().contains(&"oublier a Maison".to_string()));
}

#[test]
fn ip_seulement_les_cartes_en_automatique() {
    let mut fixe = carte(3, "Ethernet 2", false, "192.168.1.50");
    fixe.description = "Realtek".into();
    let pc = FauxPc::nouveau(Reglage {
        cartes_avant: vec![carte(7, "Ethernet", true, "192.168.1.20"), fixe.clone()],
        cartes_apres: vec![carte(7, "Ethernet", true, "192.168.1.21"), fixe],
        ip_rendre: Err("erreur 50".into()),
        ..Reglage::default()
    });
    let (r, j) = faire_sur(&pc, Demande::IpRenouveler);
    let r = r.unwrap();
    assert_eq!(r["cartes"].as_array().unwrap().len(), 1);
    assert_eq!(
        r["cartes"][0],
        json!({ "nom": "Ethernet", "avant": ["192.168.1.20"], "apres": ["192.168.1.21"], "ok": true })
    );
    // (rendre a raté : l'adresse est redemandée quand même ; la carte à adresse fixe n'est jamais touchée)
    assert_eq!(pc.appels(), ["cartes", "rendre 7", "redemander 7 (90 s au plus)", "cartes"]);
    assert!(j.iter().any(|l| l.contains("redemandée quand même")), "{j:?}");
    assert!(j.iter().any(|l| l.contains("« Ethernet 2 » laissée telle quelle")), "{j:?}");
}

#[test]
fn ip_la_box_ne_repond_pas() {
    // L'adresse de secours de Windows (169.254) : pas d'internet, la phrase dit quoi faire
    let pc = FauxPc::nouveau(Reglage {
        cartes_avant: vec![carte(7, "Wi-Fi", true, "192.168.1.20")],
        cartes_apres: vec![carte(7, "Wi-Fi", true, "169.254.3.4")],
        ..Reglage::default()
    });
    assert_eq!(faire_sur(&pc, Demande::IpRenouveler).0, Err(phrases::BOX.into()));
    // Pas de réponse à temps
    let pc = FauxPc::nouveau(Reglage {
        cartes_avant: vec![carte(7, "Wi-Fi", true, "192.168.1.20")],
        cartes_apres: vec![carte(7, "Wi-Fi", true, "192.168.1.20")],
        ip_redemander: Err("pas de réponse de la box en 90 s".into()),
        ..Reglage::default()
    });
    assert_eq!(faire_sur(&pc, Demande::IpRenouveler).0, Err(phrases::BOX.into()));
    // Aucune carte en automatique : rien n'est touché
    let pc =
        FauxPc::nouveau(Reglage { cartes_avant: vec![carte(3, "Ethernet", false, "10.0.0.2")], ..Reglage::default() });
    assert_eq!(faire_sur(&pc, Demande::IpRenouveler).0, Err(phrases::SANS_CARTE.into()));
    assert_eq!(pc.appels(), ["cartes"]);
}

#[test]
fn energie() {
    let pc = FauxPc::nouveau(Reglage {
        plans: ["a1b2", "381b4222-f694-41f0-9685-ff5bb260df2e"].map(String::from).into(),
        ..Reglage::default()
    });
    let (r, j) = faire_sur(&pc, Demande::EnergieParDefaut);
    assert_eq!(r.unwrap(), json!({ "actif_apres": "381b4222-f694-41f0-9685-ff5bb260df2e", "code": 0 }));
    assert_eq!(pc.appels(), ["powercfg"]);
    assert!(j[0].contains("a1b2 avant"));
    for p in [Ok(1), Err("powercfg non lancé".to_string())] {
        let pc = FauxPc::nouveau(Reglage { powercfg: p, ..Reglage::default() });
        assert_eq!(faire_sur(&pc, Demande::EnergieParDefaut).0, Err(phrases::ENERGIE.into()));
    }
}

#[test]
fn maj_vider_comme_il_faut() {
    let pc = FauxPc::default();
    let (r, _) = faire_sur(&pc, Demande::MajVider);
    let r = r.unwrap();
    assert_eq!(r["renomme_en"], OLD);
    assert_eq!(r["services"], json!({ "wuauserv": "en marche", "bits": "en marche" }));
    assert_eq!(
        pc.appels(),
        [
            "arrêter wuauserv".to_string(),
            "arrêter bits".into(),
            format!("renommer {SD} en {OLD}"),
            "démarrer bits".into(),
            "démarrer wuauserv".into(),
        ]
    );
}

#[test]
fn maj_vider_un_old_d_avant() {
    // Supprimé d'abord (un reste jetable)
    let mut r = Reglage::default();
    r.entrees.insert(OLD.into(), Entree::Dossier);
    let pc = FauxPc::nouveau(r);
    assert_eq!(faire_sur(&pc, Demande::MajVider).0.unwrap()["renomme_en"], OLD);
    assert_eq!(pc.appels()[0], format!("supprimer {OLD}"));
    // Il ne part pas : un nom daté
    let mut r = Reglage { suppression: Err("utilisé".into()), ..Reglage::default() };
    r.entrees.insert(OLD.into(), Entree::Dossier);
    let pc = FauxPc::nouveau(r);
    let date = format!("{OLD}-20261114-100509");
    assert_eq!(faire_sur(&pc, Demande::MajVider).0.unwrap()["renomme_en"], date.as_str());
    assert!(pc.appels().contains(&format!("renommer {SD} en {date}")));
}

#[test]
fn maj_vider_service_bloque_abandon_propre_et_services_relances() {
    let pc = FauxPc::nouveau(Reglage { bloques: vec!["bits".into()], ..Reglage::default() });
    let (r, j) = faire_sur(&pc, Demande::MajVider);
    assert_eq!(r, Err(mindcare_aide_regles::maj::SERVICE_BLOQUE.into()));
    // Jamais de renommage, jamais un deuxième essai ; wuauserv (arrêté pour rien) est relancé
    assert_eq!(pc.appels(), ["arrêter wuauserv", "arrêter bits", "démarrer bits", "démarrer wuauserv"]);
    assert!(j.iter().any(|l| l.contains("ne s'arrête pas")), "{j:?}");
    assert!(pc.r.lock().unwrap().services["wuauserv"]);
}

#[test]
fn maj_vider_renommage_refuse_trois_essais_et_services_relances() {
    let pc = FauxPc::nouveau(Reglage { renommages_refuses: u32::MAX, ..Reglage::default() });
    let (r, _) = faire_sur(&pc, Demande::MajVider);
    assert_eq!(r, Err(mindcare_aide_regles::maj::RENOMMAGE_REFUSE.into()));
    let a = pc.appels();
    assert_eq!(a.iter().filter(|x| x.starts_with("renommer")).count(), 3);
    assert_eq!(a.iter().filter(|x| x.starts_with("pause")).count(), 2);
    assert_eq!(a[a.len() - 2..], ["démarrer bits".to_string(), "démarrer wuauserv".into()]);
    // Windows a relancé un service entre l'arrêt et le renommage : le deuxième essai passe
    let pc = FauxPc::nouveau(Reglage { renommages_refuses: 1, ..Reglage::default() });
    assert_eq!(faire_sur(&pc, Demande::MajVider).0.unwrap()["renomme_en"], OLD);
    assert_eq!(pc.appels().iter().filter(|x| x.starts_with("arrêter wuauserv")).count(), 2);
}

#[test]
fn maj_vider_remet_les_services_comme_avant() {
    // Un service arrêté avant reste arrêté (la machine de GitHub : Windows Update désactivé)
    let mut r = Reglage::default();
    r.services.insert("wuauserv".into(), false);
    let pc = FauxPc::nouveau(r);
    let (rep, _) = faire_sur(&pc, Demande::MajVider);
    assert_eq!(rep.unwrap()["services"], json!({ "wuauserv": "arrêté", "bits": "en marche" }));
    assert!(!pc.appels().contains(&"démarrer wuauserv".to_string()));
    // Un service qui ne redémarre pas : la réponse le dit
    let pc = FauxPc::nouveau(Reglage { sans_relance: vec!["wuauserv".into()], ..Reglage::default() });
    assert_eq!(faire_sur(&pc, Demande::MajVider).0, Err(mindcare_aide_regles::maj::RELANCE_RATEE.into()));
}

#[test]
fn maj_vider_jamais_un_lien_ni_rien_quand_le_dossier_manque() {
    for e in [Entree::Lien, Entree::Fichier] {
        let mut r = Reglage::default();
        r.entrees.insert(SD.into(), e);
        let pc = FauxPc::nouveau(r);
        assert_eq!(faire_sur(&pc, Demande::MajVider).0, Err(mindcare_aide_regles::maj::LIEN.into()));
        assert!(pc.appels().is_empty(), "{e:?}");
    }
    let mut r = Reglage::default();
    r.entrees.clear();
    let pc = FauxPc::nouveau(r);
    let rep = faire_sur(&pc, Demande::MajVider).0.unwrap();
    assert_eq!(rep["renomme_en"], Value::Null);
    assert!(pc.appels().is_empty());
}
