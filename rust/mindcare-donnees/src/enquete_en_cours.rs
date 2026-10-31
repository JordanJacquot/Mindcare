//! L'enquête en cours, sans l'écran : ce que modules\contenu.ps1 garde et décide pour les données (`New-DiagId`,
//! `Start-Diagnostic`, `Open-Diagnostic`, `Get-EnqueteEnCours`, `Show-Etape`, `Test-Reprise`, `Get-PairesEnAttente`,
//! `Add-Choix`, `Undo-Choix`, la partie enregistrée de `Show-Verdict`, `Get-ResumeEnCours`).
//!
//! Reprise par Id : chaque réponse gardée ne va qu'à la question qui porte son Id. Une question qui n'est plus là est
//! sautée, une nouvelle est posée, et les suivantes reprennent leur réponse. Ce qui n'a pas pu être rejoué n'est jamais
//! effacé : les réponses encore valables restent gardées avec l'enquête, en attente de leur question ; une enquête
//! reprise dont aucune réponse n'a pu être rejouée reste telle quelle.

use serde_json::Value;

use crate::donnees::Donnees;
use crate::historique::{Enregistrement, IndicesGardes, Paire, convertir_indices, paires};
use crate::moment::Moment;
use crate::rapport::{SurLePc, texte_rapport};
use crate::valeurs::entier;
use mindcare_moteur::ps::{champ, champ_v, egal, parcours, tableau, texte, vrai};
use mindcare_moteur::verdict::minuscule;
use mindcare_moteur::{Catalogue, Enquete, Objet, TypeVerdict, enquete, titre_verdict};

/// `New-DiagId` : « 20261030-140500-0a1f » (la date, puis un nombre au hasard de 0 à 65535 en hexadécimal, donné par
/// l'appelant).
pub fn nouvel_id(m: &Moment, hasard: u16) -> String {
    format!("{}-{hasard:04x}", m.compact())
}

/// Des réponses déjà connues, par Id de question (une table PowerShell : sans tenir compte des majuscules).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreRempli(pub Vec<(String, i64)>);

