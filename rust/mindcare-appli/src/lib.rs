//! Mindcare en Rust : la fenêtre de Mindcare dans Mindset et son programme (étape 4 du plan « Mindcare en Rust »).
//!
//! - [`fenetre`] : la fenêtre « principale », créée dans le code (jamais dans tauri.conf.json), cachée et sans
//!   prendre la main, titre exact « Mindcare », dossier WebView2 et options du navigateur de suite-commun
//!   (docs\regles-tauri.md du dépôt Mindset, règle 7), navigation fermée, aucune nouvelle fenêtre ;
//! - [`commandes`] : les deux seules commandes que la page peut appeler (`suivre_page`, `noter_visibilite`),
//!   déclarées dans build.rs et autorisées à la seule fenêtre « principale » (capabilities\principale.json) ;
//! - [`pages`] et [`effets`] : les pages de Mindcare pour le kit d'écrans, et leurs effets exécutés (enquête gardée,
//!   rapport, presse-papiers, lecture de Windows) ;
//! - [`pack`] : la réponse du protocole `pack` (les images et polices du thème, règle 6), par suite-commun ;
//! - [`preparer`] et [`contexte`] : ce que le programme et les tests donnent à Tauri ;
//! - `programme` : le programme Mindset.exe tout branché (étape 4.4 : démarrage, verrou, fenêtre, surveillances) ;
//! - `windows` : le seul module où le code « unsafe » est permis (verrou, signal, fenêtre, registre, WebView2) ;
//! - `essai` et `png` : la construction d'essai seulement (option cargo `essai`, jamais livrée).
//!
//! Le programme (src\main.rs) appelle `programme::lancer` ; les tests (tests\permissions.rs) font tourner la
//! fenêtre avec le faux Windows de Tauri (option « test ») : aucune vraie fenêtre, aucune page chargée.

#![deny(unsafe_code)]

pub mod commandes;
pub mod effets;
#[cfg(feature = "essai")]
pub mod essai;
pub mod fenetre;
pub mod pack;
pub mod pages;
#[cfg(any(feature = "essai", test))]
pub mod png;
pub mod pont;
#[cfg(windows)]
pub mod programme;
#[cfg(windows)]
pub mod windows;

pub use commandes::Page;

/// Le nom de l'appli : titre de sa fenêtre et de sa page, nom de son dossier WebView2 (`WebView2-Tauri\Mindcare`)
pub const NOM_APPLI: &str = "Mindcare";
/// La version de l'appli (celle de son Cargo.toml, la même que version.json et la fiche de version de l'exe)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ajoute à Tauri la page partagée et les commandes de l'appli (les seules que la page peut appeler, chacune avec
/// sa permission dans capabilities\principale.json).
pub fn preparer<R: tauri::Runtime>(constructeur: tauri::Builder<R>, page: Page) -> tauri::Builder<R> {
    constructeur.manage(page).invoke_handler(tauri::generate_handler![
        commandes::suivre_page,
        commandes::noter_visibilite,
        commandes::action
    ])
}

/// La configuration de Tauri lue à la construction (tauri.conf.json, capabilities\, les pages de ui\ rangées dans
/// l'exe, avec leurs empreintes ajoutées à la CSP).
pub fn contexte<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}
