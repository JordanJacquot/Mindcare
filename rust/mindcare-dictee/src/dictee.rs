//! La dictée elle-même : `Start-Dictee`, `Step-Dictee`, `Stop-Dictee`, `Start-DicteeEcriture`,
//! `Step-DicteeEcriture`, `Stop-DicteeSansSuite` (modules\dictee.ps1), sans écran. Le micro et le lancement de
//! whisper passent par des traits ([`Materiel`], [`Lanceur`]) : les vrais sont dans le module `windows`, les tests
//! donnent un faux micro (le vrai n'est jamais ouvert) et un faux whisper.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use mindcare_telechargement::Paquet;
use suite_commun::micro::{self, Choix, Trouve};

use crate::nettoyage::nettoyer;
use crate::son::{Mesures, wav};
use crate::whisper;

/// Les messages pour les gens (ceux du PowerShell, sauf « pas de micro », qui y était le même que « micro refusé »)
pub mod messages {
    /// Aucun micro sur le PC
    pub const PAS_DE_MICRO: &str = "Je ne trouve aucun micro. Branche un micro, puis réessaie.";
    /// Windows refuse le micro, ou un autre programme le tient (`Start-Dictee`)
    pub const MICRO_REFUSE: &str = "Je n'ai pas accès au micro. Vérifie qu'il est branché. Dans les paramètres de Windows (Confidentialité, Microphone), les applications de bureau doivent avoir le droit de l'utiliser.";
    /// Rien entendu pendant 7 secondes
    pub const RIEN_ENTENDU: &str = "Je n'ai rien entendu. Appuie sur le micro et parle un peu plus fort.";
    /// Plus aucun son ne vient du micro (débranché pendant l'écoute)
    pub const MICRO_PERDU: &str = "Je n'entends plus le micro. Vérifie qu'il est bien branché, puis réessaie.";
    /// Le son n'a pas pu être gardé
    pub const PAS_MARCHE: &str = "La dictée n'a pas marché cette fois. Réessaie, ou écris ta phrase.";
    /// whisper ne démarre pas
    pub const PAS_DEMARRE: &str = "La dictée n'a pas pu démarrer. Réessaie, ou écris ta phrase.";
    /// whisper n'a rien écrit de sensé
    pub const PAS_COMPRIS: &str =
        "Je n'ai pas compris ce que tu as dit. Réessaie en parlant plus près du micro, ou écris ta phrase.";
}

/// Le temps laissé à whisper pour écrire la phrase
pub const WHISPER_MAX: Duration = Duration::from_secs(60);

/// Un micro (`MindcareMicro`). Le vrai est `windows::MicroWindows` ; les tests en donnent un faux.
pub trait Micro: Send {
    /// Commence à enregistrer : `numero` est celui du micro (dans l'ordre de [`Materiel::noms`]), ou
    /// [`micro::APPAREIL_DE_WINDOWS`]. Une erreur : le code de Windows (micro absent, refusé ou déjà pris).
    fn demarrer(&mut self, numero: u32) -> Result<(), u32>;
    /// Ce qu'il a entendu jusqu'ici
    fn mesures(&self) -> Mesures;
    /// Arrête d'enregistrer (sans effet s'il n'enregistre pas)
    fn arreter(&mut self);
    /// Le son enregistré : 16 bits, 16 kHz, mono
    fn son(&self) -> Vec<u8>;
}

/// Les micros du PC
pub trait Materiel: Send {
    /// Il y a au moins un micro (`HasDevice`)
    fn micro_present(&self) -> bool;
    /// Leurs noms, dans l'ordre de leurs numéros (`Noms`) : seulement lus, aucun n'est ouvert
    fn noms(&self) -> Vec<String>;
    /// Un micro prêt à enregistrer (`New-DicteeMicro`)
    fn nouveau_micro(&self) -> Box<dyn Micro>;
}

/// Un programme lancé (whisper)
pub trait Processus: Send {
    /// `None` tant qu'il tourne ; une fois fini, son code de sortie (`None` s'il n'est pas connu)
    fn fini(&mut self) -> Option<Option<i32>>;
    /// L'arrête et attend qu'il soit parti
    fn tuer(&mut self);
    /// Son numéro de processus
    fn numero(&self) -> u32;
}

