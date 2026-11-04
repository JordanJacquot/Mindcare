//! Les pannes qui concernent un PC : le champ « PourPc » d'une panne du catalogue, et le profil du PC (portable ou
//! fixe, et sa marque) qui dit lesquelles montrer.
//!
//! Dans enquetes.json, une panne propre à un genre de PC porte par exemple :
//!
//! ```json
//! "PourPc": { "Type": "portable" }
//! "PourPc": { "Type": "fixe" }
//! "PourPc": { "Marque": ["HP"] }
//! ```
//!
//! Les deux champs peuvent aller ensemble (les deux doivent alors aller) ; « Marque » est une liste (une des marques
//! suffit). Une panne sans « PourPc » concerne tous les PC.
//!
//! La règle d'or : **ce qu'on ne sait pas ne cache rien**. Un profil dont le type est inconnu montre les pannes
//! « portable » et « fixe » ; une marque inconnue montre les pannes de toutes les marques. Le choix « je ne sais pas »
//! montre donc tout, comme avant.
//!
//! Le nom « PourPc » (et pas « Pour ») : sur une étape, « Pour » est déjà la liste des suspects à qui elle sert. Le
//! champ n'est lu que sur une panne : cacher une étape changerait le parcours d'une enquête déjà gardée (les réponses
//! sont rejouées dans l'ordre), et le catalogue sait déjà sauter les étapes d'un portable par ses notes (« portable »,
//! « fixe »). PowerShell et l'appli du téléphone ne le lisent pas (un champ inconnu d'une panne y est ignoré).

use serde_json::Value;

use crate::ps::{Objet, champ, egal};

/// Le nom du champ dans enquetes.json.
pub const CHAMP: &str = "PourPc";

/// Portable ou PC fixe (une tour, un PC tout-en-un...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypePc {
    Portable,
    Fixe,
}

impl TypePc {
    /// Le mot du catalogue (« portable », « fixe ») : le même que les notes des réponses.
    pub fn nom(self) -> &'static str {
        match self {
            TypePc::Portable => "portable",
            TypePc::Fixe => "fixe",
        }
    }

    /// Depuis le mot du catalogue (sans tenir compte des majuscules ; « tour » vaut « fixe »).
    pub fn depuis(t: &str) -> Option<TypePc> {
        match t.trim().to_lowercase().as_str() {
            "portable" => Some(TypePc::Portable),
            "fixe" | "tour" => Some(TypePc::Fixe),
            _ => None,
        }
    }

    /// Pour les gens : « Portable », « PC fixe ».
    pub fn libelle(self) -> &'static str {
        match self {
            TypePc::Portable => "Portable",
            TypePc::Fixe => "PC fixe",
        }
    }
}

/// Les marques qu'on peut citer dans « PourPc » et choisir à la main, avec leurs noms simples. La détection rend
/// toujours un de ces noms quand elle reconnaît la marque (`mindcare_windows::marque_simple`).
pub const MARQUES: [&str; 22] = [
    "Acer",
    "Apple",
    "ASRock",
    "ASUS",
    "Biostar",
    "Dell",
    "EVGA",
    "Fujitsu",
    "Gigabyte",
    "HP",
    "Huawei",
    "Intel",
    "Lenovo",
    "LG",
    "Medion",
    "Microsoft Surface",
    "MSI",
    "Razer",
    "Samsung",
    "Sony",
    "Toshiba",
    "Xiaomi",
];

/// Le nom simple d'une marque de la liste, tel qu'écrit dans [`MARQUES`] (sans tenir compte des majuscules).
pub fn marque_connue(m: &str) -> Option<&'static str> {
    MARQUES.iter().copied().find(|x| x.eq_ignore_ascii_case(m.trim()))
}

/// Ce qu'on sait du PC pour trier les pannes. Chaque champ peut être inconnu (rien) : il ne cache alors rien.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfilPc {
    pub genre: Option<TypePc>,
    /// La marque qui compte pour les pannes, en nom simple (« HP », « ASUS »...) : celle du PC pour un PC de marque,
    /// celle de la carte mère pour un PC monté.
    pub marque: Option<String>,
}

impl ProfilPc {
    /// « Je ne sais pas » : tout est montré.
    pub fn inconnu() -> ProfilPc {
        ProfilPc::default()
    }

    /// Rien n'est connu (tout est montré).
    pub fn est_inconnu(&self) -> bool {
        self.genre.is_none() && self.marque.as_deref().is_none_or(|m| m.trim().is_empty())
    }

