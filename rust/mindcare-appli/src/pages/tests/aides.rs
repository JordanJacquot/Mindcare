//! Les pages de l'assistant et de la dictée (étape 6.4), sans rien lancer : chaque état de leurs lignes (installer,
//! en cours avec sa barre, à réparer, à mettre à jour, erreur, prêt, écoute, écrit...) au format du kit, chaque action
//! montrée acceptée, les textes du PowerShell, et les parcours (Entrée, « Pour être sûr », la phrase dictée, la page
//! quittée). Ce qui se passe vraiment (installer, poser la phrase, le micro) est jugé dans tests\aides.rs.

use serde_json::{Value, json};

use mindcare_assistant::{Choix, Decision, Etat as EtatAssistant, MESSAGE_ECHEC};
use mindcare_dictee::Etat as EtatDictee;
use mindcare_telechargement::Etat as EtatInstallation;

use super::super::aides::{Aides, POUR_ETRE_SUR, ids as ia, textes as x};
use super::{Banc, cat, toutes_acceptees, verifier_kit};
use crate::pages::{Effet, ecrans};

/// Des aides branchées : l'assistant et la dictée installés et au repos.
fn pretes() -> Aides {
    Aides {
        installation_assistant: Some(EtatInstallation::Pret),
        assistant: Some(EtatAssistant::Repos),
        installation_dictee: Some(EtatInstallation::Pret),
        dictee: Some(EtatDictee::Prete),
        ..Aides::default()
    }
}

fn banc(aides: Aides) -> Banc {
    let mut b = Banc::nouveau(None);
    b.etat.aides = aides;
    b.etat.ecran = ecrans::RACONTE.into();
    b.json = b.etat.etat_kit(cat(), None);
    b
}

fn raconte(b: &Banc) -> Value {
    b.etat.etat_kit(cat(), None)["pages"][ecrans::RACONTE].clone()
}

fn ligne(page: &Value, nom: &str) -> Value {
    page["blocs"]
        .as_array()
        .and_then(|l| l.iter().find(|x| x["type"] == "ligne" && x["texte"] == nom).cloned())
        .unwrap_or(Value::Null)
}

fn demander_montre(page: &Value) -> bool {
    page["blocs"].as_array().is_some_and(|l| l.iter().any(|x| x["id"] == ia::DEMANDER))
}

/// Le format du kit et les actions acceptées, pour cet état.
fn propre(b: &Banc) {
    let json = b.etat.etat_kit(cat(), None);
    let e = verifier_kit(&json);
    assert!(e.is_empty(), "format du kit : {e:#?}");
    let refus = toutes_acceptees(&b.etat, &json);
    assert!(refus.is_empty(), "actions refusées : {refus:#?}");
}

#[test]
fn sans_aides_la_page_de_l_etape_5() {
    let b = Banc::nouveau(None);
    let p = raconte(&b);
    assert!(p.get("actions").is_none());
    assert_eq!(ligne(&p, x::LIGNE_DICTEE), Value::Null);
    assert_eq!(ligne(&p, x::LIGNE_ASSISTANT), Value::Null);
    assert_eq!(p["sous"], "Avec tes mots : Mindcare trouve la bonne enquête.");
}

