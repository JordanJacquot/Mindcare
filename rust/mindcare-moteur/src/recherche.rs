//! La recherche par mots (Find-Pannes), sans internet, toujours les mêmes résultats : les pannes dont le nom, la phrase
//! ou les mots contiennent les mots tapés, les plus proches d'abord.
//!
//! Les pannes trouvées sont toujours celles de Find-Pannes (le moteur PowerShell). Leur ordre ne l'est plus depuis le
//! 05/11/2026 : « il s'éteint tout seul » mettait « Il s'éteint d'un coup » seulement 6e, derrière « L'écran s'éteint
//! trop vite ». Le classement est expliqué dans [`score_classement`] ; l'écart voulu avec PowerShell, et les phrases de
//! tous les jours qui le mesurent, sont dans tests\divergences.rs et tests\phrases.rs.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::catalogue::Catalogue;
use crate::pc::{ProfilPc, Tri, trier};
use crate::ps::{Objet, champ, tableau, texte};

/// Les petits mots qui ne disent rien de la panne (« mon pc ne démarre plus » : seulement « demarre »).
#[rustfmt::skip]
const VIDES: [&str; 35] = [
    "il", "le", "la", "les", "un", "une", "de", "du", "des", "ne", "pas", "plus", "mon", "ma", "mes", "pc", "ordi",
    "ordinateur", "et", "ou", "est", "en", "sur", "qui", "que", "se", "ca", "tout", "au", "aux", "je", "j", "me", "quand",
    "avec",
];

/// Les mots qui insistent sans rien dire de la panne (« il chauffe trop », « il est très lent », « j'ai », « tout le
/// temps ») : ils servent à trouver les pannes, mais ne comptent pas pour les classer.
#[rustfmt::skip]
const FAIBLES: [&str; 13] = [
    "tres", "beaucoup", "trop", "fait", "ai", "vraiment", "super", "hyper", "grave", "completement", "encore", "devenu",
    "temps",
];

/// Les mots qui disent que quelque chose ne marche pas (« ne s'éteint pas », « plus de son »).
const NEGATIONS: [&str; 8] = ["ne", "pas", "plus", "rien", "jamais", "n", "aucun", "aucune"];

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

/// La racine d'un mot tapé : sans sa dernière lettre s'il a plus de 5 lettres (« redemarre » trouve « redemarrage »).
fn racine(m: &str) -> &str {
    if m.len() > 5 { &m[..m.len() - 1] } else { m }
}

/// Un texte simple dit-il que quelque chose ne marche pas ?
fn negatif(t: &str) -> bool {
    t.split(' ').any(|m| NEGATIONS.contains(&m))
}

/// Les mots qui comptent dans un texte simple pour le classement (ni petits mots, ni mots qui insistent).
fn mots_forts(t: &str) -> impl Iterator<Item = &str> {
    t.split(' ').filter(|m| m.len() >= 2 && !VIDES.contains(m) && !FAIBLES.contains(m))
}

/// Une panne mise en texte simple pour la recherche (préparée une fois par catalogue, voir `Catalogue::fiches`).
#[derive(Debug, Clone)]
pub(crate) struct Fiche {
    titre: String,
    /// Tout son texte : le nom, la phrase, les mots, le nom de la famille.
    tout: String,
    phrase: String,
    famille: String,
    /// Ses mots (le champ « Mots »), chacun en texte simple.
    mots: Vec<String>,
    /// Ses mots à la suite.
    mots_joints: String,
}

pub(crate) fn fiches(cat: &Catalogue) -> Vec<Fiche> {
    cat.pannes()
        .iter()
        .map(|p| {
            let titre = texte(champ(p, "Titre"));
            let phrase = texte(champ(p, "Phrase"));
            let famille = texte(cat.famille(&texte(champ(p, "Famille"))).and_then(|f| champ(f, "Titre")));
            let mots: Vec<String> = tableau(champ(p, "Mots")).into_iter().map(|m| texte(Some(m))).collect();
            Fiche {
                tout: simple(&format!("{titre} {phrase} {} {famille}", mots.join(" "))),
                titre: simple(&titre),
                phrase: simple(&phrase),
                famille: simple(&famille),
                mots_joints: simple(&mots.join(" ")),
                mots: mots.iter().map(|m| simple(m)).collect(),
            }
        })
        .collect()
}

/// Le score de Find-Pannes (le moteur PowerShell) : 3 par mot tapé dans le nom, 2 ailleurs, plus la phrase entière
/// (6 dans le nom, 4 ailleurs). Il choisit les pannes trouvées (score non nul), et donnait leur ordre avant.
fn score_trouvee(f: &Fiche, mots: &[String], phrase: &str) -> i64 {
    let mut score = 0i64;
    for m in mots {
        let r = racine(m);
        if debut_de_mot(&f.titre, r) {
            score += 3;
        } else if debut_de_mot(&f.tout, r) {
            score += 2;
        }
    }
    // La phrase tapée telle quelle (« pas de son ») compte plus que des mots épars
    if phrase.len() >= 4 {
        if f.titre.contains(phrase) {
            score += 6;
        } else if f.tout.contains(phrase) {
            score += 4;
        }
    }
    score
}

