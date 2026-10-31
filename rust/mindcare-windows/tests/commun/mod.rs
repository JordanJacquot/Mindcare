//! Ce que les tests partagent : la lecture des juges exportés par outils\exporter-juges-windows.ps1, le faux Windows
//! rempli avec leurs données (les mêmes que celles des fausses fonctions de l'exporteur), une fausse horloge, et les
//! indices mis sous la même forme que dans les juges.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Mutex;

use mindcare_windows::{
    Barrette, Batterie, Disque, DisquePhysique, Evenement, Filtre, Horloge, IndiceWindows, IndicesWindows, InfosPc,
    Mesure, Moment, Partition, Place, SourceWindows,
};
use serde_json::Value;

/// Le dossier des juges (tests\juges).
pub fn dossier_juges() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("juges")
}

/// Un juge.
pub fn lire_juge(nom: &str) -> Value {
    let chemin = dossier_juges().join(nom);
    let texte = std::fs::read_to_string(&chemin).unwrap_or_else(|e| panic!("{} : {e}", chemin.display()));
    serde_json::from_str(&texte).unwrap_or_else(|e| panic!("{nom} : {e}"))
}

pub fn date(v: &Value) -> Moment {
    let t = v.as_str().unwrap_or_else(|| panic!("date attendue : {v}"));
    Moment::lire(t).unwrap_or_else(|| panic!("date illisible : {t}"))
}

