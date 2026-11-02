//! L'heure locale du PC, pour le journal et etat.json (`Get-Date`).

use std::sync::Arc;

use mindcare_donnees::{Horloge, Moment};
use windows::Win32::System::SystemInformation::GetLocalTime;

/// Le 1er janvier 2000 : seulement si Windows rendait une date impossible.
const SECOURS: Moment = Moment { annee: 2000, mois: 1, jour: 1, heure: 0, minute: 0, seconde: 0 };

/// L'heure locale, à la seconde.
pub fn maintenant() -> Moment {
    // SAFETY : simple lecture de l'heure, structure rendue par valeur
    let t = unsafe { GetLocalTime() };
    Moment::nouveau(
        i32::from(t.wYear),
        u32::from(t.wMonth),
        u32::from(t.wDay),
        u32::from(t.wHour),
        u32::from(t.wMinute),
        u32::from(t.wSecond),
    )
    .unwrap_or(SECOURS)
}

/// L'horloge du journal.
pub fn horloge() -> Horloge {
    Arc::new(maintenant)
}

#[cfg(test)]
mod tests {
    #[test]
    fn heure_plausible() {
        let m = super::maintenant();
        assert!(m.annee >= 2026, "{m:?}");
    }
}
