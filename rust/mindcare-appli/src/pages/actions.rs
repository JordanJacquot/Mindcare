//! Les messages du kit appliqués à l'état : `{ type: 'action', id, valeur, ecran }`, `{ type: 'menu' }`,
//! `{ type: 'ecran', id }` et `{ type: 'pret' }`. Chaque action rend la liste des effets à exécuter.

use serde_json::Value;

use mindcare_moteur::Catalogue;
use mindcare_moteur::ps::{champ, champ_v, egal, tableau, texte, vrai};

use super::accueil::{DERNIERE, REPRENDRE};
use super::familles::FAMILLE;
use super::raconte::{PANNE, RECHERCHE};
use super::{Effet, Etat, Vue, ecrans};

/// Les ids des actions envoyées par les pages.
pub mod ids {
    /// Une réponse à l'étape (valeur : son numéro)
    pub const REP: &str = "rep";
    /// « Je ne sais pas »
    pub const NSP: &str = "nsp";
    /// « Étape précédente », « Changer ma dernière réponse »
    pub const PRECEDENT: &str = "precedent";
    /// Changer une réponse déjà donnée (« changer:2 »)
    pub const CHANGER: &str = "changer:";
    /// Un geste coché ou décoché (valeur : { valeur: son numéro, oui })
    pub const GESTE: &str = "geste";
    /// Copier une commande (valeur : son numéro)
    pub const COMMANDE: &str = "commande";
    /// « Je le ferai plus tard »
    pub const PLUS_TARD: &str = "plus-tard";
    /// « Accueil », « Retour à l'accueil »
    pub const ACCUEIL: &str = "accueil";
    /// Le bouton B sur une page de l'enquête (`retour: 'appli'`)
    pub const RETOUR: &str = "retour";
    /// « Pour le réparateur »
    pub const REPARATEUR: &str = "reparateur";
    /// « Enregistrer le rapport sur le Bureau »
    pub const BUREAU: &str = "bureau";
    /// « Copier le rapport »
    pub const COPIER: &str = "copier";
}

/// Le message en bulle quand un test est remis à plus tard.
pub const PLUS_TARD_MESSAGE: &str = "Mindcare s'en souviendra : l'enquête t'attend à l'accueil.";

