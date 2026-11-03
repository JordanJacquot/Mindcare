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
use mindcare_moteur::{Catalogue, Enquete, INDICES_CONNUS, Indices, Objet, TypeVerdict, chercher, enquete};

use super::actions::{PLUS_TARD_MESSAGE, ids};
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
                    let r = self.rapport().ok_or_else(|| "aucun rapport".to_string()).and_then(|t| {
                        enregistrer_rapport(&self.dossier, &t, self.donnees.journal())
                            .map(|f| f.file_name().unwrap_or_default().to_string_lossy().into_owned())
                            .map_err(|e| e.to_string())
                    });
                    file.extend(self.etat.rapport_enregistre(r));
                }
                Effet::CopierRapport => {
                    let r = self.rapport().map(|_| ()).ok_or_else(|| "aucun rapport".to_string());
                    file.extend(self.etat.rapport_copie(r));
                }
                Effet::CopierCommande { numero, .. } => file.extend(self.etat.commande_copiee(numero, Ok(()))),
                Effet::MenuMindset => self.menu = true,
                Effet::Inconnue(x) => self.inconnues.push(x),
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
];
const COULEURS: &[&str] = &["P", "S", "T", "vert", "orange", "rouge", "fond", "fond2"];
const TONS: &[&str] = &["", "ok", "attention", "urgent"];

fn texte_plein(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty())
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
        let blocs = p["blocs"].as_array().cloned().unwrap_or_default();
        if blocs.is_empty() {
            ep("page vide (aucun bloc)".into());
        }
        for b in &blocs {
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
                }
                "case" => manque(&["id", "texte"]) || !b["oui"].is_boolean(),
                "bouton" => {
                    manque(&["id", "texte"])
                        || b.get("style").and_then(Value::as_str).is_some_and(|s| s != "second" && s != "danger")
                }
                "champ" => manque(&["id"]) || !b["valeur"].is_string(),
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
    for (id, p) in etat["pages"].as_object().into_iter().flatten() {
        let mut ajoute = |a: &str, v: Value| l.push((a.to_string(), v, id.clone()));
        if p.get("retour").is_some() {
            ajoute(ids::RETOUR, Value::Null);
        }
        for a in p["actions"].as_array().into_iter().flatten() {
            ajoute(a["id"].as_str().unwrap_or_default(), a.get("valeur").cloned().unwrap_or(Value::Null));
        }
        for b in p["module"]["boutons"].as_array().into_iter().flatten() {
            ajoute(b["id"].as_str().unwrap_or_default(), b.get("valeur").cloned().unwrap_or(Value::Null));
        }
        for b in p["blocs"].as_array().into_iter().flatten() {
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
                "champ" => ajoute(bid, json!("écran bleu")),
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

/// Les blocs d'une page, par type.
fn blocs<'a>(page: &'a Value, ty: &str) -> Vec<&'a Value> {
    page["blocs"].as_array().into_iter().flatten().filter(|b| b["type"] == ty).collect()
}

