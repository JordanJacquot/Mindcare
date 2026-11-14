//! Copié de D:\Projets\Nevermind\rust\aide-regles\src\texte.rs (seulement ce dont les règles de Mindcare se servent).
//!
//! Petits outils sur les textes, pour comparer comme .NET et PowerShell : sans tenir compte des majuscules, lettre par
//! lettre (`OrdinalIgnoreCase` : majuscule simple de chaque lettre). Une lettre dont la majuscule (ou la minuscule)
//! s'écrit en plusieurs lettres reste telle quelle, comme dans .NET.

/// Majuscule simple d'une lettre (elle-même si sa majuscule fait plusieurs lettres)
pub fn majuscule(c: char) -> char {
    let mut m = c.to_uppercase();
    match (m.next(), m.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

/// Minuscule simple d'une lettre (elle-même si sa minuscule fait plusieurs lettres)
pub fn minuscule(c: char) -> char {
    let mut m = c.to_lowercase();
    match (m.next(), m.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

/// Le texte en minuscules, comme `ToLowerInvariant` de .NET (aucune normalisation des accents)
pub fn minuscules(s: &str) -> String {
    s.chars().map(minuscule).collect()
}

/// Égaux sans tenir compte des majuscules (`OrdinalIgnoreCase`)
pub fn egal(a: &str, b: &str) -> bool {
    a.chars().map(majuscule).eq(b.chars().map(majuscule))
}

/// `s` commence par `debut`, sans tenir compte des majuscules
pub fn commence(s: &str, debut: &str) -> bool {
    let mut lettres = s.chars();
    debut.chars().all(|d| lettres.next().is_some_and(|c| majuscule(c) == majuscule(d)))
}

/// `s` contient `morceau`, sans tenir compte des majuscules
pub fn contient(s: &str, morceau: &str) -> bool {
    s.char_indices().any(|(i, _)| commence(&s[i..], morceau))
}

/// Sans les espaces du début et de la fin (`Trim()` de .NET)
pub fn sans_espaces(s: &str) -> &str {
    s.trim_matches(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparaisons() {
        assert!(egal("C:\\WINDOWS", "c:\\windows"));
        assert!(!egal("C:\\WINDOWS", "c:\\windows\\"));
        assert!(commence("S-1-5-18", "s-1-"));
        assert!(!commence("S-1", "S-1-5"));
        assert!(contient("Hyper-V Virtual Ethernet Adapter", "VIRTUAL ETHERNET"));
        assert!(!contient("Realtek", "virtual"));
        assert_eq!(minuscules("ÉTÉ İstanbul"), "été İstanbul");
        assert_eq!(sans_espaces("  -Sid x \r\n"), "-Sid x");
    }
}