/// Un nombre envoyé par le kit (« 2 », ou 2).
fn entier(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

impl Etat {
    /// Applique un message du kit, et rend ce qu'il faut faire ensuite. `nouvel_id` donne l'Id d'une nouvelle enquête
    /// (`mindcare_donnees::nouvel_id` avec l'heure et un nombre au hasard) : appelé seulement quand une enquête
    /// commence.
    pub fn agir(&mut self, cat: &Catalogue, message: &Value, nouvel_id: &mut dyn FnMut() -> String) -> Vec<Effet> {
        match texte(message.get("type")).as_str() {
            "menu" => vec![Effet::MenuMindset],
            "pret" => vec![Effet::rafraichir()],
            "ecran" => {
                self.ecran = texte(message.get("id"));
                Vec::new()
            }
            "action" => {
                let id = texte(message.get("id"));
                let ecran = texte(message.get("ecran"));
                self.action(cat, &id, message.get("valeur"), &ecran, nouvel_id)
            }
            autre => vec![Effet::Inconnue(format!("message « {autre} »"))],
        }
    }

    fn action(
        &mut self,
        cat: &Catalogue,
        id: &str,
        valeur: Option<&Value>,
        ecran: &str,
        nouvel_id: &mut dyn FnMut() -> String,
    ) -> Vec<Effet> {
        if let Some(p) = id.strip_prefix(PANNE) {
            return self.commencer(cat, p, nouvel_id);
        }
        if let Some(f) = id.strip_prefix(FAMILLE) {
            if cat.famille(f).is_none() {
                return vec![Effet::afficher(ecrans::FAMILLES)];
            }
            self.famille = Some(f.to_string());
            return vec![Effet::afficher(ecrans::FAMILLE)];
        }
        if let Some(k) = id.strip_prefix(ids::CHANGER) {
            // Changer une réponse : on revient à cette question
            let Ok(k) = k.parse::<usize>() else { return vec![Effet::Inconnue(id.to_string())] };
            if !matches!(self.vue(cat), Some(Vue::Etape(_) | Vue::Reponse(_))) {
                return vec![Effet::rafraichir()];
            }
            self.en_cours.choix.truncate(k);
            return self.avancer(cat);
        }
        match id {
            ecrans::RACONTE | ecrans::FAMILLES | ecrans::AIDE => vec![Effet::afficher(id)],
            RECHERCHE => {
                self.recherche = texte(valeur);
                vec![Effet::rafraichir()]
            }
            REPRENDRE | DERNIERE => self.reprendre(cat),
            ids::REP | ids::NSP => {
                let n = if id == ids::NSP { Some(-1) } else { entier(valeur) };
                let Some(Vue::Etape(e)) = self.vue(cat) else { return vec![Effet::rafraichir()] };
                let nb = e.etape.as_ref().map(|et| tableau(champ(et, "Reponses")).len()).unwrap_or(0) as i64;
                match n {
                    Some(n) if n == -1 || (0..nb).contains(&n) => {
                        self.en_cours.choix.push(n);
                        self.avancer(cat)
                    }
                    _ => vec![Effet::rafraichir()],
                }
            }
            ids::PRECEDENT => self.precedent(cat),
            // (B sur une page de l'enquête : comme « Étape précédente » ; ailleurs, rien à faire)
            ids::RETOUR if ecran == ecrans::ENQUETE => self.precedent(cat),
            ids::RETOUR => vec![Effet::rafraichir()],
            ids::GESTE => {
                let Some(Vue::Etape(e)) = self.vue(cat) else { return vec![Effet::rafraichir()] };
                let etape = e.etape.as_ref().map(|et| texte(champ(et, "Id"))).unwrap_or_default();
                let (k, oui) = match valeur {
                    Some(Value::Object(_)) => (entier(champ_v(valeur, "valeur")), vrai(champ_v(valeur, "oui"))),
                    _ => (None, false),
                };
                if let Some(k) = k.and_then(|k| usize::try_from(k).ok()) {
                    self.gestes.retain(|(i, x)| !(*x == k && egal(i, &etape)));
                    if oui {
                        self.gestes.push((etape, k));
                    }
                }
                vec![Effet::rafraichir()]
            }
            ids::COMMANDE => {
                let Some(Vue::Etape(e)) = self.vue(cat) else { return vec![Effet::rafraichir()] };
                let lignes: Vec<String> = e
                    .etape
                    .as_ref()
                    .and_then(|et| champ(et, "Commande"))
                    .map(|c| mindcare_moteur::ps::parcours(champ_v(Some(c), "Lignes")))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|x| texte(Some(x)))
                    .collect();
                match entier(valeur).and_then(|k| usize::try_from(k).ok()).filter(|k| *k < lignes.len()) {
                    Some(k) => vec![Effet::CopierCommande { numero: k, texte: lignes[k].clone() }],
                    None => vec![Effet::rafraichir()],
                }
            }
            ids::PLUS_TARD => {
                self.retour_accueil();
                vec![
                    Effet::Journal("Test remis à plus tard".into()),
                    Effet::message(PLUS_TARD_MESSAGE, false),
                    Effet::afficher(ecrans::ACCUEIL),
                ]
            }
            ids::ACCUEIL => {
                self.retour_accueil();
                vec![Effet::afficher(ecrans::ACCUEIL)]
            }
            ids::REPARATEUR => match self.vue(cat) {
                Some(Vue::Reponse(_)) => vec![Effet::afficher(ecrans::REPARATEUR)],
                _ => vec![Effet::rafraichir()],
            },
            ids::BUREAU | ids::COPIER => match self.vue(cat) {
                Some(Vue::Reponse(_)) => {
                    vec![if id == ids::BUREAU { Effet::RapportBureau } else { Effet::CopierRapport }]
                }
                _ => vec![Effet::rafraichir()],
            },
            _ => vec![Effet::Inconnue(id.to_string())],
        }
    }

    /// `Start-Diagnostic` : une nouvelle enquête sur cette panne.
    fn commencer(&mut self, cat: &Catalogue, panne: &str, nouvel_id: &mut dyn FnMut() -> String) -> Vec<Effet> {
        if cat.panne(panne).is_none() {
            self.retour_accueil();
            return vec![Effet::afficher(ecrans::ACCUEIL)];
        }
        let diag_id = nouvel_id();
        self.nouvelle_page_enquete();
        let Some(ligne) = self.en_cours.preparer(cat, panne, None, diag_id) else {
            self.retour_accueil();
            return vec![Effet::afficher(ecrans::ACCUEIL)];
        };
        let mut effets = vec![Effet::Journal(ligne)];
        effets.extend(self.avancer(cat));
        effets
    }

    /// `Open-Diagnostic` : la dernière enquête gardée, là où on en était (son étape, ou sa réponse).
    fn reprendre(&mut self, cat: &Catalogue) -> Vec<Effet> {
        let d = self.historique.first().cloned();
        let id = d.as_ref().map(|d| texte(champ(d, "Id"))).unwrap_or_default();
        self.nouvelle_page_enquete();
        match self.en_cours.preparer_reprise(cat, &id, d.as_ref()) {
            Ok(ligne) => {
                let mut effets = vec![Effet::Journal(ligne)];
                effets.extend(self.avancer(cat));
                effets
            }
            Err(ligne) => {
                self.retour_accueil();
                vec![Effet::Journal(ligne), Effet::afficher(ecrans::ACCUEIL)]
            }
        }
    }

    /// `Undo-Choix` : la réponse d'avant ; à la première question, la famille de la panne.
    fn precedent(&mut self, cat: &Catalogue) -> Vec<Effet> {
        let Some(panne) = self.en_cours.panne.as_ref() else { return vec![Effet::afficher(ecrans::ACCUEIL)] };
        if self.en_cours.choix.is_empty() {
            let famille = texte(champ(panne, "Famille"));
            self.retour_accueil();
            if cat.famille(&famille).is_some() {
                self.famille = Some(famille);
                return vec![Effet::afficher(ecrans::FAMILLE)];
            }
            return vec![Effet::afficher(ecrans::ACCUEIL)];
        }
        self.en_cours.choix.pop();
        self.avancer(cat)
    }
}
