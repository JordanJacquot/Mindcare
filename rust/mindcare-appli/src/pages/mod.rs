//! Les pages de Mindcare, construites en Rust pour le kit d'écrans de la suite (kit.js) : sans fenêtre, sans Tauri,
//! sans disque, testables (étape 5.2 du plan « Mindcare en Rust »).
//!
//! Le programme garde un [`Etat`] (l'écran, la recherche, la famille choisie, l'enquête en cours de mindcare-donnees,
//! les gestes cochés, les enquêtes gardées, ce que Windows a noté...). Deux choses seulement :
//! - [`Etat::etat_kit`] construit l'état envoyé au kit (`kit.etat({...})` : app, theme, accueil, pages), comme
//!   `envoyer()` de la maquette validée ;
//! - [`Etat::agir`] applique un message reçu du kit (`{ type: 'action', id, valeur, ecran }`, `{ type: 'menu' }`...)
//!   et rend la liste des [`Effet`]s à exécuter, dans l'ordre (enregistrer l'enquête, lire Windows, copier le
//!   rapport...). Rien n'est exécuté ici : c'est le travail de programme.rs, qui rappelle ensuite l'état avec le
//!   résultat ([`Etat::windows_lu`], [`Etat::rapport_enregistre`], [`Etat::rapport_copie`],
//!   [`Etat::commande_copiee`]).
//!
//! Les règles et les textes sont ceux de Mindcare PowerShell (modules\contenu.ps1) : l'enquête suit le moteur
//! (mindcare-moteur) et la reprise par Id de mindcare-donnees (`EnqueteEnCours::avancer`), jamais un moteur à part.
//!
//! Les étapes automatiques (ce que Windows a noté) : comme `Wait-Journaux`, sur le PC en panne (réponse « cepc »)
//! l'enquête attend la lecture de Windows avant d'avancer. La page « enquete » devient alors une page d'attente
//! (« Mindcare regarde ce que Windows a noté... », avec Étape précédente et Accueil) et l'effet [`Effet::LireWindows`]
//! est rendu une fois ; quand la lecture est finie (ou abandonnée au bout de 30 secondes, comme le PowerShell),
//! programme.rs appelle [`Etat::windows_lu`], qui fait avancer l'enquête si elle attendait toujours.

mod accueil;
mod actions;
mod aides;
mod enquete;
mod familles;
#[cfg(test)]
mod juges;
mod outils;
mod raconte;
mod reparateur;
mod reparer;
mod reponse;
#[cfg(test)]
mod tests;
mod ton_pc;

/// Les ids des actions des pages, et ceux de l'accueil (reprendre l'enquête en cours, revoir la dernière).
pub use accueil::{DERNIERE, REPRENDRE};
pub use actions::{PLUS_TARD_MESSAGE, ids};
/// L'assistant et la dictée dans les pages (étape 6.4) : leur photo, leurs ids d'action, leurs textes.
pub use aides::{Aides, POUR_ETRE_SUR, PourEtreSur, ids as ids_aides, textes as textes_aides};
/// Le début de l'id d'une action qui commence une enquête (`panne:<id>`).
pub use raconte::PANNE;
/// « Le faire pour moi » : où en est l'action lancée, et ses textes.
pub use reparer::{EtatReparation, Reste, SuiviReparation, TextesAction, textes as textes_reparer, textes_action};
/// Le PC reconnu, et les textes de l'écran « Ton PC ».
pub use ton_pc::{PcReconnu, textes as textes_pc};

use serde_json::{Map, Value, json};

use mindcare_donnees::{Avance, ChoixPc, EnqueteEnCours, IndicesGardes};
use mindcare_moteur::{Catalogue, Enquete, Objet, TypePc};

/// L'id de chaque page (et de l'écran où aller).
pub mod ecrans {
    pub const ACCUEIL: &str = "accueil";
    pub const RACONTE: &str = "raconte";
    pub const FAMILLES: &str = "familles";
    pub const FAMILLE: &str = "famille";
    pub const ENQUETE: &str = "enquete";
    pub const REPARATEUR: &str = "reparateur";
    pub const AIDE: &str = "aide";
    /// « Ton PC » (la pastille de la barre du haut), et le choix de la marque d'un autre PC
    pub const TON_PC: &str = "tonpc";
    pub const TON_PC_MARQUE: &str = "tonpc-marque";
    /// Seulement quand le catalogue est illisible ([`super::etat_sans_catalogue`])
    pub const PROBLEME: &str = "probleme";
}

