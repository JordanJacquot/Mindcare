//! Le téléchargement en arrière-plan (`Start-Telechargement`, modules\ia.ps1) : sur un fil à part, avec reprise si
//! la connexion se coupe, l'empreinte vérifiée, la place libre vérifiée avant, l'extraction des seuls fichiers voulus.
//! Les pages suivent l'avancement ([`Avancement`]) ; Mindcare qui se ferme l'annule ([`Telechargement::annuler`]).
//!
//! Les mêmes règles que le PowerShell, et les mêmes messages :
//! - assez de place ? Ce qui reste à télécharger plus une marge, vérifié avant de commencer ;
//! - un fichier commencé (« .telechargement ») est repris là où il s'est arrêté (en-tête Range) ; si le serveur
//!   renvoie tout le fichier (pas de réponse 206), il recommence au début ;
//! - une coupure de la connexion est réessayée, avec une pause de plus en plus longue ; après plusieurs coupures :
//!   « connexion instable ». Une erreur d'écriture sur le disque n'est pas une coupure : disque plein, ou fichier
//!   bloqué (un antivirus) au bout de trois essais ;
//! - une mauvaise empreinte : le fichier est effacé et téléchargé une deuxième fois, puis « arrive abîmé » ;
//! - un zip : seuls les fichiers voulus sont extraits, le programme en dernier ([`crate::zip`]).
//!
//! Le réseau ([`Transport`]) et la place libre ([`Disque`]) sont donnés par l'appelant : WinHTTP et Windows dans
//! Mindcare (`crate::windows`), un faux serveur sur 127.0.0.1 dans les tests.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::fichiers::{Fichier, MO};
use crate::zip;

/// La pause avant de réessayer un téléchargement coupé, multipliée par le nombre d'essais (`$script:IaPause`)
pub const PAUSE: Duration = Duration::from_secs(3);

/// Une réponse du serveur, lue morceau par morceau
pub trait Reponse: Send {
    /// Le code de la réponse (200, 206)
    fn statut(&self) -> u16;
    /// Lit la suite (0 : fini). Une erreur est une coupure de la connexion.
    fn lire(&mut self, tampon: &mut [u8]) -> Result<usize, String>;
}

/// Le réseau : demande un fichier, à partir de l'octet `depuis` (0 : tout le fichier)
pub trait Transport: Send + Sync {
    /// Une erreur (serveur injoignable, réponse autre que 200 ou 206) est une coupure de la connexion
    fn ouvrir(&self, url: &str, depuis: u64) -> Result<Box<dyn Reponse>, String>;
}

/// La place libre sur le disque d'un dossier, `None` si on ne sait pas (dossier réseau...) : `Get-PlaceLibre`
pub trait Disque: Send + Sync {
    fn place_libre(&self, dossier: &Path) -> Option<u64>;
}

/// Où en est un téléchargement (`{ Recu, Total, Etape, Erreur, Fini }` du PowerShell), lisible par les pages
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Avancement {
    /// Les octets reçus (fichiers finis compris)
    pub recu: u64,
    /// La taille de tous les fichiers à télécharger
    pub total: u64,
    /// Le fichier en cours (« moteur de l'assistant », « moteur de l'assistant (vérification) »)
    pub etape: String,
    /// Vide si tout va bien ; sinon le message pour les gens, en français simple (« il n'y a pas assez de place... »)
    pub erreur: String,
    pub fini: bool,
    /// Arrêté par Mindcare (qui se ferme) : ni réussi ni en erreur. Ce qui est commencé reste pour la reprise.
    pub annule: bool,
}

impl Avancement {
    /// Le pourcentage montré (arrondi au-dessous, comme `[math]::Floor`)
    pub fn pourcent(&self) -> u8 {
        (self.recu.min(self.total) * 100).checked_div(self.total).unwrap_or(0) as u8
    }
}

/// Un téléchargement lancé. Le lâcher (Mindcare qui se ferme) l'annule.
pub struct Telechargement {
    etat: Arc<Mutex<Avancement>>,
    annuler: Arc<AtomicBool>,
    fil: Option<JoinHandle<()>>,
}

fn verrou(etat: &Mutex<Avancement>) -> MutexGuard<'_, Avancement> {
    etat.lock().unwrap_or_else(|e| e.into_inner())
}

