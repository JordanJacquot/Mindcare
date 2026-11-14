//! Repris de D:\Projets\Nevermind\rust\nevermind-aide\src\systeme.rs (plan de l'Aide de Mindcare, 14/11/2026),
//! réduit à ce dont l'Aide de Mindcare se sert.
//!
//! Tout ce que l'Aide demande à Windows, derrière un trait : l'exe livré n'a qu'une seule façon de faire, les vrais
//! appels de Windows (module `windows`) ; seuls les tests de `cargo test` le remplacent par un faux PC.

use mindcare_aide_regles::cartes::Carte;
use std::time::Duration;

/// Qui fait tourner l'Aide
#[derive(Clone, Debug)]
pub struct Identite {
    /// SID du compte (S-1-5-18 en SYSTEM)
    pub compte: String,
    /// Avec les droits administrateur
    pub admin: bool,
    /// Numéro du processus
    pub pid: u32,
}

/// Ce qu'il y a à un chemin (sans jamais suivre un lien)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entree {
    Absente,
    Dossier,
    /// Un lien (jonction, lien symbolique) : jamais suivi
    Lien,
    /// Un fichier (ou autre chose qu'un dossier)
    Fichier,
}

/// Les Wi-Fi enregistrés d'une carte Wi-Fi
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarteWifi {
    /// Son identifiant (GUID), tel que Windows le donne
    pub id: String,
    pub profils: Vec<String>,
}

/// Les appels à Windows dont l'Aide se sert (après la vérification de la demande)
pub trait Systeme: Send + Sync + 'static {
    // --- Qui, où, quand
    fn identite(&self) -> Identite;
    /// Le dossier de MindcareAide.exe (celui de cette copie de Mindcare)
    fn racine(&self) -> String;
    /// L'heure locale, « 2026-11-14T12:00:00 »
    fn heure_locale(&self) -> String;
    /// Attendre (le faux PC des tests n'attend pas)
    fn pause(&self, d: Duration);

    // --- Processus (pour savoir si Mindcare est ouvert, et qui appelle)
    /// Les numéros des processus en cours
    fn processus(&self) -> Vec<i64>;
    /// Le chemin complet du programme du processus (vide si illisible)
    fn chemin_processus(&self, pid: i64) -> String;
    /// Le compte (SID) d'un processus, vide si Windows ne le dit pas
    fn compte_processus(&self, pid: i64) -> String;

    // --- wifi.oublier
    /// Les cartes Wi-Fi et leurs Wi-Fi enregistrés ; `Ok(None)` : pas de Wi-Fi sur ce PC (pas de service, ou aucune
    /// carte)
    fn wifi(&self) -> Result<Option<Vec<CarteWifi>>, String>;
    /// Fait oublier ce Wi-Fi à cette carte
    fn wifi_oublier(&self, carte: &str, profil: &str) -> Result<(), String>;

    // --- ip.renouveler
    /// Les cartes réseau (IPv4)
    fn cartes(&self) -> Result<Vec<Carte>, String>;
    /// Rend l'adresse de la carte de ce numéro à la box
    fn ip_rendre(&self, index: u32) -> Result<(), String>;
    /// Redemande une adresse à la box pour la carte de ce numéro, en attendant au plus `limite`
    fn ip_redemander(&self, index: u32, limite: Duration) -> Result<(), String>;

    // --- energie.par-defaut
    /// `powercfg -restoredefaultschemes` (System32, sans fenêtre) : son code de sortie
    fn powercfg_par_defaut(&self) -> Result<i32, String>;
    /// Le mode d'alimentation actif (son GUID)
    fn plan_actif(&self) -> Option<String>;

    // --- maj.vider
    /// Le dossier de Windows (« C:\Windows »), donné par Windows et jamais par une variable
    fn dossier_windows(&self) -> String;
    /// Le service tourne-t-il ? (`Err` : introuvable ou illisible)
    fn service_tourne(&self, nom: &str) -> Result<bool, String>;
    /// Arrête le service et attend qu'il soit arrêté (60 s au plus ; `Err` s'il ne s'arrête pas). Jamais tué.
    fn service_arreter(&self, nom: &str) -> Result<(), String>;
    /// Démarre le service et attend qu'il tourne (60 s au plus)
    fn service_demarrer(&self, nom: &str) -> Result<(), String>;
    fn entree(&self, chemin: &str) -> Entree;
    /// Supprime ce dossier et tout ce qu'il contient, sans jamais suivre un lien (un lien est retiré, pas ce vers quoi
    /// il pointe)
    fn supprimer_dossier(&self, chemin: &str) -> Result<(), String>;
    /// Renomme un dossier (même disque)
    fn renommer(&self, de: &str, vers: &str) -> Result<(), String>;
}

/// Une connexion au canal de l'Aide
pub trait Connexion: Send {
    /// Le processus qui s'est connecté (0 si inconnu)
    fn client(&self) -> u32;
    /// Lit ce qui arrive dans `tampon`, en attendant au plus `ms` millisecondes : `Ok(None)` si rien n'est arrivé à
    /// temps, `Ok(Some(0))` si le client a coupé
    fn lire(&mut self, tampon: &mut [u8], ms: u32) -> Result<Option<usize>, String>;
    /// Écrit tout, et attend que le client l'ait lu
    fn ecrire(&mut self, octets: &[u8]) -> Result<(), String>;
}

/// Le canal en attente d'une connexion
pub trait Ecoute: Send {
    type C: Connexion + 'static;
    /// Attend une connexion au plus `ms` millisecondes : `Ok(true)` si un client s'est connecté
    fn attendre(&mut self, ms: u32) -> Result<bool, String>;
    /// Le canal connecté, qui devient la connexion de ce client
    fn connexion(self) -> Self::C;
}

/// Pourquoi un exemplaire du canal n'a pas été créé
#[derive(Debug)]
pub enum ErreurCanal {
    /// Le canal a été pris par un autre programme (propriétaire inattendu) : l'Aide s'arrête
    Repris(String),
    /// Toutes les connexions sont prises, ou autre erreur passagère
    Indisponible(String),
}

/// Le verrou « une seule Aide à la fois »
pub enum Verrouillage {
    /// Le verrou est pris, gardé tant que `garde` vit. `remarque` : à noter au journal
    Pris { garde: Box<dyn Send>, remarque: Option<String> },
    /// Une autre Aide tient déjà le verrou : l'Aide s'arrête, et dit pourquoi au journal
    Occupe(String),
}

/// Ce dont le démarrage et l'écoute de l'Aide ont besoin, en plus des demandes
pub trait Hote: Systeme {
    type E: Ecoute;
    /// Prend le verrou « une seule Aide à la fois » de ce nom (`Global\Mindcare-Aide-<sid>-<suffixe>`)
    fn verrou(&self, nom: &str) -> Verrouillage;
    /// Un exemplaire du canal `nom`, avec les droits `sddl`. `premier` : le tout premier (refusé par Windows si un autre
    /// programme a déjà un canal de ce nom). `proprietaire` : le propriétaire attendu des exemplaires suivants.
    fn canal(&self, nom: &str, sddl: &str, premier: bool, proprietaire: Option<&str>) -> Result<Self::E, ErreurCanal>;
}
