//! Les données de Mindcare, sans écran et sans appel à Windows (seulement des fichiers) : le portage exact de
//! modules\donnees.ps1 (dossier, journal, fichiers JSON, réglages), modules\historique.ps1 (enquêtes gardées, rapport,
//! carte de Mindset), de la lecture de suite.json (modules\mindset.ps1) et de la partie sans écran de
//! modules\contenu.ps1 (enquête en cours, reprise par Id, textes du rapport).
//!
//! La référence reste Mindcare PowerShell : les tests (tests\) rejouent les cas exportés depuis ses vrais modules par
//! outils\exporter-juges-donnees.ps1, et doivent trouver exactement les mêmes réponses. La seule différence voulue
//! (constat 23 : un fichier vide n'est jamais rangé en copie de secours) est écrite dans tests\juges\divergences.json.
//!
//! Ce que Windows doit fournir est donné par l'appelant (l'autre crate) : l'heure locale ([`Horloge`]), les
//! variables d'environnement, le modèle du PC ([`rapport::LecturePc`]), les barrettes de mémoire, les indices du
//! journal de Windows, le nombre au hasard d'un nouvel Id.
//!
//! - [`moment`] : les dates (écrites, relues, montrées en français) ;
//! - [`journal`] : le journal (Write-Log) ;
//! - [`fichiers`] : les fichiers JSON (Save-JsonFile, Read-JsonFile, fichier pris, Get-FichierRelu), avec `json_sur` ;
//! - [`reglages`] : Get-Setting, Set-Setting ;
//! - [`historique`] : diagnostics.json (Read-Historique, Save-Diagnostic, Get-Paires, ConvertTo-Indices,
//!   Get-DiagnosticInfo) ;
//! - [`carte`] : la carte de Mindset (Get-Court, Get-PhraseCarte, Get-EtatCarte, Write-EtatMindset) ;
//! - [`rapport`] : le rapport pour le réparateur (Get-InfosPC, Get-ResumeTexte, Save-Resume, ses textes) ;
//! - [`enquete_en_cours`] : l'enquête en cours et sa reprise (New-DiagId, Open-Diagnostic, Show-Etape,
//!   Get-PairesEnAttente...) ;
//! - [`dossier`] : le dossier de données (Get-CheminNormal, Read-Suite, Test-GereeParMindset, Get-DonneesMindset) ;
//! - [`donnees`] : tout réuni ;
//! - [`valeurs`] : les règles de PowerShell en plus de celles du moteur.

pub mod carte;
pub mod donnees;
pub mod dossier;
pub mod enquete_en_cours;
pub mod fichiers;
pub mod historique;
pub mod journal;
pub mod moment;
pub mod rapport;
pub mod reglages;
pub mod valeurs;

pub use carte::{Carte, EtatMindset, court, etat_carte, phrase_carte};
pub use donnees::Donnees;
pub use enquete_en_cours::{Affichage, EnqueteEnCours, PreRempli, Reprise, nouvel_id};
pub use fichiers::{FichierRelu, Fichiers};
pub use historique::{Enregistrement, Historique, IndiceGarde, IndicesGardes, Paire, info_diagnostic, paires};
pub use journal::{Horloge, Journal, horloge_figee};
pub use moment::{Moment, format_date};
pub use rapport::{LecturePc, SurLePc, infos_pc, texte_rapport};
pub use reglages::Reglages;
