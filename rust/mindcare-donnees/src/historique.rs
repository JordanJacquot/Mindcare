//! Les enquêtes gardées (diagnostics.json, dans le dossier de données) : les 10 dernières, finies ou non, pour en
//! reprendre une, revoir sa réponse, ou en faire un rapport pour le réparateur (modules\historique.ps1).
//!
//! Une enquête gardée : { Id, Panne, Choix, Reponses ([{ Etape, Reponse }] : chaque réponse avec l'Id de sa question),
//! Debut, Maj (dates), Fini, Suspect (trouvé), Titre (la réponse), Indices (finie : ce que Windows avait noté, pour la
//! revoir à l'identique) }. Le fichier : { Version: 2, Diagnostics: [...] }.
//!
//! Les anciens formats se lisent aussi : version 1 (Choix et Causes, sans Suspect ni Titre), version 2 sans Reponses
//! (réponses rejouées par leur place) ni Indices. Les enquêtes lues sont gardées telles quelles (tous leurs champs),
//! comme les objets de PowerShell : elles sont réécrites à l'identique.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::fichiers::Fichiers;
use crate::valeurs::egal_texte;
use mindcare_moteur::ps::{champ, champ_v, cle, egal, parcours, tableau, texte, vrai};
use mindcare_moteur::{Catalogue, Enquete, Indice, Indices, Objet};

/// Le nombre d'enquêtes gardées.
pub const HISTORIQUE_MAX: usize = 10;

/// Un indice de Windows tel qu'il est gardé avec une enquête finie (`Get-Indices`, `ConvertTo-Indices`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndiceGarde {
    pub vu: bool,
    pub texte: String,
    pub texte_ok: String,
    pub affiche: bool,
    pub partiel: bool,
}

/// Des indices (une table PowerShell : les noms sans tenir compte des majuscules, le premier nom écrit est gardé).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndicesGardes(pub Vec<(String, IndiceGarde)>);

impl IndicesGardes {
    /// Un indice par son nom.
    pub fn get(&self, nom: &str) -> Option<&IndiceGarde> {
        self.0.iter().find(|(k, _)| egal(k, nom)).map(|(_, v)| v)
    }

    /// `$h[$nom] = $indice`.
    pub fn mettre(&mut self, nom: &str, indice: IndiceGarde) {
        match self.0.iter_mut().find(|(k, _)| egal(k, nom)) {
            Some((_, v)) => *v = indice,
            None => self.0.push((nom.to_string(), indice)),
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Les indices pour le moteur.
    pub fn moteur(&self) -> Indices {
        self.0
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    Indice { vu: v.vu, texte: v.texte.clone(), texte_ok: v.texte_ok.clone(), partiel: v.partiel },
                )
            })
            .collect()
    }

    /// Comme ils sont écrits dans diagnostics.json : rangés par nom (`Sort-Object`).
    pub fn json(&self) -> Value {
        let mut noms: Vec<&(String, IndiceGarde)> = self.0.iter().collect();
        noms.sort_by(|a, b| ordre_culture(&a.0, &b.0));
        let mut o = Map::new();
        for (k, v) in noms {
            o.insert(
                k.clone(),
                json!({ "Vu": v.vu, "Texte": v.texte, "TexteOk": v.texte_ok, "Affiche": v.affiche, "Partiel": v.partiel }),
            );
        }
        Value::Object(o)
    }
}

/// L'ordre de `Sort-Object` sur des noms (culture du PC, sans tenir compte des majuscules) : les tirets et les
/// apostrophes ne comptent qu'en dernier, comme le tri des mots de Windows.
pub fn ordre_culture(a: &str, b: &str) -> Ordering {
    let sans = |t: &str| t.chars().filter(|c| *c != '-' && *c != '\'').flat_map(char::to_lowercase).collect::<String>();
    sans(a).cmp(&sans(b)).then_with(|| a.to_lowercase().cmp(&b.to_lowercase())).then_with(|| a.cmp(b))
}

/// `ConvertTo-Indices` : les indices gardés avec une enquête finie (lus du fichier).
pub fn convertir_indices(gardes: Option<&Value>) -> IndicesGardes {
    let mut h = IndicesGardes::default();
    if !vrai(gardes) {
        return h;
    }
    if let Some(Value::Object(o)) = gardes {
        for (k, v) in o {
            let v = Some(v);
            h.mettre(
                k,
                IndiceGarde {
                    vu: vrai(champ_v(v, "Vu")),
                    texte: texte(champ_v(v, "Texte")),
                    texte_ok: texte(champ_v(v, "TexteOk")),
                    affiche: vrai(champ_v(v, "Affiche")),
                    partiel: vrai(champ_v(v, "Partiel")),
                },
            );
        }
    }
    h
}

