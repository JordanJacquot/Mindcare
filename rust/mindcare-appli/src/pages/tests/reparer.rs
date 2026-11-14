//! « Le faire pour moi » dans les pages : avec le vrai catalogue, le bouton (« Si le problème est sur ce PC, Mindcare
//! peut le faire pour toi ») pour les 6 tests sans droits administrateur ; jamais quand la personne a répondu que le
//! problème n'est pas sur ce PC, ni quand « Ton PC » vise un autre PC, ni pour test-sfc et test-gaming-services, ni
//! (tant que l'Aide administrateur n'est pas là) pour les 4 tests qui demandent ces droits. Puis le parcours bouton,
//! attente, fini ou raté, avec une FAUSSE exécution : les pages ne lancent rien, la fin est donnée ici à la main.
//!
//! Le vrai catalogue ne pose la question « Sur ce PC ? » dans aucune des pannes de ces tests : le cas « Non, sur un
//! autre appareil » prend une copie où ces pannes commencent par elle ([`crate::reparer::catalogue_avec_cepc`]).

use std::collections::VecDeque;
use std::sync::OnceLock;

use serde_json::{Value, json};

use mindcare_donnees::{ChoixPc, IndicesGardes};
use mindcare_moteur::ps::{champ, egal, tableau};
use mindcare_moteur::{Catalogue, Enquete, ProfilPc, enquete};

use super::super::accueil::page_aide;
use super::{a_plat, actions_montrees, blocs, t, verifier_kit};
use crate::pages::{
    Effet, Etat, EtatReparation, SuiviReparation, Vue, ecrans, ids, textes_action, textes_reparer as x,
};
use crate::reparer::Reparation;

/// Les 6 tests que Mindcare fait lui-même, et les 4 qui attendent l'Aide administrateur.
const SANS_ADMIN: [(&str, Reparation); 6] = [
    ("test-dns", Reparation::Dns),
    ("test-wsreset", Reparation::CacheStore),
    ("test-cache-icones", Reparation::CacheIcones),
    ("test-vcredist", Reparation::VisualCpp),
    ("test-java", Reparation::Java),
    ("test-reset-parametres", Reparation::Parametres),
];
const ADMIN: [&str; 4] = ["test-profil-wifi", "test-ipconfig", "test-alim-defaut", "test-depanneur-maj"];

/// Le vrai catalogue.
fn cat() -> &'static Catalogue {
    super::cat()
}

/// Le vrai catalogue, avec « Sur ce PC ? » au début des pannes de ces tests (seulement pour le cas « Non »).
fn cat_cepc() -> &'static Catalogue {
    static CAT: OnceLock<Catalogue> = OnceLock::new();
    CAT.get_or_init(crate::reparer::catalogue_avec_cepc)
}

/// Les pages sans fenêtre ni disque : ce que Windows a noté est lu tout de suite (rien), l'enquête n'est pas
/// enregistrée, « Le faire pour moi » est seulement noté (jamais rien de lancé).
struct Banc {
    cat: &'static Catalogue,
    etat: Etat,
    json: Value,
    messages: Vec<(String, bool)>,
    journal: Vec<String>,
    reparations: Vec<Reparation>,
    windows_update: usize,
    erreurs_kit: Vec<String>,
}

impl Banc {
    fn nouveau() -> Banc {
        Banc::dans(cat())
    }

    fn dans(cat: &'static Catalogue) -> Banc {
        let etat = Etat::nouveau("0.1.0", "neon", Vec::new());
        let json = etat.etat_kit(cat, None);
        Banc {
            cat,
            etat,
            json,
            messages: Vec::new(),
            journal: Vec::new(),
            reparations: Vec::new(),
            windows_update: 0,
            erreurs_kit: Vec::new(),
        }
    }

    fn action(&mut self, id: &str, valeur: Value, ecran: &str) -> Vec<Effet> {
        let m = json!({ "type": "action", "id": id, "valeur": valeur, "ecran": ecran });
        let effets = self.etat.agir(self.cat, &m, &mut || "20261114-101500-0001".to_string());
        self.executer(effets.clone());
        effets
    }

