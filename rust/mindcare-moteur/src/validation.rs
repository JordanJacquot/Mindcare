//! Ce qui ne va pas dans le catalogue (Test-Enquetes) : une liste vide, tout est bon. Les messages sont exactement ceux
//! de PowerShell, dans le même ordre.

use std::collections::HashSet;

use serde_json::Value;

use crate::catalogue::Catalogue;
use crate::enquete::INDICES_CONNUS;
use crate::ps::{
    champ, champ_v, cle, contient, contient_valeur, egal, egal_valeurs, parcours, tableau, texte, vrai, vrai_liste,
};

/// Les guillemets qui cassent une commande copiée dans Windows.
const GUILLEMETS: [char; 6] = ['\u{ab}', '\u{bb}', '\u{201c}', '\u{201d}', '\u{2018}', '\u{2019}'];

/// Vérifie le catalogue lu, et le texte du fichier lui-même (pas de tiret long).
pub fn verifier(cat: &Catalogue, texte_fichier: &str) -> Vec<String> {
    let mut pb: Vec<String> = Vec::new();
    if texte_fichier.contains(['\u{2013}', '\u{2014}']) {
        pb.push("Tiret long dans le fichier".to_string());
    }
    let suspects: Vec<String> = cat.suspects().keys().cloned().collect();
    for (nom, s) in cat.suspects() {
        let manque = ["Titre", "Trouve", "Pourquoi", "AFaire", "Pro"].iter().any(|c| !vrai(champ_v(Some(s), c)));
        if manque {
            pb.push(format!("Suspect {nom} : Titre, Trouve, Pourquoi, AFaire ou Pro manquant"));
        }
    }
    let familles: Vec<String> = cat.familles().iter().map(|f| texte(champ(f, "Id"))).collect();
    let mut ids: Vec<Value> = Vec::new();
    for p in cat.pannes() {
        let id_panne = champ(p, "Id").cloned().unwrap_or(Value::Null);
        let nom = format!("Panne {}", texte(Some(&id_panne)));
        if ids.iter().any(|i| egal_valeurs(i, &id_panne)) {
            pb.push(format!("{nom} : en double"));
        }
        ids.push(id_panne);
        if !vrai(champ(p, "Titre")) || !vrai(champ(p, "Phrase")) {
            pb.push(format!("{nom} : Titre ou Phrase manquant"));
        }
        let famille = texte(champ(p, "Famille"));
        if !familles.iter().any(|f| egal(f, &famille)) {
            pb.push(format!("{nom} : famille inconnue \u{ab} {famille} \u{bb}"));
        }
        for s in parcours(champ(p, "Suspects")) {
            if !contient(&suspects, s) {
                pb.push(format!("{nom} : suspect inconnu \u{ab} {} \u{bb}", texte(Some(s))));
            }
        }
        for e in parcours(champ(p, "Etapes")) {
            let reference = champ_v(Some(e), "Ref");
            if vrai(reference) && !vrai(cat.test(&texte(reference))) {
                pb.push(format!("{nom} : test partagé inconnu \u{ab} {} \u{bb}", texte(reference)));
            }
        }
        let suspects_panne = tableau(champ(p, "Suspects"));
        let absent = |s: &Value| vrai(Some(s)) && !contient_valeur(&suspects_panne, s);
        let mut vus: HashSet<String> = HashSet::new();
        for e in cat.etapes(p) {
            let id = texte(champ(&e, "Id"));
            let ou = format!("{nom}, étape {id}");
            if !vrai(champ(&e, "Id")) {
                pb.push(format!("{nom} : une étape sans Id"));
            } else if vus.contains(&cle(&id)) {
                pb.push(format!("{ou} : en double"));
            }
            vus.insert(cle(&id));
            let genre = champ(&e, "Type").cloned().unwrap_or(Value::Null);
            if !["question", "test", "auto"].iter().any(|t| egal_valeurs(&Value::String((*t).to_string()), &genre)) {
                pb.push(format!("{ou} : type inconnu \u{ab} {} \u{bb}", texte(Some(&genre))));
            }
            for s in tableau(champ(&e, "Pour")) {
                if absent(s) {
                    pb.push(format!("{ou} : suspect \u{ab} {} \u{bb} absent de la panne", texte(Some(s))));
                }
            }
            // (les effets : SiVu et SiNonVu d'une vérification automatique, sinon les réponses)
            let effets: Vec<&Value> = if egal(&texte(Some(&genre)), "auto") {
                let indice = texte(champ(&e, "Indice"));
                if !INDICES_CONNUS.iter().any(|i| egal(i, &indice)) {
                    pb.push(format!("{ou} : indice de Windows inconnu \u{ab} {indice} \u{bb}"));
                }
                [champ(&e, "SiVu"), champ(&e, "SiNonVu")].into_iter().flatten().filter(|v| vrai(Some(v))).collect()
            } else {
                if !vrai(champ(&e, "Texte")) || !vrai(champ(&e, "Court")) {
                    pb.push(format!("{ou} : Texte ou Court manquant"));
                }
                if tableau(champ(&e, "Reponses")).len() < 2 {
                    pb.push(format!("{ou} : moins de 2 réponses"));
                }
                if egal(&texte(Some(&genre)), "test") && !vrai(champ(&e, "Gestes")) {
                    pb.push(format!("{ou} : un test sans gestes"));
                }
                let commande = champ(&e, "Commande");
                if vrai(commande) {
                    let lignes = tableau(champ_v(commande, "Lignes"));
                    if !vrai_liste(&lignes) {
                        pb.push(format!("{ou} : une commande sans lignes"));
                    }
                    for l in &lignes {
                        let l = texte(Some(l));
                        if l.contains(GUILLEMETS) {
                            pb.push(format!("{ou} : guillemets français dans la commande \u{ab} {l} \u{bb}"));
                        }
                    }
                    let copier = tableau(champ(&e, "Gestes"))
                        .into_iter()
                        .filter(|g| !g.is_null() && texte(Some(g)).to_lowercase().contains("copier"))
                        .collect::<Vec<_>>();
                    if !vrai_liste(&copier) {
                        pb.push(format!("{ou} : les gestes ne disent pas comment copier la commande"));
                    }
                }
                for r in parcours(champ(&e, "Reponses")) {
                    if !vrai(champ_v(Some(r), "Texte")) {
                        pb.push(format!("{ou} : une réponse sans texte"));
                    }
                }
                tableau(champ(&e, "Reponses"))
            };
            for f in effets {
                let mut cites = tableau(champ_v(Some(f), "Ecarte"));
                cites.extend(tableau(champ_v(Some(f), "Garde")));
                cites.extend(tableau(champ_v(Some(f), "Confirme")));
                for s in cites {
                    if absent(s) {
                        pb.push(format!("{ou} : suspect \u{ab} {} \u{bb} absent de la panne", texte(Some(s))));
                    }
                }
            }
        }
    }
    for f in cat.familles() {
        if ["Id", "Titre", "Phrase", "Icone"].iter().any(|c| !vrai(champ(f, c))) {
            pb.push(format!("Famille {} : Id, Titre, Phrase ou Icone manquant", texte(champ(f, "Id"))));
        }
    }
    if !vrai_liste(&ids.iter().collect::<Vec<_>>()) {
        pb.push("Aucune panne".to_string());
    }
    pb
}