fn texte(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn liste(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or_default()
}

fn petit<T: TryFrom<u64>>(v: &Value) -> Option<T> {
    v.as_u64().and_then(|n| T::try_from(n).ok())
}

/// Le faux Windows : les données brutes d'un cas de juge.
pub struct FauxWindows {
    pub maintenant: Moment,
    sonde: Mesure<Option<Moment>>,
    evenements: Vec<Evenement>,
    echecs: Vec<String>,
    disques: Mesure<Vec<Disque>>,
    partitions: Mesure<Vec<Partition>>,
    physiques: Mesure<Vec<DisquePhysique>>,
    lecteur: String,
    place: Mesure<Place>,
    batterie: Mesure<Option<Batterie>>,
    demarrage: Mesure<Moment>,
    barrettes: Mesure<Vec<Barrette>>,
}

/// Une mesure : l'erreur du cas (`Erreurs.<nom>`), sinon la valeur.
fn mesure<T>(erreurs: &Value, nom: &str, valeur: impl FnOnce() -> T) -> Mesure<T> {
    match erreurs.get(nom) {
        Some(e) => Err(texte(e)),
        None => Ok(valeur()),
    }
}

pub fn barrette(b: &Value) -> Barrette {
    Barrette {
        capacite: b["Capacite"].as_u64(),
        fabricant: texte(&b["Fabricant"]),
        reference: texte(&b["Reference"]),
        emplacement: texte(&b["Emplacement"]),
    }
}

impl FauxWindows {
    /// Le faux Windows des données d'un cas (`Donnees`), et ses barrettes éventuelles.
    pub fn depuis(d: &Value, barrettes: &[Value]) -> FauxWindows {
        let erreurs = &d["Erreurs"];
        let sonde = &d["Sonde"];
        let sonde = match (sonde.get("Erreur"), sonde.get("Debut")) {
            (Some(e), _) => Err(texte(e)),
            (None, Some(debut)) => Ok(Some(date(debut))),
            (None, None) => Ok(None),
        };
        let evenements = liste(&d["Evenements"])
            .iter()
            .map(|e| Evenement {
                id: petit(&e["Id"]).unwrap_or(0),
                fournisseur: texte(&e["Fournisseur"]),
                niveau: petit(&e["Niveau"]).unwrap_or(0),
                date: date(&e["Date"]),
                message: texte(&e["Message"]),
                proprietes: liste(&e["Proprietes"]).iter().map(texte).collect(),
            })
            .collect();
        FauxWindows {
            maintenant: date(&d["Maintenant"]),
            sonde,
            evenements,
            echecs: liste(&d["Echecs"]).iter().map(texte).collect(),
            disques: mesure(erreurs, "Disques", || {
                liste(&d["Disques"])
                    .iter()
                    .map(|x| Disque { numero: petit(&x["Numero"]).unwrap_or(0), bus: petit(&x["Bus"]) })
                    .collect()
            }),
            partitions: mesure(erreurs, "Partitions", || {
                liste(&d["Partitions"])
                    .iter()
                    .map(|x| Partition {
                        disque: petit(&x["Disque"]).unwrap_or(0),
                        lettre: petit(&x["Lettre"]).unwrap_or(0),
                    })
                    .collect()
            }),
            physiques: mesure(erreurs, "DisquesPhysiques", || {
                liste(&d["DisquesPhysiques"])
                    .iter()
                    .map(|x| DisquePhysique { nom: texte(&x["Nom"]), bus: petit(&x["Bus"]), sante: petit(&x["Sante"]) })
                    .collect()
            }),
            lecteur: texte(&d["Lecteur"]),
            place: mesure(erreurs, "Place", || Place {
                libre: d["Place"]["Libre"].as_u64().unwrap_or(0),
                taille: d["Place"]["Taille"].as_u64().unwrap_or(0),
            }),
            batterie: mesure(erreurs, "Batterie", || {
                let b = &d["Batterie"];
                (!b.is_null()).then(|| Batterie { origine: texte(&b["Origine"]), pleine: texte(&b["Pleine"]) })
            }),
            demarrage: mesure(erreurs, "Demarrage", || date(&d["Demarrage"])),
            barrettes: mesure(erreurs, "Barrettes", || barrettes.iter().map(barrette).collect()),
        }
    }
}

impl SourceWindows for FauxWindows {
    fn plus_ancien(&self) -> Mesure<Option<Moment>> {
        self.sonde.clone()
    }

    /// Comme la fausse Get-WinEvent de l'exporteur : le fournisseur (sans tenir compte des majuscules), les numéros,
    /// les niveaux, la date de début, les plus récents d'abord, au plus `max`.
    fn evenements(&self, f: &Filtre) -> Mesure<Vec<Evenement>> {
        if self.echecs.iter().any(|e| e.eq_ignore_ascii_case(&f.fournisseur)) {
            return Err(format!("La requête au journal a échoué ({})", f.fournisseur));
        }
        let mut l: Vec<Evenement> = self
            .evenements
            .iter()
            .filter(|e| {
                e.fournisseur.eq_ignore_ascii_case(&f.fournisseur)
                    && (f.ids.is_empty() || f.ids.contains(&e.id))
                    && (f.niveaux.is_empty() || f.niveaux.contains(&e.niveau))
                    && e.date >= f.depuis
            })
            .cloned()
            .collect();
        l.sort_by_key(|e| std::cmp::Reverse(e.date));
        l.truncate(f.max);
        Ok(l)
    }

    fn disques(&self) -> Mesure<Vec<Disque>> {
        self.disques.clone()
    }

    fn partitions(&self) -> Mesure<Vec<Partition>> {
        self.partitions.clone()
    }

    fn disques_physiques(&self) -> Mesure<Vec<DisquePhysique>> {
        self.physiques.clone()
    }

    fn lecteur_systeme(&self) -> String {
        self.lecteur.clone()
    }

    fn place(&self, _lecteur: &str) -> Mesure<Place> {
        self.place.clone()
    }

    fn batterie(&self) -> Mesure<Option<Batterie>> {
        self.batterie.clone()
    }

    fn demarrage(&self) -> Mesure<Moment> {
        self.demarrage.clone()
    }

    fn barrettes(&self) -> Mesure<Vec<Barrette>> {
        self.barrettes.clone()
    }

    fn infos_pc(&self) -> Mesure<InfosPc> {
        Ok(InfosPc::default())
    }
}

/// Une horloge qu'on avance à la main.
pub struct HorlogeFixe(pub Mutex<Moment>);

impl HorlogeFixe {
    pub fn nouvelle(m: Moment) -> HorlogeFixe {
        HorlogeFixe(Mutex::new(m))
    }

    pub fn regler(&self, m: Moment) {
        *self.0.lock().expect("horloge") = m;
    }
}

impl Horloge for HorlogeFixe {
    fn maintenant(&self) -> Moment {
        *self.0.lock().expect("horloge")
    }
}

/// Les indices d'un juge (un objet { nom : { Vu, Texte, TexteOk, Affiche, Partiel } }).
pub fn indices_juge(v: &Value) -> IndicesWindows {
    v.as_object()
        .unwrap_or_else(|| panic!("indices attendus : {v}"))
        .iter()
        .map(|(k, i)| {
            let ind = IndiceWindows {
                vu: i["Vu"].as_bool().expect("Vu"),
                texte: texte(&i["Texte"]),
                texte_ok: texte(&i["TexteOk"]),
                affiche: i["Affiche"].as_bool().expect("Affiche"),
                partiel: i["Partiel"].as_bool().expect("Partiel"),
            };
            (k.clone(), ind)
        })
        .collect()
}

/// Ce qui diffère entre deux tables d'indices, indice par indice (rien : pareilles).
pub fn ecart(attendu: &IndicesWindows, obtenu: &IndicesWindows) -> Option<String> {
    let mut noms: Vec<&String> = attendu.keys().chain(obtenu.keys()).collect();
    noms.sort();
    noms.dedup();
    let diff: Vec<String> = noms
        .into_iter()
        .filter(|n| attendu.get(*n) != obtenu.get(*n))
        .map(|n| format!("{n} : PowerShell {:?}\n      Rust       {:?}", attendu.get(n), obtenu.get(n)))
        .collect();
    if diff.is_empty() { None } else { Some(diff.join("\n    ")) }
}

/// Les noms des indices (triés : une BTreeMap).
pub fn noms(i: &IndicesWindows) -> Vec<&str> {
    i.keys().map(String::as_str).collect()
}

/// Échoue en montrant les premiers écarts (et leur nombre).
pub fn verdict_final(quoi: &str, total: usize, ecarts: &[String]) {
    assert!(total > 0, "{quoi} : aucun cas dans le juge");
    if !ecarts.is_empty() {
        let premiers: Vec<&String> = ecarts.iter().take(10).collect();
        panic!(
            "{quoi} : {} écart(s) sur {total}\n{}",
            ecarts.len(),
            premiers.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
        );
    }
    println!("{quoi} : {total} cas, tous pareils");
}
