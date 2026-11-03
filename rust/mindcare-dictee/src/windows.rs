//! Le seul module qui parle à Windows (et le seul où le code « unsafe » est permis) :
//! - [`MaterielWindows`] et [`MicroWindows`] : le micro par waveIn, comme `MindcareMicro` (natif.cs) : la liste des
//!   micros, puis l'enregistrement en 16 kHz, 16 bits, mono, par tampons de 100 ms, sur un fil à part ;
//! - [`LanceurWindows`] : whisper-cli lancé sans fenêtre (CREATE_NO_WINDOW), relié à un Job Object « arrêté avec
//!   Mindcare » (`ArreterAvecMindcare`) : il meurt avec elle, même fermée de force ;
//! - [`processus_vivant`] : un programme porte encore ce numéro (fichiers de voix orphelins).
//!
//! Les tests n'ouvrent JAMAIS le vrai micro : ils donnent leur propre [`crate::Materiel`].

#![allow(unsafe_code)]

use std::ffi::{OsString, c_void};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::Audio::{
    CALLBACK_EVENT, HWAVEIN, WAVE_FORMAT_PCM, WAVEFORMATEX, WAVEHDR, WAVEINCAPSW, WHDR_DONE, waveInAddBuffer,
    waveInClose, waveInGetDevCapsW, waveInGetNumDevs, waveInOpen, waveInPrepareHeader, waveInReset, waveInStart,
    waveInUnprepareHeader,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows::Win32::System::Threading::{
    CreateEventW, GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, SetEvent, WaitForSingleObject,
};
use windows::core::PCWSTR;

use crate::dictee::{Lance, Lanceur, Materiel, Micro};
use crate::son::{Analyse, FREQUENCE, Mesures, SON_MAX, TAMPON};

/// Aucune fenêtre, même un instant (CREATE_NO_WINDOW)
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Le code d'une erreur de waveIn sans code de Windows (MMSYSERR_ERROR)
const ERREUR_MICRO: u32 = 1;
/// Le nombre de tampons donnés au micro (4 x 100 ms, comme MindcareMicro)
const TAMPONS: usize = 4;

/// Les micros du PC, par waveIn
pub struct MaterielWindows;

impl Materiel for MaterielWindows {
    fn micro_present(&self) -> bool {
        // SAFETY : aucune donnée passée, rend seulement un nombre
        unsafe { waveInGetNumDevs() > 0 }
    }

    fn noms(&self) -> Vec<String> {
        // SAFETY : aucune donnée passée, rend seulement un nombre
        let n = unsafe { waveInGetNumDevs() };
        (0..n)
            .map(|d| {
                let mut c = WAVEINCAPSW::default();
                // SAFETY : `c` est une structure WAVEINCAPSW à nous, de la taille annoncée ; Windows ne fait qu'y
                // écrire le nom et les capacités du micro numéro `d` (aucun micro n'est ouvert)
                let r = unsafe { waveInGetDevCapsW(d as usize, &mut c, size_of::<WAVEINCAPSW>() as u32) };
                if r != 0 {
                    return String::new(); // (un nom illisible vaut « », pour que les numéros restent les bons)
                }
                let nom = c.szPname;
                let fin = nom.iter().position(|&x| x == 0).unwrap_or(nom.len());
                String::from_utf16_lossy(&nom[..fin])
            })
            .collect()
    }

    fn nouveau_micro(&self) -> Box<dyn Micro> {
        Box::new(MicroWindows::default())
    }
}

/// Un événement de Windows (le micro le lève quand un tampon est plein), fermé quand on le lâche
struct Evenement(isize);

impl Evenement {
    fn poignee(&self) -> HANDLE {
        HANDLE(self.0 as *mut c_void)
    }
}

impl Drop for Evenement {
    fn drop(&mut self) {
        // SAFETY : une poignée ouverte par CreateEventW, fermée une seule fois, après la fin du fil qui s'en sert
        let _ = unsafe { CloseHandle(self.poignee()) };
    }
}

/// Ce que le fil du micro et la dictée partagent
struct Partage {
    analyse: Mutex<Analyse>,
    pcm: Mutex<Vec<u8>>,
    encore: AtomicBool,
    evenement: Evenement,
}

/// Le micro par waveIn (`MindcareMicro`)
#[derive(Default)]
pub struct MicroWindows {
    partage: Option<Arc<Partage>>,
    fil: Option<JoinHandle<()>>,
}

/// Un en-tête de tampon, aligné sur 8 octets (WAVEHDR est déclaré « packed » : sans cela, ses champs pourraient ne pas
/// être alignés, et la lecture « volatile » le demande)
#[repr(C, align(8))]
struct Entete(WAVEHDR);

impl Micro for MicroWindows {
    fn demarrer(&mut self, numero: u32) -> Result<(), u32> {
        if self.fil.is_some() {
            return Err(ERREUR_MICRO);
        }
        // SAFETY : un événement sans nom ni attributs, à remise automatique ; sa poignée est gardée par `Evenement`
        let ev = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }.map_err(|_| ERREUR_MICRO)?;
        let partage = Arc::new(Partage {
            analyse: Mutex::new(Analyse::default()),
            pcm: Mutex::new(Vec::new()),
            encore: AtomicBool::new(true),
            evenement: Evenement(ev.0 as isize),
        });
        let (tx, rx) = mpsc::channel();
        let p = partage.clone();
        let fil = std::thread::Builder::new()
            .name("micro".into())
            .spawn(move || enregistrer(numero, &p, &tx))
            .map_err(|_| ERREUR_MICRO)?;
        let code = rx.recv().unwrap_or(ERREUR_MICRO);
        if code != 0 {
            let _ = fil.join();
            return Err(code);
        }
        self.partage = Some(partage);
        self.fil = Some(fil);
        Ok(())
    }

    fn mesures(&self) -> Mesures {
        self.partage.as_ref().map(|p| p.analyse.lock().unwrap_or_else(|e| e.into_inner()).mesures).unwrap_or_default()
    }

    fn arreter(&mut self) {
        if let Some(p) = &self.partage {
            p.encore.store(false, Ordering::SeqCst);
            // SAFETY : l'événement est encore ouvert (gardé par `p`)
            let _ = unsafe { SetEvent(p.evenement.poignee()) };
        }
        if let Some(f) = self.fil.take() {
            let _ = f.join();
        }
    }

    fn son(&self) -> Vec<u8> {
        self.partage.as_ref().map(|p| p.pcm.lock().unwrap_or_else(|e| e.into_inner()).clone()).unwrap_or_default()
    }
}

