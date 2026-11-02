//! Mindcare en Rust : le programme Mindset.exe du dossier de Mindcare (étape 4 du plan « Mindcare en Rust »).
//!
//! Étapes 4.1 et 4.2 : les dépendances figées (Tauri, windows), la configuration de Tauri (tauri.conf.json,
//! capabilities\, build.rs), la page (ui\) et la bibliothèque de l'appli (la fenêtre, ses commandes, le protocole
//! `pack`), construites et vérifiées. Ce programme ne crée encore aucune fenêtre et ne fait rien : il lance la
//! fenêtre « principale » à l'étape 4.4, avec sa vie dans Mindset (crate mindcare-vie). Rien ici n'est livré aux
//! utilisateurs.

// Pas de console : c'est une appli à fenêtre (même en construction de mise au point, aucune console ne s'ouvre).
#![windows_subsystem = "windows"]

fn main() {
    // (la bibliothèque et Tauri sont nommés pour qu'ils soient bien construits et liés dès maintenant)
    let _ = (tauri::VERSION, mindcare_appli::VERSION);
}