/// Un programme lancé, et s'il a pu être relié à Mindcare (arrêté avec elle, même fermée de force)
pub struct Lance {
    pub processus: Box<dyn Processus>,
    /// `Err` : la raison, pour le journal (il tourne quand même)
    pub relie: Result<(), String>,
}

/// Ce qui lance whisper. Le vrai (`windows::LanceurWindows`) : sans fenêtre, relié à Mindcare par un Job Object.
pub trait Lanceur: Send {
    /// Lance `programme` avec ses arguments, dans `dossier`. Une erreur : la raison, pour le journal.
    fn lancer(&self, programme: &Path, arguments: &[OsString], dossier: &Path) -> Result<Lance, String>;
    /// Un programme porte encore ce numéro (pour les fichiers de voix orphelins)
    fn vivant(&self, pid: u32) -> bool;
}

impl Processus for std::process::Child {
    fn fini(&mut self) -> Option<Option<i32>> {
        match self.try_wait() {
            Ok(Some(s)) => Some(s.code()),
            Ok(None) => None,
            Err(_) => Some(None),
        }
    }
    fn tuer(&mut self) {
        let _ = self.kill();
        let _ = self.wait();
    }
    fn numero(&self) -> u32 {
        self.id()
    }
}

/// Où la dictée trouve ce dont elle a besoin
#[derive(Clone, Debug)]
pub struct Reglages {
    /// Les fichiers de la dictée (`Paquet::dictee(dossier de données)`) : whisper-cli et son modèle
    pub paquet: Paquet,
    /// `%LOCALAPPDATA%\Mindset\suite.json` (le micro choisi dans Mindset) ; `None` : celui de Windows
    pub suite_json: Option<PathBuf>,
    /// Le dossier temporaire, où le son vit quelques secondes (`$env:TEMP`)
    pub temp: PathBuf,
    /// Le numéro de Mindcare (dans le nom des fichiers de voix)
    pub pid: u32,
    /// Les fils de whisper ([`whisper::fils`])
    pub fils: usize,
}

impl Reglages {
    /// Les réglages de Mindcare : les fichiers de la dictée dans `donnees`, le dossier temporaire de Windows, le
    /// numéro de ce processus, la moitié des processeurs
    pub fn de_mindcare(donnees: &Path, suite_json: Option<PathBuf>) -> Reglages {
        let processeurs = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        Reglages {
            paquet: Paquet::dictee(donnees),
            suite_json,
            temp: std::env::temp_dir(),
            pid: std::process::id(),
            fils: whisper::fils(processeurs),
        }
    }
}

/// Ce que la page montre
#[derive(Clone, Debug, PartialEq)]
pub enum Etat {
    /// whisper ou son modèle manque (l'installation, elle, est suivie par `mindcare_telechargement::Installation`)
    PasInstallee,
    /// Prête : le bouton Parler
    Prete,
    /// On écoute (« Je t'écoute »), avec le niveau du son de 0 à 1 pour l'animation
    Ecoute { niveau: f32 },
    /// whisper écrit la phrase (quelques secondes)
    Ecrit,
    /// La phrase est écrite : à mettre dans la recherche ([`Dictee::prendre_phrase`])
    PhrasePrete(String),
    /// Ça n'a pas marché, ou rien entendu : le message pour les gens (voir [`messages`])
    Erreur(String),
}

/// Ce que fait un appui sur le bouton (ce que rend `Start-Dictee`)
#[derive(Clone, Debug, PartialEq)]
pub enum Appui {
    /// Pas encore installée : la page propose l'installation
    Installer,
    /// L'écoute commence
    Ecoute,
    /// Deuxième appui : l'écoute s'arrête, whisper écrit
    Fin,
    /// whisper écrit encore : rien n'est relancé
    Occupee,
    /// Le micro n'a pas pu s'ouvrir : le message pour les gens
    Erreur(String),
}

struct EcouteEnCours {
    micro: Box<dyn Micro>,
    debut: Instant,
    micro_perdu: bool,
    niveau: f32,
}

struct Ecriture {
    processus: Box<dyn Processus>,
    son: PathBuf,
    sortie: PathBuf,
    debut: Instant,
}

