//! Les règles de PowerShell que les données suivent en plus de celles du moteur (`mindcare_moteur::ps`) : la
//! conversion `[int]`, et `-eq` entre une valeur lue du fichier et un texte.

use serde_json::Value;

use mindcare_moteur::ps::egal;

/// `[int]$x` : rien vaut 0, vrai 1, un nombre à virgule est arrondi au pair le plus proche (comme .NET), un texte est
/// lu comme un nombre. `None` : PowerShell s'arrêterait sur une erreur (texte qui n'est pas un nombre, trop grand).
pub fn entier(v: Option<&Value>) -> Option<i64> {
    let x = match v {
        None | Some(Value::Null) => return Some(0),
        Some(Value::Bool(b)) => return Some(i64::from(*b)),
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i as f64
            } else {
                n.as_f64()?
            }
        }
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                return Some(0);
            }
            t.parse::<f64>().ok().filter(|x| x.is_finite())?
        }
        Some(_) => return None,
    };
    let r = x.round_ties_even();
    (r >= f64::from(i32::MIN) && r <= f64::from(i32::MAX)).then_some(r as i64)
}

/// `$valeur -eq $texte` quand la valeur vient du fichier : un texte se compare sans tenir compte des majuscules, un
/// nombre comme un nombre, vrai ou faux comme `[bool]$texte` ; rien n'égale jamais un texte.
pub fn egal_texte(v: Option<&Value>, t: &str) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => egal(s, t),
        Some(Value::Number(n)) => {
            let x = n.as_f64();
            t.trim().parse::<f64>().ok().is_some_and(|y| Some(y) == x)
        }
        Some(Value::Bool(b)) => *b == !t.is_empty(),
        Some(Value::Array(a)) => {
            let egaux: Vec<&Value> = a.iter().filter(|x| egal_texte(Some(x), t)).collect();
            mindcare_moteur::ps::vrai_liste(&egaux)
        }
        Some(Value::Object(_)) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn entier_comme_powershell() {
        assert_eq!(entier(None), Some(0));
        assert_eq!(entier(Some(&json!(2.5))), Some(2));
        assert_eq!(entier(Some(&json!(3.5))), Some(4));
        assert_eq!(entier(Some(&json!(" 7 "))), Some(7));
        assert_eq!(entier(Some(&json!("abc"))), None);
        assert_eq!(entier(Some(&json!(true))), Some(1));
    }

    #[test]
    fn egal_comme_powershell() {
        assert!(egal_texte(Some(&json!("ABC")), "abc"));
        assert!(egal_texte(Some(&json!(5)), "5"));
        assert!(!egal_texte(None, ""));
    }
}
