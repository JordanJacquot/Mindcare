//! Les pages branchées sur le programme (étape 5.3 du plan « Mindcare en Rust ») : les messages du kit vont aux pages
//! (`pages::Etat::agir`), et leurs effets sont exécutés ici, dans l'ordre, sur le fil de la fenêtre.
//!
//! - [`Branchement`] : l'état des pages, le catalogue des enquêtes (absent ou abîmé : la page « Mindcare n'a pas pu lire
//!   ses enquêtes », jamais un plantage), la lecture de Windows et ce qui sort de l'appli (Bureau, presse-papiers) ;
//! - [`LectureWindows`] : ce que Windows a noté (mindcare-windows), lu sur un fil à part dès le démarrage comme
//!   `Start-LectureJournaux` ; une enquête qui l'attend le reçoit dès qu'il est lu, ou au bout de 30 secondes avec ce
//!   qui a pu être lu (`Wait-Journaux`). La fenêtre ne se fige jamais : le programme regarde où en est la lecture à
//!   chaque pas de sa minuterie ([`Branchement::tour`]) ;
//! - [`Exterieur`] : le Bureau et le presse-papiers. Le programme livré donne les vrais (src\windows) ; la
//!   construction d'essai et les tests en donnent des faux, dans leur dossier d'essai (jamais ceux de la personne).
//!
//! - [`Outils`] (étape 6.4) : l'assistant (llama-server), la dictée (le micro et whisper) et leurs deux installations.
//!   Rien ne se fige : la phrase est posée sur un fil à part, la dictée vit sur le sien, les téléchargements aussi ; le
//!   programme regarde où ils en sont à chaque pas de sa minuterie, et en fait une photo pour les pages
//!   ([`pages::Aides`]). Le programme livré donne les vrais ; la construction d'essai, de faux micros et des fichiers
//!   pris sur un faux serveur sur ce PC ; les tests, des faux sans aucun programme lancé.
//!
//! Rien ici ne dépend de Tauri : la page est vue par [`VersLaPage`] (la vraie [`crate::Page`], ou un faux dans les
//! tests).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mindcare_assistant::{Assistant, Question, Tache, decider};
use mindcare_dictee::{Appui, DicteeEnFond, Etat as EtatDictee};
use mindcare_donnees::rapport::enregistrer_rapport;
use mindcare_donnees::{Donnees, IndiceGarde, IndicesGardes, LecturePc, PcDuRapport, SurLePc, infos_pc};
use mindcare_moteur::Catalogue;
use mindcare_telechargement::{Disque, Etat as EtatInstallation, Fin, Installation, Transport};
use mindcare_windows::{Horloge, IndicesWindows, LectureJournaux, PcDetecte, SourceWindows, a_relire, barrettes};
use serde_json::{Value, json};

use crate::pages::{self, Effet};
use crate::pont::{self, MessagePage};

/// La photo des aides est reprise tous les 5 pas de la minuterie (500 ms, comme `Start-SuiviIa`) ; la dictée et la
/// phrase posée sont suivies à chaque pas pendant qu'elles travaillent.
const PAS_PHOTO: u32 = 5;

/// L'assistant, la dictée et leurs installations, tels que l'appli les garde pendant qu'elle tourne.
pub struct Outils {
    pub assistant: Assistant,
    pub installation_assistant: Installation,
    pub dictee: DicteeEnFond,
    pub installation_dictee: Installation,
    /// Le réseau des téléchargements (WinHTTP ; une erreur : aucune installation possible, dite au journal)
    pub transport: Result<Arc<dyn Transport>, String>,
    pub disque: Arc<dyn Disque>,
    /// La phrase posée et sa réponse attendue
    question: Option<(String, Tache)>,
    pas: u32,
    /// « Assistant : une nouvelle version est proposée » est déjà au journal
    maj_notee: bool,
}

impl Outils {
    pub fn nouveaux(
        assistant: Assistant,
        installation_assistant: Installation,
        dictee: DicteeEnFond,
        installation_dictee: Installation,
        transport: Result<Arc<dyn Transport>, String>,
        disque: Arc<dyn Disque>,
    ) -> Outils {
        Outils {
            assistant,
            installation_assistant,
            dictee,
            installation_dictee,
            transport,
            disque,
            question: None,
            pas: 0,
            maj_notee: false,
        }
    }

    /// Une phrase est posée et sa réponse pas encore lue.
    pub fn question_en_cours(&self) -> bool {
        self.question.is_some()
    }

