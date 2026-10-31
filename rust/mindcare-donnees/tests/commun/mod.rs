//! Ce que les tests partagent : le vrai catalogue, la lecture des juges exportés par
//! outils\exporter-juges-donnees.ps1, et le rejeu d'un scénario dans un dossier d'essai neuf (jamais le vrai dossier
//! de données : un dossier temporaire créé par le test, effacé à la fin).
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use mindcare_donnees::carte::etat_carte;
use mindcare_donnees::enquete_en_cours::nouvel_id;
use mindcare_donnees::rapport::enregistrer_rapport;
use mindcare_donnees::{
    Affichage, Donnees, EnqueteEnCours, Enregistrement, Horloge, IndiceGarde, IndicesGardes, Moment, Paire, PreRempli,
    SurLePc,
};
use mindcare_moteur::Catalogue;
use serde_json::{Map, Value, json};

/// Le dossier des juges (tests\juges).
pub fn dossier_juges() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("juges")
}

/// Le dossier Mindcare\fichiers du dépôt.
pub fn fichiers_mindcare() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("Mindcare").join("fichiers")
}

/// Le vrai catalogue, lu une seule fois.
pub fn catalogue() -> &'static Catalogue {
    static CAT: OnceLock<Catalogue> = OnceLock::new();
    CAT.get_or_init(|| Catalogue::lire_fichier(&fichiers_mindcare().join("enquetes.json")).expect("enquetes.json"))
}

/// La version de Mindcare (Mindcare.ps1), écrite à la fin du rapport.
pub fn version() -> String {
    let t = std::fs::read_to_string(fichiers_mindcare().join("Mindcare.ps1")).expect("Mindcare.ps1");
    let debut = t.find("$AppVersion = '").expect("$AppVersion") + "$AppVersion = '".len();
    t[debut..].split('\'').next().unwrap_or_default().to_string()
}

/// L'empreinte du catalogue, comme l'export (FNV-1a sur 32 bits, sans les retours chariot).
pub fn empreinte(octets: &[u8]) -> String {
    let mut h: u32 = 2_166_136_261;
    for b in octets.iter().filter(|b| **b != 13) {
        h = (h ^ u32::from(*b)).wrapping_mul(16_777_619);
    }
    format!("{h:08x}")
}

/// Un juge, après avoir vérifié qu'il a été exporté avec ce catalogue-ci.
pub fn lire_juge(nom: &str) -> Value {
    let chemin = dossier_juges().join(nom);
    let texte = std::fs::read_to_string(&chemin).unwrap_or_else(|e| panic!("{} : {e}", chemin.display()));
    let juge: Value = serde_json::from_str(&texte).unwrap_or_else(|e| panic!("{nom} : {e}"));
    let octets = std::fs::read(fichiers_mindcare().join("enquetes.json")).expect("enquetes.json");
    assert_eq!(
        empreinte(&octets),
        juge["Catalogue"]["Empreinte"].as_str().unwrap_or_default(),
        "{nom} a été exporté avec un autre catalogue : relance outils\\exporter-juges-donnees.ps1"
    );
    juge
}

/// Les juges dont le nom commence par ce début, dans l'ordre des noms.
pub fn juges_commencant_par(debut: &str) -> Vec<String> {
    let mut noms: Vec<String> = std::fs::read_dir(dossier_juges())
        .expect("tests\\juges")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .filter(|n| n.starts_with(debut) && n.ends_with(".json"))
        .collect();
    noms.sort();
    noms
}

/// Échoue en montrant les premiers écarts (et leur nombre).
pub fn verdict_final(quoi: &str, total: usize, ecarts: &[String]) {
    assert!(total > 0, "{quoi} : aucun cas dans le juge");
    if !ecarts.is_empty() {
        let premiers: Vec<&str> = ecarts.iter().take(12).map(String::as_str).collect();
        panic!("{quoi} : {} écart(s) sur {total}\n{}", ecarts.len(), premiers.join("\n"));
    }
    println!("{quoi} : {total} cas, tous pareils");
}

