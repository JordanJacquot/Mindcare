//! MindcareAide.exe : la seule partie de Mindcare qui tourne avec les droits administrateur (plan de l'Aide de
//! Mindcare, 14/11/2026). Cadre copié de NevermindAide.exe (D:\Projets\Nevermind\rust\nevermind-aide\src\main.rs).
//!
//! Lancé par la tâche planifiée « \Mindset\Mindcare (aide) » que l'installateur de Mindset crée à l'installation de
//! Mindcare (champs « aide » et « aide_exe » de apps.json), en SYSTEM, avec `-Sid <utilisateur>` (inscrit par
//! l'administrateur qui a créé la tâche, jamais déduit d'autre chose). Jamais par la demande de Windows. Aucune
//! fenêtre, jamais (en SYSTEM, personne ne la verrait). Aucun fichier écrit, et rien lu chez l'utilisateur.
//!
//! Elle ne sait faire que 4 réparations, sans aucun paramètre (mindcare-aide-regles, demandes.rs), pour Mindset.exe
//! du dossier de Mindcare seulement, une à la fois, et s'arrête toute seule 20 s après la fermeture de Mindcare.

#![windows_subsystem = "windows"]
#![deny(unsafe_op_in_unsafe_fn)]

mod actions;
#[cfg(test)]
mod essais;
mod serveur;
mod systeme;
mod windows;

use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    let sid = mindcare_aide_regles::sid::lire_arguments(&args);
    // (avant tout : DLL de System32 seulement, dossier courant System32)
    windows::preparer_processus();
    let pc = Arc::new(windows::Windows::new());
    serveur::lancer(pc, sid.as_deref(), serveur::Delais::NORMAUX);
    // Aucun fil laissé par une demande ne garde le processus ouvert
    std::process::exit(0);
}
