//! Copié de D:\Projets\Nevermind\rust\aide-regles\src\client.rs (plan de l'Aide de Mindcare, 14/11/2026).
//!
//! Qui peut parler à l'Aide : seulement Mindset.exe du dossier de cette copie de Mindcare (le programme qui fait
//! tourner Mindcare), reconnu par son chemin complet lu par Windows. Jamais par le nom : toutes les applis de la suite
//! tournent dans un « Mindset.exe ». Le relais Mindcare.exe ne parle jamais à l'Aide.

use crate::{chemin, texte};

/// Le programme `chemin_client` (tel que Windows le donne) est-il Mindset.exe du dossier `racine` ?
/// Le chemin reçu est comparé tel quel : rien n'est « nettoyé » qui pourrait faire passer un autre fichier pour lui.
pub fn accepte(chemin_client: &str, racine: &str) -> bool {
    if chemin_client.is_empty() || racine.is_empty() {
        return false;
    }
    match chemin::complet(&chemin::joindre(racine, "Mindset.exe")) {
        Some(attendu) => texte::egal(chemin_client, &attendu),
        None => false,
    }
}