/// Un texte d'un juge.
pub fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

/// Un moment « 2026-10-30T14:05:00 ».
pub fn moment(t: &str) -> Moment {
    Moment::lire(t).unwrap_or_else(|| panic!("date {t}"))
}

/// Des indices d'un juge (une liste de { Id, Vu, Texte, TexteOk, Affiche, Partiel }), comme une table PowerShell.
pub fn indices(v: &Value) -> IndicesGardes {
    let mut g = IndicesGardes::default();
    for i in v.as_array().into_iter().flatten() {
        g.mettre(
            s(&i["Id"]),
            IndiceGarde {
                vu: i["Vu"].as_bool().unwrap_or(false),
                texte: s(&i["Texte"]).to_string(),
                texte_ok: s(&i["TexteOk"]).to_string(),
                affiche: i["Affiche"].as_bool().unwrap_or(false),
                partiel: i["Partiel"].as_bool().unwrap_or(false),
            },
        );
    }
    g
}

/// Du base64 (les fichiers de départ des scénarios).
pub fn base64(t: &str) -> Vec<u8> {
    let val = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("base64 : {c}"),
        }
    };
    let o: Vec<u8> = t.bytes().filter(|c| *c != b'=').collect();
    let mut r = Vec::new();
    for bloc in o.chunks(4) {
        let mut n = 0u32;
        for (i, c) in bloc.iter().enumerate() {
            n |= val(*c) << (18 - 6 * i);
        }
        r.push((n >> 16) as u8);
        if bloc.len() > 2 {
            r.push((n >> 8) as u8);
        }
        if bloc.len() > 3 {
            r.push(n as u8);
        }
    }
    r
}

/// Un dossier d'essai neuf sous le dossier temporaire, effacé à la fin (jamais le vrai dossier de données).
pub struct DossierEssai(pub PathBuf);

impl DossierEssai {
    pub fn nouveau(nom: &str) -> DossierEssai {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!(
            "mindcare-donnees-{nom}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dossier d'essai");
        DossierEssai(d)
    }
}

impl Drop for DossierEssai {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Un fichier gardé ouvert sans partage, comme par un autre programme (antivirus, OneDrive).
pub fn prendre(chemin: &Path) -> File {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(chemin)
        .unwrap_or_else(|e| panic!("{} : {e}", chemin.display()))
}

// ---------------------------------------------------------------------------
// Ce qu'on compare : comme l'exporteur
// ---------------------------------------------------------------------------

/// « .abime-20261030-140500 » devient « .abime-* ».
fn sans_dates_abime(t: &str) -> String {
    let mut r = String::new();
    let mut reste = t;
    while let Some(i) = reste.find(".abime-") {
        r.push_str(&reste[..i + 7]);
        let apres = &reste[i + 7..];
        let o = apres.as_bytes();
        let date = o.len() >= 15
            && o[..8].iter().all(u8::is_ascii_digit)
            && o[8] == b'-'
            && o[9..15].iter().all(u8::is_ascii_digit);
        if date {
            r.push('*');
            reste = &apres[15..];
        } else {
            reste = apres;
        }
    }
    r.push_str(reste);
    r
}

/// Une ligne du journal comme l'exporteur la garde.
pub fn normal(t: &str, dossier: &str) -> String {
    let mut t = sans_dates_abime(&t.replace(dossier, "<D>"));
    // (la raison d'un fichier illisible : le message de .NET ou de serde_json)
    if let Some(i) = t.find(" Fichier illisible (")
        && let Some(j) = t[i..].find(") : ")
    {
        t = format!("{} : *", &t[..i + j + 1]);
    }
    if let Some(i) = t.find("(déjà su : ")
        && let Some(j) = t[i..].find(')')
    {
        let dedans = &t[i + "(déjà su : ".len()..i + j];
        let mut l: Vec<&str> = dedans.split(", ").collect();
        l.sort_by_key(|x| x.to_uppercase());
        t = format!("{}(déjà su : {}){}", &t[..i], l.join(", "), &t[i + j + 1..]);
    }
    t
}

/// Tous les textes d'une valeur, normalisés.
fn normaliser(v: &Value, dossier: &str) -> Value {
    match v {
        Value::String(t) => Value::String(sans_dates_abime(&t.replace(dossier, "<D>"))),
        Value::Array(a) => Value::Array(a.iter().map(|x| normaliser(x, dossier)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), normaliser(x, dossier))).collect()),
        x => x.clone(),
    }
}

