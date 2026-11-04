//! L'accueil (ses tuiles et ses ronds) et la page « Comment ça marche ».

use serde_json::{Value, json};

use mindcare_donnees::historique::info_diagnostic;
use mindcare_moteur::Catalogue;
use mindcare_moteur::ps::{champ, vrai};

use super::outils::{nombre, t};
use super::{Etat, ecrans};

/// Les ids des tuiles qui ne mènent pas à une page : une action pour l'appli.
pub const REPRENDRE: &str = "reprendre";
pub const DERNIERE: &str = "derniere";

/// L'accueil : « Raconte ton problème », « Ça concerne quoi ? », puis la dernière enquête gardée, à reprendre si elle
/// n'est pas finie (le bandeau « Ton enquête ... n'est pas finie » du PowerShell), ou à revoir si elle l'est.
pub fn accueil(etat: &Etat, cat: &Catalogue) -> Value {
    let familles = cat.familles().len();
    // (les pannes qui concernent le PC choisi)
    let pannes = etat.trier(cat.pannes().iter().collect()).0.len();
    // (avec la dictée : « ou ta voix », la maquette validée)
    let sous = if etat.aides.branchees() { "Avec tes mots ou ta voix" } else { "Avec tes mots" };
    let mut tuiles = vec![
        json!({ "id": ecrans::RACONTE, "nom": "Raconte ton problème", "icone": "loupe", "sous": sous,
                "couleur": "P" }),
        json!({ "id": ecrans::FAMILLES, "nom": "Ça concerne quoi ?", "icone": "liste",
                "sous": format!("{}, {}", nombre(familles, "famille", "familles"), nombre(pannes, "panne", "pannes")),
                "couleur": "S" }),
    ];
    if let Some(d) = etat.historique.first() {
        let info = info_diagnostic(Some(cat), d);
        if let Some(p) = info.panne {
            let titre = t(p, "Titre");
            if vrai(champ(d, "Fini")) {
                tuiles.push(json!({ "id": DERNIERE, "nom": "Dernière enquête", "icone": "rapport", "sous": titre,
                                    "couleur": "vert" }));
            } else {
                // (la panne seulement, comme « Dernière enquête » : avec la date en plus, la ligne de la tuile devenait
                // trop petite pour être lue, capture du 03/11)
                tuiles.push(json!({ "id": REPRENDRE, "nom": "Reprendre l'enquête", "icone": "actu", "sous": titre,
                                    "couleur": "T", "alerte": true }));
            }
        }
    }
    json!({
        "tuiles": tuiles,
        "ronds": [{ "id": ecrans::AIDE, "nom": "Comment ça marche", "icone": "aide", "couleur": "T" }],
    })
}

/// « Comment ça marche » (les textes des maquettes validées).
pub fn page_aide() -> Value {
    json!({
        "titre": "Comment ça marche",
        "sous": "Mindcare trouve pourquoi ton PC est en panne, en répondant à quelques questions simples.",
        "blocs": [
            { "type": "verdict", "icone": "loupe", "couleur": "P", "surtitre": "Une enquête",
              "titre": "Une question à la fois",
              "texte": "Chaque réponse écarte des suspects, jusqu'à la bonne piste." },
            { "type": "titre", "texte": "Les questions qu'on se pose" },
            { "type": "faq", "elements": [
            { "question": "Est-ce que Mindcare répare mon PC ?",
              "reponse": "Non. Elle t'explique quoi faire, montre les commandes à taper avec un bouton Copier, mais ne lance jamais rien toute seule.",
              "icone": "bouclier" },
            { "question": "Qu'est-ce que Mindcare lit sur mon PC ?",
              "reponse": "Si tu dis que c'est ce PC-là qui a le problème : ce que Windows a noté (coupures, écrans bleus, erreurs du disque), sans rien modifier et sans droits administrateur.",
              "icone": "loupe" },
            { "question": "Mes réponses sont-elles envoyées ?",
              "reponse": "Non. Les 10 dernières enquêtes restent sur ce PC. Le rapport pour le réparateur, c'est toi qui choisis à qui le montrer.",
              "icone": "coeur" },
        ] }],
    })
}
