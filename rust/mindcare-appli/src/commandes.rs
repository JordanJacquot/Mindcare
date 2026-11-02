//! Les commandes que la page de Mindcare peut appeler : les deux seules, chacune déclarée dans le manifeste de
//! build.rs et autorisée à la fenêtre « principale » seulement (capabilities\principale.json). Elles ne lisent ni
//! n'écrivent rien sur le disque et ne lancent rien : elles passent l'état à la page et notent ce qu'elle voit.

use std::sync::{Mutex, MutexGuard};

use serde_json::{Map, Value, json};
use tauri::State;
use tauri::ipc::Channel;

/// Une couleur que la page accepte : « #RRGGBB » (celles des thèmes de suite-commun, vérifiées à la lecture).
fn couleur_valable(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// Un nom de variable CSS que la page accepte : des lettres et des chiffres (« P », « fond2 »).
fn nom_valable(n: &str) -> bool {
    (1..=16).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Ce que la page affiche (titre, version, gérée par Mindset ou non, couleurs du thème), en JSON. Les couleurs sont
/// les variables CSS du thème (`suite_commun::themes::Theme::variables_css` : `--P`, `--S`, `--T`, `--fond`,
/// `--fond2`) ; une variable au nom ou à la couleur mal écrits n'est pas passée (la page vérifie aussi).
pub fn etat_page(version: &str, geree: bool, variables: &[(&str, String)]) -> Value {
    let mut couleurs = Map::new();
    for (nom, valeur) in variables {
        if nom_valable(nom) && couleur_valable(valeur) {
            couleurs.insert((*nom).to_string(), json!(valeur));
        }
    }
    json!({ "titre": crate::NOM_APPLI, "version": version, "geree": geree, "couleurs": couleurs })
}

/// Le journal de l'appli, vu d'ici (une ligne de texte)
type Journal = Box<dyn Fn(&str) + Send + Sync>;

/// L'état partagé entre l'appli et sa page : le dernier état connu, le canal ouvert par la page (un seul : une
/// page rechargée reprend sa place), et ce que la page a dit de sa visibilité.
pub struct Page {
    etat: Mutex<Value>,
    canal: Mutex<Option<Channel<Value>>>,
    visible: Mutex<Option<bool>>,
    journal: Journal,
}

/// Un verrou, même si un autre fil s'est arrêté en le tenant (la valeur gardée reste bonne : un simple
/// remplacement)
fn tenir<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Page {
    pub fn new(etat: Value, journal: impl Fn(&str) + Send + Sync + 'static) -> Page {
        Page { etat: Mutex::new(etat), canal: Mutex::new(None), visible: Mutex::new(None), journal: Box::new(journal) }
    }

    /// Le dernier état connu
    pub fn etat(&self) -> Value {
        tenir(&self.etat).clone()
    }

    /// Un nouvel état (thème changé...) : gardé, et envoyé à la page si elle suit déjà
    pub fn changer(&self, etat: Value) {
        *tenir(&self.etat) = etat.clone();
        if let Some(canal) = tenir(&self.canal).as_ref()
            && let Err(e) = canal.send(etat)
        {
            (self.journal)(&format!("Page : état non envoyé ({e})"));
        }
    }

    /// Ce que la page a dit en dernier : visible (`Some(true)`), cachée, ou rien encore
    pub fn visible(&self) -> Option<bool> {
        *tenir(&self.visible)
    }
}

/// Les lignes du journal quand la page change de visibilité
pub const JOURNAL_CACHEE: &str = "Écran : caché, en pause";
pub const JOURNAL_VISIBLE: &str = "Écran : de nouveau à l'écran";

/// La page commence à suivre l'état de l'appli : il lui est envoyé tout de suite, puis à chaque changement
/// ([`Page::changer`]).
#[tauri::command]
pub fn suivre_page(canal: Channel<Value>, page: State<'_, Page>) -> Result<(), String> {
    canal.send(page.etat()).map_err(|e| format!("état non envoyé ({e})"))?;
    *tenir(&page.canal) = Some(canal);
    Ok(())
}

/// La page est devenue visible ou cachée (document.visibilityState) : noté au journal quand cela change. Cachée,
/// WebView2 met la page en pause (la veille, `SetIsVisible`, décidée par la vie de l'appli).
#[tauri::command]
pub fn noter_visibilite(visible: bool, page: State<'_, Page>) {
    let avant = tenir(&page.visible).replace(visible);
    if avant != Some(visible) {
        (page.journal)(if visible { JOURNAL_VISIBLE } else { JOURNAL_CACHEE });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etat_page_avec_et_sans_theme() {
        let theme = suite_commun::themes::Theme::neon();
        let e = etat_page("0.1.0", true, &theme.variables_css());
        assert_eq!(e["titre"], "Mindcare");
        assert_eq!(e["version"], "0.1.0");
        assert_eq!(e["geree"], true);
        assert_eq!(
            e["couleurs"],
            json!({ "P": "#00E5FF", "S": "#B04BFF", "T": "#FF2EB5", "fond": "#06050F", "fond2": "#0C0920" })
        );
        let e = etat_page("0.1.0", false, &[]);
        assert_eq!(e["geree"], false);
        assert_eq!(e["couleurs"], json!({}));
    }

    /// Une couleur ou un nom mal écrits ne vont pas à la page (contre-épreuve : sans le contrôle, ils passeraient).
    #[test]
    fn variables_mal_ecrites_ecartees() {
        let v = [
            ("P", "#00E5FF".to_string()),
            ("S", "red".to_string()),
            ("T", "#FF2EB5;background:url(x)".to_string()),
            ("fond", "#80112233".to_string()),
            ("x}y", "#000000".to_string()),
            ("", "#000000".to_string()),
        ];
        assert_eq!(etat_page("0.1.0", true, &v)["couleurs"], json!({ "P": "#00E5FF" }));
    }
}
