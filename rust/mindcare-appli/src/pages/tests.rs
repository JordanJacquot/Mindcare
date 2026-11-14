//! Les tests des pages : chaque page construite pour de vrais cas du catalogue, des parcours au hasard sur toutes les
//! pannes (on atteint toujours une réponse finale, la même que le moteur), le format du kit vérifié à chaque écran, et
//! chaque action montrée acceptée.
//!
//! Le banc d'essai exécute les effets comme le fera programme.rs, avec les vraies données de mindcare-donnees dans un
//! dossier temporaire neuf (jamais le vrai dossier de Mindcare), une horloge figée et un faux Windows.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Map, Value, json};

use mindcare_donnees::rapport::{SurLePc, enregistrer_rapport, reparateur as textes_rapport};
use mindcare_donnees::{Donnees, IndiceGarde, IndicesGardes, Moment, horloge_figee, titre_reponse};
use mindcare_moteur::ps::{champ, champ_v, egal, parcours, tableau, texte, vrai};
use mindcare_moteur::{Catalogue, Enquete, HORS_SUJET, INDICES_CONNUS, Indices, Objet, TypeVerdict, chercher, enquete};

use super::actions::{PLUS_TARD_MESSAGE, ids};

/// Les pages de l'assistant et de la dictée (étape 6.4).
mod aides;
/// Le PC dans les pages (étape 8) : le tri, la bande, « Ton PC ».
mod pc;
/// « Le faire pour moi » : le bouton, l'attente, fini ou raté.
mod reparer;
use super::outils::{gestes_materiel, suspects_montres, t, textes};
use super::{Effet, Etat, Vue, ecrans, etat_sans_catalogue};

// ------------------------------------------------------------------------------------------------ le catalogue

fn fichiers_mindcare() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("Mindcare").join("fichiers")
}

fn cat() -> &'static Catalogue {
    static CAT: OnceLock<Catalogue> = OnceLock::new();
    CAT.get_or_init(|| Catalogue::lire_fichier(&fichiers_mindcare().join("enquetes.json")).expect("enquetes.json"))
}

fn maintenant() -> Moment {
    Moment::lire("2026-11-03T10:15:00").expect("date")
}

// ------------------------------------------------------------------------------------------------ un faux Windows

/// Ce que Windows a noté : rien de lisible (aucun indice), tout vu, ou au hasard.
fn indices(mode: u64, h: &mut Hasard) -> IndicesGardes {
    let mut l = IndicesGardes::default();
    if mode == 0 {
        return l;
    }
    for nom in INDICES_CONNUS {
        let vu = mode == 1 || h.sous(2) == 0;
        let partiel = mode == 2 && h.sous(4) == 0;
        l.mettre(
            nom,
            IndiceGarde {
                vu,
                texte: format!("Windows a noté : {nom}"),
                texte_ok: format!("Rien de noté : {nom}"),
                affiche: true,
                partiel,
            },
        );
    }
    l
}

/// Un hasard toujours le même pour une même graine (xorshift).
struct Hasard(u64);

