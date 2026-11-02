//! Les permissions de la page (docs\regles-tauri.md du dépôt Mindset, règle 2), avec la vraie configuration
//! (tauri.conf.json, capabilities\, manifeste de build.rs) et le faux Windows de Tauri (option « test ») : aucune
//! vraie fenêtre, aucune page chargée, rien d'écrit ailleurs que dans un dossier d'essai sous %TEMP%.
//!
//! Une commande autorisée passe ; tout le reste est refusé : une commande hors du manifeste, une commande de Tauri
//! lui-même (aucune permission `core:` donnée), une commande de l'appli depuis une autre fenêtre que « principale »,
//! ou depuis une adresse extérieure.

use std::path::PathBuf;

use mindcare_appli::{Page, commandes, fenetre};
use serde_json::json;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Un dossier d'essai neuf sous le dossier temporaire, effacé à la fin (jamais les vraies données)
struct Essai(PathBuf);
impl Essai {
    fn new(nom: &str) -> Essai {
        let d = std::env::temp_dir().join(format!("mindcare-permissions-{}-{nom}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dossier d'essai");
        Essai(d)
    }
}
impl Drop for Essai {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn appli() -> App<MockRuntime> {
    let page = Page::new(commandes::etat_page("0.1.0", true, None), |_| {});
    mindcare_appli::preparer(mock_builder(), page).build(mindcare_appli::contexte()).expect("appli d'essai")
}

/// La fenêtre « principale », créée par le vrai code de l'appli (avec le faux Windows de Tauri)
fn principale(app: &App<MockRuntime>, essai: &Essai) -> WebviewWindow<MockRuntime> {
    let (f, choix) =
        fenetre::creer_principale(app, &essai.0.join("suite"), &essai.0.join("Mindcare")).expect("fenêtre");
    // Le dossier WebView2 est celui de la suite, sous le dossier d'essai
    assert_eq!(choix.dossier, essai.0.join("suite").join("WebView2-Tauri").join("Mindcare"));
    assert!(!choix.repli);
    f
}

fn demande(commande: &str, adresse: &str, corps: serde_json::Value) -> InvokeRequest {
    InvokeRequest {
        cmd: commande.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: adresse.parse().expect("adresse"),
        body: InvokeBody::Json(corps),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

const PAGES: &str = "http://tauri.localhost/index.html";

/// Refusée par le contrôle des permissions de Tauri (et pas pour une autre raison, un argument manquant par exemple)
fn refusee(r: Result<tauri::ipc::InvokeResponseBody, serde_json::Value>, quoi: &str) {
    match r {
        Err(serde_json::Value::String(raison)) => {
            assert!(raison.contains("not allowed"), "{quoi} refusée, mais pas par les permissions : {raison}")
        }
        autre => panic!("{quoi} aurait dû être refusée par les permissions : {autre:?}"),
    }
}

#[test]
fn la_fenetre_principale_appelle_ses_commandes() {
    let essai = Essai::new("permise");
    let app = appli();
    let f = principale(&app, &essai);
    // (le faux Windows de Tauri ne garde pas le titre : « Mindcare » est vérifié sur la vraie fenêtre, étape 4.4)
    assert_eq!(f.label(), "principale");
    let r = get_ipc_response(&f, demande("noter_visibilite", PAGES, json!({ "visible": true })));
    assert!(r.is_ok(), "noter_visibilite refusée : {r:?}");
    assert_eq!(app.state::<Page>().visible(), Some(true));
    // Le canal de l'état : ouvert par la page, l'état lui est envoyé
    let r = get_ipc_response(&f, demande("suivre_page", PAGES, json!({ "canal": "__CHANNEL__:7" })));
    assert!(r.is_ok(), "suivre_page refusée : {r:?}");
    // Un argument du mauvais type est refusé par la commande elle-même
    let r = get_ipc_response(&f, demande("noter_visibilite", PAGES, json!({ "visible": "oui" })));
    assert!(r.is_err());
    assert_eq!(app.state::<Page>().visible(), Some(true));
}

#[test]
fn une_commande_hors_du_manifeste_est_refusee() {
    let essai = Essai::new("hors-manifeste");
    let app = appli();
    let f = principale(&app, &essai);
    for commande in ["ouvrir_fichier", "plugin:shell|open", "plugin:fs|read_file"] {
        refusee(get_ipc_response(&f, demande(commande, PAGES, json!({}))), commande);
    }
}

#[test]
fn les_commandes_de_tauri_lui_meme_sont_refusees() {
    let essai = Essai::new("core");
    let app = appli();
    let f = principale(&app, &essai);
    for (commande, corps) in [
        ("plugin:window|close", json!({ "label": "principale" })),
        ("plugin:webview|internal_toggle_devtools", json!({})),
        ("plugin:event|emit", json!({ "event": "essai", "payload": null })),
        ("plugin:app|version", json!({})),
        ("plugin:path|resolve_directory", json!({ "directory": 1 })),
    ] {
        refusee(get_ipc_response(&f, demande(commande, PAGES, corps)), commande);
    }
}

#[test]
fn une_autre_fenetre_ne_peut_rien_appeler() {
    let essai = Essai::new("autre-fenetre");
    let app = appli();
    let _ = principale(&app, &essai);
    // (une fenêtre que l'appli ne crée jamais, seulement pour l'essai, dans le faux Windows de Tauri)
    let autre = WebviewWindowBuilder::new(&app, "autre", WebviewUrl::App("index.html".into())).build().expect("autre");
    for (commande, corps) in
        [("noter_visibilite", json!({ "visible": true })), ("suivre_page", json!({ "canal": "__CHANNEL__:7" }))]
    {
        refusee(get_ipc_response(&autre, demande(commande, PAGES, corps)), commande);
    }
    assert_eq!(app.state::<Page>().visible(), None);
}

#[test]
fn une_adresse_exterieure_ne_peut_rien_appeler() {
    let essai = Essai::new("exterieure");
    let app = appli();
    let f = principale(&app, &essai);
    // (pack.localhost aussi : le protocole des images du thème n'est pas une page de l'appli)
    for adresse in ["https://exemple.com/", "http://tauri.localhost.exemple.com/", "http://pack.localhost/x.png"] {
        refusee(get_ipc_response(&f, demande("noter_visibilite", adresse, json!({ "visible": true }))), adresse);
    }
    assert_eq!(app.state::<Page>().visible(), None);
}
