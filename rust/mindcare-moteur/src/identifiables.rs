//! « Chaque suspect peut être identifié » (Get-SuspectsTrouvables de outils\tester.ps1) : chaque suspect de chaque
//! panne doit pouvoir être la réponse (trouvé, ou tout le reste écarté) au bout d'au moins une suite de réponses. Les
//! suites sont essayées des plus courtes aux plus longues, 4000 au plus par panne.

use std::collections::VecDeque;

use crate::catalogue::Catalogue;
use crate::enquete::{INDICES_CONNUS, Indice, Indices, TypeVerdict, enquete};
use crate::ps::{Objet, champ, egal, tableau, texte};

/// Le nombre d'enquêtes essayées au plus pour une panne.
pub const ESSAIS: usize = 4000;

/// Les suspects d'une panne qui peuvent être la réponse, dans l'ordre où ils sont trouvés.
pub fn suspects_trouvables(cat: &Catalogue, panne: &Objet, indices: Option<&Indices>) -> Vec<String> {
    let total = tableau(champ(panne, "Suspects")).len();
    let mut trouves: Vec<String> = Vec::new();
    let mut file: VecDeque<Vec<i64>> = VecDeque::from([Vec::new()]);
    let mut n = 0;
    while n < ESSAIS && trouves.len() < total {
        let Some(choix) = file.pop_front() else { break };
        n += 1;
        let e = enquete(cat, panne, &choix, indices);
        match &e.etape {
            None => {
                if let Some(v) = &e.verdict
                    && matches!(v.genre, TypeVerdict::Trouve | TypeVerdict::Elimination)
                {
                    let s = v.suspect.clone().unwrap_or_default();
                    if !trouves.iter().any(|t| egal(t, &s)) {
                        trouves.push(s);
                    }
                }
            }
            Some(et) => {
                for k in 0..tableau(champ(et, "Reponses")).len() {
                    let mut suite = choix.clone();
                    suite.push(k as i64);
                    file.push_back(suite);
                }
            }
        }
    }
    trouves
}

/// Tous les indices de Windows « vus » (pour les suspects qui ne se trouvent que sur le PC en panne).
pub fn tous_vus() -> Indices {
    INDICES_CONNUS
        .iter()
        .map(|i| {
            let ind =
                Indice { vu: true, texte: (*i).to_string(), texte_ok: "rien de noté".to_string(), partiel: false };
            ((*i).to_string(), ind)
        })
        .collect()
}

/// Les suspects jamais identifiés (« panne : suspect »), sans indices puis, s'il en manque, avec tous les indices vus.
/// Une liste vide : chaque suspect peut être la réponse.
pub fn introuvables(cat: &Catalogue) -> Vec<String> {
    let vus = tous_vus();
    let mut res = Vec::new();
    for p in cat.pannes() {
        let t = suspects_trouvables(cat, p, None);
        let suspects: Vec<String> = tableau(champ(p, "Suspects")).into_iter().map(|s| texte(Some(s))).collect();
        let mut manque: Vec<String> = suspects.into_iter().filter(|s| !t.iter().any(|x| egal(x, s))).collect();
        if !manque.is_empty() {
            let t2 = suspects_trouvables(cat, p, Some(&vus));
            manque.retain(|s| !t2.iter().any(|x| egal(x, s)));
        }
        for m in manque {
            res.push(format!("{} : {m}", texte(champ(p, "Id"))));
        }
    }
    res
}
