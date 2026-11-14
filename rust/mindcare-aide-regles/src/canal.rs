//! Copié de D:\Projets\Nevermind\rust\aide-regles\src\canal.rs (plan de l'Aide de Mindcare, 14/11/2026), préfixe
//! « Mindcare » au lieu de « Nevermind », sans les sessions de mesure (propres à Nevermind).
//!
//! Nom du canal et du verrou de l'Aide : propres au compte servi et au dossier de cette copie de Mindcare. Une copie
//! de test ne parle jamais à l'Aide de la copie installée, et l'Aide de Mindcare n'a jamais le nom de celle de
//! Nevermind.

use crate::{chemin, sha256, texte};

/// Le début des noms de l'Aide de Mindcare
pub const PREFIXE: &str = "Mindcare-Aide";

/// Le suffixe : les 6 premiers octets (12 caractères hexadécimaux) du SHA-256 du dossier complet, sans « \ » final,
/// en minuscules. `None` si le dossier n'est pas un chemin « X:\... ».
pub fn suffixe(dossier: &str) -> Option<String> {
    let plein = chemin::complet(dossier)?;
    let d = texte::minuscules(plein.trim_end_matches('\\'));
    let e = sha256::empreinte(d.as_bytes());
    Some(e[..6].iter().map(|o| format!("{o:02x}")).collect())
}

/// Le nom du canal : `Mindcare-Aide-<sid>-<suffixe>`
pub fn nom_canal(sid: &str, suffixe: &str) -> String {
    format!("{PREFIXE}-{sid}-{suffixe}")
}

/// Le nom du verrou (une seule Aide à la fois) : `Global\Mindcare-Aide-<sid>-<suffixe>`, commun à toutes les sessions
/// de Windows (l'Aide en SYSTEM tourne dans la session 0)
pub fn nom_verrou(sid: &str, suffixe: &str) -> String {
    format!("Global\\{PREFIXE}-{sid}-{suffixe}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_propres_au_compte_et_a_la_copie() {
        let (a, b) = ("S-1-5-21-1-2-3-1001", "S-1-5-21-1-2-3-1002");
        let s = suffixe("C:\\Mindset\\Programmes\\Mindcare").unwrap();
        let t = suffixe("D:\\Essai\\Mindcare").unwrap();
        assert_eq!(s.len(), 12);
        assert!(s.bytes().all(|o| o.is_ascii_hexdigit() && !o.is_ascii_uppercase()));
        // (le même dossier écrit autrement : le même suffixe)
        assert_eq!(suffixe("c:\\mindset\\programmes\\MINDCARE\\"), Some(s.clone()));
        assert_eq!(suffixe("C:\\Mindset\\Programmes\\x\\..\\Mindcare"), Some(s.clone()));
        assert_eq!(suffixe("Mindcare"), None);
        assert_eq!(suffixe("\\\\serveur\\Mindcare"), None);
        assert_eq!(nom_canal(a, &s), format!("Mindcare-Aide-{a}-{s}"));
        assert_eq!(nom_verrou(a, &s), format!("Global\\Mindcare-Aide-{a}-{s}"));
        let noms = [nom_canal(a, &s), nom_canal(b, &s), nom_canal(a, &t)];
        for (i, x) in noms.iter().enumerate() {
            assert!(noms.iter().skip(i + 1).all(|y| y != x), "{noms:?}");
            assert!(!x.starts_with("Nevermind"), "{x}");
        }
    }
}
