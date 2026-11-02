//! Mindcare en Rust : le programme Mindset.exe du dossier de Mindcare (étape 4 du plan « Mindcare en Rust »).
//!
//! Étape 4.1 : seulement les dépendances figées (Tauri, windows) et construites. Ce programme ne crée encore aucune
//! fenêtre et ne fait rien : la fenêtre « principale » arrive à l'étape 4.4, sa vie dans Mindset (verrou,
//! demandes, accrochage) est dans la crate mindcare-vie. Rien ici n'est livré aux utilisateurs.

// Pas de console : c'est une appli à fenêtre (même en construction de mise au point, aucune console ne s'ouvre).
#![windows_subsystem = "windows"]

fn main() {
    // (les dépendances sont nommées pour qu'elles soient bien construites et liées dès maintenant)
    let _ = tauri::VERSION;
}
