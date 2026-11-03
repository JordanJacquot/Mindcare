//! La question posée à llama-server et la lecture de sa réponse : le portage exact de `Start-IaQuestion` (le corps
//! de la requête, la confiance) et de `Get-IaReponse` (la lecture des idées), modules\ia.ps1.

use mindcare_moteur::ps::{Objet, champ, egal, parcours, texte, vrai};
use serde_json::{Value, json};

/// Ce que l'assistant doit faire (`$IaSysteme`) : le même texte que le PowerShell, sur lequel le modèle a été entraîné
pub const SYSTEME: &str = "Tu es Mindcare. Une personne décrit le problème de son ordinateur. Réponds seulement en JSON : la panne du catalogue qui correspond, et les réponses qu'elle donne déjà.";

/// Une idée de l'assistant : une panne et les réponses que la phrase donne déjà (Id de l'étape, numéro de la
/// réponse), dans l'ordre de sa réponse
#[derive(Clone, Debug, PartialEq)]
pub struct Idee {
    pub panne: String,
    pub reponses: Vec<(String, i64)>,
}

/// La réponse de l'assistant (`Get-IaReponse`) : sa panne, ses réponses, sa confiance, ses autres idées (seulement
/// dans les tests), ou une erreur (la vraie cause, pour le journal)
#[derive(Clone, Debug, PartialEq)]
pub struct Reponse {
    pub panne: String,
    pub reponses: Vec<(String, i64)>,
    pub confiance: f64,
    pub autres: Vec<Idee>,
    pub erreur: String,
}

impl Reponse {
    /// Pas de réponse, et pourquoi
    pub fn vide(erreur: impl Into<String>) -> Reponse {
        Reponse {
            panne: String::new(),
            reponses: Vec::new(),
            confiance: 0.0,
            autres: Vec::new(),
            erreur: erreur.into(),
        }
    }
}

/// Le corps de la question (`$corps` de Start-IaQuestion) : le système, la phrase, la température 0, 120 jetons au
/// plus, les probabilités des jetons, et la grammaire du catalogue (la panne ne peut être qu'un des Id permis, les
/// réponses des nombres entiers)
pub fn corps_question(phrase: &str, permises: &[String]) -> Vec<u8> {
    let corps = json!({
        "messages": [
            { "role": "system", "content": SYSTEME },
            { "role": "user", "content": phrase },
        ],
        "temperature": 0,
        "max_tokens": 120,
        "logprobs": true,
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "r",
                "schema": {
                    "type": "object",
                    "required": ["panne", "reponses"],
                    "properties": {
                        "panne": { "type": "string", "enum": permises },
                        "reponses": { "type": "object", "additionalProperties": { "type": "integer" } },
                    },
                },
            },
        },
    });
    corps.to_string().into_bytes()
}

/// Le texte de la réponse et la confiance, lus dans la réponse de llama-server (`/v1/chat/completions`)
pub fn lire_completion(corps: &[u8]) -> Result<(String, f64), String> {
    let texte_recu = String::from_utf8_lossy(corps);
    let r: Value = serde_json::from_str(texte_recu.trim_start_matches('\u{feff}'))
        .map_err(|_| format!("réponse illisible ({})", extrait(&texte_recu)))?;
    let choix = r.get("choices").and_then(Value::as_array).and_then(|a| a.first());
    let Some(choix) = choix else {
        return Err(format!("réponse illisible ({})", extrait(&texte_recu)));
    };
    let c = texte(choix.get("message").and_then(|m| m.get("content")));
    let jetons = choix.get("logprobs").and_then(|l| l.get("content"));
    let p = confiance(&c, jetons);
    Ok((c, p))
}

fn extrait(t: &str) -> String {
    let mut e: String = t.chars().take(200).collect();
    if t.chars().count() > 200 {
        e.push_str("...");
    }
    e
}

/// La confiance : la probabilité des jetons qui écrivent le nom de la panne (ceux qui touchent l'intérieur des
/// guillemets après `"panne":`). Exactement le calcul du PowerShell, en unités UTF-16 comme les chaînes de .NET ; si
/// le texte n'a pas cette forme, 0 ; sans probabilités, 1 (rien à multiplier).
pub fn confiance(c: &str, jetons: Option<&Value>) -> f64 {
    confiance_ps(c, jetons).unwrap_or(0.0)
}

