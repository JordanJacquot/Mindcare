//! La phrase de la réponse finale (Get-VerdictTitre) : « Une barrette de mémoire (RAM) est abîmée », « Il chauffe trop :
//! un ventilateur ne tourne plus », « Il reste la mémoire ou l'alimentation : un réparateur départagera ».

use crate::catalogue::Catalogue;
use crate::enquete::{TypeVerdict, Verdict};
use crate::ps::{champ, texte};
use crate::recherche::simple;

/// Un nom en minuscule au début (« la mémoire (RAM) »), sans toucher aux noms propres (Windows, RAM).
pub fn minuscule(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(premier) => premier.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

/// La phrase de la réponse finale (vide tant que l'enquête n'est pas finie).
pub fn titre_verdict(cat: &Catalogue, verdict: Option<&Verdict>) -> String {
    let Some(v) = verdict else { return String::new() };
    let champ_suspect =
        |id: Option<&String>, nom: &str| texte(id.and_then(|id| cat.suspect(id)).and_then(|s| champ(s, nom)));
    match v.genre {
        TypeVerdict::Trouve => {
            let trouve = champ_suspect(v.suspect.as_ref(), "Trouve");
            if v.detail.is_empty() { trouve } else { format!("{trouve} : {}", v.detail) }
        }
        TypeVerdict::Elimination => champ_suspect(v.suspect.as_ref(), "Trouve"),
        TypeVerdict::Reparateur => {
            let noms: Vec<String> = v.restants.iter().map(|id| minuscule(&champ_suspect(Some(id), "Titre"))).collect();
            format!("Il reste {} : un réparateur départagera", noms.join(" ou "))
        }
        TypeVerdict::Rien => "Aucune piste ne correspond : montre ce rapport à un réparateur".to_string(),
    }
}

/// Un texte avec une majuscule au début (« la batterie » : « La batterie »).
pub fn majuscule(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(premier) => premier.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Ce texte simple commence-t-il par ces mots (des mots entiers) ?
fn commence_par(t: &str, debut: &str) -> bool {
    !debut.is_empty() && (t == debut || t.strip_prefix(debut).is_some_and(|r| r.starts_with(' ')))
}

/// Les articles qui ouvrent un nom (« la batterie », « un pilote »), en texte simple.
const ARTICLES: [&str; 7] = ["le", "la", "l", "les", "un", "une", "des"];

/// La phrase de la réponse à montrer quand une réponse a confirmé le suspect avec un détail (son « Trouve » et le
/// « Detail » de la réponse). [`titre_verdict`] les colle (« Trouve : Detail », comme Get-VerdictTitre, que les
/// tests d'équivalence comparent) ; cela répétait parfois ce que le détail précise :
/// - « La batterie (ou le chargeur) lâche » et « la batterie » : « La batterie lâche » (le détail choisit entre les
///   deux du « (ou ...) ») ;
/// - « Le courant coupe : la prise, la multiprise ou l'électricité de la maison » et « la multiprise » : « Le courant
///   coupe : la multiprise » (le détail choisit dans la liste) ;
/// - « Un pilote de Windows fait planter le PC » et « le pilote de la carte graphique manque » : le détail seul, qui
///   parle de la même chose en plus précis ;
/// - « Danger : une pièce a peut-être grillé » et « le chargeur » : le détail entre parenthèses (pas deux « : ») ;
/// - sinon, comme avant : « Il chauffe trop : la poussière ».
pub fn titre_avec_detail(trouve: &str, detail: &str) -> String {
    let d = simple(detail);
    if d.is_empty() {
        return trouve.to_string();
    }
    if let Some(i) = trouve.find(" (ou ")
        && let Some(j) = trouve[i..].find(')').map(|j| i + j)
    {
        let (a, b, reste) = (&trouve[..i], &trouve[i + " (ou ".len()..j], &trouve[j + 1..]);
        if d == simple(a) || commence_par(&d, &simple(b)) {
            return format!("{}{reste}", majuscule(detail));
        }
    }
    if let Some((intro, liste)) = trouve.split_once(" : ")
        && liste.split(", ").flat_map(|x| x.split(" ou ")).any(|x| simple(x) == d)
    {
        return format!("{intro} : {detail}");
    }
    let t = simple(trouve);
    let (mt, md): (Vec<&str>, Vec<&str>) = (t.split(' ').collect(), d.split(' ').collect());
    if mt.len() >= 2 && md.len() >= 3 && ARTICLES.contains(&mt[0]) && ARTICLES.contains(&md[0]) && mt[1] == md[1] {
        return majuscule(detail);
    }
    if trouve.contains(" : ") { format!("{trouve} ({detail})") } else { format!("{trouve} : {detail}") }
}

/// La phrase de la réponse à montrer (l'écran, l'historique, le rapport) : [`titre_verdict`], sauf pour un suspect
/// confirmé avec un détail ([`titre_avec_detail`]).
pub fn titre_affiche(cat: &Catalogue, verdict: Option<&Verdict>) -> String {
    match verdict {
        Some(v) if v.genre == TypeVerdict::Trouve && !v.detail.is_empty() => {
            let trouve = texte(v.suspect.as_ref().and_then(|id| cat.suspect(id)).and_then(|s| champ(s, "Trouve")));
            titre_avec_detail(&trouve, &v.detail)
        }
        _ => titre_verdict(cat, verdict),
    }
}