/// Une réponse avec l'Id de sa question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paire {
    pub etape: String,
    pub reponse: i64,
}

impl Paire {
    pub fn json(&self) -> Value {
        json!({ "Etape": self.etape, "Reponse": self.reponse })
    }
}

/// `Get-Paires` : les réponses d'une enquête avec l'Id de leur question (une réponse par question posée).
pub fn paires(e: &Enquete) -> Vec<Paire> {
    e.faits
        .iter()
        .enumerate()
        .map(|(k, f)| Paire { etape: texte(champ(&f.etape, "Id")), reponse: e.choix.get(k).copied().unwrap_or(0) })
        .collect()
}

/// Ce qu'on demande d'enregistrer (`Save-Diagnostic`).
#[derive(Debug, Clone)]
pub struct Enregistrement<'a> {
    pub id: &'a str,
    pub panne: &'a str,
    pub choix: &'a [i64],
    pub fini: bool,
    pub suspect: Option<&'a str>,
    pub titre: &'a str,
    pub reponses: &'a [Paire],
    pub indices: Option<&'a IndicesGardes>,
    /// Une enquête reprise de l'historique : sans aucune réponse rejouée, elle reste telle quelle.
    pub reprise: bool,
}

/// Les enquêtes gardées, de la plus récente à la plus ancienne.
#[derive(Debug, Clone)]
pub struct Historique {
    fichier: PathBuf,
    pub liste: Vec<Objet>,
    /// (les enquêtes enregistrées depuis la lecture : gardées si le fichier était pris au lancement)
    changes: HashSet<String>,
}

/// `$d.Id -and $d.Panne` : une enquête gardée valable.
fn valable(d: &Objet) -> bool {
    vrai(champ(d, "Id")) && vrai(champ(d, "Panne"))
}

/// Les enquêtes valables d'un contenu de diagnostics.json.
fn diagnostics(j: &Objet) -> Vec<Objet> {
    parcours(champ(j, "Diagnostics")).into_iter().filter_map(Value::as_object).filter(|d| valable(d)).cloned().collect()
}

/// `"$($_.Etape)=$($_.Reponse)"` pour chaque réponse, séparées par des virgules (rien passé dans un pipeline fait un
/// tour, avec rien).
fn texte_reponses(v: Option<&Value>) -> String {
    let elements: Vec<&Value> = match v {
        None | Some(Value::Null) => vec![&Value::Null],
        Some(Value::Array(a)) => a.iter().collect(),
        Some(x) => vec![x],
    };
    elements
        .iter()
        .map(|x| format!("{}={}", texte(champ_v(Some(x), "Etape")), texte(champ_v(Some(x), "Reponse"))))
        .collect::<Vec<_>>()
        .join(",")
}

impl Historique {
    /// Un historique vide (avant `lire`).
    pub fn nouveau(fichier: &Path) -> Historique {
        Historique { fichier: fichier.to_path_buf(), liste: Vec::new(), changes: HashSet::new() }
    }

    pub fn fichier(&self) -> &Path {
        &self.fichier
    }

    /// `Read-Historique` : relit diagnostics.json (v2 ou ancien format).
    pub fn lire(&mut self, fichiers: &mut Fichiers) {
        let j = fichiers.lire(&self.fichier);
        self.liste = j.as_ref().map(diagnostics).unwrap_or_default();
        self.changes.clear();
    }

    /// Une enquête gardée par son Id.
    pub fn trouver(&self, id: &str) -> Option<&Objet> {
        self.liste.iter().find(|d| egal_texte(champ(d, "Id"), id))
    }

