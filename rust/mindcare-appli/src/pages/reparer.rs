//! « Le faire pour moi » dans les pages (la maquette validée par Jordan) : un test de la liste fermée (champ
//! « Reparer ») montre d'abord le bouton, et ses gestes repliés dessous (« Ou le faire toi-même »). Le bouton porte
//! lui-même la question « sur ce PC » (« Si le problème est sur ce PC, Mindcare peut le faire pour toi ») : le clic
//! vaut un oui, sans rien changer à l'enquête (aucune note « cepc » ajoutée). Pendant : une jauge et les étapes, les
//! réponses attendent la fin ; un clic de plus ne relance rien. Fini : « Fait par Mindcare », le geste qui reste pour
//! la personne, puis les réponses. Raté : « Mindcare n'a pas pu le faire », et les gestes dépliés.
//!
//! Pas de bouton quand la personne a déjà dit que le problème n'est pas sur ce PC (la question « Sur ce PC ? » posée,
//! et répondue sans la note « cepc »), quand « Ton PC » est réglé sur un autre PC, dans Mindcare Mobile, pour test-sfc
//! et test-gaming-services, et pour une manipulation sur le matériel : rien ne change. Les 4 actions qui demandent les
//! droits administrateur ont leur bouton seulement quand l'Aide de Mindcare est là ([`Etat::aide_administrateur_la`] :
//! sa tâche SYSTEM et MindcareAide.exe) ; sinon leur page reste celle d'aujourd'hui, avec les gestes à faire soi-même.
//! Quand l'Aide n'a pas pu le faire, sa phrase est montrée telle quelle, au-dessus des gestes dépliés.

use serde_json::{Value, json};

use mindcare_moteur::Enquete;
use mindcare_moteur::ps::{champ, egal, texte};

use super::actions::ids;
use super::outils::{gestes_materiel, textes as textes_de};
use super::{Effet, Etat};
use crate::reparer::Reparation;

/// Les textes communs.
pub mod textes {
    pub const PEUT: &str = "Si le problème est sur ce PC, Mindcare peut le faire pour toi";
    pub const BOUTON: &str = "Le faire pour moi";
    pub const SOI_MEME: &str = "Ou le faire toi-même";
    pub const PENDANT: &str = "Mindcare s'en occupe...";
    pub const PENDANT_NOTE: &str = "Laisse Mindcare ouvert. Tu pourras répondre dès que ce sera fini.";
    pub const FAIT: &str = "Fait par Mindcare";
    pub const RESTE_NOTE: &str = "Le seul geste qui reste pour toi";
    pub const RATE: &str = "Mindcare n'a pas pu le faire";
    pub const RATE_RIEN: &str = "Rien n'a été changé sur ton PC";
    pub const RATE_PARTIE: &str = "Mindcare n'a fait qu'une partie";
    pub const RATE_TEXTE: &str = "Tu peux le faire toi-même avec les gestes juste en dessous.";
    pub const COCHE: &str = "Coche chaque geste une fois fait";
    pub const WINDOWS_UPDATE: &str = "Ouvrir Windows Update";
    pub const WINDOWS_UPDATE_RATE: &str =
        "Mindcare n'a pas pu ouvrir Windows Update. Ouvre les Paramètres (touche Windows + I), puis Windows Update.";
}
use textes as x;

impl Etat {
    /// L'Aide de Mindcare est-elle là (les 4 actions qui demandent les droits administrateur) ? Sa tâche SYSTEM pour ce
    /// compte et MindcareAide.exe, lus par effets.rs au démarrage. Tant qu'elle répond non, ces tests restent comme
    /// aujourd'hui, sans bouton (jamais de demande de Windows à la place).
    pub fn aide_administrateur_la(&self) -> bool {
        self.aide_admin
    }
}

/// Ce qui reste à faire pour la personne une fois l'action faite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reste {
    /// Relancer la recherche des mises à jour : le bouton « Ouvrir Windows Update »
    WindowsUpdate,
    /// Un geste simple, en une ligne
    Geste(&'static str),
}