    /// La photo pour les pages : les installations, l'assistant, la dictée (le niveau du son mis à 0 : la page n'est
    /// pas redessinée à chaque son). Une mise à jour proposée est notée au journal, une fois (`Update-BandeauIa`).
    fn photo(&mut self, a: &mut pages::Aides, journal: Option<&mindcare_donnees::Journal>) {
        a.installation_assistant = Some(self.installation_assistant.etat());
        a.assistant = Some(self.assistant.etat());
        a.installation_dictee = Some(self.installation_dictee.etat());
        a.dictee = Some(match self.dictee.etat() {
            EtatDictee::Ecoute { .. } => EtatDictee::Ecoute { niveau: 0.0 },
            e => e,
        });
        if matches!(a.installation_assistant, Some(EtatInstallation::AMettreAJour { .. }))
            && !self.maj_notee
            && let Some(j) = journal
        {
            self.maj_notee = true;
            j.ecrire("Assistant : une nouvelle version est proposée");
        }
    }

    /// Lance une installation (`Start-IaInstall`, `Start-DicteeInstall`) : rien si elle tourne déjà. Rend la ligne du
    /// journal.
    fn installer(&mut self, assistant: bool) -> Option<String> {
        let transport = match &self.transport {
            Ok(t) => Arc::clone(t),
            Err(e) => return Some(format!("Installation impossible : {e}")),
        };
        let (inst, nom) = if assistant {
            (&mut self.installation_assistant, "Installation de l'assistant")
        } else {
            (&mut self.installation_dictee, "Installation de la dictée")
        };
        inst.lancer(transport, self.disque.as_ref()).map(|mo| format!("{nom} ({mo} Mo)"))
    }

    /// Mindcare se ferme (`Stop-Ia`, `Stop-Dictees`) : llama-server arrêté, la dictée arrêtée sans suite (whisper
    /// arrêté, le son effacé), les téléchargements arrêtés (ce qui est commencé reste pour la reprise).
    pub fn arreter(&mut self) {
        self.question = None;
        self.dictee.annuler();
        self.assistant.arreter_serveur();
        self.installation_assistant.annuler();
        self.installation_dictee.annuler();
    }
}

/// `$AttenteJournauxMax` (contenu.ps1) : une enquête n'attend pas Windows plus longtemps.
pub const ATTENTE_WINDOWS_MAX: Duration = Duration::from_secs(30);

/// Ce que l'appli envoie à sa page : un état (avec l'écran où aller), ou un ordre au kit.
pub trait VersLaPage {
    /// Un nouvel état, gardé et envoyé ; `ecran` : y aller tout de suite.
    fn etat(&self, etat: Value, ecran: Option<&str>);
    /// Un ordre au kit (`{ kit: 'message', ... }`).
    fn ordre(&self, ordre: Value);
}

impl VersLaPage for crate::Page {
    fn etat(&self, etat: Value, ecran: Option<&str>) {
        self.changer_et_aller(etat, ecran);
    }
    fn ordre(&self, ordre: Value) {
        crate::Page::ordre(self, ordre);
    }
}

/// Ce qui sort de l'appli : le Bureau (pour le rapport) et le presse-papiers.
pub trait Exterieur {
    /// Le dossier du Bureau de la personne (`[Environment]::GetFolderPath('Desktop')`).
    fn bureau(&self) -> Result<PathBuf, String>;
    /// Met ce texte dans le presse-papiers (`[System.Windows.Clipboard]::SetText`).
    fn copier(&mut self, texte: &str) -> Result<(), String>;
}

/// `Get-Indices` : les indices de Windows tels que l'enquête et l'historique les gardent.
pub fn indices_gardes(i: &IndicesWindows) -> IndicesGardes {
    let mut h = IndicesGardes::default();
    for (k, v) in i {
        h.mettre(
            k,
            IndiceGarde {
                vu: v.vu,
                texte: v.texte.clone(),
                texte_ok: v.texte_ok.clone(),
                affiche: v.affiche,
                partiel: v.partiel,
            },
        );
    }
    h
}

/// Le reste de ce que Windows dit du PC, lu une fois sur un fil à part (`Get-Barrettes`, `Get-InfosPC`).
#[derive(Debug, Clone)]
struct LuPc {
    barrettes: Result<Vec<String>, String>,
    infos: Result<LecturePc, String>,
}