#[test]
fn chaque_etat_de_la_ligne_de_l_assistant() {
    let cas: Vec<(EtatInstallation, &str, Option<&str>, bool)> = vec![
        (
            EtatInstallation::Rien { taille_mo: 673 },
            "Mindcare peut comprendre tes phrases (« mon pc s'éteint quand je joue ») : installe son assistant, environ 673 Mo, une seule fois. Il marche ensuite sans internet.",
            Some(x::INSTALLER),
            false,
        ),
        (
            EtatInstallation::EnCours { pourcent: 42, etape: "moteur de l'assistant".into() },
            "Installation de l'assistant : 42 % (moteur de l'assistant). Tu peux continuer à te servir de Mindcare.",
            None,
            true,
        ),
        (
            EtatInstallation::AReparer { taille_mo: 32 },
            "L'installation de l'assistant n'est pas allée jusqu'au bout (Mindcare a peut-être été fermée pendant) : répare-la, environ 32 Mo, une seule fois.",
            Some(x::REPARER),
            false,
        ),
        (
            EtatInstallation::AMettreAJour { taille_mo: 610 },
            "Une nouvelle version de l'assistant comprend mieux tes phrases : mets-la à jour (environ 610 Mo, une seule fois). L'assistant actuel marche en attendant.",
            Some(x::METTRE_A_JOUR),
            false,
        ),
        (
            EtatInstallation::Erreur(
                "il n'y a pas assez de place sur le disque. Fais de la place, puis réessaie".into(),
            ),
            "L'assistant n'a pas pu s'installer : il n'y a pas assez de place sur le disque. Fais de la place, puis réessaie.",
            Some(x::REESSAYER),
            false,
        ),
        (
            EtatInstallation::Pret,
            "L'assistant est prêt : raconte ton problème avec tes mots, puis appuie sur Entrée.",
            None,
            false,
        ),
    ];
    for (inst, note, bouton, barre) in cas {
        let mut a = pretes();
        let installe = matches!(inst, EtatInstallation::Pret | EtatInstallation::AMettreAJour { .. });
        a.assistant = Some(if installe { EtatAssistant::Repos } else { EtatAssistant::PasInstalle });
        a.installation_assistant = Some(inst.clone());
        let mut b = banc(a);
        b.etat.recherche = "écran bleu".into();
        let p = raconte(&b);
        let l = ligne(&p, x::LIGNE_ASSISTANT);
        assert_eq!(l["note"], note, "{inst:?}");
        assert_eq!(l["bouton"]["texte"].as_str(), bouton, "{inst:?}");
        assert_eq!(l["bouton"]["id"].as_str(), bouton.map(|_| ia::INSTALLER_IA), "{inst:?}");
        assert_eq!(l["barre"].is_u64(), barre, "{inst:?}");
        // (« Demander à l'assistant » : seulement s'il marche, à jour ou en attendant la mise à jour)
        assert_eq!(demander_montre(&p), installe, "{inst:?}");
        assert_eq!(
            p["blocs"][0]["indice"],
            if installe {
                x::INDICE_ASSISTANT
            } else {
                "Par exemple : « bip », « le son grésille », « écran bleu »..."
            }
        );
        propre(&b);
    }
}

#[test]
fn l_assistant_en_marche() {
    for (etat, note) in [
        (EtatAssistant::Demarre, "L'assistant se prépare, quelques secondes..."),
        (EtatAssistant::Pret, "L'assistant est prêt : raconte ton problème avec tes mots, puis appuie sur Entrée."),
        (EtatAssistant::Reflechit, x::LIT),
        // (une erreur passée n'est dite que juste après la phrase)
        (
            EtatAssistant::Erreur { message: MESSAGE_ECHEC.into() },
            "L'assistant est prêt : raconte ton problème avec tes mots, puis appuie sur Entrée.",
        ),
    ] {
        let mut a = pretes();
        a.assistant = Some(etat.clone());
        let b = banc(a);
        assert_eq!(ligne(&raconte(&b), x::LIGNE_ASSISTANT)["note"], note, "{etat:?}");
    }
    let mut a = pretes();
    a.echec = true;
    assert_eq!(ligne(&raconte(&banc(a)), x::LIGNE_ASSISTANT)["note"], MESSAGE_ECHEC);
    let mut a = pretes();
    a.lit = true;
    let mut b = banc(a);
    b.etat.recherche = "mon pc s'éteint".into();
    let p = raconte(&b);
    assert_eq!(ligne(&p, x::LIGNE_ASSISTANT)["note"], x::LIT);
    assert!(!demander_montre(&p), "« Demander » pendant qu'il lit");
}

/// Un cas de la ligne de la dictée : l'installation, la dictée, son message, la note attendue, le bouton, Parler.
type CasDictee<'a> = (EtatInstallation, EtatDictee, Option<&'a str>, String, Option<&'a str>, &'a str);

