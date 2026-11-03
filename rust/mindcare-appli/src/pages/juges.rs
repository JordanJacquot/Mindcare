//! Les juges du moteur rejoués par les PAGES (étape 5.5 du plan « Mindcare en Rust ») : les parcours au hasard et les
//! cas nommés exportés du vrai Mindcare PowerShell par outils\exporter-juges.ps1 (rust\mindcare-moteur\tests\juges),
//! joués ici comme le ferait une personne devant l'écran : `panne:<id>`, puis chaque réponse par le bouton de l'étape
//! (`rep`, ou « Je ne sais pas » pour un numéro hors de la liste, comme le moteur le compte), la lecture de Windows
//! rendue tout de suite avec les indices du juge (aucun indice : rien de lisible). À chaque pas, l'étape montrée doit
//! être celle du PowerShell (avec « encore N étapes ») ; à la fin, l'enquête montrée (son étape ou sa réponse finale,
//! avec ses preuves, ses écartés, ses réponses et son titre) doit être exactement celle du PowerShell.

use serde_json::{Value, json};

use mindcare_donnees::{IndiceGarde, IndicesGardes};
use mindcare_moteur::ps::{champ, tableau, texte};
use mindcare_moteur::{Catalogue, Indices};

use super::actions::ids;
use super::raconte::PANNE;
use super::{Effet, Etat, Vue, ecrans};

#[allow(dead_code, clippy::all)]
#[path = "../../../mindcare-moteur/tests/commun/mod.rs"]
mod commun;

use commun::{catalogue, choix, ecart, empreinte, indices, resoudre, resultat};

fn dossier_juges() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("mindcare-moteur").join("tests").join("juges")
}

/// Un juge du moteur, après avoir vérifié qu'il a été exporté avec ce catalogue-ci.
fn lire_juge(nom: &str) -> Value {
    let chemin = dossier_juges().join(nom);
    let t = std::fs::read_to_string(&chemin).unwrap_or_else(|e| panic!("{} : {e}", chemin.display()));
    let juge: Value = serde_json::from_str(&t).unwrap_or_else(|e| panic!("{nom} : {e}"));
    let octets = std::fs::read(commun::chemin_catalogue()).expect("enquetes.json");
    assert_eq!(
        empreinte(&octets),
        juge["Catalogue"]["Empreinte"].as_str().unwrap_or_default(),
        "{nom} a été exporté avec un autre catalogue : relance outils\\exporter-juges.ps1"
    );
    juge
}

/// Ce que Windows a noté, tel que les pages le reçoivent (`windows_lu`).
fn gardes(ind: Option<&Indices>) -> IndicesGardes {
    let mut g = IndicesGardes::default();
    for (k, i) in ind.into_iter().flatten() {
        g.mettre(
            k,
            IndiceGarde {
                vu: i.vu,
                texte: i.texte.clone(),
                texte_ok: i.texte_ok.clone(),
                affiche: true,
                partiel: i.partiel,
            },
        );
    }
    g
}

/// Exécute ce dont l'enquête a besoin : la lecture de Windows, rendue tout de suite (le reste ne change pas l'enquête).
fn executer(etat: &mut Etat, cat: &Catalogue, effets: Vec<Effet>, ind: Option<&Indices>, lectures: &mut usize) {
    let mut file: std::collections::VecDeque<Effet> = effets.into();
    while let Some(e) = file.pop_front() {
        if e == Effet::LireWindows {
            *lectures += 1;
            file.extend(etat.windows_lu(cat, gardes(ind), Vec::new()));
        }
    }
}