impl Drop for MicroWindows {
    fn drop(&mut self) {
        self.arreter();
    }
}

/// Le fil du micro : ouvre le micro, rend 0 (ou le code de Windows) par `tx`, puis range le son de chaque tampon
/// plein jusqu'à l'arrêt ; enfin rend les tampons et ferme le micro. Tout ce que le micro touche (en-têtes et
/// tampons) vit ici, à une adresse fixe, jusqu'à sa fermeture.
fn enregistrer(numero: u32, p: &Partage, tx: &mpsc::Sender<u32>) {
    let format = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM as u16,
        nChannels: 1,
        nSamplesPerSec: FREQUENCE,
        nAvgBytesPerSec: FREQUENCE * 2,
        nBlockAlign: 2,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    let ev = p.evenement.poignee();
    let mut h = HWAVEIN::default();
    // SAFETY : `h` et `format` sont à nous et vivent pendant l'appel ; l'événement reste ouvert tant que `p` vit, et
    // `p` vit plus longtemps que le micro (fermé à la fin de cette fonction)
    let r = unsafe { waveInOpen(Some(&raw mut h), numero, &format, Some(ev.0 as usize), None, CALLBACK_EVENT) };
    if r != 0 {
        let _ = tx.send(r);
        return;
    }
    let taille = size_of::<WAVEHDR>() as u32;
    let entetes: Vec<*mut Entete> = (0..TAMPONS)
        .map(|_| {
            let donnees = Box::into_raw(vec![0u8; TAMPON].into_boxed_slice()) as *mut u8;
            let e =
                WAVEHDR { lpData: windows::core::PSTR(donnees), dwBufferLength: TAMPON as u32, ..Default::default() };
            Box::into_raw(Box::new(Entete(e)))
        })
        .collect();
    for &e in &entetes {
        // SAFETY : `e` pointe vers un en-tête à nous, aligné, avec son tampon de TAMPON octets ; ni l'un ni l'autre
        // ne bouge ni n'est libéré avant waveInClose
        unsafe {
            waveInPrepareHeader(h, &raw mut (*e).0, taille);
            waveInAddBuffer(h, &raw mut (*e).0, taille);
        }
    }
    // SAFETY : `h` est le micro ouvert ci-dessus
    let r = unsafe { waveInStart(h) };
    let _ = tx.send(r);
    while r == 0 && p.encore.load(Ordering::SeqCst) {
        // SAFETY : l'événement est ouvert tant que `p` vit
        let _ = unsafe { WaitForSingleObject(ev, 200) };
        for &e in &entetes {
            if !p.encore.load(Ordering::SeqCst) {
                break;
            }
            // SAFETY : l'en-tête est à nous (voir plus haut) ; Windows y écrit depuis un autre fil, d'où la lecture
            // « volatile » de ses champs (alignés : `Entete` est aligné sur 8, et dwFlags est à l'octet 24)
            let fait = unsafe { std::ptr::read_volatile(&raw const (*e).0.dwFlags) } & WHDR_DONE != 0;
            if !fait {
                continue;
            }
            std::sync::atomic::fence(Ordering::Acquire);
            // SAFETY : le tampon est rendu par Windows (WHDR_DONE) : il n'y écrit plus jusqu'au prochain
            // waveInAddBuffer ; on n'en lit que les octets enregistrés, au plus TAMPON
            let octets = unsafe {
                let n = (std::ptr::read_volatile(&raw const (*e).0.dwBytesRecorded) as usize).min(TAMPON);
                std::slice::from_raw_parts((*e).0.lpData.0, n).to_vec()
            };
            if !octets.is_empty() {
                let mut pcm = p.pcm.lock().unwrap_or_else(|x| x.into_inner());
                if pcm.len() + octets.len() <= SON_MAX {
                    pcm.extend_from_slice(&octets);
                }
                drop(pcm);
                p.analyse.lock().unwrap_or_else(|x| x.into_inner()).ajouter(&octets);
            }
            // SAFETY : le tampon est rendu, on le redonne au micro (mêmes adresses, toujours à nous)
            unsafe {
                waveInUnprepareHeader(h, &raw mut (*e).0, taille);
                std::ptr::write_volatile(&raw mut (*e).0.dwFlags, 0);
                std::ptr::write_volatile(&raw mut (*e).0.dwBytesRecorded, 0);
                waveInPrepareHeader(h, &raw mut (*e).0, taille);
                waveInAddBuffer(h, &raw mut (*e).0, taille);
            }
        }
    }
    // SAFETY : waveInReset rend tous les tampons ; ils sont détachés puis le micro est fermé : Windows ne les touche
    // plus, on peut libérer les en-têtes et les tampons (créés par Box::into_raw ci-dessus, libérés une seule fois)
    unsafe {
        waveInReset(h);
        for &e in &entetes {
            waveInUnprepareHeader(h, &raw mut (*e).0, taille);
        }
        waveInClose(h);
        for e in entetes {
            let e = Box::from_raw(e);
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(e.0.lpData.0, TAMPON)));
        }
    }
}

