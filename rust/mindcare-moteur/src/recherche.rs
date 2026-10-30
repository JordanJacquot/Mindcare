//! La recherche par mots (Find-Pannes), sans internet, toujours les mêmes résultats : les pannes dont le nom, la phrase
//! ou les mots contiennent les mots tapés, les plus proches d'abord.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::catalogue::Catalogue;
use crate::ps::{Objet, champ, tableau, texte};

/// Les petits mots qui ne disent rien de la panne (« mon pc ne démarre plus » : seulement « demarre »).
#[rustfmt::skip]
const VIDES: [&str; 35] = [
    "il", "le", "la", "les", "un", "une", "de", "du", "des", "ne", "pas", "plus", "mon", "ma", "mes", "pc", "ordi",
    "ordinateur", "et", "ou", "est", "en", "sur", "qui", "que", "se", "ca", "tout", "au", "aux", "je", "j", "me", "quand",
    "avec",
];

/// Sans accents, en minuscules, seulement des lettres a-z, des chiffres et des espaces simples (ConvertTo-Simple).
///
/// Comme .NET : la forme décomposée (FormD), les signes combinants retirés, les minuscules, puis tout ce qui n'est pas
/// a-z, 0-9 ou une espace devient une espace. (Seul écart connu : .NET ne retire que les signes « Mn » ; les rares
/// signes « Mc » et « Me », des écritures indiennes par exemple, sont retirés ici au lieu de devenir une espace.)
pub fn simple(t: &str) -> String {
    let mut sortie = String::with_capacity(t.len());
    for c in t.nfd().filter(|c| !is_combining_mark(*c)) {
        for m in c.to_lowercase() {
            sortie.push(if m.is_ascii_lowercase() || m.is_ascii_digit() || m == ' ' { m } else { ' ' });
        }
    }
    sortie.split(' ').filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Le texte contient-il ce mot au début d'un mot (`-match "\bmot"` sur un texte déjà simple) ?
fn debut_de_mot(t: &str, racine: &str) -> bool {
    t.match_indices(racine).any(|(i, _)| i == 0 || t.as_bytes()[i - 1] == b' ')
}

/// Les mots qui comptent dans une recherche (au moins 2 lettres, pas un petit mot).
pub fn mots_utiles(requete: &str) -> Vec<String> {
    simple(requete).split(' ').filter(|m| m.len() >= 2 && !VIDES.contains(m)).map(str::to_string).collect()
}

/// Les pannes qui correspondent aux mots tapés, les plus proches d'abord (à score égal, l'ordre du catalogue).
pub fn chercher<'a>(cat: &'a Catalogue, requete: &str) -> Vec<&'a Objet> {
    let mots = mots_utiles(requete);
    if mots.is_empty() {
        return Vec::new();
    }
    let phrase = simple(requete);
    let mut res: Vec<(&Objet, i64, usize)> = Vec::new();
    for (ordre, p) in cat.pannes().iter().enumerate() {
        let fam = cat.famille(&texte(champ(p, "Famille")));
        let titre = simple(&texte(champ(p, "Titre")));
        let mots_panne: Vec<String> = tableau(champ(p, "Mots")).into_iter().map(|m| texte(Some(m))).collect();
        let tout = simple(&format!(
            "{} {} {} {}",
            texte(champ(p, "Titre")),
            texte(champ(p, "Phrase")),
            mots_panne.join(" "),
            texte(fam.and_then(|f| champ(f, "Titre")))
        ));
        let mut score = 0i64;
        for m in &mots {
            // (« redemarre » trouve « redemarrage »)
            let racine = if m.len() > 5 { &m[..m.len() - 1] } else { m.as_str() };
            if debut_de_mot(&titre, racine) {
                score += 3;
            } else if debut_de_mot(&tout, racine) {
                score += 2;
            }
        }
        // La phrase tapée telle quelle (« pas de son ») compte plus que des mots épars
        if phrase.len() >= 4 {
            if titre.contains(&phrase) {
                score += 6;
            } else if tout.contains(&phrase) {
                score += 4;
            }
        }
        if score != 0 {
            res.push((p, score, ordre));
        }
    }
    res.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
    res.into_iter().map(|(p, _, _)| p).collect()
}