/// Une lecture de Windows finie (ou abandonnée), à donner à l'enquête qui l'attendait.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lu {
    /// Ce que Windows a noté (vide : rien de lu à temps, ou la lecture a raté)
    pub indices: IndicesGardes,
    /// Les lignes du journal de Mindcare
    pub journal: Vec<String>,
}

/// Le PC reconnu, et la raison d'une détection ratée (`mindcare_windows::detecter`).
pub type Detection = (PcDetecte, Option<String>);

/// La lecture de ce que Windows a noté (journaux.ps1), sans jamais bloquer le fil de la fenêtre.
pub struct LectureWindows {
    source: Arc<dyn SourceWindows>,
    horloge: Arc<dyn Horloge>,
    lecture: LectureJournaux,
    lancee: bool,
    delai_max: Duration,
    /// L'enquête attend la lecture depuis cet instant (effet `LireWindows`), et la lecture était-elle déjà finie
    attente: Option<(Instant, bool)>,
    pc: Arc<Mutex<Option<LuPc>>>,
    /// La raison d'une lecture du PC ratée est déjà au journal (barrettes, modèle du PC)
    pc_note: bool,
    infos_note: bool,
    /// Le PC reconnu (`mindcare_windows::detecter`, sur son fil), et la raison d'une détection ratée
    detection: Arc<Mutex<Option<Detection>>>,
}

impl LectureWindows {
    /// Une lecture pas encore lancée (`delai_max` : 30 secondes, sauf dans les tests).
    pub fn nouvelle(source: Arc<dyn SourceWindows>, horloge: Arc<dyn Horloge>, delai_max: Duration) -> LectureWindows {
        LectureWindows {
            lecture: LectureJournaux::nouvelle(Arc::clone(&source), Arc::clone(&horloge)),
            source,
            horloge,
            lancee: false,
            delai_max,
            attente: None,
            pc: Arc::new(Mutex::new(None)),
            pc_note: false,
            infos_note: false,
            detection: Arc::new(Mutex::new(None)),
        }
    }

    /// `Start-LectureJournaux` (au démarrage) : ce que Windows a noté, puis les barrettes et le modèle du PC, chacun
    /// sur son fil ; et la détection du PC (portable ou fixe, marque...) sur un fil de plus, pour trier les pannes.
    pub fn lancer(&mut self) {
        if self.lancee {
            return;
        }
        self.lancee = true;
        self.lecture.lancer();
        let (source, pc) = (Arc::clone(&self.source), Arc::clone(&self.pc));
        let _ = std::thread::Builder::new().name("mindcare-pc".to_string()).spawn(move || {
            let lu = LuPc {
                barrettes: barrettes(source.as_ref()),
                infos: source.infos_pc().map(|i| LecturePc {
                    fabricant: Some(i.fabricant),
                    modele: Some(i.modele),
                    systeme: Some(i.systeme),
                }),
            };
            *pc.lock().unwrap_or_else(|e| e.into_inner()) = Some(lu);
        });
        let (source, detection) = (Arc::clone(&self.source), Arc::clone(&self.detection));
        let _ = std::thread::Builder::new().name("mindcare-detection".to_string()).spawn(move || {
            let lu = mindcare_windows::detecter(source.as_ref());
            *detection.lock().unwrap_or_else(|e| e.into_inner()) = Some(lu);
        });
    }

