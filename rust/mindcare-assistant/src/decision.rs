//! Ce que Mindcare fait de la réponse de l'assistant : le portage exact de la fin de `Request-Ia` et de
//! `Show-PourEtreSur` / `Select-PourEtreSur` (modules\contenu.ps1).
//!
//! - sûr (confiance d'au moins 0,95) : l'enquête commence tout de suite, avec les réponses que la phrase donne déjà
//!   (le garde-fou, Select-PreRempli, ne garde que celles dont la phrase parle vraiment) ;
//! - il hésite : la question « Pour être sûr », sur ce qu'on voit (le titre et la phrase de chaque panne), jamais
//!   une liste de pannes à deviner. Ses idées : la sienne, puis les premières de la recherche par mots ;
//! - rien de lisible : la recherche par mots reste, et la personne voit un message (jamais un bandeau vide).
//!
//! Décision de Jordan du 24/10 : jamais d'achat sur une supposition. L'assistant ne fait que choisir l'enquête ; c'est
//! l'enquête qui conclut, avec ses propres preuves.

use mindcare_moteur::ps::{champ, egal, texte};
use mindcare_moteur::{Catalogue, chercher, pre_rempli};

use crate::requete::{Idee, Reponse};

/// La confiance au-dessus de laquelle l'assistant lance l'enquête tout seul (`$IaSeuil`, mesurée dans
/// outils\ia\confiance.py : à 0,95, 1 erreur sur 122 phrases écrites à la main)
pub const SEUIL: f64 = 0.95;

/// Le message quand l'assistant n'a rien donné de lisible (Show-EchecIa)
pub const MESSAGE_ECHEC: &str = "L'assistant n'a pas réussi à lire ta phrase cette fois. Regarde les pannes ci-dessous, ou appuie encore sur Entrée pour réessayer.";
/// La bulle de la mascotte au même moment
pub const BULLE_ECHEC: &str = "Je n'ai pas compris cette fois. Regarde les pannes en dessous.";

/// Les textes de la page « Pour être sûr » (Show-PourEtreSur)
pub mod pour_etre_sur {
    pub const ETIQUETTE: &str = "POUR ÊTRE SÛR";
    pub const QUESTION: &str = "Laquelle de ces phrases ressemble le plus à ce qui se passe ?";
    pub const AUCUNE: &str = "Aucune de ces phrases";
    pub const CE_QUE_TU_AS_ECRIT: &str = "Ce que tu as écrit";
    pub const EXPLICATION: &str = "Plusieurs pannes y ressemblent : une question pour choisir la bonne enquête.";
    pub const BULLE: &str = "Je veux être sûr de bien comprendre.";
}

/// Une réponse possible de « Pour être sûr » : ce qu'on voit (le titre et la phrase de la panne), et l'enquête
/// qu'elle lance
#[derive(Clone, Debug, PartialEq)]
pub struct Choix {
    pub panne: String,
    pub titre: String,
    pub phrase: String,
    /// Les réponses déjà données par la phrase (après le garde-fou)
    pub reponses: Vec<(String, i64)>,
}

/// Ce que la page fait ensuite
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    /// L'enquête commence tout de suite (Start-Diagnostic), avec ces réponses déjà données
    Enquete { panne: String, reponses: Vec<(String, i64)> },
    /// La question « Pour être sûr » (2 ou 3 réponses, puis « Aucune de ces phrases »)
    PourEtreSur { phrase: String, choix: Vec<Choix> },
    /// Rien de lisible : la recherche par mots reste, la page montre [`MESSAGE_ECHEC`]. `cause` est déjà au journal.
    Echec { cause: String },
}

/// La décision, comme Request-Ia. `journal` reçoit les mêmes lignes que le PowerShell.
pub fn decider(cat: &Catalogue, phrase: &str, r: &Reponse, journal: &dyn Fn(&str)) -> Decision {
    if !r.erreur.is_empty() || cat.panne(&r.panne).is_none() {
        let cause = if r.erreur.is_empty() { format!("panne inconnue ({})", r.panne) } else { r.erreur.clone() };
        journal(&format!("Assistant : {cause}"));
        return Decision::Echec { cause };
    }
    let sur = r.confiance >= SEUIL;
    // (ses autres idées : les premières de la recherche par mots, plus justes que les siennes quand il hésite)
    let mut candidates: Vec<Idee> = vec![Idee { panne: r.panne.clone(), reponses: r.reponses.clone() }];
    if !sur {
        for p in chercher(cat, phrase).into_iter().take(3) {
            candidates.push(Idee { panne: texte(champ(p, "Id")), reponses: Vec::new() });
        }
        candidates.extend(r.autres.iter().cloned());
    }
    // (seulement les pannes du catalogue, chacune une fois sans tenir compte des majuscules, 3 au plus)
    let mut idees: Vec<Idee> = Vec::new();
    for i in candidates {
        if cat.panne(&i.panne).is_some() && !idees.iter().any(|x| egal(&x.panne, &i.panne)) {
            idees.push(i);
        }
    }
    idees.truncate(3);
    // (chaque idée garde seulement les réponses que la phrase dit vraiment : le garde-fou)
    for (n, i) in idees.iter_mut().enumerate() {
        let propose: Vec<&str> = i.reponses.iter().map(|(k, _)| k.as_str()).collect();
        let propose = propose.join(",");
        i.reponses = pre_rempli(cat, cat.panne(&i.panne), &i.reponses, phrase);
        let gardees: Vec<&str> = i.reponses.iter().map(|(k, _)| k.as_str()).collect();
        let sur_a =
            if n == 0 { format!(" (sûr à {} %)", (100.0 * r.confiance).round_ties_even()) } else { String::new() };
        journal(&format!(
            "Assistant : ({} caractères) -> {} {} (proposé : {propose}){sur_a}",
            phrase.encode_utf16().count(),
            i.panne,
            gardees.join(",")
        ));
    }
    // Sûr : l'enquête commence tout de suite. Il hésite : la question « Pour être sûr », sur ce qu'on voit
    if sur || idees.len() < 2 {
        let i = idees.swap_remove(0);
        return Decision::Enquete { panne: i.panne, reponses: i.reponses };
    }
    let choix: Vec<Choix> = idees
        .into_iter()
        .map(|i| {
            let p = cat.panne(&i.panne);
            Choix {
                titre: texte(p.and_then(|p| champ(p, "Titre"))),
                phrase: texte(p.and_then(|p| champ(p, "Phrase"))),
                panne: i.panne,
                reponses: i.reponses,
            }
        })
        .collect();
    let noms: Vec<&str> = choix.iter().map(|c| c.panne.as_str()).collect();
    journal(&format!("Pour être sûr : {}", noms.join(", ")));
    Decision::PourEtreSur { phrase: phrase.to_string(), choix }
}

/// La réponse choisie dans « Pour être sûr » (Select-PourEtreSur) : l'enquête à lancer, ou `None` pour « Aucune de
/// ces phrases » (retour à l'accueil, aux questions guidées)
pub fn choisir(choix: &[Choix], n: i64, journal: &dyn Fn(&str)) -> Option<(String, Vec<(String, i64)>)> {
    match usize::try_from(n).ok().and_then(|i| choix.get(i)) {
        Some(c) => {
            journal(&format!("Pour être sûr : {}", c.panne));
            Some((c.panne.clone(), c.reponses.clone()))
        }
        None => {
            journal("Pour être sûr : aucune de ces phrases");
            None
        }
    }
}
