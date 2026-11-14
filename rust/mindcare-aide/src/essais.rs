//! Le faux PC des tests (jamais dans l'exe livré) : il répond comme on le lui dit et note chaque appel, sans jamais
//! rien toucher sur le vrai PC (ni Wi-Fi, ni réseau, ni alimentation, ni service, ni fichier). Il sert aussi de faux
//! hôte : un faux canal, dont les connexions sont données par le test.

use crate::systeme::{CarteWifi, Connexion, Ecoute, Entree, ErreurCanal, Hote, Identite, Systeme, Verrouillage};
use mindcare_aide_regles::cartes::Carte;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const ELODIE: &str = "S-1-5-21-1111111111-2222222222-3333333333-1001";
pub const BRUNO: &str = "S-1-5-21-1111111111-2222222222-3333333333-1002";
pub const RACINE: &str = "C:\\Mindset\\Programmes\\Mindcare";
pub const WINDOWS: &str = "C:\\Windows";
pub const SD: &str = "C:\\Windows\\SoftwareDistribution";
pub const OLD: &str = "C:\\Windows\\SoftwareDistribution.old";

/// Ce que le faux PC répond (modifiable par le test)
pub struct Reglage {
    pub identite: Identite,
    /// (numéro, chemin, compte)
    pub processus: Vec<(i64, String, String)>,
    pub wifi: Result<Option<Vec<CarteWifi>>, String>,
    /// Les Wi-Fi que Windows refuse d'oublier
    pub wifi_gardes: Vec<String>,
    pub cartes_avant: Vec<Carte>,
    pub cartes_apres: Vec<Carte>,
    pub ip_rendre: Result<(), String>,
    pub ip_redemander: Result<(), String>,
    pub powercfg: Result<i32, String>,
    /// powercfg fait paniquer l'Aide (une erreur interne)
    pub powercfg_panique: bool,
    pub plans: VecDeque<String>,
    /// Les services qui tournent
    pub services: HashMap<String, bool>,
    /// Les services qui ne s'arrêtent pas
    pub bloques: Vec<String>,
    /// Les services qui ne redémarrent pas
    pub sans_relance: Vec<String>,
    pub entrees: HashMap<String, Entree>,
    /// Le nombre de renommages refusés avant le premier accepté (u32::MAX : toujours refusé)
    pub renommages_refuses: u32,
    pub suppression: Result<(), String>,
}

impl Default for Reglage {
    fn default() -> Self {
        Reglage {
            identite: Identite { compte: "S-1-5-18".into(), admin: true, pid: 4242 },
            processus: Vec::new(),
            wifi: Ok(None),
            wifi_gardes: Vec::new(),
            cartes_avant: Vec::new(),
            cartes_apres: Vec::new(),
            ip_rendre: Ok(()),
            ip_redemander: Ok(()),
            powercfg: Ok(0),
            powercfg_panique: false,
            plans: VecDeque::new(),
            services: [("wuauserv".to_string(), true), ("bits".to_string(), true)].into(),
            bloques: Vec::new(),
            sans_relance: Vec::new(),
            entrees: [(SD.to_string(), Entree::Dossier)].into(),
            renommages_refuses: 0,
            suppression: Ok(()),
        }
    }
}

/// Une connexion donnée par le test : le processus qui appelle et ce qu'il envoie ; la réponse est gardée
#[derive(Clone)]
pub struct Appel {
    pub client: u32,
    pub envoi: Vec<u8>,
    pub reponse: Arc<Mutex<Vec<u8>>>,
}

impl Appel {
    pub fn nouveau(client: u32, ligne: &str) -> Appel {
        Appel { client, envoi: format!("{ligne}\r\n").into_bytes(), reponse: Arc::default() }
    }
    pub fn reponse(&self) -> String {
        String::from_utf8_lossy(&self.reponse.lock().unwrap()).into_owned()
    }
}

/// Un canal créé par l'Aide : (nom, droits, premier, propriétaire)
pub type CanalCree = (String, String, bool, Option<String>);

#[derive(Default)]
pub struct FauxPc {
    pub r: Mutex<Reglage>,
    /// Chaque appel fait, dans l'ordre
    pub appels: Mutex<Vec<String>>,
    /// Les connexions qui attendent (faux canal)
    pub file: Mutex<VecDeque<Appel>>,
    /// Les canaux créés : (nom, droits, premier, propriétaire)
    pub canaux: Mutex<Vec<CanalCree>>,
}

