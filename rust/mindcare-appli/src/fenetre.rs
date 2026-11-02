//! La fenêtre « principale » de Mindcare, créée dans le code (règle 7 de docs\regles-tauri.md, dépôt Mindset : le
//! dossier WebView2 se calcule au lancement). Une seule, jamais recréée ; le programme (étape 4.4) l'appelle une fois.
//!
//! - cachée et sans prendre la main au départ : Mindset l'accroche, puis la montre (contrat-appli, partie 7) ;
//! - titre exact « Mindcare » (Mindset la retrouve à sa classe « Tauri Window » et à ce titre) ;
//! - dossier WebView2 `<données de la suite>\WebView2-Tauri\Mindcare` s'il s'écrit, sinon
//!   `<données de Mindcare>\ecran-webview`, et options fixes du navigateur : tout par suite-commun ;
//! - la taille de la fenêtre de Mindcare PowerShell (interface.xaml:3 : 1100 x 760, au moins 900 x 620) ;
//! - outils de développement, raccourcis de zoom, saisie automatique et téléchargements coupés ; navigation fermée
//!   aux pages de l'appli, aucune nouvelle fenêtre. Le menu du clic droit, les raccourcis du navigateur et la saisie
//!   des mots de passe se coupent dans les réglages de WebView2, que Tauri ne transmet pas (src\windows\webview2.rs).

use std::path::Path;
use std::sync::Arc;

use suite_commun::webview::{self, DossierEcran};
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{Manager, Runtime, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Le nom (label) de la fenêtre : celui de capabilities\principale.json
pub const LABEL: &str = "principale";
/// La taille de la fenêtre (interface.xaml:3, `Width` et `Height`), en pixels logiques
pub const TAILLE: (f64, f64) = (1100.0, 760.0);
/// La taille minimale hors de Mindset (interface.xaml:3, `MinWidth` et `MinHeight`) ; 0 une fois accrochée
pub const TAILLE_MIN: (f64, f64) = (900.0, 620.0);

/// Seules les pages de l'appli s'affichent : `http://tauri.localhost/...` (l'adresse des pages rangées dans l'exe,
/// sous Windows), sans nom d'utilisateur, mot de passe ni port. Toute autre adresse est refusée.
pub fn adresse_permise(adresse: &Url) -> bool {
    adresse.scheme() == "http"
        && adresse.host_str() == Some("tauri.localhost")
        && adresse.port().is_none()
        && adresse.username().is_empty()
        && adresse.password().is_none()
}

/// L'adresse de la page de l'appli (`WebviewUrl::App("index.html")`, rangée dans l'exe, sous Windows)
pub const ADRESSE_PAGE: &str = "http://tauri.localhost/index.html";

/// Le journal de l'appli, vu de la fenêtre (une ligne de texte)
pub type Journal = Arc<dyn Fn(&str) + Send + Sync>;

/// Une adresse coupée pour le journal (elle peut venir d'une page)
fn court(adresse: &Url) -> String {
    adresse.as_str().chars().take(120).collect()
}

/// Crée la fenêtre « principale », cachée. `donnees_suite` : le dossier des données de la suite
/// (`%LOCALAPPDATA%\Mindset`, ou `X:\Mindset\Donnees`) ; `donnees_appli` : celui de Mindcare. Rend aussi le dossier
/// WebView2 choisi (et pourquoi le repli, s'il a fallu) pour le journal. Le chargement de la page et chaque adresse
/// refusée vont au `journal` (pour aider à distance).
pub fn creer_principale<R: Runtime, M: Manager<R>>(
    app: &M,
    donnees_suite: &Path,
    donnees_appli: &Path,
    journal: Journal,
) -> tauri::Result<(WebviewWindow<R>, DossierEcran)> {
    let choix = webview::choisir_dossier(donnees_suite, crate::NOM_APPLI, donnees_appli);
    let pour_navigation = Arc::clone(&journal);
    let fenetre = WebviewWindowBuilder::new(app, "principale", WebviewUrl::App("index.html".into()))
        .title(crate::NOM_APPLI)
        .inner_size(TAILLE.0, TAILLE.1)
        .min_inner_size(TAILLE_MIN.0, TAILLE_MIN.1)
        .center()
        .visible(false)
        .focused(false)
        .devtools(false)
        .zoom_hotkeys_enabled(false)
        .general_autofill_enabled(false)
        .data_directory(choix.dossier.clone())
        .additional_browser_args(webview::OPTIONS_NAVIGATEUR)
        .on_navigation(move |a| {
            let permise = adresse_permise(a);
            if !permise {
                pour_navigation(&format!("Écran : adresse refusée {}", court(a)));
            }
            permise
        })
        .on_page_load(move |_, p| {
            if p.event() == PageLoadEvent::Finished {
                journal(&format!("Écran : page chargée {}", court(p.url())));
            }
        })
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
        for a in [
            ADRESSE_PAGE,
            "http://tauri.localhost/",
            "http://tauri.localhost/index.html",
            "http://tauri.localhost/ui/page.js?x=1",
        ] {
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