/// Le score qui classe les pannes trouvées, en nombres entiers (le téléphone calcule exactement le même) :
/// - chaque mot tapé pèse `2000 / (n + 5)`, n étant le nombre de pannes qui l'ont quelque part : un mot rare compte
///   plus (« eteint », dans 18 pannes, plus que « seul », dans 42) ; il compte 3 fois son poids dans le nom ou les
///   mots de la panne, 2 fois dans sa phrase ou le nom de sa famille (les mots qui insistent ne comptent pas) ;
/// - la phrase tapée telle quelle, comme avant : 600 dans le nom, 400 ailleurs ;
/// - la part du nom de la panne couverte par les mots tapés, jusqu'à 300 : « il s'éteint tout seul » couvre la moitié
///   de « Il s'éteint d'un coup », mais seulement le quart de « L'écran s'éteint trop vite » ;
/// - 200 de moins quand l'un dit que ça ne marche pas et l'autre non (« Il ne s'éteint pas » pour « il s'éteint tout
///   seul »), sauf pour un seul mot tapé sans négation (« casque » ne dit pas si le casque marche) ;
/// - 100 par mot fort d'un des mots de la panne écrit en entier dans la phrase tapée (« s'éteint », « bruit »).
fn score_classement(f: &Fiche, mots: &[String], phrase: &str, nombres: &[i64]) -> i64 {
    let racines: Vec<(&str, i64)> = mots
        .iter()
        .zip(nombres)
        .filter(|(m, _)| !FAIBLES.contains(&m.as_str()))
        .map(|(m, n)| (racine(m), 2000 / (n + 5)))
        .collect();
    let mut score = 0i64;
    for (r, poids) in &racines {
        if debut_de_mot(&f.titre, r) || debut_de_mot(&f.mots_joints, r) {
            score += 3 * poids;
        } else if debut_de_mot(&f.phrase, r) || debut_de_mot(&f.famille, r) {
            score += 2 * poids;
        }
    }
    if phrase.len() >= 4 {
        if f.titre.contains(phrase) {
            score += 600;
        } else if f.tout.contains(phrase) {
            score += 400;
        }
    }
    let du_nom: Vec<&str> = mots_forts(&f.titre).collect();
    if !du_nom.is_empty() {
        let couverts = du_nom.iter().filter(|m| racines.iter().any(|(r, _)| m.starts_with(r))).count();
        score += 300 * couverts as i64 / du_nom.len() as i64;
    }
    let tape_negatif = negatif(phrase);
    if tape_negatif != negatif(&f.titre) && (tape_negatif || mots.len() >= 2) {
        score -= 200;
    }
    let entouree = format!(" {phrase} ");
    let ecrits = f
        .mots
        .iter()
        .filter(|m| m.len() >= 4 && entouree.contains(&format!(" {m} ")))
        .map(|m| mots_forts(m).count() as i64)
        .max()
        .unwrap_or(0);
    score + 100 * ecrits
}

/// Les pannes trouvées pour ces mots, avec leur place dans le catalogue, avant le tri.
fn trouvees<'c>(fiches: &'c [Fiche], mots: &[String], phrase: &str) -> Vec<(usize, &'c Fiche)> {
    fiches.iter().enumerate().filter(|(_, f)| score_trouvee(f, mots, phrase) != 0).collect()
}

/// Les pannes qui correspondent aux mots tapés, les plus proches d'abord (à score égal, l'ordre du catalogue).
pub fn chercher<'a>(cat: &'a Catalogue, requete: &str) -> Vec<&'a Objet> {
    let mots = mots_utiles(requete);
    if mots.is_empty() {
        return Vec::new();
    }
    let phrase = simple(requete);
    let fiches = cat.fiches();
    let nombres: Vec<i64> =
        mots.iter().map(|m| fiches.iter().filter(|f| debut_de_mot(&f.tout, racine(m))).count() as i64).collect();
    let mut res: Vec<(i64, usize)> = trouvees(fiches, &mots, &phrase)
        .into_iter()
        .map(|(ordre, f)| (score_classement(f, &mots, &phrase, &nombres), ordre))
        .collect();
    res.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    res.into_iter().map(|(_, ordre)| &cat.pannes()[ordre]).collect()
}

/// Les mêmes pannes que [`chercher`], dans l'ordre de Find-Pannes (le moteur PowerShell) : seulement pour vérifier,
/// dans les tests, que les pannes trouvées n'ont pas changé (tests\recherches.rs).
#[doc(hidden)]
pub fn chercher_comme_powershell<'a>(cat: &'a Catalogue, requete: &str) -> Vec<&'a Objet> {
    let mots = mots_utiles(requete);
    if mots.is_empty() {
        return Vec::new();
    }
    let phrase = simple(requete);
    let fiches = cat.fiches();
    let mut res: Vec<(i64, usize)> = trouvees(fiches, &mots, &phrase)
        .into_iter()
        .map(|(ordre, f)| (score_trouvee(f, &mots, &phrase), ordre))
        .collect();
    res.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    res.into_iter().map(|(_, ordre)| &cat.pannes()[ordre]).collect()
}

/// La recherche par mots pour ce PC : les pannes trouvées qui le concernent (même ordre que [`chercher`]), et combien
/// de pannes trouvées sont cachées parce qu'elles ne le concernent pas (« PourPc », voir [`crate::pc`]).
pub fn chercher_pour<'a>(cat: &'a Catalogue, requete: &str, profil: &ProfilPc) -> Tri<'a> {
    trier(chercher(cat, requete), profil)
}