#[test]
fn chaque_etat_de_la_ligne_de_la_dictee() {
    let installer = "Pour parler au lieu d'écrire, installe la dictée : environ 190 Mo, une seule fois. Elle marche ensuite sans internet, et ta voix n'est jamais gardée.";
    let cas: Vec<CasDictee> = vec![
        (EtatInstallation::Rien { taille_mo: 190 }, EtatDictee::PasInstallee, None, installer.into(), Some(x::INSTALLER), x::PARLER),
        (EtatInstallation::AReparer { taille_mo: 190 }, EtatDictee::PasInstallee, None, installer.into(), Some(x::REPARER), x::PARLER),
        (
            EtatInstallation::EnCours { pourcent: 7, etape: "français de la dictée".into() },
            EtatDictee::PasInstallee,
            None,
            "Installation de la dictée : 7 % (français de la dictée). Tu peux continuer à te servir de Mindcare.".into(),
            None,
            x::PARLER,
        ),
        (
            EtatInstallation::Erreur("le fichier « français de la dictée » arrive abîmé (deux fois de suite)".into()),
            EtatDictee::PasInstallee,
            None,
            "La dictée n'a pas pu s'installer : le fichier « français de la dictée » arrive abîmé (deux fois de suite).".into(),
            Some(x::REESSAYER),
            x::PARLER,
        ),
        (EtatInstallation::Pret, EtatDictee::Prete, None, x::DICTEE_PRETE.into(), None, x::PARLER),
        (EtatInstallation::Pret, EtatDictee::Ecoute { niveau: 0.0 }, None, x::DICTEE_ECOUTE.into(), None, x::ARRETER),
        (EtatInstallation::Pret, EtatDictee::Ecrit, None, x::DICTEE_ECRIT.into(), None, x::PARLER),
        (
            EtatInstallation::Pret,
            EtatDictee::Erreur(mindcare_dictee::messages::RIEN_ENTENDU.into()),
            None,
            mindcare_dictee::messages::RIEN_ENTENDU.into(),
            None,
            x::PARLER,
        ),
        (EtatInstallation::Pret, EtatDictee::Prete, Some(x::DICTEE_COMPRIS), x::DICTEE_COMPRIS.into(), None, x::PARLER),
    ];
    for (inst, dictee, message, note, bouton, parler) in cas {
        let mut a = pretes();
        a.installation_dictee = Some(inst.clone());
        a.dictee = Some(dictee.clone());
        a.message_dictee = message.map(str::to_string);
        let b = banc(a);
        let p = raconte(&b);
        let l = ligne(&p, x::LIGNE_DICTEE);
        assert_eq!(l["note"], note.as_str(), "{inst:?} {dictee:?}");
        assert_eq!(l["bouton"]["texte"].as_str(), bouton, "{inst:?}");
        assert_eq!(l["bouton"]["id"].as_str(), bouton.map(|_| ia::INSTALLER_DICTEE), "{inst:?}");
        assert_eq!(p["actions"], json!([{ "id": ia::PARLER, "texte": parler, "icone": "son" }]), "{dictee:?}");
        assert_eq!(p["sous"], "Avec tes mots ou avec ta voix : Mindcare trouve la bonne enquête.");
        // (la maquette : la dictée, puis l'assistant, juste sous le champ)
        assert_eq!(p["blocs"][1]["texte"], x::LIGNE_DICTEE);
        assert_eq!(p["blocs"][2]["texte"], x::LIGNE_ASSISTANT);
        propre(&b);
    }
    let accueil = banc(pretes()).etat.etat_kit(cat(), None)["accueil"]["tuiles"][0]["sous"].clone();
    assert_eq!(accueil, "Avec tes mots ou ta voix");
}

