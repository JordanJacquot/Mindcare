//! Les expressions régulières et les conversions de PowerShell que les règles reproduisent, écrites à la main (pas de
//! bibliothèque d'expressions régulières pour quatre motifs simples) :
//!
//! - `'0x([0-9a-fA-F]{8})'` : le code d'un écran bleu ([`code_hex`]) ;
//! - `'\b([A-Za-z]):'` : la lettre d'un lecteur ([`lettre_lecteur`]) ;
//! - `'(?i)Harddisk(\d+)\\|\b(?:disk|disque)\s+(\d+)\b'` : le numéro d'un disque ([`numero_disque`]) ;
//! - `-match 'A|B|C'` sans tenir compte des majuscules ([`contient_un`]).
//!
//! Comme .NET, la première correspondance est la plus à gauche. Les « caractères de mot » de `\b` sont les lettres,
//! les chiffres et « _ » (.NET : lettres, chiffres décimaux, marques et ponctuations de liaison ; la différence ne
//! touche que des caractères qui n'apparaissent pas dans ces journaux). `\d` : les chiffres 0 à 9 (.NET accepte aussi
//! les chiffres d'autres écritures, que `[int]` refuserait ensuite : l'erreur compte alors, comme ici).

/// Un « caractère de mot » de `\b`.
fn de_mot(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Vrai si `t[i..]` commence par `motif` (en minuscules ASCII) sans tenir compte des majuscules ; rend la fin.
fn commence_par_sans_casse(t: &str, i: usize, motif: &str) -> Option<usize> {
    let mut fin = i;
    let mut car = t[i..].chars();
    for m in motif.chars() {
        let c = car.next()?;
        if !c.to_lowercase().eq(std::iter::once(m)) {
            return None;
        }
        fin += c.len_utf8();
    }
    Some(fin)
}

/// Les chiffres 0 à 9 à partir de `i` (au moins un) : rend la fin.
fn chiffres(t: &str, i: usize) -> Option<usize> {
    let n = t[i..].bytes().take_while(u8::is_ascii_digit).count();
    (n > 0).then_some(i + n)
}

/// Le caractère avant la position `i` (rien au début).
fn avant(t: &str, i: usize) -> Option<char> {
    t[..i].chars().next_back()
}

/// `'0x([0-9a-fA-F]{8})'` : les 8 chiffres hexadécimaux du premier « 0x » qui en a 8 derrière lui.
pub fn code_hex(t: &str) -> Option<&str> {
    let o = t.as_bytes();
    (0..o.len().saturating_sub(9)).find_map(|i| {
        (o[i] == b'0' && o[i + 1] == b'x' && o[i + 2..i + 10].iter().all(u8::is_ascii_hexdigit))
            .then(|| &t[i + 2..i + 10])
    })
}

/// `'\b([A-Za-z]):'` : la première lettre suivie de « : » qui commence un mot.
pub fn lettre_lecteur(t: &str) -> Option<char> {
    t.char_indices().find_map(|(i, c)| {
        (c.is_ascii_alphabetic() && t[i + 1..].starts_with(':') && !avant(t, i).is_some_and(de_mot)).then_some(c)
    })
}

/// `'(?i)Harddisk(\d+)\\|\b(?:disk|disque)\s+(\d+)\b'` : les chiffres du premier « Harddisk3\ » ou « disque 3 ».
pub fn numero_disque(t: &str) -> Option<&str> {
    t.char_indices().find_map(|(i, _)| harddisk(t, i).or_else(|| disque(t, i)))
}

/// `Harddisk(\d+)\\` à la position `i`.
fn harddisk(t: &str, i: usize) -> Option<&str> {
    let d = commence_par_sans_casse(t, i, "harddisk")?;
    let f = chiffres(t, d)?;
    t[f..].starts_with('\\').then(|| &t[d..f])
}

/// `\b(?:disk|disque)\s+(\d+)\b` à la position `i`.
fn disque(t: &str, i: usize) -> Option<&str> {
    if avant(t, i).is_some_and(de_mot) {
        return None;
    }
    let m = commence_par_sans_casse(t, i, "disk").or_else(|| commence_par_sans_casse(t, i, "disque"))?;
    let espaces: usize = t[m..].chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum();
    if espaces == 0 {
        return None;
    }
    let d = m + espaces;
    let f = chiffres(t, d)?;
    // (\d+ puis \b : le caractère suivant ne doit pas être de mot ; revenir en arrière sur les chiffres n'y change rien)
    (!t[f..].chars().next().is_some_and(de_mot)).then(|| &t[d..f])
}

/// `$t -match 'A|B|C'` pour des mots simples : l'un d'eux apparaît, sans tenir compte des majuscules.
pub fn contient_un(t: &str, mots: &[&str]) -> bool {
    let t = t.to_lowercase();
    mots.iter().any(|m| t.contains(&m.to_lowercase()))
}

/// `[math]::Round($x)` et `[int]$x` : l'entier le plus proche, les moitiés vers le pair.
pub fn arrondi(x: f64) -> f64 {
    x.round_ties_even()
}

/// Un nombre écrit comme PowerShell l'écrit dans un texte (`"$x"`), pour un nombre déjà arrondi.
pub fn nombre(x: f64) -> String {
    if x == 0.0 { "0".to_string() } else { format!("{x}") }
}

/// `[double]$texte` : vide vaut 0 ; des espaces autour sont permis ; rien si ce n'est pas un nombre (PowerShell lève
/// une erreur).
pub fn double(t: &str) -> Option<f64> {
    let t = t.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    if !t.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+' | b'e' | b'E')) {
        return None;
    }
    t.parse().ok()
}