/// La dictée de Mindcare. La page appelle [`Dictee::appui`] (bouton Parler), [`Dictee::etape`] toutes les 80 ms tant
/// qu'elle est [`Dictee::occupee`] (ou laisse [`crate::DicteeEnFond`] le faire), lit [`Dictee::etat`], prend la phrase,
/// et appelle [`Dictee::annuler`] quand on quitte l'accueil. Lâchée (Mindcare se ferme), elle arrête tout et efface le
/// son.
pub struct Dictee {
    reglages: Reglages,
    materiel: Box<dyn Materiel>,
    lanceur: Box<dyn Lanceur>,
    ecoute: Option<EcouteEnCours>,
    ecriture: Option<Ecriture>,
    fin: Option<Etat>,
    journal: Vec<String>,
}

fn effacer(f: &Path) {
    let _ = std::fs::remove_file(f);
}

/// `[math]::Round($s, 1)` affiché en français : « 2 », « 2,4 »
fn secondes(d: Duration) -> String {
    let r = (d.as_secs_f64() * 10.0).round() / 10.0;
    if r.fract() == 0.0 { format!("{r:.0}") } else { format!("{r:.1}").replace('.', ",") }
}

/// Lu comme `[IO.File]::ReadAllText($f, UTF8)` : marque UTF-8 enlevée, octet invalide remplacé
fn lire_texte(f: &Path) -> String {
    match std::fs::read(f) {
        Ok(o) => {
            let o = o.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&o);
            String::from_utf8_lossy(o).into_owned()
        }
        Err(_) => String::new(),
    }
}

impl Dictee {
    /// La dictée, prête. Les fichiers de voix laissés par une fermeture forcée sont effacés tout de suite
    /// (constat 25n).
    pub fn nouvelle(reglages: Reglages, materiel: Box<dyn Materiel>, lanceur: Box<dyn Lanceur>) -> Dictee {
        let mut d =
            Dictee { reglages, materiel, lanceur, ecoute: None, ecriture: None, fin: None, journal: Vec::new() };
        let lanceur = &d.lanceur;
        let n = whisper::effacer_orphelins(&d.reglages.temp, d.reglages.pid, &|p| lanceur.vivant(p), SystemTime::now());
        if n > 0 {
            d.journal.push(format!("Dictée : {n} fichier(s) de voix laissé(s) par une fermeture forcée, effacé(s)"));
        }
        d
    }

    /// whisper-cli, ses .dll et le modèle sont là (`Test-DicteeInstallee`)
    pub fn installee(&self) -> bool {
        self.reglages.paquet.installe()
    }

    /// On écoute (`Test-DicteeEcoute`)
    pub fn ecoute(&self) -> bool {
        self.ecoute.is_some()
    }

    /// On écoute ou whisper écrit (`Test-DicteeOccupee`) : [`Dictee::etape`] est à appeler
    pub fn occupee(&self) -> bool {
        self.ecoute.is_some() || self.ecriture.is_some()
    }

    /// Ce que la page montre
    pub fn etat(&self) -> Etat {
        if let Some(e) = &self.ecoute {
            return Etat::Ecoute { niveau: e.niveau };
        }
        if self.ecriture.is_some() {
            return Etat::Ecrit;
        }
        if let Some(f) = &self.fin {
            return f.clone();
        }
        if self.installee() { Etat::Prete } else { Etat::PasInstallee }
    }

    /// La phrase écrite, une seule fois ; ensuite la dictée est de nouveau prête
    pub fn prendre_phrase(&mut self) -> Option<String> {
        match self.fin.take() {
            Some(Etat::PhrasePrete(p)) => Some(p),
            autre => {
                self.fin = autre;
                None
            }
        }
    }

    /// Le message affiché est lu : la dictée est de nouveau prête
    pub fn effacer_message(&mut self) {
        if matches!(self.fin, Some(Etat::Erreur(_))) {
            self.fin = None;
        }
    }

    /// Les lignes pour le journal de Mindcare depuis la dernière fois (jamais la phrase dite, seulement sa longueur)
    pub fn prendre_journal(&mut self) -> Vec<String> {
        std::mem::take(&mut self.journal)
    }

    /// Le fichier son de l'écoute en cours (`<temp>\mindcare-<pid>-dictee.wav`)
    pub fn fichier_son(&self) -> PathBuf {
        whisper::fichier_son(&self.reglages.temp, self.reglages.pid)
    }

