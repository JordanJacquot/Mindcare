//! Les règles de PowerShell que le moteur reproduit à la lettre. Le moteur de référence est en PowerShell
//! (modules\enquete.ps1) : pour donner exactement les mêmes réponses, le Rust lit le catalogue comme lui.
//!
//! - Un champ se lit sans tenir compte des majuscules (`$p.titre` lit « Titre »).
//! - Vrai ou faux : rien, un texte vide, 0 et une liste vide sont faux ; une liste d'un seul élément vaut cet élément.
//! - `@($x)` fait une liste ; rien y devient une liste d'un seul « rien ». `foreach` sur rien ne fait aucun tour.
//! - `-eq`, `-contains` et les clés des tables (`@{}`) ne tiennent pas compte des majuscules ; rien n'égale que rien.
//! - `.Contains()` et `.Remove()` d'une liste .NET (les suspects restants), eux, tiennent compte des majuscules.

use serde_json::{Map, Value};

/// Un objet du catalogue (une panne, une étape, une réponse...) : ses champs, dans l'ordre du fichier.
pub type Objet = Map<String, Value>;

static RIEN: Value = Value::Null;

/// Un champ d'un objet, sans tenir compte des majuscules (le nom exact d'abord).
pub fn champ<'a>(o: &'a Objet, nom: &str) -> Option<&'a Value> {
    o.get(nom).or_else(|| o.iter().find(|(k, _)| k.eq_ignore_ascii_case(nom)).map(|(_, v)| v))
}

/// Un champ d'une valeur quelconque : rien si ce n'est pas un objet (`$texte.Champ` vaut rien).
pub fn champ_v<'a>(v: Option<&'a Value>, nom: &str) -> Option<&'a Value> {
    match v {
        Some(Value::Object(o)) => champ(o, nom),
        _ => None,
    }
}

/// Écrit un champ comme `$o[$nom] = ...` dans une table ordonnée : un champ du même nom (majuscules ou pas) est
/// remplacé à sa place, sinon il est ajouté à la fin.
pub fn mettre(o: &mut Objet, nom: &str, valeur: Value) {
    let cle = o.keys().find(|k| k.eq_ignore_ascii_case(nom)).cloned();
    match cle {
        Some(k) => {
            o.insert(k, valeur);
        }
        None => {
            o.insert(nom.to_string(), valeur);
        }
    }
}

/// Vrai ou faux, comme `if ($x)` en PowerShell.
pub fn vrai(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => vrai_liste(&a.iter().collect::<Vec<_>>()),
        Some(Value::Object(_)) => true,
    }
}

/// Vrai ou faux d'une liste : vide, faux ; un seul élément, cet élément ; plusieurs, vrai.
pub fn vrai_liste(l: &[&Value]) -> bool {
    match l {
        [] => false,
        [x] => vrai(Some(x)),
        _ => true,
    }
}

/// Le texte d'une valeur, comme `[string]$x` (une liste : ses éléments séparés par une espace).
pub fn texte(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(true)) => "True".to_string(),
        Some(Value::Bool(false)) => "False".to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Array(a)) => a.iter().map(|x| texte(Some(x))).collect::<Vec<_>>().join(" "),
        // (PowerShell écrirait « @{...} » : jamais dans le catalogue)
        Some(Value::Object(_)) => String::new(),
    }
}

/// `@($x)` : une liste. Rien donne une liste d'un seul « rien » (`@($null).Count` vaut 1).
pub fn tableau(v: Option<&Value>) -> Vec<&Value> {
    match v {
        None => vec![&RIEN],
        Some(Value::Array(a)) => a.iter().collect(),
        Some(x) => vec![x],
    }
}

/// `foreach ($x in $v)` ou `$v | ...` : rien ne fait aucun tour.
pub fn parcours(v: Option<&Value>) -> Vec<&Value> {
    match v {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a.iter().collect(),
        Some(x) => vec![x],
    }
}

/// `-eq` entre deux textes : sans tenir compte des majuscules.
pub fn egal(a: &str, b: &str) -> bool {
    a == b || a.chars().flat_map(char::to_lowercase).eq(b.chars().flat_map(char::to_lowercase))
}

/// `-eq` entre deux valeurs : rien n'égale que rien ; sinon leurs textes, sans tenir compte des majuscules.
pub fn egal_valeurs(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Null, _) | (_, Value::Null) => false,
        _ => egal(&texte(Some(a)), &texte(Some(b))),
    }
}

/// `$liste -contains $x` pour une liste de textes (un texte n'égale jamais rien).
pub fn contient(liste: &[String], x: &Value) -> bool {
    !x.is_null() && {
        let t = texte(Some(x));
        liste.iter().any(|e| egal(e, &t))
    }
}

/// `$liste -contains $x` pour une liste de valeurs.
pub fn contient_valeur(liste: &[&Value], x: &Value) -> bool {
    liste.iter().any(|e| egal_valeurs(e, x))
}

/// La clé d'une table PowerShell (`@{}`) : sans tenir compte des majuscules.
pub fn cle(t: &str) -> String {
    t.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn vrai_comme_powershell() {
        assert!(!vrai(None));
        assert!(!vrai(Some(&json!(""))));
        assert!(vrai(Some(&json!("0"))));
        assert!(!vrai(Some(&json!(0))));
        assert!(!vrai(Some(&json!([]))));
        assert!(!vrai(Some(&json!([""]))));
        assert!(vrai(Some(&json!(["", ""]))));
        assert!(vrai(Some(&json!({}))));
    }

    #[test]
    fn listes_comme_powershell() {
        assert_eq!(tableau(None).len(), 1);
        assert_eq!(parcours(None).len(), 0);
        assert_eq!(tableau(Some(&json!("a"))).len(), 1);
        assert_eq!(texte(Some(&json!(["a", null, "b"]))), "a  b");
        assert!(contient(&["Portable".to_string()], &json!("portable")));
        assert!(!contient(&[String::new()], &Value::Null));
    }
}
