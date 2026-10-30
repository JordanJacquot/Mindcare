//! Le moteur des enquêtes : le portage exact de Get-Enquete (modules\enquete.ps1).
//!
//! Une enquête part des suspects d'une panne et les élimine, étape par étape : des questions, des vérifications
//! automatiques (ce que Windows a noté, si Mindcare est ouvert sur le PC en panne) et des tests à faire. Elle finit quand
//! un suspect est confirmé, ou quand il n'y a plus d'étape : un seul suspect restant est la réponse, plusieurs vont au
//! réparateur avec le rapport. Un parcours, ce sont les réponses données dans l'ordre : le numéro de la réponse (0, 1...),
//! ou -1 pour « Je ne sais pas ». Les mêmes réponses donnent toujours la même enquête (pas d'IA, pas de hasard).

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::catalogue::Catalogue;
use crate::ps::{Objet, champ, champ_v, cle, contient, egal, parcours, tableau, texte, vrai};

/// Les indices de Windows que le catalogue peut demander (`$IndicesConnus`).
pub const INDICES_CONNUS: [&str; 16] = [
    "coupures",
    "ecransbleus",
    "bsod-memoire",
    "bsod-pilote",
    "bsod-disque",
    "bsod-materiel",
    "materiel",
    "testmemoire-erreurs",
    "testmemoire-ok",
    "disque",
    "pilotegraphique",
    "majredemarrage",
    "freinage",
    "placelibre",
    "allume",
    "usurebatterie",
];

/// Ce que Windows a noté pour un indice (journaux.ps1) : vu ou pas, le texte de ce qui a été vu, celui de la bonne
/// nouvelle, et « partiel » (le journal ne remonte pas assez loin : s'il n'a rien vu, ça ne prouve rien).
///
/// Une mesure qui n'a pas pu être faite ne doit PAS être donnée au moteur (constat 20 de l'audit) : un indice absent
/// n'écarte aucune piste, alors qu'un indice « pas vu » écarte. Voir tests\juges\divergences.json.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Indice {
    pub vu: bool,
    pub texte: String,
    pub texte_ok: String,
    pub partiel: bool,
}

/// Les indices de Windows, par nom (la table de journaux.ps1). Les noms se lisent sans tenir compte des majuscules.
pub type Indices = HashMap<String, Indice>;

/// Un suspect écarté, et pourquoi (la preuve de la réponse, ou « Ne concerne pas ce type de PC »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ecarte {
    pub id: String,
    pub raison: String,
}

/// Une réponse donnée : l'étape, la réponse (son texte, ou « Je ne sais pas »), et les textes de l'étape.
#[derive(Debug, Clone, PartialEq)]
pub struct Fait {
    pub etape: Objet,
    pub reponse: String,
    pub court: String,
    pub texte: String,
}

/// Ce que Windows a noté et qui a servi (une ligne du rapport).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Auto {
    pub texte: String,
    pub vu: bool,
}

/// Le genre de la réponse finale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeVerdict {
    /// Un suspect confirmé par une réponse.
    Trouve,
    /// Un seul suspect restant, tous les autres écartés.
    Elimination,
    /// Plusieurs suspects restants : un réparateur départagera.
    Reparateur,
    /// Aucun suspect restant.
    Rien,
}

impl TypeVerdict {
    /// Le nom du fichier et de PowerShell (trouve, elimination, reparateur, rien).
    pub fn nom(self) -> &'static str {
        match self {
            TypeVerdict::Trouve => "trouve",
            TypeVerdict::Elimination => "elimination",
            TypeVerdict::Reparateur => "reparateur",
            TypeVerdict::Rien => "rien",
        }
    }
}

/// La réponse finale d'une enquête.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub genre: TypeVerdict,
    /// Le suspect trouvé (trouve, elimination).
    pub suspect: Option<String>,
    /// Le détail de la réponse qui a confirmé (trouve).
    pub detail: String,
    pub restants: Vec<String>,
}

/// Une enquête rejouée.
#[derive(Debug, Clone, PartialEq)]
pub struct Enquete {
    /// L'étape où on en est (aucune une fois l'enquête finie).
    pub etape: Option<Objet>,
    /// Les réponses qui ont servi (celles en trop sont coupées).
    pub choix: Vec<i64>,
    pub restants: Vec<String>,
    pub ecartes: Vec<Ecarte>,
    /// Les mots retenus des réponses (« portable », « cepc »...).
    pub notes: Vec<String>,
    pub preuves: Vec<String>,
    pub faits: Vec<Fait>,
    pub auto: Vec<Auto>,
    pub verdict: Option<Verdict>,
    /// Les étapes qui peuvent encore venir après celle-ci (au plus) : pour « Étape 2 sur 5 ».
    pub a_venir: usize,
}

