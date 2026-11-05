//! Le catalogue des pannes (enquetes.json, le même fichier pour le PC et le téléphone), lu tel quel : Read-Enquetes,
//! Get-Panne, Get-Famille, Get-Suspect et Get-Etapes de modules\enquete.ps1.

use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;

use crate::pc::{ProfilPc, Tri, trier};
use crate::ps::{Objet, champ, egal, mettre, parcours, texte, vrai};

/// Un groupe de pannes d'une famille (une carte dépliable de l'écran de la famille) : « Il ne s'allume pas »...
#[derive(Debug, Clone, PartialEq)]
pub struct Groupe<'a> {
    pub id: String,
    /// Le nom court (« Il ne s'allume pas »).
    pub nom: String,
    /// Une phrase d'exemple (« Rien ne se passe, ou juste une lumière »).
    pub exemple: String,
    /// Le nom d'une image de Mindcare\fichiers\images, sans « .svg ».
    pub icone: String,
    /// Ses pannes, dans l'ordre du fichier.
    pub pannes: Vec<&'a Objet>,
}

/// Les groupes d'une famille pour un PC : les groupes qui gardent au moins une panne, et combien de pannes de la
/// famille sont cachées.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupesFamille<'a> {
    pub groupes: Vec<Groupe<'a>>,
    pub cachees: usize,
}

/// Le catalogue lu. Il ne change jamais une fois lu.
#[derive(Debug, Clone)]
pub struct Catalogue {
    racine: Objet,
    pannes: Vec<Objet>,
    familles: Vec<Objet>,
    vide: Objet,
    /// Les pannes en texte simple pour la recherche, préparées à la première recherche.
    fiches: OnceLock<Vec<crate::recherche::Fiche>>,
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
        Ok(Catalogue { racine, pannes, familles, vide: Objet::new(), fiches: OnceLock::new() })
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

    /// Les pannes en texte simple pour la recherche (dans l'ordre du fichier), préparées une seule fois.
    pub(crate) fn fiches(&self) -> &[crate::recherche::Fiche] {
        self.fiches.get_or_init(|| crate::recherche::fiches(self))
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

    /// Les pannes d'une famille (son Id, sans tenir compte des majuscules), dans l'ordre du fichier.
    pub fn pannes_famille(&self, famille: &str) -> Vec<&Objet> {
        self.pannes.iter().filter(|p| egal(&texte(champ(p, "Famille")), famille)).collect()
    }

    /// Les pannes d'une famille qui concernent ce PC, et combien sont cachées (« PourPc », voir [`crate::pc`]).
    pub fn pannes_famille_pour(&self, famille: &str, profil: &ProfilPc) -> Tri<'_> {
        trier(self.pannes_famille(famille), profil)
    }

    /// Toutes les pannes qui concernent ce PC, et combien sont cachées.
    pub fn pannes_pour(&self, profil: &ProfilPc) -> Tri<'_> {
        trier(&self.pannes, profil)
    }

    /// Les groupes d'une famille (son champ « Groupes »), dans l'ordre du fichier, chacun avec ses pannes (celles dont
    /// le champ « Groupe » le cite, dans l'ordre du fichier). Une panne sans groupe connu (refusé par la validation)
    /// n'est jamais perdue : elle va dans un dernier groupe « Autres pannes » (Id vide). Une famille sans groupes : un
    /// seul groupe « Autres pannes » avec toutes ses pannes.
    pub fn groupes(&self, famille: &str) -> Vec<Groupe<'_>> {
        self.groupes_de(self.pannes_famille(famille), famille)
    }

    /// Les groupes d'une famille après le filtre du profil (« PourPc ») : un groupe vidé par le filtre disparaît, et
    /// `cachees` dit combien de pannes de la famille sont cachées.
    pub fn groupes_pour(&self, famille: &str, profil: &ProfilPc) -> GroupesFamille<'_> {
        let tri = self.pannes_famille_pour(famille, profil);
        let groupes = self.groupes_de(tri.visibles, famille).into_iter().filter(|g| !g.pannes.is_empty()).collect();
        GroupesFamille { groupes, cachees: tri.cachees }
    }

    fn groupes_de<'a>(&'a self, pannes: Vec<&'a Objet>, famille: &str) -> Vec<Groupe<'a>> {
        let t = |o: &Objet, c: &str| texte(champ(o, c));
        let mut groupes: Vec<Groupe<'a>> = self
            .famille(famille)
            .map(|f| parcours(champ(f, "Groupes")))
            .unwrap_or_default()
            .into_iter()
            .filter_map(Value::as_object)
            .map(|g| Groupe {
                id: t(g, "Id"),
                nom: t(g, "Nom"),
                exemple: t(g, "Exemple"),
                icone: t(g, "Icone"),
                pannes: Vec::new(),
            })
            .filter(|g| !g.id.is_empty())
            .collect();
        let mut autres = Vec::new();
        for p in pannes {
            let id = t(p, "Groupe");
            match groupes.iter_mut().find(|g| !id.is_empty() && egal(&g.id, &id)) {
                Some(g) => g.pannes.push(p),
                None => autres.push(p),
            }
        }
        if !autres.is_empty() {
            groupes.push(Groupe {
                id: String::new(),
                nom: "Autres pannes".to_string(),
                exemple: String::new(),
                icone: self.famille(famille).map(|f| t(f, "Icone")).unwrap_or_default(),
                pannes: autres,
            });
        }
        groupes
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