/// Les textes d'une action, pour des gens pas à l'aise avec un PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextesAction {
    /// Sous « Si le problème est sur ce PC, Mindcare peut le faire pour toi »
    pub note: &'static str,
    /// Les étapes montrées pendant (une par programme lancé, dans l'ordre)
    pub etapes: &'static [&'static str],
    /// Le titre et la ligne de « Fait par Mindcare »
    pub fait: &'static str,
    pub fait_texte: &'static str,
    pub reste: Reste,
}

const UN_CLIC: &str = "Un clic, aucune fenêtre à ouvrir. Quelques secondes.";
const INSTALLER: &str =
    "Un clic, aucune fenêtre à ouvrir. Quelques minutes. Windows peut te demander une autorisation : accepte.";

/// Les textes de chaque action.
pub fn textes_action(r: Reparation) -> TextesAction {
    let t = |note, etapes, fait, fait_texte, reste| TextesAction { note, etapes, fait, fait_texte, reste };
    match r {
        Reparation::Dns => t(
            UN_CLIC,
            &["Vidage du carnet d'adresses d'internet"],
            "Le carnet d'adresses d'internet est vidé",
            "Le PC va redemander les bonnes adresses des sites.",
            Reste::Geste("Ferme le navigateur, rouvre-le, et réessaie le site."),
        ),
        Reparation::CacheStore => t(
            "Un clic. Le Microsoft Store va se rouvrir tout seul. Environ une minute.",
            &["Vidage du cache du Microsoft Store"],
            "Le cache du Microsoft Store est vidé",
            "Le Store se rouvre tout seul.",
            Reste::Geste("Réessaie dans le Store ce qui bloquait."),
        ),
        Reparation::CacheIcones => t(
            UN_CLIC,
            &["Remise à neuf des images des icônes"],
            "Les images des icônes sont refaites",
            "Windows les redessine tout de suite.",
            Reste::Geste("Si les icônes restent blanches : redémarre le PC."),
        ),
        Reparation::VisualCpp => t(
            INSTALLER,
            &["Installation des fichiers Visual C++ (64 bits)", "Installation des fichiers Visual C++ (32 bits)"],
            "Les fichiers Visual C++ sont installés",
            "Les deux versions sont en place sur ton PC.",
            Reste::Geste("Redémarre le PC, puis relance le programme."),
        ),
        Reparation::Java => t(
            INSTALLER,
            &["Installation de Java"],
            "Java est installé",
            "Les programmes qui en ont besoin peuvent s'en servir.",
            Reste::Geste("Relance le programme (ou double-clique sur le fichier .jar)."),
        ),
        Reparation::Parametres => t(
            "Un clic, aucune fenêtre à ouvrir. Environ une minute.",
            &["Remise à zéro de l'appli Paramètres"],
            "L'appli Paramètres est remise à zéro",
            "Elle repart comme au premier jour.",
            Reste::Geste("Redémarre le PC, puis ouvre les Paramètres (touche Windows + I)."),
        ),
        Reparation::ProfilWifi => t(
            "Un clic. Le PC oublie les Wi-Fi enregistrés et se déconnecte : c'est normal. Il faudra retaper le mot de passe de ton Wi-Fi : prépare-le avant (il est souvent écrit sous ta box).",
            &["Oubli des Wi-Fi enregistrés"],
            "Le PC a oublié les Wi-Fi enregistrés",
            "Il est déconnecté : c'est normal.",
            Reste::Geste(
                "Reconnecte-toi : clique sur l'icône du Wi-Fi en bas à droite, choisis ton Wi-Fi et tape son mot de passe.",
            ),
        ),
        Reparation::AdresseIp => t(
            "Un clic. Internet se coupe un instant : c'est normal.",
            &["Rendu de l'ancienne adresse", "Demande d'une nouvelle adresse à la box"],
            "Le PC a une nouvelle adresse",
            "La box lui en a donné une neuve.",
            Reste::Geste("Réessaie internet."),
        ),
        Reparation::Alimentation => t(
            UN_CLIC,
            &["Retour aux réglages d'alimentation d'origine"],
            "Les réglages d'alimentation sont ceux d'origine",
            "Les réglages changés à la main sont effacés.",
            Reste::Geste("Essaie de nouveau, pendant quelques jours."),
        ),
        Reparation::MisesAJour => t(
            "Un clic, aucune fenêtre à ouvrir. Environ une minute.",
            &[
                "Arrêt du service des mises à jour",
                "Mise de côté des mises à jour téléchargées",
                "Redémarrage du service des mises à jour",
            ],
            "Les mises à jour téléchargées sont mises de côté",
            "Windows va les retélécharger proprement.",
            Reste::WindowsUpdate,
        ),
    }
}