impl FauxPc {
    pub fn nouveau(r: Reglage) -> Arc<FauxPc> {
        Arc::new(FauxPc { r: Mutex::new(r), ..FauxPc::default() })
    }
    fn noter(&self, a: String) {
        self.appels.lock().unwrap().push(a);
    }
    pub fn appels(&self) -> Vec<String> {
        self.appels.lock().unwrap().clone()
    }
    fn r(&self) -> std::sync::MutexGuard<'_, Reglage> {
        self.r.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Une carte réseau de test
pub fn carte(index: u32, nom: &str, dhcp: bool, adresse: &str) -> Carte {
    Carte {
        index,
        nom: nom.into(),
        description: format!("Carte {nom}"),
        genre: 6,
        active: true,
        dhcp,
        passerelle: true,
        adresses: vec![adresse.into()],
    }
}

impl Systeme for FauxPc {
    fn identite(&self) -> Identite {
        self.r().identite.clone()
    }
    fn racine(&self) -> String {
        RACINE.into()
    }
    fn heure_locale(&self) -> String {
        "2026-11-14T10:05:09".into()
    }
    fn pause(&self, d: Duration) {
        self.noter(format!("pause {} ms", d.as_millis()));
    }
    fn processus(&self) -> Vec<i64> {
        self.r().processus.iter().map(|p| p.0).collect()
    }
    fn chemin_processus(&self, pid: i64) -> String {
        self.r().processus.iter().find(|p| p.0 == pid).map(|p| p.1.clone()).unwrap_or_default()
    }
    fn compte_processus(&self, pid: i64) -> String {
        self.r().processus.iter().find(|p| p.0 == pid).map(|p| p.2.clone()).unwrap_or_default()
    }
    fn wifi(&self) -> Result<Option<Vec<CarteWifi>>, String> {
        self.noter("wifi".into());
        self.r().wifi.clone()
    }
    fn wifi_oublier(&self, carte: &str, profil: &str) -> Result<(), String> {
        self.noter(format!("oublier {carte} {profil}"));
        if self.r().wifi_gardes.iter().any(|g| g == profil) { Err("erreur 5".into()) } else { Ok(()) }
    }
    fn cartes(&self) -> Result<Vec<Carte>, String> {
        let premier = !self.appels().iter().any(|a| a == "cartes");
        self.noter("cartes".into());
        let r = self.r();
        Ok(if premier { r.cartes_avant.clone() } else { r.cartes_apres.clone() })
    }
    fn ip_rendre(&self, index: u32) -> Result<(), String> {
        self.noter(format!("rendre {index}"));
        self.r().ip_rendre.clone()
    }
    fn ip_redemander(&self, index: u32, limite: Duration) -> Result<(), String> {
        self.noter(format!("redemander {index} ({} s au plus)", limite.as_millis().div_ceil(1000)));
        self.r().ip_redemander.clone()
    }
    fn powercfg_par_defaut(&self) -> Result<i32, String> {
        self.noter("powercfg".into());
        if self.r().powercfg_panique {
            panic!("panique d'essai");
        }
        self.r().powercfg.clone()
    }
    fn plan_actif(&self) -> Option<String> {
        self.r().plans.pop_front()
    }
    fn dossier_windows(&self) -> String {
        WINDOWS.into()
    }
    fn service_tourne(&self, nom: &str) -> Result<bool, String> {
        self.r().services.get(nom).copied().ok_or_else(|| format!("service {nom} introuvable"))
    }
    fn service_arreter(&self, nom: &str) -> Result<(), String> {
        self.noter(format!("arrêter {nom}"));
        let mut r = self.r();
        if r.bloques.iter().any(|b| b == nom) {
            return Err(format!("service {nom} toujours en marche après 60 s"));
        }
        r.services.insert(nom.into(), false);
        Ok(())
    }
    fn service_demarrer(&self, nom: &str) -> Result<(), String> {
        self.noter(format!("démarrer {nom}"));
        let mut r = self.r();
        if r.sans_relance.iter().any(|b| b == nom) {
            return Err(format!("service {nom} : désactivé"));
        }
        r.services.insert(nom.into(), true);
        Ok(())
    }
    fn entree(&self, chemin: &str) -> Entree {
        self.r().entrees.get(chemin).copied().unwrap_or(Entree::Absente)
    }
    fn supprimer_dossier(&self, chemin: &str) -> Result<(), String> {
        self.noter(format!("supprimer {chemin}"));
        let mut r = self.r();
        r.suppression.clone()?;
        r.entrees.remove(chemin);
        Ok(())
    }
    fn renommer(&self, de: &str, vers: &str) -> Result<(), String> {
        self.noter(format!("renommer {de} en {vers}"));
        let mut r = self.r();
        if r.renommages_refuses > 0 {
            if r.renommages_refuses != u32::MAX {
                r.renommages_refuses -= 1;
            }
            return Err("accès refusé (32)".into());
        }
        if let Some(e) = r.entrees.remove(de) {
            r.entrees.insert(vers.into(), e);
        }
        Ok(())
    }
}

/// Une fausse connexion : elle rend ce que le test a mis dans `envoi`, puis « coupé »
pub struct FausseConnexion {
    appel: Appel,
    lu: bool,
}

impl Connexion for FausseConnexion {
    fn client(&self) -> u32 {
        self.appel.client
    }
    fn lire(&mut self, tampon: &mut [u8], _ms: u32) -> Result<Option<usize>, String> {
        if self.lu {
            return Ok(Some(0));
        }
        self.lu = true;
        let n = self.appel.envoi.len().min(tampon.len());
        tampon[..n].copy_from_slice(&self.appel.envoi[..n]);
        Ok(Some(n))
    }
    fn ecrire(&mut self, octets: &[u8]) -> Result<(), String> {
        self.appel.reponse.lock().unwrap().extend_from_slice(octets);
        Ok(())
    }
}

/// Un faux exemplaire du canal : connecté dès qu'une connexion attend dans la file du faux PC
pub struct FausseEcoute {
    pc: Arc<FauxPc>,
    pret: Option<Appel>,
}

impl Ecoute for FausseEcoute {
    type C = FausseConnexion;
    fn attendre(&mut self, ms: u32) -> Result<bool, String> {
        if self.pret.is_none() {
            self.pret = self.pc.file.lock().unwrap().pop_front();
        }
        if self.pret.is_none() {
            std::thread::sleep(Duration::from_millis(u64::from(ms.min(10))));
        }
        Ok(self.pret.is_some())
    }
    fn connexion(self) -> FausseConnexion {
        FausseConnexion { appel: self.pret.expect("connexion sans client"), lu: false }
    }
}

/// Le faux hôte : un faux PC partagé, et le canal qu'il donne
pub struct FauxHote(pub Arc<FauxPc>);

macro_rules! deleguer {
    ($($f:ident($($a:ident: $t:ty),*) -> $r:ty;)*) => {
        $(fn $f(&self, $($a: $t),*) -> $r { self.0.$f($($a),*) })*
    };
}

impl Systeme for FauxHote {
    fn pause(&self, d: Duration) {
        self.0.pause(d)
    }
    deleguer! {
        identite() -> Identite;
        racine() -> String;
        heure_locale() -> String;
        processus() -> Vec<i64>;
        chemin_processus(pid: i64) -> String;
        compte_processus(pid: i64) -> String;
        wifi() -> Result<Option<Vec<CarteWifi>>, String>;
        wifi_oublier(carte: &str, profil: &str) -> Result<(), String>;
        cartes() -> Result<Vec<Carte>, String>;
        ip_rendre(index: u32) -> Result<(), String>;
        ip_redemander(index: u32, limite: Duration) -> Result<(), String>;
        powercfg_par_defaut() -> Result<i32, String>;
        plan_actif() -> Option<String>;
        dossier_windows() -> String;
        service_tourne(nom: &str) -> Result<bool, String>;
        service_arreter(nom: &str) -> Result<(), String>;
        service_demarrer(nom: &str) -> Result<(), String>;
        entree(chemin: &str) -> Entree;
        supprimer_dossier(chemin: &str) -> Result<(), String>;
        renommer(de: &str, vers: &str) -> Result<(), String>;
    }
}

impl Hote for FauxHote {
    type E = FausseEcoute;
    fn verrou(&self, _nom: &str) -> Verrouillage {
        Verrouillage::Pris { garde: Box::new(()), remarque: None }
    }
    fn canal(
        &self,
        nom: &str,
        sddl: &str,
        premier: bool,
        proprietaire: Option<&str>,
    ) -> Result<FausseEcoute, ErreurCanal> {
        self.0.canaux.lock().unwrap().push((nom.into(), sddl.into(), premier, proprietaire.map(str::to_string)));
        Ok(FausseEcoute { pc: Arc::clone(&self.0), pret: None })
    }
}

/// Une fausse connexion pour cet appel (pour `serveur::traiter`)
pub fn connexion(appel: Appel) -> FausseConnexion {
    FausseConnexion { appel, lu: false }
}