impl Hasard {
    fn nouveau(graine: u64) -> Hasard {
        Hasard(graine.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn suivant(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn sous(&mut self, n: u64) -> u64 {
        self.suivant() % n.max(1)
    }
}

// ------------------------------------------------------------------------------------------------ le banc d'essai

static DOSSIERS: AtomicUsize = AtomicUsize::new(0);

/// L'appli sans fenêtre : l'état des pages, ses effets exécutés comme le fera programme.rs.
struct Banc {
    etat: Etat,
    donnees: Donnees,
    dossier: PathBuf,
    ids: u32,
    /// Le dernier état envoyé au kit
    json: Value,
    /// Le dernier écran demandé
    ecran: Option<String>,
    messages: Vec<(String, bool)>,
    journal: Vec<String>,
    /// Ce que Windows répond tout de suite (sinon, la lecture reste en attente)
    windows: Option<IndicesGardes>,
    lectures: usize,
    menu: bool,
    inconnues: Vec<String>,
    erreurs_kit: Vec<String>,
    /// Les effets de l'assistant et de la dictée demandés (exécutés par effets.rs, testés dans effets\aides.rs)
    aides: Vec<Effet>,
    /// « Le faire pour moi » demandé (jamais rien de lancé ici : la fin est donnée par le test), et « Ouvrir Windows
    /// Update »
    reparations: Vec<crate::reparer::Reparation>,
    windows_update: usize,
}

impl Banc {
    fn nouveau(windows: Option<IndicesGardes>) -> Banc {
        let n = DOSSIERS.fetch_add(1, Ordering::SeqCst);
        let dossier = std::env::temp_dir().join(format!("mindcare-pages-essai-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dossier);
        let mut donnees = Donnees::ouvrir(&dossier, horloge_figee(maintenant()), false);
        donnees.lire_historique();
        let etat = Etat::nouveau("0.1.0", "neon", donnees.historique.liste.clone());
        let json = etat.etat_kit(cat(), None);
        Banc {
            etat,
            donnees,
            dossier,
            ids: 0,
            json,
            ecran: None,
            messages: Vec::new(),
            journal: Vec::new(),
            windows,
            lectures: 0,
            menu: false,
            inconnues: Vec::new(),
            erreurs_kit: Vec::new(),
            aides: Vec::new(),
            reparations: Vec::new(),
            windows_update: 0,
        }
    }

    /// Un message du kit.
    fn message(&mut self, m: Value) -> Vec<Effet> {
        let mut ids = self.ids;
        let effets = self.etat.agir(cat(), &m, &mut || {
            ids += 1;
            format!("20261103-101500-{ids:04x}")
        });
        self.ids = ids;
        self.executer(effets.clone());
        effets
    }

    /// Une action, depuis l'écran donné.
    fn action(&mut self, id: &str, valeur: Value, ecran: &str) -> Vec<Effet> {
        self.message(json!({ "type": "action", "id": id, "valeur": valeur, "ecran": ecran }))
    }

    fn executer(&mut self, effets: Vec<Effet>) {
        let mut file: VecDeque<Effet> = effets.into();
        while let Some(e) = file.pop_front() {
            match e {
                Effet::Afficher { ecran } => {
                    if self.etat.rapport_a_refaire(cat()) {
                        self.etat.rapport = self.rapport();
                    }
                    self.json = self.etat.etat_kit(cat(), ecran.as_deref());
                    self.erreurs_kit.extend(verifier_kit(&self.json));
                    if ecran.is_some() {
                        self.ecran = ecran;
                    }
                }
                Effet::Message { texte, bravo } => self.messages.push((texte, bravo)),
                Effet::Journal(l) => self.journal.push(l),
                Effet::EnregistrerEnquete => {
                    let (e, jour) = self.etat.enquete_a_enregistrer(cat()).expect("une enquête prête à enregistrer");
                    self.etat.en_cours.enregistrer(cat(), &mut self.donnees, e, &jour);
                    self.etat.historique = self.donnees.historique.liste.clone();
                }
                Effet::LireWindows => {
                    self.lectures += 1;
                    if let Some(i) = self.windows.clone() {
                        file.extend(self.etat.windows_lu(cat(), i, vec!["8 Go Kingston KF3200 (DIMM1)".into()]));
                    }
                }
                Effet::RapportBureau => {
                    self.etat.rapport = self.rapport();
                    let r = self.rapport().ok_or_else(|| "aucun rapport".to_string()).and_then(|t| {
                        enregistrer_rapport(&self.dossier, &t, self.donnees.journal())
                            .map(|f| f.file_name().unwrap_or_default().to_string_lossy().into_owned())
                            .map_err(|e| e.to_string())
                    });
                    file.extend(self.etat.rapport_enregistre(r));
                }
                Effet::CopierRapport => {
                    self.etat.rapport = self.rapport();
                    let r = self.rapport().map(|_| ()).ok_or_else(|| "aucun rapport".to_string());
                    file.extend(self.etat.rapport_copie(r));
                }
                Effet::CopierCommande { numero, .. } => file.extend(self.etat.commande_copiee(numero, Ok(()))),
                Effet::MenuMindset => self.menu = true,
                Effet::Inconnue(x) => self.inconnues.push(x),
                Effet::DemarrerAssistant
                | Effet::Demander { .. }
                | Effet::OublierQuestion
                | Effet::InstallerAssistant
                | Effet::InstallerDictee
                | Effet::Parler
                | Effet::AnnulerDictee
                | Effet::EffacerMessageDictee => self.aides.push(e),
                Effet::ChoisirPc(c) => self.donnees.choisir_pc(&c),
                Effet::Reparer(r) => self.reparations.push(r),
                Effet::OuvrirWindowsUpdate => self.windows_update += 1,
            }
        }
    }

    fn rapport(&self) -> Option<String> {
        self.etat.en_cours.rapport(cat(), &maintenant(), &SurLePc::default(), "0.1.0")
    }

    fn page(&self, id: &str) -> Option<&Value> {
        self.json["pages"].get(id)
    }

    fn enquete(&self) -> &Value {
        self.page(ecrans::ENQUETE).expect("la page de l'enquête")
    }

    /// Commence une enquête depuis « Raconte ton problème ».
    fn commencer(&mut self, panne: &str) {
        self.action(&format!("panne:{panne}"), Value::Null, ecrans::RACONTE);
        assert_eq!(self.ecran.as_deref(), Some(ecrans::ENQUETE), "{panne} : l'enquête ne s'ouvre pas");
    }

    /// Donne ces réponses, une à une, depuis la page de l'enquête.
    fn repondre(&mut self, choix: &[i64]) {
        for c in choix {
            if *c < 0 {
                self.action(ids::NSP, Value::Null, ecrans::ENQUETE);
            } else {
                self.action(ids::REP, json!(c.to_string()), ecrans::ENQUETE);
            }
        }
    }

    fn vue(&self) -> Option<Vue> {
        self.etat.vue(cat())
    }

    fn propre(&self) {
        assert!(
            self.erreurs_kit.is_empty(),
            "format du kit : {:#?}",
            &self.erreurs_kit[..self.erreurs_kit.len().min(10)]
        );
        assert!(self.inconnues.is_empty(), "actions inconnues : {:?}", self.inconnues);
    }
}

impl Drop for Banc {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dossier);
    }
}

// ------------------------------------------------------------------------------------------------ le format du kit

/// Les types de blocs que le kit sait dessiner (en tête de kit.js).
const BLOCS: &[&str] = &[
    "titre",
    "texte",
    "note",
    "score",
    "ligne",
    "bascule",
    "case",
    "bouton",
    "champ",
    "choix",
    "cartes",
    "outils",
    "raccourcis",
    "mesures",
    "pastilles",
    "reglages",
    "faq",
    "jauge",
    "liste",
    "image",
    "courbe",
    "mois",
    "rdv",
    "grille",
    "chiffres",
    "banniere",
    "reseau",
    "apercu",
    "etapes",
    "groupe",
    "bande",
    "segments",
    "suspects",
    "verdict",
    "repli",
    "rangs",
    "colonnes",
    "citation",
    "petit",
    "depliables",
];
/// Les icônes dessinées par le kit.
const ICONES: &[&str] = &[
    "pc",
    "tel",
    "eclair",
    "marche",
    "wifi",
    "cable",
    "aucune",
    "courbe",
    "loupe",
    "actu",
    "reglages",
    "aide",
    "plus",
    "agenda",
    "cloche",
    "coeur",
    "liste",
    "maison",
    "bouclier",
    "poubelle",
    "balai",
    "dossier",
    "horloge",
    "sauve",
    "euro",
    "etoile",
    "son",
    "manette",
    "radar",
    "jauge",
    "puce",
    "memoire",
    "disque",
    "test",
    "rapport",
    "alerte",
    "ok",
    "app",
    "windows",
    "ondes",
    "ecran",
    "box",
    "portable",
    "tablette",
    "console",
    "tv",
    "camera",
    "imprimante",
    "enceinte",
    "ampoule",
    "repeteur",
    "inconnu",
    "internet",
    "fleche",
    "retour",
    "croix",
    "coche",
    "voyants",
    "demarrage",
    "moniteur",
    "thermo",
    "batterie",
    "souris",
    "cle",
    "etincelle",
    "micro",
    "question",
    "bas",
];
/// Les styles de bouton du kit.
const STYLES: &[&str] = &["second", "danger", "lien", "discret", "doux", "principal"];
const COULEURS: &[&str] = &["P", "S", "T", "vert", "orange", "rouge", "fond", "fond2"];
const TONS: &[&str] = &["", "ok", "attention", "urgent"];

fn texte_plein(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty())
}

/// Tous les blocs d'une liste, ceux des parties repliées et des colonnes compris, dans l'ordre.
fn a_plat(liste: &Value) -> Vec<&Value> {
    let mut l = Vec::new();
    for b in liste.as_array().into_iter().flatten() {
        l.push(b);
        l.extend(a_plat(&b["blocs"]));
        if b["type"] == "colonnes" {
            for c in b["elements"].as_array().into_iter().flatten() {
                l.extend(a_plat(&c["blocs"]));
            }
        }
    }
    l
}

/// Ce qui ne va pas dans un état envoyé au kit (une liste vide : tout va bien).
fn verifier_kit(etat: &Value) -> Vec<String> {
    let mut err = Vec::new();
    let mut e = |m: String| err.push(m);
    if !texte_plein(&etat["app"]["nom"]) || !etat["app"]["version"].is_string() {
        e("app : nom ou version".into());
    }
    let images = fichiers_mindcare().join("images");
    let image_ok = |url: &str| {
        url == super::LOGO
            || url.strip_prefix("images/").is_some_and(|n| n.ends_with(".svg") && images.join(n).is_file())
    };
    if !image_ok(etat["app"]["logo"].as_str().unwrap_or_default()) {
        e("app : logo".into());
    }
    let pages = etat["pages"].as_object().cloned().unwrap_or_default();
    let tuiles = etat["accueil"]["tuiles"].as_array().cloned().unwrap_or_default();
    if tuiles.is_empty() {
        e("accueil : aucune tuile".into());
    }
    for x in tuiles.iter().chain(etat["accueil"]["ronds"].as_array().into_iter().flatten()) {
        if !texte_plein(&x["id"]) || !texte_plein(&x["nom"]) {
            e(format!("accueil : tuile ou rond sans id ou nom : {x}"));
        }
        if let Some(i) = x["icone"].as_str()
            && !ICONES.contains(&i)
        {
            e(format!("accueil : icône inconnue {i}"));
        }
        if let Some(c) = x["couleur"].as_str()
            && !COULEURS.contains(&c)
        {
            e(format!("accueil : couleur inconnue {c}"));
        }
    }
    if let Some(pc) = etat.get("pc")
        && (!texte_plein(&pc["id"])
            || !texte_plein(&pc["texte"])
            || pc.get("icone").and_then(Value::as_str).is_some_and(|i| !ICONES.contains(&i)))
    {
        e(format!("pastille Ton PC mal remplie : {pc}"));
    }
    if let Some(ecran) = etat.get("ecran").and_then(Value::as_str)
        && ecran != ecrans::ACCUEIL
        && !pages.contains_key(ecran)
    {
        e(format!("écran demandé sans page : {ecran}"));
    }
    for (id, p) in &pages {
        let mut ep = |m: String| e(format!("page {id} : {m}"));
        if !texte_plein(&p["titre"]) {
            ep("sans titre".into());
        }
        if let Some(parent) = p.get("parent").and_then(Value::as_str)
            && !pages.contains_key(parent)
        {
            ep(format!("parent sans page : {parent}"));
        }
        if let Some(r) = p.get("retour")
            && r != "appli"
        {
            ep(format!("retour inconnu : {r}"));
        }
        if let Some(c) = p.get("chemin")
            && c.as_array().is_none_or(|l| l.iter().any(|x| !texte_plein(x)))
        {
            ep(format!("chemin mal rempli : {c}"));
        }
        if let Some(fo) = p.get("forme")
            && fo != "question"
            && fo != "carte"
        {
            ep(format!("forme inconnue : {fo}"));
        }
        if let Some(i) = p.get("icone").and_then(Value::as_str)
            && !ICONES.contains(&i)
        {
            ep(format!("icône inconnue {i}"));
        }
        if let Some(c) = p.get("couleur").and_then(Value::as_str)
            && !COULEURS.contains(&c)
        {
            ep(format!("couleur inconnue {c}"));
        }
        for a in p["actions"].as_array().into_iter().flatten() {
            if !texte_plein(&a["id"]) || !texte_plein(&a["texte"]) {
                ep(format!("action sans id ou texte : {a}"));
            }
            if let Some(i) = a.get("icone").and_then(Value::as_str)
                && !ICONES.contains(&i)
            {
                ep(format!("action : icône inconnue {i}"));
            }
        }
        if let Some(m) = p.get("module") {
            if !texte_plein(&m["titre"]) {
                ep("module sans titre".into());
            }
            if let Some(a) = m.get("anneau") {
                let (v, s) = (a["valeur"].as_f64(), a["sur"].as_f64());
                if !matches!((v, s), (Some(v), Some(s)) if s > 0.0 && (0.0..=s).contains(&v))
                    || !texte_plein(&a["texte"])
                {
                    ep(format!("anneau mal rempli : {a}"));
                }
            } else if m.get("icone").is_none() && m.get("image").is_none() {
                ep("module sans anneau, icône ni image".into());
            }
            if let Some(i) = m.get("image").and_then(Value::as_str)
                && !image_ok(i)
            {
                ep(format!("image absente : {i}"));
            }
            for b in m["boutons"].as_array().into_iter().flatten() {
                if !texte_plein(&b["id"]) || !texte_plein(&b["texte"]) {
                    ep(format!("bouton du module sans id ou texte : {b}"));
                }
            }
            for r in m["resume"].as_array().into_iter().flatten() {
                if !texte_plein(&r["nom"]) || !texte_plein(&r["valeur"]) {
                    ep(format!("résumé mal rempli : {r}"));
                }
            }
        }
        if p["blocs"].as_array().is_none_or(Vec::is_empty) {
            ep("page vide (aucun bloc)".into());
        }
        // (le choix de départ : un élément de la page)
        if let Some(fo) = p.get("focus").and_then(Value::as_str)
            && !actions_de_page(p).iter().any(|(id, v)| {
                let k = |pre: &str| format!("{pre}{id}");
                fo == k("o:") && v.is_null() || fo == k("l:") || fo == k("o:") && !v.is_null()
            })
        {
            ep(format!("focus sans élément : {fo}"));
        }
        if let Some(pied) = p.get("pied")
            && pied.as_array().is_none_or(|l| l.is_empty() || l.iter().any(|b| b["type"] != "bouton"))
        {
            ep(format!("pied sans boutons : {pied}"));
        }
        for b in tous_les_blocs(p) {
            let ty = b["type"].as_str().unwrap_or_default();
            if !BLOCS.contains(&ty) {
                ep(format!("type de bloc inconnu : {b}"));
                continue;
            }
            let manque = |champs: &[&str]| champs.iter().any(|c| !texte_plein(&b[*c]));
            let mauvais = match ty {
                "titre" | "texte" | "note" => manque(&["texte"]),
                "ligne" => {
                    manque(&["texte"])
                        || b.get("bouton").is_some_and(|x| !texte_plein(&x["id"]) || !texte_plein(&x["texte"]))
                        || b.get("ton").and_then(Value::as_str).is_some_and(|t| !TONS.contains(&t))
                        || b.get("icone").and_then(Value::as_str).is_some_and(|i| !ICONES.contains(&i))
                        || b.get("couleur").and_then(Value::as_str).is_some_and(|c| !COULEURS.contains(&c))
                        || b.get("barre").is_some_and(|x| !x.as_f64().is_some_and(|v| (0.0..=100.0).contains(&v)))
                }
                "case" => manque(&["id", "texte"]) || !b["oui"].is_boolean(),
                "bouton" => {
                    manque(&["id", "texte"])
                        || b.get("style").and_then(Value::as_str).is_some_and(|s| !STYLES.contains(&s))
                        || b.get("icone").and_then(Value::as_str).is_some_and(|i| !ICONES.contains(&i))
                }
                "champ" => {
                    manque(&["id"])
                        || !b["valeur"].is_string()
                        || b.get("bouton").is_some_and(|x| {
                            !texte_plein(&x["id"])
                                || !texte_plein(&x["texte"])
                                || x.get("icone").and_then(Value::as_str).is_some_and(|i| !ICONES.contains(&i))
                        })
                }
                "groupe" => b["elements"].as_array().is_none_or(|l| {
                    l.is_empty()
                        || l.iter().any(|x| {
                            !texte_plein(&x["id"])
                                || !texte_plein(&x["titre"])
                                || x.get("icone").and_then(Value::as_str).is_some_and(|i| !ICONES.contains(&i))
                        })
                }),
                "depliables" => b["elements"].as_array().is_none_or(|l| {
                    l.is_empty()
                        || l.iter().any(|x| {
                            !texte_plein(&x["id"])
                                || !texte_plein(&x["titre"])
                                || x.get("secours").and_then(Value::as_str).is_some_and(|i| !ICONES.contains(&i))
                                || !x["icone"].as_str().is_some_and(|i| ICONES.contains(&i))
                                || x["pannes"].as_array().is_none_or(|p| {
                                    p.is_empty()
                                        || p.iter().any(|y| !texte_plein(&y["id"]) || !texte_plein(&y["texte"]))
                                })
                        })
                }),
                "bande" => {
                    manque(&["texte"]) || !texte_plein(&b["bouton"]["id"]) || !texte_plein(&b["bouton"]["texte"])
                }
                "segments" => {
                    !matches!((b["valeur"].as_u64(), b["sur"].as_u64()), (Some(v), Some(s)) if s > 0 && v <= s)
                }
                "suspects" => {
                    manque(&["titre"])
                        || b["elements"].as_array().is_none_or(|l| {
                            l.is_empty() || l.iter().any(|x| !texte_plein(&x["nom"]) || !texte_plein(&x["etat"]))
                        })
                }
                "verdict" => {
                    manque(&["titre"])
                        || !b["icone"].as_str().is_some_and(|i| ICONES.contains(&i))
                        || !b["couleur"].as_str().is_some_and(|c| COULEURS.contains(&c))
                }
                "repli" => manque(&["titre"]) || b["blocs"].as_array().is_none_or(Vec::is_empty),
                "rangs" => {
                    !matches!(b["forme"].as_str(), Some("liste" | "qr" | "kv"))
                        || b["rangs"].as_array().is_none_or(|l| l.is_empty() && !texte_plein(&b["note"]))
                        || b["rangs"].as_array().into_iter().flatten().any(|x| match b["forme"].as_str() {
                            Some("qr") => !texte_plein(&x["texte"]),
                            Some("kv") => !texte_plein(&x["cle"]) || !texte_plein(&x["valeur"]),
                            _ => !texte_plein(&x["texte"]) && !texte_plein(&x["gras"]),
                        })
                }
                "colonnes" => b["elements"].as_array().is_none_or(|l| l.len() != 2),
                "citation" | "petit" => manque(&["texte"]),
                "cartes" => {
                    manque(&["id"])
                        || b["options"].as_array().is_none_or(|o| {
                            o.is_empty() || o.iter().any(|x| !texte_plein(&x["id"]) || !texte_plein(&x["titre"]))
                        })
                }
                "grille" => b["elements"].as_array().is_none_or(|l| {
                    l.is_empty()
                        || l.iter().any(|x| {
                            !texte_plein(&x["id"])
                                || !texte_plein(&x["titre"])
                                || x.get("image").and_then(Value::as_str).is_some_and(|i| !image_ok(i))
                        })
                }),
                "jauge" => manque(&["texte"]) || !b["valeur"].as_f64().is_some_and(|v| (0.0..=100.0).contains(&v)),
                "liste" => b["elements"].as_array().is_none_or(|l| l.is_empty() || l.iter().any(|x| !texte_plein(x))),
                "etapes" => {
                    b["elements"].as_array().is_none_or(|l| l.is_empty() || l.iter().any(|x| !texte_plein(&x["texte"])))
                }
                "image" => !b["url"].as_str().is_some_and(image_ok),
                "faq" => b["elements"].as_array().is_none_or(|l| {
                    l.is_empty() || l.iter().any(|x| !texte_plein(&x["question"]) || !texte_plein(&x["reponse"]))
                }),
                _ => false,
            };
            if mauvais {
                ep(format!("bloc mal rempli : {b}"));
            }
        }
    }
    // Les textes : jamais de tiret long, ni de trou (« undefined », « null »)
    fn textes_de(v: &Value, sortie: &mut Vec<String>) {
        match v {
            Value::String(s) => sortie.push(s.clone()),
            Value::Array(a) => a.iter().for_each(|x| textes_de(x, sortie)),
            Value::Object(o) => o.values().for_each(|x| textes_de(x, sortie)),
            _ => {}
        }
    }
    let mut tous = Vec::new();
    textes_de(etat, &mut tous);
    for s in tous {
        if s.contains('\u{2013}') || s.contains('\u{2014}') {
            e(format!("tiret long : {s}"));
        }
        if s == "undefined" || s == "null" || s.contains("{}") {
            e(format!("texte à trou : {s}"));
        }
    }
    err
}

/// Chaque geste qu'on peut faire dans cet état : (id, valeur, écran).
fn actions_montrees(etat: &Value) -> Vec<(String, Value, String)> {
    let mut l = Vec::new();
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    for x in etat["accueil"]["tuiles"].as_array().into_iter().flatten() {
        l.push((s(&x["id"]), Value::Null, ecrans::ACCUEIL.to_string()));
    }
    for x in etat["accueil"]["ronds"].as_array().into_iter().flatten() {
        l.push((s(&x["id"]), Value::Null, ecrans::ACCUEIL.to_string()));
    }
    if let Some(pc) = etat.get("pc") {
        l.push((s(&pc["id"]), Value::Null, ecrans::ACCUEIL.to_string()));
    }
    for (id, p) in etat["pages"].as_object().into_iter().flatten() {
        for (a, v) in actions_de_page(p) {
            l.push((a, v, id.clone()));
        }
    }
    l
}

/// Chaque geste qu'on peut faire sur une page : (id, valeur).
fn actions_de_page(p: &Value) -> Vec<(String, Value)> {
    let mut l = Vec::new();
    {
        let mut ajoute = |a: &str, v: Value| l.push((a.to_string(), v));
        if p.get("retour").is_some() {
            ajoute(ids::RETOUR, Value::Null);
        }
        for a in p["actions"].as_array().into_iter().flatten() {
            ajoute(a["id"].as_str().unwrap_or_default(), a.get("valeur").cloned().unwrap_or(Value::Null));
        }
        for b in p["module"]["boutons"].as_array().into_iter().flatten() {
            ajoute(b["id"].as_str().unwrap_or_default(), b.get("valeur").cloned().unwrap_or(Value::Null));
        }
        for b in tous_les_blocs(p) {
            let bid = b["id"].as_str().unwrap_or_default();
            match b["type"].as_str().unwrap_or_default() {
                "ligne" => {
                    if !bid.is_empty() {
                        ajoute(bid, Value::Null);
                    }
                    if let Some(x) = b.get("bouton") {
                        ajoute(x["id"].as_str().unwrap_or_default(), x.get("valeur").cloned().unwrap_or(Value::Null));
                    }
                }
                "bouton" => ajoute(bid, b.get("valeur").cloned().unwrap_or(Value::Null)),
                "case" => ajoute(bid, json!({ "valeur": b["valeur"], "oui": !b["oui"].as_bool().unwrap_or(false) })),
                "champ" => {
                    ajoute(bid, json!("écran bleu"));
                    if let Some(x) = b.get("bouton") {
                        ajoute(x["id"].as_str().unwrap_or_default(), Value::Null);
                    }
                }
                "bande" => ajoute(b["bouton"]["id"].as_str().unwrap_or_default(), Value::Null),
                "groupe" => {
                    for x in b["elements"].as_array().into_iter().flatten() {
                        ajoute(x["id"].as_str().unwrap_or_default(), Value::Null);
                    }
                }
                "depliables" => {
                    for x in b["elements"].as_array().into_iter().flatten() {
                        for p in x["pannes"].as_array().into_iter().flatten() {
                            ajoute(p["id"].as_str().unwrap_or_default(), Value::Null);
                        }
                    }
                }
                "cartes" => {
                    for o in b["options"].as_array().into_iter().flatten() {
                        ajoute(bid, o["id"].clone());
                    }
                }
                "grille" => {
                    for x in b["elements"].as_array().into_iter().flatten() {
                        ajoute(x["id"].as_str().unwrap_or_default(), Value::Null);
                    }
                }
                _ => {}
            }
        }
    }
    l
}

/// Chaque action montrée est acceptée (jouée sur une copie de l'état : aucune action inconnue).
fn toutes_acceptees(etat: &Etat, json: &Value) -> Vec<String> {
    let mut refus = Vec::new();
    for (id, valeur, ecran) in actions_montrees(json) {
        let mut copie = etat.clone();
        let effets =
            copie.agir(cat(), &json!({ "type": "action", "id": id, "valeur": valeur, "ecran": ecran }), &mut || {
                "20261103-101500-ffff".to_string()
            });
        if effets.is_empty() || effets.iter().any(|e| matches!(e, Effet::Inconnue(_))) {
            refus.push(format!("{id} ({ecran}) : {effets:?}"));
        }
    }
    refus
}

// ------------------------------------------------------------------------------------------------ des cas du catalogue

/// Une suite de réponses du moteur (sans Windows) qui mène à un cas voulu : (panne, réponses).
fn chercher_parcours(cond: impl Fn(&Objet, &Enquete) -> bool) -> Option<(String, Vec<i64>)> {
    for p in cat().pannes() {
        let mut file: VecDeque<Vec<i64>> = VecDeque::from([Vec::new()]);
        let mut n = 0;
        while let Some(choix) = file.pop_front() {
            n += 1;
            if n > 3000 {
                break;
            }
            let e = enquete(cat(), p, &choix, None);
            if cond(p, &e) {
                return Some((t(p, "Id"), choix));
            }
            if let Some(et) = &e.etape {
                for k in 0..tableau(champ(et, "Reponses")).len() {
                    let mut suite = choix.clone();
                    suite.push(k as i64);
                    file.push_back(suite);
                }
            }
        }
    }
    None
}

fn genre(e: &Enquete) -> Option<TypeVerdict> {
    e.verdict.as_ref().map(|v| v.genre)
}

fn suspect_trouve<'a>(e: &Enquete) -> Option<&'a Objet> {
    e.verdict.as_ref().and_then(|v| v.suspect.as_deref()).and_then(|s| cat().suspect(s))
}

/// Les blocs d'une page, par type (ceux des parties repliées, des colonnes et du pied compris).
fn blocs<'a>(page: &'a Value, ty: &str) -> Vec<&'a Value> {
    tous_les_blocs(page).into_iter().filter(|b| b["type"] == ty).collect()
}