/// Le logo de Mindcare, dans le dossier ui.
pub const LOGO: &str = "logo.svg";

/// Ce que le programme doit faire après un message du kit, dans l'ordre de la liste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effet {
    /// Envoyer l'état au kit ([`Etat::etat_kit`]), en allant sur cet écran s'il est donné.
    Afficher { ecran: Option<String> },
    /// Un message en bulle (`{ kit: 'message', texte, bravo }`).
    Message { texte: String, bravo: bool },
    /// Une ligne du journal de Mindcare.
    Journal(String),
    /// Enregistrer l'enquête en cours, telle que l'état la montre (une étape, ou la réponse finale) :
    /// `let (e, jour) = etat.enquete_a_enregistrer(&cat)` puis `etat.en_cours.enregistrer(&cat, &mut donnees, e,
    /// &jour)`, puis recopier les enquêtes gardées dans l'état (`etat.historique = donnees.historique.liste.clone()`).
    /// Pour une réponse finale, cela écrit aussi au journal la réponse et prépare le rapport (`resume_de`).
    EnregistrerEnquete,
    /// Lire ce que Windows a noté sur ce PC (en arrière-plan, sans figer la fenêtre), et les barrettes de mémoire ;
    /// une fois lu, ou au bout de 30 secondes : [`Etat::windows_lu`]. Rendu une seule fois par lecture attendue.
    LireWindows,
    /// Enregistrer le rapport sur le Bureau (`etat.en_cours.rapport(...)`, rangé aussi dans [`Etat::rapport`] : la
    /// page montre le texte enregistré, `rapport::enregistrer_rapport`) ; puis [`Etat::rapport_enregistre`] avec le
    /// nom du fichier ou l'erreur. Le fichier ne s'ouvre jamais dans un autre programme.
    RapportBureau,
    /// Copier le rapport dans le presse-papiers (rangé aussi dans [`Etat::rapport`]) ; puis [`Etat::rapport_copie`].
    CopierRapport,
    /// Copier une commande à taper dans le Terminal (Mindcare ne la lance jamais) ; puis [`Etat::commande_copiee`].
    CopierCommande { numero: usize, texte: String },
    /// Retour depuis l'accueil (`{ type: 'menu' }`) : dans Mindset, retour à la console.
    MenuMindset,
    /// Un message ou une action que les pages n'ont jamais envoyé (noté au journal, rien d'autre).
    Inconnue(String),
    /// On écrit dans la recherche : l'assistant se prépare (`Start-Ia` : llama-server lancé s'il ne tourne pas, sans
    /// attendre la phrase).
    DemarrerAssistant,
    /// « Demander à l'assistant » (Entrée) : la phrase posée sur un fil à part (`Start-IaQuestion`), la fenêtre ne se
    /// fige pas ; la réponse est attendue par la minuterie, puis donnée à [`Etat::decision_assistant`]. Sans
    /// assistant : [`Etat::question_impossible`].
    Demander { phrase: String },
    /// La phrase posée n'intéresse plus (on a écrit autre chose, ou quitté la page) : sa réponse sera laissée.
    OublierQuestion,
    /// Installer (réparer, mettre à jour, réessayer) l'assistant : un seul téléchargement à la fois (`Start-IaInstall`).
    InstallerAssistant,
    /// Installer (réparer, réessayer) la dictée (`Start-DicteeInstall`).
    InstallerDictee,
    /// Le bouton Parler (`Start-Dictee`) : écouter, ou arrêter d'écouter.
    Parler,
    /// On a quitté « Raconte ton problème » : la dictée s'arrête sans suite (`Stop-DicteeSansSuite`).
    AnnulerDictee,
    /// Le message de la dictée est lu (`$script:DicteeMessage = $null`).
    EffacerMessageDictee,
    /// Garder le PC choisi dans les réglages (`Donnees::choisir_pc` : un autre PC, ou « Revenir à mon PC »).
    ChoisirPc(ChoixPc),
    /// « Le faire pour moi » : l'action de la liste fermée, déroulée sur un fil à part (jamais deux à la fois) ; à
    /// chaque étape faite, [`Etat::reparation_avance`], puis à la fin [`Etat::reparation_finie`].
    Reparer(crate::reparer::Reparation),
    /// « Ouvrir Windows Update » (la page des Paramètres) ; puis [`Etat::windows_update_ouvert`].
    OuvrirWindowsUpdate,
}