#[test]
fn ecrire_prepare_l_assistant_et_entree_pose_la_phrase() {
    let mut b = banc(pretes());
    // Une phrase nouvelle : l'assistant se prépare (Start-Ia), rien n'est posé
    let e = b.action("recherche", json!("mon pc s'éteint"), ecrans::RACONTE);
    assert_eq!(e, vec![Effet::DemarrerAssistant, Effet::Afficher { ecran: None }]);
    // Entrée : le kit renvoie la même phrase (Request-Ia)
    let e = b.action("recherche", json!("mon pc s'éteint"), ecrans::RACONTE);
    assert_eq!(e, vec![Effet::Demander { phrase: "mon pc s'éteint".into() }, Effet::Afficher { ecran: None }]);
    assert!(b.etat.aides.lit);
    // Entrée une deuxième fois pendant qu'il lit : ignorée ; « Demander à l'assistant » aussi
    assert_eq!(b.action("recherche", json!("mon pc s'éteint"), ecrans::RACONTE), vec![Effet::Afficher { ecran: None }]);
    assert_eq!(b.action(ia::DEMANDER, Value::Null, ecrans::RACONTE), vec![Effet::Afficher { ecran: None }]);
    // On écrit autre chose : la phrase posée est oubliée
    let e = b.action("recherche", json!("mon pc s'éteint tout seul"), ecrans::RACONTE);
    assert_eq!(e, vec![Effet::OublierQuestion, Effet::DemarrerAssistant, Effet::Afficher { ecran: None }]);
    assert!(!b.etat.aides.lit);
    // Une recherche vide n'est jamais posée
    b.action("recherche", json!(""), ecrans::RACONTE);
    assert_eq!(b.action("recherche", json!(""), ecrans::RACONTE), vec![Effet::Afficher { ecran: None }]);
    // Sans assistant installé : rien ne se prépare, rien n'est posé (la recherche par mots reste)
    let mut a = pretes();
    a.assistant = Some(EtatAssistant::PasInstalle);
    a.installation_assistant = Some(EtatInstallation::Rien { taille_mo: 673 });
    let mut b = banc(a);
    assert_eq!(b.action("recherche", json!("bip"), ecrans::RACONTE), vec![Effet::Afficher { ecran: None }]);
    assert_eq!(b.action("recherche", json!("bip"), ecrans::RACONTE), vec![Effet::Afficher { ecran: None }]);
    b.propre();
}

#[test]
fn la_reponse_de_l_assistant() {
    // Sûr : l'enquête commence tout de suite, avec les réponses déjà données
    let mut b = banc(pretes());
    b.action("recherche", json!("mon pc portable s'éteint"), ecrans::RACONTE);
    b.action(ia::DEMANDER, Value::Null, ecrans::RACONTE);
    let d = Decision::Enquete { panne: "seteint".into(), reponses: vec![("type".into(), 0)] };
    let e = b.etat.decision_assistant(cat(), d, &mut || "20261103-101500-0001".into());
    b.executer(e);
    assert_eq!(b.ecran.as_deref(), Some(ecrans::ENQUETE));
    assert_eq!(b.etat.en_cours.pre_rempli.as_ref().map(|r| r.0.clone()), Some(vec![("type".to_string(), 0)]));
    assert!(b.journal.iter().any(|l| l == "Enquête : seteint (déjà su : type)"), "{:?}", b.journal);
    // Rien de lisible : le message, la recherche par mots reste
    let mut b = banc(pretes());
    b.action("recherche", json!("mon pc"), ecrans::RACONTE);
    b.action(ia::DEMANDER, Value::Null, ecrans::RACONTE);
    let e = b.etat.decision_assistant(cat(), Decision::Echec { cause: "x".into() }, &mut || "id".into());
    assert_eq!(e, vec![Effet::Afficher { ecran: None }]);
    assert_eq!(ligne(&raconte(&b), x::LIGNE_ASSISTANT)["note"], MESSAGE_ECHEC);
    // ... jusqu'à ce qu'on écrive autre chose
    b.action("recherche", json!("mon pc chauffe"), ecrans::RACONTE);
    assert!(!b.etat.aides.echec);
    // Une phrase oubliée (on a quitté la page) : sa réponse ne fait rien
    let mut b = banc(pretes());
    b.action("recherche", json!("mon pc"), ecrans::RACONTE);
    b.action(ia::DEMANDER, Value::Null, ecrans::RACONTE);
    let e = b.message(json!({ "type": "ecran", "id": ecrans::FAMILLES }));
    assert!(e.contains(&Effet::OublierQuestion), "{e:?}");
    let d = Decision::Enquete { panne: "seteint".into(), reponses: Vec::new() };
    assert!(b.etat.decision_assistant(cat(), d, &mut || "id".into()).is_empty());
    assert!(b.etat.en_cours.panne.is_none());
    // Une autre action pendant qu'il lit : oubliée aussi
    let mut b = banc(pretes());
    b.action("recherche", json!("mon pc"), ecrans::RACONTE);
    b.action(ia::DEMANDER, Value::Null, ecrans::RACONTE);
    let e = b.action(ecrans::FAMILLES, Value::Null, ecrans::RACONTE);
    assert_eq!(e.first(), Some(&Effet::OublierQuestion));
    b.propre();
}