/// Un parcours joué par les pages : l'écart avec le PowerShell, ou rien. `pas` : l'étape attendue avant chaque réponse
/// (son Id, et le nombre d'étapes qui peuvent encore venir).
pub(super) fn rejouer(
    cat: &Catalogue,
    panne: &str,
    entree: &[i64],
    ind: Option<&Indices>,
    pas: &[(String, u64)],
    attendu: &Value,
) -> Option<String> {
    let mut etat = Etat::nouveau("0.1.0", "neon", Vec::new());
    let mut n = 0u32;
    let mut nouvel_id = || {
        n += 1;
        format!("20261103-101500-{n:04x}")
    };
    let mut lectures = 0;
    let action =
        |id: &str, valeur: Value, ecran: &str| json!({ "type": "action", "id": id, "valeur": valeur, "ecran": ecran });
    let effets = etat.agir(cat, &action(&format!("{PANNE}{panne}"), Value::Null, ecrans::RACONTE), &mut nouvel_id);
    executer(&mut etat, cat, effets, ind, &mut lectures);
    let mut normalises = Vec::new();
    let mut ecarts = Vec::new();
    for (i, &c) in entree.iter().enumerate() {
        let vue = etat.vue(cat);
        if let Some((id_attendu, a_venir)) = pas.get(i) {
            let montre = match &vue {
                Some(Vue::Etape(e)) => {
                    (e.etape.as_ref().map(|x| texte(champ(x, "Id"))).unwrap_or_default(), e.a_venir as u64)
                }
                Some(Vue::Reponse(_)) => ("(réponse finale)".to_string(), 0),
                Some(Vue::Attente) => ("(attente de Windows)".to_string(), 0),
                None => ("(aucune enquête)".to_string(), 0),
            };
            if (montre.0.as_str(), montre.1) != (id_attendu.as_str(), *a_venir) {
                ecarts.push(format!("pas {i} : PowerShell ({id_attendu}, {a_venir}) / pages {montre:?}"));
            }
        }
        // (le bouton de l'étape, ou « Je ne sais pas » pour un numéro hors de la liste ; après la réponse finale, les
        // réponses en trop sont envoyées telles quelles : les pages doivent les ignorer)
        let message = match &vue {
            Some(Vue::Etape(e)) => {
                let nb = e.etape.as_ref().map(|et| tableau(champ(et, "Reponses")).len()).unwrap_or(0) as i64;
                if (0..nb).contains(&c) {
                    normalises.push(c);
                    action(ids::REP, json!(c.to_string()), ecrans::ENQUETE)
                } else {
                    normalises.push(-1);
                    action(ids::NSP, Value::Null, ecrans::ENQUETE)
                }
            }
            _ => action(ids::REP, json!(c), ecrans::ENQUETE),
        };
        let effets = etat.agir(cat, &message, &mut nouvel_id);
        executer(&mut etat, cat, effets, ind, &mut lectures);
    }
    let obtenu = match etat.vue(cat) {
        Some(Vue::Etape(e)) | Some(Vue::Reponse(e)) => resultat(cat, &e),
        Some(Vue::Attente) => return Some("les pages attendent toujours Windows".into()),
        None => return Some("aucune enquête montrée".into()),
    };
    // (le PowerShell garde le numéro hors de la liste ; les pages, « Je ne sais pas » : le même fait pour le moteur)
    let mut attendu = attendu.clone();
    let nb = attendu["Choix"].as_array().map(Vec::len).unwrap_or(0);
    if nb <= normalises.len() && choix(&attendu["Choix"]) == entree[..nb.min(entree.len())] {
        attendu["Choix"] = json!(normalises[..nb]);
    }
    if let Some(d) = ecart(&attendu, &obtenu) {
        ecarts.push(d);
    }
    // L'écran lui-même : la page de l'enquête existe, et celle du réparateur avec une réponse finale
    let kit = etat.etat_kit(cat, None);
    if kit["pages"].get(ecrans::ENQUETE).is_none() {
        ecarts.push("pas de page « enquete »".into());
    }
    if attendu["Verdict"].is_array() && kit["pages"].get(ecrans::REPARATEUR).is_none() {
        ecarts.push("réponse finale sans page « Pour le réparateur »".into());
    }
    if ecarts.is_empty() { None } else { Some(ecarts.join("\n    ")) }
}

/// Les parcours d'un fichier : (nombre de cas, dont avec indices de Windows, dont finis par une réponse, écarts).
fn rejouer_fichier(nom: &str) -> (usize, usize, usize, Vec<String>) {
    let juge = lire_juge(nom);
    let cat = catalogue();
    let textes: Vec<String> =
        juge["Textes"].as_array().expect("Textes").iter().map(|t| t.as_str().unwrap_or_default().to_string()).collect();
    let tables: Vec<Option<Indices>> = juge["Indices"].as_array().expect("Indices").iter().map(indices).collect();
    let cas = juge["Parcours"].as_array().expect("Parcours");
    let (mut avec, mut finis, mut ecarts) = (0, 0, Vec::new());
    for c in cas {
        let panne = c["Panne"].as_str().unwrap_or_default();
        // (comme parcours.rs du moteur : un numéro de table, sinon aucun indice)
        let ind = match c["Indices"].as_i64() {
            Some(i) if i >= 0 => tables[usize::try_from(i).expect("numéro")].as_ref(),
            _ => None,
        };
        avec += usize::from(ind.is_some());
        let entree = choix(&c["Entree"]);
        let pas: Vec<(String, u64)> = c["Pas"]
            .as_array()
            .map(|p| {
                p.iter().map(|x| (x[0].as_str().unwrap_or_default().to_string(), x[1].as_u64().unwrap_or(0))).collect()
            })
            .unwrap_or_default();
        let attendu = resoudre(&c["R"], &textes);
        finis += usize::from(attendu["Verdict"].is_array());
        if let Some(d) = rejouer(cat, panne, &entree, ind, &pas, &attendu) {
            ecarts.push(format!("{nom}, {panne} {entree:?} (indices {}) :\n    {d}", c["Indices"]));
        }
    }
    (cas.len(), avec, finis, ecarts)
}

