//! Repris de D:\Projets\Nevermind\rust\nevermind-aide\src\windows\services.rs (plan de l'Aide de Mindcare,
//! 14/11/2026) : lire, arrêter et démarrer un service de Windows (ceux des mises à jour, `maj.vider`). Le type de
//! démarrage n'est jamais changé, un service n'est jamais tué, et ceux qui en dépendent ne sont jamais arrêtés (un
//! service qui en a encore en marche ne s'arrête pas : la réparation abandonne proprement).

use super::outils::message;
use std::time::{Duration, Instant};
use windows::Win32::System::Services::{
    CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_HANDLE,
    SC_MANAGER_CONNECT, SC_STATUS_PROCESS_INFO, SERVICE_CONTROL_STOP, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
    SERVICE_START, SERVICE_STATUS, SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_PROCESS, SERVICE_STOP, SERVICE_STOPPED,
    StartServiceW,
};
use windows::core::{HSTRING, PCWSTR};

/// L'attente d'un arrêt ou d'un démarrage, au plus
const ATTENTE: Duration = Duration::from_secs(60);

/// Une poignée du gestionnaire de services, refermée toute seule
struct Sc(SC_HANDLE);

impl Drop for Sc {
    fn drop(&mut self) {
        // SAFETY : poignée ouverte par nous
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}

fn ouvrir(nom: &str, acces: u32) -> Result<Sc, String> {
    // SAFETY : connexion au gestionnaire de services de ce PC, puis ouverture d'un service par son nom
    unsafe {
        let scm = Sc(OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT).map_err(|e| message(&e))?);
        OpenServiceW(scm.0, &HSTRING::from(nom), acces).map(Sc).map_err(|e| {
            if e.code().0 as u32 == 0x8007_0424 {
                format!("service {nom} introuvable")
            } else {
                format!("service {nom} : {}", message(&e))
            }
        })
    }
}

fn etat(s: &Sc) -> Option<SERVICE_STATUS_CURRENT_STATE> {
    let mut b = vec![0u8; size_of::<SERVICE_STATUS_PROCESS>()];
    let mut n = 0u32;
    // SAFETY : tampon de la taille de SERVICE_STATUS_PROCESS
    unsafe {
        QueryServiceStatusEx(s.0, SC_STATUS_PROCESS_INFO, Some(&mut b), &mut n).ok()?;
        Some((*(b.as_ptr() as *const SERVICE_STATUS_PROCESS)).dwCurrentState)
    }
}

/// Attend que le service soit dans l'état voulu (60 s au plus)
fn attendre(s: &Sc, voulu: SERVICE_STATUS_CURRENT_STATE) -> bool {
    let fin = Instant::now() + ATTENTE;
    while Instant::now() < fin {
        if etat(s) == Some(voulu) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

/// Le service tourne-t-il (ou est-il en train de démarrer) ?
pub fn tourne(nom: &str) -> Result<bool, String> {
    let s = ouvrir(nom, SERVICE_QUERY_STATUS)?;
    match etat(&s) {
        Some(e) => Ok(e != SERVICE_STOPPED),
        None => Err(format!("service {nom} illisible")),
    }
}

/// Arrête le service et attend qu'il soit arrêté
pub fn arreter(nom: &str) -> Result<(), String> {
    let s = ouvrir(nom, SERVICE_STOP | SERVICE_QUERY_STATUS)?;
    if etat(&s) == Some(SERVICE_STOPPED) {
        return Ok(());
    }
    let mut st = SERVICE_STATUS::default();
    // SAFETY : demande d'arrêt (0x80070426 : le service ne tourne déjà plus)
    if let Err(e) = unsafe { ControlService(s.0, SERVICE_CONTROL_STOP, &mut st) }
        && e.code().0 as u32 != 0x8007_0426
    {
        return Err(format!("service {nom} : {}", message(&e)));
    }
    if !attendre(&s, SERVICE_STOPPED) {
        return Err(format!("service {nom} toujours en marche après {} s", ATTENTE.as_secs()));
    }
    Ok(())
}

/// Démarre le service et attend qu'il tourne
pub fn demarrer(nom: &str) -> Result<(), String> {
    let s = ouvrir(nom, SERVICE_START | SERVICE_QUERY_STATUS)?;
    // SAFETY : démarrage sans argument (0x80070420 : il tourne déjà)
    if let Err(e) = unsafe { StartServiceW(s.0, None) }
        && e.code().0 as u32 != 0x8007_0420
    {
        return Err(format!("service {nom} : {}", message(&e)));
    }
    if !attendre(&s, SERVICE_RUNNING) {
        return Err(format!("service {nom} pas démarré après {} s", ATTENTE.as_secs()));
    }
    Ok(())
}