    /// Le bouton Parler (`Start-Dictee`) : écouter, ou arrêter d'écouter (deuxième appui)
    pub fn appui(&mut self) -> Appui {
        self.appui_a(Instant::now())
    }

    /// [`Dictee::appui`] à l'heure donnée (pour les tests)
    pub fn appui_a(&mut self, maintenant: Instant) -> Appui {
        if self.ecoute.is_some() {
            self.arreter_ecoute(maintenant);
            return Appui::Fin;
        }
        if self.ecriture.is_some() {
            return Appui::Occupee;
        }
        if !self.installee() {
            return Appui::Installer;
        }
        self.fin = None;
        if !self.materiel.micro_present() {
            self.journal.push("Dictée : aucun micro sur ce PC".into());
            return self.erreur_appui(messages::PAS_DE_MICRO);
        }
        let numero = self.micro_choisi();
        let mut m = self.materiel.nouveau_micro();
        if let Err(code) = m.demarrer(numero) {
            self.journal.push(format!("Dictée : micro indisponible (code {code})"));
            m.arreter();
            return self.erreur_appui(messages::MICRO_REFUSE);
        }
        self.ecoute = Some(EcouteEnCours { micro: m, debut: maintenant, micro_perdu: false, niveau: 0.0 });
        self.journal.push("Dictée : écoute".into());
        Appui::Ecoute
    }

    fn erreur_appui(&mut self, message: &str) -> Appui {
        self.fin = Some(Etat::Erreur(message.into()));
        Appui::Erreur(message.into())
    }

    /// `Get-DicteeMicro` : le numéro du micro choisi dans Mindset, ou celui de Windows s'il n'y en a pas, ou s'il est
    /// débranché. suite.json est lu 3 fois au plus s'il est pris (plus d'une demi-seconde : celui de Windows).
    pub fn micro_choisi(&mut self) -> u32 {
        let Some(f) = &self.reglages.suite_json else { return micro::APPAREIL_DE_WINDOWS };
        let choix = micro::lire(f, 3).son.micro;
        let Choix::Appareil(nom) = &choix else { return micro::APPAREIL_DE_WINDOWS };
        let trouve = micro::trouver(&choix, &self.materiel.noms());
        if trouve == Trouve::Debranche {
            self.journal.push(format!(
                "Dictée : le micro « {nom} » choisi dans Mindset n'est pas branché, celui de Windows à la place"
            ));
        }
        trouve.numero()
    }

    /// Toutes les 80 ms tant que la dictée est occupée (`Step-Dictee` et `Step-DicteeEcriture`)
    pub fn etape(&mut self) {
        self.etape_a(Instant::now());
    }

    /// [`Dictee::etape`] à l'heure donnée (pour les tests)
    pub fn etape_a(&mut self, maintenant: Instant) {
        self.etape_ecoute(maintenant);
        self.etape_ecriture(maintenant);
    }

    /// `Step-Dictee` : on s'arrête une seconde après la fin de la phrase, après 7 secondes sans rien entendre, ou au
    /// bout de 25 secondes de son (30 en tout). Micro débranché pendant l'écoute : plus aucun son n'arrive, arrêt au
    /// bout de 3 secondes.
    fn etape_ecoute(&mut self, maintenant: Instant) {
        let Some(e) = &mut self.ecoute else { return };
        let m = e.micro.mesures();
        e.niveau = m.niveau;
        let ecoule = maintenant.saturating_duration_since(e.debut).as_millis();
        if ecoule.saturating_sub(u128::from(m.millis)) > 3000 {
            e.micro_perdu = true;
            self.arreter_ecoute(maintenant);
            return;
        }
        let fin = (m.entendu && m.millis_depuis_voix > 1000)
            || (!m.entendu && m.millis > 7000)
            || m.millis > 25_000
            || ecoule > 30_000;
        if fin {
            self.arreter_ecoute(maintenant);
        }
    }