    /// Le PC reconnu, une fois la détection finie (réussie ou pas, avec sa raison).
    pub fn pc_detecte(&self) -> Option<Detection> {
        self.detection.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// L'enquête a besoin de ce que Windows a noté (effet `LireWindows`) : la lecture est lancée si elle ne l'est pas,
    /// relancée si elle date de plus de 10 minutes (constat 21), et attendue à partir de `maintenant`.
    pub fn demander(&mut self, maintenant: Instant) {
        if self.lancee {
            self.lecture.debut_enquete();
        } else {
            self.lancer();
        }
        self.attente = Some((maintenant, self.lecture.lus()));
    }

    /// Une enquête attend la lecture.
    pub fn attendue(&self) -> bool {
        self.attente.is_some()
    }

    /// Les indices sont trop vieux pour une nouvelle enquête (plus de 10 minutes, constat 21) : elle les redemandera.
    pub fn perimee(&self) -> bool {
        self.lecture.lancee_a().is_some_and(|l| a_relire(l, self.horloge.maintenant()))
    }

    /// Le pas de la minuterie : la lecture attendue est finie, ou attendue depuis trop longtemps (elle est alors
    /// abandonnée : l'enquête continue sans elle, `Stop-AttenteJournaux`). Rien tant qu'il faut encore attendre.
    pub fn verifier(&mut self, maintenant: Instant) -> Option<Lu> {
        let (debut, deja_lue) = self.attente?;
        let ecoule = maintenant.saturating_duration_since(debut);
        if !self.lecture.lus() && ecoule < self.delai_max {
            return None;
        }
        self.attente = None;
        let mut journal = Vec::new();
        if !self.lecture.lus() {
            self.lecture.arreter_attente();
        }
        // (Wait-Journaux ne note l'attente que s'il a attendu)
        if !deja_lue {
            journal.push(format!("Journaux de Windows : attendus {:.1} s", ecoule.as_secs_f64()));
        }
        let indices = match self.lecture.indices() {
            Some(i) => {
                let vus: Vec<&str> = i.iter().filter(|(_, v)| v.vu).map(|(k, _)| k.as_str()).collect();
                journal.push(format!("Journaux de Windows : {}", vus.join(", ")));
                if let Some(j) = i.get("journal") {
                    journal.push(format!("Journaux de Windows : {}", j.texte_ok));
                }
                indices_gardes(&i)
            }
            None => {
                let cause = self.lecture.cause_absence().unwrap_or_else(|| "lecture finie sans résultat".into());
                journal.push(format!("Journaux de Windows non lus : {cause}"));
                IndicesGardes::default()
            }
        };
        Some(Lu { indices, journal })
    }

    fn pc(&self) -> Option<LuPc> {
        self.pc.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Les barrettes de mémoire, si elles sont lues (`Get-Barrettes`). Illisibles : une ligne au journal (une fois).
    pub fn barrettes(&mut self, journal: &mindcare_donnees::Journal) -> Option<Vec<String>> {
        match self.pc()?.barrettes {
            Ok(b) => Some(b),
            Err(e) => {
                if !self.pc_note {
                    self.pc_note = true;
                    journal.ecrire(&format!("Barrettes de mémoire illisibles : {e}"));
                }
                Some(Vec::new())
            }
        }
    }

    /// `Get-InfosPC` : « Fabricant Modèle, Windows » (vide s'il n'est pas encore lu, ou illisible : au journal, une
    /// fois : le rapport est refait à chaque affichage d'une réponse finale).
    pub fn infos(&mut self, journal: &mindcare_donnees::Journal) -> String {
        match self.pc().map(|p| p.infos) {
            Some(Ok(l)) => infos_pc(Ok(&l), journal),
            Some(Err(e)) if !self.infos_note => {
                self.infos_note = true;
                infos_pc(Err(&e), journal)
            }
            Some(Err(_)) | None => String::new(),
        }
    }
}

/// Ce dont les effets ont besoin, le temps d'un message : la page, les données, et le retour au menu de Mindset.
pub struct Monde<'a> {
    pub page: &'a dyn VersLaPage,
    pub donnees: &'a mut Donnees,
    /// `Request-MenuMindset` (seulement dans Mindset) : rend la ligne du journal
    pub menu: &'a mut dyn FnMut() -> String,
}

/// Les pages branchées : leur état, le catalogue, la lecture de Windows et l'extérieur.
pub struct Branchement {
    pub pages: pages::Etat,
    pub cat: Option<Arc<Catalogue>>,
    pub windows: LectureWindows,
    pub exterieur: Box<dyn Exterieur>,
    /// L'assistant et la dictée ([`Branchement::brancher`]) ; sans eux, « Raconte ton problème » est celle de
    /// l'étape 5
    pub outils: Option<Outils>,
    hasard: u64,
    /// Le PC reconnu est déjà donné aux pages (une seule fois)
    pc_donne: bool,
}

/// Le catalogue des enquêtes, lu dans ce fichier (`Read-Enquetes`) : enquetes.json à côté du programme.
pub fn charger_catalogue(fichier: &Path) -> Result<Arc<Catalogue>, String> {
    Catalogue::lire_fichier(fichier).map(Arc::new)
}

/// Un message du kit tel que les pages le lisent (`{ type: 'action', id, valeur, ecran }`...).
pub fn message_kit(m: &MessagePage) -> Value {
    match m {
        MessagePage::Pret => json!({ "type": "pret" }),
        MessagePage::Menu => json!({ "type": "menu" }),
        MessagePage::Ecran { id } => json!({ "type": "ecran", "id": id }),
        MessagePage::Action { id, valeur, ecran } => {
            json!({ "type": "action", "id": id, "valeur": valeur, "ecran": ecran })
        }
        MessagePage::Erreur { texte } => json!({ "type": "erreur", "texte": texte }),
        MessagePage::Reponse { id, valeur } => json!({ "type": "reponse", "id": id, "valeur": valeur }),
    }
}

/// Un nombre au hasard pour l'Id d'une nouvelle enquête (xorshift ; `New-DiagId` prend un nombre de 0 à 65535).
fn hasard(x: &mut u64) -> u16 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    (*x >> 24) as u16
}

