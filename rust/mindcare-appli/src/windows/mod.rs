//! Le seul module de Mindcare qui parle directement à Windows, et le seul où le code « unsafe » est permis (chaque
//! bloc a son commentaire SAFETY). Il remplit les traits de mindcare-vie ([`systeme::SystemeWindows`] pour
//! `Systeme`, [`fenetre::FenetreTauri`] pour `Fenetre`) avec les vrais appels :
//!
//! - [`synchro`] : le verrou `Local\Mindcare-Instance` (`CreateMutexW`, `WaitForSingleObject`) et le signal
//!   d'affichage `Local\Mindcare-Afficher` (`CreateEventW`, `OpenEventW`, `SetEvent`) ;
//! - [`fenetre`] : accrochée dans Mindset (`GetWindowLongPtrW`, `GetParent`), visible (`IsWindowVisible`, `IsIconic`),
//!   montrée sans prendre la main (`ShowWindow(SW_SHOWNA)`), taille minimale gardée quand Mindset change la
//!   taille (`SetWindowSubclass`, `WM_WINDOWPOSCHANGING`) ;
//! - [`registre`] : `InstallLocation` de Mindset dans HKLM (lecture seule) ;
//! - [`webview2`] : les réglages de WebView2 que Tauri ne transmet pas, et la veille de la page (`SetIsVisible`) ;
//! - [`identite`] : `SetCurrentProcessExplicitAppUserModelID("JordanJacquot.Mindcare")` (la barre des tâches) ;
//! - [`heure`] : l'heure locale du journal (`GetLocalTime`) ;
//! - [`message`] : la boîte « Mindcare s'utilise avec Mindset » (`MessageBoxW`) ;
//! - `capture` (construction d'essai seulement) : l'image de la fenêtre (`PrintWindow`).
//!
//! Rien ici n'écrit dans le registre, ne touche aux fenêtres d'un autre programme ni ne lance quoi que ce soit sans
//! qu'on le lui demande.

#![allow(unsafe_code)]

#[cfg(feature = "essai")]
pub mod capture;
pub mod fenetre;
pub mod heure;
pub mod identite;
pub mod message;
pub mod registre;
pub mod synchro;
pub mod systeme;
pub mod webview2;

use windows::Win32::Foundation::{HANDLE, HWND};

/// Un handle de Windows gardé comme un nombre : il passe d'un fil à l'autre, et se rend en `HANDLE` au moment de
/// l'appel.
pub(crate) fn handle(h: usize) -> HANDLE {
    HANDLE(h as *mut core::ffi::c_void)
}

/// Une fenêtre gardée comme un nombre (le `HWND` de Tauri), rendue en `HWND` au moment de l'appel.
pub(crate) fn hwnd(h: usize) -> HWND {
    HWND(h as *mut core::ffi::c_void)
}
