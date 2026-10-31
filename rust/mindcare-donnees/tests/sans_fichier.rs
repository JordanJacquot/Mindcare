//! Les cas sans scénario, exportés depuis les fonctions des vrais modules PowerShell : dates (Format-Date), lignes
//! coupées (Get-Court), cartes de Mindset (Get-EtatCarte), modèle du PC (Get-InfosPC), rapports (Get-ResumeTexte,
//! Get-Paires), réponses en attente (Get-PairesEnAttente, Test-Reprise), indices gardés (ConvertTo-Indices), chemins
//! (Get-CheminNormal, Get-DonneesMindset, Test-GereeParMindset), Id (New-DiagId) et suite.json (Read-Suite).

mod commun;

use commun::{DossierEssai, base64, catalogue, indices, lire_juge, moment, prendre, s, verdict_final};
use mindcare_donnees::dossier::{Suite, chemin_normal, donnees_mindset, gere_par_mindset, lire_suite};
use mindcare_donnees::enquete_en_cours::{EnqueteEnCours, PreRempli, Reprise, nouvel_id};
use mindcare_donnees::historique::convertir_indices;
use mindcare_donnees::rapport::LecturePc;
use mindcare_donnees::{
    Journal, SurLePc, court, etat_carte, format_date, horloge_figee, infos_pc, paires, texte_rapport,
};
use mindcare_moteur::enquete;
use serde_json::{Value, json};

fn cas(nom: &str) -> Vec<Value> {
    lire_juge(nom)["Cas"].as_array().cloned().unwrap_or_default()
}

