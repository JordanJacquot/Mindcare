//! Le garde-fou de l'assistant (Select-PreRempli, modules\ia.ps1) : une réponse réglée d'avance par l'assistant n'est
//! gardée que si la phrase de la personne en parle vraiment (un mot de la question ou de la réponse s'y trouve). Sinon
//! la question sera posée : mieux vaut une question de plus qu'une réponse inventée.

use crate::catalogue::Catalogue;
use crate::ps::{Objet, champ, champ_v, egal, tableau, texte};
use crate::recherche::simple;

/// Les petits mots qui ne prouvent rien (« quand », « avec »...).
const VIDES: [&str; 22] = [
    "avec", "dans", "pour", "plus", "tout", "quand", "est", "elle", "il", "pas", "une", "des", "les", "que", "qui",
    "sur", "ton", "ses", "son", "mon", "oui", "non",
];

/// Les 5 premières lettres d'un mot (déjà simple : seulement a-z et 0-9).
fn debut(m: &str) -> &str {
    &m[..m.len().min(5)]
}

/// Les réponses réglées d'avance à garder : (Id de l'étape, numéro de la réponse), dans l'ordre des étapes de la panne.
/// Les Id se lisent sans tenir compte des majuscules (la table `@{}` de PowerShell).
pub fn pre_rempli(
    cat: &Catalogue,
    panne: Option<&Objet>,
    reponses: &[(String, i64)],
    phrase: &str,
) -> Vec<(String, i64)> {
    let mut garde: Vec<(String, i64)> = Vec::new();
    let Some(panne) = panne else { return garde };
    let mots: Vec<String> =
        simple(phrase).split(' ').filter(|m| m.len() >= 4 && !VIDES.contains(m)).map(str::to_string).collect();
    // (les mots de la panne elle-même ne comptent pas : « s'éteint » ne dit rien de la question « la maison s'éteint
    // aussi ? »)
    let mots_panne: Vec<String> = tableau(champ(panne, "Mots")).into_iter().map(|m| texte(Some(m))).collect();
    let de_panne_texte =
        simple(&format!("{} {} {}", texte(champ(panne, "Titre")), texte(champ(panne, "Phrase")), mots_panne.join(" ")));
    let de_panne: Vec<&str> = de_panne_texte.split(' ').filter(|m| m.len() >= 4).map(debut).collect();
    for et in cat.etapes(panne) {
        let id = texte(champ(&et, "Id"));
        let Some((_, n)) = reponses.iter().find(|(k, _)| egal(k, &id)) else { continue };
        let n = *n;
        let liste = tableau(champ(&et, "Reponses"));
        let Some(r) = usize::try_from(n).ok().and_then(|i| liste.get(i)) else { continue };
        let t = simple(&format!(
            "{} {} {}",
            texte(champ(&et, "Texte")),
            texte(champ(&et, "Court")),
            texte(champ_v(Some(r), "Texte"))
        ));
        let cles: Vec<&str> = t
            .split(' ')
            .filter(|m| m.len() >= 4 && !VIDES.contains(m))
            .map(debut)
            .filter(|c| !de_panne.contains(c))
            .collect();
        if mots.iter().any(|m| cles.iter().any(|c| m.starts_with(c))) {
            // (une table `@{}` : un Id déjà gardé, aux majuscules près, garde sa place et prend la nouvelle valeur)
            match garde.iter_mut().find(|(k, _)| egal(k, &id)) {
                Some(g) => g.1 = n,
                None => garde.push((id, n)),
            }
        }
    }
    garde
}
