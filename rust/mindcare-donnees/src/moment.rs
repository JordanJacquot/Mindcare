//! Les dates de Mindcare : un moment en heure locale (donné par l'appelant : ce crate ne lit jamais l'horloge de
//! Windows), écrit comme `Get-Date -Format s`, relu comme `[datetime]::Parse` (culture invariante) et montré en français
//! comme `Format-Date` (modules\historique.ps1).

use std::fmt;

/// Un moment en heure locale, à la seconde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Moment {
    pub annee: i32,
    pub mois: u32,
    pub jour: u32,
    pub heure: u32,
    pub minute: u32,
    pub seconde: u32,
}

/// Les mois en français (culture fr-FR de .NET : les mêmes noms après un jour).
const MOIS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

/// Une année bissextile (calendrier grégorien).
fn bissextile(a: i32) -> bool {
    (a % 4 == 0 && a % 100 != 0) || a % 400 == 0
}

/// Le nombre de jours d'un mois.
fn jours_du_mois(a: i32, m: u32) -> u32 {
    match m {
        2 if bissextile(a) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl Moment {
    /// Un moment, s'il existe (1er janvier de l'an 1 au 31 décembre 9999, comme .NET).
    pub fn nouveau(annee: i32, mois: u32, jour: u32, heure: u32, minute: u32, seconde: u32) -> Option<Moment> {
        let ok = (1..=9999).contains(&annee)
            && (1..=12).contains(&mois)
            && jour >= 1
            && jour <= jours_du_mois(annee, mois)
            && heure < 24
            && minute < 60
            && seconde < 60;
        ok.then_some(Moment { annee, mois, jour, heure, minute, seconde })
    }

    /// `Get-Date -Format s` : « 2026-10-30T14:05:00 ».
    pub fn iso(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.annee, self.mois, self.jour, self.heure, self.minute, self.seconde
        )
    }

    /// `Get-Date -Format yyyyMMdd-HHmmss` (le nom d'un fichier mis de côté, le début d'un Id d'enquête).
    pub fn compact(&self) -> String {
        format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}",
            self.annee, self.mois, self.jour, self.heure, self.minute, self.seconde
        )
    }

    /// Relu comme `[datetime]::Parse($texte, InvariantCulture)` pour les formes que Mindcare écrit ou qu'on peut
    /// raisonnablement trouver dans le fichier : « 2026-10-30T14:05:00 » (secondes, fractions et heure facultatives,
    /// « T » ou une espace), « 2026/10/30 14:05 », « 10/30/2026 14:05:00 » (mois d'abord, AM/PM permis). Le reste :
    /// `None` (Format-Date rend alors le texte tel quel). Pas lus, alors que .NET les lit : une date avec un fuseau
    /// (« Z », « +02:00 » : .NET la convertirait dans le fuseau du PC), les noms de mois (« 30 Oct 2026 »), une heure
    /// seule (.NET prend le jour même). Mindcare n'écrit jamais ces formes.
    pub fn lire(texte: &str) -> Option<Moment> {
        let t = texte.trim();
        let (date, reste) = match t.find(['T', ' ']) {
            Some(i) => (&t[..i], t[i + 1..].trim_start()),
            None => (t, ""),
        };
        let (annee, mois, jour) = if date.contains('-') {
            let p: Vec<&str> = date.split('-').collect();
            if p.len() != 3 || p[0].len() != 4 {
                return None;
            }
            (nombre(p[0])?, nombre(p[1])?, nombre(p[2])?)
        } else if date.contains('/') {
            let p: Vec<&str> = date.split('/').collect();
            if p.len() != 3 {
                return None;
            }
            if p[0].len() == 4 {
                (nombre(p[0])?, nombre(p[1])?, nombre(p[2])?)
            } else if p[2].len() == 4 {
                (nombre(p[2])?, nombre(p[0])?, nombre(p[1])?)
            } else {
                return None;
            }
        } else {
            return None;
        };
        let (heure, minute, seconde) = if reste.is_empty() {
            (0, 0, 0)
        } else {
            let (h, ampm) = match reste.rsplit_once(' ') {
                Some((h, a)) if a.eq_ignore_ascii_case("AM") || a.eq_ignore_ascii_case("PM") => {
                    (h.trim_end(), Some(a.eq_ignore_ascii_case("PM")))
                }
                _ => (reste, None),
            };
            let p: Vec<&str> = h.split(':').collect();
            if p.len() < 2 || p.len() > 3 {
                return None;
            }
            let heure = nombre(p[0])?;
            let minute = nombre(p[1])?;
            let seconde = match p.get(2) {
                None => 0,
                Some(s) => {
                    let (sec, fraction) = match s.split_once('.') {
                        Some((a, f)) => (a, Some(f)),
                        None => (*s, None),
                    };
                    if fraction.is_some_and(|f| f.is_empty() || f.len() > 7 || !f.bytes().all(|b| b.is_ascii_digit())) {
                        return None;
                    }
                    nombre(sec)?
                }
            };
            let heure = match ampm {
                None => heure,
                Some(_) if heure == 0 || heure > 12 => return None,
                Some(true) => heure % 12 + 12,
                Some(false) => heure % 12,
            };
            (heure, minute, seconde)
        };
        Moment::nouveau(i32::try_from(annee).ok()?, mois, jour, heure, minute, seconde)
    }
}

/// Un nombre de 1 à 4 chiffres.
fn nombre(t: &str) -> Option<u32> {
    if t.is_empty() || t.len() > 4 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

impl fmt::Display for Moment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.iso())
    }
}

/// `Format-Date` : « 30 octobre à 14h05 » (avec l'année : « 30 octobre 2026 à 14h05 »), « 1er » pour le premier du
/// mois. Un texte qui n'est pas une date est rendu tel quel.
pub fn format_date(iso: &str, avec_annee: bool) -> String {
    match Moment::lire(iso) {
        Some(m) => format_moment(&m, avec_annee),
        None => iso.to_string(),
    }
}

/// Un moment en français, comme `Format-Date`.
pub fn format_moment(m: &Moment, avec_annee: bool) -> String {
    let jour = if m.jour == 1 { "1er".to_string() } else { m.jour.to_string() };
    let mois = MOIS[(m.mois - 1) as usize];
    if avec_annee {
        format!("{jour} {mois} {:04} à {:02}h{:02}", m.annee, m.heure, m.minute)
    } else {
        format!("{jour} {mois} à {:02}h{:02}", m.heure, m.minute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_lues_et_montrees() {
        assert_eq!(format_date("2026-10-30T14:05:00", false), "30 octobre à 14h05");
        assert_eq!(format_date("2026-02-01T09:00:00", true), "1er février 2026 à 09h00");
        assert_eq!(format_date("2026-02-30T09:00:00", false), "2026-02-30T09:00:00");
        assert_eq!(format_date("", false), "");
        assert_eq!(format_date("10/30/2026 2:05 PM", false), "30 octobre à 14h05");
    }
}
