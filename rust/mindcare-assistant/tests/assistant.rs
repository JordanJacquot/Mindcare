//! Les juges de l'assistant : les cas de outils\tester-assistant.ps1 (Bug 4, Bug 6, constats 25 et 27) rejoués
//! contre un faux llama-server en Rust (dans le test lui-même), et ceux de l'étape 6.2 : port pris, serveur qui ne
//! démarre pas, qui répond mal, qui n'est pas le nôtre, lent, arrêt après 5 minutes (horloge simulée), arrêt à la
//! fermeture. Puis la décision (contenu.ps1) avec le vrai catalogue.

mod commun;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use commun::{Banc, Comportement, Journal, Scenario, attendre, comportement_bruit, delais_courts, repond};
use mindcare_assistant::requete::lire_texte;
use mindcare_assistant::{Decision, Etat, Horloge, MESSAGE_ECHEC, Question, Reponse, SYSTEME, choisir, decider};
use mindcare_moteur::{Catalogue, pre_rempli};
use serde_json::{Value, json};

const MINUTE: Duration = Duration::from_secs(60);

fn permises() -> Vec<String> {
    vec!["bruit".to_string()]
}

/// Pose la phrase et attend la réponse
fn poser(b: &Banc, phrase: &str) -> Reponse {
    match b.assistant.poser(phrase, &permises()) {
        Question::EnCours(t) => t.attendre(),
        Question::SansAssistant => panic!("pas d'assistant"),
        Question::NeDemarrePas { cause } => panic!("il ne démarre pas : {cause}"),
    }
}

fn catalogue() -> Catalogue {
    let chemin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Mindcare/fichiers/enquetes.json");
    Catalogue::lire_fichier(&chemin).expect("enquetes.json")
}

// -----------------------------------------------------------------------------------------------------------------
// Bug 6 : pas chargé à l'ouverture, seulement quand on s'en sert, et arrêté après quelques minutes sans question
// -----------------------------------------------------------------------------------------------------------------

#[test]
fn pas_lance_a_l_ouverture_de_mindcare() {
    let b = Banc::nouveau("ouverture", Scenario::Normal(comportement_bruit()), delais_courts());
    assert_eq!(b.assistant.etat(), Etat::Repos);
    assert!(!b.assistant.lance());
    assert_eq!(b.lanceur.nb_lancements(), 0);
}

#[test]
fn pas_installe_la_recherche_par_mots_reste() {
    let b = Banc::nouveau("pas-installe", Scenario::Normal(comportement_bruit()), delais_courts());
    std::fs::remove_file(b.essai.0.join("ia").join("ggml.dll")).unwrap();
    assert_eq!(b.assistant.etat(), Etat::PasInstalle);
    assert!(matches!(b.assistant.poser("mon pc fait du bruit", &permises()), Question::SansAssistant));
    assert_eq!(b.lanceur.nb_lancements(), 0);
}

#[test]
fn lance_a_la_premiere_question_garde_puis_arrete_apres_5_minutes_et_relance() {
    let b = Banc::nouveau("repos", Scenario::Normal(comportement_bruit()), delais_courts());
    // lancé à la première question s'il ne tourne pas
    let r = poser(&b, "mon pc fait du bruit");
    assert_eq!((r.panne.as_str(), r.erreur.as_str()), ("bruit", ""));
    assert!(b.assistant.lance());
    assert_eq!(b.assistant.etat(), Etat::Pret);
    let (port, _) = b.assistant.port_et_cle();
    // gardé tant qu'il sert (1 minute après), arrêté après 5 minutes sans question
    let t0 = b.horloge.maintenant();
    assert!(!b.assistant.arreter_si_inutile(t0 + MINUTE));
    assert!(b.assistant.lance());
    assert!(!b.assistant.arreter_si_inutile(t0 + 4 * MINUTE + Duration::from_secs(59)));
    assert!(b.assistant.arreter_si_inutile(t0 + 5 * MINUTE + Duration::from_secs(1)));
    assert!(!b.assistant.lance());
    assert!(b.lanceur.lances.lock().unwrap()[0].1.load(Ordering::SeqCst), "le processus n'a pas été arrêté");
    assert!(!repond(port), "le port répond encore");
    assert_eq!(b.journal.compte("Assistant arrêté : pas de question depuis 5 minutes (relancé à la prochaine)"), 1);
    assert_eq!(b.assistant.etat(), Etat::Repos);
    // relancé tout seul à la question suivante, avec une clé neuve
    let cle1 = b.lanceur.ordres.lock().unwrap()[0].variable("LLAMA_API_KEY").unwrap().to_string();
    let r = poser(&b, "mon pc fait du bruit");
    assert_eq!(r.panne, "bruit");
    assert_eq!(b.lanceur.nb_lancements(), 2);
    assert_ne!(b.lanceur.dernier_ordre().variable("LLAMA_API_KEY").unwrap(), cle1);
}

