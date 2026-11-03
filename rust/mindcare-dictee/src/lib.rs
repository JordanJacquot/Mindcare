//! La dictée de Mindcare (étape 6.3 de « Mindcare en Rust ») : le portage de modules\dictee.ps1 et de la classe
//! `MindcareMicro` (natif.cs), qui font foi. Sans écran : les pages lisent seulement l'[`Etat`].
//!
//! On appuie sur Parler, on parle, et la phrase s'écrit dans la recherche : on peut la corriger avant d'appuyer sur
//! Entrée. whisper (whisper.cpp, modèle « small », en français) écrit ce qui a été dit, sur ce PC : rien ne part sur
//! Internet. Il n'est lancé que le temps d'écrire une phrase (quelques secondes), sans fenêtre, et s'arrête avec
//! Mindcare. Le son est effacé dès que la phrase est écrite (et aussi en cas d'erreur ou d'annulation) ; ceux laissés
//! par une fermeture forcée sont effacés au démarrage (constat 25n). Le micro : celui choisi dans les réglages de
//! Mindset (suite.json, par son nom, lu par suite-commun), sinon celui de Windows.
//!
//! - [`dictee`] : la dictée elle-même ([`Dictee`], [`Etat`], [`Appui`]) et ses traits ([`Materiel`], [`Micro`],
//!   [`Lanceur`]), pour que les tests donnent un faux micro et un faux whisper ;
//! - [`fond`] : la dictée sur un fil à part, prête pour les pages ([`DicteeEnFond`]) ;
//! - [`son`] : l'analyse du son (on parle, on s'est tu) et le fichier .wav ;
//! - [`whisper`] : les arguments de whisper-cli, ses fichiers, les fichiers de voix orphelins ;
//! - [`nettoyage`] : `ConvertFrom-Dictee` ;
//! - `windows` : le vrai micro (waveIn), le lancement sans fenêtre et le Job Object, les seuls appels à Windows.
//!
//! L'installation (le téléchargement de whisper et de son modèle) est dans mindcare-telechargement.

pub mod dictee;
pub mod fond;
pub mod nettoyage;
pub mod son;
pub mod whisper;
#[cfg(windows)]
pub mod windows;

pub use dictee::{Appui, Dictee, Etat, Lance, Lanceur, Materiel, Micro, Processus, Reglages, messages};
pub use fond::DicteeEnFond;
pub use son::Mesures;

/// La dictée de Mindcare avec le vrai micro et le vrai lancement de whisper (sans fenêtre, relié à Mindcare).
/// `donnees` : le dossier de données de Mindcare ; `suite_json` : `%LOCALAPPDATA%\Mindset\suite.json`.
#[cfg(windows)]
pub fn dictee_de_mindcare(donnees: &std::path::Path, suite_json: Option<std::path::PathBuf>) -> DicteeEnFond {
    DicteeEnFond::lancer(Dictee::nouvelle(
        Reglages::de_mindcare(donnees, suite_json),
        Box::new(windows::MaterielWindows),
        Box::new(windows::LanceurWindows),
    ))
}