fn conclure(quoi: &str, total: usize, ecarts: &[String]) {
    assert!(total > 0, "{quoi} : aucun cas");
    assert!(
        ecarts.is_empty(),
        "{quoi} : {} écart(s) sur {total}\n{}",
        ecarts.len(),
        ecarts.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn parcours_du_powershell_rejoues_par_les_pages() {
    let mut noms: Vec<String> = std::fs::read_dir(dossier_juges())
        .expect("juges du moteur")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .filter(|n| n.starts_with("parcours-") && n.ends_with(".json"))
        .collect();
    noms.sort();
    assert!(noms.len() >= 6, "{noms:?}");
    let r: Vec<_> = std::thread::scope(|s| {
        let fils: Vec<_> = noms.iter().map(|n| s.spawn(move || rejouer_fichier(n))).collect();
        fils.into_iter().map(|f| f.join().expect("fil")).collect()
    });
    let total: usize = r.iter().map(|x| x.0).sum();
    let avec: usize = r.iter().map(|x| x.1).sum();
    let finis: usize = r.iter().map(|x| x.2).sum();
    let ecarts: Vec<String> = r.into_iter().flat_map(|x| x.3).collect();
    println!(
        "parcours rejoués par les pages : {total} (dont {avec} avec indices de Windows, {finis} jusqu'à une réponse finale)"
    );
    assert!(avec > 1000 && finis > 10_000, "{avec} avec indices, {finis} finis");
    conclure("parcours rejoués par les pages", total, &ecarts);
}

#[test]
fn cas_nommes_rejoues_par_les_pages() {
    let juge = lire_juge("cas-nommes.json");
    let cat = catalogue();
    let cas = juge["Cas"].as_array().expect("Cas");
    let mut ecarts = Vec::new();
    for c in cas {
        let ind = indices(&c["Indices"]);
        let panne = c["Panne"].as_str().unwrap_or_default();
        if let Some(d) = rejouer(cat, panne, &choix(&c["Entree"]), ind.as_ref(), &[], &c["R"]) {
            ecarts.push(format!("« {} » :\n    {d}", c["Nom"]));
        }
    }
    println!("cas nommés rejoués par les pages : {}", cas.len());
    assert!(cas.len() >= 44);
    conclure("cas nommés rejoués par les pages", cas.len(), &ecarts);
}

/// Contre-épreuves : un juge à la mauvaise réponse, une réponse changée en route, une étape attendue fausse : les
/// pages doivent le voir.
#[test]
fn un_juge_faux_se_voit() {
    let juge = lire_juge("cas-nommes.json");
    let cat = catalogue();
    let c = juge["Cas"].as_array().unwrap().iter().find(|c| c["R"]["Verdict"].is_array()).expect("un cas fini");
    let panne = c["Panne"].as_str().unwrap();
    let entree = choix(&c["Entree"]);
    let ind = indices(&c["Indices"]);
    assert_eq!(rejouer(cat, panne, &entree, ind.as_ref(), &[], &c["R"]), None, "le vrai juge passe");
    // La mauvaise réponse finale
    let mut faux = c["R"].clone();
    faux["Titre"] = json!("Une autre réponse");
    faux["Verdict"][0] = json!("reparateur");
    assert!(rejouer(cat, panne, &entree, ind.as_ref(), &[], &faux).is_some(), "mauvaise réponse non vue");
    // Une réponse donnée autrement
    let mut autre = entree.clone();
    let k = autre.iter().position(|x| *x >= 0).expect("une réponse");
    autre[k] = if autre[k] == 0 { 1 } else { 0 };
    assert!(rejouer(cat, panne, &autre, ind.as_ref(), &[], &c["R"]).is_some(), "réponse changée non vue");
    // Une étape attendue fausse au premier pas
    assert!(
        rejouer(cat, panne, &entree, ind.as_ref(), &[("une-autre-etape".into(), 0)], &c["R"]).is_some(),
        "étape fausse non vue"
    );
}
