//! Construction de Mindset.exe (le programme de Mindcare) par tauri-build : rien d'autre que des fichiers lus.
//!
//! - le manifeste des commandes de l'appli (docs\regles-tauri.md du dépôt Mindset, règle 2) : sans lui, Tauri
//!   laisserait toute page locale appeler toutes les commandes ; avec lui, chaque commande demande sa permission
//!   `allow-<commande>`, donnée fenêtre par fenêtre dans capabilities\ ;
//! - l'icône de Mindcare (icons\Mindcare.ico, copie de Mindcare\fichiers\Mindcare.ico) ;
//! - la fiche de version : tauri-build écrit productName (« Mindset », tauri.conf.json) comme description de l'exe,
//!   ce qu'affiche le Gestionnaire des tâches (contrat-appli, partie 4), et la version du Cargo.toml ;
//! - le runtime C rangé dans l'exe (réglage par défaut de tauri-build) : aucune VCRUNTIME140.dll à installer.

fn main() {
    println!("cargo:rerun-if-changed=icons/Mindcare.ico");
    let windows = tauri_build::WindowsAttributes::new().window_icon_path("icons/Mindcare.ico");
    // Les commandes de src\commandes.rs, toutes ici (verifier-tauri.ps1 compare les deux listes)
    let attributs = tauri_build::Attributes::new()
        .windows_attributes(windows)
        .app_manifest(tauri_build::AppManifest::new().commands(&["suivre_page", "noter_visibilite"]));
    tauri_build::try_build(attributs).expect("construction de Mindcare (tauri-build)");
    // Les exe des tests (tests\) : tauri-build ne donne son manifeste qu'au programme. Sans les contrôles modernes de
    // Windows (comctl32 version 6, option « common-controls-v6 »), ils ne démarrent pas (STATUS_ENTRYPOINT_NOT_FOUND).
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' \
             version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
