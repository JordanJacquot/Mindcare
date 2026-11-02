//! La vie de Mindcare avec Mindset, sans Tauri et sans Windows (étape 4.3 du plan « Mindcare en Rust ») : le portage
//! de Mindcare.ps1, modules\mindset.ps1, modules\systeme.ps1 et modules\evenements.ps1 (Mindcare PowerShell 0.15.3),
//! pour ce qui touche au contrat entre Mindset et une appli (D:\Projets\Mindset\docs\contrat-appli.md).
//!
//! Tout ce qui parle à Windows ou au disque est derrière des traits ([`Systeme`], [`Disque`], [`Fenetre`]) : le
//! programme (mindcare-appli) les remplit avec Windows et Tauri, les tests avec des faux. Le temps est donné par
//! l'appelant ([`Vie::avancer`]) : un scénario de plusieurs minutes se rejoue en un instant.
//!
//! - [`arguments`] : `-Demarrage` (aussi `/Demarrage`, sans tenir compte des majuscules) ;
//! - [`mindset`] : le programme de Mindset (HKLM, jamais suite.json) et l'ouverture d'une page dans Mindset ;
//! - [`demarrage`] : copie bloquée, copie gérée lancée à la main, verrou, signal, dossier de données ;
//! - [`vie`] : les surveillances toutes les 500 ms (accrochage, veille de la page, signal d'affichage,
//!   quitter.demande, suite.json, etat.json), la croix, la fin et la relance ;
//! - [`disque_reel`] : le [`Disque`] du programme (std::fs, suite.json par dossier.rs, etat.json par carte.rs).
//!
//! Chaque règle cite la ligne PowerShell qu'elle reprend (fichier:ligne, dans D:\Projets\Mindcare\Mindcare\fichiers).

#![forbid(unsafe_code)]

pub mod arguments;
pub mod demarrage;
pub mod disque_reel;
pub mod mindset;
pub mod monde;
pub mod vie;

pub use arguments::demarrage_demande;
pub use demarrage::{Depart, Environnement, Lancement, demarrer, theme_choisi};
pub use disque_reel::DisqueReel;
pub use mindset::{mindset_exe, ouvrir_page_mindset};
pub use monde::{AttenteVerrou, Date, Disque, Fenetre, Monde, Niveau, Noms, Systeme, Taille};
pub use vie::{Fin, Reglages, TOUR, Vie, echelle};