// ---------------------------------------------------------------------------
// Le rejeu d'un scénario
// ---------------------------------------------------------------------------

/// Ce qui est montré, comme les écrans remplacés de l'exporteur.
fn vu(a: &Affichage) -> Value {
    match a {
        Affichage::Accueil => json!({ "Ecran": "accueil" }),
        Affichage::Famille(f) => json!({ "Ecran": "famille", "Famille": f }),
        Affichage::Etape(e) => json!({
            "Ecran": "etape",
            "Etape": e.etape.as_ref().map(|x| mindcare_moteur::ps::texte(mindcare_moteur::ps::champ(x, "Id"))).unwrap_or_default()
        }),
        Affichage::Reponse(_, titre) => json!({ "Ecran": "reponse", "Titre": titre }),
    }
}

fn etat_session(s: &EnqueteEnCours, vu: &Value) -> Value {
    let pr = s.pre_rempli.as_ref().map(|p| {
        let mut l: Vec<&(String, i64)> = p.0.iter().collect();
        l.sort_by_key(|(k, _)| k.to_uppercase());
        Value::Array(l.iter().map(|(k, v)| json!([k, v])).collect())
    });
    json!({ "Vu": vu, "DiagId": s.diag_id, "Choix": s.choix, "PreRempli": pr, "Reprise": s.est_reprise() })
}

/// Le texte d'un fichier comme ReadAllText.
fn texte_fichier(p: &Path) -> String {
    suite_commun::json_sur::texte_du_fichier(&std::fs::read(p).unwrap_or_default())
}