/// Où en est l'action lancée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatReparation {
    /// En cours : les étapes déjà faites
    EnCours {
        faits: usize,
    },
    Faite,
    /// Ratée après ces étapes faites
    Ratee {
        faits: usize,
    },
}

/// L'action lancée : pour quelle enquête et quelle étape, laquelle, où elle en est.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiviReparation {
    pub diag_id: String,
    pub etape: String,
    pub action: Reparation,
    pub etat: EtatReparation,
    /// Ratée par l'Aide de Mindcare : sa phrase, montrée telle quelle (les autres échecs vont seulement au journal)
    pub message: Option<String>,
}

impl SuiviReparation {
    pub fn en_cours(&self) -> bool {
        matches!(self.etat, EtatReparation::EnCours { .. })
    }
}

/// L'Id de l'étape montrée.
fn id_etape(e: &Enquete) -> String {
    e.etape.as_ref().map(|et| texte(champ(et, "Id"))).unwrap_or_default()
}

impl Etat {
    /// L'action que Mindcare peut faire pour cette étape : un test de la liste fermée, sans geste sur le matériel, et
    /// sans droits administrateur tant que l'Aide n'est pas là. Jamais quand la personne a déjà dit que le problème
    /// est ailleurs (« Sur ce PC ? » répondue sans la note « cepc ») ni quand « Ton PC » vise un autre PC.
    pub(crate) fn reparation_possible(&self, e: &Enquete) -> Option<Reparation> {
        let et = e.etape.as_ref()?;
        let cepc_posee = e.faits.iter().any(|f| egal(&texte(champ(&f.etape, "Id")), "cepc"));
        let ailleurs = cepc_posee && !e.notes.iter().any(|n| egal(n, "cepc"));
        if ailleurs || self.choix_pc.pour_un_autre() || gestes_materiel(&textes_de(et, "Gestes")) {
            return None;
        }
        Reparation::de_l_etape(et).filter(|r| !r.admin() || self.aide_administrateur_la())
    }

    /// L'action lancée pour cette étape-ci de cette enquête-ci (rien pour une autre).
    pub(crate) fn suivi_ici(&self, e: &Enquete) -> Option<&SuiviReparation> {
        let id = id_etape(e);
        self.reparation.as_ref().filter(|s| egal(&s.diag_id, &self.en_cours.diag_id) && egal(&s.etape, &id))
    }

    /// Une autre enquête (ou l'accueil) : l'action terminée est oubliée. Dans la même enquête, elle reste : revenir sur
    /// son étape montre ce qu'elle a donné (jamais le bouton une seconde fois). Celle qui tourne encore est toujours
    /// gardée : aucune autre ne peut partir avant sa fin.
    pub(super) fn oublier_reparation_finie(&mut self) {
        if !self.reparation.as_ref().is_some_and(SuiviReparation::en_cours) {
            self.reparation = None;
        }
    }

    /// Les réponses attendent la fin de l'action de cette étape.
    pub(super) fn reponses_en_attente(&self, e: &Enquete) -> bool {
        self.suivi_ici(e).is_some_and(SuiviReparation::en_cours)
    }