fn opt(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

#[test]
fn dates() {
    let l = cas("dates.json");
    let mut ecarts = Vec::new();
    for c in &l {
        let iso = s(&c["Iso"]);
        for (champ, annee) in [("Courte", false), ("Longue", true)] {
            let r = format_date(iso, annee);
            if r != s(&c[champ]) {
                ecarts.push(format!("« {iso} » ({champ}) : PowerShell « {} » / Rust « {r} »", s(&c[champ])));
            }
        }
    }
    verdict_final("dates", l.len(), &ecarts);
}

#[test]
fn lignes_coupees() {
    let l = cas("courts.json");
    let mut ecarts = Vec::new();
    for c in &l {
        let r = court(s(&c["Texte"]), c["Max"].as_u64().unwrap_or(80) as usize);
        if r != s(&c["R"]) {
            ecarts.push(format!(
                "« {} » ({}) : PowerShell « {} » / Rust « {r} »",
                s(&c["Texte"]),
                c["Max"],
                s(&c["R"])
            ));
        }
    }
    verdict_final("lignes coupées", l.len(), &ecarts);
}

#[test]
fn cartes() {
    let l = cas("cartes.json");
    let mut ecarts = Vec::new();
    for c in &l {
        let historique: Vec<_> = match c["Diagnostic"].as_str() {
            Some(t) => vec![serde_json::from_str::<Value>(t).expect("Diagnostic").as_object().cloned().expect("objet")],
            None => Vec::new(),
        };
        let cat = if c["Catalogue"].as_bool().unwrap_or(true) { Some(catalogue()) } else { None };
        let r = etat_carte(cat, &historique).json();
        if r != c["R"] {
            ecarts.push(format!("{} :\n    PowerShell {}\n    Rust {r}", c["Diagnostic"], c["R"]));
        }
    }
    verdict_final("cartes", l.len(), &ecarts);
}

#[test]
fn modele_du_pc() {
    let l = cas("infos-pc.json");
    let mut ecarts = Vec::new();
    for c in &l {
        let d = DossierEssai::nouveau("infos");
        let journal = Journal::ouvrir(&d.0.join("journal.txt"), horloge_figee(moment("2026-10-30T14:05:00")));
        let lec = &c["Lecture"];
        let lecture =
            LecturePc { fabricant: opt(&lec["Fabricant"]), modele: opt(&lec["Modele"]), systeme: opt(&lec["Systeme"]) };
        let erreur = format!("Classe {} illisible", s(&lec["Erreur"]));
        let r = if s(&lec["Erreur"]).is_empty() {
            infos_pc(Ok(&lecture), &journal)
        } else {
            infos_pc(Err(&erreur), &journal)
        };
        let lignes: Vec<String> = std::fs::read_to_string(journal.fichier())
            .unwrap_or_default()
            .lines()
            .map(|l| l.get(20..).unwrap_or_default().to_string())
            .collect();
        if r != s(&c["R"]) || json!(lignes) != c["Journal"] {
            ecarts.push(format!("{lec} : PowerShell « {} » {} / Rust « {r} » {lignes:?}", s(&c["R"]), c["Journal"]));
        }
    }
    verdict_final("modèle du PC", l.len(), &ecarts);
}

#[test]
fn rapports() {
    let cat = catalogue();
    let l = cas("rapports.json");
    let mut ecarts = Vec::new();
    for c in &l {
        let p = cat.panne(s(&c["Panne"])).expect("panne");
        let entree: Vec<i64> = c["Entree"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
        let ind = if c["Indices"].is_null() { None } else { Some(indices(&c["Indices"]).moteur()) };
        let e = enquete(cat, p, &entree, ind.as_ref());
        let jour = indices(&c["IndicesJour"]);
        let barrettes: Vec<String> =
            c["Barrettes"].as_array().into_iter().flatten().map(|b| s(b).to_string()).collect();
        let pc = SurLePc { infos: s(&c["InfosPc"]), barrettes: &barrettes, indices_du_jour: Some(&jour) };
        let r = texte_rapport(cat, p, &e, s(&c["Titre"]), &moment(s(&c["Maintenant"])), &pc, s(&c["Version"]));
        if r != s(&c["R"]) {
            ecarts.push(format!("{} {:?} :\n--- PowerShell\n{}\n--- Rust\n{r}", s(&c["Panne"]), entree, s(&c["R"])));
        }
        let pr: Vec<Value> = paires(&e).iter().map(|x| json!([x.etape, x.reponse])).collect();
        if json!(pr) != c["Paires"] {
            ecarts.push(format!("{} {:?} : paires PowerShell {} / Rust {pr:?}", s(&c["Panne"]), entree, c["Paires"]));
        }
    }
    verdict_final("rapports", l.len(), &ecarts);
}

#[test]
fn reponses_en_attente() {
    let cat = catalogue();
    let l = cas("attente.json");
    let mut ecarts = Vec::new();
    for c in &l {
        let p = cat.panne(s(&c["Panne"])).expect("panne");
        let entree: Vec<i64> = c["Entree"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
        let e = enquete(cat, p, &entree, None);
        let reprise = c["Reprise"].as_str().map(|t| Reprise {
            id: "r1".to_string(),
            reponses: serde_json::from_str::<Value>(t).expect("Reprise").as_array().cloned().unwrap_or_default(),
        });
        let pre_rempli = c["PreRempli"].as_array().map(|k| {
            let mut pr = PreRempli::default();
            for x in k {
                pr.mettre(s(x), 0);
            }
            pr
        });
        let session = EnqueteEnCours {
            panne: Some(p.clone()),
            diag_id: s(&c["DiagId"]).to_string(),
            pre_rempli,
            reprise,
            ..EnqueteEnCours::default()
        };
        let r: Vec<Value> = session.paires_en_attente(cat, &e).iter().map(|x| json!([x.etape, x.reponse])).collect();
        if json!(r) != c["R"] || json!(session.est_reprise()) != c["TestReprise"] {
            ecarts.push(format!(
                "{} {:?} reprise {} : PowerShell {} {} / Rust {r:?} {}",
                s(&c["Panne"]),
                entree,
                c["Reprise"],
                c["R"],
                c["TestReprise"],
                session.est_reprise()
            ));
        }
    }
    verdict_final("réponses en attente", l.len(), &ecarts);
}

#[test]
fn indices_gardes_relus() {
    let l = cas("indices.json");
    let mut ecarts = Vec::new();
    for c in &l {
        let g: Value = serde_json::from_str(s(&c["Gardes"])).expect("Gardes");
        let t = convertir_indices(Some(&g));
        let mut r: Vec<(String, Value)> =
            t.0.iter().map(|(k, v)| (k.clone(), json!([k, v.vu, v.texte, v.texte_ok, v.affiche, v.partiel]))).collect();
        r.sort_by_key(|(k, _)| k.to_lowercase());
        let r: Vec<Value> = r.into_iter().map(|(_, v)| v).collect();
        if json!(r) != c["R"] {
            ecarts.push(format!("{} : PowerShell {} / Rust {r:?}", s(&c["Gardes"]), c["R"]));
        }
    }
    verdict_final("indices gardés", l.len(), &ecarts);
}

#[test]
fn chemins_et_id() {
    let l = cas("chemins.json");
    let mut ecarts = Vec::new();
    for c in &l {
        let entree = s(&c["Entree"]);
        let r = match s(&c["Fonction"]) {
            "Get-CheminNormal" => json!(chemin_normal(entree)),
            "Get-DonneesMindset" => {
                json!(donnees_mindset(if entree.is_empty() { None } else { Some(entree) }, s(&c["Local"])))
            }
            "Test-GereeParMindset" => {
                let suite = Suite {
                    dossier: opt(&c["Suite"]["Dossier"]),
                    pris: c["Suite"]["Pris"].as_bool().unwrap_or(false),
                    ..Suite::default()
                };
                let pf = [c["ProgramFiles"].as_str(), c["ProgramW6432"].as_str()];
                json!(gere_par_mindset(&suite, Some(entree), &pf))
            }
            "New-DiagId" => json!(nouvel_id(&moment(s(&c["Maintenant"])), c["Hasard"].as_u64().unwrap_or(0) as u16)),
            autre => panic!("fonction inconnue : {autre}"),
        };
        if r != c["R"] {
            ecarts.push(format!(
                "{} « {entree} » {} : PowerShell {} / Rust {r}",
                s(&c["Fonction"]),
                c["Suite"],
                c["R"]
            ));
        }
    }
    verdict_final("chemins et Id", l.len(), &ecarts);
}

#[test]
fn suite_json() {
    let l = cas("suites.json");
    let mut ecarts = Vec::new();
    for c in &l {
        let d = DossierEssai::nouveau("suite");
        let f = d.0.join("suite.json");
        if let Some(b) = c["Contenu"].as_str() {
            std::fs::write(&f, base64(b)).expect("suite.json");
        }
        let verrou = if c["Pris"].as_bool().unwrap_or(false) { Some(prendre(&f)) } else { None };
        let su = lire_suite(Some(&f), 2);
        drop(verrou);
        let r = json!({
            "Existe": su.existe, "Lisible": su.lisible, "Pris": su.pris, "Theme": su.theme, "Dossier": su.dossier, "Micro": su.micro
        });
        if r != c["R"] {
            ecarts.push(format!("{} : PowerShell {} / Rust {r}", c["Contenu"], c["R"]));
        }
    }
    verdict_final("suite.json", l.len(), &ecarts);
}
