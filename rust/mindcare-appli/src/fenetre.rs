//! La fenêtre « principale » de Mindcare, créée dans le code (règle 7 de docs\regles-tauri.md, dépôt Mindset : le
//! dossier WebView2 se calcule au lancement). Une seule, jamais recréée ; le programme (étape 4.4) l'appelle une fois.
//!
//! - cachée et sans prendre la main au départ : Mindset l'accroche, puis la montre (contrat-appli, partie 7) ;
//! - titre exact « Mindcare » (Mindset la retrouve à sa classe « Tauri Window » et à ce titre) ;
//! - dossier WebView2 `<données de la suite>\WebView2-Tauri\Mindcare` s'il s'écrit, sinon
//!   `<données de Mindcare>\ecran-webview`, et options fixes du navigateur : tout par suite-commun ;
//! - outils de développement, raccourcis de zoom et téléchargements coupés ; navigation fermée aux pages de l'appli,
//!   aucune nouvelle fenêtre. Le menu du clic droit, les raccourcis du navigateur et la saisie automatique se
//!   coupent dans les réglages de WebView2 (étape 4.4, src\windows).

use std::path::Path;

use suite_commun::webview::{self, DossierEcran};
use tauri::webview::NewWindowResponse;
use tauri::{Manager, Runtime, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Le nom (label) de la fenêtre : celui de capabilities\principale.json
pub const LABEL: &str = "principale";

/// Seules les pages de l'appli s'affichent : `http://tauri.localhost/...` (l'adresse des pages rangées dans l'exe,
/// sous Windows), sans nom d'utilisateur, mot de passe ni port. Toute autre adresse est refusée.
pub fn adresse_permise(adresse: &Url) -> bool {
    adresse.scheme() == "http"
        && adresse.host_str() == Some("tauri.localhost")
        && adresse.port().is_none()
        && adresse.username().is_empty()
        && adresse.password().is_none()
}

/// Crée la fenêtre « principale », cachée. `donnees_suite` : le dossier des données de la suite
/// (`%LOCALAPPDATA%\Mindset`, ou `X:\Mindset\Donnees`) ; `donnees_appli` : celui de Mindcare. Rend aussi le dossier
/// WebView2 choisi (et pourquoi le repli, s'il a fallu) pour le journal.
pub fn creer_principale<R: Runtime, M: Manager<R>>(
    app: &M,
    donnees_suite: &Path,
    donnees_appli: &Path,
) -> tauri::Result<(WebviewWindow<R>, DossierEcran)> {
    let choix = webview::choisir_dossier(donnees_suite, crate::NOM_APPLI, donnees_appli);
    let fenetre = WebviewWindowBuilder::new(app, "principale", WebviewUrl::App("index.html".into()))
        .title(crate::NOM_APPLI)
        .visible(false)
        .focused(false)
        .devtools(false)
        .zoom_hotkeys_enabled(false)
        .data_directory(choix.dossier.clone())
        .additional_browser_args(webview::OPTIONS_NAVIGATEUR)
        .on_navigation(adresse_permise)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .build()?;
    Ok((fenetre, choix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adresse(a: &str) -> Url {
        a.parse().expect("adresse d'essai")
    }

    #[test]
    fn seules_les_pages_de_l_appli_sont_permises() {
        for a in
            ["http://tauri.localhost/", "http://tauri.localhost/index.html", "http://tauri.localhost/ui/page.js?x=1"]
        {
            assert!(adresse_permise(&adresse(a)), "{a} devrait être permise");
        }
        for a in [
            "https://tauri.localhost/",
            "http://tauri.localhost:8080/",
            "http://ipc.localhost/suivre_page",
            "http://pack.localhost/fond.png",
            "http://tauri.localhost.exemple.com/",
            "http://exemple.com/",
            "https://github.com/JordanJacquot/Mindset",
            "http://moi:secret@tauri.localhost/",
            "file:///C:/Windows/win.ini",
            "about:blank",
            "javascript:alert(1)",
            "data:text/html,salut",
        ] {
            assert!(!adresse_permise(&adresse(a)), "{a} devrait être refusée");
        }
    }
}
