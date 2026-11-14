//! Copié de D:\Projets\Nevermind\rust\aide-regles\src\serveur.rs (plan de l'Aide de Mindcare, 14/11/2026).
//!
//! Mindcare reconnaît l'Aide : un canal créé par le système (l'Aide en SYSTEM) ou par les administrateurs, qu'un
//! programme sans droits ne peut pas créer ; sinon, celui qui répond doit être MindcareAide.exe de ce dossier.

use crate::texte;

/// Mindcare parle-t-il au programme `chemin` (créateur du canal), propriétaire du canal `proprietaire` ?
pub fn accepte(chemin: &str, proprietaire: &str, exe_attendu: &str) -> bool {
    if proprietaire == "S-1-5-18" || proprietaire == "S-1-5-32-544" {
        return true;
    }
    !chemin.is_empty() && !exe_attendu.is_empty() && texte::egal(chemin, exe_attendu)
}