impl Effet {
    fn afficher(ecran: &str) -> Effet {
        Effet::Afficher { ecran: Some(ecran.to_string()) }
    }
    fn rafraichir() -> Effet {
        Effet::Afficher { ecran: None }
    }
    fn message(texte: &str, bravo: bool) -> Effet {
        Effet::Message { texte: texte.to_string(), bravo }
    }
}

/// Ce que dit le dernier essai de rapport (enregistré, copié, ou raté), montré sous ses boutons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoRapport {
    pub texte: String,
    pub reussi: bool,
}

/// L'état de l'appli, tout ce dont les pages ont besoin.
#[derive(Debug, Clone, Default)]
pub struct Etat {
    /// La version de Mindcare (montrée dans la barre)
    pub version: String,
    /// Le thème du kit (`neon`, `crepuscule`...)
    pub theme: String,
    /// Ce que programme.rs ajoute tel quel à l'état du kit (pack d'un thème perso, couleurs, sons, connexion...)
    pub habillage: Map<String, Value>,
    /// Le texte de « Raconte ton problème »
    pub recherche: String,
    /// La famille ouverte (page « famille »)
    pub famille: Option<String>,
    /// L'enquête en cours (aucune panne : pas d'enquête), avec sa reprise par Id (mindcare-donnees)
    pub en_cours: EnqueteEnCours,
    /// Les gestes cochés : (Id de l'étape, numéro du geste)
    pub gestes: Vec<(String, usize)>,
    /// La commande copiée (son numéro, et si la copie a marché)
    pub commande: Option<(usize, bool)>,
    /// Ce que Windows a noté aujourd'hui (`None` : pas encore lu)
    pub indices_du_jour: Option<IndicesGardes>,
    /// Ce que Windows a noté date de plus de 10 minutes et aucune lecture n'est attendue (constat 21) : posé par
    /// effets.rs avant chaque message et chaque réponse de l'assistant. Une enquête neuve (commencée ou reprise)
    /// oublie alors `indices_du_jour` et redemande la lecture.
    pub indices_perimes: bool,
    /// La lecture de Windows est demandée (effet [`Effet::LireWindows`] déjà rendu)
    pub windows_demande: bool,
    /// Les barrettes de mémoire de ce PC (« 8 Go Kingston... »), lues avec Windows
    pub barrettes: Vec<String>,
    /// Les enquêtes gardées (diagnostics.json), la plus récente d'abord : recopiées par programme.rs
    pub historique: Vec<Objet>,
    /// Le rapport a été copié
    pub copie: bool,
    /// Le dernier essai d'enregistrer ou de copier le rapport
    pub info_rapport: Option<InfoRapport>,
    /// Le texte du rapport montré dans « Pour le réparateur » (`Get-ResumeEnCours`), préparé par programme.rs avant
    /// chaque affichage d'une réponse finale ([`Etat::rapport_a_refaire`]) ; une fois enregistré ou copié, c'est le
    /// texte enregistré ou copié, gardé tel quel. `None` : pas encore prêt.
    pub rapport: Option<String>,
    /// Le dernier écran dit par le kit (`{ type: 'ecran', id }`)
    pub ecran: String,
    /// L'assistant et la dictée (étape 6.4) : leur photo, prise par effets.rs
    pub aides: Aides,
    /// Le PC reconnu par Mindcare (la détection, lue sur un fil à part au démarrage) ; `None` : pas encore reconnu
    /// (rien n'est caché, pas de pastille).
    pub pc: Option<PcReconnu>,
    /// Le PC dont on regarde les pannes (gardé dans les réglages, recopié ici par effets.rs)
    pub choix_pc: ChoixPc,
    /// « Tout afficher » choisi dans la bande : rien n'est caché, pour cette fois seulement (jamais gardé)
    pub tout_afficher: bool,
    /// Le genre de l'autre PC choisi, en attendant sa marque (`Some(None)` : je ne sais pas)
    pub genre_choisi: Option<Option<TypePc>>,
    /// L'écran d'où « Ton PC » a été ouvert (Retour y ramène)
    pub avant_pc: Option<String>,
    /// « Le faire pour moi » : l'action lancée (une seule à la fois), gardée tant qu'elle tourne
    pub reparation: Option<SuiviReparation>,
    /// L'Aide de Mindcare est là (sa tâche SYSTEM et MindcareAide.exe, lus par effets.rs) : les 4 actions qui demandent
    /// les droits administrateur ont alors leur bouton. Non au départ.
    pub aide_admin: bool,
}

