//! `maj.vider` : les mises à jour téléchargées mises de côté. Le dossier `<Windows>\SoftwareDistribution` est renommé
//! en `SoftwareDistribution.old` (un `.old` resté d'une fois d'avant est d'abord supprimé : c'est un reste jetable ; s'il
//! ne part pas, le nom est daté). Les services qui s'en servent (Windows Update, puis le service de transfert BITS) sont
//! arrêtés avant, et TOUJOURS remis comme ils étaient après, même quand quelque chose a raté. Jamais un service tué :
//! un service qui ne s'arrête pas (une mise à jour en cours d'installation) fait abandonner proprement.

/// Les services arrêtés avant le renommage, dans cet ordre (relancés dans l'ordre inverse)
pub const SERVICES: [&str; 2] = ["wuauserv", "bits"];

/// Les essais (Windows peut relancer un service entre l'arrêt et le renommage : UsoSvc, WaaSMedicSvc)
pub const ESSAIS: u32 = 3;

/// Le dossier des mises à jour téléchargées
pub fn dossier(windows: &str) -> String {
    format!("{}\\SoftwareDistribution", windows.trim_end_matches('\\'))
}

/// Son nom une fois mis de côté
pub fn mis_de_cote(windows: &str) -> String {
    format!("{}.old", dossier(windows))
}

/// Le nom daté, quand un `.old` resté d'avant ne part pas : `.old-AAAAMMJJ-HHMMSS`, d'après l'heure locale
/// « 2026-11-14T10:05:09 ». `None` si l'heure n'a pas cette forme.
pub fn mis_de_cote_date(windows: &str, heure_locale: &str) -> Option<String> {
    let b = heure_locale.as_bytes();
    let forme = b.len() == 19
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            _ => c.is_ascii_digit(),
        });
    if !forme {
        return None;
    }
    let chiffres: String = heure_locale.chars().filter(char::is_ascii_digit).collect();
    Some(format!("{}-{}-{}", mis_de_cote(windows), &chiffres[..8], &chiffres[8..]))
}

/// Ce que dit Mindcare quand un service ne s'arrête pas
pub const SERVICE_BLOQUE: &str = "Windows est en train d'installer une mise à jour. Redémarre le PC, puis réessaie.";
/// Ce que dit Mindcare quand le dossier n'a pas pu être mis de côté
pub const RENOMMAGE_REFUSE: &str =
    "Windows n'a pas laissé mettre les mises à jour de côté. Redémarre le PC, puis réessaie.";
/// Ce que dit Mindcare quand un service n'a pas pu être relancé
pub const RELANCE_RATEE: &str =
    "Le service des mises à jour ne s'est pas relancé. Redémarre le PC : Windows le relancera tout seul.";
/// Ce que dit Mindcare quand le dossier est un lien (jamais suivi)
pub const LIEN: &str = "Le dossier des mises à jour n'est pas un dossier normal : Mindcare n'y touche pas.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms() {
        assert_eq!(dossier("C:\\Windows"), "C:\\Windows\\SoftwareDistribution");
        assert_eq!(dossier("C:\\Windows\\"), "C:\\Windows\\SoftwareDistribution");
        assert_eq!(mis_de_cote("C:\\Windows"), "C:\\Windows\\SoftwareDistribution.old");
        assert_eq!(
            mis_de_cote_date("C:\\Windows", "2026-11-14T10:05:09").as_deref(),
            Some("C:\\Windows\\SoftwareDistribution.old-20261114-100509")
        );
        assert_eq!(mis_de_cote_date("C:\\Windows", "2026-11-14 10:05:09"), None);
        assert_eq!(mis_de_cote_date("C:\\Windows", "..\\..\\x"), None);
        assert_eq!(mis_de_cote_date("C:\\Windows", ""), None);
    }
}