/// Tous les blocs d'une page, ceux du pied (les boutons toujours visibles) à la fin.
fn tous_les_blocs(page: &Value) -> Vec<&Value> {
    let mut l = a_plat(&page["blocs"]);
    l.extend(a_plat(&page["pied"]));
    l
}

/// Ce texte est dans la page : un bloc, ou une ligne d'une liste numérotée ou d'une carte de lignes.
fn a_texte(page: &Value, texte: &str) -> bool {
    a_plat(&page["blocs"]).into_iter().any(|b| {
        b["texte"] == texte
            || ["elements", "rangs"].iter().any(|c| b[*c].as_array().into_iter().flatten().any(|x| x["texte"] == texte))
    })
}

/// Les pannes des cartes dépliables d'une page (leurs ids), dans l'ordre.
fn pannes_depliees(page: &Value) -> Vec<String> {
    blocs(page, "depliables")
        .into_iter()
        .flat_map(|b| b["elements"].as_array().cloned().unwrap_or_default())
        .flat_map(|x| x["pannes"].as_array().cloned().unwrap_or_default())
        .map(|p| texte(p.get("id")))
        .collect()
}

/// Le badge attendu d'une réponse (Show-Verdict), écrit ici sans passer par reponse.rs.
fn badge_attendu(e: &Enquete) -> (&'static str, &'static str) {
    let s = suspect_trouve(e);
    let urgent = s.is_some_and(|s| vrai(champ(s, "Urgent")));
    let piece = s.is_some_and(|s| vrai(champ(s, "Piece")));
    match genre(e).expect("une réponse") {
        TypeVerdict::Trouve if urgent => ("Urgent", "rouge"),
        TypeVerdict::Trouve => ("Trouvé", "vert"),
        TypeVerdict::Elimination if piece => ("La piste la plus probable", "orange"),
        TypeVerdict::Elimination => ("Tout le reste est écarté", "vert"),
        TypeVerdict::Reparateur => ("À faire vérifier par un réparateur", "orange"),
        TypeVerdict::Rien => ("Aucune piste ne correspond", "orange"),
    }
}

/// La page de réponse montre la même réponse que le moteur pour ces réponses et ce que Windows a noté.
fn verifier_reponse(banc: &Banc, panne: &str, choix: &[i64], jour: Option<&Indices>) {
    let p = cat().panne(panne).expect("panne");
    let attendue = enquete(cat(), p, choix, jour);
    assert!(attendue.etape.is_none(), "{panne} {choix:?} : le moteur n'a pas fini");
    let Some(Vue::Reponse(e)) = banc.vue() else { panic!("{panne} {choix:?} : pas de réponse finale montrée") };
    assert_eq!(e.verdict, attendue.verdict, "{panne} {choix:?} : pas la même réponse que le moteur");
    assert_eq!(banc.etat.en_cours.choix, choix, "{panne} : pas les mêmes réponses");
    let page = banc.enquete();
    let (badge, couleur) = badge_attendu(&attendue);
    // (la carte de la réponse en premier)
    let v = &page["blocs"][0];
    assert_eq!(v["type"], "verdict", "{panne} {choix:?}");
    assert_eq!(v["surtitre"], badge, "{panne} {choix:?}");
    assert_eq!(v["couleur"], couleur, "{panne} {choix:?}");
    assert_eq!(v["titre"], titre_reponse(cat(), &attendue).titre, "{panne} {choix:?}");
    assert!(v["icone"] != "coche" || couleur == "vert", "{panne} : une coche sur « {badge} »");
    assert!(banc.page(ecrans::REPARATEUR).is_some(), "{panne} : pas de page pour le réparateur");
}

// ------------------------------------------------------------------------------------------------ les tests

