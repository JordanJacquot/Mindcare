//! « Ouvrir Nevermind » dans la réponse : pour les pannes de jeu qui ont le champ « Appli » (6 pannes), une ligne
//! « Nevermind règle ça pour toi » avec son bouton, au lieu de refaire ses réglages dans Mindcare. Seulement sur le PC
//! (Mindcare Mobile ne lit pas ce champ) et seulement quand Mindcare est affiché dans Mindset : c'est Mindset qui ouvre
//! Nevermind (ou montre sa page pour l'installer), jamais Mindcare.
//!
//! Le clic rend l'effet [`Effet::OuvrirAppli`] (la demande écrite et attendue sur un fil à part, par effets.rs) ; un
//! clic pendant l'attente est ignoré. La fin revient dans [`Etat::ouverture_finie`].

use serde_json::{Value, json};

use mindcare_moteur::{Appli, Catalogue};

use super::actions::ids;
use super::{Effet, Etat, Vue};
use crate::ouvrir::Fin;

/// Les textes de la ligne (maquette validée par Jordan, écran « 6. FPS »).
pub mod textes {
    pub const TITRE: &str = "Nevermind règle ça pour toi";
    pub const NOTE: &str =
        "Mode de jeu, alimentation, pilote de la carte graphique : il vérifie tout et corrige en un clic.";
    pub const OUVRIR: &str = "Ouvrir Nevermind";
    pub const INSTALLER: &str = "L'installer avec Mindset";
    /// La bulle quand Mindset n'a pas lu la demande (trop ancien)
    pub const DEPUIS_L_ACCUEIL: &str = "Ouvre Nevermind depuis l'accueil de Mindset";
}
use textes as x;

impl Etat {
    /// L'appli que la réponse montrée propose d'ouvrir : sa panne a le champ « Appli », Mindcare est affiché dans
    /// Mindset, et l'on sait déjà si elle est installée. Rien sinon (pas de ligne).
    pub(crate) fn appli_proposee(&self) -> Option<(Appli, bool)> {
        if !self.dans_mindset {
            return None;
        }
        let installee = self.nevermind_installe?;
        let a = self.en_cours.panne.as_ref().and_then(Appli::de_la_panne)?;
        Some((a, installee))
    }

    /// Le clic sur « Ouvrir Nevermind » (ou « L'installer avec Mindset ») : la même demande dans les deux cas.
    pub(super) fn ouvrir_appli(&mut self, cat: &Catalogue) -> Vec<Effet> {
        if !matches!(self.vue(cat), Some(Vue::Reponse(_))) {
            return vec![Effet::rafraichir()];
        }
        let Some((a, _)) = self.appli_proposee() else { return vec![Effet::rafraichir()] };
        // (une demande à la fois : un clic pendant l'attente est ignoré)
        if self.ouverture.is_some() {
            return vec![Effet::rafraichir()];
        }
        self.ouverture = Some(a);
        vec![Effet::OuvrirAppli(a)]
    }

    /// La demande est finie : lue par Mindset (il ouvre l'appli, rien à dire), reprise au bout de 3 s ou pas écrite
    /// (la bulle dit quoi faire). La ligne du journal vient d'effets.rs.
    pub fn ouverture_finie(&mut self, fin: &Fin) -> Vec<Effet> {
        self.ouverture = None;
        match fin {
            Fin::Lue => Vec::new(),
            Fin::Reprise | Fin::Ratee(_) => vec![Effet::message(x::DEPUIS_L_ACCUEIL, false)],
        }
    }

    /// Mindcare est affiché dans Mindset, ou ne l'est plus : rend vrai si la page doit être redessinée.
    pub fn noter_dans_mindset(&mut self, oui: bool) -> bool {
        let avant = self.appli_proposee();
        self.dans_mindset = oui;
        avant != self.appli_proposee()
    }

    /// On sait (de nouveau) si Nevermind est installé : rend vrai si la page doit être redessinée.
    pub fn noter_nevermind(&mut self, installe: bool) -> bool {
        let avant = self.appli_proposee();
        self.nevermind_installe = Some(installe);
        avant != self.appli_proposee()
    }
}

/// La ligne de la réponse, avec son bouton (rien si la réponse ne propose aucune appli).
pub(super) fn ligne(etat: &Etat) -> Option<Value> {
    let (a, installee) = etat.appli_proposee()?;
    let bouton = match a {
        Appli::Nevermind if installee => x::OUVRIR,
        Appli::Nevermind => x::INSTALLER,
    };
    Some(json!({ "type": "ligne", "icone": "jauge", "couleur": "P", "texte": x::TITRE, "note": x::NOTE,
                 "bouton": { "id": ids::OUVRIR, "texte": bouton } }))
}