fn choix(id: &str) -> Choix {
    let p = cat().panne(id).expect(id);
    let t = |c: &str| mindcare_moteur::ps::texte(mindcare_moteur::ps::champ(p, c));
    Choix { panne: id.into(), titre: t("Titre"), phrase: t("Phrase"), reponses: vec![("maison".into(), 0)] }
}

#[test]
fn pour_etre_sur() {
    let mut b = banc(pretes());
    b.action("recherche", json!("mon pc s'éteint d'un coup"), ecrans::RACONTE);
    b.action(ia::DEMANDER, Value::Null, ecrans::RACONTE);
    let d = Decision::PourEtreSur {
        phrase: "mon pc s'éteint d'un coup".into(),
        choix: vec![choix("seteint"), choix("redemarre"), choix("ecranbleu")],
    };
    let e = b.etat.decision_assistant(cat(), d, &mut || "id".into());
    assert_eq!(e, vec![Effet::Afficher { ecran: Some(POUR_ETRE_SUR.into()) }]);
    b.executer(e);
    let p = b.json["pages"][POUR_ETRE_SUR].clone();
    assert_eq!(p["titre"], "Pour être sûr");
    assert_eq!(p["parent"], ecrans::RACONTE);
    assert_eq!(
        p["module"],
        json!({ "icone": "aide", "couleur": "T", "titre": "Ce que tu as écrit", "texte": "« mon pc s'éteint d'un coup »" })
    );
    assert_eq!(
        p["blocs"][0],
        json!({ "type": "titre", "texte": "Laquelle de ces phrases ressemble le plus à ce qui se passe ?" })
    );
    let options = p["blocs"][1]["options"].as_array().cloned().unwrap_or_default();
    assert_eq!(options.len(), 3);
    for (o, c) in options.iter().zip(["seteint", "redemarre", "ecranbleu"]) {
        let ch = choix(c);
        assert_eq!(o["titre"], ch.titre.as_str());
        assert_eq!(o["texte"], ch.phrase.as_str());
    }
    assert_eq!(
        p["blocs"][2],
        json!({ "type": "bouton", "id": "aucune", "texte": "Aucune de ces phrases", "style": "second" })
    );
    b.propre();
    propre(&b);
    // Une réponse : son enquête, avec ses réponses déjà données
    let mut c = b.etat.clone();
    let e =
        c.agir(cat(), &json!({ "type": "action", "id": ia::IDEE, "valeur": "1", "ecran": POUR_ETRE_SUR }), &mut || {
            "id".into()
        });
    assert!(e.contains(&Effet::Journal("Pour être sûr : redemarre".into())), "{e:?}");
    assert!(e.contains(&Effet::Afficher { ecran: Some(ecrans::ENQUETE.into()) }), "{e:?}");
    assert_eq!(c.en_cours.pre_rempli.as_ref().map(|r| r.0.clone()), Some(vec![("maison".to_string(), 0)]));
    assert!(c.aides.pour_etre_sur.is_none());
    // « Aucune de ces phrases » : les questions guidées
    let e = b.action(ia::AUCUNE, Value::Null, POUR_ETRE_SUR);
    assert_eq!(
        e,
        vec![
            Effet::Journal("Pour être sûr : aucune de ces phrases".into()),
            Effet::Afficher { ecran: Some(ecrans::FAMILLES.into()) }
        ]
    );
    assert!(b.etat.en_cours.panne.is_none());
    assert!(b.json["pages"].get(POUR_ETRE_SUR).is_none());
}