impl Branchement {
    /// Les pages au démarrage (l'accueil), avec les enquêtes gardées des données. Un catalogue illisible est noté au
    /// journal : les pages montrent alors seulement le problème.
    pub fn nouveau(
        version: &str,
        theme: &str,
        cat: Result<Arc<Catalogue>, String>,
        donnees: &Donnees,
        windows: LectureWindows,
        exterieur: Box<dyn Exterieur>,
    ) -> Branchement {
        let graine = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
            ^ (u64::from(std::process::id()) << 32);
        if let Err(e) = &cat {
            donnees.journal().ecrire(&format!("Enquêtes illisibles : {e}"));
        }
        let mut pages = pages::Etat::nouveau(version, theme, donnees.historique.liste.clone());
        // (le PC choisi, gardé dans les réglages : « mon PC » ou un autre)
        pages.choix_pc = donnees.choix_pc();
        Branchement { pages, cat: cat.ok(), windows, exterieur, outils: None, hasard: graine | 1, pc_donne: false }
    }

    /// L'assistant et la dictée rejoignent les pages (avec un catalogue lisible seulement : sans lui, la page du
    /// problème n'a rien à en faire). Rien n'est lancé : llama-server attend qu'on écrive, le micro qu'on appuie.
    pub fn brancher(&mut self, mut outils: Outils) {
        if self.cat.is_none() {
            return;
        }
        outils.photo(&mut self.pages.aides, None);
        self.outils = Some(outils);
    }

    /// Mindcare se ferme : l'assistant, la dictée et les téléchargements s'arrêtent.
    pub fn arreter(&mut self) {
        if let Some(o) = self.outils.as_mut() {
            o.arreter();
        }
    }

    /// L'état du kit (sans catalogue : la page « Mindcare n'a pas pu lire ses enquêtes »).
    pub fn etat_kit(&self) -> Value {
        match &self.cat {
            Some(cat) => self.pages.etat_kit(cat, None),
            None => pages::etat_sans_catalogue(&self.pages.version, &self.pages.theme),
        }
    }

    /// Un message du kit : noté au journal, appliqué aux pages, puis leurs effets exécutés.
    pub fn recevoir(&mut self, m: &MessagePage, monde: &mut Monde) {
        // (« menu » : la ligne vient du retour lui-même, comme avant)
        if *m != MessagePage::Menu {
            monde.donnees.journal().ecrire(&pont::ligne_journal(m));
        }
        let Some(cat) = self.cat.clone() else {
            // (sans catalogue : la page du problème, et le retour à Mindset ; rien d'autre n'a de sens)
            let effets = match m {
                MessagePage::Pret => vec![Effet::Afficher { ecran: None }],
                MessagePage::Menu => vec![Effet::MenuMindset],
                _ => Vec::new(),
            };
            self.executer(effets, monde);
            return;
        };
        self.noter_indices_perimes();
        let message = message_kit(m);
        let journal = monde.donnees.journal().clone();
        let mut graine = self.hasard;
        let effets = self
            .pages
            .agir(&cat, &message, &mut || mindcare_donnees::nouvel_id(&journal.maintenant(), hasard(&mut graine)));
        self.hasard = graine;
        self.executer(effets, monde);
    }

    /// Constat 21 : les pages savent, avant chaque message et chaque réponse de l'assistant, si ce que Windows a
    /// noté date de plus de 10 minutes (sans lecture attendue). Toute enquête neuve qui part de là (commencée par
    /// mots, par familles, par l'assistant, par « Pour être sûr », ou reprise) le redemande alors.
    fn noter_indices_perimes(&mut self) {
        self.pages.indices_perimes = self.windows.perimee() && !self.windows.attendue();
    }