    fn executer(&mut self, effets: Vec<Effet>) {
        let mut file: VecDeque<Effet> = effets.into();
        while let Some(e) = file.pop_front() {
            match e {
                Effet::Afficher { ecran } => {
                    self.json = self.etat.etat_kit(self.cat, ecran.as_deref());
                    self.erreurs_kit.extend(verifier_kit(&self.json));
                }
                Effet::Message { texte, bravo } => self.messages.push((texte, bravo)),
                Effet::Journal(l) => self.journal.push(l),
                Effet::LireWindows => file.extend(self.etat.windows_lu(self.cat, IndicesGardes::default(), Vec::new())),
                Effet::Reparer(r) => self.reparations.push(r),
                Effet::OuvrirWindowsUpdate => self.windows_update += 1,
                Effet::Inconnue(x) => self.erreurs_kit.push(format!("action inconnue : {x}")),
                _ => {}
            }
        }
    }

    fn enquete(&self) -> &Value {
        &self.json["pages"][ecrans::ENQUETE]
    }

    fn vue(&self) -> Option<Vue> {
        self.etat.vue(self.cat)
    }

    fn propre(&self) {
        assert!(self.erreurs_kit.is_empty(), "{:#?}", self.erreurs_kit);
    }

    /// Chaque action montrée est acceptée (jouée sur une copie de l'état).
    fn toutes_acceptees(&self) -> Vec<String> {
        let mut refus = Vec::new();
        for (id, valeur, ecran) in actions_montrees(&self.json) {
            let mut copie = self.etat.clone();
            let m = json!({ "type": "action", "id": id, "valeur": valeur, "ecran": ecran });
            let effets = copie.agir(self.cat, &m, &mut || "20261114-101500-ffff".to_string());
            if effets.is_empty() || effets.iter().any(|e| matches!(e, Effet::Inconnue(_))) {
                refus.push(format!("{id} ({ecran}) : {effets:?}"));
            }
        }
        refus
    }
}

fn cepc(e: &Enquete) -> bool {
    e.notes.iter().any(|n| egal(n, "cepc"))
}

/// La question « Sur ce PC ? » a été posée, et répondue sans « Oui, sur ce PC-là ».
fn dit_ailleurs(e: &Enquete) -> bool {
    e.faits.iter().any(|f| egal(&t(&f.etape, "Id"), "cepc")) && !cepc(e)
}

/// L'enquête comme les pages la rejouent avec un Windows lu sans rien de noté.
fn rejouee(c: &Catalogue, p: &mindcare_moteur::Objet, choix: &[i64]) -> Enquete {
    let e = enquete(c, p, choix, None);
    if cepc(&e) { enquete(c, p, choix, Some(&IndicesGardes::default().moteur())) } else { e }
}

/// Une suite de réponses qui mène à ce test dans ce catalogue, après un « Non » à « Sur ce PC ? » ou sans.
fn chemin_dans(c: &Catalogue, test: &str, ailleurs: bool) -> Option<(String, Vec<i64>)> {
    for p in c.pannes() {
        let mut file: VecDeque<Vec<i64>> = VecDeque::from([Vec::new()]);
        let mut n = 0;
        while let Some(choix) = file.pop_front() {
            n += 1;
            if n > 3000 {
                break;
            }
            let e = rejouee(c, p, &choix);
            let Some(et) = &e.etape else { continue };
            if egal(&t(et, "Id"), test) && dit_ailleurs(&e) == ailleurs {
                return Some((t(p, "Id"), choix));
            }
            for k in 0..tableau(champ(et, "Reponses")).len() {
                let mut suite = choix.clone();
                suite.push(k as i64);
                file.push_back(suite);
            }
        }
    }
    None
}

fn chemin_vers(test: &str) -> Option<(String, Vec<i64>)> {
    chemin_dans(cat(), test, false)
}