#[test]
fn accueil_d_une_appli_neuve() {
    let mut b = Banc::nouveau(None);
    b.message(json!({ "type": "pret" }));
    b.propre();
    let tuiles: Vec<&str> =
        b.json["accueil"]["tuiles"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(tuiles, [ecrans::RACONTE, ecrans::FAMILLES]);
    assert_eq!(b.json["accueil"]["tuiles"][1]["sous"], "15 familles, 300 pannes");
    assert_eq!(b.json["accueil"]["ronds"][0]["id"], ecrans::AIDE);
    for p in [ecrans::RACONTE, ecrans::FAMILLES, ecrans::AIDE] {
        assert!(b.page(p).is_some(), "page {p}");
    }
    for p in [ecrans::ENQUETE, ecrans::FAMILLE, ecrans::REPARATEUR, "pourEtreSur"] {
        assert!(b.page(p).is_none(), "page {p} sans raison");
    }
    assert_eq!(b.json["app"]["nom"], "Mindcare");
    assert_eq!(b.json["theme"], "neon");
    assert!(verifier_kit(&b.json).is_empty());
    assert!(toutes_acceptees(&b.etat, &b.json).is_empty());
    // (l'habillage de programme.rs passe tel quel, sans écraser ce que les pages construisent)
    b.etat.habillage.insert("sons".into(), json!(true));
    b.etat.habillage.insert("app".into(), json!("non"));
    let j = b.etat.etat_kit(cat(), Some(ecrans::AIDE));
    assert_eq!(j["sons"], true);
    assert_eq!(j["app"]["nom"], "Mindcare");
    assert_eq!(j["ecran"], ecrans::AIDE);
}

#[test]
fn raconte_trouve_comme_le_moteur() {
    let mut b = Banc::nouveau(None);
    for requete in ["écran bleu", "bip", "le son grésille", "il redémarre tout seul", "mon pc", "zzzz qqqq", "wifi"]
    {
        b.action("recherche", json!(requete), ecrans::RACONTE);
        b.propre();
        let page = b.page(ecrans::RACONTE).unwrap().clone();
        assert_eq!(page["blocs"][0]["type"], "champ");
        assert_eq!(page["blocs"][0]["valeur"], requete);
        for interdit in ["parler", "installer-dictee", "installer-ia", "entree"] {
            assert!(!actions_montrees(&b.json).iter().any(|(id, _, _)| id == interdit), "{interdit} avant l'étape 6");
        }
        let trouvees: Vec<String> = chercher(cat(), requete).into_iter().map(|p| t(p, "Id")).collect();
        let lignes: Vec<String> = blocs(&page, "ligne").iter().map(|l| l["id"].as_str().unwrap().to_string()).collect();
        let attendues: Vec<String> = trouvees.iter().take(8).map(|i| format!("panne:{i}")).collect();
        assert_eq!(lignes, attendues, "« {requete} »");
        let titre = blocs(&page, "titre")[0]["texte"].as_str().unwrap().to_string();
        match trouvees.len() {
            0 => assert_eq!(titre, format!("Rien ne correspond à « {requete} »")),
            1 => assert_eq!(titre, "Une panne correspond"),
            n => assert_eq!(titre, format!("{n} pannes correspondent")),
        }
    }
    // Une panne trouvée se commence
    b.action("recherche", json!("écran bleu"), ecrans::RACONTE);
    let premiere = blocs(b.page(ecrans::RACONTE).unwrap(), "ligne")[0]["id"].as_str().unwrap().to_string();
    b.action(&premiere, Value::Null, ecrans::RACONTE);
    assert_eq!(b.ecran.as_deref(), Some(ecrans::ENQUETE));
    assert!(b.journal.iter().any(|l| l.starts_with("Enquête : ")));
    // Vide : une phrase pour expliquer, aucune ligne
    b.action("recherche", json!("   "), ecrans::RACONTE);
    let page = b.page(ecrans::RACONTE).unwrap();
    assert!(blocs(page, "ligne").is_empty());
    assert_eq!(blocs(page, "note").len(), 1);
    b.propre();
}

#[test]
fn familles_et_leurs_pannes() {
    let mut b = Banc::nouveau(None);
    let grille = &blocs(b.page(ecrans::FAMILLES).unwrap(), "groupe")[0];
    assert_eq!(grille["colonnes"], 2);
    assert_eq!(grille["elements"].as_array().unwrap().len(), cat().familles().len());
    // (chaque famille du catalogue a son icône, centrée dans sa pastille : pas celle d'une liste par défaut ; chaque
    // image des groupes a son dessin au trait)
    for x in grille["elements"].as_array().unwrap() {
        assert_ne!(x["icone"], "liste", "{} sans icône à elle", x["id"]);
    }
    for f in cat().familles() {
        for g in parcours(champ(f, "Groupes")) {
            let image = texte(champ_v(Some(g), "Icone"));
            let icone = super::familles::icone_image(&image);
            assert!(icone.is_some_and(|i| ICONES.contains(&i)), "image de groupe sans icône : {image}");
        }
    }
    assert!(blocs(b.page(ecrans::FAMILLES).unwrap(), "bande").is_empty(), "la bande sans détection du PC");
    for f in cat().familles() {
        let id = t(f, "Id");
        b.action(&format!("famille:{id}"), Value::Null, ecrans::FAMILLES);
        b.propre();
        assert_eq!(b.ecran.as_deref(), Some(ecrans::FAMILLE));
        let page = b.page(ecrans::FAMILLE).unwrap();
        assert_eq!(page["titre"], t(f, "Titre"));
        assert_eq!(page["parent"], ecrans::FAMILLES);
        let attendues: Vec<String> = cat()
            .pannes()
            .iter()
            .filter(|p| egal(&t(p, "Famille"), &id))
            .map(|p| format!("panne:{}", t(p, "Id")))
            .collect();
        // (chaque panne de la famille une fois, rangée dans la carte de son groupe)
        let mut montrees = pannes_depliees(page);
        let premiere = montrees[0].clone();
        montrees.sort();
        let mut attendues = attendues;
        attendues.sort();
        assert_eq!(montrees, attendues, "famille {id}");
        // (la première carte ouverte, sa première panne choisie ; les cartes des groupes dans l'ordre du catalogue)
        let cartes = blocs(page, "depliables")[0]["elements"].as_array().unwrap().clone();
        let groupes: Vec<String> =
            parcours(champ(f, "Groupes")).into_iter().map(|g| texte(champ_v(Some(g), "Id"))).collect();
        let cartes_ids: Vec<String> =
            cartes.iter().map(|c| texte(c.get("id"))).filter(|c| c.as_str() != "autres").collect();
        assert!(cartes_ids.iter().all(|c| groupes.contains(c)), "famille {id} : {cartes_ids:?}");
        assert!(
            cartes_ids
                .windows(2)
                .all(|w| { groupes.iter().position(|g| *g == w[0]) < groupes.iter().position(|g| *g == w[1]) })
        );
        assert_eq!(cartes[0]["ouvert"], true, "famille {id}");
        assert_eq!(cartes.iter().filter(|c| c["ouvert"] == true).count(), 1, "famille {id}");
        let total: u64 = cartes.iter().map(|c| c["nombre"].as_u64().unwrap()).sum();
        assert_eq!(total as usize, attendues.len(), "famille {id}");
        assert_eq!(page["focus"], format!("l:{premiere}"), "famille {id}");
        assert!(blocs(page, "bouton").iter().any(|x| x["id"] == ecrans::RACONTE), "famille {id}");
        assert!(toutes_acceptees(&b.etat, &b.json).is_empty());
    }
    // Une famille inconnue : retour à la liste
    let e = b.action("famille:nexistepas", Value::Null, ecrans::FAMILLES);
    assert_eq!(e, vec![Effet::Afficher { ecran: Some(ecrans::FAMILLES.into()) }]);
}

#[test]
fn premiere_etape_de_chaque_panne() {
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    for p in cat().pannes() {
        let id = t(p, "Id");
        b.commencer(&id);
        b.propre();
        let attendue = enquete(cat(), p, &[], None);
        let Some(et) = &attendue.etape else {
            verifier_reponse(&b, &id, &[], None);
            continue;
        };
        let page = b.enquete().clone();
        assert_eq!(page["titre"], t(et, "Texte"), "{id}");
        assert_eq!(page["retour"], "appli");
        assert_eq!(page["etape"], format!("Étape 1 sur {}", 1 + attendue.a_venir), "{id}");
        assert_eq!(page["forme"], "question", "{id}");
        let segments = blocs(&page, "segments")[0];
        assert_eq!(
            (segments["valeur"].as_u64(), segments["sur"].as_u64()),
            (Some(1), Some(1 + attendue.a_venir as u64))
        );
        let cartes = blocs(&page, "cartes")[0];
        assert_eq!(cartes["id"], ids::REP);
        assert_eq!(cartes["options"].as_array().unwrap().len(), tableau(champ(et, "Reponses")).len(), "{id}");
        let test = egal(&t(et, "Type"), "test");
        let sortie = if test { ids::PLUS_TARD } else { ids::ACCUEIL };
        assert!(blocs(&page, "bouton").iter().any(|x| x["id"] == sortie), "{id}");
        let montres = suspects_montres(cat(), p);
        let suspects = blocs(&page, "suspects");
        if montres.is_empty() {
            assert!(suspects.is_empty(), "{id}");
        } else {
            let l = suspects[0]["elements"].as_array().unwrap();
            assert_eq!(l.len(), montres.len(), "{id}");
            assert!(l.iter().all(|x| x["etat"] == "À vérifier"), "{id} : un suspect écarté à la première étape");
            assert!(suspects[0]["droite"].as_str().unwrap().starts_with("0 écarté"), "{id}");
            // (juste sous les réponses et leurs liens : ils suivent la hauteur de la question)
            let liste = page["blocs"].as_array().unwrap();
            let ou = liste.iter().position(|x| x["type"] == "suspects").unwrap();
            assert_eq!(liste[ou - 1]["type"], "bouton", "{id}");
        }
        assert!(blocs(&page, "bouton").iter().any(|x| x["id"] == ids::PRECEDENT));
        assert!(blocs(&page, "bouton").iter().any(|x| x["id"] == ids::NSP));
        assert!(!page["blocs"].as_array().unwrap().iter().any(|x| x["texte"] == "Tes réponses"));
        let refus = toutes_acceptees(&b.etat, &b.json);
        assert!(refus.is_empty(), "{id} : {refus:?}");
    }
}

/// Le cœur : des parcours au hasard sur toutes les pannes, en jouant les actions des pages comme une personne.
#[test]
fn parcours_au_hasard_sur_toutes_les_pannes() {
    let mut reponses = 0usize;
    let mut ecrans_vus = 0usize;
    for (n, p) in cat().pannes().iter().enumerate() {
        let id = t(p, "Id");
        for essai in 0..3u64 {
            let mut h = Hasard::nouveau((n as u64) * 7 + essai + 1);
            let jour = indices(essai, &mut h);
            let mut b = Banc::nouveau(Some(jour.clone()));
            b.commencer(&id);
            let mut attendu: Vec<i64> = Vec::new();
            let mut fini = false;
            for _ in 0..400 {
                b.propre();
                ecrans_vus += 1;
                let refus = toutes_acceptees(&b.etat, &b.json);
                assert!(refus.is_empty(), "{id} {attendu:?} : actions refusées {refus:?}");
                match b.vue() {
                    None => {
                        // (Étape précédente à la première question : la famille de la panne, on recommence)
                        assert_eq!(b.ecran.as_deref(), Some(ecrans::FAMILLE), "{id} : sans enquête hors de la famille");
                        b.action(&format!("panne:{id}"), Value::Null, ecrans::FAMILLE);
                        attendu.clear();
                    }
                    Some(Vue::Attente) => panic!("{id} : l'enquête attend Windows alors qu'il a répondu"),
                    Some(Vue::Reponse(_)) => {
                        verifier_reponse(&b, &id, &attendu, Some(&jour.moteur()));
                        reponses += 1;
                        fini = true;
                        match h.sous(10) {
                            0..=1 => {
                                b.action(ids::PRECEDENT, Value::Null, ecrans::ENQUETE);
                                if attendu.pop().is_none() {
                                    assert!(b.vue().is_none());
                                }
                            }
                            2 => {
                                // Revoir la dernière enquête depuis l'accueil : la même réponse
                                b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
                                assert!(b.vue().is_none());
                                assert_eq!(b.json["accueil"]["tuiles"][2]["id"], super::accueil::DERNIERE);
                                b.action(super::accueil::DERNIERE, Value::Null, ecrans::ACCUEIL);
                                verifier_reponse(&b, &id, &attendu, Some(&jour.moteur()));
                                break;
                            }
                            _ => break,
                        }
                    }
                    Some(Vue::Etape(e)) => {
                        let nb = tableau(champ(e.etape.as_ref().unwrap(), "Reponses")).len() as u64;
                        match h.sous(100) {
                            0..=4 => {
                                b.action(ids::PRECEDENT, Value::Null, ecrans::ENQUETE);
                                if attendu.pop().is_none() {
                                    assert!(b.vue().is_none(), "{id} : Étape précédente à la première question");
                                }
                            }
                            5..=7 => {
                                b.action(ids::RETOUR, Value::Null, ecrans::ENQUETE);
                                attendu.pop();
                            }
                            8..=11 if !attendu.is_empty() => {
                                let k = h.sous(attendu.len() as u64) as usize;
                                b.action(&format!("changer:{k}"), Value::Null, ecrans::ENQUETE);
                                attendu.truncate(k);
                            }
                            12..=14 if !attendu.is_empty() => {
                                // Plus tard (ou l'accueil), puis reprendre : la même enquête, au même endroit
                                let gardee = b.etat.historique.first().is_some_and(|d| {
                                    egal(&texte(champ(d, "Id")), &b.etat.en_cours.diag_id) && !vrai(champ(d, "Fini"))
                                });
                                b.action(ids::PLUS_TARD, Value::Null, ecrans::ENQUETE);
                                assert!(b.vue().is_none());
                                if gardee {
                                    assert_eq!(b.json["accueil"]["tuiles"][2]["id"], super::accueil::REPRENDRE);
                                    b.action(super::accueil::REPRENDRE, Value::Null, ecrans::ACCUEIL);
                                    assert_eq!(b.etat.en_cours.choix, attendu, "{id} : reprise");
                                } else {
                                    b.action(&format!("panne:{id}"), Value::Null, ecrans::ACCUEIL);
                                    attendu.clear();
                                }
                            }
                            15..=17 => {
                                // Cocher un geste (une manipulation) ne change pas l'enquête
                                b.action(ids::GESTE, json!({ "valeur": "0", "oui": true }), ecrans::ENQUETE);
                            }
                            18..=30 => {
                                b.action(ids::NSP, Value::Null, ecrans::ENQUETE);
                                attendu.push(-1);
                            }
                            _ => {
                                let k = h.sous(nb) as i64;
                                b.action(ids::REP, json!(k.to_string()), ecrans::ENQUETE);
                                attendu.push(k);
                            }
                        }
                        if b.vue().is_some() {
                            assert_eq!(b.etat.en_cours.choix, attendu, "{id} : les réponses ne suivent pas");
                        }
                    }
                }
            }
            assert!(fini, "{id} (essai {essai}) : aucune réponse finale au bout de 400 gestes");
            b.propre();
        }
    }
    assert!(reponses >= 900, "{reponses} réponses finales seulement");
    eprintln!("{ecrans_vus} écrans vérifiés, {reponses} réponses finales comparées au moteur");
}

/// Les réponses données une à une par les pages finissent comme le moteur, pour chaque panne (sans Windows, puis
/// avec tout vu sur ce PC), en largeur sur les premières suites de réponses.
#[test]
fn reponses_comme_le_moteur_en_largeur() {
    let mut n = 0;
    for p in cat().pannes() {
        let id = t(p, "Id");
        let mut b = Banc::nouveau(Some(IndicesGardes::default()));
        let mut file: VecDeque<Vec<i64>> = VecDeque::from([Vec::new()]);
        let mut essais = 0;
        while let Some(choix) = file.pop_front() {
            essais += 1;
            if essais > 25 {
                break;
            }
            let e = enquete(cat(), p, &choix, None);
            match &e.etape {
                Some(et) => {
                    for k in -1..tableau(champ(et, "Reponses")).len() as i64 {
                        let mut suite = choix.clone();
                        suite.push(k);
                        file.push_back(suite);
                    }
                }
                None => {
                    b.commencer(&id);
                    b.repondre(&choix);
                    verifier_reponse(&b, &id, &choix, None);
                    b.propre();
                    n += 1;
                }
            }
        }
    }
    eprintln!("{n} réponses finales comparées au moteur");
    assert!(n > 2000, "{n} réponses seulement");
}

#[test]
fn etape_a_faire_gestes_et_securite() {
    let (panne, choix) = chercher_parcours(|_, e| {
        e.etape.as_ref().is_some_and(|et| egal(&t(et, "Type"), "test") && gestes_materiel(&textes(et, "Gestes")))
    })
    .expect("une manipulation sur le matériel");
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    b.commencer(&panne);
    b.repondre(&choix);
    b.propre();
    let p = cat().panne(&panne).unwrap();
    let e = enquete(cat(), p, &choix, None);
    let et = e.etape.as_ref().unwrap();
    let page = b.enquete().clone();
    let gestes = textes(et, "Gestes");
    let cases = blocs(&page, "case");
    assert_eq!(cases.len(), gestes.len());
    assert_eq!(cases[0]["texte"], format!("1. {}", gestes[0]));
    assert!(cases.iter().all(|c| c["oui"] == false));
    let securite = texte(cat().json().get("Securite").and_then(|s| s.get(0)));
    assert!(a_texte(&page, &securite), "{panne} : la consigne de sécurité manque");
    assert_eq!(page["forme"], "question");
    assert!(blocs(&page, "bouton").iter().any(|x| x["id"] == ids::PLUS_TARD && x["texte"] == "Je le ferai plus tard"));
    assert!(page["sous"].as_str().unwrap().starts_with("Une manipulation à faire"));
    assert!(a_texte(&page, "Coche chaque geste une fois fait"));
    assert!(a_texte(&page, "Ensuite, dis à Mindcare ce que ça a donné"));
    // Cocher, décocher
    b.action(ids::GESTE, json!({ "valeur": "1", "oui": true }), ecrans::ENQUETE);
    assert_eq!(blocs(b.enquete(), "case")[1]["oui"], true);
    assert_eq!(blocs(b.enquete(), "case")[0]["oui"], false);
    b.action(ids::GESTE, json!({ "valeur": "1", "oui": false }), ecrans::ENQUETE);
    assert_eq!(blocs(b.enquete(), "case")[1]["oui"], false);
    // Plus tard : le journal, la bulle, l'accueil
    b.action(ids::PLUS_TARD, Value::Null, ecrans::ENQUETE);
    assert!(b.journal.contains(&"Test remis à plus tard".to_string()));
    assert!(b.messages.contains(&(PLUS_TARD_MESSAGE.to_string(), false)));
    assert_eq!(b.ecran.as_deref(), Some(ecrans::ACCUEIL));
    assert!(b.page(ecrans::ENQUETE).is_none());
    b.propre();

    // Une question (pas une manipulation) : pas de case, pas de sécurité, « Accueil »
    let (panne, choix) =
        chercher_parcours(|_, e| e.etape.as_ref().is_some_and(|et| !egal(&t(et, "Type"), "test"))).unwrap();
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    b.commencer(&panne);
    b.repondre(&choix);
    let page = b.enquete();
    assert!(blocs(page, "case").is_empty());
    assert!(!a_texte(page, &securite));
    assert!(blocs(page, "bouton").iter().any(|x| x["id"] == ids::ACCUEIL));
    assert!(page.get("sous").is_none());
}

#[test]
fn commande_a_copier() {
    // (une commande sans « Le faire pour moi » : ses lignes sont montrées dépliées, comme avant ; avec le bouton, elles
    // sont repliées dessous, voir tests/reparer.rs)
    let (panne, choix) = chercher_parcours(|_, e| {
        e.etape.as_ref().is_some_and(|et| champ(et, "Commande").is_some() && champ(et, "Reparer").is_none())
    })
    .expect("une étape avec une commande");
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    b.commencer(&panne);
    b.repondre(&choix);
    b.propre();
    let e = enquete(cat(), cat().panne(&panne).unwrap(), &choix, None);
    let commande = champ(e.etape.as_ref().unwrap(), "Commande").unwrap();
    let lignes: Vec<String> = parcours(champ_v(Some(commande), "Lignes")).into_iter().map(|x| texte(Some(x))).collect();
    let montrees: Vec<&Value> =
        blocs(b.enquete(), "ligne").into_iter().filter(|l| l["bouton"]["id"] == ids::COMMANDE).collect();
    assert_eq!(montrees.len(), lignes.len());
    assert_eq!(montrees[0]["texte"], lignes[0]);
    assert_eq!(montrees[0]["bouton"]["texte"], "Copier");
    if vrai(champ_v(Some(commande), "Admin")) {
        assert_eq!(montrees[0]["note"], "À taper dans le Terminal (administrateur)");
    }
    // (juste sous le geste qui dit de la copier)
    let page = b.enquete().clone();
    let liste = page["blocs"].as_array().unwrap();
    let ou = liste.iter().position(|x| x["bouton"]["id"] == ids::COMMANDE).unwrap();
    assert!(liste[ou - 1]["texte"].as_str().unwrap().contains("Copier"));
    let effets = b.action(ids::COMMANDE, json!("0"), ecrans::ENQUETE);
    assert_eq!(effets, vec![Effet::CopierCommande { numero: 0, texte: lignes[0].clone() }]);
    let bouton = blocs(b.enquete(), "ligne").into_iter().find(|l| l["bouton"]["id"] == ids::COMMANDE).unwrap();
    assert_eq!(bouton["bouton"]["texte"], "Copiée");
    // Ratée : « Copie impossible », et une ligne au journal
    let effets = b.etat.commande_copiee(0, Err("presse-papiers pris".into()));
    assert!(effets.contains(&Effet::Journal("Commande non copiée : presse-papiers pris".into())));
    b.executer(effets);
    let bouton = blocs(b.enquete(), "ligne").into_iter().find(|l| l["bouton"]["id"] == ids::COMMANDE).unwrap();
    assert_eq!(bouton["bouton"]["texte"], "Copie impossible");
    // Un numéro qui n'existe pas : rien
    assert_eq!(b.action(ids::COMMANDE, json!("99"), ecrans::ENQUETE), vec![Effet::Afficher { ecran: None }]);
}

/// Un cas de réponse finale cherché dans le catalogue.
type Condition = Box<dyn Fn(&Enquete) -> bool>;

#[test]
fn reponses_finales_de_chaque_genre() {
    let piece = |e: &Enquete| suspect_trouve(e).is_some_and(|s| vrai(champ(s, "Piece")));
    let urgent = |e: &Enquete| suspect_trouve(e).is_some_and(|s| vrai(champ(s, "Urgent")));
    let cas: Vec<(&str, Condition)> = vec![
        ("pièce trouvée", Box::new(move |e| genre(e) == Some(TypeVerdict::Trouve) && piece(e) && !urgent(e))),
        ("pièce supposée", Box::new(move |e| genre(e) == Some(TypeVerdict::Elimination) && piece(e))),
        ("urgent", Box::new(move |e| genre(e) == Some(TypeVerdict::Trouve) && urgent(e))),
        ("tout le reste écarté", Box::new(move |e| genre(e) == Some(TypeVerdict::Elimination) && !piece(e))),
        ("réparateur", Box::new(|e| genre(e) == Some(TypeVerdict::Reparateur))),
    ];
    let securite: Vec<String> =
        parcours(cat().json().get("Securite")).into_iter().map(|x| texte(Some(x))).collect::<Vec<_>>();
    for (nom, cond) in cas {
        let (panne, choix) = chercher_parcours(|_, e| cond(e)).unwrap_or_else(|| panic!("aucun cas « {nom} »"));
        let mut b = Banc::nouveau(Some(IndicesGardes::default()));
        b.commencer(&panne);
        b.repondre(&choix);
        b.propre();
        verifier_reponse(&b, &panne, &choix, None);
        let e = enquete(cat(), cat().panne(&panne).unwrap(), &choix, None);
        let page = b.enquete().clone();
        assert_eq!(page["titre"], t(cat().panne(&panne).unwrap(), "Titre"));
        assert_eq!(page["sous"], "La réponse de Mindcare");
        let boutons: Vec<&str> = blocs(&page, "bouton").iter().map(|a| a["id"].as_str().unwrap()).collect();
        assert_eq!(boutons, [ids::ACCUEIL, ids::REPARATEUR, ids::PRECEDENT], "{nom}");
        assert_eq!(page["focus"], format!("o:{}", ids::ACCUEIL), "{nom} : Retour à l'accueil choisi à l'ouverture");
        assert!(page.get("actions").is_none(), "{nom}");
        // (comment on le sait : replié, avec son résumé)
        let repli = blocs(&page, "repli");
        assert_eq!(repli.len(), 1, "{nom}");
        assert_eq!(repli[0]["titre"], "Comment Mindcare le sait", "{nom}");
        assert!(texte_plein(&repli[0]["resume"]), "{nom}");
        assert!(
            !page["blocs"].as_array().unwrap().iter().any(|x| x["type"] == "liste"),
            "{nom} : les preuves dépliées"
        );
        assert!(a_texte(&page, "Ce qu'il faut faire"), "{nom}");
        assert!(a_texte(&page, &securite.join(" ")), "{nom} : la sécurité");
        assert_eq!(page["blocs"].as_array().unwrap().last().unwrap()["type"], "petit", "{nom} : la sécurité en bas");
        if !e.preuves.is_empty() {
            assert_eq!(blocs(&page, "liste")[0]["elements"], json!(e.preuves), "{nom}");
        }
        let garantie = a_texte(&page, super::reponse::PIECE_GARANTIE);
        let supposee = a_texte(&page, super::reponse::PIECE_SUPPOSEE);
        match nom {
            "pièce trouvée" => {
                assert!(garantie && !supposee, "{nom}");
                let j = std::fs::read_to_string(b.donnees.journal().fichier()).unwrap_or_default();
                assert!(j.contains("Avant d'acheter : pièce confirmée"), "{nom} : journal");
            }
            "pièce supposée" => {
                assert!(supposee && !garantie, "{nom}");
                assert!(page["blocs"][0]["titre"].as_str().unwrap().starts_with("À vérifier : "));
            }
            _ => assert!(!garantie && !supposee, "{nom}"),
        }
        let s = suspect_trouve(&e);
        if let Some(s) = s {
            let a_faire = textes(s, "AFaire");
            let elements = blocs(&page, "etapes")[0]["elements"].as_array().unwrap().clone();
            let numerotes: Vec<&Value> = elements.iter().filter(|x| x.get("icone").is_none()).collect();
            assert_eq!(numerotes.len(), a_faire.len(), "{nom}");
            for (x, a) in numerotes.iter().zip(&a_faire) {
                assert_eq!(x["texte"], a.as_str(), "{nom}");
            }
            if !t(s, "Pro").is_empty() {
                let conseil = elements.iter().find(|x| x["texte"] == t(s, "Pro").as_str()).expect("le conseil");
                assert_eq!(conseil["titre"], "Ensuite, chez un réparateur :", "{nom}");
                assert_eq!(conseil["icone"], "cle", "{nom}");
            }
        } else {
            let v = e.verdict.as_ref().unwrap();
            let lignes: Vec<&Value> = blocs(&page, "ligne");
            for r in &v.restants {
                assert!(lignes.iter().any(|l| l["texte"] == t(cat().suspect(r).unwrap(), "Titre")), "{nom} : {r}");
            }
        }
        // Les écartés (sans les dangers, ni ce qui n'a rien à voir avec ce PC), en minuscule, une seule fois chacun
        let ecartes: Vec<&Value> = blocs(&page, "texte")
            .into_iter()
            .filter(|x| x["texte"].as_str().unwrap().starts_with("Écartés : "))
            .collect();
        let montrer = e.ecartes_montres(cat()).any(|x| !cat().suspect(&x.id).is_some_and(|s| vrai(champ(s, "Urgent"))));
        assert_eq!(ecartes.len(), usize::from(montrer), "{nom}");
        assert!(toutes_acceptees(&b.etat, &b.json).is_empty());
        // Changer ma dernière réponse : l'étape d'avant
        if !choix.is_empty() {
            b.action(ids::PRECEDENT, Value::Null, ecrans::ENQUETE);
            assert!(matches!(b.vue(), Some(Vue::Etape(_))), "{nom}");
            assert_eq!(b.etat.en_cours.choix, choix[..choix.len() - 1]);
        }
    }
}

#[test]
fn reprendre_une_enquete_et_revoir_la_derniere() {
    let (panne, choix) = chercher_parcours(|_, e| genre(e) == Some(TypeVerdict::Trouve) && e.faits.len() >= 3).unwrap();
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    b.commencer(&panne);
    b.repondre(&choix[..2]);
    b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
    assert!(b.page(ecrans::ENQUETE).is_none());
    let tuile = b.json["accueil"]["tuiles"][2].clone();
    assert_eq!(tuile["id"], super::accueil::REPRENDRE);
    assert_eq!(tuile["alerte"], true);
    assert_eq!(tuile["nom"], "Reprendre l'enquête");
    assert_eq!(tuile["sous"], t(cat().panne(&panne).unwrap(), "Titre"));
    b.action(super::accueil::REPRENDRE, Value::Null, ecrans::ACCUEIL);
    assert_eq!(b.ecran.as_deref(), Some(ecrans::ENQUETE));
    assert_eq!(b.etat.en_cours.choix, choix[..2]);
    assert!(b.journal.iter().any(|l| l.starts_with("Reprise de l'enquête ")));
    assert_eq!(blocs(b.enquete(), "ligne").iter().filter(|l| l["valeur"] == "Changer").count(), 2);
    b.repondre(&choix[2..]);
    verifier_reponse(&b, &panne, &choix, None);
    let titre = b.enquete()["blocs"][0]["titre"].clone();
    // Finie : « Dernière enquête », la même réponse
    b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
    let tuile = b.json["accueil"]["tuiles"][2].clone();
    assert_eq!(tuile["id"], super::accueil::DERNIERE);
    assert_eq!(tuile["sous"], t(cat().panne(&panne).unwrap(), "Titre"));
    b.action(super::accueil::DERNIERE, Value::Null, ecrans::ACCUEIL);
    verifier_reponse(&b, &panne, &choix, None);
    assert_eq!(b.enquete()["blocs"][0]["titre"], titre);
    // (revoir ne déplace ni ne change l'enquête gardée)
    assert_eq!(b.etat.historique.len(), 1);
    b.propre();
    // Changer une réponse au milieu : on revient à cette question
    b.action("changer:1", Value::Null, ecrans::ENQUETE);
    assert_eq!(b.etat.en_cours.choix, choix[..1]);
    b.propre();
}

#[test]
fn precedent_a_la_premiere_question_et_retour() {
    let p = &cat().pannes()[0];
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    b.commencer(&t(p, "Id"));
    b.action(ids::RETOUR, Value::Null, ecrans::ENQUETE);
    assert_eq!(b.ecran.as_deref(), Some(ecrans::FAMILLE));
    assert_eq!(b.page(ecrans::FAMILLE).unwrap()["titre"], t(cat().famille(&t(p, "Famille")).unwrap(), "Titre"));
    assert!(b.vue().is_none());
    // Une réponse sans enquête (un écran resté ouvert) : rien ne casse
    assert_eq!(b.action(ids::REP, json!("0"), ecrans::ENQUETE), vec![Effet::Afficher { ecran: None }]);
    assert_eq!(b.action(ids::REPARATEUR, Value::Null, ecrans::ENQUETE), vec![Effet::Afficher { ecran: None }]);
    assert_eq!(b.action(ids::BUREAU, Value::Null, ecrans::REPARATEUR), vec![Effet::Afficher { ecran: None }]);
    // Retour ailleurs que dans l'enquête : rien
    assert_eq!(b.action(ids::RETOUR, Value::Null, ecrans::AIDE), vec![Effet::Afficher { ecran: None }]);
    b.propre();
}

#[test]
fn rapport_pour_le_reparateur() {
    let (panne, choix) = chercher_parcours(|_, e| genre(e) == Some(TypeVerdict::Reparateur)).unwrap();
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    b.commencer(&panne);
    b.repondre(&choix);
    b.action(ids::REPARATEUR, Value::Null, ecrans::ENQUETE);
    assert_eq!(b.ecran.as_deref(), Some(ecrans::REPARATEUR));
    let page = b.page(ecrans::REPARATEUR).unwrap().clone();
    assert_eq!(page["titre"], textes_rapport::TITRE);
    assert_eq!(page["sous"], super::reparateur::titres::SOUS);
    assert_eq!(page["parent"], ecrans::ENQUETE);
    let boutons = page["actions"].as_array().unwrap();
    assert_eq!(boutons[0], json!({ "id": ids::COPIER, "texte": "Copier", "style": "second" }));
    assert_eq!(boutons[1], json!({ "id": ids::BUREAU, "texte": "Enregistrer sur le Bureau", "style": "principal" }));
    assert_eq!(page["focus"], format!("o:{}", ids::BUREAU));
    // Le rapport est montré dans la page, ligne par ligne, avant même d'être enregistré
    let texte_rapport = b.rapport().unwrap();
    assert_eq!(b.etat.rapport.as_deref(), Some(texte_rapport.as_str()));
    assert_eq!(page["blocs"][0]["type"], "verdict");
    assert_eq!(page["blocs"][0]["petit"], format!("Panne : {}", t(cat().panne(&panne).unwrap(), "Titre")));
    assert_rapport_montre(&page, &texte_rapport);
    // Sur le Bureau (ici, le dossier d'essai) : dit dans la page seulement, pas en bulle (elle recouvrait le titre)
    let messages = b.messages.len();
    b.action(ids::BUREAU, Value::Null, ecrans::REPARATEUR);
    let nom = "Mindcare, diagnostic du 3-11-2026 10h15.txt";
    assert!(b.dossier.join(nom).is_file(), "le rapport n'est pas écrit");
    let attendu = textes_rapport::enregistre(nom);
    assert_eq!(b.messages.len(), messages, "une bulle en plus du texte de la page");
    assert!(a_texte(b.page(ecrans::REPARATEUR).unwrap(), &attendu));
    assert_eq!(b.page(ecrans::REPARATEUR).unwrap()["blocs"][0]["texte"], attendu.as_str(), "le résultat en haut");
    let rapport = std::fs::read_to_string(b.dossier.join(nom)).unwrap();
    let rapport = rapport.trim_start_matches('\u{feff}');
    assert!(rapport.contains(&format!("Panne : {}", t(cat().panne(&panne).unwrap(), "Titre"))));
    // (la page montre le texte du fichier, tel quel)
    assert_eq!(b.etat.rapport.as_deref(), Some(rapport));
    assert_rapport_montre(b.page(ecrans::REPARATEUR).unwrap(), rapport);
    // Copié
    b.action(ids::COPIER, Value::Null, ecrans::REPARATEUR);
    assert_eq!(b.page(ecrans::REPARATEUR).unwrap()["actions"][0]["texte"], "Copié");
    assert!(a_texte(b.page(ecrans::REPARATEUR).unwrap(), textes_rapport::COPIE));
    assert_eq!(b.messages.len(), messages, "une bulle en plus du texte de la page");
    // Raté
    let effets = b.etat.rapport_enregistre(Err("accès refusé".into()));
    assert!(effets.contains(&Effet::Journal("Rapport non enregistré : accès refusé".into())));
    b.executer(effets);
    assert!(a_texte(b.page(ecrans::REPARATEUR).unwrap(), textes_rapport::ECHEC_ENREGISTRER));
    let effets = b.etat.rapport_copie(Err("pris".into()));
    assert!(effets.contains(&Effet::Journal("Rapport non copié : pris".into())));
    b.executer(effets);
    assert!(a_texte(b.page(ecrans::REPARATEUR).unwrap(), textes_rapport::ECHEC_COPIE));
    b.propre();
    // Une nouvelle enquête oublie le rapport copié
    b.commencer(&panne);
    assert!(!b.etat.copie && b.etat.info_rapport.is_none() && b.etat.rapport.is_none());
}

/// Chaque ligne du rapport se retrouve dans la page, et seulement elles (sans le titre du texte ni les titres de ses
/// parties, que la page nomme à sa façon ; sans les « - » des listes ni le retrait des réponses) : la page est relue
/// ligne à ligne comme le texte les écrit (la panne, la réponse, la date ; « Nom (raison) » d'un écarté ; « Il reste :
/// a, b » ; « Clé : valeur » du PC ; une question puis sa réponse), puis comparée au texte. Les questions et leurs
/// réponses gardent leur ordre.
fn assert_rapport_montre(page: &Value, rapport: &str) {
    let mut montres: Vec<String> = Vec::new();
    let mut reponses: Vec<String> = Vec::new();
    for b in a_plat(&page["blocs"]) {
        match b["type"].as_str() {
            Some("verdict") => {
                montres.push(texte(b.get("petit")));
                montres.push(format!("Réponse : {}", texte(b.get("titre"))));
                montres.push(format!("Date : {}", texte(b.get("droite")).trim_start_matches("Enquête du ")));
            }
            Some("rangs") => {
                let rangs = b["rangs"].as_array().unwrap();
                match (b["forme"].as_str(), b["titre"].as_str()) {
                    (Some("qr"), _) => {
                        for r in rangs {
                            reponses.push(texte(r.get("texte")));
                            reponses.push(texte(r.get("valeur")));
                        }
                    }
                    (Some("kv"), _) => montres
                        .extend(rangs.iter().map(|r| format!("{} : {}", texte(r.get("cle")), texte(r.get("valeur"))))),
                    (_, Some(super::reparateur::titres::RESTE)) => {
                        let noms: Vec<String> = rangs
                            .iter()
                            .map(|r| texte(r.get(if r.get("gras").is_some() { "gras" } else { "texte" })))
                            .collect();
                        montres.push(format!("Il reste : {}", noms.join(", ")));
                    }
                    _ => {
                        for r in rangs {
                            montres.push(match (r.get("gras"), texte(r.get("texte"))) {
                                (Some(g), t) if t.is_empty() => texte(Some(g)),
                                (Some(g), t) => format!("{} ({t})", texte(Some(g))),
                                (None, t) => t,
                            });
                        }
                    }
                }
            }
            // (« Fait avec Mindcare ... »)
            Some("petit") if texte(b.get("texte")).starts_with("Fait avec") => montres.push(texte(b.get("texte"))),
            _ => {}
        }
    }
    let mut attendus: Vec<String> = Vec::new();
    let mut reponses_attendues: Vec<String> = Vec::new();
    let mut dans_reponses = false;
    for l in rapport.lines().skip(1) {
        if l.trim().is_empty() {
            dans_reponses = false;
            continue;
        }
        if l.trim_end().ends_with(" :") {
            dans_reponses = l.starts_with("Les réponses");
            continue;
        }
        let x = l.strip_prefix("- ").unwrap_or(l).trim().to_string();
        if dans_reponses { reponses_attendues.push(x) } else { attendus.push(x) }
    }
    assert_eq!(reponses, reponses_attendues, "les questions et les réponses montrées ne sont pas celles du fichier");
    montres.retain(|m| !m.is_empty() && m != "Date : " && m != "Réponse : ");
    montres.sort();
    attendus.sort();
    assert_eq!(montres, attendus, "le rapport montré n'est pas celui du fichier");
}

/// Contre-épreuve : un rapport pas encore prêt montre ce qu'il contiendra, sans planter ; un texte vide aussi.
#[test]
fn reparateur_sans_le_texte() {
    let (panne, choix) = chercher_parcours(|_, e| genre(e) == Some(TypeVerdict::Reparateur)).unwrap();
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    b.commencer(&panne);
    b.repondre(&choix);
    for r in [None, Some(String::new())] {
        b.etat.rapport = r;
        let p = super::reparateur::page(&b.etat, cat());
        assert!(a_texte(&p, "Ce qu'il contient"), "{p}");
    }
    // Et l'inverse : une ligne perdue en route se voit
    let t = b.rapport().unwrap();
    b.etat.rapport = Some(t.clone());
    let p = super::reparateur::page(&b.etat, cat());
    let faux = t.replace("Les réponses :", "Les réponses :\r\n- en trop");
    let r = std::panic::catch_unwind(|| assert_rapport_montre(&p, &faux));
    assert!(r.is_err(), "une ligne du fichier absente de la page doit se voir");
    b.propre();
}

/// Un portable : le bloc d'alimentation (écarté d'office, il ne concerne pas ce type de PC) n'apparaît nulle part, ni
/// dans les suspects de l'étape (ni barré ni compté), ni dans la réponse, ni dans le rapport ou sa page ; une piste
/// écartée par une vraie réponse, elle, apparaît toujours.
#[test]
fn rien_de_hors_sujet_pour_un_portable() {
    let alim = super::outils::titre_suspect(cat(), "alimentation");
    let portable =
        |e: &Enquete| e.notes.iter().any(|n| n == "portable") && e.ecartes.iter().any(|x| x.id == "alimentation");
    // Pendant l'enquête : la carte des suspects
    let (panne, choix) = chercher_parcours(|p, e| {
        textes(p, "Suspects").iter().any(|s| s == "alimentation") && portable(e) && e.etape.is_some()
    })
    .expect("une enquête en cours sur un portable");
    let mut b = Banc::nouveau(None);
    b.commencer(&panne);
    b.repondre(&choix);
    let Some(Vue::Etape(e)) = b.vue() else { panic!("{panne} {choix:?} : pas d'étape montrée") };
    assert!(e.hors_sujet(cat(), "alimentation"), "{panne} {choix:?}");
    let carte = blocs(b.enquete(), "suspects");
    let carte = carte.first().expect("la carte des suspects");
    let elements = carte["elements"].as_array().unwrap();
    assert!(elements.iter().all(|x| x["nom"] != alim.as_str()), "{panne} : {carte}");
    let attendus: Vec<String> =
        suspects_montres(cat(), cat().panne(&panne).unwrap()).into_iter().filter(|s| s != "alimentation").collect();
    assert_eq!(elements.len(), attendus.len(), "{panne} : {carte}");
    let barres = elements.iter().filter(|x| x["etat"] == super::enquete::textes::ECARTE).count();
    let droite = texte(carte.get("droite"));
    let compte = if barres <= 1 { format!("{barres} écarté,") } else { format!("{barres} écartés,") };
    assert!(droite.starts_with(&compte), "{panne} : {droite} au lieu de {compte}");

    // La réponse finale, avec une vraie piste écartée à côté
    let (panne, choix) = chercher_parcours(|p, e| {
        textes(p, "Suspects").iter().any(|s| s == "alimentation")
            && portable(e)
            && e.etape.is_none()
            && e.verdict.is_some()
            && e.ecartes_montres(cat()).any(|x| !x.raison.is_empty() && !super::outils::urgent(cat(), &x.id))
    })
    .expect("une réponse finale sur un portable avec une vraie piste écartée");
    let mut b = Banc::nouveau(None);
    b.commencer(&panne);
    b.repondre(&choix);
    let Some(Vue::Reponse(e)) = b.vue() else { panic!("{panne} {choix:?} : pas de réponse finale montrée") };
    assert!(e.hors_sujet(cat(), "alimentation"), "{panne} {choix:?}");
    let vraie = e
        .ecartes_montres(cat())
        .find(|x| !x.raison.is_empty() && !super::outils::urgent(cat(), &x.id))
        .cloned()
        .expect("une vraie piste écartée");
    let vraie_nom = super::outils::titre_suspect(cat(), &vraie.id);
    let page = b.enquete().clone();
    let ecartes: Vec<String> = blocs(&page, "texte")
        .into_iter()
        .map(|x| texte(x.get("texte")))
        .filter(|x| x.starts_with("Écartés : "))
        .collect();
    assert_eq!(ecartes.len(), 1, "{panne} {choix:?}");
    assert!(!ecartes[0].contains(&alim.to_lowercase()), "{panne} : {}", ecartes[0]);
    assert!(ecartes[0].contains(&vraie_nom.to_lowercase()), "{panne} : {}", ecartes[0]);
    let resume = blocs(&page, "repli").iter().map(|r| texte(r.get("resume"))).collect::<Vec<_>>().join(" ");
    let n = ecartes[0].matches(", ").count() + 1;
    let piste = if n == 1 { "1 piste écartée".to_string() } else { format!("{n} pistes écartées") };
    assert!(resume.contains(&piste), "{panne} : {resume} au lieu de {piste}");
    // Le rapport et sa page
    let rapport = b.rapport().unwrap();
    assert!(!rapport.contains(HORS_SUJET), "{rapport}");
    assert!(!rapport.lines().any(|l| l.starts_with(&format!("- {alim}"))), "{rapport}");
    assert!(rapport.contains(&format!("- {vraie_nom} ({})", vraie.raison)), "{rapport}");
    b.action(ids::REPARATEUR, Value::Null, ecrans::ENQUETE);
    let page = b.page(ecrans::REPARATEUR).unwrap().clone();
    assert_rapport_montre(&page, &rapport);
    let gras: Vec<String> = blocs(&page, "rangs")
        .into_iter()
        .flat_map(|r| r["rangs"].as_array().cloned().unwrap_or_default())
        .map(|r| texte(r.get("gras")))
        .collect();
    assert!(!gras.contains(&alim), "{panne} : {gras:?}");
    assert!(gras.contains(&vraie_nom), "{panne} : {gras:?}");
    // Un rapport écrit par une version d'avant (avec la ligne du bloc d'alimentation) : la page ne la montre pas
    let ancien = rapport.replacen("Écartés :\r\n", &format!("Écartés :\r\n- {alim} ({HORS_SUJET})\r\n"), 1);
    assert_ne!(ancien, rapport, "le rapport n'a pas de partie « Écartés »");
    b.etat.rapport = Some(ancien);
    let p = super::reparateur::page(&b.etat, cat());
    assert!(!p.to_string().contains(HORS_SUJET), "{p}");
    assert!(p.to_string().contains(&vraie_nom), "{p}");
    b.propre();
}

/// Sur le PC en panne, l'enquête attend ce que Windows a noté (Wait-Journaux), puis reprend.
#[test]
fn windows_attendu_sur_le_pc_en_panne() {
    let cepc = |e: &Enquete| e.notes.iter().any(|n| egal(n, "cepc"));
    let vus = mindcare_moteur::identifiables::tous_vus();
    let (panne, choix) = chercher_parcours(|p, e| {
        // (sur ce PC-là, et des étapes automatiques s'en servent ensuite)
        cepc(e) && e.etape.is_some() && !enquete(cat(), p, &e.choix, Some(&vus)).auto.is_empty()
    })
    .expect("une enquête faite sur ce PC");
    // La dernière réponse est celle qui dit « sur ce PC-là »
    let p = cat().panne(&panne).unwrap();
    let mut avant = choix.clone();
    while cepc(&enquete(cat(), p, &avant, None)) {
        avant.pop();
    }
    let mut b = Banc::nouveau(None);
    b.commencer(&panne);
    b.repondre(&avant);
    let effets = b.repondre_un(choix[avant.len()]);
    assert!(effets.contains(&Effet::LireWindows), "{effets:?}");
    assert!(!effets.contains(&Effet::EnregistrerEnquete), "rien n'est enregistré pendant l'attente");
    assert!(matches!(b.vue(), Some(Vue::Attente)));
    let page = b.enquete().clone();
    assert_eq!(page["titre"], "Mindcare regarde ce que Windows a noté...");
    let boutons: Vec<&str> = blocs(&page, "bouton").iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(boutons, [ids::PRECEDENT, ids::ACCUEIL]);
    assert!(toutes_acceptees(&b.etat, &b.json).is_empty());
    b.propre();
    // Étape précédente pendant l'attente, puis la même réponse : la lecture n'est pas redemandée
    b.action(ids::PRECEDENT, Value::Null, ecrans::ENQUETE);
    assert!(matches!(b.vue(), Some(Vue::Etape(_))));
    let effets = b.repondre_un(choix[avant.len()]);
    assert!(!effets.contains(&Effet::LireWindows));
    assert_eq!(b.lectures, 1);
    // Windows a fini : l'enquête avance avec ce qu'il a noté, sans changer d'écran
    let tout_vu = indices(1, &mut Hasard::nouveau(1));
    let effets = b.etat.windows_lu(cat(), tout_vu.clone(), Vec::new());
    assert!(effets.contains(&Effet::EnregistrerEnquete), "l'enquête qui attendait n'avance pas : {effets:?}");
    assert!(effets.iter().all(|e| !matches!(e, Effet::Afficher { ecran: Some(_) })), "l'écran ne change pas");
    b.executer(effets);
    // (l'enquête est gardée avec la réponse « sur ce PC-là »)
    let gardee = b.etat.historique.first().expect("enquête gardée");
    assert_eq!(champ(gardee, "Choix"), Some(&json!(b.etat.en_cours.choix)));
    assert!(b.etat.en_cours.choix.len() > avant.len());
    assert!(!matches!(b.vue(), Some(Vue::Attente) | None));
    let attendue = enquete(cat(), p, &b.etat.en_cours.choix, Some(&tout_vu.moteur()));
    let Some(Vue::Etape(e) | Vue::Reponse(e)) = b.vue() else { unreachable!() };
    assert_eq!(e.restants, attendue.restants);
    assert_eq!(e.auto, attendue.auto);
    assert!(!e.auto.is_empty(), "les étapes automatiques ont servi");
    b.propre();

    // Partie ailleurs pendant la lecture : rien n'avance tout seul, et rien n'est enregistré
    let mut b = Banc::nouveau(None);
    b.commencer(&panne);
    b.repondre(&choix[..=avant.len()]);
    assert!(matches!(b.vue(), Some(Vue::Attente)));
    b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
    let effets = b.etat.windows_lu(cat(), tout_vu, Vec::new());
    assert_eq!(effets, vec![Effet::Afficher { ecran: None }]);
}

impl Banc {
    fn repondre_un(&mut self, c: i64) -> Vec<Effet> {
        if c < 0 {
            self.action(ids::NSP, Value::Null, ecrans::ENQUETE)
        } else {
            self.action(ids::REP, json!(c.to_string()), ecrans::ENQUETE)
        }
    }
}

#[test]
fn menu_messages_et_actions_inconnues() {
    let mut b = Banc::nouveau(None);
    assert_eq!(b.message(json!({ "type": "menu" })), vec![Effet::MenuMindset]);
    assert!(b.menu);
    assert!(b.message(json!({ "type": "ecran", "id": "familles" })).is_empty());
    assert_eq!(b.etat.ecran, "familles");
    assert_eq!(b.action("nimporte", Value::Null, ecrans::ACCUEIL), vec![Effet::Inconnue("nimporte".into())]);
    assert_eq!(b.action("changer:x", Value::Null, ecrans::ENQUETE), vec![Effet::Inconnue("changer:x".into())]);
    assert!(matches!(b.message(json!({ "type": "bizarre" }))[0], Effet::Inconnue(_)));
    // Une panne inconnue : l'accueil
    assert_eq!(
        b.action("panne:nexistepas", Value::Null, ecrans::RACONTE),
        vec![Effet::Afficher { ecran: Some(ecrans::ACCUEIL.into()) }]
    );
    // Les tuiles et le rond qui ont leur page y mènent
    for p in [ecrans::RACONTE, ecrans::FAMILLES, ecrans::AIDE] {
        assert_eq!(b.action(p, Value::Null, ecrans::ACCUEIL), vec![Effet::Afficher { ecran: Some(p.into()) }]);
    }
    // Reprendre sans aucune enquête gardée : l'accueil, une ligne au journal
    let effets = b.action(super::accueil::REPRENDRE, Value::Null, ecrans::ACCUEIL);
    assert!(effets.contains(&Effet::Afficher { ecran: Some(ecrans::ACCUEIL.into()) }));
    assert!(matches!(&effets[0], Effet::Journal(l) if l.ends_with("introuvable")));
}

#[test]
fn catalogue_illisible() {
    let j = etat_sans_catalogue("0.1.0", "neon");
    assert!(verifier_kit(&j).is_empty(), "{:?}", verifier_kit(&j));
    assert_eq!(j["accueil"]["tuiles"][0]["nom"], "Mindcare n'a pas pu lire ses enquêtes");
}

#[test]
fn le_verificateur_du_kit_voit_les_erreurs() {
    let mut b = Banc::nouveau(Some(IndicesGardes::default()));
    b.commencer(&t(&cat().pannes()[0], "Id"));
    let bon = b.json.clone();
    assert!(verifier_kit(&bon).is_empty());
    let casse = |f: &dyn Fn(&mut Value)| {
        let mut j = bon.clone();
        f(&mut j);
        assert!(!verifier_kit(&j).is_empty(), "erreur pas vue");
    };
    casse(&|j| j["pages"]["enquete"]["blocs"][0]["type"] = json!("inconnu"));
    casse(&|j| j["pages"]["enquete"]["blocs"] = json!([]));
    casse(&|j| j["pages"]["enquete"]["titre"] = json!(""));
    casse(&|j| j["pages"]["familles"]["blocs"][0]["elements"][0]["icone"] = json!("inconnue"));
    casse(&|j| {
        j["pages"]["enquete"]["blocs"] = json!([{ "type": "repli", "titre": "x", "blocs": [{ "type": "inconnu" }] }])
    });
    casse(&|j| j["pages"]["enquete"]["chemin"] = json!(["", "b"]));
    casse(&|j| j["pages"]["enquete"]["focus"] = json!("o:absent"));
    casse(&|j| j["pc"] = json!({ "id": "", "texte": "Portable" }));
    casse(&|j| j["pages"]["raconte"]["sous"] = json!("un tiret \u{2014} long"));
    casse(&|j| j["pages"]["enquete"]["blocs"][0]["sur"] = json!(0));
    casse(&|j| j["ecran"] = json!("pourEtreSur"));
    casse(&|j| j["accueil"]["tuiles"] = json!([]));
    let mut j: Map<String, Value> = bon.as_object().unwrap().clone();
    j.insert(
        "pages".into(),
        json!({ "x": { "titre": "X", "parent": "absente", "blocs": [{ "type": "texte", "texte": "a" }] } }),
    );
    assert!(!verifier_kit(&Value::Object(j)).is_empty());
}

/// Une famille avec des groupes au catalogue (« Groupes » : Id, Titre, Exemple, Icone ; « Groupe » sur chaque
/// panne) : une carte par groupe, dans son ordre, la première ouverte ; les pannes sans groupe dans « Autres pannes ».
#[test]
fn famille_en_cartes_par_groupe() {
    let mut j = cat().json().clone();
    let familles = j.get_mut("Familles").and_then(Value::as_array_mut).unwrap();
    let f = familles.iter_mut().find(|f| f["Id"] == "allumage").unwrap();
    f["Groupes"] = json!([
        { "Id": "vide", "Titre": "Un groupe sans panne", "Exemple": "x", "Icone": "marche" },
        { "Id": "eteint", "Titre": "Il s'éteint ou redémarre", "Exemple": "Il démarre, puis s'arrête", "Icone": "actu" },
        { "Id": "rien", "Titre": "Il ne s'allume pas", "Exemple": "Rien ne se passe", "Icone": "icone-inconnue" },
    ]);
    let pannes: Vec<String> = super::familles::pannes_de(cat(), "allumage").iter().map(|p| t(p, "Id")).collect();
    assert!(pannes.len() >= 4);
    for p in j.get_mut("Pannes").and_then(Value::as_array_mut).unwrap() {
        let id = texte(p.get("Id"));
        if id == pannes[0] || id == pannes[2] {
            p["Groupe"] = json!("rien");
        } else if id == pannes[1] {
            p["Groupe"] = json!("eteint");
        }
    }
    let c = Catalogue::lire(&Value::Object(j).to_string()).expect("catalogue modifié");
    let page = super::familles::page_famille(&Etat::default(), &c, "allumage").unwrap();
    let cartes = blocs(&page, "depliables")[0]["elements"].as_array().unwrap().clone();
    let ids: Vec<&str> = cartes.iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["eteint", "rien", "autres"], "le groupe vide n'a pas de carte");
    assert_eq!(cartes[0]["ouvert"], true);
    assert!(cartes[1].get("ouvert").is_none());
    assert_eq!(cartes[1]["nombre"], 2);
    assert_eq!(cartes[1]["pannes"][1]["id"], format!("panne:{}", pannes[2]));
    assert_eq!(cartes[1]["secours"], "marche", "l'icône de la famille si celle du groupe est inconnue du kit");
    assert_eq!(cartes[2]["titre"], "Autres pannes");
    assert_eq!(cartes[2]["nombre"].as_u64().unwrap() as usize, pannes.len() - 3);
    assert_eq!(page["focus"], format!("l:panne:{}", pannes[1]));
    let mut toutes = pannes_depliees(&page);
    toutes.sort();
    let mut attendues: Vec<String> = pannes.iter().map(|p| format!("panne:{p}")).collect();
    attendues.sort();
    assert_eq!(toutes, attendues, "chaque panne une fois");
}

/// Aucun tiret long ni thème perso dans le code des pages (ni dans ses textes).
#[test]
fn code_propre() {
    let dossier = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join("pages");
    let perso = [
        concat!("ma", "rio"),
        concat!("pok", "emon"),
        concat!("pok", "émon"),
        concat!("fire ", "emblem"),
        concat!("do", "fus"),
    ];
    let mut vus = 0;
    // (les fichiers des pages, et ceux de leurs sous-dossiers : tests\aides.rs)
    let mut fichiers = Vec::new();
    let mut dossiers = vec![dossier];
    while let Some(d) = dossiers.pop() {
        for f in std::fs::read_dir(&d).unwrap() {
            let f = f.unwrap().path();
            if f.is_dir() { dossiers.push(f) } else { fichiers.push(f) }
        }
    }
    for f in fichiers {
        let texte = std::fs::read_to_string(&f).unwrap();
        vus += 1;
        assert!(!texte.contains('\u{2013}') && !texte.contains('\u{2014}'), "{} : tiret long", f.display());
        let bas = texte.to_lowercase();
        for m in perso {
            assert!(!bas.contains(m), "{} : {m}", f.display());
        }
    }
    assert!(vus >= 9);
}