#[test]
fn la_minuterie_l_arrete_toute_seule_apres_5_minutes() {
    let mut d = delais_courts();
    d.veille = Duration::from_millis(50);
    let b = Banc::nouveau("minuterie", Scenario::Normal(comportement_bruit()), d);
    b.assistant.demarrer();
    assert!(attendre(Duration::from_secs(5), || b.assistant.etat() == Etat::Pret));
    // (la vraie heure passe, pas l'horloge simulée : il reste)
    std::thread::sleep(Duration::from_millis(300));
    assert!(b.assistant.lance());
    b.horloge.avancer(5 * MINUTE + Duration::from_secs(1));
    assert!(attendre(Duration::from_secs(5), || !b.assistant.lance()), "pas arrêté par la minuterie");
}

#[test]
fn une_question_en_cours_compte_comme_on_s_en_sert() {
    let c = Comportement { lenteur: Duration::from_millis(800), ..comportement_bruit() };
    let b = Banc::nouveau("en-cours", Scenario::Normal(c), delais_courts());
    let Question::EnCours(t) = b.assistant.poser("mon pc fait du bruit", &permises()) else { panic!() };
    assert!(attendre(Duration::from_secs(5), || b.assistant.etat() == Etat::Reflechit));
    // une question qui dure n'est pas coupée par la minuterie
    assert!(!b.assistant.arreter_si_inutile(b.horloge.maintenant() + 10 * MINUTE));
    assert_eq!(t.attendre().panne, "bruit");
}

#[test]
fn arrete_a_la_fermeture_de_mindcare() {
    let b = Banc::nouveau("fermeture", Scenario::Normal(comportement_bruit()), delais_courts());
    b.assistant.demarrer();
    assert!(attendre(Duration::from_secs(5), || b.assistant.etat() == Etat::Pret));
    let (port, _) = b.assistant.port_et_cle();
    assert!(repond(port));
    let Banc { assistant, lanceur, .. } = b;
    drop(assistant);
    assert!(lanceur.lances.lock().unwrap()[0].1.load(Ordering::SeqCst), "pas arrêté à la fermeture");
    assert!(!repond(port));
}

// -----------------------------------------------------------------------------------------------------------------
// Constat 27 et constat 25m : la clé, le port, 127.0.0.1, c'est bien LUI qui répond
// -----------------------------------------------------------------------------------------------------------------

