//! Les téléchargements de l'assistant et de la dictée de Mindcare (étape 6.1 de « Mindcare en Rust ») : le portage de
//! `Start-Telechargement` et des fonctions d'installation de modules\ia.ps1 et modules\dictee.ps1, qui font foi.
//!
//! Sans fenêtre et sans écran : les pages de l'assistant et de la dictée (étapes 6.2 et 6.3) lisent seulement
//! l'[`Etat`] d'une [`Installation`]. Une crate à part, et pas un module de mindcare-appli : ses tests se
//! construisent sans Tauri (vite), et le code « unsafe » (WinHTTP) reste dans un seul petit module.
//!
//! - [`fichiers`] : les fichiers (mêmes adresses, tailles, empreintes et motifs que le PowerShell) et ce qui est
//!   installé (moteur complet, installation coupée, ancien modèle gardé en attendant, anciens effacés après) ;
//! - [`telechargement`] : le téléchargement sur un fil à part (reprise, essais avec pause, empreinte, place libre,
//!   annulation) ;
//! - [`zip`] : l'extraction des seuls fichiers voulus d'un zip, sans chemin qui sorte du dossier ;
//! - [`installation`] : ce que voient les pages (rien, en cours, prêt, à réparer, à mettre à jour, erreur) ;
//! - `windows` : WinHTTP (le réseau) et la place libre, les seuls appels à Windows.
//!
//! Aucune bibliothèque nouvelle : sha2 et flate2 sont déjà dans Cargo.lock (Tauri s'en sert), WinHTTP vient de la
//! crate windows de la suite.

pub mod fichiers;
pub mod installation;
pub mod telechargement;
#[cfg(windows)]
pub mod windows;
pub mod zip;

pub use fichiers::{Extraire, Fichier, MO, Motif, Paquet};
pub use installation::{Etat, Fin, Installation};
pub use telechargement::{Avancement, Disque, PAUSE, Reponse, Telechargement, Transport};