/// L'état d'une enquête en cours de route.
struct Etat {
    restants: Vec<String>,
    ecartes: Vec<Ecarte>,
    notes: Vec<String>,
    preuves: Vec<String>,
    confirme: Option<String>,
    detail: String,
}

impl Etat {
    /// Retire la première fois ce suspect (`List.Remove` : en tenant compte des majuscules).
    fn retire(&mut self, id: &str) -> bool {
        match self.restants.iter().position(|r| r == id) {
            Some(i) => {
                self.restants.remove(i);
                true
            }
            None => false,
        }
    }

    /// Une réponse (ou une vérification) : ses mots retenus, les suspects qu'elle écarte, garde ou confirme, et sa
    /// preuve. Confirme ne compte que si le suspect est encore là.
    fn applique(&mut self, effet: Option<&Value>, preuve: &str) {
        if !vrai(effet) {
            return;
        }
        let mut agit = false;
        let note = champ_v(effet, "Note");
        if vrai(note) {
            self.notes.push(texte(note));
        }
        for valeur in tableau(champ_v(effet, "Ecarte")) {
            let id = texte(Some(valeur));
            if vrai(Some(valeur)) && self.retire(&id) {
                self.ecartes.push(Ecarte { id, raison: preuve.to_string() });
                agit = true;
            }
        }
        let garde = champ_v(effet, "Garde");
        if vrai(garde) {
            let garde = tableau(garde);
            for id in self.restants.clone() {
                if !garde.iter().any(|g| !g.is_null() && egal(&texte(Some(g)), &id)) {
                    self.retire(&id);
                    self.ecartes.push(Ecarte { id, raison: preuve.to_string() });
                    agit = true;
                }
            }
        }
        let confirme = champ_v(effet, "Confirme");
        if vrai(confirme) {
            let c = texte(confirme);
            if self.restants.contains(&c) {
                self.confirme = Some(c);
                self.detail = texte(champ_v(effet, "Detail"));
                agit = true;
            }
        }
        if agit && !preuve.is_empty() && !self.preuves.iter().any(|p| egal(p, preuve)) {
            self.preuves.push(preuve.to_string());
        }
    }

    /// L'étape peut-elle venir maintenant ? (Pour : un de ses suspects reste ; SiNote : un de ces mots est retenu ;
    /// SaufNote : aucun de ces mots n'est retenu)
    fn possible(&self, x: &Objet) -> bool {
        let pour = champ(x, "Pour");
        if vrai(pour) && !parcours(pour).iter().any(|s| self.restants.contains(&texte(Some(s)))) {
            return false;
        }
        let si = champ(x, "SiNote");
        if vrai(si) && !parcours(si).iter().any(|n| contient(&self.notes, n)) {
            return false;
        }
        let sauf = champ(x, "SaufNote");
        if vrai(sauf) && parcours(sauf).iter().any(|n| contient(&self.notes, n)) {
            return false;
        }
        true
    }
}

/// Un indice par son nom, sans tenir compte des majuscules (une table `@{}` de PowerShell).
fn indice<'a>(indices: &'a Indices, nom: &str) -> Option<&'a Indice> {
    indices.get(nom).or_else(|| indices.iter().find(|(k, _)| egal(k, nom)).map(|(_, v)| v))
}

fn est_auto(x: &Objet) -> bool {
    egal(&texte(champ(x, "Type")), "auto")
}