    /// `Stop-Dictee` : fin de l'écoute ; whisper écrit ce qui a été dit, sinon un message
    fn arreter_ecoute(&mut self, maintenant: Instant) {
        let Some(mut e) = self.ecoute.take() else { return };
        e.micro.arreter();
        if !e.micro.mesures().entendu {
            let (journal, message) = if e.micro_perdu {
                ("plus aucun son ne vient du micro", messages::MICRO_PERDU)
            } else {
                ("rien entendu", messages::RIEN_ENTENDU)
            };
            self.journal.push(format!("Dictée : {journal}"));
            self.fin = Some(Etat::Erreur(message.into()));
            return;
        }
        let son = self.fichier_son();
        if let Err(err) = std::fs::write(&son, wav(&e.micro.son())) {
            self.journal.push(format!("Dictée : son non gardé ({err})"));
            effacer(&son);
            self.fin = Some(Etat::Erreur(messages::PAS_MARCHE.into()));
            return;
        }
        drop(e);
        self.lancer_whisper(son, maintenant);
    }

    /// `Start-DicteeEcriture` : whisper écrit la phrase, sans fenêtre, relié à Mindcare
    fn lancer_whisper(&mut self, son: PathBuf, maintenant: Instant) {
        let r = &self.reglages;
        let sortie = whisper::sortie(&r.temp, r.pid);
        let txt = whisper::sortie_txt(&r.temp, r.pid);
        effacer(&txt);
        let arguments = whisper::arguments(&r.paquet.modele, &son, r.fils, &sortie);
        match self.lanceur.lancer(&r.paquet.programme, &arguments, &r.paquet.dossier) {
            Err(err) => {
                self.journal.push(format!("Dictée : Whisper ne démarre pas ({err})"));
                effacer(&son);
                self.fin = Some(Etat::Erreur(messages::PAS_DEMARRE.into()));
            }
            Ok(lance) => {
                if let Err(err) = lance.relie {
                    self.journal.push(format!("Dictée : pas reliée à Mindcare ({err})"));
                }
                self.ecriture = Some(Ecriture { processus: lance.processus, son, sortie: txt, debut: maintenant });
            }
        }
    }

    /// `Step-DicteeEcriture` : whisper a fini ? (60 secondes au plus) Le son est effacé tout de suite, puis ce qu'il a
    /// écrit, une fois lu.
    fn etape_ecriture(&mut self, maintenant: Instant) {
        let Some(e) = &mut self.ecriture else { return };
        let duree = maintenant.saturating_duration_since(e.debut);
        let code = match e.processus.fini() {
            Some(code) => code,
            None if duree < WHISPER_MAX => return,
            None => {
                e.processus.tuer();
                self.journal.push("Dictée : Whisper trop long, arrêté".into());
                e.processus.fini().flatten()
            }
        };
        let Some(e) = self.ecriture.take() else { return };
        effacer(&e.son);
        let brut = lire_texte(&e.sortie);
        effacer(&e.sortie);
        let texte = nettoyer(&brut);
        let code = match code {
            Some(0) => String::new(),
            Some(c) => format!(" (code {c})"),
            None => " (code ?)".into(),
        };
        // (la phrase n'est pas écrite dans le journal, seulement sa longueur, comme pour l'assistant)
        self.journal.push(format!("Dictée : {} s, {} caractères{code}", secondes(duree), texte.encode_utf16().count()));
        self.fin =
            Some(if texte.is_empty() { Etat::Erreur(messages::PAS_COMPRIS.into()) } else { Etat::PhrasePrete(texte) });
    }

    /// `Stop-DicteeSansSuite` : on quitte l'accueil, ou Mindcare se ferme. On arrête d'écouter et whisper, sans
    /// suite ; le son et ce que whisper a écrit sont effacés.
    pub fn annuler(&mut self) {
        if let Some(mut e) = self.ecoute.take() {
            e.micro.arreter();
        }
        if let Some(mut e) = self.ecriture.take() {
            if e.processus.fini().is_none() {
                e.processus.tuer();
            }
            effacer(&e.son);
            effacer(&e.sortie);
        }
        // (une écoute arrêtée avant d'écrire le son : rien n'est resté, mais on s'en assure)
        effacer(&self.fichier_son());
        self.fin = None;
    }
}

impl Drop for Dictee {
    fn drop(&mut self) {
        self.annuler();
    }
}

#[cfg(test)]
mod tests {
    use super::secondes;
    use std::time::Duration;

    #[test]
    fn secondes_en_francais() {
        assert_eq!(secondes(Duration::from_millis(2000)), "2");
        assert_eq!(secondes(Duration::from_millis(2440)), "2,4");
        assert_eq!(secondes(Duration::from_millis(2960)), "3");
        assert_eq!(secondes(Duration::from_millis(60_050)), "60,1");
    }
}