/// `Get-Quantite` : « 1 erreur », « 0 erreur », « 3 erreurs ».
pub fn quantite(n: i64, un: &str, plusieurs: &str) -> String {
    if n <= 1 { format!("{n} {un}") } else { format!("{n} {plusieurs}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_hex() {
        assert_eq!(code_hex("0x0000009f (0x1, 0x2)"), Some("0000009f"));
        assert_eq!(code_hex("0x123 puis 0xC000021A"), Some("C000021A"));
        assert_eq!(code_hex("0X0000009F"), None);
        assert_eq!(code_hex("0x0000009"), None);
        assert_eq!(code_hex("é0x0000009F"), Some("0000009F"));
    }

    #[test]
    fn lettres() {
        assert_eq!(lettre_lecteur("Le volume C: est abîmé"), Some('C'));
        assert_eq!(lettre_lecteur("\\Device\\HarddiskVolume3 e:"), Some('e'));
        assert_eq!(lettre_lecteur("AC: rien"), None);
        assert_eq!(lettre_lecteur("éC: rien"), None);
        assert_eq!(lettre_lecteur("(D:)"), Some('D'));
    }

    #[test]
    fn disques() {
        assert_eq!(numero_disque("\\Device\\Harddisk2\\DR2"), Some("2"));
        assert_eq!(numero_disque("HARDDISK12\\"), Some("12"));
        assert_eq!(numero_disque("Harddisk2"), None);
        assert_eq!(numero_disque("le disque  4 a une erreur"), Some("4"));
        assert_eq!(numero_disque("Disk 1."), Some("1"));
        assert_eq!(numero_disque("disk 1a"), None);
        assert_eq!(numero_disque("mydisk 1"), None);
        assert_eq!(numero_disque("disk1"), None);
        assert_eq!(numero_disque("disk 3 et Harddisk1\\"), Some("3"));
    }

    #[test]
    fn nombres() {
        assert_eq!(arrondi(2.5), 2.0);
        assert_eq!(arrondi(3.5), 4.0);
        assert_eq!(nombre(arrondi(0.4)), "0");
        assert_eq!(nombre(42.0), "42");
        assert_eq!(double(" 12 "), Some(12.0));
        assert_eq!(double(""), Some(0.0));
        assert_eq!(double("abc"), None);
    }
}