fn confiance_ps(c: &str, jetons: Option<&Value>) -> Option<f64> {
    let u: Vec<u16> = c.encode_utf16().collect();
    // `IndexOf(x, depuis)` de .NET : -1 si absent, une exception si `depuis` sort de la chaîne
    let caractere = |x: char, depuis: i64| -> Option<i64> {
        let depuis = usize::try_from(depuis).ok().filter(|d| *d <= u.len())?;
        let x = x as u16;
        Some(u[depuis..].iter().position(|y| *y == x).map_or(-1, |i| (i + depuis) as i64))
    };
    let motif: Vec<u16> = "\"panne\"".encode_utf16().collect();
    let p_panne = u.windows(motif.len()).position(|w| w == motif.as_slice()).map_or(-1, |i| i as i64);
    let d = caractere('"', caractere(':', p_panne)?)? + 1;
    let f = caractere('"', d)?;
    let mut pos: i64 = 0;
    let mut lp = 0.0f64;
    for j in parcours(jetons) {
        let o = j.as_object();
        let t = texte(o.and_then(|o| champ(o, "token")));
        let n = t.encode_utf16().count() as i64;
        if pos < f && pos + n > d {
            lp += double(o.and_then(|o| champ(o, "logprob")))?;
        }
        pos += n;
    }
    Some(lp.exp())
}

/// `[double]` de PowerShell (rien : 0 ; un texte : lu comme un nombre ; autre chose : une exception)
fn double(v: Option<&Value>) -> Option<f64> {
    match v {
        None | Some(Value::Null) => Some(0.0),
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
        Some(Value::String(s)) => {
            let s = s.trim();
            if s.is_empty() { Some(0.0) } else { s.parse::<f64>().ok() }
        }
        _ => None,
    }
}

/// `[int]` de PowerShell : arrondi au pair le plus proche (2,5 donne 2), dans les limites d'un Int32, sinon une
/// exception (ici `None`)
pub fn entier(v: &Value) -> Option<i64> {
    let f = match v {
        Value::Null => return Some(0),
        Value::Bool(b) => return Some(i64::from(*b)),
        Value::Number(n) => match n.as_i64() {
            Some(i) => i as f64,
            None => n.as_f64()?,
        },
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Some(0);
            }
            s.parse::<f64>().ok()?
        }
        _ => return None,
    };
    let r = f.round_ties_even();
    (r.is_finite() && r >= f64::from(i32::MIN) && r <= f64::from(i32::MAX)).then_some(r as i64)
}

/// Une idée lue (`$j` dans Get-IaReponse) : une panne (obligatoire), ses réponses
fn lire_idee(j: Option<&Value>) -> Result<Idee, ()> {
    let o: Option<&Objet> = j.and_then(Value::as_object);
    let panne = o.and_then(|o| champ(o, "panne"));
    if !vrai(panne) {
        return Err(());
    }
    let mut reponses: Vec<(String, i64)> = Vec::new();
    let rep = o.and_then(|o| champ(o, "reponses"));
    if vrai(rep)
        && let Some(Value::Object(r)) = rep
    {
        for (k, v) in r {
            let n = entier(v).ok_or(())?;
            // (une table `@{}` : sans tenir compte des majuscules, la dernière valeur gagne)
            match reponses.iter_mut().find(|(x, _)| egal(x, k)) {
                Some(e) => e.1 = n,
                None => reponses.push((k.clone(), n)),
            }
        }
    }
    Ok(Idee { panne: texte(panne), reponses })
}