/// Le banc arrivé sur ce test, dans ce catalogue.
fn banc_dans(c: &'static Catalogue, test: &str, ailleurs: bool) -> Banc {
    let (panne, choix) = chemin_dans(c, test, ailleurs).unwrap_or_else(|| panic!("{test} introuvable ({ailleurs})"));
    let mut b = Banc::dans(c);
    b.action(&format!("panne:{panne}"), Value::Null, ecrans::RACONTE);
    for c in &choix {
        if *c < 0 {
            b.action(ids::NSP, Value::Null, ecrans::ENQUETE);
        } else {
            b.action(ids::REP, json!(c.to_string()), ecrans::ENQUETE);
        }
    }
    assert_eq!(etape(&b).as_deref(), Some(test), "{panne} {choix:?}");
    b
}

/// Le banc arrivé sur ce test, avec le vrai catalogue.
fn banc_sur(test: &str) -> Banc {
    banc_dans(cat(), test, false)
}

/// Les gestes du test montré.
fn gestes_de(b: &Banc) -> Vec<String> {
    match b.vue() {
        Some(Vue::Etape(e)) => super::textes(e.etape.as_ref().unwrap(), "Gestes"),
        _ => Vec::new(),
    }
}

fn etape(b: &Banc) -> Option<String> {
    match b.vue()? {
        Vue::Etape(e) => e.etape.as_ref().map(|et| t(et, "Id")),
        _ => None,
    }
}

/// Pas de bouton : la page reste celle d'aujourd'hui, et le bouton demandé quand même ne lance rien.
fn sans_bouton(b: &mut Banc, quoi: &str) {
    b.propre();
    let page = b.enquete().clone();
    assert!(bouton(&page).is_none(), "{quoi} : bouton");
    assert!(blocs(&page, "repli").is_empty(), "{quoi}");
    assert!(types_du_haut(&page).contains(&"case".to_string()), "{quoi} : les gestes comme aujourd'hui");
    assert!(a_les_reponses(&page), "{quoi}");
    assert!(b.toutes_acceptees().is_empty(), "{quoi}");
    let effets = b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    assert_eq!(effets, vec![Effet::Afficher { ecran: None }], "{quoi}");
    assert!(b.reparations.is_empty() && b.etat.reparation.is_none(), "{quoi}");
}

#[test]
fn rien_si_la_personne_a_dit_non_sur_un_autre_appareil() {
    for (test, _) in SANS_ADMIN {
        let mut b = banc_dans(cat_cepc(), test, true);
        sans_bouton(&mut b, test);
        // (« Oui, sur ce PC-là » : le bouton est là)
        let b = banc_dans(cat_cepc(), test, false);
        assert!(bouton(b.enquete()).is_some(), "{test} : pas de bouton après « Oui »");
    }
}

#[test]
fn rien_si_ton_pc_vise_un_autre_pc() {
    for (test, _) in SANS_ADMIN {
        let mut b = banc_sur(test);
        assert!(bouton(b.enquete()).is_some(), "{test}");
        b.etat.choix_pc = ChoixPc::Autre(ProfilPc::inconnu());
        b.executer(vec![Effet::Afficher { ecran: None }]);
        sans_bouton(&mut b, test);
        // (revenu à « mon PC » : le bouton revient)
        b.etat.choix_pc = ChoixPc::MonPc;
        b.executer(vec![Effet::Afficher { ecran: None }]);
        assert!(bouton(b.enquete()).is_some(), "{test}");
    }
}

#[test]
fn jamais_pour_sfc_les_services_de_jeu_ni_les_tests_admin() {
    for test in ["test-sfc", "test-gaming-services"].into_iter().chain(ADMIN) {
        let mut b = banc_sur(test);
        sans_bouton(&mut b, test);
    }
}

/// Le bouton « Le faire pour moi » de la page, s'il y est.
fn bouton(page: &Value) -> Option<Value> {
    blocs(page, "ligne").into_iter().find(|l| l["bouton"]["id"] == ids::FAIRE).cloned()
}

fn a_les_reponses(page: &Value) -> bool {
    !blocs(page, "cartes").is_empty()
}

/// Les blocs du haut de la page (sans ceux des parties repliées).
fn types_du_haut(page: &Value) -> Vec<String> {
    page["blocs"].as_array().unwrap().iter().map(|b| b["type"].as_str().unwrap_or_default().to_string()).collect()
}