/// Où en est l'enquête montrée.
pub(crate) enum Vue {
    /// Elle attend ce que Windows a noté
    Attente,
    /// Une étape à faire
    Etape(Box<Enquete>),
    /// La réponse finale
    Reponse(Box<Enquete>),
}

impl Etat {
    /// Un état neuf : l'accueil, aucune enquête en cours.
    pub fn nouveau(version: &str, theme: &str, historique: Vec<Objet>) -> Etat {
        Etat {
            version: version.to_string(),
            theme: theme.to_string(),
            historique,
            ecran: ecrans::ACCUEIL.to_string(),
            ..Etat::default()
        }
    }

    /// L'état à envoyer au kit (`kit.etat(...)`), avec l'écran où aller (`None` : rester où l'on est).
    pub fn etat_kit(&self, cat: &Catalogue, ecran: Option<&str>) -> Value {
        let mut pages = Map::new();
        pages.insert(ecrans::RACONTE.into(), raconte::page(self, cat));
        pages.insert(ecrans::FAMILLES.into(), familles::page_familles(self, cat));
        if let Some(p) = self.famille.as_deref().and_then(|f| familles::page_famille(self, cat, f)) {
            pages.insert(ecrans::FAMILLE.into(), p);
        }
        match self.vue(cat) {
            Some(Vue::Attente) => {
                pages.insert(ecrans::ENQUETE.into(), enquete::page_attente(self, cat));
            }
            Some(Vue::Etape(e)) => {
                pages.insert(ecrans::ENQUETE.into(), enquete::page(self, cat, &e));
            }
            Some(Vue::Reponse(e)) => {
                pages.insert(ecrans::ENQUETE.into(), reponse::page(self, cat, &e));
                pages.insert(ecrans::REPARATEUR.into(), reparateur::page(self, cat));
            }
            None => {}
        }
        if let Some(p) = &self.aides.pour_etre_sur {
            pages.insert(POUR_ETRE_SUR.into(), aides::page_pour_etre_sur(p));
        }
        pages.insert(ecrans::AIDE.into(), accueil::page_aide());
        let pastille = self.libelle_pastille();
        if pastille.is_some() {
            // (Retour ramène à l'écran d'où « Ton PC » a été ouvert, s'il est toujours là)
            let parent = self.avant_pc.as_deref().filter(|e| pages.contains_key(*e)).map(str::to_string);
            pages.insert(ecrans::TON_PC.into(), ton_pc::page(self, parent.as_deref()));
            if let Some(g) = self.genre_choisi {
                pages.insert(ecrans::TON_PC_MARQUE.into(), ton_pc::page_marque(g));
            }
        }
        let mut etat = json!({
            "app": { "nom": crate::NOM_APPLI, "version": self.version, "logo": LOGO },
            "theme": self.theme,
            "accueil": accueil::accueil(self, cat),
            "pages": pages,
        });
        if let Value::Object(o) = &mut etat {
            for (k, v) in &self.habillage {
                if !o.contains_key(k) {
                    o.insert(k.clone(), v.clone());
                }
            }
            if let Some(pc) = pastille.as_deref() {
                o.insert("pc".into(), pastille_pc(pc));
            }
            if let Some(e) = ecran {
                o.insert("ecran".into(), json!(e));
            }
        }
        etat
    }