#[test]
fn la_question_est_envoyee_avec_la_cle_du_lancement_sur_127_0_0_1() {
    let b = Banc::nouveau("cle", Scenario::Normal(comportement_bruit()), delais_courts());
    let r = poser(&b, "mon pc fait du bruit");
    assert_eq!(r.panne, "bruit");
    let ordre = b.lanceur.dernier_ordre();
    let cle = ordre.variable("LLAMA_API_KEY").unwrap().to_string();
    assert!(cle.len() >= 32);
    // la clé par une variable d'environnement, jamais dans les arguments
    assert!(ordre.arguments.iter().all(|a| !a.contains(&cle)));
    assert!(!ordre.arguments.iter().any(|a| a.contains("api-key")));
    let attendus = [
        "-m",
        &b.essai.0.join("ia").join("mindcare-0.6b-v3-q8_0.gguf").to_string_lossy(),
        "--host",
        "127.0.0.1",
        "--port",
        &ordre.port.to_string(),
        "-ngl",
        "99",
        "-c",
        "2048",
        "-np",
        "1",
        "--no-webui",
    ]
    .map(|s| s.to_string());
    assert_eq!(ordre.arguments, attendus);
    assert_eq!(ordre.variable("LLAMA_ARG_CHAT_TEMPLATE_KWARGS"), Some(r#"{"enable_thinking":false}"#));
    // lancé depuis sa copie Mindset.exe (une seule ligne « Mindset » dans le Gestionnaire des tâches)
    assert_eq!(ordre.programme.file_name().unwrap(), "Mindset.exe");
    assert!(b.journal.texte().contains("Assistant lancé (port"));
}

#[test]
fn la_requete_est_celle_du_powershell() {
    let b = Banc::nouveau("requete", Scenario::Normal(comportement_bruit()), delais_courts());
    let permises = vec!["bruit".to_string(), "seteint".to_string()];
    let Question::EnCours(t) = b.assistant.poser("mon pc fait du bruit quand je joue, ça grésille", &permises) else {
        panic!()
    };
    assert_eq!(t.attendre().panne, "bruit");
    let cle = b.lanceur.dernier_ordre().variable("LLAMA_API_KEY").unwrap().to_string();
    let q = b.lanceur.questions();
    assert_eq!(q.len(), 1);
    assert_eq!((q[0].methode.as_str(), q[0].autorisation.clone()), ("POST", Some(format!("Bearer {cle}"))));
    let corps: Value = serde_json::from_str(&q[0].corps).unwrap();
    assert_eq!(corps["messages"][0], json!({ "role": "system", "content": SYSTEME }));
    assert_eq!(
        corps["messages"][1],
        json!({ "role": "user", "content": "mon pc fait du bruit quand je joue, ça grésille" })
    );
    assert_eq!((corps["temperature"].clone(), corps["max_tokens"].clone()), (json!(0), json!(120)));
    assert_eq!(corps["logprobs"], true);
    assert_eq!(corps["response_format"]["type"], "json_schema");
    assert_eq!(corps["response_format"]["json_schema"]["name"], "r");
    let schema = &corps["response_format"]["json_schema"]["schema"];
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["panne"], json!({ "type": "string", "enum": ["bruit", "seteint"] }));
    assert_eq!(schema["required"], json!(["panne", "reponses"]));
    assert_eq!(
        schema["properties"]["reponses"],
        json!({ "type": "object", "additionalProperties": { "type": "integer" } })
    );
    // (avant la phrase : /health, puis /props sans la clé et avec : c'est bien lui)
    let tout = b.lanceur.recus.lock().unwrap()[0].lock().unwrap().clone();
    let props: Vec<Option<String>> =
        tout.iter().filter(|r| r.chemin == "/props").map(|r| r.autorisation.clone()).collect();
    assert_eq!(props, vec![None, Some(format!("Bearer {cle}"))]);
    assert_eq!(tout.last().unwrap().chemin, "/v1/chat/completions");
}

#[test]
fn port_pris_par_un_autre_programme_rien_ne_lui_est_envoye() {
    let etranger = Comportement { cle: None, ..comportement_bruit() };
    let b = Banc::nouveau("port-pris", Scenario::PortVole(etranger), delais_courts());
    let q = b.assistant.poser("mon pc fait du bruit", &permises());
    let r = match q {
        Question::EnCours(t) => t.attendre(),
        Question::NeDemarrePas { cause } => Reponse::vide(cause),
        Question::SansAssistant => panic!(),
    };
    assert!(!r.erreur.is_empty(), "une réponse venue d'un autre programme : {r:?}");
    assert_eq!(b.lanceur.phrases_aux_etrangers(), 0, "la phrase est partie chez un autre programme");
    // la vraie cause au journal : notre llama-server arrêté (son code, sa ligne), ou l'autre programme reconnu
    let j = b.journal.texte();
    assert!(
        j.contains("Assistant arrêté tout seul (code 1) ; ses dernières lignes : couldn't bind")
            || j.contains("ce n'est pas le nôtre"),
        "{j}"
    );
    assert!(matches!(b.assistant.etat(), Etat::Erreur { .. }));
}

#[test]
fn un_autre_llama_server_qui_n_est_pas_le_notre() {
    for (nom, cle) in
        [("autre-cle", Some("une-autre-cle-qui-n-est-pas-la-notre-0123456789".to_string())), ("sans-cle", None)]
    {
        let etranger = Comportement { cle, ..comportement_bruit() };
        let b = Banc::nouveau(nom, Scenario::Etranger(etranger), delais_courts());
        let r = poser(&b, "mon pc fait du bruit");
        assert!(r.erreur.contains("ce n'est pas le nôtre"), "{nom} : {r:?}");
        assert_eq!(b.lanceur.phrases_aux_etrangers(), 0, "{nom} : la phrase est partie chez un autre");
        assert!(b.journal.texte().contains("ce n'est pas le nôtre (sans la clé"), "{nom}");
        // notre llama-server (muet) a été arrêté
        assert!(b.lanceur.lances.lock().unwrap()[0].1.load(Ordering::SeqCst), "{nom}");
        assert!(matches!(b.assistant.etat(), Etat::Erreur { .. }));
        let d = decider(&catalogue(), "mon pc fait du bruit", &r, &|_| {});
        assert!(matches!(d, Decision::Echec { .. }));
    }
}

// -----------------------------------------------------------------------------------------------------------------
// Bug 4 : quand l'assistant échoue, l'utilisateur le voit, et le journal dit pourquoi
// -----------------------------------------------------------------------------------------------------------------

#[test]
fn la_vraie_cause_d_un_echec_est_gardee_llama_server_injoignable() {
    let c = Comportement { fermer_apres_verification: true, ..comportement_bruit() };
    let b = Banc::nouveau("injoignable", Scenario::Normal(c), delais_courts());
    let r = poser(&b, "mon pc fait du bruit");
    assert!(r.erreur.starts_with("pas de réponse : connexion à l'assistant impossible ("), "{}", r.erreur);
    let journal = Journal::default();
    let d = decider(&catalogue(), "mon pc fait du bruit", &r, journal.fonction().as_ref());
    assert!(matches!(d, Decision::Echec { ref cause } if cause.contains("connexion")));
    assert!(journal.texte().starts_with("Assistant : pas de réponse : connexion"));
}

#[test]
fn reponse_illisible_l_utilisateur_voit_un_message() {
    let c = Comportement { contenu: "pas du json".to_string(), ..Comportement::default() };
    let b = Banc::nouveau("illisible", Scenario::Normal(c), delais_courts());
    let r = poser(&b, "mon pc s'éteint quand je joue");
    assert_eq!(r.erreur, "réponse illisible (pas du json)");
    let d = decider(&catalogue(), "mon pc s'éteint quand je joue", &r, b.journal.fonction().as_ref());
    assert_eq!(d, Decision::Echec { cause: "réponse illisible (pas du json)".to_string() });
    assert!(b.journal.texte().contains("Assistant : réponse illisible (pas du json)"));
    assert!(MESSAGE_ECHEC.contains("n'a pas réussi à lire ta phrase"));
}

#[test]
fn installe_mais_il_ne_demarre_pas_l_utilisateur_voit_un_message() {
    let b = Banc::nouveau("casse", Scenario::PasUnProgramme, delais_courts());
    let q = b.assistant.poser("mon pc s'éteint quand je joue", &permises());
    assert!(matches!(q, Question::NeDemarrePas { ref cause } if cause.contains("lancement impossible")));
    let j = b.journal.texte();
    assert!(j.contains("Assistant : lancement impossible (%1 n’est pas une application Win32 valide"), "{j}");
    assert!(j.contains("Assistant : pas de réponse : il ne démarre pas"));
    assert_eq!(b.assistant.etat(), Etat::Erreur { message: MESSAGE_ECHEC.to_string() });
}

#[test]
fn il_repond_mal() {
    let cas: [(&str, Comportement, &str); 5] = [
        (
            "500",
            Comportement { brut: Some((500, "{\"error\":\"boum\"}".into())), ..Comportement::default() },
            "pas de réponse : erreur 500 de l'assistant ({\"error\":\"boum\"})",
        ),
        (
            "pas-json",
            Comportement { brut: Some((200, "<html>".into())), ..Comportement::default() },
            "réponse illisible (<html>)",
        ),
        (
            "sans-panne",
            Comportement { contenu: r#"{"reponses":{}}"#.into(), ..Comportement::default() },
            "réponse illisible ({\"reponses\":{}})",
        ),
        ("vide", Comportement { contenu: String::new(), ..Comportement::default() }, "pas de réponse : réponse vide"),
        (
            "reponse-pas-entiere",
            Comportement { contenu: r#"{"panne":"bruit","reponses":{"a":"x"}}"#.into(), ..Comportement::default() },
            "réponse illisible ({\"panne\":\"bruit\",\"reponses\":{\"a\":\"x\"}})",
        ),
    ];
    for (nom, c, attendu) in cas {
        let b = Banc::nouveau(nom, Scenario::Normal(c), delais_courts());
        let r = poser(&b, "mon pc fait du bruit");
        assert_eq!(r.erreur, attendu, "{nom}");
        assert!(matches!(decider(&catalogue(), "x", &r, &|_| {}), Decision::Echec { .. }), "{nom}");
    }
}

#[test]
fn reponse_en_morceaux_et_modele_qui_charge() {
    let c = Comportement { morceaux: true, chargement: Duration::from_millis(400), ..comportement_bruit() };
    let b = Banc::nouveau("charge", Scenario::Normal(c), delais_courts());
    b.assistant.demarrer();
    assert!(attendre(Duration::from_secs(2), || b.assistant.etat() == Etat::Demarre));
    let r = poser(&b, "mon pc fait du bruit");
    assert_eq!((r.panne.as_str(), r.erreur.as_str()), ("bruit", ""));
}

#[test]
fn il_ne_demarre_pas_a_temps() {
    let b = Banc::nouveau("muet", Scenario::Muet, delais_courts());
    let debut = Instant::now();
    let r = poser(&b, "mon pc fait du bruit");
    assert!(r.erreur.starts_with("pas de réponse : l'assistant n'était pas prêt après"), "{}", r.erreur);
    // l'attente juste après le lancement (6 s ici, 100 s pour de vrai), moins le temps gardé pour la réponse (2 s)
    assert!(debut.elapsed() >= Duration::from_secs(3) && debut.elapsed() < Duration::from_secs(6));
    assert_eq!(b.assistant.etat(), Etat::Demarre);
}

#[test]
fn il_est_lent() {
    // la requête a 3 s : au-delà, la vraie cause est le délai
    let c = Comportement { lenteur: Duration::from_secs(5), ..comportement_bruit() };
    let b = Banc::nouveau("lent", Scenario::Normal(c.clone()), delais_courts());
    let r = poser(&b, "mon pc fait du bruit");
    assert_eq!(r.erreur, "pas de réponse : l'assistant n'a pas répondu en 3 s");
    // la page n'attend pas plus que l'attente (5 s une fois lancé depuis longtemps) : « trop long »
    let mut d = delais_courts();
    d.requete = Duration::from_secs(30);
    let c = Comportement { lenteur: Duration::from_secs(9), ..c };
    let b = Banc::nouveau("trop-long", Scenario::Normal(c), d);
    b.assistant.demarrer();
    assert!(attendre(Duration::from_secs(5), || b.assistant.etat() == Etat::Pret));
    b.horloge.avancer(2 * MINUTE);
    let debut = Instant::now();
    let r = poser(&b, "mon pc fait du bruit");
    assert_eq!(r.erreur, "trop long (5 s)");
    assert!(debut.elapsed() < Duration::from_secs(6));
}

// -----------------------------------------------------------------------------------------------------------------
// Constat 25 : un assistant qui plante n'est pas relancé à chaque lettre, et on sait pourquoi
// -----------------------------------------------------------------------------------------------------------------

#[test]
fn assistant_qui_plante() {
    let b = Banc::nouveau("plante", Scenario::Plante, delais_courts());
    for _ in 0..6 {
        b.assistant.demarrer(); // (six lettres tapées)
    }
    let j = b.journal.texte();
    // pas relancé à chaque lettre (une fois par minute au plus)
    assert_eq!(b.journal.compte("Assistant lancé"), 1, "{j}");
    // son code et ses dernières lignes au journal, une seule fois
    assert_eq!(b.journal.compte("Assistant arrêté tout seul (code 3)"), 1, "{j}");
    assert!(j.contains("ligne 23") && !j.contains("ligne 4 |") && j.contains("ligne 6 |"), "{j}");
    // sa clé par une variable d'environnement, pas dans ses arguments
    let cle = b.lanceur.dernier_ordre().variable("LLAMA_API_KEY").unwrap().to_string();
    assert!(j.contains(&format!("cle={cle}")));
    let args = j.split("arguments=").nth(1).unwrap().lines().next().unwrap();
    assert!(args.starts_with("-m ") && !args.contains(&cle) && !args.contains("api-key"), "{args}");
    let empreintes = b.assistant.empreintes_calculees();
    assert!(empreintes >= 1);
    // il ne démarre pas : la page le dit
    assert!(matches!(b.assistant.poser("x", &permises()), Question::NeDemarrePas { .. }));
    // relancé une minute plus tard, sans nouvelle empreinte, avec une clé neuve
    b.horloge.avancer(Duration::from_secs(61));
    b.assistant.demarrer();
    assert_eq!(b.journal.compte("Assistant lancé"), 2);
    assert_eq!(b.assistant.empreintes_calculees(), empreintes);
    assert_ne!(b.lanceur.dernier_ordre().variable("LLAMA_API_KEY").unwrap(), cle);
}

/// Audit Rust (petit de Mindcare) : llama-server qui ne peut même pas être lancé (bloqué par l'antivirus, fichier
/// abîmé) n'est pas relancé à chaque pause de frappe, mais une fois par minute au plus ; une installation neuve
/// (l'assistant arrêté par Mindcare) le relance tout de suite.
#[test]
fn lancement_impossible_pas_relance_a_chaque_lettre() {
    let b = Banc::nouveau("bloque", Scenario::PasUnProgramme, delais_courts());
    for _ in 0..6 {
        b.assistant.demarrer(); // (six pauses de frappe)
    }
    let j = b.journal.texte();
    assert_eq!(b.lanceur.nb_lancements(), 1, "{j}");
    assert_eq!(b.journal.compte("Assistant : lancement impossible"), 1, "{j}");
    // il ne démarre pas : la page le dit, avec la vraie cause, sans nouvel essai
    let q = b.assistant.poser("mon pc fait du bruit", &permises());
    assert!(matches!(q, Question::NeDemarrePas { ref cause } if cause.contains("lancement impossible")));
    assert_eq!(b.lanceur.nb_lancements(), 1);
    assert_eq!(b.assistant.etat(), Etat::Erreur { message: MESSAGE_ECHEC.to_string() });
    // 59 secondes plus tard : toujours rien
    b.horloge.avancer(Duration::from_secs(59));
    b.assistant.demarrer();
    assert_eq!(b.lanceur.nb_lancements(), 1);
    // une minute plus tard : un nouvel essai, qui marche (l'antivirus l'a laissé passer)
    *b.lanceur.scenario.lock().unwrap() = Scenario::Normal(comportement_bruit());
    b.horloge.avancer(Duration::from_secs(2));
    b.assistant.demarrer();
    assert_eq!(b.lanceur.nb_lancements(), 2);
    assert!(b.assistant.lance());
    // de nouveau bloqué après un arrêt par Mindcare (une installation neuve) : essayé tout de suite, puis attente
    b.assistant.arreter_serveur();
    *b.lanceur.scenario.lock().unwrap() = Scenario::PasUnProgramme;
    b.assistant.demarrer();
    assert_eq!(b.lanceur.nb_lancements(), 3);
    b.assistant.demarrer();
    assert_eq!(b.lanceur.nb_lancements(), 3);
    b.assistant.arreter_serveur();
    b.assistant.demarrer();
    assert_eq!(b.lanceur.nb_lancements(), 4);
}

// -----------------------------------------------------------------------------------------------------------------
// La décision (Request-Ia, Show-PourEtreSur) avec le vrai catalogue
// -----------------------------------------------------------------------------------------------------------------

fn reponse(panne: &str, reponses: &[(&str, i64)], confiance: f64) -> Reponse {
    Reponse {
        panne: panne.to_string(),
        reponses: reponses.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        confiance,
        autres: Vec::new(),
        erreur: String::new(),
    }
}

#[test]
fn sur_l_enquete_commence_avec_les_reponses_que_la_phrase_donne_vraiment() {
    let cat = catalogue();
    let phrase = "mon pc portable s'éteint d'un coup, et les lumières de la maison vacillent";
    let r = reponse("seteint", &[("type", 0), ("maison", 0), ("surbatterie", 1)], 0.97);
    let journal = Journal::default();
    let d = decider(&cat, phrase, &r, journal.fonction().as_ref());
    let attendu = pre_rempli(&cat, cat.panne("seteint"), &r.reponses, phrase);
    assert_eq!(d, Decision::Enquete { panne: "seteint".into(), reponses: attendu.clone() });
    // (le garde-fou : « sur batterie » n'est pas dans la phrase, il sera demandé)
    assert_eq!(attendu, vec![("type".to_string(), 0), ("maison".to_string(), 0)]);
    assert_eq!(
        journal.texte(),
        format!(
            "Assistant : ({} caractères) -> seteint type,maison (proposé : type,maison,surbatterie) (sûr à 97 %)",
            phrase.encode_utf16().count()
        )
    );
}

#[test]
fn il_hesite_la_question_pour_etre_sur_sur_ce_qu_on_voit() {
    let cat = catalogue();
    let phrase = "mon pc s'éteint d'un coup";
    let r = reponse("seteint", &[("maison", 0)], 0.6);
    let journal = Journal::default();
    let Decision::PourEtreSur { phrase: p, choix } = decider(&cat, phrase, &r, journal.fonction().as_ref()) else {
        panic!("pas de « Pour être sûr »")
    };
    assert_eq!(p, phrase);
    assert!((2..=3).contains(&choix.len()), "{choix:?}");
    assert_eq!(choix[0].panne, "seteint");
    // ses autres idées : les premières de la recherche par mots, chacune une fois
    let mots: Vec<String> = mindcare_moteur::chercher(&cat, phrase)
        .into_iter()
        .take(3)
        .map(|p| p["Id"].as_str().unwrap().to_string())
        .filter(|id| id != "seteint")
        .collect();
    let autres: Vec<&str> = choix[1..].iter().map(|c| c.panne.as_str()).collect();
    assert_eq!(autres, mots.iter().map(String::as_str).take(choix.len() - 1).collect::<Vec<_>>());
    // ce qu'on voit : le titre et la phrase de chaque panne, jamais un nom de panne à deviner
    for c in &choix {
        let pa = cat.panne(&c.panne).unwrap();
        assert_eq!(c.titre, pa["Titre"].as_str().unwrap());
        assert_eq!(c.phrase, pa["Phrase"].as_str().unwrap());
    }
    assert!(journal.texte().ends_with(&format!(
        "Pour être sûr : {}",
        choix.iter().map(|c| c.panne.as_str()).collect::<Vec<_>>().join(", ")
    )));
    // choisir une phrase lance son enquête ; « Aucune de ces phrases » ramène à l'accueil
    let j = Journal::default();
    assert_eq!(choisir(&choix, 1, j.fonction().as_ref()), Some((choix[1].panne.clone(), choix[1].reponses.clone())));
    assert_eq!(choisir(&choix, -1, j.fonction().as_ref()), None);
    assert_eq!(choisir(&choix, 9, j.fonction().as_ref()), None);
    assert_eq!(
        j.texte(),
        format!(
            "Pour être sûr : {}\nPour être sûr : aucune de ces phrases\nPour être sûr : aucune de ces phrases",
            choix[1].panne
        )
    );
}

#[test]
fn il_hesite_mais_rien_d_autre_ne_ressemble_l_enquete_commence() {
    let cat = catalogue();
    let r = reponse("seteint", &[], 0.5);
    let d = decider(&cat, "zzzz qqqq", &r, &|_| {});
    assert_eq!(d, Decision::Enquete { panne: "seteint".into(), reponses: Vec::new() });
}

#[test]
fn il_hesite_ses_autres_idees_viennent_apres_la_recherche_par_mots_sans_doublon() {
    let cat = catalogue();
    let r = lire_texte(
        r#"[{"c":"{\"panne\":\"SETEINT\",\"reponses\":{}}","p":0.4},{"c":"{\"panne\":\"fige\",\"reponses\":{}}","p":0.1},{"c":"{\"panne\":\"inconnue\",\"reponses\":{}}","p":0.1}]"#,
    );
    let Decision::PourEtreSur { choix, .. } = decider(&cat, "zzzz qqqq", &r, &|_| {}) else { panic!() };
    assert_eq!(choix.iter().map(|c| c.panne.as_str()).collect::<Vec<_>>(), ["SETEINT", "fige"]);
    // sûr : ses autres idées ne comptent pas
    let mut r = r;
    r.confiance = 0.99;
    assert!(matches!(decider(&cat, "zzzz qqqq", &r, &|_| {}), Decision::Enquete { .. }));
}

#[test]
fn le_seuil_est_0_95() {
    let cat = catalogue();
    let phrase = "mon pc s'éteint d'un coup";
    assert!(matches!(decider(&cat, phrase, &reponse("seteint", &[], 0.95), &|_| {}), Decision::Enquete { .. }));
    assert!(matches!(decider(&cat, phrase, &reponse("seteint", &[], 0.9499), &|_| {}), Decision::PourEtreSur { .. }));
}

#[test]
fn panne_inconnue_ou_erreur_la_recherche_par_mots_reste() {
    let cat = catalogue();
    let j = Journal::default();
    assert_eq!(
        decider(&cat, "x", &reponse("pas-une-panne", &[], 1.0), j.fonction().as_ref()),
        Decision::Echec { cause: "panne inconnue (pas-une-panne)".into() }
    );
    assert_eq!(
        decider(&cat, "x", &Reponse::vide("trop long (40 s)"), j.fonction().as_ref()),
        Decision::Echec { cause: "trop long (40 s)".into() }
    );
    assert_eq!(j.texte(), "Assistant : panne inconnue (pas-une-panne)\nAssistant : trop long (40 s)");
}

#[test]
fn la_confiance_vient_des_jetons_du_nom_de_la_panne_de_bout_en_bout() {
    // {"panne":"seteint",...} : les jetons « set » et « eint » ont une probabilité de 0,6 en tout : il hésite
    let jetons = json!([
        { "token": "{\"panne\":\"", "logprob": -0.01 },
        { "token": "set", "logprob": (0.8f64).ln() },
        { "token": "eint", "logprob": (0.75f64).ln() },
        { "token": "\",\"reponses\":{}}", "logprob": -2.0 },
    ]);
    let c = Comportement {
        contenu: r#"{"panne":"seteint","reponses":{}}"#.into(),
        jetons: Some(jetons),
        ..Comportement::default()
    };
    let b = Banc::nouveau("confiance", Scenario::Normal(c), delais_courts());
    let r = poser(&b, "mon pc s'éteint d'un coup");
    assert!((r.confiance - 0.6).abs() < 1e-9, "{}", r.confiance);
    assert!(matches!(decider(&catalogue(), "mon pc s'éteint d'un coup", &r, &|_| {}), Decision::PourEtreSur { .. }));
}