#[test]
fn le_bouton_avec_le_vrai_catalogue_pour_les_six_tests() {
    for (test, r) in SANS_ADMIN {
        let b = banc_sur(test);
        b.propre();
        let page = b.enquete().clone();
        let l = bouton(&page).unwrap_or_else(|| panic!("{test} : pas de bouton"));
        assert_eq!(l["texte"], x::PEUT);
        assert_eq!(l["bouton"]["texte"], x::BOUTON);
        assert_eq!(l["note"], textes_action(r).note);
        // Le bouton d'abord, puis les gestes et les commandes repliés
        let haut = page["blocs"].as_array().unwrap();
        let ou_bouton = haut.iter().position(|b| b["bouton"]["id"] == ids::FAIRE).unwrap();
        let repli = haut.iter().position(|b| b["type"] == "repli").expect("le repli");
        assert_eq!(repli, ou_bouton + 1, "{test}");
        assert_eq!(haut[repli]["titre"], x::SOI_MEME);
        let dedans = a_plat(&haut[repli]["blocs"]);
        let gestes = gestes_de(&b);
        assert_eq!(dedans.iter().filter(|b| b["type"] == "case").count(), gestes.len(), "{test}");
        assert_eq!(haut[repli]["resume"], format!("{} gestes", gestes.len()));
        assert!(dedans.iter().any(|b| b["bouton"]["id"] == ids::COMMANDE), "{test} : la commande à copier");
        assert!(!types_du_haut(&page).contains(&"case".to_string()), "{test} : des gestes dépliés");
        assert!(a_les_reponses(&page));
        assert!(b.toutes_acceptees().is_empty());
    }
}

#[test]
fn bouton_attente_puis_fait() {
    let mut b = banc_sur("test-dns");
    let choix = b.etat.en_cours.choix.clone();
    let effets = b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    assert!(effets.contains(&Effet::Reparer(Reparation::Dns)), "{effets:?}");
    assert!(effets.contains(&Effet::Journal("Le faire pour moi : dns (test-dns)".into())));
    assert_eq!(b.reparations, [Reparation::Dns]);
    // Pendant : la jauge, les étapes, la note ; pas de réponses, pas de bouton, pas de gestes
    b.propre();
    let page = b.enquete().clone();
    let jauge = blocs(&page, "jauge");
    assert_eq!(jauge.len(), 1);
    assert_eq!(jauge[0]["texte"], x::PENDANT);
    let etapes = blocs(&page, "etapes");
    assert_eq!(etapes[0]["elements"][0]["texte"], textes_action(Reparation::Dns).etapes[0]);
    assert!(etapes[0]["elements"][0].get("fait").is_none());
    assert!(blocs(&page, "note").iter().any(|n| n["texte"] == x::PENDANT_NOTE));
    assert!(!a_les_reponses(&page));
    assert!(blocs(&page, "bouton").iter().all(|x| x["id"] != ids::NSP && x["id"] != ids::PLUS_TARD));
    assert!(bouton(&page).is_none() && blocs(&page, "case").is_empty());
    assert!(b.toutes_acceptees().is_empty());
    // Un clic de plus ne relance rien ; une réponse envoyée quand même est laissée
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    assert_eq!(b.reparations, [Reparation::Dns]);
    b.action(ids::REP, json!("0"), ecrans::ENQUETE);
    b.action(ids::NSP, Value::Null, ecrans::ENQUETE);
    assert_eq!(b.etat.en_cours.choix, choix);
    // Une étape faite : cochée
    let effets = b.etat.reparation_avance(1);
    b.executer(effets);
    assert_eq!(blocs(b.enquete(), "etapes")[0]["elements"][0]["fait"], true);
    // Fini : « Fait par Mindcare », le geste qui reste, puis les réponses
    let effets = b.etat.reparation_finie(Ok(()));
    assert!(effets.contains(&Effet::Journal("Le faire pour moi : fait".into())));
    b.executer(effets);
    b.propre();
    let page = b.enquete().clone();
    let v = blocs(&page, "verdict");
    assert_eq!(v.len(), 1);
    assert_eq!(
        (v[0]["surtitre"].clone(), v[0]["couleur"].clone(), v[0]["compact"].clone()),
        (json!(x::FAIT), json!("vert"), json!(true))
    );
    assert_eq!(v[0]["titre"], textes_action(Reparation::Dns).fait);
    assert!(blocs(&page, "ligne").iter().any(|l| l["note"] == x::RESTE_NOTE));
    assert!(a_les_reponses(&page));
    assert!(bouton(&page).is_none() && blocs(&page, "jauge").is_empty());
    assert!(b.toutes_acceptees().is_empty());
    // Encore un clic : rien
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    assert_eq!(b.reparations, [Reparation::Dns]);
    // La réponse fait avancer l'enquête ; l'étape suivante ne montre rien de l'action
    b.action(ids::REP, json!("1"), ecrans::ENQUETE);
    assert!(b.etat.en_cours.choix.len() > choix.len());
    assert_ne!(etape(&b).as_deref(), Some("test-dns"));
    assert!(!b.enquete().to_string().contains(x::FAIT));
    // Étape précédente : de retour sur le test, ce qu'elle a donné, jamais le bouton une seconde fois
    b.action(ids::PRECEDENT, Value::Null, ecrans::ENQUETE);
    assert_eq!(etape(&b).as_deref(), Some("test-dns"));
    assert_eq!(blocs(b.enquete(), "verdict")[0]["surtitre"], x::FAIT);
    assert!(bouton(b.enquete()).is_none());
    // Une autre enquête : l'action finie est oubliée
    b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
    assert!(b.etat.reparation.is_none());
}