    /// La réponse de l'assistant, décidée, donnée aux pages (l'enquête peut commencer tout de suite).
    fn decision_assistant(
        &mut self,
        cat: &Catalogue,
        d: mindcare_assistant::Decision,
        journal: &mindcare_donnees::Journal,
    ) -> Vec<Effet> {
        self.noter_indices_perimes();
        let mut graine = self.hasard;
        let effets = self.pages.decision_assistant(cat, d, &mut || {
            mindcare_donnees::nouvel_id(&journal.maintenant(), hasard(&mut graine))
        });
        self.hasard = graine;
        effets
    }

    /// Le pas de la minuterie : une lecture de Windows attendue et finie (ou trop longue) fait avancer l'enquête ;
    /// des barrettes lues entre-temps rejoignent l'état.
    pub fn tour(&mut self, maintenant: Instant, monde: &mut Monde) {
        let effets = self.pc_fini(monde.donnees);
        if !effets.is_empty() {
            self.executer(effets, monde);
        }
        let effets = self.windows_fini(maintenant, monde.donnees);
        if !effets.is_empty() {
            self.executer(effets, monde);
        } else if self.pages.barrettes.is_empty()
            && let Some(b) = self.windows.barrettes(monde.donnees.journal()).filter(|b| !b.is_empty())
        {
            self.pages.barrettes = b;
        }
        let effets = self.tour_aides(monde);
        if !effets.is_empty() {
            self.executer(effets, monde);
        }
    }

    /// Le pas de la minuterie pour l'assistant et la dictée : le journal de la dictée, la fin d'une installation
    /// (`Start-SuiviIa`, `Start-SuiviDictee`), la phrase dictée, la réponse de l'assistant (`$script:IaAttente`), puis
    /// la photo : la page est redessinée seulement si quelque chose a changé.
    fn tour_aides(&mut self, monde: &mut Monde) -> Vec<Effet> {
        let journal = monde.donnees.journal().clone();
        let Some(cat) = self.cat.clone() else { return Vec::new() };
        let Some(o) = self.outils.as_mut() else { return Vec::new() };
        o.pas = o.pas.wrapping_add(1);
        let mut effets = Vec::new();
        let mut photo = self.pages.aides.dictee_occupee()
            || o.installation_assistant.en_cours()
            || o.installation_dictee.en_cours()
            || o.pas % PAS_PHOTO == 0;
        for l in o.dictee.prendre_journal() {
            journal.ecrire(&l);
            photo = true;
        }
        let (fin_assistant, fin_dictee) = (o.installation_assistant.fin(), o.installation_dictee.fin());
        photo |= fin_assistant.is_some() || fin_dictee.is_some();
        match fin_assistant {
            Some(Fin::Installe) => {
                // (l'ancien llama-server peut encore lire l'ancien modèle : arrêté d'abord, relancé à la prochaine
                // phrase, puis les anciens modèles effacés)
                journal.ecrire("Assistant installé");
                o.assistant.arreter_serveur();
                for l in o.installation_assistant.paquet.effacer_anciens() {
                    journal.ecrire(&l);
                }
            }
            Some(Fin::Erreur(e)) => journal.ecrire(&format!("Assistant non installé : {e}")),
            Some(Fin::Annule) | None => {}
        }
        match fin_dictee {
            Some(Fin::Installe) => {
                journal.ecrire("Dictée installée");
                self.pages.aides.message_dictee = Some(pages::textes_aides::DICTEE_PRETE.to_string());
            }
            Some(Fin::Erreur(e)) => journal.ecrire(&format!("Dictée non installée : {e}")),
            Some(Fin::Annule) | None => {}
        }
        if let Some(phrase) = o.dictee.prendre_phrase() {
            effets.extend(self.pages.phrase_dictee(phrase));
        }
        let reponse = o.question.as_ref().and_then(|(phrase, t)| t.reponse().map(|r| (phrase.clone(), r)));
        if let Some((phrase, r)) = reponse {
            o.question = None;
            monde.page.ordre(json!({ "kit": "occupe", "texte": "" }));
            let d = decider(&cat, &phrase, &r, &|l| journal.ecrire(l));
            effets.extend(self.decision_assistant(&cat, d, &journal));
        }
        let Some(o) = self.outils.as_mut() else { return effets };
        // (la photo touche au disque : tous les 5 pas, ou tout de suite quand quelque chose bouge)
        if !photo && effets.is_empty() {
            return effets;
        }
        let avant = self.pages.aides.clone();
        o.photo(&mut self.pages.aides, Some(&journal));
        if self.pages.aides != avant && !effets.iter().any(|e| matches!(e, Effet::Afficher { .. })) {
            effets.push(Effet::Afficher { ecran: None });
        }
        effets
    }