    /// « Le faire pour moi » : l'action part (une seule fois ; un clic de plus, ou pendant une autre, ne relance rien).
    pub(super) fn faire_pour_moi(&mut self, e: &Enquete) -> Vec<Effet> {
        let deja = self.reparation.as_ref().is_some_and(SuiviReparation::en_cours) || self.suivi_ici(e).is_some();
        let Some(r) = self.reparation_possible(e).filter(|_| !deja) else { return vec![Effet::rafraichir()] };
        let etape = id_etape(e);
        self.reparation = Some(SuiviReparation {
            diag_id: self.en_cours.diag_id.clone(),
            etape: etape.clone(),
            action: r,
            etat: EtatReparation::EnCours { faits: 0 },
            message: None,
        });
        vec![
            Effet::Journal(format!("Le faire pour moi : {} ({etape})", r.id())),
            Effet::Reparer(r),
            Effet::rafraichir(),
        ]
    }

    /// « Ouvrir Windows Update », une fois les mises à jour mises de côté.
    pub(super) fn ouvrir_windows_update(&mut self, e: &Enquete) -> Vec<Effet> {
        let ok = self
            .suivi_ici(e)
            .is_some_and(|s| s.etat == EtatReparation::Faite && textes_action(s.action).reste == Reste::WindowsUpdate);
        if ok { vec![Effet::OuvrirWindowsUpdate] } else { vec![Effet::rafraichir()] }
    }

    /// L'action lancée a fait une étape de plus.
    pub fn reparation_avance(&mut self, faits: usize) -> Vec<Effet> {
        match self.reparation.as_mut() {
            Some(s) if s.en_cours() => {
                s.etat = EtatReparation::EnCours { faits };
                vec![Effet::rafraichir()]
            }
            _ => Vec::new(),
        }
    }

    /// L'action lancée est finie : faite, ou ratée (pourquoi, pour le journal).
    pub fn reparation_finie(&mut self, resultat: Result<(), String>) -> Vec<Effet> {
        let ligne = match &resultat {
            Ok(()) => "Le faire pour moi : fait".to_string(),
            Err(e) => format!("Le faire pour moi : raté ({e})"),
        };
        let mut effets = vec![Effet::Journal(ligne)];
        if let Some(s) = self.reparation.as_mut().filter(|s| s.en_cours()) {
            let EtatReparation::EnCours { faits } = s.etat else { unreachable!() };
            s.etat = if resultat.is_ok() { EtatReparation::Faite } else { EtatReparation::Ratee { faits } };
            // (la phrase de l'Aide est écrite pour la personne ; celle d'un programme lancé par Mindcare, non)
            s.message = resultat.err().filter(|_| s.action.admin());
            effets.push(Effet::rafraichir());
        }
        effets
    }

    /// La page Windows Update s'est ouverte, ou pas (une bulle dit alors comment l'ouvrir).
    pub fn windows_update_ouvert(&mut self, resultat: Result<(), String>) -> Vec<Effet> {
        match resultat {
            Ok(()) => vec![Effet::Journal("Windows Update ouvert".into())],
            Err(e) => vec![
                Effet::Journal(format!("Windows Update non ouvert : {e}")),
                Effet::message(x::WINDOWS_UPDATE_RATE, false),
            ],
        }
    }
}

