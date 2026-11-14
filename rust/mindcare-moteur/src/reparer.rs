//! « Le faire pour moi » : les tests du catalogue que Mindcare PC peut faire à la place de la personne (champ
//! « Reparer » d'un test partagé). Une liste FERMÉE d'actions, connue du code : le catalogue ne donne qu'un nom
//! d'action, jamais une commande. Mindcare PowerShell et Mindcare Mobile ne lisent pas ce champ (rien ne change pour
//! eux) ; seul Mindcare PC en Rust s'en sert, et seulement sur le PC en panne.
//!
//! Jamais pour la réparation des fichiers de Windows (test-sfc : DISM et sfc, c'est pour le réparateur), ni pour la
//! réinstallation des services de jeu (test-gaming-services : elle supprime une partie de Windows), ni pour une panne
//! de matériel.

use serde_json::Value;

use crate::catalogue::Catalogue;
use crate::ps::{Objet, champ, egal, texte};

/// Le nom du champ dans enquetes.json.
pub const CHAMP: &str = "Reparer";

/// Les tests qui ne doivent jamais avoir d'action, quoi qu'il arrive.
pub const JAMAIS: [&str; 2] = ["test-sfc", "test-gaming-services"];

/// Une action que Mindcare sait faire à la place de la personne (liste fermée, sans aucun paramètre).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reparation {
    /// Vider le carnet d'adresses d'internet (`ipconfig /flushdns`)
    Dns,
    /// Vider le cache du Microsoft Store (`wsreset.exe`)
    CacheStore,
    /// Refaire les images des icônes (`ie4uinit.exe -show`)
    CacheIcones,
    /// Installer les fichiers Visual C++ (winget, x64 puis x86)
    VisualCpp,
    /// Installer Java (winget, Temurin 21)
    Java,
    /// Remettre l'appli Paramètres à zéro (`Reset-AppxPackage`)
    Parametres,
    /// Faire oublier les Wi-Fi enregistrés (droits administrateur)
    ProfilWifi,
    /// Redemander une adresse à la box (droits administrateur)
    AdresseIp,
    /// Remettre les réglages d'alimentation d'origine (droits administrateur)
    Alimentation,
    /// Mettre de côté les mises à jour téléchargées (droits administrateur)
    MisesAJour,
}

impl Reparation {
    /// Toutes les actions, dans l'ordre.
    pub const TOUTES: [Reparation; 10] = [
        Reparation::Dns,
        Reparation::CacheStore,
        Reparation::CacheIcones,
        Reparation::VisualCpp,
        Reparation::Java,
        Reparation::Parametres,
        Reparation::ProfilWifi,
        Reparation::AdresseIp,
        Reparation::Alimentation,
        Reparation::MisesAJour,
    ];

    /// Son nom dans le catalogue.
    pub fn id(self) -> &'static str {
        match self {
            Reparation::Dns => "dns",
            Reparation::CacheStore => "cache-store",
            Reparation::CacheIcones => "cache-icones",
            Reparation::VisualCpp => "visual-cpp",
            Reparation::Java => "java",
            Reparation::Parametres => "parametres",
            Reparation::ProfilWifi => "profil-wifi",
            Reparation::AdresseIp => "adresse-ip",
            Reparation::Alimentation => "alimentation",
            Reparation::MisesAJour => "mises-a-jour",
        }
    }

    /// L'action de ce nom (exactement, en minuscules) ; aucune pour un nom inconnu.
    pub fn depuis_id(id: &str) -> Option<Reparation> {
        Reparation::TOUTES.into_iter().find(|r| r.id() == id)
    }

    /// Elle demande les droits administrateur (l'Aide administrateur, pas encore là).
    pub fn admin(self) -> bool {
        matches!(
            self,
            Reparation::ProfilWifi | Reparation::AdresseIp | Reparation::Alimentation | Reparation::MisesAJour
        )
    }

    /// L'action d'une étape prête (`Catalogue::etapes`) : son champ « Reparer », s'il nomme une action connue et que
    /// l'étape n'est pas un test interdit. Rien sinon.
    pub fn de_l_etape(etape: &Objet) -> Option<Reparation> {
        let id = texte(champ(etape, "Id"));
        if JAMAIS.iter().any(|j| egal(j, &id)) || !egal(&texte(champ(etape, "Type")), "test") {
            return None;
        }
        match champ(etape, CHAMP) {
            Some(Value::String(s)) => Reparation::depuis_id(s),
            _ => None,
        }
    }
}

/// Le champ « Reparer » du catalogue (seulement en Rust : PowerShell et le téléphone ne le lisent pas). Une liste
/// vide : tout est bon. Il n'est permis que sur un test partagé (« Tests ») qui a une commande, jamais sur test-sfc ni
/// test-gaming-services, et il doit nommer une action connue du code. Jamais sur l'étape propre d'une panne (une
/// action ne se décide qu'une fois, sur le test partagé).
pub fn verifier_reparer(cat: &Catalogue) -> Vec<String> {
    let mut pb = Vec::new();
    for (nom, t) in cat.tests() {
        let Some(o) = t.as_object() else { continue };
        let Some(v) = champ(o, CHAMP) else { continue };
        let ou = format!("Test {nom}");
        match v {
            Value::String(s) if Reparation::depuis_id(s).is_some() => {}
            autre => pb.push(format!("{ou} : action inconnue \u{ab} {} \u{bb}", texte(Some(autre)))),
        }
        if JAMAIS.iter().any(|j| egal(j, nom)) {
            pb.push(format!("{ou} : jamais d'action pour ce test"));
        }
        if !egal(&texte(champ(o, "Type")), "test") {
            pb.push(format!("{ou} : une action seulement sur un test"));
        }
        if champ(o, "Commande").is_none() {
            pb.push(format!("{ou} : une action sans commande à montrer"));
        }
    }
    for p in cat.pannes() {
        for e in crate::ps::parcours(champ(p, "Etapes")) {
            if e.as_object().is_some_and(|o| champ(o, CHAMP).is_some()) {
                let id = texte(champ(p, "Id"));
                let etape = e.as_object().map(|o| texte(champ(o, "Id"))).unwrap_or_default();
                let etape = if etape.is_empty() {
                    e.as_object().map(|o| texte(champ(o, "Ref"))).unwrap_or_default()
                } else {
                    etape
                };
                pb.push(format!("Panne {id}, étape {etape} : Reparer n'est lu que sur un test partagé"));
            }
        }
    }
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_et_actions() {
        for r in Reparation::TOUTES {
            assert_eq!(Reparation::depuis_id(r.id()), Some(r));
        }
        assert_eq!(Reparation::depuis_id("sfc"), None);
        assert_eq!(Reparation::depuis_id("DNS"), None, "le nom exact seulement");
        assert_eq!(Reparation::TOUTES.iter().filter(|r| r.admin()).count(), 4);
    }
}
