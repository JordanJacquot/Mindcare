//! L'installation de l'assistant ou de la dictée, vue par les pages : `Start-IaInstall`, `Start-DicteeInstall`, et ce
//! que montrent `Update-BandeauIa` et `Update-ZoneDictee` (modules\contenu.ps1), réduit à un [`Etat`].

use std::sync::Arc;
use std::time::Duration;

use crate::fichiers::Paquet;
use crate::telechargement::{Disque, PAUSE, Telechargement, Transport};

/// Ce que les pages montrent, dans l'ordre où le PowerShell le choisit (`Update-BandeauIa`)
#[derive(Clone, Debug, PartialEq)]
pub enum Etat {
    /// Le téléchargement tourne : « Installation de l'assistant : 42 % (moteur de l'assistant) »
    EnCours { pourcent: u8, etape: String },
    /// Le moteur et le modèle de cette version sont là
    Pret,
    /// Le dernier téléchargement n'a pas marché (bouton « Réessayer ») : le message pour les gens
    Erreur(String),
    /// Le programme est là sans toutes ses .dll : une installation coupée (bouton « Réparer »)
    AReparer { taille_mo: u64 },
    /// Un ancien modèle marche en attendant : la nouvelle version est proposée (bouton « Mettre à jour »)
    AMettreAJour { taille_mo: u64 },
    /// Rien n'est installé (bouton « Installer »)
    Rien { taille_mo: u64 },
}

/// Une installation (l'assistant ou la dictée) et son téléchargement en cours ou fini
pub struct Installation {
    pub paquet: Paquet,
    /// La pause avant de réessayer après une coupure ([`PAUSE`] ; zéro dans les tests)
    pub pause: Duration,
    telechargement: Option<Telechargement>,
    fin_rendue: bool,
}

/// La fin d'un téléchargement, rendue une fois par [`Installation::fin`] (pour le journal, et pour l'assistant :
/// arrêter llama-server puis effacer les anciens modèles)
#[derive(Clone, Debug, PartialEq)]
pub enum Fin {
    Installe,
    Erreur(String),
    Annule,
}

impl Installation {
    pub fn nouvelle(paquet: Paquet) -> Installation {
        Installation { paquet, pause: PAUSE, telechargement: None, fin_rendue: false }
    }

    /// Le téléchargement tourne
    pub fn en_cours(&self) -> bool {
        self.telechargement.as_ref().is_some_and(|t| !t.fini())
    }

    /// Lance le téléchargement de ce qui manque, sauf s'il tourne déjà (`Start-IaInstall`). Rend la ligne du journal
    /// (« Installation de l'assistant (670 Mo) ») s'il commence vraiment.
    pub fn lancer(&mut self, transport: Arc<dyn Transport>, disque: &dyn Disque) -> Option<u64> {
        if self.en_cours() {
            return None;
        }
        let p = &self.paquet;
        let t = Telechargement::lancer(p.manquants(), &p.dossier, p.marge, transport, disque, self.pause);
        let a = t.avancement();
        self.telechargement = Some(t);
        self.fin_rendue = false;
        (!a.fini).then(|| (a.total as f64 / crate::fichiers::MO as f64).round() as u64)
    }

    /// Ce que les pages montrent
    pub fn etat(&self) -> Etat {
        let a = self.telechargement.as_ref().map(|t| t.avancement());
        if let Some(a) = &a
            && !a.fini
        {
            return Etat::EnCours { pourcent: a.pourcent(), etape: a.etape.clone() };
        }
        let p = &self.paquet;
        if p.a_jour() {
            return Etat::Pret;
        }
        let taille_mo = p.taille_manquants_mo();
        if let Some(a) = &a
            && !a.erreur.is_empty()
        {
            return Etat::Erreur(a.erreur.clone());
        }
        if p.incomplet() {
            Etat::AReparer { taille_mo }
        } else if p.installe() {
            Etat::AMettreAJour { taille_mo }
        } else {
            Etat::Rien { taille_mo }
        }
    }

    /// La fin du dernier téléchargement, une seule fois (`Start-SuiviIa`) ; `None` s'il tourne encore ou si elle est
    /// déjà rendue
    pub fn fin(&mut self) -> Option<Fin> {
        let a = self.telechargement.as_ref()?.avancement();
        if !a.fini || self.fin_rendue {
            return None;
        }
        self.fin_rendue = true;
        Some(if a.annule {
            Fin::Annule
        } else if a.erreur.is_empty() {
            Fin::Installe
        } else {
            Fin::Erreur(a.erreur)
        })
    }

    /// Mindcare se ferme : le téléchargement s'arrête (ce qui est commencé reste, pour la reprise)
    pub fn annuler(&self) {
        if let Some(t) = &self.telechargement {
            t.annuler();
        }
    }

    /// Attend la fin du téléchargement en cours, au plus `delai` (vrai s'il n'y en a pas, ou s'il est fini)
    pub fn attendre(&mut self, delai: Duration) -> bool {
        self.telechargement.as_mut().is_none_or(|t| t.attendre(delai))
    }

    /// Où en est le téléchargement (pour le pourcentage précis, ou les tests)
    pub fn avancement(&self) -> Option<crate::telechargement::Avancement> {
        self.telechargement.as_ref().map(|t| t.avancement())
    }
}