fn a_texte(page: &Value, texte: &str) -> bool {
    page["blocs"].as_array().into_iter().flatten().any(|b| b["texte"] == texte)
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
    assert_eq!(page["module"]["titre"], badge, "{panne} {choix:?}");
    assert_eq!(page["module"]["couleur"], couleur, "{panne} {choix:?}");
    assert_eq!(page["module"]["texte"], titre_reponse(cat(), &attendue).titre, "{panne} {choix:?}");
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
    let grille = &blocs(b.page(ecrans::FAMILLES).unwrap(), "grille")[0]["elements"];
    assert_eq!(grille.as_array().unwrap().len(), cat().familles().len());
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
        let lignes: Vec<String> = blocs(page, "ligne").iter().map(|l| l["id"].as_str().unwrap().to_string()).collect();
        assert_eq!(lignes, attendues, "famille {id}");
        assert_eq!(page["module"]["resume"][0]["valeur"], attendues.len().to_string());
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
        let jauge = blocs(&page, "jauge")[0];
        assert_eq!(jauge["texte"], format!("Étape 1 sur {}", 1 + attendue.a_venir), "{id}");
        let cartes = blocs(&page, "cartes")[0];
        assert_eq!(cartes["id"], ids::REP);
        assert_eq!(cartes["options"].as_array().unwrap().len(), tableau(champ(et, "Reponses")).len(), "{id}");
        let test = egal(&t(et, "Type"), "test");
        assert_eq!(page["actions"][0]["id"], if test { ids::PLUS_TARD } else { ids::ACCUEIL }, "{id}");
        let montres = suspects_montres(cat(), p);
        if !montres.is_empty() {
            assert_eq!(page["module"]["anneau"]["sur"], montres.len(), "{id}");
            assert_eq!(page["module"]["anneau"]["valeur"], 0, "{id}");
            assert_eq!(page["module"]["resume"].as_array().unwrap().len(), montres.len(), "{id}");
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
    assert_eq!(page["icone"], "test");
    assert_eq!(
        page["actions"][0],
        json!({ "id": ids::PLUS_TARD, "texte": "Je le ferai plus tard", "icone": "horloge" })
    );
    assert!(page["sous"].as_str().unwrap().contains("une manipulation à faire"));
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
    assert_eq!(page["actions"][0]["id"], ids::ACCUEIL);
}

#[test]
fn commande_a_copier() {
    let (panne, choix) = chercher_parcours(|_, e| e.etape.as_ref().is_some_and(|et| champ(et, "Commande").is_some()))
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
        let actions: Vec<&str> =
            page["actions"].as_array().unwrap().iter().map(|a| a["id"].as_str().unwrap()).collect();
        assert_eq!(actions, [ids::PRECEDENT, ids::REPARATEUR], "{nom}");
        assert_eq!(page["module"]["boutons"][0]["id"], ids::ACCUEIL);
        assert!(a_texte(&page, "Comment on le sait"), "{nom}");
        assert!(a_texte(&page, "Ce qu'il faut faire"), "{nom}");
        assert!(a_texte(&page, &securite.join(" ")), "{nom} : la sécurité");
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
                assert!(page["module"]["texte"].as_str().unwrap().starts_with("À vérifier : "));
            }
            _ => assert!(!garantie && !supposee, "{nom}"),
        }
        let s = suspect_trouve(&e);
        if let Some(s) = s {
            let a_faire = textes(s, "AFaire");
            if !a_faire.is_empty() {
                assert_eq!(blocs(&page, "etapes")[0]["elements"].as_array().unwrap().len(), a_faire.len(), "{nom}");
            }
            if !t(s, "Pro").is_empty() {
                assert!(a_texte(&page, "Quand aller chez un réparateur"), "{nom}");
                assert!(a_texte(&page, &t(s, "Pro")), "{nom}");
            }
        } else {
            let v = e.verdict.as_ref().unwrap();
            let lignes: Vec<&Value> = blocs(&page, "ligne");
            for r in &v.restants {
                assert!(lignes.iter().any(|l| l["texte"] == t(cat().suspect(r).unwrap(), "Titre")), "{nom} : {r}");
            }
        }
        // Les écartés (sans les dangers), en minuscule, une seule fois chacun
        let ecartes: Vec<&Value> = blocs(&page, "texte")
            .into_iter()
            .filter(|x| x["texte"].as_str().unwrap().starts_with("Écartés : "))
            .collect();
        let montrer = e.ecartes.iter().any(|x| !cat().suspect(&x.id).is_some_and(|s| vrai(champ(s, "Urgent"))));
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
    assert_eq!(
        tuile["sous"],
        format!("{} · commencée le 3 novembre à 10h15", t(cat().panne(&panne).unwrap(), "Titre"))
    );
    b.action(super::accueil::REPRENDRE, Value::Null, ecrans::ACCUEIL);
    assert_eq!(b.ecran.as_deref(), Some(ecrans::ENQUETE));
    assert_eq!(b.etat.en_cours.choix, choix[..2]);
    assert!(b.journal.iter().any(|l| l.starts_with("Reprise de l'enquête ")));
    assert_eq!(blocs(b.enquete(), "ligne").iter().filter(|l| l["valeur"] == "Changer").count(), 2);
    b.repondre(&choix[2..]);
    verifier_reponse(&b, &panne, &choix, None);
    let titre = b.enquete()["module"]["texte"].clone();
    // Finie : « Dernière enquête », la même réponse
    b.action(ids::ACCUEIL, Value::Null, ecrans::ENQUETE);
    let tuile = b.json["accueil"]["tuiles"][2].clone();
    assert_eq!(tuile["id"], super::accueil::DERNIERE);
    assert_eq!(tuile["sous"], t(cat().panne(&panne).unwrap(), "Titre"));
    b.action(super::accueil::DERNIERE, Value::Null, ecrans::ACCUEIL);
    verifier_reponse(&b, &panne, &choix, None);
    assert_eq!(b.enquete()["module"]["texte"], titre);
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
    assert_eq!(page["sous"], textes_rapport::EXPLICATION);
    assert_eq!(page["parent"], ecrans::ENQUETE);
    let boutons = page["module"]["boutons"].as_array().unwrap();
    assert_eq!(boutons[0], json!({ "id": ids::BUREAU, "texte": textes_rapport::BOUTON_ENREGISTRER }));
    assert_eq!(boutons[1]["texte"], textes_rapport::BOUTON_COPIER);
    // Sur le Bureau (ici, le dossier d'essai)
    b.action(ids::BUREAU, Value::Null, ecrans::REPARATEUR);
    let nom = "Mindcare, diagnostic du 3-11-2026 10h15.txt";
    assert!(b.dossier.join(nom).is_file(), "le rapport n'est pas écrit");
    let attendu = textes_rapport::enregistre(nom);
    assert_eq!(b.messages.last(), Some(&(attendu.clone(), true)));
    assert!(a_texte(b.page(ecrans::REPARATEUR).unwrap(), &attendu));
    let rapport = std::fs::read_to_string(b.dossier.join(nom)).unwrap();
    assert!(rapport.contains(&format!("Panne : {}", t(cat().panne(&panne).unwrap(), "Titre"))));
    // Copié
    b.action(ids::COPIER, Value::Null, ecrans::REPARATEUR);
    assert_eq!(b.page(ecrans::REPARATEUR).unwrap()["module"]["boutons"][1]["texte"], "Copié");
    assert_eq!(b.messages.last(), Some(&(textes_rapport::COPIE.to_string(), true)));
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
    assert!(!b.etat.copie && b.etat.info_rapport.is_none());
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
    casse(&|j| j["pages"]["familles"]["blocs"][0]["elements"][0]["image"] = json!("images/absente.svg"));
    casse(&|j| j["pages"]["raconte"]["sous"] = json!("un tiret \u{2014} long"));
    casse(&|j| j["pages"]["enquete"]["module"]["anneau"]["sur"] = json!(0));
    casse(&|j| j["ecran"] = json!("pourEtreSur"));
    casse(&|j| j["accueil"]["tuiles"] = json!([]));
    let mut j: Map<String, Value> = bon.as_object().unwrap().clone();
    j.insert(
        "pages".into(),
        json!({ "x": { "titre": "X", "parent": "absente", "blocs": [{ "type": "texte", "texte": "a" }] } }),
    );
    assert!(!verifier_kit(&Value::Object(j)).is_empty());
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
    for f in std::fs::read_dir(&dossier).unwrap() {
        let f = f.unwrap().path();
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
