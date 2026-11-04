//! « Raconte ton problème » : la recherche par mots du moteur (Find-Pannes), les plus proches d'abord ; avec
//! l'assistant et la dictée (étape 6.4, src\pages\aides.rs) : le bouton Parler dans le champ, les lignes « La dictée »
//! et « L'assistant », et « Demander à l'assistant » juste sous le champ dès qu'une phrase est écrite. Le design A2 :
//! les pannes trouvées en une liste groupée (titre, phrase, étiquette de famille, chevron).

use serde_json::{Value, json};

use mindcare_moteur::{Catalogue, chercher};

use super::Etat;
use super::aides;
use super::outils::t;

/// L'id du champ de recherche.
pub const RECHERCHE: &str = "recherche";
/// Le début de l'id d'une panne à commencer (« panne:redemarre »).
pub const PANNE: &str = "panne:";
/// Les pannes trouvées montrées au plus (Update-Accueil : les 8 premières).
pub const MONTREES: usize = 8;

pub fn page(etat: &Etat, cat: &Catalogue) -> Value {
    let a = &etat.aides;
    let indice = if a.assistant_installe() {
        aides::textes::INDICE_ASSISTANT
    } else {
        "Par exemple : « bip », « le son grésille », « écran bleu »..."
    };
    let mut champ = json!({
        "type": "champ", "id": RECHERCHE, "recherche": true, "valeur": etat.recherche, "indice": indice,
        "texte": "Ton problème",
    });
    // (le bouton Parler, dans le champ à droite : seulement avec la dictée)
    if let Some(b) = aides::bouton_parler(a) {
        champ["bouton"] = b;
    }
    let mut blocs = vec![champ];
    let texte = etat.recherche.trim();
    // (juste sous le champ, pas sous les 8 pannes : un bouton qu'il faut faire défiler pour voir est souvent manqué)
    if !texte.is_empty() {
        blocs.extend(aides::bouton_demander(etat));
    }
    blocs.extend(aides::ligne_dictee(a));
    blocs.extend(aides::ligne_assistant(a));
    if texte.is_empty() {
        blocs.push(json!({ "type": "note",
            "texte": "Écris ce qui se passe avec tes mots : les pannes qui ressemblent s'affichent juste en dessous." }));
    } else {
        // (seulement les pannes qui concernent le PC choisi ; la bande dit combien sont cachées)
        let (trouvees, cachees) = etat.trier(chercher(cat, texte));
        blocs.extend(etat.bande(&cachees));
        if trouvees.is_empty() {
            blocs.push(json!({ "type": "titre", "texte": format!("Rien ne correspond à « {texte} »") }));
            blocs.push(json!({ "type": "note",
                "texte": "Essaie d'autres mots, ou choisis ce qui est concerné dans « Ça concerne quoi ? »." }));
        } else {
            let n = trouvees.len();
            let titre = if n == 1 { "Une panne correspond".to_string() } else { format!("{n} pannes correspondent") };
            let droite = if n > MONTREES {
                format!("Les {MONTREES} plus proches")
            } else if n > 1 {
                "Les plus proches d'abord".to_string()
            } else {
                String::new()
            };
            blocs.push(json!({ "type": "titre", "texte": titre, "droite": droite }));
            for p in trouvees.into_iter().take(MONTREES) {
                let famille = cat.famille(&t(p, "Famille")).map(|f| t(f, "Titre")).unwrap_or_default();
                blocs.push(json!({
                    "type": "ligne", "id": format!("{PANNE}{}", t(p, "Id")), "texte": t(p, "Titre"),
                    "note": t(p, "Phrase"), "etiquette": famille,
                }));
            }
        }
    }
    let sous = if a.branchees() {
        "Avec tes mots ou avec ta voix : Mindcare trouve la bonne enquête."
    } else {
        "Avec tes mots : Mindcare trouve la bonne enquête."
    };
    json!({
        "titre": "Raconte ton problème",
        "sous": sous,
        "blocs": blocs,
    })
}
