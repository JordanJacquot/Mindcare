//! Les messages du kit appliqués à l'état : `{ type: 'action', id, valeur, ecran }`, `{ type: 'menu' }`,
//! `{ type: 'ecran', id }` et `{ type: 'pret' }`. Chaque action rend la liste des effets à exécuter.

use std::cell::RefCell;

use serde_json::Value;

use mindcare_assistant::Decision;
use mindcare_dictee::Etat as EtatDictee;
use mindcare_donnees::PreRempli;
use mindcare_moteur::Catalogue;
use mindcare_moteur::ps::{champ, champ_v, egal, tableau, texte, vrai};

use super::accueil::{DERNIERE, REPRENDRE};
use super::aides::{POUR_ETRE_SUR, PourEtreSur, ids as ia, textes as textes_aides};
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
    /// « Le faire pour moi »
    pub const FAIRE: &str = "faire-pour-moi";
    /// « Ouvrir Windows Update », une fois les mises à jour mises de côté
    pub const WINDOWS_UPDATE: &str = "windows-update";
    /// « Ouvrir Nevermind », « L'installer avec Mindset » (la réponse d'une panne de jeu, dans Mindset)
    pub const OUVRIR: &str = "ouvrir-appli";
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
    /// La pastille « Ton PC » de la barre du haut (l'écran « Ton PC »)
    pub const TON_PC: &str = "ton-pc";
    /// « Tout afficher » dans la bande « Adapté à ton PC » (pour cette fois)
    pub const TOUT_AFFICHER: &str = "tout-afficher";
    /// « Adapter à mon PC » dans la bande, après « Tout afficher »
    pub const ADAPTER: &str = "adapter-pc";
    /// « Revenir à mon PC » dans « Ton PC »
    pub const MON_PC: &str = "pc-mon-pc";
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
                self.ecran_change()
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
        // (l'assistant lit une phrase, et on fait autre chose : sa réponse ne nous emmènera pas ailleurs)
        let garde = matches!(id, RECHERCHE | ia::DEMANDER | ia::PARLER | ia::INSTALLER_IA | ia::INSTALLER_DICTEE);
        let mut effets = Vec::new();
        if self.aides.lit && !garde {
            self.aides.lit = false;
            effets.push(Effet::OublierQuestion);
        }
        effets.extend(self.action_page(cat, id, valeur, ecran, nouvel_id));
        effets
    }

    fn action_page(
        &mut self,
        cat: &Catalogue,
        id: &str,
        valeur: Option<&Value>,
        ecran: &str,
        nouvel_id: &mut dyn FnMut() -> String,
    ) -> Vec<Effet> {
        if let Some(p) = id.strip_prefix(PANNE) {
            return self.commencer(cat, p, None, nouvel_id);
        }
        if let Some(f) = id.strip_prefix(FAMILLE) {
            if cat.famille(f).is_none() {
                return vec![Effet::afficher(ecrans::FAMILLES)];
            }
            self.famille = Some(f.to_string());
            return vec![Effet::afficher(ecrans::FAMILLE)];
        }
        if let Some(effets) = self.action_pc(id, ecran) {
            return effets;
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
            RECHERCHE => self.recherche_tapee(texte(valeur)),
            ia::DEMANDER => self.demander(),
            ia::PARLER => vec![Effet::Parler, Effet::rafraichir()],
            ia::INSTALLER_IA => vec![Effet::InstallerAssistant, Effet::rafraichir()],
            ia::INSTALLER_DICTEE => vec![Effet::InstallerDictee, Effet::rafraichir()],
            ia::IDEE => match entier(valeur) {
                Some(n) => self.pour_etre_sur_choisi(cat, n, nouvel_id),
                None => vec![Effet::Inconnue(format!("{id} sans numéro"))],
            },
            ia::AUCUNE => self.pour_etre_sur_choisi(cat, -1, nouvel_id),
            REPRENDRE | DERNIERE => self.reprendre(cat),
            ids::REP | ids::NSP => {
                let n = if id == ids::NSP { Some(-1) } else { entier(valeur) };
                let Some(Vue::Etape(e)) = self.vue(cat) else { return vec![Effet::rafraichir()] };
                // (les réponses attendent la fin de « Le faire pour moi »)
                if self.reponses_en_attente(&e) {
                    return vec![Effet::rafraichir()];
                }
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
            ids::FAIRE | ids::WINDOWS_UPDATE => {
                let Some(Vue::Etape(e)) = self.vue(cat) else { return vec![Effet::rafraichir()] };
                if id == ids::FAIRE { self.faire_pour_moi(&e) } else { self.ouvrir_windows_update(&e) }
            }
            ids::OUVRIR => self.ouvrir_appli(cat),
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

    /// `Start-Diagnostic` : une nouvelle enquête sur cette panne (`pre_rempli` : les réponses que la phrase donnée à
    /// l'assistant dit déjà).
    fn commencer(
        &mut self,
        cat: &Catalogue,
        panne: &str,
        pre_rempli: Option<&PreRempli>,
        nouvel_id: &mut dyn FnMut() -> String,
    ) -> Vec<Effet> {
        self.aides.pour_etre_sur = None;
        if cat.panne(panne).is_none() {
            self.retour_accueil();
            return vec![Effet::afficher(ecrans::ACCUEIL)];
        }
        let diag_id = nouvel_id();
        self.oublier_indices_perimes();
        self.nouvelle_page_enquete();
        let Some(ligne) = self.en_cours.preparer(cat, panne, pre_rempli, diag_id) else {
            self.retour_accueil();
            return vec![Effet::afficher(ecrans::ACCUEIL)];
        };
        let mut effets = vec![Effet::Journal(ligne)];
        effets.extend(self.avancer(cat));
        effets
    }

    /// Constat 21 : au départ de toute enquête neuve (par mots, par familles, par l'assistant, « Pour être sûr »,
    /// « Reprendre »), ce que Windows a noté il y a plus de 10 minutes est oublié : l'enquête le redemande.
    fn oublier_indices_perimes(&mut self) {
        if std::mem::take(&mut self.indices_perimes) {
            self.indices_du_jour = None;
        }
    }

    /// `Open-Diagnostic` : la dernière enquête gardée, là où on en était (son étape, ou sa réponse).
    fn reprendre(&mut self, cat: &Catalogue) -> Vec<Effet> {
        let d = self.historique.first().cloned();
        let id = d.as_ref().map(|d| texte(champ(d, "Id"))).unwrap_or_default();
        // (une enquête finie revue garde ses propres indices : seule une enquête pas finie relit Windows)
        self.oublier_indices_perimes();
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

    /// Le texte de la recherche envoyé par le kit : pendant qu'on tape (un court instant après la dernière touche), ou
    /// Entrée. Le kit envoie les deux de la même façon ; Entrée renvoie la phrase déjà reçue (tapée puis Entrée, ou
    /// écrite par la dictée puis Entrée) : c'est `Request-Ia`. Une phrase nouvelle : `TextChanged` (l'assistant se
    /// prépare dès qu'on écrit, `Clear-ZoneIa` : son dernier échec et la phrase en cours sont oubliés).
    fn recherche_tapee(&mut self, v: String) -> Vec<Effet> {
        if !self.aides.branchees() {
            self.recherche = v;
            return vec![Effet::rafraichir()];
        }
        if v == self.recherche && !v.trim().is_empty() {
            return self.demander();
        }
        self.recherche = v;
        self.aides.echec = false;
        let mut effets = Vec::new();
        if self.aides.lit {
            self.aides.lit = false;
            effets.push(Effet::OublierQuestion);
        }
        if !self.recherche.trim().is_empty() && self.aides.assistant_installe() {
            effets.push(Effet::DemarrerAssistant);
        }
        effets.push(Effet::rafraichir());
        effets
    }

    /// `Request-Ia` : la phrase de la recherche est posée à l'assistant (rien sans assistant, ni sans phrase ; une
    /// deuxième fois pendant qu'il lit : ignorée). Le message de la dictée est lu.
    fn demander(&mut self) -> Vec<Effet> {
        let mut effets = Vec::new();
        if self.aides.message_dictee.take().is_some() || matches!(self.aides.dictee, Some(EtatDictee::Erreur(_))) {
            effets.push(Effet::EffacerMessageDictee);
        }
        let phrase = self.recherche.trim().to_string();
        if !self.aides.lit && !phrase.is_empty() && self.aides.assistant_installe() {
            self.aides.lit = true;
            self.aides.echec = false;
            effets.push(Effet::Demander { phrase });
        }
        effets.push(Effet::rafraichir());
        effets
    }

    /// Le kit a changé d'écran. En quittant « Raconte ton problème » (l'accueil du PowerShell, `Show-Accueil`) : la
    /// dictée s'arrête sans suite, son message est oublié, et la phrase posée à l'assistant aussi.
    fn ecran_change(&mut self) -> Vec<Effet> {
        if self.ecran == ecrans::RACONTE || !self.aides.branchees() {
            return Vec::new();
        }
        let mut effets = Vec::new();
        if self.aides.dictee_occupee() {
            effets.push(Effet::AnnulerDictee);
        }
        if self.aides.message_dictee.take().is_some() || matches!(self.aides.dictee, Some(EtatDictee::Erreur(_))) {
            effets.push(Effet::EffacerMessageDictee);
        }
        if self.aides.lit {
            self.aides.lit = false;
            effets.push(Effet::OublierQuestion);
        }
        if !effets.is_empty() {
            effets.push(Effet::rafraichir());
        }
        effets
    }

    /// `Select-PourEtreSur` : l'enquête de la phrase choisie (avec ses réponses déjà données), ou « Aucune de ces
    /// phrases » (`n` hors de la liste) : les questions guidées (« Ça concerne quoi ? »).
    fn pour_etre_sur_choisi(&mut self, cat: &Catalogue, n: i64, nouvel_id: &mut dyn FnMut() -> String) -> Vec<Effet> {
        let Some(p) = self.aides.pour_etre_sur.take() else { return vec![Effet::rafraichir()] };
        let lignes = RefCell::new(Vec::new());
        let choisi = mindcare_assistant::choisir(&p.choix, n, &|l| lignes.borrow_mut().push(l.to_string()));
        let mut effets: Vec<Effet> = lignes.into_inner().into_iter().map(Effet::Journal).collect();
        match choisi {
            Some((panne, reponses)) => {
                effets.extend(self.commencer(cat, &panne, Some(&PreRempli(reponses)), nouvel_id))
            }
            None => effets.push(Effet::afficher(ecrans::FAMILLES)),
        }
        effets
    }

    /// La réponse de l'assistant, décidée (`Request-Ia`) : l'enquête commence tout de suite (avec les réponses que la
    /// phrase donne déjà), ou « Pour être sûr », ou le message d'échec (la recherche par mots reste). Une phrase
    /// oubliée entre-temps (on a écrit autre chose, ou quitté la page) : rien.
    pub fn decision_assistant(
        &mut self,
        cat: &Catalogue,
        d: Decision,
        nouvel_id: &mut dyn FnMut() -> String,
    ) -> Vec<Effet> {
        if !self.aides.lit {
            return Vec::new();
        }
        self.aides.lit = false;
        match d {
            Decision::Enquete { panne, reponses } => self.commencer(cat, &panne, Some(&PreRempli(reponses)), nouvel_id),
            Decision::PourEtreSur { phrase, choix } => {
                self.aides.pour_etre_sur = Some(PourEtreSur { phrase, choix });
                vec![Effet::afficher(POUR_ETRE_SUR)]
            }
            Decision::Echec { .. } => {
                self.aides.echec = true;
                vec![Effet::rafraichir()]
            }
        }
    }

    /// La phrase n'a pas pu partir : l'assistant n'est pas installé (rien à dire, la recherche par mots reste), ou il
    /// ne démarre pas (le message d'échec).
    pub fn question_impossible(&mut self, installe: bool) -> Vec<Effet> {
        self.aides.lit = false;
        self.aides.echec = installe;
        vec![Effet::rafraichir()]
    }

    /// La phrase écrite par la dictée (`Set-DicteeTexte`) : dans la recherche, à corriger avant Entrée. Seulement si
    /// « Raconte ton problème » est toujours à l'écran (sinon elle est laissée).
    pub fn phrase_dictee(&mut self, phrase: String) -> Vec<Effet> {
        if self.ecran != ecrans::RACONTE {
            self.aides.message_dictee = None;
            return Vec::new();
        }
        self.recherche = phrase;
        self.aides.echec = false;
        self.aides.message_dictee = Some(textes_aides::DICTEE_COMPRIS.to_string());
        let mut effets = Vec::new();
        if self.aides.lit {
            self.aides.lit = false;
            effets.push(Effet::OublierQuestion);
        }
        if self.aides.assistant_installe() {
            effets.push(Effet::DemarrerAssistant);
        }
        effets.push(Effet::rafraichir());
        effets
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
