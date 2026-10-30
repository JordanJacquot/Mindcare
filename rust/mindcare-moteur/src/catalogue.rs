//! Le catalogue des pannes (enquetes.json, le même fichier pour le PC et le téléphone), lu tel quel : Read-Enquetes,
//! Get-Panne, Get-Famille, Get-Suspect et Get-Etapes de modules\enquete.ps1.

use std::path::Path;

use serde_json::Value;

use crate::ps::{Objet, champ, egal, mettre, parcours, texte, vrai};

/// Le catalogue lu. Il ne change jamais une fois lu.
#[derive(Debug, Clone)]
pub struct Catalogue {
    racine: Objet,
    pannes: Vec<Objet>,
    familles: Vec<Objet>,
    vide: Objet,
}

impl Catalogue {
    /// Lit le texte de enquetes.json (Read-Enquetes). Un fichier qui n'est pas un objet JSON est refusé.
    pub fn lire(texte_json: &str) -> Result<Catalogue, String> {
        let t = texte_json.strip_prefix('\u{feff}').unwrap_or(texte_json);
        let v: Value = serde_json::from_str(t).map_err(|e| format!("enquetes.json illisible : {e}"))?;
        let Value::Object(racine) = v else {
            return Err("enquetes.json illisible : ce n'est pas un objet".to_string());
        };
        // (une panne ou une famille qui n'est pas un objet n'a aucun champ, comme `$texte.Id` en PowerShell)
        let objets = |nom: &str| -> Vec<Objet> {
            parcours(champ(&racine, nom)).into_iter().map(|x| x.as_object().cloned().unwrap_or_default()).collect()
        };
        let pannes = objets("Pannes");
        let familles = objets("Familles");
        Ok(Catalogue { racine, pannes, familles, vide: Objet::new() })
    }

    /// Lit enquetes.json sur le disque.
    pub fn lire_fichier(chemin: &Path) -> Result<Catalogue, String> {
        let octets = std::fs::read(chemin).map_err(|e| format!("{} illisible : {e}", chemin.display()))?;
        Catalogue::lire(&String::from_utf8_lossy(&octets))
    }

    /// Le fichier lui-même, pour lire ce que le moteur n'utilise pas (Version, Securite...).
    pub fn json(&self) -> &Objet {
        &self.racine
    }

    /// Les pannes, dans l'ordre du fichier.
    pub fn pannes(&self) -> &[Objet] {
        &self.pannes
    }

    /// Les familles, dans l'ordre du fichier.
    pub fn familles(&self) -> &[Objet] {
        &self.familles
    }

    /// Une panne par son Id (Get-Panne : la première, sans tenir compte des majuscules).
    pub fn panne(&self, id: &str) -> Option<&Objet> {
        self.pannes.iter().find(|p| champ(p, "Id").is_some_and(|v| !v.is_null() && egal(&texte(Some(v)), id)))
    }

    /// Une famille par son Id (Get-Famille).
    pub fn famille(&self, id: &str) -> Option<&Objet> {
        self.familles.iter().find(|f| champ(f, "Id").is_some_and(|v| !v.is_null() && egal(&texte(Some(v)), id)))
    }

    /// Tous les suspects (nom, valeur), dans l'ordre du fichier.
    pub fn suspects(&self) -> &Objet {
        match champ(&self.racine, "Suspects") {
            Some(Value::Object(o)) => o,
            _ => &self.vide,
        }
    }

    /// Les tests partagés (nom, valeur).
    pub fn tests(&self) -> &Objet {
        match champ(&self.racine, "Tests") {
            Some(Value::Object(o)) => o,
            _ => &self.vide,
        }
    }

    /// Un suspect par son Id (Get-Suspect : `$Donnees.Suspects.$Id`, sans tenir compte des majuscules). Aucun pour un
    /// Id vide.
    pub fn suspect(&self, id: &str) -> Option<&Objet> {
        if id.is_empty() {
            return None;
        }
        champ(self.suspects(), id).and_then(Value::as_object)
    }

    /// Un test partagé par son nom (`$Donnees.Tests.($nom)`), tel quel.
    pub fn test(&self, nom: &str) -> Option<&Value> {
        champ(self.tests(), nom)
    }

    /// Les étapes d'une panne, prêtes (Get-Etapes) : une étape « Ref » prend le test partagé du même nom (et son Id),
    /// puis ses propres champs par-dessus (sauf Ref).
    pub fn etapes(&self, panne: &Objet) -> Vec<Objet> {
        parcours(champ(panne, "Etapes"))
            .into_iter()
            .map(|e| {
                let propre = e.as_object();
                let reference = propre.and_then(|o| champ(o, "Ref"));
                let base = if vrai(reference) { self.test(&texte(reference)) } else { None };
                let mut o = Objet::new();
                if vrai(base) {
                    // (un test partagé qui n'est pas un objet n'apporte que son Id : jamais dans le catalogue)
                    if let Some(Value::Object(b)) = base {
                        for (k, v) in b {
                            mettre(&mut o, k, v.clone());
                        }
                    }
                    mettre(&mut o, "Id", Value::String(texte(reference)));
                }
                if let Some(p) = propre {
                    for (k, v) in p {
                        if !k.eq_ignore_ascii_case("Ref") {
                            mettre(&mut o, k, v.clone());
                        }
                    }
                }
                o
            })
            .collect()
    }
}