/// Rejoue une enquête (Get-Enquete) : la panne, les réponses données (-1, ou un numéro hors de la liste : « Je ne sais
/// pas »), et ce que Windows a noté (seulement utilisé si une réponse a retenu « cepc » : Mindcare est ouvert sur le PC
/// en panne).
pub fn enquete(cat: &Catalogue, panne: &Objet, choix: &[i64], indices: Option<&Indices>) -> Enquete {
    let etapes = cat.etapes(panne);
    let mut st = Etat {
        restants: parcours(champ(panne, "Suspects")).into_iter().map(|s| texte(Some(s))).collect(),
        ecartes: Vec::new(),
        notes: Vec::new(),
        preuves: Vec::new(),
        confirme: None,
        detail: String::new(),
    };
    let mut faits = Vec::new();
    let mut auto: Vec<Auto> = Vec::new();
    let mut faites: HashSet<String> = HashSet::new();
    let mut n = 0usize;
    let mut courante: Option<Objet> = None;

    while st.confirme.is_none() {
        // Les suspects qui ne concernent pas ce PC (un portable n'a pas de bloc d'alimentation...)
        for id in st.restants.clone() {
            let sauf = cat.suspect(&id).and_then(|s| champ(s, "Sauf"));
            if parcours(sauf).iter().any(|x| vrai(Some(x)) && contient(&st.notes, x)) {
                st.retire(&id);
                st.ecartes.push(Ecarte { id, raison: "Ne concerne pas ce type de PC".to_string() });
            }
        }
        let Some(et) = etapes.iter().find(|x| !faites.contains(&cle(&texte(champ(x, "Id")))) && st.possible(x)) else {
            break;
        };
        faites.insert(cle(&texte(champ(et, "Id"))));
        if est_auto(et) {
            // Ce que Windows a noté : seulement si Mindcare est ouvert sur le PC en panne
            let Some(indices) = indices else { continue };
            if !st.notes.iter().any(|x| egal(x, "cepc")) {
                continue;
            }
            let Some(ind) = indice(indices, &texte(champ(et, "Indice"))) else { continue };
            let t = if ind.vu { &ind.texte } else { &ind.texte_ok };
            if !t.is_empty() && !auto.iter().any(|a| egal(&a.texte, t)) {
                auto.push(Auto { texte: t.clone(), vu: ind.vu });
            }
            // (un journal qui ne remonte pas assez loin et n'a rien vu ne prouve rien : aucune piste écartée)
            if !ind.vu && ind.partiel {
                continue;
            }
            st.applique(champ(et, if ind.vu { "SiVu" } else { "SiNonVu" }), t);
            continue;
        }
        if n >= choix.len() {
            courante = Some(et.clone());
            break;
        }
        let c = choix[n];
        n += 1;
        let reponses = tableau(champ(et, "Reponses"));
        let r = usize::try_from(c).ok().and_then(|i| reponses.get(i).copied()).filter(|r| vrai(Some(r)));
        let court = texte(champ(et, "Court"));
        faits.push(Fait {
            etape: et.clone(),
            reponse: match r {
                Some(r) => texte(champ_v(Some(r), "Texte")),
                None => "Je ne sais pas".to_string(),
            },
            court: court.clone(),
            texte: texte(champ(et, "Texte")),
        });
        if let Some(r) = r {
            let preuve = if vrai(champ_v(Some(r), "SansPreuve")) {
                String::new()
            } else if vrai(champ_v(Some(r), "Preuve")) {
                texte(champ_v(Some(r), "Preuve"))
            } else {
                format!("{court} : {}", texte(champ_v(Some(r), "Texte")))
            };
            st.applique(Some(r), &preuve);
        }
    }

    // Les étapes qui peuvent encore venir après celle-ci (au plus) : pour « Étape 2 sur 5 »
    let a_venir = if courante.is_some() {
        etapes
            .iter()
            .filter(|x| !faites.contains(&cle(&texte(champ(x, "Id")))) && !est_auto(x) && st.possible(x))
            .count()
    } else {
        0
    };

    let verdict = if courante.is_some() {
        None
    } else if let Some(c) = &st.confirme {
        Some(Verdict {
            genre: TypeVerdict::Trouve,
            suspect: Some(c.clone()),
            detail: st.detail.clone(),
            restants: vec![c.clone()],
        })
    } else {
        match st.restants.len() {
            1 => Some(Verdict {
                genre: TypeVerdict::Elimination,
                suspect: Some(st.restants[0].clone()),
                detail: String::new(),
                restants: st.restants.clone(),
            }),
            0 => Some(Verdict { genre: TypeVerdict::Rien, suspect: None, detail: String::new(), restants: Vec::new() }),
            _ => Some(Verdict {
                genre: TypeVerdict::Reparateur,
                suspect: None,
                detail: String::new(),
                restants: st.restants.clone(),
            }),
        }
    };

    Enquete {
        etape: courante,
        choix: choix[..n].to_vec(),
        restants: st.restants,
        ecartes: st.ecartes,
        notes: st.notes,
        preuves: st.preuves,
        faits,
        auto,
        verdict,
        a_venir,
    }
}