#[test]
fn bouton_attente_puis_rate() {
    // Raté avant d'avoir rien fait : rien n'a changé, les gestes dépliés
    let mut b = banc_sur("test-dns");
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    let effets = b.etat.reparation_finie(Err("ipconfig.exe : code 1".into()));
    assert!(effets.contains(&Effet::Journal("Le faire pour moi : raté (ipconfig.exe : code 1)".into())));
    b.executer(effets);
    b.propre();
    let page = b.enquete().clone();
    let v = blocs(&page, "verdict");
    assert_eq!((v[0]["surtitre"].clone(), v[0]["couleur"].clone()), (json!(x::RATE), json!("orange")));
    assert_eq!(v[0]["titre"], x::RATE_RIEN);
    assert_eq!(v[0]["texte"], x::RATE_TEXTE);
    assert!(types_du_haut(&page).contains(&"case".to_string()), "les gestes dépliés");
    assert!(blocs(&page, "repli").is_empty() && bouton(&page).is_none());
    assert!(blocs(&page, "ligne").iter().any(|l| l["bouton"]["id"] == ids::COMMANDE));
    assert!(a_les_reponses(&page));
    // Pas de nouvel essai par le bouton (les gestes sont là)
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    assert_eq!(b.reparations, [Reparation::Dns]);

    // Raté après une étape sur deux : « une partie seulement »
    let mut b = banc_sur("test-vcredist");
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    let effets = b.etat.reparation_avance(1);
    b.executer(effets);
    let e = blocs(b.enquete(), "etapes")[0]["elements"].clone();
    assert_eq!((e[0]["fait"].clone(), e[1].get("fait").cloned()), (json!(true), None));
    assert_eq!(blocs(b.enquete(), "jauge")[0]["valeur"], 75);
    let effets = b.etat.reparation_finie(Err("winget.exe : code 1".into()));
    b.executer(effets);
    b.propre();
    assert_eq!(blocs(b.enquete(), "verdict")[0]["titre"], x::RATE_PARTIE);
}

