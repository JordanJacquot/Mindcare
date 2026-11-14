//! Copié de D:\Projets\Nevermind\rust\aide-regles\src\tache.rs (plan de l'Aide de Mindcare, 14/11/2026).
//!
//! Mindcare ne lance la tâche de l'Aide que si c'est la tâche SYSTEM pour son utilisateur, lue dans le XML de
//! `schtasks /Query /XML` (que l'utilisateur a le droit de lire : l'installateur de Mindset lui donne la lecture et le
//! lancement de sa tâche, à lui seul).

use crate::{texte, xml};

/// Le nom complet de la tâche (créée par l'installateur de Mindset, champ « aide » de apps.json)
pub const TACHE: &str = "\\Mindset\\Mindcare (aide)";
/// Le programme de l'Aide, à côté de Mindset.exe dans le dossier de Mindcare (champ « aide_exe » de apps.json)
pub const EXE: &str = "MindcareAide.exe";

/// L'état de la tâche :
/// - `"systeme"` : la bonne (en SYSTEM, MindcareAide.exe de ce dossier, pour cet utilisateur (`-Sid`), active) ;
/// - `"autre"` : une tâche à ce nom, mais pas celle-là (désactivée, autre compte, autre programme...) ;
/// - `""` : pas de tâche (ou illisible pour cet utilisateur).
///
/// `traduire` : un nom de compte en SID (Windows), pour une tâche dont le compte est écrit par son nom.
pub fn etat(xml_tache: &str, exe: &str, sid: &str, traduire: &dyn Fn(&str) -> Option<String>) -> &'static str {
    if xml_tache.is_empty() {
        return "";
    }
    let Some(doc) = xml::lire(xml_tache) else {
        return "autre";
    };
    if doc.nom != "Task" {
        return "autre";
    }
    let mut compte = doc.texte_de(&["Principals", "Principal", "UserId"]);
    if !compte.is_empty() && !texte::commence(&compte, "S-1-") {
        compte = traduire(&compte).unwrap_or_default();
    }
    let actions = doc.chercher(&["Actions", "Exec"]);
    let active = !texte::egal(&doc.texte_de(&["Settings", "Enabled"]), "false");
    if texte::egal(&compte, "S-1-5-18") && active && actions.len() == 1 {
        let commande = actions[0].texte_de(&["Command"]);
        let arguments = actions[0].texte_de(&["Arguments"]);
        if texte::egal(commande.trim_matches('"'), exe) && texte::sans_espaces(&arguments) == format!("-Sid {sid}") {
            return "systeme";
        }
    }
    "autre"
}