/// Disque plein : ERROR_DISK_FULL (112) ou ERROR_HANDLE_DISK_FULL (39)
pub fn est_plein(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(112) | Some(39)) || e.kind() == io::ErrorKind::StorageFull
}

const FAIS_DE_LA_PLACE: &str = "Fais de la place (la corbeille, les vieux téléchargements), puis réessaie";

/// Pourquoi le téléchargement s'arrête avant la fin
enum Fin {
    Annule,
    Erreur(String),
}

/// Pourquoi un essai de téléchargement s'arrête avant la fin du fichier
enum Coupe {
    Annule,
    Reseau,
    /// Une erreur en écrivant sur le disque (pas une coupure de la connexion)
    Ecriture(io::Error),
}

impl Telechargement {
    /// Lance le téléchargement de `manquants` dans `dossier` (`Start-Telechargement`). La place libre est vérifiée
    /// tout de suite : s'il n'y en a pas assez, il est fini avant d'avoir commencé, avec un message.
    pub fn lancer(
        manquants: Vec<Fichier>,
        dossier: &Path,
        marge: u64,
        transport: Arc<dyn Transport>,
        disque: &dyn Disque,
        pause: Duration,
    ) -> Telechargement {
        let total = manquants.iter().map(|f| f.taille).sum();
        let etat = Arc::new(Mutex::new(Avancement { total, ..Avancement::default() }));
        let annuler = Arc::new(AtomicBool::new(false));
        // Assez de place ? (ce qui reste à télécharger, plus les programmes une fois extraits et une marge)
        let reste: u64 =
            manquants.iter().map(|f| f.taille.saturating_sub(taille_de(&dossier.join(f.nom_temporaire())))).sum();
        let besoin = reste + marge;
        if let Some(libre) = disque.place_libre(dossier)
            && libre < besoin
        {
            let mut e = verrou(&etat);
            e.erreur = format!(
                "il n'y a pas assez de place sur le disque (il faut environ {} Mo de libre, il en reste {} Mo). {FAIS_DE_LA_PLACE}",
                besoin.div_ceil(MO),
                libre / MO
            );
            e.fini = true;
            drop(e);
            return Telechargement { etat, annuler, fil: None };
        }
        let (etat_fil, annuler_fil, dossier) = (etat.clone(), annuler.clone(), dossier.to_path_buf());
        let fil = std::thread::Builder::new().name("telechargement".into()).spawn(move || {
            let travail = Travail { etat: &etat_fil, annuler: &annuler_fil, transport: &*transport, pause };
            let fin = travail.tout(&manquants, &dossier);
            let mut e = verrou(&etat_fil);
            match fin {
                Ok(()) => {}
                Err(Fin::Annule) => e.annule = true,
                Err(Fin::Erreur(m)) => e.erreur = m,
            }
            e.fini = true;
        });
        match fil {
            Ok(fil) => Telechargement { etat, annuler, fil: Some(fil) },
            Err(err) => {
                let mut e = verrou(&etat);
                e.erreur = format!("Mindcare n'a pas pu commencer le téléchargement ({err}). Réessaie");
                e.fini = true;
                drop(e);
                Telechargement { etat, annuler, fil: None }
            }
        }
    }

    /// Où il en est (une copie, pour les pages)
    pub fn avancement(&self) -> Avancement {
        verrou(&self.etat).clone()
    }

    pub fn fini(&self) -> bool {
        verrou(&self.etat).fini
    }

    /// Arrête le téléchargement (Mindcare se ferme) sans attendre : il s'arrête au prochain morceau lu. Le fichier
    /// commencé reste, pour la reprise.
    pub fn annuler(&self) {
        self.annuler.store(true, Ordering::SeqCst);
    }

