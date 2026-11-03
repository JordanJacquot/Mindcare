//! Le moteur de l'assistant de Mindcare (étape 6.2 de « Mindcare en Rust ») : le portage de modules\ia.ps1 (le
//! lancement de llama-server, la question, la lecture de la réponse, le garde-fou) et de sa partie dans
//! modules\contenu.ps1 (la décision : enquête, « Pour être sûr », ou recherche par mots). Le PowerShell fait foi.
//!
//! Sans écran : les pages lisent l'[`Etat`] de l'[`Assistant`], posent la phrase ([`Assistant::poser`]), attendent
//! la [`Tache`], puis suivent la [`Decision`] ([`decider`]).
//!
//! - [`serveur`] : llama-server lancé seulement quand on s'en sert, sur 127.0.0.1, port gardé jusqu'au lancement,
//!   clé neuve par variable d'environnement, vérifié (c'est bien LUI qui répond), arrêté après 5 minutes sans
//!   question, à la fermeture et avec Mindcare même tuée ; la vraie cause d'un échec au journal ;
//! - [`requete`] : la question (le système, la grammaire du catalogue) et la lecture de la réponse (la confiance) ;
//! - [`decision`] : sûr, il hésite, ou rien ; le garde-fou (Select-PreRempli, dans mindcare-moteur) ;
//! - [`http`] : un petit client HTTP vers 127.0.0.1 seulement ;
//! - `windows` : le lancement sans fenêtre, le Job Object et la clé aléatoire, les seuls appels à Windows.
//!
//! Aucune bibliothèque nouvelle.

pub mod decision;
pub mod http;
pub mod requete;
pub mod serveur;
#[cfg(windows)]
pub mod windows;

pub use decision::{BULLE_ECHEC, Choix, Decision, MESSAGE_ECHEC, SEUIL, choisir, decider};
pub use requete::{Idee, Reponse, SYSTEME};
pub use serveur::{
    Assistant, Delais, DernieresLignes, Etat, Horloge, HorlogeSimulee, HorlogeVraie, Lance, Lanceur, Ordre, Processus,
    Question, Reglages, Tache,
};

/// L'assistant tel que Mindcare le lance : le vrai llama-server (sans fenêtre, relié à Mindcare), la vraie heure et
/// les délais du PowerShell
#[cfg(windows)]
pub fn assistant(donnees: &std::path::Path, journal: std::sync::Arc<dyn Fn(&str) + Send + Sync>) -> Assistant {
    use std::sync::Arc;
    Assistant::nouveau(
        Reglages { paquet: mindcare_telechargement::Paquet::assistant(donnees), delais: Delais::default() },
        Arc::new(windows::LanceurWindows),
        Arc::new(HorlogeVraie),
        journal,
    )
}