    /// `Save-Diagnostic` : enregistre où en est une enquête, qui passe en tête de liste. Sans aucune réponse, elle
    /// n'est pas gardée (sauf une enquête reprise : elle reste alors telle quelle). Revoir une enquête sans rien y
    /// changer ne la déplace pas. Vrai si l'historique a été enregistré (ou a essayé de l'être) : la carte de Mindset
    /// est alors à réécrire.
    pub fn sauver(&mut self, fichiers: &mut Fichiers, e: &Enregistrement) -> bool {
        let mut ancien = self.trouver(e.id).cloned();
        if let Some(a) = &ancien {
            let memes_reponses = champ(a, "Reponses").is_none()
                || egal(
                    &texte_reponses(champ(a, "Reponses")),
                    &e.reponses.iter().map(|p| format!("{}={}", p.etape, p.reponse)).collect::<Vec<_>>().join(","),
                );
            let choix_ancien = tableau(champ(a, "Choix")).iter().map(|x| texte(Some(x))).collect::<Vec<_>>().join(",");
            let choix = e.choix.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            if egal(&choix_ancien, &choix)
                && vrai(champ(a, "Fini")) == e.fini
                && egal(&texte(champ(a, "Titre")), e.titre)
                && memes_reponses
            {
                return false;
            }
            if e.reprise && e.choix.is_empty() {
                return false;
            }
        }
        self.changes.insert(cle(e.id));
        let f = fichiers.relire(&self.fichier);
        if f.relu {
            // (celles de cette session d'abord, en tête comme avant ; puis celles du fichier, dans leur ordre)
            let changes = &self.changes;
            let pris_session = |d: &Objet| changes.contains(&cle(&texte(champ(d, "Id"))));
            let mut liste: Vec<Objet> = self.liste.iter().filter(|d| pris_session(d)).cloned().collect();
            if let Some(c) = &f.contenu {
                liste.extend(diagnostics(c).into_iter().filter(|d| !pris_session(d)));
            }
            self.liste = liste;
            ancien = self.trouver(e.id).cloned();
        }
        let mut liste: Vec<Objet> = self.liste.iter().filter(|d| !egal_texte(champ(d, "Id"), e.id)).cloned().collect();
        if !e.choix.is_empty() {
            let maintenant = fichiers.journal.maintenant().iso();
            let debut = match &ancien {
                Some(a) => texte(champ(a, "Debut")),
                None => maintenant.clone(),
            };
            let mut d = Map::new();
            d.insert("Id".into(), json!(e.id));
            d.insert("Panne".into(), json!(e.panne));
            d.insert("Choix".into(), json!(e.choix));
            d.insert("Reponses".into(), Value::Array(e.reponses.iter().map(Paire::json).collect()));
            d.insert("Debut".into(), json!(debut));
            d.insert("Maj".into(), json!(maintenant));
            d.insert("Fini".into(), json!(e.fini));
            d.insert("Suspect".into(), json!(e.suspect.unwrap_or_default()));
            d.insert("Titre".into(), json!(e.titre));
            if e.fini
                && let Some(i) = e.indices
                && !i.is_empty()
            {
                d.insert("Indices".into(), i.json());
            }
            liste.insert(0, d);
        }
        liste.truncate(HISTORIQUE_MAX);
        self.liste = liste;
        let fichier = json!({ "Version": 2, "Diagnostics": self.liste });
        if let Err(err) = fichiers.sauver(&self.fichier, &fichier) {
            fichiers.journal.ecrire(&format!("Enquêtes non enregistrées : {err}"));
        }
        true
    }
}

/// Ce que dit une enquête gardée (`Get-DiagnosticInfo`) : sa panne (absente d'une ancienne version : elle n'est plus
/// montrée), son suspect trouvé, et une phrase (la réponse, ou « pas finie »).
#[derive(Debug, Clone, PartialEq)]
pub struct InfoDiagnostic<'a> {
    pub panne: Option<&'a Objet>,
    pub suspect: Option<&'a Objet>,
    pub phrase: String,
}

/// `Get-DiagnosticInfo`.
pub fn info_diagnostic<'a>(cat: Option<&'a Catalogue>, d: &Objet) -> InfoDiagnostic<'a> {
    let panne = cat.and_then(|c| c.panne(&texte(champ(d, "Panne"))));
    let suspect =
        if vrai(champ(d, "Suspect")) { cat.and_then(|c| c.suspect(&texte(champ(d, "Suspect")))) } else { None };
    let phrase = if !vrai(champ(d, "Fini")) {
        "pas finie".to_string()
    } else if vrai(champ(d, "Titre")) {
        texte(champ(d, "Titre"))
    } else {
        "à faire vérifier".to_string()
    };
    InfoDiagnostic { panne, suspect, phrase }
}