impl PreRempli {
    pub fn get(&self, id: &str) -> Option<i64> {
        self.0.iter().find(|(k, _)| egal(k, id)).map(|(_, v)| *v)
    }
    pub fn contient(&self, id: &str) -> bool {
        self.get(id).is_some()
    }
    pub fn mettre(&mut self, id: &str, n: i64) {
        match self.0.iter_mut().find(|(k, _)| egal(k, id)) {
            Some((_, v)) => *v = n,
            None => self.0.push((id.to_string(), n)),
        }
    }
    pub fn retirer(&mut self, id: &str) {
        self.0.retain(|(k, _)| !egal(k, id));
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Une enquête reprise de l'historique : son Id et ses réponses gardées, telles quelles.
#[derive(Debug, Clone, PartialEq)]
pub struct Reprise {
    pub id: String,
    pub reponses: Vec<Value>,
}

/// La réponse montrée (pour le rapport : `$script:ResumeDe`).
#[derive(Debug, Clone)]
pub struct ResumeDe {
    pub panne: Objet,
    pub enquete: Enquete,
    pub titre: String,
}

/// Ce qu'il faut montrer après une action.
#[derive(Debug, Clone)]
pub enum Affichage {
    /// L'accueil (panne introuvable)
    Accueil,
    /// La famille de la panne (« Étape précédente » à la première question)
    Famille(String),
    /// Une étape de l'enquête
    Etape(Box<Enquete>),
    /// La réponse finale, avec sa phrase
    Reponse(Box<Enquete>, String),
}

/// L'enquête en cours.
#[derive(Debug, Clone, Default)]
pub struct EnqueteEnCours {
    pub panne: Option<Objet>,
    /// L'enquête telle qu'elle est gardée dans l'historique
    pub diag_id: String,
    /// Les réponses données jusqu'ici
    pub choix: Vec<i64>,
    /// Les réponses déjà connues (la phrase de l'assistant, ou une enquête reprise), données toutes seules
    pub pre_rempli: Option<PreRempli>,
    pub reprise: Option<Reprise>,
    /// Une enquête finie revue : ce que Windows avait noté ce jour-là
    pub indices_gardes: Option<IndicesGardes>,
    pub resume_de: Option<ResumeDe>,
}

impl EnqueteEnCours {
    /// L'Id de la panne en cours.
    fn panne_id(&self) -> String {
        texte(self.panne.as_ref().and_then(|p| champ(p, "Id")))
    }

    /// `Test-Reprise` : une enquête reprise de l'historique, et pas une nouvelle.
    pub fn est_reprise(&self) -> bool {
        self.reprise.as_ref().is_some_and(|r| egal(&r.id, &self.diag_id))
    }

    /// `Start-Diagnostic` : une nouvelle enquête (`pre_rempli` : les réponses déjà données par la phrase ; la question
    /// de sécurité « danger » est toujours posée).
    pub fn commencer(
        &mut self,
        cat: &Catalogue,
        donnees: &mut Donnees,
        id: &str,
        pre_rempli: Option<&PreRempli>,
        diag_id: String,
        indices_du_jour: &IndicesGardes,
    ) -> Affichage {
        // (une panne inconnue : plus de panne en cours, comme PowerShell, et l'accueil)
        let Some(p) = cat.panne(id) else {
            self.panne = None;
            return Affichage::Accueil;
        };
        self.panne = Some(p.clone());
        self.choix.clear();
        self.diag_id = diag_id;
        self.indices_gardes = None;
        self.reprise = None;
        self.pre_rempli = pre_rempli.map(|r| {
            let mut r = r.clone();
            r.retirer("danger");
            r
        });
        let deja = match pre_rempli {
            Some(r) if !r.is_empty() => {
                format!(" (déjà su : {})", r.0.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", "))
            }
            _ => String::new(),
        };
        donnees.journal().ecrire(&format!("Enquête : {id}{deja}"));
        self.montrer(cat, donnees, indices_du_jour)
    }

    /// `Open-Diagnostic` : une enquête gardée, là où on en était (son étape, ou sa réponse).
    pub fn ouvrir(
        &mut self,
        cat: &Catalogue,
        donnees: &mut Donnees,
        id: &str,
        indices_du_jour: &IndicesGardes,
    ) -> Affichage {
        let d = donnees.historique.trouver(id).cloned();
        let p = d.as_ref().and_then(|d| cat.panne(&texte(champ(d, "Panne"))));
        let (Some(d), Some(p)) = (d, p) else {
            donnees.journal().ecrire(&format!("Enquête {id} introuvable"));
            return Affichage::Accueil;
        };
        self.panne = Some(p.clone());
        self.diag_id = id.to_string();
        // (finie : revue avec ce que Windows avait noté ce jour-là, pas celui d'aujourd'hui : sa réponse ne change pas
        // toute seule)
        self.indices_gardes = if vrai(champ(&d, "Fini")) && champ(&d, "Indices").is_some() && vrai(champ(&d, "Indices"))
        {
            Some(convertir_indices(champ(&d, "Indices")))
        } else {
            None
        };
        self.reprise = None;
        let reponses = champ(&d, "Reponses");
        if reponses.is_some() && !tableau(reponses).is_empty() {
            // Chaque réponse ne va qu'à la question qui porte son Id
            self.choix.clear();
            let mut pr = PreRempli::default();
            for r in parcours(reponses) {
                if let Some(n) = entier(champ_v(Some(r), "Reponse")) {
                    pr.mettre(&texte(champ_v(Some(r), "Etape")), n);
                }
            }
            self.pre_rempli = Some(pr);
            // (le catalogue a pu changer depuis : ce qui n'a pas pu être rejoué n'est jamais effacé)
            self.reprise =
                Some(Reprise { id: id.to_string(), reponses: tableau(reponses).into_iter().cloned().collect() });
        } else {
            // (ancien format : les numéros seulement)
            self.choix = parcours(champ(&d, "Choix")).into_iter().filter_map(|n| entier(Some(n))).collect();
            self.pre_rempli = None;
        }
        donnees.journal().ecrire(&format!("Reprise de l'enquête {id} ({})", texte(champ(p, "Id"))));
        self.montrer(cat, donnees, indices_du_jour)
    }

    /// `Get-EnqueteEnCours` : l'enquête rejouée ; ouverte sur le PC en panne (« cepc »), avec ce que Windows a noté
    /// (ce jour-là pour une enquête finie revue, sinon aujourd'hui).
    pub fn enquete(&self, cat: &Catalogue, indices_du_jour: &IndicesGardes) -> Option<Enquete> {
        let p = self.panne.as_ref()?;
        let e = enquete(cat, p, &self.choix, None);
        if e.notes.iter().any(|n| egal(n, "cepc")) {
            let i = self.indices_gardes.as_ref().unwrap_or(indices_du_jour).moteur();
            return Some(enquete(cat, p, &self.choix, Some(&i)));
        }
        Some(e)
    }

    /// `Show-Etape` sans l'écran : les questions auxquelles on a déjà répondu reçoivent leur réponse toutes seules (une
    /// seule fois), puis l'étape est enregistrée (une enquête déjà finie le reste tant qu'une nouvelle réponse ne la
    /// remplace pas), ou c'est la réponse finale.
    pub fn montrer(&mut self, cat: &Catalogue, donnees: &mut Donnees, indices_du_jour: &IndicesGardes) -> Affichage {
        let Some(mut e) = self.enquete(cat, indices_du_jour) else { return Affichage::Accueil };
        while let Some(etape) = &e.etape {
            let id = texte(champ(etape, "Id"));
            let Some(n) = self.pre_rempli.as_ref().and_then(|r| r.get(&id)) else { break };
            if let Some(r) = self.pre_rempli.as_mut() {
                r.retirer(&id);
            }
            // (-1 : « Je ne sais pas », gardé dans une enquête reprise)
            if n < -1 || n >= tableau(champ(etape, "Reponses")).len() as i64 {
                break;
            }
            self.choix = e.choix.clone();
            self.choix.push(n);
            match self.enquete(cat, indices_du_jour) {
                Some(x) => e = x,
                None => break,
            }
        }
        self.choix = e.choix.clone();
        if e.etape.is_some() {
            let fini = donnees.historique.trouver(&self.diag_id).is_some_and(|a| vrai(champ(a, "Fini")));
            if !fini {
                let mut reponses = paires(&e);
                reponses.extend(self.paires_en_attente(cat, &e));
                let panne = self.panne_id();
                let choix = self.choix.clone();
                donnees.sauver_diagnostic(
                    Some(cat),
                    &Enregistrement {
                        id: &self.diag_id,
                        panne: &panne,
                        choix: &choix,
                        fini: false,
                        suspect: None,
                        titre: "",
                        reponses: &reponses,
                        indices: None,
                        reprise: self.est_reprise(),
                    },
                );
            }
            return Affichage::Etape(Box::new(e));
        }
        self.reponse(cat, donnees, e, indices_du_jour)
    }

    /// La partie enregistrée de `Show-Verdict` : la phrase de la réponse (une pièce seulement restée par élimination
    /// n'a pas été testée : « À vérifier : ... »), l'enquête finie gardée avec ce que Windows avait noté.
    fn reponse(
        &mut self,
        cat: &Catalogue,
        donnees: &mut Donnees,
        e: Enquete,
        indices_du_jour: &IndicesGardes,
    ) -> Affichage {
        let v = e.verdict.clone();
        let mut titre = titre_verdict(cat, v.as_ref());
        let suspect = v.as_ref().and_then(|v| v.suspect.as_deref()).and_then(|s| cat.suspect(s));
        let piece = suspect.is_some_and(|s| vrai(champ(s, "Piece")));
        let supposee = piece && v.as_ref().is_some_and(|v| v.genre == TypeVerdict::Elimination);
        if supposee {
            titre = format!("À vérifier : {}", minuscule(&texte(suspect.and_then(|s| champ(s, "Titre")))));
        }
        let panne = self.panne_id();
        let genre = v.as_ref().map(|v| v.genre.nom()).unwrap_or_default();
        let suspect_id = v.as_ref().and_then(|v| v.suspect.clone()).filter(|s| !s.is_empty());
        donnees.journal().ecrire(&format!(
            "Réponse {panne} : {genre}{} (réponses {})",
            suspect_id.as_ref().map(|s| format!(" {s}")).unwrap_or_default(),
            self.choix.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        ));
        let indices = if e.notes.iter().any(|n| egal(n, "cepc")) {
            Some(self.indices_gardes.clone().unwrap_or_else(|| indices_du_jour.clone()))
        } else {
            None
        };
        let reponses = paires(&e);
        let choix = self.choix.clone();
        donnees.sauver_diagnostic(
            Some(cat),
            &Enregistrement {
                id: &self.diag_id,
                panne: &panne,
                choix: &choix,
                fini: true,
                suspect: suspect_id.as_deref(),
                titre: &titre,
                reponses: &reponses,
                indices: indices.as_ref(),
                reprise: self.est_reprise(),
            },
        );
        if let Some(p) = &self.panne {
            self.resume_de = Some(ResumeDe { panne: p.clone(), enquete: e.clone(), titre: titre.clone() });
        }
        if piece {
            donnees.journal().ecrire(&format!(
                "Avant d'acheter : pièce {}",
                if supposee { "supposée (pas testée)" } else { "confirmée" }
            ));
        }
        Affichage::Reponse(Box::new(e), titre)
    }

    /// `Add-Choix` : une réponse de plus.
    pub fn repondre(
        &mut self,
        cat: &Catalogue,
        donnees: &mut Donnees,
        n: i64,
        indices_du_jour: &IndicesGardes,
    ) -> Affichage {
        self.choix.push(n);
        self.montrer(cat, donnees, indices_du_jour)
    }

    /// `Undo-Choix` : « Étape précédente » (à la première question : la famille de la panne).
    pub fn retour(&mut self, cat: &Catalogue, donnees: &mut Donnees, indices_du_jour: &IndicesGardes) -> Affichage {
        if self.choix.is_empty() {
            return Affichage::Famille(texte(self.panne.as_ref().and_then(|p| champ(p, "Famille"))));
        }
        self.choix.pop();
        self.montrer(cat, donnees, indices_du_jour)
    }

    /// `Get-PairesEnAttente` : les réponses d'une enquête reprise pas encore rejouées (leur question n'est pas encore
    /// revenue) et toujours valables (la question existe encore dans le catalogue, et cette réponse aussi). Celles
    /// d'une question disparue sont laissées.
    pub fn paires_en_attente(&self, cat: &Catalogue, e: &Enquete) -> Vec<Paire> {
        let Some(pr) = self.pre_rempli.as_ref().filter(|r| !r.is_empty()) else { return Vec::new() };
        if !self.est_reprise() {
            return Vec::new();
        }
        let Some(panne) = &self.panne else { return Vec::new() };
        let mut nb: Vec<(String, i64)> = Vec::new();
        for x in cat.etapes(panne) {
            if !egal(&texte(champ(&x, "Type")), "auto") {
                let id = texte(champ(&x, "Id"));
                let n = tableau(champ(&x, "Reponses")).len() as i64;
                match nb.iter_mut().find(|(k, _)| egal(k, &id)) {
                    Some((_, v)) => *v = n,
                    None => nb.push((id, n)),
                }
            }
        }
        let faites: Vec<String> = e.faits.iter().map(|f| texte(champ(&f.etape, "Id"))).collect();
        let mut l = Vec::new();
        for r in self.reprise.iter().flat_map(|r| r.reponses.iter()) {
            let id = texte(champ_v(Some(r), "Etape"));
            let Some(n) = entier(champ_v(Some(r), "Reponse")) else { continue };
            let max = nb.iter().find(|(k, _)| egal(k, &id)).map(|(_, v)| *v);
            if pr.contient(&id) && !faites.iter().any(|f| egal(f, &id)) && max.is_some_and(|m| n >= -1 && n < m) {
                l.push(Paire { etape: id, reponse: n });
            }
        }
        l
    }

    /// `Get-ResumeEnCours` : le rapport de la réponse montrée.
    pub fn rapport(&self, cat: &Catalogue, maintenant: &Moment, pc: &SurLePc, version: &str) -> Option<String> {
        let r = self.resume_de.as_ref()?;
        Some(texte_rapport(cat, &r.panne, &r.enquete, &r.titre, maintenant, pc, version))
    }
}
