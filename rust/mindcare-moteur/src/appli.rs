//! « Ouvrir Nevermind » : les pannes dont la réponse propose d'ouvrir une autre appli de la suite (champ « Appli »
//! d'une panne). Une liste FERMÉE, connue du code : le catalogue ne donne qu'un nom d'appli, jamais un chemin ni une
//! commande. Mindcare PowerShell et Mindcare Mobile ne lisent pas ce champ (rien ne change pour eux) ; seul Mindcare
//! PC en Rust s'en sert, et seulement quand il est affiché dans Mindset.

use serde_json::Value;

use crate::catalogue::Catalogue;
use crate::ps::{Objet, champ, parcours, texte};

/// Le nom du champ dans enquetes.json.
pub const CHAMP: &str = "Appli";

/// Une appli de la suite qu'une réponse peut proposer d'ouvrir (liste fermée).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Appli {
    /// Nevermind, qui règle le PC pour les jeux
    Nevermind,
}

impl Appli {
    /// Toutes les applis, dans l'ordre.
    pub const TOUTES: [Appli; 1] = [Appli::Nevermind];

    /// Son id dans la suite (apps.json, suite.json) et dans le catalogue.
    pub fn id(self) -> &'static str {
        match self {
            Appli::Nevermind => "nevermind",
        }
    }

    /// Son nom, tel qu'il s'affiche.
    pub fn nom(self) -> &'static str {
        match self {
            Appli::Nevermind => "Nevermind",
        }
    }

    /// L'appli de cet id (exactement, en minuscules) ; aucune pour un id inconnu.
    pub fn depuis_id(id: &str) -> Option<Appli> {
        Appli::TOUTES.into_iter().find(|a| a.id() == id)
    }

    /// L'appli proposée par cette panne : son champ « Appli », s'il nomme une appli connue. Rien sinon.
    pub fn de_la_panne(panne: &Objet) -> Option<Appli> {
        match champ(panne, CHAMP) {
            Some(Value::String(s)) => Appli::depuis_id(s),
            _ => None,
        }
    }
}

/// Le champ « Appli » du catalogue (seulement en Rust : PowerShell et le téléphone ne le lisent pas). Une liste vide :
/// tout est bon. Il n'est permis que sur une panne, et il doit nommer une appli connue du code ; jamais sur une étape,
/// un test partagé, un suspect ni une famille.
pub fn verifier_appli(cat: &Catalogue) -> Vec<String> {
    let mut pb = Vec::new();
    for p in cat.pannes() {
        let id = texte(champ(p, "Id"));
        if let Some(v) = champ(p, CHAMP) {
            match v {
                Value::String(s) if Appli::depuis_id(s).is_some() => {}
                autre => pb.push(format!("Panne {id} : appli inconnue \u{ab} {} \u{bb}", texte(Some(autre)))),
            }
        }
        for e in parcours(champ(p, "Etapes")) {
            let Some(o) = e.as_object() else { continue };
            if champ(o, CHAMP).is_some() {
                let mut etape = texte(champ(o, "Id"));
                if etape.is_empty() {
                    etape = texte(champ(o, "Ref"));
                }
                pb.push(format!("Panne {id}, étape {etape} : Appli n'est lu que sur une panne"));
            }
        }
    }
    for (nom, t) in cat.tests() {
        if t.as_object().is_some_and(|o| champ(o, CHAMP).is_some()) {
            pb.push(format!("Test {nom} : Appli n'est lu que sur une panne"));
        }
    }
    for (nom, s) in cat.suspects() {
        if s.as_object().is_some_and(|o| champ(o, CHAMP).is_some()) {
            pb.push(format!("Suspect {nom} : Appli n'est lu que sur une panne"));
        }
    }
    for f in cat.familles() {
        if champ(f, CHAMP).is_some() {
            pb.push(format!("Famille {} : Appli n'est lu que sur une panne", texte(champ(f, "Id"))));
        }
    }
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_et_applis() {
        for a in Appli::TOUTES {
            assert_eq!(Appli::depuis_id(a.id()), Some(a));
        }
        assert_eq!(Appli::depuis_id("Nevermind"), None, "le nom exact seulement");
        assert_eq!(Appli::depuis_id("mindset"), None);
        assert_eq!(Appli::Nevermind.nom(), "Nevermind");
    }
}