/// Rejoue un scénario d'un juge ; rend ce que l'exporteur a noté pour lui (Resultats, Historique, Reglages,
/// Fichiers, Abimes, Journal, Pris).
pub fn jouer(sc: &Value) -> Value {
    let cat = catalogue();
    let essai = DossierEssai::nouveau("scenario");
    let dir = essai.0.clone();
    let dossier = dir.display().to_string();
    for (nom, b64) in sc["Fichiers"].as_object().into_iter().flatten() {
        std::fs::write(dir.join(nom), base64(s(b64))).expect("fichier de départ");
    }
    if let Some(g) = sc["JournalGenere"].as_object() {
        let lignes = g["Lignes"].as_u64().unwrap_or(0);
        let largeur = g["Largeur"].as_u64().unwrap_or(1);
        let mut t = String::new();
        for i in 0..lignes {
            t.push_str(&format!("ancienne ligne {i} "));
            t.push_str(&"x".repeat((i % largeur) as usize));
            t.push_str(if i % 7 == 3 { "\n" } else { "\r\n" });
        }
        std::fs::write(dir.join("journal.txt"), t).expect("journal");
    }
    let mut verrous: HashMap<String, File> = HashMap::new();
    for n in sc["Pris"].as_array().into_iter().flatten() {
        verrous.insert(s(n).to_string(), prendre(&dir.join(s(n))));
    }
    let heure = Arc::new(Mutex::new(moment(s(&sc["Maintenant"]))));
    let h2 = Arc::clone(&heure);
    let horloge: Horloge = Arc::new(move || *h2.lock().expect("heure"));
    let infos = s(&sc["InfosPc"]).to_string();
    let barrettes: Vec<String> = sc["Barrettes"].as_array().into_iter().flatten().map(|b| s(b).to_string()).collect();
    let version = version();
    let mut donnees: Option<Donnees> = None;
    let mut session = EnqueteEnCours::default();
    let mut jour = IndicesGardes::default();
    let mut resultats = Vec::new();
    for op in sc["Ops"].as_array().into_iter().flatten() {
        let mut r = Value::Null;
        let fichier = || dir.join(s(&op["Fichier"]));
        match s(&op["Op"]) {
            "Demarrer" => {
                donnees = Some(Donnees::ouvrir(&dir, Arc::clone(&horloge), true));
                session = EnqueteEnCours::default();
            }
            "LireHistorique" => donnees.as_mut().expect("Demarrer").lire_historique(),
            "Prendre" => {
                verrous.insert(s(&op["Fichier"]).to_string(), prendre(&fichier()));
            }
            "Liberer" => {
                verrous.remove(s(&op["Fichier"]));
            }
            "Ecrire" => std::fs::write(fichier(), s(&op["Texte"])).expect("Ecrire"),
            "Effacer" => {
                let _ = std::fs::remove_file(fichier());
            }
            "Heure" => *heure.lock().expect("heure") = moment(s(&op["Maintenant"])),
            "IndicesDuJour" => jour = indices(&op["Indices"]),
            "Reglage" => {
                let defaut: Value = serde_json::from_str(s(&op["Defaut"])).expect("Defaut");
                r = donnees.as_ref().expect("Demarrer").reglages.lire(s(&op["Nom"]), defaut);
            }
            "ChangerReglage" => {
                let valeur: Value = serde_json::from_str(s(&op["Valeur"])).expect("Valeur");
                let d = donnees.as_mut().expect("Demarrer");
                d.reglages.changer(&mut d.fichiers, s(&op["Nom"]), valeur);
            }
            "LireJson" => {
                r = donnees.as_mut().expect("Demarrer").fichiers.lire(&fichier()).map_or(Value::Null, Value::Object);
            }
            "Relire" => {
                let f = donnees.as_mut().expect("Demarrer").fichiers.relire(&fichier());
                r = json!({ "Libre": f.libre, "Relu": f.relu, "Contenu": f.contenu });
            }
            "Sauver" => {
                let choix: Vec<i64> = op["Choix"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
                let reponses: Vec<Paire> = op["Reponses"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| Paire { etape: s(&p[0]).to_string(), reponse: p[1].as_i64().unwrap_or(0) })
                    .collect();
                let ind = if op["Indices"].is_null() { None } else { Some(indices(&op["Indices"])) };
                let suspect = op["Suspect"].as_str();
                donnees.as_mut().expect("Demarrer").sauver_diagnostic(
                    Some(cat),
                    &Enregistrement {
                        id: s(&op["Id"]),
                        panne: s(&op["Panne"]),
                        choix: &choix,
                        fini: op["Fini"].as_bool().unwrap_or(false),
                        suspect,
                        titre: s(&op["Titre"]),
                        reponses: &reponses,
                        indices: ind.as_ref(),
                        reprise: op["Reprise"].as_bool().unwrap_or(false),
                    },
                );
            }
            "Carte" => r = etat_carte(Some(cat), &donnees.as_ref().expect("Demarrer").historique.liste).json(),
            "EcrireEtat" => donnees.as_mut().expect("Demarrer").ecrire_etat(Some(cat)),
            "Commencer" => {
                let pr = if op["PreRempli"].is_null() {
                    None
                } else {
                    let mut p = PreRempli::default();
                    for x in op["PreRempli"].as_array().into_iter().flatten() {
                        p.mettre(s(&x[0]), x[1].as_i64().unwrap_or(0));
                    }
                    Some(p)
                };
                let maintenant = *heure.lock().expect("heure");
                let id = nouvel_id(&maintenant, op["Hasard"].as_u64().unwrap_or(0) as u16);
                let d = donnees.as_mut().expect("Demarrer");
                let ecran = vu(&session.commencer(cat, d, s(&op["Panne"]), pr.as_ref(), id, &jour));
                r = etat_session(&session, &ecran);
            }
            "Ouvrir" => {
                let d = donnees.as_mut().expect("Demarrer");
                let ecran = vu(&session.ouvrir(cat, d, s(&op["Id"]), &jour));
                r = etat_session(&session, &ecran);
            }
            "Repondre" => {
                let d = donnees.as_mut().expect("Demarrer");
                let ecran = vu(&session.repondre(cat, d, op["N"].as_i64().unwrap_or(0), &jour));
                r = etat_session(&session, &ecran);
            }
            "Retour" => {
                let d = donnees.as_mut().expect("Demarrer");
                let ecran = vu(&session.retour(cat, d, &jour));
                r = etat_session(&session, &ecran);
            }
            "Rapport" | "EnregistrerRapport" => {
                let maintenant = *heure.lock().expect("heure");
                let pc = SurLePc { infos: &infos, barrettes: &barrettes, indices_du_jour: Some(&jour) };
                if let Some(t) = session.rapport(cat, &maintenant, &pc, &version) {
                    r = if s(&op["Op"]) == "Rapport" {
                        Value::String(t)
                    } else {
                        let d = donnees.as_ref().expect("Demarrer");
                        let f = enregistrer_rapport(&dir, &t, d.journal()).expect("rapport");
                        Value::String(f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                    };
                }
            }
            autre => panic!("opération inconnue : {autre}"),
        }
        resultats.push(normaliser(&r, &dossier));
    }
    // (les fichiers encore pris sont libérés pour lire ce qu'ils contiennent)
    verrous.clear();
    let mut fichiers = Map::new();
    let mut abimes: Vec<(String, String)> = Vec::new();
    let mut noms: Vec<String> = std::fs::read_dir(&dir)
        .expect("dossier d'essai")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    noms.sort();
    for nom in noms {
        if nom == "journal.txt" || nom.ends_with(".tmp") {
            continue;
        }
        let t = texte_fichier(&dir.join(&nom));
        match nom.find(".abime-") {
            Some(i) => abimes.push((nom[..i].to_string(), t)),
            None => {
                fichiers.insert(nom, Value::String(t));
            }
        }
    }
    // (une ligne du journal qui ne commence pas par une date continue la précédente : un message sur plusieurs lignes)
    let mut lignes: Vec<String> = Vec::new();
    if dir.join("journal.txt").is_file() {
        for l in mindcare_donnees::journal::lignes(&texte_fichier(&dir.join("journal.txt"))) {
            match lignes.last_mut() {
                Some(der) if !commence_par_date(l) => {
                    der.push('\n');
                    der.push_str(l);
                }
                _ => lignes.push(l.to_string()),
            }
        }
    }
    let journal: Vec<String> = lignes.iter().map(|l| normal(l, &dossier)).collect();
    let d = donnees.as_ref();
    let mut pris: Vec<String> = d
        .map(|d| d.fichiers.fichiers_pris())
        .unwrap_or_default()
        .iter()
        .filter_map(|p| Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    pris.sort();
    json!({
        "Resultats": resultats,
        "Historique": normaliser(&Value::Array(d.map(|d| d.historique.liste.iter().cloned().map(Value::Object).collect()).unwrap_or_default()), &dossier),
        "Reglages": d.map(|d| Value::Object(d.reglages.valeurs().clone())).unwrap_or(Value::Null),
        "Fichiers": normaliser(&Value::Object(fichiers), &dossier),
        "Abimes": abimes.iter().map(|(de, t)| json!({ "De": de, "Texte": t })).collect::<Vec<_>>(),
        "Journal": journal,
        "Pris": pris,
    })
}

/// Un texte de fichier comme valeur comparable : le JSON qu'il contient s'il en contient, sinon le texte.
fn contenu(v: &Value) -> Value {
    match v.as_str().map(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}'))) {
        Some(Ok(j)) if !s(v).trim().is_empty() => json!({ "Json": j }),
        _ => v.clone(),
    }
}

/// Les fichiers mis de côté, sans doublon, rangés.
fn abimes(v: &Value) -> Vec<(String, Value)> {
    let mut l: Vec<(String, Value)> =
        v.as_array().into_iter().flatten().map(|a| (s(&a["De"]).to_string(), contenu(&a["Texte"]))).collect();
    l.sort_by_key(|(d, t)| format!("{d}|{t}"));
    l.dedup();
    l
}

/// Ce qui diffère entre ce que le juge attend et ce que le Rust a obtenu, ou rien.
pub fn ecart(attendu: &Value, obtenu: &Value) -> Option<String> {
    let mut d = Vec::new();
    let ra = attendu["Resultats"].as_array().cloned().unwrap_or_default();
    let ro = obtenu["Resultats"].as_array().cloned().unwrap_or_default();
    if ra.len() != ro.len() {
        d.push(format!("Resultats : {} contre {}", ra.len(), ro.len()));
    }
    for (i, (a, o)) in ra.iter().zip(ro.iter()).enumerate() {
        if a != o {
            d.push(format!("opération {i} : PowerShell {a}\n      Rust {o}"));
        }
    }
    for c in ["Historique", "Reglages", "Pris", "Journal"] {
        if attendu[c] != obtenu[c] {
            d.push(format!("{c} : PowerShell {}\n      Rust {}", attendu[c], obtenu[c]));
        }
    }
    let fa = attendu["Fichiers"].as_object().cloned().unwrap_or_default();
    let fo = obtenu["Fichiers"].as_object().cloned().unwrap_or_default();
    let mut noms: Vec<&String> = fa.keys().chain(fo.keys()).collect();
    noms.sort();
    noms.dedup();
    for n in noms {
        let a = fa.get(n).map(contenu);
        let o = fo.get(n).map(contenu);
        if a != o {
            d.push(format!("fichier {n} : PowerShell {a:?}\n      Rust {o:?}"));
        }
    }
    if abimes(&attendu["Abimes"]) != abimes(&obtenu["Abimes"]) {
        d.push(format!("Abimes : PowerShell {}\n      Rust {}", attendu["Abimes"], obtenu["Abimes"]));
    }
    if d.is_empty() { None } else { Some(d.join("\n    ")) }
}

/// Rejoue des scénarios en parallèle (les fichiers pris attendent 1,2 s à chaque lecture) ; rend les écarts.
pub fn jouer_tous(scenarios: &[Value], attendu: &str) -> Vec<String> {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 8);
    let ecarts = Mutex::new(Vec::new());
    let suivant = AtomicUsize::new(0);
    std::thread::scope(|t| {
        for _ in 0..n {
            t.spawn(|| {
                loop {
                    let i = suivant.fetch_add(1, Ordering::SeqCst);
                    let Some(sc) = scenarios.get(i) else { break };
                    let obtenu = jouer(sc);
                    if let Some(e) = ecart(&sc[attendu], &obtenu) {
                        ecarts.lock().expect("écarts").push(format!("« {} » :\n    {e}", s(&sc["Nom"])));
                    }
                }
            });
        }
    });
    ecarts.into_inner().expect("écarts")
}

/// Une ligne du journal qui commence par sa date (« 2026-10-30T14:05:00 »).
fn commence_par_date(l: &str) -> bool {
    let o = l.as_bytes();
    let chiffres = |a: usize, b: usize| o.len() > b && o[a..b].iter().all(u8::is_ascii_digit);
    o.len() >= 20
        && chiffres(0, 4)
        && o[4] == b'-'
        && chiffres(5, 7)
        && o[7] == b'-'
        && chiffres(8, 10)
        && o[10] == b'T'
        && chiffres(11, 13)
        && o[13] == b':'
        && chiffres(14, 16)
        && o[16] == b':'
        && chiffres(17, 19)
        && o[19] == b' '
}