    /// Le libellé court de la pastille : « Portable HP », « PC fixe ASUS », « Portable », « PC de marque Dell »...
    /// Rien de connu : « Tous les PC » (tout est montré).
    pub fn libelle_court(&self) -> String {
        let marque = self.marque.as_deref().map(str::trim).filter(|m| !m.is_empty());
        match (self.genre, marque) {
            (Some(g), Some(m)) => format!("{} {m}", g.libelle()),
            (Some(g), None) => g.libelle().to_string(),
            (None, Some(m)) => format!("PC {m}"),
            (None, None) => "Tous les PC".to_string(),
        }
    }
}

/// Le PC décrit pour le rapport du réparateur, ligne par ligne, déjà écrit pour les gens (rien : pas reconnu). Jamais
/// de nom d'utilisateur ni de numéro de série. Rempli par `mindcare_windows::PcDetecte::fiche`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FichePc {
    /// « Portable », « PC fixe », « PC fixe monté ».
    pub genre: Option<String>,
    /// « HP Pavilion 15-eh1xxx ».
    pub modele: Option<String>,
    /// « ASUS ROG STRIX B550-F GAMING ».
    pub carte_mere: Option<String>,
    pub processeur: Option<String>,
    /// « 16 Go (2 barrettes) ».
    pub memoire: Option<String>,
    /// « SSD NVMe de 476 Go, 120 Go libres ».
    pub disque: Option<String>,
    /// « Windows 11, version 24H2 ».
    pub windows: Option<String>,
}

/// Une panne (ou tout objet du catalogue) concerne-t-elle ce profil ? Sans « PourPc », ou un « PourPc » mal écrit
/// (refusé par la validation du catalogue) : oui. Ce qui est inconnu dans le profil ne cache rien.
pub fn concerne(objet: &Objet, profil: &ProfilPc) -> bool {
    let Some(Value::Object(pour)) = champ(objet, CHAMP) else { return true };
    if let (Some(Value::String(t)), Some(g)) = (champ(pour, "Type"), profil.genre)
        && let Some(voulu) = TypePc::depuis(t)
        && voulu != g
    {
        return false;
    }
    if let (Some(Value::Array(l)), Some(m)) = (champ(pour, "Marque"), profil.marque.as_deref())
        && !m.trim().is_empty()
        && !l.is_empty()
        && !l.iter().any(|x| x.as_str().is_some_and(|x| egal(x.trim(), m.trim())))
    {
        return false;
    }
    true
}

/// Ce qui ne va pas dans un « PourPc » (vide : tout va bien). Un objet avec « Type » (« portable » ou « fixe »)
/// et/ou « Marque » (une liste non vide de marques de [`MARQUES`]), rien d'autre.
pub fn problemes(pour: &Value) -> Vec<String> {
    let Value::Object(o) = pour else {
        return vec!["PourPc doit être un objet, par exemple { \"Type\": \"portable\" }".to_string()];
    };
    let mut pb = Vec::new();
    if o.is_empty() {
        pb.push("PourPc vide".to_string());
    }
    for (k, v) in o {
        match k.as_str() {
            "Type" => match v {
                Value::String(t) if t == "portable" || t == "fixe" => {}
                _ => pb.push(format!("PourPc : Type \u{ab} {} \u{bb} inconnu (portable ou fixe)", texte_brut(v))),
            },
            "Marque" => match v {
                Value::Array(l) if !l.is_empty() => {
                    for m in l {
                        match m.as_str() {
                            Some(s) if MARQUES.contains(&s) => {}
                            _ => pb.push(format!("PourPc : marque \u{ab} {} \u{bb} inconnue", texte_brut(m))),
                        }
                    }
                }
                _ => pb.push("PourPc : Marque doit être une liste de marques, par exemple [\"HP\"]".to_string()),
            },
            _ => pb.push(format!("PourPc : champ \u{ab} {k} \u{bb} inconnu (Type ou Marque)")),
        }
    }
    pb
}

fn texte_brut(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        autre => autre.to_string(),
    }
}

/// Des pannes triées pour un profil : celles à montrer (dans l'ordre reçu) et le nombre de celles cachées.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tri<'a> {
    pub visibles: Vec<&'a Objet>,
    pub cachees: usize,
}

/// Trie des pannes pour un profil.
pub fn trier<'a>(pannes: impl IntoIterator<Item = &'a Objet>, profil: &ProfilPc) -> Tri<'a> {
    let mut t = Tri::default();
    for p in pannes {
        if concerne(p, profil) {
            t.visibles.push(p);
        } else {
            t.cachees += 1;
        }
    }
    t
}
