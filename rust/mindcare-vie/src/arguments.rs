//! Les arguments du programme.
//!
//! - `Mindcare.ps1:11` : `param([switch]$Demarrage)`, le seul paramètre du script ;
//! - `outils\lanceur.cs:36` : le lanceur le passe au script si un argument vaut `-Demarrage`, sans tenir compte des
//!   majuscules (`StringComparison.OrdinalIgnoreCase`), où qu'il soit ; tout autre argument est ignoré.
//!
//! Écart voulu (plan de l'étape 4 et contrat-appli partie 6) : `/Demarrage` est accepté aussi, comme le lanceur de
//! Mindset. Le lanceur de Mindcare PowerShell ne le connaissait pas.

use std::ffi::OsStr;

/// Le nom de l'option, sans son « - » ou « / ».
const DEMARRAGE: &str = "Demarrage";

/// Un argument qui demande le lancement caché : `-Demarrage` ou `/Demarrage`, sans tenir compte des majuscules
/// (comparaison ordinale : seulement les lettres A à Z, comme OrdinalIgnoreCase sur ce mot).
fn est_demarrage(a: &OsStr) -> bool {
    let Some(a) = a.to_str() else { return false };
    // (lanceur.cs:36 : -Demarrage ; plan 4.3 : aussi /Demarrage)
    a.strip_prefix('-').or_else(|| a.strip_prefix('/')).is_some_and(|n| n.eq_ignore_ascii_case(DEMARRAGE))
}

/// `-Demarrage` parmi les arguments (sans le nom du programme) : lancée cachée, par Mindset ou avec Windows
/// (lanceur.cs:36, `Array.Exists`).
pub fn demarrage_demande<I, S>(arguments: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    arguments.into_iter().any(|a| est_demarrage(a.as_ref()))
}