#[test]
fn la_dictee_dans_la_page() {
    // Parler, installer : des effets pour effets.rs
    let mut b = banc(pretes());
    assert_eq!(
        b.action(ia::PARLER, Value::Null, ecrans::RACONTE),
        vec![Effet::Parler, Effet::Afficher { ecran: None }]
    );
    assert_eq!(
        b.action(ia::INSTALLER_DICTEE, Value::Null, ecrans::RACONTE),
        vec![Effet::InstallerDictee, Effet::Afficher { ecran: None }]
    );
    assert_eq!(
        b.action(ia::INSTALLER_IA, Value::Null, ecrans::RACONTE),
        vec![Effet::InstallerAssistant, Effet::Afficher { ecran: None }]
    );
    // La phrase dictée : dans la recherche, à corriger ; l'assistant se prépare
    let e = b.etat.phrase_dictee("le wifi coupe".into());
    assert_eq!(e, vec![Effet::DemarrerAssistant, Effet::Afficher { ecran: None }]);
    assert_eq!(b.etat.recherche, "le wifi coupe");
    let p = raconte(&b);
    assert_eq!(p["blocs"][0]["valeur"], "le wifi coupe");
    assert_eq!(ligne(&p, x::LIGNE_DICTEE)["note"], x::DICTEE_COMPRIS);
    // Entrée : la phrase part, le message est lu
    let e = b.action("recherche", json!("le wifi coupe"), ecrans::RACONTE);
    assert_eq!(
        e,
        vec![
            Effet::EffacerMessageDictee,
            Effet::Demander { phrase: "le wifi coupe".into() },
            Effet::Afficher { ecran: None }
        ]
    );
    // La page quittée pendant l'écoute : la dictée s'arrête sans suite, et la phrase n'arrive pas
    let mut a = pretes();
    a.dictee = Some(EtatDictee::Ecoute { niveau: 0.0 });
    let mut b = banc(a);
    let e = b.message(json!({ "type": "ecran", "id": ecrans::ACCUEIL }));
    assert_eq!(e, vec![Effet::AnnulerDictee, Effet::Afficher { ecran: None }]);
    assert!(b.etat.phrase_dictee("perdue".into()).is_empty());
    assert!(b.etat.recherche.is_empty());
    // Rien en cours : changer d'écran ne fait rien
    let mut b = banc(pretes());
    assert!(b.message(json!({ "type": "ecran", "id": ecrans::FAMILLES })).is_empty());
    assert!(b.message(json!({ "type": "ecran", "id": ecrans::RACONTE })).is_empty());
}

#[test]
fn le_verificateur_voit_une_barre_impossible() {
    let mut b = banc(pretes());
    b.etat.aides.installation_assistant = Some(EtatInstallation::EnCours { pourcent: 42, etape: "x".into() });
    let mut json = b.etat.etat_kit(cat(), None);
    assert!(verifier_kit(&json).is_empty());
    let blocs = json["pages"][ecrans::RACONTE]["blocs"].as_array_mut().unwrap();
    let l = blocs.iter_mut().find(|v| v["texte"] == x::LIGNE_ASSISTANT).unwrap();
    l["barre"] = json!(150);
    assert!(!verifier_kit(&json).is_empty(), "une barre à 150 % passe");
    json["pages"][ecrans::RACONTE]["actions"][0]["icone"] = json!("micro-inconnu");
    assert!(verifier_kit(&json).iter().any(|e| e.contains("micro-inconnu")));
    b.propre();
}