    /// La détection du PC est finie (une seule fois) : les pages trient les pannes pour lui. Rien sinon.
    fn pc_fini(&mut self, donnees: &Donnees) -> Vec<Effet> {
        if self.pc_donne || self.cat.is_none() {
            return Vec::new();
        }
        let Some((pc, erreur)) = self.windows.pc_detecte() else { return Vec::new() };
        self.pc_donne = true;
        let profil = pc.profil();
        donnees.journal().ecrire(&match &erreur {
            Some(e) => format!("PC non reconnu : {e}"),
            None if profil.est_inconnu() => "PC non reconnu : rien de sûr".to_string(),
            None => format!("PC reconnu : {}", profil.libelle_court()),
        });
        self.pages.pc_reconnu(profil, pc.fiche())
    }

    /// La lecture de Windows attendue est finie (ou abandonnée) : l'enquête qui l'attendait avance. Rien sinon.
    fn windows_fini(&mut self, maintenant: Instant, donnees: &Donnees) -> Vec<Effet> {
        let Some(cat) = self.cat.clone() else { return Vec::new() };
        let Some(lu) = self.windows.verifier(maintenant) else { return Vec::new() };
        for l in &lu.journal {
            donnees.journal().ecrire(l);
        }
        let b = self.windows.barrettes(donnees.journal()).unwrap_or_else(|| self.pages.barrettes.clone());
        self.pages.windows_lu(&cat, lu.indices, b)
    }

    /// Le rapport de la réponse montrée (`Get-ResumeEnCours`).
    fn rapport(&mut self, cat: &Catalogue, donnees: &Donnees) -> Option<String> {
        let journal = donnees.journal();
        let infos = self.windows.infos(journal);
        if self.pages.barrettes.is_empty()
            && let Some(b) = self.windows.barrettes(journal)
        {
            self.pages.barrettes = b;
        }
        let pc = SurLePc {
            infos: &infos,
            barrettes: &self.pages.barrettes,
            indices_du_jour: self.pages.indices_du_jour.as_ref(),
        };
        // (la fiche du PC reconnu, écrite pour une enquête faite sur lui ; l'autre PC choisi, pour une autre)
        let choisi = match &self.pages.choix_pc {
            mindcare_donnees::ChoixPc::Autre(p) => Some(p),
            mindcare_donnees::ChoixPc::MonPc => None,
        };
        let detail = PcDuRapport { fiche: self.pages.pc.as_ref().map(|p| &p.fiche), choisi };
        self.pages.en_cours.rapport_pc(cat, &journal.maintenant(), &pc, &detail, &self.pages.version)
    }

    /// Exécute les effets, dans l'ordre ; ceux que rend un résultat (rapport enregistré...) passent avant la suite.
    pub fn executer(&mut self, effets: Vec<Effet>, monde: &mut Monde) {
        let mut file: VecDeque<Effet> = effets.into();
        while let Some(e) = file.pop_front() {
            let suite = self.effet(e, monde);
            for x in suite.into_iter().rev() {
                file.push_front(x);
            }
        }
    }