/// Lance whisper-cli sans fenêtre, relié à Mindcare
pub struct LanceurWindows;

impl Lanceur for LanceurWindows {
    fn lancer(&self, programme: &Path, arguments: &[OsString], dossier: &Path) -> Result<Lance, String> {
        let enfant = Command::new(programme)
            .args(arguments)
            .current_dir(dossier)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
        let relie = arreter_avec_mindcare(&enfant);
        Ok(Lance { processus: Box::new(enfant), relie })
    }

    fn vivant(&self, pid: u32) -> bool {
        processus_vivant(pid)
    }
}

/// `ArreterAvecMindcare` : le programme rejoint un Job Object « tué à la fermeture », créé une fois et jamais fermé à
/// la main : Windows le ferme quand Mindcare s'arrête (même tuée), et tout ce qu'il contient s'arrête avec elle.
pub fn arreter_avec_mindcare(enfant: &Child) -> Result<(), String> {
    static JOB: OnceLock<Result<isize, String>> = OnceLock::new();
    let job = JOB.get_or_init(creer_job).clone()?;
    // SAFETY : la poignée du job reste ouverte jusqu'à la fin de Mindcare ; celle du programme est gardée par `enfant`
    unsafe { AssignProcessToJobObject(HANDLE(job as *mut c_void), HANDLE(enfant.as_raw_handle())) }
        .map_err(|e| format!("ajout au job : {}", e.message()))
}

fn creer_job() -> Result<isize, String> {
    // SAFETY : un job sans nom, attributs par défaut (sa poignée n'est pas héritée par les programmes lancés)
    let j =
        unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(|e| format!("création du job : {}", e.message()))?;
    let mut l = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    l.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY : `l` est à nous, de la taille annoncée, et ne sert que pendant l'appel
    let r = unsafe {
        SetInformationJobObject(
            j,
            JobObjectExtendedLimitInformation,
            (&raw const l).cast::<c_void>(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if let Err(e) = r {
        // SAFETY : la poignée ouverte ci-dessus, fermée une seule fois
        let _ = unsafe { CloseHandle(j) };
        return Err(format!("réglage du job : {}", e.message()));
    }
    Ok(j.0 as isize)
}

/// Un programme porte encore ce numéro (lu seulement : rien n'est changé dans ce programme). Un refus d'accès veut dire
/// qu'il existe.
pub fn processus_vivant(pid: u32) -> bool {
    // SAFETY : une simple ouverture en lecture des informations du programme ; la poignée est refermée plus bas
    match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Err(e) => e.code().0 as u32 == 0x8007_0005,
        Ok(h) => {
            let mut code = 0u32;
            // SAFETY : `h` est ouverte ci-dessus, `code` est à nous
            let r = unsafe { GetExitCodeProcess(h, &mut code) };
            // SAFETY : la poignée ouverte ci-dessus, fermée une seule fois
            let _ = unsafe { CloseHandle(h) };
            r.is_err() || code == 259 // STILL_ACTIVE
        }
    }
}