#[test]
fn partir_pendant_l_action_puis_revenir() {
    let mut b = banc_sur("test-java");
    let choix = b.etat.en_cours.choix.clone();
    b.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    // Étape précédente pendant l'action : elle continue, et rien d'autre ne part
    b.action(ids::PRECEDENT, Value::Null, ecrans::ENQUETE);
    assert!(b.etat.reparation.as_ref().is_some_and(SuiviReparation::en_cours));
    assert_ne!(etape(&b).as_deref(), Some("test-java"));
    // L'accueil pendant l'action : elle reste suivie ; une autre enquête ne peut rien lancer avant sa fin
    b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
    assert!(b.etat.reparation.as_ref().is_some_and(SuiviReparation::en_cours));
    let mut autre = banc_sur("test-dns");
    autre.etat.reparation = b.etat.reparation.clone();
    autre.action(ids::FAIRE, Value::Null, ecrans::ENQUETE);
    assert!(autre.reparations.is_empty(), "une seconde action pendant la première");
    // (la même enquête reprise : de retour sur le test, la jauge, puis « Fait par Mindcare » à la fin)
    let (panne, _) = chemin_vers("test-java").unwrap();
    let mut b2 = Banc::nouveau();
    b2.action(&format!("panne:{panne}"), Value::Null, ecrans::RACONTE);
    b2.etat.reparation = b.etat.reparation.clone().map(|mut s| {
        s.diag_id = b2.etat.en_cours.diag_id.clone();
        s
    });
    for c in &choix {
        if *c < 0 {
            b2.action(ids::NSP, Value::Null, ecrans::ENQUETE);
        } else {
            b2.action(ids::REP, json!(c.to_string()), ecrans::ENQUETE);
        }
    }
    b2.executer(vec![Effet::Afficher { ecran: None }]);
    assert_eq!(etape(&b2).as_deref(), Some("test-java"));
    assert!(!blocs(b2.enquete(), "jauge").is_empty());
    let effets = b2.etat.reparation_finie(Ok(()));
    b2.executer(effets);
    assert_eq!(blocs(b2.enquete(), "verdict")[0]["surtitre"], x::FAIT);
    assert!(b2.reparations.is_empty());
}

#[test]
fn ouvrir_windows_update_une_fois_les_mises_a_jour_mises_de_cote() {
    // (l'action des mises à jour attend l'Aide administrateur : sa fin est posée à la main, pour voir son écran)
    let Some(_) = chemin_vers("test-depanneur-maj") else { panic!("test-depanneur-maj introuvable") };
    let mut b = banc_sur("test-depanneur-maj");
    let diag_id = b.etat.en_cours.diag_id.clone();
    // Sans action faite : le bouton ne fait rien
    assert_eq!(b.action(ids::WINDOWS_UPDATE, Value::Null, ecrans::ENQUETE), vec![Effet::Afficher { ecran: None }]);
    b.etat.reparation = Some(SuiviReparation {
        diag_id,
        etape: "test-depanneur-maj".into(),
        action: Reparation::MisesAJour,
        etat: EtatReparation::Faite,
    });
    b.executer(vec![Effet::Afficher { ecran: None }]);
    b.propre();
    let page = b.enquete().clone();
    let l = blocs(&page, "ligne").into_iter().find(|l| l["bouton"]["id"] == ids::WINDOWS_UPDATE).cloned().unwrap();
    assert_eq!(l["bouton"]["texte"], x::WINDOWS_UPDATE);
    assert_eq!(l["note"], x::RESTE_NOTE);
    assert!(b.toutes_acceptees().is_empty());
    assert_eq!(b.action(ids::WINDOWS_UPDATE, Value::Null, ecrans::ENQUETE), vec![Effet::OuvrirWindowsUpdate]);
    assert_eq!(b.windows_update, 1);
    // Pas ouvert : une bulle dit comment faire
    let effets = b.etat.windows_update_ouvert(Err("code 2".into()));
    b.executer(effets);
    assert!(b.messages.contains(&(x::WINDOWS_UPDATE_RATE.to_string(), false)));
}

#[test]
fn la_faq_dit_le_bouton() {
    let p = page_aide();
    let reponse = p["blocs"][2]["elements"][0]["reponse"].as_str().unwrap();
    assert!(reponse.contains("ne lance rien sans que tu cliques"), "{reponse}");
    assert!(reponse.contains("« Le faire pour moi »"), "{reponse}");
    assert!(!reponse.contains("ne lance jamais rien"));
}
