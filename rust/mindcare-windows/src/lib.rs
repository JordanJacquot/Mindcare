//! Ce que Mindcare lit dans Windows, avant et pendant une enquête : le portage de modules\journaux.ps1 (et de la
//! lecture des valeurs brutes de Get-InfosPC, modules\historique.ps1).
//!
//! La bibliothèque est en deux parts :
//! - les règles, sans aucun appel à Windows : elles reçoivent des données brutes par le trait [`SourceWindows`] et en
//!   font des indices. Les tests (tests\) leur donnent un faux Windows rempli avec les juges exportés depuis le vrai
//!   journaux.ps1 par outils\exporter-juges-windows.ps1, et doivent trouver exactement les mêmes indices ;
//! - le module `windows`, le seul qui parle à Windows (et le seul où le code « unsafe » est permis), qui remplit le
//!   trait avec les vraies lectures. Tout se lit sans droits administrateur et sans rien modifier.
//!
//! - [`moment`] : les dates (l'heure locale, en ticks comme `[DateTime]`), et « le 1er novembre » ;
//! - [`source`] : ce que la lecture demande à Windows ([`SourceWindows`], [`Horloge`]) ;
//! - [`indices`] : les règles (Get-IndicesWindows, Select-ErreursDisque, Get-Barrettes) ;
//! - [`lecture`] : la lecture en arrière-plan (Start-LectureJournaux... Stop-LectureJournaux) ;
//! - [`motifs`] : les expressions régulières et les conversions de PowerShell reproduites.
//!
//! Les différences voulues avec PowerShell (constats 20 et 21) sont écrites dans tests\juges\windows-divergences.json.

pub mod indices;
pub mod lecture;
pub mod moment;
pub mod motifs;
pub mod source;
#[cfg(windows)]
pub mod windows;

pub use indices::{IndiceWindows, IndicesWindows, JOURS, barrettes, erreurs_disque, indices_windows, vers_moteur};
pub use lecture::{LectureJournaux, RELIRE_APRES_MINUTES, a_relire};
pub use moment::Moment;
pub use source::{
    Barrette, Batterie, Disque, DisquePhysique, Evenement, Filtre, Horloge, InfosPc, Mesure, Partition, Place,
    SourceWindows,
};
