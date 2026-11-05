//! Le moteur des enquêtes de Mindcare, sans écran et sans Windows : le portage exact de modules\enquete.ps1 (et du
//! garde-fou de l'assistant, Select-PreRempli de modules\ia.ps1).
//!
//! La référence reste le moteur PowerShell : les tests (tests\) rejouent des milliers d'enquêtes, de recherches et de
//! cas exportés depuis lui par outils\exporter-juges.ps1, et doivent trouver exactement les mêmes réponses. Les seules
//! différences voulues sont écrites dans tests\juges\divergences.json et tests\divergences.rs, dont l'ordre des
//! résultats de la recherche (les pannes trouvées restent celles de PowerShell ; leur ordre ne le suit plus depuis le
//! 05/11/2026, voir [`recherche`]).
//!
//! - [`catalogue`] : lire enquetes.json, les pannes, les suspects, les étapes prêtes (Get-Etapes) ;
//! - [`enquete`] : rejouer une enquête (Get-Enquete) ;
//! - [`verdict`] : la phrase de la réponse finale (Get-VerdictTitre) ;
//! - [`recherche`] : la recherche par mots (les pannes de Find-Pannes, mieux classées) ;
//! - [`garde_fou`] : le garde-fou de l'assistant (Select-PreRempli) ;
//! - [`validation`] : ce qui ne va pas dans le catalogue (Test-Enquetes) ;
//! - [`pc`] : les pannes qui concernent ce PC (le champ « PourPc », le profil du PC) ;
//! - [`identifiables`] : chaque suspect peut-il être la réponse ?
//! - [`ps`] : les règles de PowerShell que tout cela reproduit (majuscules, vrai ou faux, listes).

pub mod catalogue;
pub mod enquete;
pub mod garde_fou;
pub mod identifiables;
pub mod pc;
pub mod ps;
pub mod recherche;
pub mod validation;
pub mod verdict;

pub use catalogue::{Catalogue, Groupe, GroupesFamille};
pub use enquete::{
    Auto, Ecarte, Enquete, Fait, HORS_SUJET, INDICES_CONNUS, Indice, Indices, TypeVerdict, Verdict, enquete,
};
pub use garde_fou::pre_rempli;
pub use pc::{MARQUES, ProfilPc, Tri, TypePc, concerne};
pub use ps::Objet;
pub use recherche::{chercher, chercher_comme_powershell, chercher_pour, simple};
pub use validation::{verifier, verifier_groupes};
pub use verdict::{titre_affiche, titre_avec_detail, titre_verdict};