    /// Attend la fin, au plus `delai`. Rend vrai s'il est fini.
    pub fn attendre(&mut self, delai: Duration) -> bool {
        let limite = Instant::now() + delai;
        while !self.fini() {
            if Instant::now() >= limite {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some(f) = self.fil.take() {
            let _ = f.join();
        }
        true
    }
}

impl Drop for Telechargement {
    fn drop(&mut self) {
        self.annuler();
    }
}

fn taille_de(p: &Path) -> u64 {
    fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// Le travail du fil
struct Travail<'a> {
    etat: &'a Mutex<Avancement>,
    annuler: &'a AtomicBool,
    transport: &'a dyn Transport,
    pause: Duration,
}

impl Travail<'_> {
    fn annule(&self) -> bool {
        self.annuler.load(Ordering::SeqCst)
    }

    /// Attend `duree`, sauf si le téléchargement est annulé pendant
    fn patienter(&self, duree: Duration) -> Result<(), Fin> {
        let limite = Instant::now() + duree;
        loop {
            if self.annule() {
                return Err(Fin::Annule);
            }
            let maintenant = Instant::now();
            if maintenant >= limite {
                return Ok(());
            }
            std::thread::sleep((limite - maintenant).min(Duration::from_millis(50)));
        }
    }

    fn etape(&self, texte: String) {
        verrou(self.etat).etape = texte;
    }

    /// Tous les fichiers, l'un après l'autre
    fn tout(&self, fichiers: &[Fichier], dossier: &Path) -> Result<(), Fin> {
        fs::create_dir_all(dossier).map_err(|e| erreur_generale(&e))?;
        let mut deja = 0u64;
        for f in fichiers {
            self.un_fichier(f, dossier, deja)?;
            deja += f.taille;
            verrou(self.etat).recu = deja;
            if self.annule() {
                return Err(Fin::Annule);
            }
        }
        Ok(())
    }

    fn un_fichier(&self, f: &Fichier, dossier: &Path, deja: u64) -> Result<(), Fin> {
        self.etape(f.nom.clone());
        let tmp = dossier.join(f.nom_temporaire());
        let instable = || {
            Fin::Erreur(format!(
                "le téléchargement de « {} » s'est coupé plusieurs fois (connexion instable ?). Réessaie : il reprendra là où il s'est arrêté",
                f.nom
            ))
        };
        let (mut coupures, mut bloque, mut refait) = (0u32, 0u32, false);
        loop {
            if self.annule() {
                return Err(Fin::Annule);
            }
            let mut existant = taille_de(&tmp);
            if existant > f.taille {
                fs::remove_file(&tmp).map_err(|e| erreur_generale(&e))?;
                existant = 0;
            }
            if existant < f.taille {
                match self.un_essai(f, &tmp, existant, deja) {
                    Ok(()) => {}
                    Err(Coupe::Annule) => return Err(Fin::Annule),
                    Err(Coupe::Ecriture(e)) => {
                        if est_plein(&e) {
                            return Err(Fin::Erreur(format!(
                                "il n'y a plus assez de place sur le disque pour « {} ». {FAIS_DE_LA_PLACE} : il reprendra là où il s'est arrêté",
                                f.nom
                            )));
                        }
                        bloque += 1;
                        if bloque >= 3 {
                            return Err(Fin::Erreur(format!(
                                "Mindcare n'arrive pas à écrire le fichier « {} » sur le disque (un antivirus le bloque peut-être). Réessaie dans un moment : il reprendra là où il s'est arrêté",
                                f.nom
                            )));
                        }
                        self.patienter(self.pause * bloque)?;
                        continue;
                    }
                    Err(Coupe::Reseau) => {
                        coupures += 1;
                        if coupures >= 6 {
                            return Err(instable());
                        }
                    }
                }
                if taille_de(&tmp) < f.taille {
                    if coupures >= 6 {
                        return Err(instable());
                    }
                    coupures += 1;
                    self.patienter((self.pause * coupures).min(Duration::from_secs(20)))?;
                    continue;
                }
            }
            self.etape(format!("{} (vérification)", f.nom));
            let empreinte = self.empreinte(&tmp)?;
            self.etape(f.nom.clone());
            if empreinte.eq_ignore_ascii_case(&f.sha256) {
                break;
            }
            fs::remove_file(&tmp).map_err(|e| erreur_generale(&e))?;
            if refait {
                return Err(Fin::Erreur(format!(
                    "le fichier « {} » arrive abîmé (deux fois de suite). Un antivirus le modifie peut-être pendant le téléchargement",
                    f.nom
                )));
            }
            refait = true;
        }
        verrou(self.etat).recu = deja + f.taille;
        match &f.extraire {
            Some(voulus) => {
                let programme = f.garder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let annule = || self.annule();
                zip::extraire_voulus(&tmp, dossier, voulus, &programme, &annule).map_err(|e| {
                    if e.kind() == io::ErrorKind::Interrupted {
                        Fin::Annule
                    } else if est_plein(&e) {
                        erreur_generale(&e)
                    } else if e.kind() == io::ErrorKind::InvalidData {
                        Fin::Erreur(format!(
                            "le fichier « {} » ne s'ouvre pas ({e}). Réessaie",
                            f.nom
                        ))
                    } else {
                        Fin::Erreur(format!(
                            "Mindcare n'arrive pas à installer « {} » sur le disque (un antivirus le bloque peut-être). Réessaie dans un moment",
                            f.nom
                        ))
                    }
                })?;
                fs::remove_file(&tmp).map_err(|e| erreur_generale(&e))?;
            }
            None => fs::rename(&tmp, &f.garder).map_err(|e| {
                if est_plein(&e) {
                    erreur_generale(&e)
                } else {
                    Fin::Erreur(format!(
                        "Mindcare n'arrive pas à installer « {} » sur le disque (un antivirus le bloque peut-être). Réessaie dans un moment",
                        f.nom
                    ))
                }
            })?,
        }
        Ok(())
    }

    /// Un essai : demande la suite du fichier et l'écrit à la fin de `tmp`
    fn un_essai(&self, f: &Fichier, tmp: &Path, existant: u64, deja: u64) -> Result<(), Coupe> {
        let mut reponse = self.transport.ouvrir(&f.url, existant).map_err(|_| Coupe::Reseau)?;
        // (le serveur renvoie tout le fichier au lieu de la suite : on recommence au début)
        let existant = if existant > 0 && reponse.statut() != 206 { 0 } else { existant };
        let mut sortie = if existant > 0 {
            OpenOptions::new().append(true).open(tmp)
        } else {
            OpenOptions::new().write(true).create(true).truncate(true).open(tmp)
        }
        .map_err(Coupe::Ecriture)?;
        let mut tampon = vec![0u8; 64 * 1024];
        let mut ecrit = existant;
        loop {
            if self.annule() {
                return Err(Coupe::Annule);
            }
            let n = reponse.lire(&mut tampon).map_err(|_| Coupe::Reseau)?;
            if n == 0 {
                return Ok(());
            }
            sortie.write_all(&tampon[..n]).map_err(Coupe::Ecriture)?;
            ecrit += n as u64;
            verrou(self.etat).recu = deja + ecrit.min(f.taille);
        }
    }

    /// L'empreinte SHA-256 d'un fichier, en majuscules (`Get-FileHash`)
    fn empreinte(&self, p: &Path) -> Result<String, Fin> {
        let mut f = fs::File::open(p).map_err(|e| erreur_generale(&e))?;
        let mut h = Sha256::new();
        let mut tampon = vec![0u8; 1024 * 1024];
        loop {
            if self.annule() {
                return Err(Fin::Annule);
            }
            let n = f.read(&mut tampon).map_err(|e| erreur_generale(&e))?;
            if n == 0 {
                break;
            }
            h.update(&tampon[..n]);
        }
        Ok(h.finalize().iter().map(|b| format!("{b:02X}")).collect())
    }
}

/// Une erreur du disque hors du téléchargement lui-même (créer le dossier, effacer, renommer)
fn erreur_generale(e: &io::Error) -> Fin {
    if est_plein(e) {
        Fin::Erreur(format!("il n'y a plus assez de place sur le disque. {FAIS_DE_LA_PLACE}"))
    } else {
        Fin::Erreur(format!("Mindcare n'arrive pas à écrire sur le disque ({e}). Réessaie dans un moment"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disque_plein_reconnu() {
        assert!(est_plein(&io::Error::from_raw_os_error(112)));
        assert!(est_plein(&io::Error::from_raw_os_error(39)));
        assert!(!est_plein(&io::Error::from_raw_os_error(32)));
    }

    #[test]
    fn pourcent_arrondi_au_dessous() {
        let a = Avancement { recu: 199, total: 200, ..Avancement::default() };
        assert_eq!(a.pourcent(), 99);
        assert_eq!(Avancement::default().pourcent(), 0);
    }
}