/// Les idées de l'assistant, lues dans un texte (`Get-IaReponse`) : la liste de ses idées `[{c, p}]` (le vrai), une
/// seule idée `{c, p}`, ou directement `{panne, reponses}` (les tests, alors sûre). `c` est le texte JSON de l'idée
/// ou l'idée elle-même.
pub fn lire_texte(t: &str) -> Reponse {
    let illisible = || Reponse::vide(format!("réponse illisible ({t})"));
    let Ok(brut) = serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')) else { return illisible() };
    let idees: Vec<(Value, f64)> = match &brut {
        Value::Array(a) => {
            let mut v = Vec::new();
            for i in a {
                let Some(p) = double(i.as_object().and_then(|o| champ(o, "p"))) else { return illisible() };
                v.push((i.as_object().and_then(|o| champ(o, "c")).cloned().unwrap_or(Value::Null), p));
            }
            v
        }
        Value::Object(o) if vrai(champ(o, "c")) => {
            let Some(p) = double(champ(o, "p")) else { return illisible() };
            vec![(champ(o, "c").cloned().unwrap_or(Value::Null), p)]
        }
        _ => vec![(Value::String(t.to_string()), 1.0)],
    };
    let mut lues: Vec<(Idee, f64)> = Vec::new();
    for (c, p) in idees {
        let j = match c {
            Value::String(s) => match serde_json::from_str::<Value>(&s) {
                Ok(v) => v,
                Err(_) => return illisible(),
            },
            autre => autre,
        };
        match lire_idee(Some(&j)) {
            Ok(i) => lues.push((i, p)),
            Err(()) => return illisible(),
        }
    }
    let mut lues = lues.into_iter();
    let Some((premiere, p)) = lues.next() else {
        // (une liste vide : ni panne ni erreur, comme `$lues[0].Panne` vide en PowerShell)
        return Reponse::vide("");
    };
    Reponse {
        panne: premiere.panne,
        reponses: premiere.reponses,
        confiance: p,
        autres: lues.map(|(i, _)| i).collect(),
        erreur: String::new(),
    }
}

/// La réponse du vrai assistant : son texte et sa confiance, lus comme `Get-IaReponse` lit `[{c, p}]`
pub fn lire_reponse(c: &str, p: f64) -> Reponse {
    let t = json!([{ "c": c, "p": p }]).to_string();
    let mut r = lire_texte(&t);
    if !r.erreur.is_empty() {
        // (le journal garde le texte de l'assistant, pas l'emballage)
        r.erreur = format!("réponse illisible ({c})");
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jetons(liste: &[(&str, f64)]) -> Value {
        Value::Array(liste.iter().map(|(t, p)| json!({ "token": t, "logprob": p })).collect())
    }

    #[test]
    fn confiance_des_jetons_du_nom() {
        let c = r#"{"panne":"bruit","reponses":{}}"#;
        // {"panne":" | bru | it | ","reponses":{}}
        let j = jetons(&[(r#"{"panne":""#, -0.5), ("bru", -0.1), ("it", -0.2), (r#"","reponses":{}}"#, -3.0)]);
        let p = confiance(c, Some(&j));
        assert!((p - (-0.3f64).exp()).abs() < 1e-12, "{p}");
        // sans probabilités : 1 ; texte d'une autre forme : 0
        assert_eq!(confiance(c, None), 1.0);
        assert_eq!(confiance("pas de panne", Some(&j)), 0.0);
    }

    #[test]
    fn entier_comme_powershell() {
        assert_eq!(entier(&json!(2.5)), Some(2));
        assert_eq!(entier(&json!(3.5)), Some(4));
        assert_eq!(entier(&json!("1")), Some(1));
        assert_eq!(entier(&json!(null)), Some(0));
        assert_eq!(entier(&json!(5_000_000_000i64)), None);
        assert_eq!(entier(&json!({})), None);
    }

    #[test]
    fn lire_les_formes() {
        let r = lire_texte(r#"{"panne":"bruit","reponses":{"Type":1}}"#);
        assert_eq!((r.panne.as_str(), r.confiance, r.reponses.clone()), ("bruit", 1.0, vec![("Type".to_string(), 1)]));
        let r = lire_texte(r#"[{"c":"{\"panne\":\"a\",\"reponses\":{}}","p":0.5},{"c":{"panne":"b"},"p":0.1}]"#);
        assert_eq!((r.panne.as_str(), r.confiance, r.autres.len()), ("a", 0.5, 1));
        assert!(lire_texte("pas du json").erreur.starts_with("réponse illisible"));
        assert!(lire_texte(r#"{"reponses":{}}"#).erreur.starts_with("réponse illisible"));
    }
}