    /// L'enquête montrée : son étape, sa réponse finale, ou l'attente de Windows (rien sans enquête en cours).
    pub(crate) fn vue(&self, cat: &Catalogue) -> Option<Vue> {
        let jour = self.indices_du_jour.as_ref();
        let e = self.en_cours.enquete_avec(cat, jour)?;
        if self.en_cours.attend_windows(&e, jour) {
            return Some(Vue::Attente);
        }
        Some(if e.etape.is_some() { Vue::Etape(Box::new(e)) } else { Vue::Reponse(Box::new(e)) })
    }

    /// Pour l'effet [`Effet::EnregistrerEnquete`] : l'enquête montrée et ce que Windows a noté aujourd'hui (vide s'il
    /// n'est pas lu : l'enquête n'en a alors pas besoin). Rien sans enquête prête (aucune, ou elle attend Windows).
    pub fn enquete_a_enregistrer(&self, cat: &Catalogue) -> Option<(Enquete, IndicesGardes)> {
        match self.vue(cat)? {
            Vue::Attente => None,
            Vue::Etape(e) | Vue::Reponse(e) => Some((*e, self.indices_du_jour.clone().unwrap_or_default())),
        }
    }

    /// `Show-Accueil` : plus d'enquête en cours, ni de famille choisie (la recherche tapée reste).
    fn retour_accueil(&mut self) {
        self.en_cours = EnqueteEnCours::default();
        self.famille = None;
        self.nouvelle_page_enquete();
    }

    /// Ce qui appartient à une seule enquête montrée (gestes, commande, rapport).
    fn nouvelle_page_enquete(&mut self) {
        self.gestes.clear();
        self.commande = None;
        self.oublier_reparation_finie();
        self.nouveau_rapport();
    }

    /// Le rapport d'une autre réponse : à refaire, pas encore enregistré ni copié.
    fn nouveau_rapport(&mut self) {
        self.copie = false;
        self.info_rapport = None;
        self.rapport = None;
    }

    /// Le rapport est à (re)faire avant d'envoyer l'état au kit : une réponse finale est montrée, et son rapport n'a
    /// pas encore été enregistré ni copié (ensuite, la page garde le texte enregistré ou copié). programme.rs le
    /// construit alors (`en_cours.rapport(...)`, avec la date et ce que Windows dit du PC) et le range dans
    /// [`Etat::rapport`].
    pub fn rapport_a_refaire(&self, cat: &Catalogue) -> bool {
        (self.rapport.is_none() || self.info_rapport.is_none()) && matches!(self.vue(cat), Some(Vue::Reponse(_)))
    }

    /// `Show-Etape` : l'enquête avance (réponses déjà connues données toutes seules), puis elle est enregistrée et
    /// montrée, ou elle attend ce que Windows a noté.
    fn avancer(&mut self, cat: &Catalogue) -> Vec<Effet> {
        self.commande = None;
        self.nouveau_rapport();
        match self.en_cours.avancer(cat, self.indices_du_jour.as_ref()) {
            None => {
                self.retour_accueil();
                vec![Effet::afficher(ecrans::ACCUEIL)]
            }
            Some(Avance::Windows) => {
                let mut effets = Vec::new();
                if !self.windows_demande {
                    self.windows_demande = true;
                    effets.push(Effet::LireWindows);
                }
                effets.push(Effet::afficher(ecrans::ENQUETE));
                effets
            }
            Some(Avance::Prete(_)) => vec![Effet::EnregistrerEnquete, Effet::afficher(ecrans::ENQUETE)],
        }
    }

