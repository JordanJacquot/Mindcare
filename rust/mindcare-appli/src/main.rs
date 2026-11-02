//! Mindcare en Rust : le programme Mindset.exe du dossier de Mindcare (étape 4 du plan « Mindcare en Rust »). Tout
//! est dans la bibliothèque (src\programme.rs) : le démarrage, la fenêtre « Mindcare » dans Mindset, sa vie et sa
//! fin. Rien ici n'est livré aux utilisateurs pour l'instant.

// Pas de console : c'est une appli à fenêtre (même en construction de mise au point, aucune console ne s'ouvre).
#![windows_subsystem = "windows"]
#![deny(unsafe_code)]

fn main() {
    mindcare_appli::programme::lancer()
}
