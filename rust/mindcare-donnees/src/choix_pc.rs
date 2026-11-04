//! Le PC dont on regarde les pannes : « mon PC » (celui reconnu par Mindcare), ou un autre choisi à la main (« je
//! regarde une panne pour un ami » : un type, une marque ou une carte mère, ou « je ne sais pas »). Le choix est gardé
//! dans les réglages (reglages.json, réglage « PcChoisi ») jusqu'à « Revenir à mon PC ».
//!
//! Un réglage abîmé (pas un objet, un type ou une marque inconnus, un fichier illisible) ne casse rien : c'est « mon
//! PC ». PowerShell garde ce réglage tel quel sans le lire.

use serde_json::{Map, Value, json};

use crate::donnees::Donnees;
use mindcare_moteur::pc::marque_connue;
use mindcare_moteur::ps::champ;
use mindcare_moteur::{ProfilPc, TypePc};

/// Le nom du réglage.
pub const REGLAGE: &str = "PcChoisi";

/// Le PC dont on regarde les pannes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ChoixPc {
    /// Le PC reconnu par Mindcare (celui où il est ouvert).
    #[default]
    MonPc,
    /// Un autre PC, décrit à la main. Rien de connu (`ProfilPc::inconnu()`) : « je ne sais pas », tout est montré.
    /// Une carte mère choisie, c'est sa marque (« ASUS »).
    Autre(ProfilPc),
}

impl ChoixPc {
    /// Le choix écrit dans un réglage (rien ou abîmé : « mon PC »).
    pub fn depuis_json(v: Option<&Value>) -> ChoixPc {
        let Some(Value::Object(o)) = v else { return ChoixPc::MonPc };
        let genre = match champ(o, "Type") {
            None | Some(Value::Null) => None,
            Some(Value::String(t)) => match TypePc::depuis(t) {
                Some(g) => Some(g),
                None => return ChoixPc::MonPc,
            },
            Some(_) => return ChoixPc::MonPc,
        };
        let marque = match champ(o, "Marque") {
            None | Some(Value::Null) => None,
            Some(Value::String(m)) => match marque_connue(m) {
                Some(m) => Some(m.to_string()),
                None => return ChoixPc::MonPc,
            },
            Some(_) => return ChoixPc::MonPc,
        };
        ChoixPc::Autre(ProfilPc { genre, marque })
    }

    /// Le choix à écrire dans le réglage (« mon PC » : rien).
    pub fn vers_json(&self) -> Value {
        match self {
            ChoixPc::MonPc => Value::Null,
            ChoixPc::Autre(p) => {
                let mut o = Map::new();
                o.insert("Type".into(), p.genre.map_or(Value::Null, |g| json!(g.nom())));
                o.insert("Marque".into(), p.marque.as_ref().map_or(Value::Null, |m| json!(m)));
                Value::Object(o)
            }
        }
    }

    /// Le profil qui trie les pannes : celui reconnu pour « mon PC » (pas encore reconnu : rien de connu, tout est
    /// montré), celui choisi sinon.
    pub fn profil(&self, detecte: Option<&ProfilPc>) -> ProfilPc {
        match self {
            ChoixPc::MonPc => detecte.cloned().unwrap_or_default(),
            ChoixPc::Autre(p) => p.clone(),
        }
    }

    /// Un autre PC que le mien est choisi (la pastille propose alors « Revenir à mon PC »).
    pub fn pour_un_autre(&self) -> bool {
        matches!(self, ChoixPc::Autre(_))
    }

    /// La pastille : « Ton PC : Portable HP », « Pour un autre PC : PC fixe ASUS », « Pour un autre PC : Tous les PC ».
    pub fn pastille(&self, detecte: Option<&ProfilPc>) -> String {
        let l = self.profil(detecte).libelle_court();
        match self {
            ChoixPc::MonPc => format!("Ton PC : {l}"),
            ChoixPc::Autre(_) => format!("Pour un autre PC : {l}"),
        }
    }
}

impl Donnees {
    /// Le PC choisi (gardé dans les réglages).
    pub fn choix_pc(&self) -> ChoixPc {
        ChoixPc::depuis_json(Some(&self.reglages.lire(REGLAGE, Value::Null)))
    }

    /// Change le PC choisi et le garde (« Revenir à mon PC » : `ChoixPc::MonPc`).
    pub fn choisir_pc(&mut self, choix: &ChoixPc) {
        self.reglages.changer(&mut self.fichiers, REGLAGE, choix.vers_json());
        self.fichiers.journal.ecrire(&match choix {
            ChoixPc::MonPc => "PC choisi : mon PC".to_string(),
            ChoixPc::Autre(p) => format!("PC choisi : un autre PC ({})", p.libelle_court()),
        });
    }
}