    /// Ce que Windows a noté est lu (ou la lecture est abandonnée : `indices` avec seulement ce qui a pu être lu,
    /// « journal » dit pourquoi), avec les barrettes de mémoire. Une enquête qui l'attendait avance.
    pub fn windows_lu(&mut self, cat: &Catalogue, indices: IndicesGardes, barrettes: Vec<String>) -> Vec<Effet> {
        let attendait = matches!(self.vue(cat), Some(Vue::Attente));
        self.indices_du_jour = Some(indices);
        self.barrettes = barrettes;
        self.windows_demande = false;
        if attendait {
            let mut effets = self.avancer(cat);
            // (rester sur l'écran où l'on est : la personne a pu aller ailleurs entre-temps)
            for e in &mut effets {
                if let Effet::Afficher { ecran } = e {
                    *ecran = None;
                }
            }
            return effets;
        }
        vec![Effet::rafraichir()]
    }

    /// Le rapport a été enregistré sur le Bureau (le nom de son fichier), ou pas (l'erreur). Comme Add-Reparateur,
    /// le résultat est dit dans la page seulement (pas de bulle en plus, qui recouvrait le titre).
    pub fn rapport_enregistre(&mut self, resultat: Result<String, String>) -> Vec<Effet> {
        use mindcare_donnees::rapport::reparateur as r;
        match resultat {
            Ok(nom) => {
                let texte = r::enregistre(&nom);
                self.info_rapport = Some(InfoRapport { texte, reussi: true });
                vec![Effet::rafraichir()]
            }
            Err(erreur) => {
                self.info_rapport = Some(InfoRapport { texte: r::ECHEC_ENREGISTRER.into(), reussi: false });
                vec![Effet::Journal(r::journal_echec_enregistrer(&erreur)), Effet::rafraichir()]
            }
        }
    }

    /// Le rapport a été copié, ou pas (l'erreur) : dit dans la page seulement, comme l'enregistrement.
    pub fn rapport_copie(&mut self, resultat: Result<(), String>) -> Vec<Effet> {
        use mindcare_donnees::rapport::reparateur as r;
        match resultat {
            Ok(()) => {
                self.copie = true;
                self.info_rapport = Some(InfoRapport { texte: r::COPIE.into(), reussi: true });
                vec![Effet::rafraichir()]
            }
            Err(erreur) => {
                self.info_rapport = Some(InfoRapport { texte: r::ECHEC_COPIE.into(), reussi: false });
                vec![Effet::Journal(r::journal_echec_copie(&erreur)), Effet::rafraichir()]
            }
        }
    }

    /// Une commande a été copiée (son numéro), ou pas (l'erreur) : son bouton dit « Copiée » ou « Copie impossible ».
    pub fn commande_copiee(&mut self, numero: usize, resultat: Result<(), String>) -> Vec<Effet> {
        self.commande = Some((numero, resultat.is_ok()));
        let mut effets = Vec::new();
        if let Err(e) = resultat {
            effets.push(Effet::Journal(format!("Commande non copiée : {e}")));
        }
        effets.push(Effet::rafraichir());
        effets
    }
}

/// La pastille « Ton PC » (`pc` de kit.js) : son action, son libellé, et l'icône d'un portable ou d'un PC de bureau.
fn pastille_pc(libelle: &str) -> Value {
    let icone = if libelle.to_lowercase().contains("portable") { "portable" } else { "pc" };
    json!({ "id": ids::TON_PC, "texte": libelle, "icone": icone })
}

/// L'état du kit quand le catalogue des pannes est illisible (« Mindcare n'a pas pu lire ses enquêtes »).
pub fn etat_sans_catalogue(version: &str, theme: &str) -> Value {
    json!({
        "app": { "nom": crate::NOM_APPLI, "version": version, "logo": LOGO },
        "theme": theme,
        "accueil": {
            "tuiles": [{ "id": ecrans::PROBLEME, "nom": "Mindcare n'a pas pu lire ses enquêtes", "icone": "alerte",
                         "sous": "Réinstalle Mindcare depuis Mindset", "couleur": "rouge" }],
            "ronds": [],
        },
        "pages": {
            ecrans::PROBLEME: {
                "titre": "Mindcare n'a pas pu lire ses enquêtes",
                "blocs": [
                    { "type": "verdict", "icone": "alerte", "couleur": "rouge", "surtitre": "Un fichier manque",
                      "titre": "Son fichier est absent ou abîmé.", "texte": "Réinstalle Mindcare depuis Mindset." },
                ],
            },
        },
    })
}
