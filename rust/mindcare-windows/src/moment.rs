//! Les dates, comme le `[DateTime]` de PowerShell : l'heure locale du PC, comptée en « ticks » (dixièmes de
//! microseconde) depuis le 1er janvier de l'an 1. Toutes les dates de la lecture de Windows sont à l'heure locale
//! (TimeCreated des événements, LastBootUpTime, Get-Date) ; seul le module Windows convertit depuis l'heure universelle.

/// Un instant, à l'heure locale du PC (les ticks de `[DateTime]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Moment(pub i64);

/// Les ticks d'une seconde.
pub const TICKS_PAR_SECONDE: i64 = 10_000_000;
/// Les ticks d'une minute.
pub const TICKS_PAR_MINUTE: i64 = 60 * TICKS_PAR_SECONDE;
/// Les ticks d'un jour.
pub const TICKS_PAR_JOUR: i64 = 86_400 * TICKS_PAR_SECONDE;

/// Les jours entre le 1er janvier de l'an 1 et le 1er janvier 1970.
const JOURS_AVANT_1970: i64 = 719_162;

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

/// Les jours depuis le 1er janvier 1970 d'une date du calendrier (algorithme « days from civil »).
fn jours_depuis_1970(annee: i64, mois: u32, jour: u32) -> i64 {
    let a = if mois <= 2 { annee - 1 } else { annee };
    let ere = a.div_euclid(400);
    let ae = a - ere * 400;
    let m = i64::from(mois);
    let jda = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(jour) - 1;
    let jde = ae * 365 + ae / 4 - ae / 100 + jda;
    ere * 146_097 + jde - 719_468
}

/// La date du calendrier d'un nombre de jours depuis le 1er janvier 1970 (algorithme « civil from days »).
fn date_de_jours(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let ere = z.div_euclid(146_097);
    let jde = z - ere * 146_097;
    let ae = (jde - jde / 1460 + jde / 36_524 - jde / 146_096) / 365;
    let jda = jde - (365 * ae + ae / 4 - ae / 100);
    let mp = (5 * jda + 2) / 153;
    let jour = (jda - (153 * mp + 2) / 5 + 1) as u32;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let annee = ae + ere * 400 + i64::from(mois <= 2);
    (annee, mois, jour)
}

fn mois_valide(annee: i64, mois: u32, jour: u32) -> bool {
    if !(1..=12).contains(&mois) || jour == 0 || !(1..=9999).contains(&annee) {
        return false;
    }
    let bissextile = (annee % 4 == 0 && annee % 100 != 0) || annee % 400 == 0;
    let max = match mois {
        2 if bissextile => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    jour <= max
}

impl Moment {
    /// Un instant d'une date et d'une heure (rien si la date n'existe pas).
    pub fn nouveau(
        annee: i64,
        mois: u32,
        jour: u32,
        heure: u32,
        minute: u32,
        seconde: u32,
        ticks: u32,
    ) -> Option<Moment> {
        if !mois_valide(annee, mois, jour) || heure > 23 || minute > 59 || seconde > 59 || ticks >= 10_000_000 {
            return None;
        }
        let jours = jours_depuis_1970(annee, mois, jour) + JOURS_AVANT_1970;
        Some(Moment(
            jours * TICKS_PAR_JOUR
                + i64::from(heure) * 3600 * TICKS_PAR_SECONDE
                + i64::from(minute) * TICKS_PAR_MINUTE
                + i64::from(seconde) * TICKS_PAR_SECONDE
                + i64::from(ticks),
        ))
    }

    /// L'année, le mois et le jour.
    pub fn date(self) -> (i64, u32, u32) {
        date_de_jours(self.0.div_euclid(TICKS_PAR_JOUR) - JOURS_AVANT_1970)
    }

    /// L'heure, la minute, la seconde et les ticks restants.
    pub fn heure(self) -> (u32, u32, u32, u32) {
        let t = self.0.rem_euclid(TICKS_PAR_JOUR);
        let s = t / TICKS_PAR_SECONDE;
        ((s / 3600) as u32, (s / 60 % 60) as u32, (s % 60) as u32, (t % TICKS_PAR_SECONDE) as u32)
    }

    /// `.AddDays(n)` (un nombre entier de jours : exact).
    pub fn plus_jours(self, n: i64) -> Moment {
        Moment(self.0 + n * TICKS_PAR_JOUR)
    }

    /// `.AddMinutes(n)`.
    pub fn plus_minutes(self, n: i64) -> Moment {
        Moment(self.0 + n * TICKS_PAR_MINUTE)
    }

    /// `($a - $b).TotalDays` de .NET Framework : les ticks multipliés par l'inverse d'un jour (pas divisés : le
    /// dernier chiffre peut différer, et `[int]` arrondit les moitiés au pair).
    pub fn jours_depuis(self, avant: Moment) -> f64 {
        (self.0 - avant.0) as f64 * (1.0 / TICKS_PAR_JOUR as f64)
    }

    /// Lit « 2026-10-31T12:05:00 » ou « 2026-10-31T12:05:00.1234567 » (le format des juges).
    pub fn lire(texte: &str) -> Option<Moment> {
        let (date, heure) = texte.split_once('T')?;
        let mut d = date.splitn(3, '-');
        let annee = d.next()?.parse().ok()?;
        let mois = d.next()?.parse().ok()?;
        let jour = d.next()?.parse().ok()?;
        let (hms, fraction) = heure.split_once('.').unwrap_or((heure, ""));
        let mut h = hms.splitn(3, ':');
        let hh = h.next()?.parse().ok()?;
        let mm = h.next()?.parse().ok()?;
        let ss = h.next()?.parse().ok()?;
        if fraction.len() > 7 || !fraction.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let ticks = if fraction.is_empty() { 0 } else { format!("{fraction:0<7}").parse().ok()? };
        Moment::nouveau(annee, mois, jour, hh, mm, ss, ticks)
    }

    /// « 2026-10-31T12:05:00.0000000 » (le format des juges).
    pub fn ecrire(self) -> String {
        let (a, mo, j) = self.date();
        let (h, mi, s, t) = self.heure();
        format!("{a:04}-{mo:02}-{j:02}T{h:02}:{mi:02}:{s:02}.{t:07}")
    }

    /// « 5 octobre », « 1er novembre » : le `$quand` de Get-IndicesWindows (`ToString('d MMMM', fr-FR)`).
    pub fn quand(self) -> String {
        let (_, mois, jour) = self.date();
        let nom = MOIS[(mois - 1) as usize];
        if jour == 1 { format!("1er {nom}") } else { format!("{jour} {nom}") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour() {
        for t in ["0001-01-01T00:00:00.0000000", "1970-01-01T00:00:00.0000000", "2024-02-29T23:59:59.9999999"] {
            assert_eq!(Moment::lire(t).map(Moment::ecrire).as_deref(), Some(t));
        }
        assert!(Moment::lire("2026-02-30T00:00:00").is_none());
        // (les ticks de .NET : ([datetime]'2026-10-31T12:00:00').Ticks)
        assert_eq!(Moment::lire("2026-10-31T12:00:00").map(|m| m.0), Some(639_290_448_000_000_000));
    }

    #[test]
    fn dates_en_francais() {
        assert_eq!(Moment::lire("2026-11-01T08:00:00").map(Moment::quand).as_deref(), Some("1er novembre"));
        assert_eq!(Moment::lire("2026-08-15T08:00:00").map(Moment::quand).as_deref(), Some("15 août"));
        assert_eq!(Moment::lire("2026-02-10T08:00:00").map(Moment::quand).as_deref(), Some("10 février"));
    }
}