    fn effet(&mut self, e: Effet, monde: &mut Monde) -> Vec<Effet> {
        let journal = monde.donnees.journal().clone();
        let cat = self.cat.clone();
        match e {
            Effet::Afficher { ecran } => {
                // (le rapport montré dans « Pour le réparateur » : refait tant qu'il n'est ni enregistré ni copié)
                if let Some(cat) = cat.as_deref()
                    && self.pages.rapport_a_refaire(cat)
                {
                    self.pages.rapport = self.rapport(cat, monde.donnees);
                }
                // (l'assistant et la dictée tels qu'ils sont à cet instant)
                if let Some(o) = self.outils.as_mut() {
                    o.photo(&mut self.pages.aides, Some(&journal));
                }
                monde.page.etat(self.etat_kit(), ecran.as_deref());
                Vec::new()
            }
            Effet::Message { texte, bravo } => {
                monde.page.ordre(json!({ "kit": "message", "texte": texte, "bravo": bravo }));
                Vec::new()
            }
            Effet::Journal(l) => {
                journal.ecrire(&l);
                Vec::new()
            }
            Effet::EnregistrerEnquete => {
                if let Some(cat) = cat.as_deref() {
                    if let Some((e, jour)) = self.pages.enquete_a_enregistrer(cat) {
                        self.pages.en_cours.enregistrer(cat, monde.donnees, e, &jour);
                    }
                    self.pages.historique = monde.donnees.historique.liste.clone();
                }
                Vec::new()
            }
            Effet::LireWindows => {
                // (déjà lue : l'enquête avance tout de suite, sans écran d'attente, comme Wait-Journaux)
                let maintenant = Instant::now();
                self.windows.demander(maintenant);
                self.windows_fini(maintenant, monde.donnees)
            }
            Effet::RapportBureau => {
                let Some(cat) = cat.as_deref() else { return Vec::new() };
                let texte = self.rapport(cat, monde.donnees);
                self.pages.rapport.clone_from(&texte);
                let r = texte.ok_or_else(|| "aucune réponse montrée".to_string()).and_then(|t| {
                    let bureau = self.exterieur.bureau()?;
                    enregistrer_rapport(&bureau, &t, &journal)
                        .map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                        .map_err(|e| e.to_string())
                });
                self.pages.rapport_enregistre(r)
            }
            Effet::CopierRapport => {
                let Some(cat) = cat.as_deref() else { return Vec::new() };
                let texte = self.rapport(cat, monde.donnees);
                self.pages.rapport.clone_from(&texte);
                let r =
                    texte.ok_or_else(|| "aucune réponse montrée".to_string()).and_then(|t| self.exterieur.copier(&t));
                self.pages.rapport_copie(r)
            }
            Effet::CopierCommande { numero, texte } => {
                let r = self.exterieur.copier(&texte);
                self.pages.commande_copiee(numero, r)
            }
            Effet::MenuMindset => {
                journal.ecrire(&(monde.menu)());
                Vec::new()
            }
            Effet::Inconnue(x) => {
                journal.ecrire(&format!("Écran : inconnu ({})", x.chars().take(120).collect::<String>()));
                Vec::new()
            }
            Effet::DemarrerAssistant => {
                if let Some(o) = &self.outils {
                    o.assistant.demarrer();
                }
                Vec::new()
            }
            Effet::Demander { phrase } => {
                let (Some(o), Some(cat)) = (self.outils.as_mut(), cat) else {
                    return self.pages.question_impossible(false);
                };
                let permises: Vec<String> = cat
                    .pannes()
                    .iter()
                    .map(|p| mindcare_moteur::ps::texte(mindcare_moteur::ps::champ(p, "Id")))
                    .collect();
                match o.assistant.poser(&phrase, &permises) {
                    Question::SansAssistant => self.pages.question_impossible(false),
                    Question::NeDemarrePas { .. } => self.pages.question_impossible(true),
                    Question::EnCours(t) => {
                        o.question = Some((phrase, t));
                        monde.page.ordre(json!({ "kit": "occupe", "texte": pages::textes_aides::LIT }));
                        Vec::new()
                    }
                }
            }
            Effet::OublierQuestion => {
                if let Some(o) = self.outils.as_mut()
                    && o.question.take().is_some()
                {
                    monde.page.ordre(json!({ "kit": "occupe", "texte": "" }));
                }
                Vec::new()
            }
            Effet::InstallerAssistant | Effet::InstallerDictee => {
                let assistant = e == Effet::InstallerAssistant;
                if let Some(o) = self.outils.as_mut()
                    && let Some(l) = o.installer(assistant)
                {
                    journal.ecrire(&l);
                }
                Vec::new()
            }
            Effet::Parler => {
                let Some(o) = &self.outils else { return Vec::new() };
                let mut suite = Vec::new();
                match o.dictee.appui() {
                    // (`Invoke-BoutonMicro` : l'installation proposée ; la ligne de la dictée a son bouton)
                    Appui::Installer => {
                        journal.ecrire("Dictée : installation proposée");
                        suite
                            .push(Effet::Message { texte: pages::textes_aides::INSTALLE_D_ABORD.into(), bravo: false });
                    }
                    Appui::Ecoute | Appui::Fin | Appui::Occupee | Appui::Erreur(_) => {}
                }
                self.pages.aides.message_dictee = None;
                for l in o.dictee.prendre_journal() {
                    journal.ecrire(&l);
                }
                suite
            }
            Effet::AnnulerDictee => {
                if let Some(o) = &self.outils {
                    o.dictee.annuler();
                }
                Vec::new()
            }
            Effet::EffacerMessageDictee => {
                if let Some(o) = &self.outils {
                    o.dictee.effacer_message();
                }
                Vec::new()
            }
            Effet::ChoisirPc(choix) => {
                monde.donnees.choisir_pc(&choix);
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests;
