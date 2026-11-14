//! Les règles de l'Aide de Mindcare (MindcareAide.exe, la tâche SYSTEM `\Mindset\Mindcare (aide)`) : seulement des
//! décisions, sans rien toucher sur le PC. MindcareAide.exe (crate mindcare-aide) et Mindcare (mindcare-appli) s'en
//! servent tous les deux, pour que les deux côtés calculent les mêmes noms et suivent les mêmes règles.
//!
//! Le cadre est copié de l'Aide de Nevermind (D:\Projets\Nevermind\rust\aide-regles), avec le préfixe « Mindcare » :
//! - [`sid`] : pour qui l'Aide travaille (`-Sid` inscrit dans la tâche) ;
//! - [`canal`] : le nom du canal et du verrou, propres au compte et au dossier ;
//! - [`client`] : qui peut parler à l'Aide (Mindset.exe du dossier de Mindcare, et rien d'autre) ;
//! - [`serveur`] : Mindcare reconnaît l'Aide (canal créé par le système) ;
//! - [`tache`] : l'état de la tâche, lu dans son XML ;
//! - [`chemin`], [`sha256`], [`texte`], `xml` : les petits outils de ces règles.
//!
//! Propre à Mindcare :
//! - [`demandes`] : la liste FERMÉE des actions, sans aucun paramètre ;
//! - [`cartes`] : les cartes réseau dont l'adresse est redemandée (`ip.renouveler`) ;
//! - [`maj`] : le nom du dossier des mises à jour mis de côté (`maj.vider`).

#![forbid(unsafe_code)]

pub mod canal;
pub mod cartes;
pub mod chemin;
pub mod client;
pub mod demandes;
pub mod maj;
pub mod serveur;
pub mod sha256;
pub mod sid;
pub mod tache;
pub mod texte;
mod xml;
