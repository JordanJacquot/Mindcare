//! L'identité de l'appli pour la barre des tâches : `JordanJacquot.Mindcare`, la même que le raccourci que Mindset
//! écrit dans le menu Démarrer (contrat-appli, partie 4 ; Mindcare.ps1:65).

use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::HSTRING;

/// L'identité de Mindcare (`JordanJacquot.<Nom>`, applis.ps1:43).
pub const ID_APPLI: &str = "JordanJacquot.Mindcare";

/// Pose l'identité du processus (avant toute fenêtre).
pub fn poser() -> windows::core::Result<()> {
    let id = HSTRING::from(ID_APPLI);
    // SAFETY : texte valide pendant l'appel ; seulement un réglage de ce processus
    unsafe { SetCurrentProcessExplicitAppUserModelID(&id) }
}