/// Les blocs d'un test avec « Le faire pour moi » : `manuels` sont les gestes et les commandes d'aujourd'hui. Rend
/// les blocs, et si les réponses se montrent (pas pendant l'action).
pub(super) fn blocs_test(
    possible: Option<Reparation>,
    suivi: Option<&SuiviReparation>,
    manuels: Vec<Value>,
    nb_gestes: usize,
) -> (Vec<Value>, bool) {
    let coche = || json!({ "type": "titre", "texte": x::COCHE });
    match (possible, suivi) {
        (_, Some(s)) => {
            let t = textes_action(s.action);
            match s.etat {
                EtatReparation::EnCours { faits } => {
                    let total = t.etapes.len().max(1);
                    let valeur = ((faits.min(total) * 2 + 1) * 100 / (total * 2)).min(100);
                    let elements: Vec<Value> = t
                        .etapes
                        .iter()
                        .enumerate()
                        .map(
                            |(k, e)| {
                                if k < faits { json!({ "texte": e, "fait": true }) } else { json!({ "texte": e }) }
                            },
                        )
                        .collect();
                    let blocs = vec![
                        json!({ "type": "jauge", "texte": x::PENDANT, "valeur": valeur }),
                        json!({ "type": "etapes", "elements": elements }),
                        json!({ "type": "note", "texte": x::PENDANT_NOTE }),
                    ];
                    (blocs, false)
                }
                EtatReparation::Faite => {
                    let mut blocs = vec![json!({ "type": "verdict", "compact": true, "icone": "ok", "couleur": "vert",
                                                 "surtitre": x::FAIT, "titre": t.fait, "texte": t.fait_texte })];
                    blocs.push(match t.reste {
                        Reste::WindowsUpdate => json!({ "type": "ligne", "icone": "actu", "couleur": "P",
                            "texte": "Relance la recherche des mises à jour", "note": x::RESTE_NOTE,
                            "bouton": { "id": ids::WINDOWS_UPDATE, "texte": x::WINDOWS_UPDATE, "style": "second" } }),
                        Reste::Geste(g) => {
                            json!({ "type": "ligne", "icone": "actu", "couleur": "P", "texte": g, "note": x::RESTE_NOTE })
                        }
                    });
                    (blocs, true)
                }
                EtatReparation::Ratee { faits } => {
                    // (ratée par l'Aide : sa phrase, telle quelle, dit ce qui s'est passé et quoi faire)
                    let titre = match &s.message {
                        Some(m) => m.as_str(),
                        None if faits == 0 => x::RATE_RIEN,
                        None => x::RATE_PARTIE,
                    };
                    let mut blocs = vec![
                        json!({ "type": "verdict", "compact": true, "icone": "alerte",
                                                 "couleur": "orange", "surtitre": x::RATE, "titre": titre,
                                                 "texte": x::RATE_TEXTE }),
                        coche(),
                    ];
                    blocs.extend(manuels);
                    (blocs, true)
                }
            }
        }
        (Some(r), None) => {
            let resume = if nb_gestes == 1 { "1 geste".to_string() } else { format!("{nb_gestes} gestes") };
            let blocs = vec![
                json!({ "type": "ligne", "icone": "eclair", "couleur": "P", "texte": x::PEUT,
                        "note": textes_action(r).note, "bouton": { "id": ids::FAIRE, "texte": x::BOUTON } }),
                json!({ "type": "repli", "titre": x::SOI_MEME, "resume": resume, "blocs": manuels }),
            ];
            (blocs, true)
        }
        (None, None) => {
            let mut blocs = vec![coche()];
            blocs.extend(manuels);
            (blocs, true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_etape_affichee_par_programme_lance() {
        for r in Reparation::TOUTES {
            let t = textes_action(r);
            assert!(!t.etapes.is_empty() && !t.note.is_empty() && !t.fait.is_empty() && !t.fait_texte.is_empty());
            if !r.admin() {
                assert_eq!(t.etapes.len(), crate::reparer::gestes(r).len(), "{r:?}");
            }
        }
        assert_eq!(textes_action(Reparation::MisesAJour).reste, Reste::WindowsUpdate);
        // (sans Aide trouvée, au départ : non)
        assert!(!Etat::default().aide_administrateur_la());
        // Le Wi-Fi : la personne est prévenue AVANT qu'il faudra retaper le mot de passe (aucune copie n'est gardée)
        let wifi = textes_action(Reparation::ProfilWifi);
        assert!(wifi.note.contains("retaper le mot de passe de ton Wi-Fi") && wifi.note.contains("sous ta box"));
        // Aucun tiret long dans les textes (règle de la suite)
        for r in Reparation::TOUTES {
            let t = textes_action(r);
            for s in [t.note, t.fait, t.fait_texte].into_iter().chain(t.etapes.iter().copied()) {
                assert!(!s.contains(['\u{2013}', '\u{2014}']), "{s}");
            }
        }
    }
}
