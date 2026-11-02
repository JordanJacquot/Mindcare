//! Les commandes que la page de Mindcare peut appeler : les deux seules, chacune déclarée dans le manifeste de
//! build.rs et autorisée à la fenêtre « principale » seulement (capabilities\principale.json). Elles ne lisent ni
//! n'écrivent rien sur le disque et ne lancent rien : elles passent l'état à la page et notent ce qu'elle voit.

use std::sync::{Mutex, MutexGuard};

use serde_json::{Value, json};
use suite_commun::themes::Couleurs;
use tauri::State;
use tauri::ipc::Channel;

/// Ce que la page affiche (titre, version, gérée par Mindset ou non, couleurs du thème), en JSON. Seules les trois
/// couleurs de la marque du thème sont passées, au format « #RRGGBB » vérifié par suite-commun.
pub fn etat_page(version: &str, geree: bool, couleurs: Option<&Couleurs>) -> Value {
    let couleurs = couleurs.map(|c| json!({ "p": c.p, "s": c.s, "t": c.t })).unwrap_or_else(|| json!({}));
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

/// La page commence à suivre l'état de l'appli : il lui est envoyé tout de suite, puis à chaque changement
/// ([`Page::changer`]).
#[tauri::command]
pub fn suivre_page(canal: Channel<Value>, page: State<'_, Page>) -> Result<(), String> {
    canal.send(page.etat()).map_err(|e| format!("état non envoyé ({e})"))?;
    *tenir(&page.canal) = Some(canal);
    Ok(())
}

/// La page est devenue visible ou cachée (document.visibilityState) : noté au journal quand cela change.
#[tauri::command]
pub fn noter_visibilite(visible: bool, page: State<'_, Page>) {
    let avant = tenir(&page.visible).replace(visible);
    if avant != Some(visible) {
        (page.journal)(if visible { "Page : visible" } else { "Page : cachée" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etat_page_avec_et_sans_theme() {
        let c = Couleurs {
            p: "#00E5FF".into(),
            s: "#B04BFF".into(),
            t: "#FF2EB5".into(),
            nh: None,
            ns: 1.0,
            map: Default::default(),
        };
        let e = etat_page("0.1.0", true, Some(&c));
        assert_eq!(e["titre"], "Mindcare");
        assert_eq!(e["version"], "0.1.0");
        assert_eq!(e["geree"], true);
        assert_eq!(e["couleurs"], json!({ "p": "#00E5FF", "s": "#B04BFF", "t": "#FF2EB5" }));
        let e = etat_page("0.1.0", false, None);
        assert_eq!(e["geree"], false);
        assert_eq!(e["couleurs"], json!({}));
    }
}
