//! `wifi.oublier` : les Wi-Fi enregistrés, lus et oubliés par l'API WLAN de Windows (wlanapi.dll), jamais par netsh
//! (sa sortie change avec la langue de Windows). wlanapi.dll est chargée de System32 au moment de s'en servir, jamais
//! liée à l'exe : sur un PC sans Wi-Fi (ou un Windows Server sans elle), l'Aide démarre quand même, et la réponse est
//! « pas de Wi-Fi sur ce PC ».

use super::outils::{fonction, guid_texte, texte};
use crate::systeme::CarteWifi;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WiFi::{WLAN_INTERFACE_INFO_LIST, WLAN_PROFILE_INFO_LIST};
use windows::core::{GUID, HSTRING, PCWSTR};

type Ouvrir = unsafe extern "system" fn(u32, *const core::ffi::c_void, *mut u32, *mut HANDLE) -> u32;
type Fermer = unsafe extern "system" fn(HANDLE, *const core::ffi::c_void) -> u32;
type Cartes = unsafe extern "system" fn(HANDLE, *const core::ffi::c_void, *mut *mut WLAN_INTERFACE_INFO_LIST) -> u32;
type Profils =
    unsafe extern "system" fn(HANDLE, *const GUID, *const core::ffi::c_void, *mut *mut WLAN_PROFILE_INFO_LIST) -> u32;
type Oublier = unsafe extern "system" fn(HANDLE, *const GUID, PCWSTR, *const core::ffi::c_void) -> u32;
type Liberer = unsafe extern "system" fn(*const core::ffi::c_void);

/// Les fonctions de wlanapi.dll
struct Wlan {
    ouvrir: Ouvrir,
    fermer: Fermer,
    cartes: Cartes,
    profils: Profils,
    oublier: Oublier,
    liberer: Liberer,
}

/// Le service WLAN ne tourne pas (ERROR_SERVICE_NOT_ACTIVE) ou n'existe pas (ERROR_SERVICE_DOES_NOT_EXIST)
const SANS_SERVICE: [u32; 2] = [1062, 1060];

fn wlan() -> Option<Wlan> {
    let f = |n: &[u8]| fonction("wlanapi.dll", n);
    // SAFETY : chaque fonction de wlanapi.dll prise avec la signature que donne la documentation de Windows
    unsafe {
        Some(Wlan {
            ouvrir: std::mem::transmute::<unsafe extern "system" fn() -> isize, Ouvrir>(f(b"WlanOpenHandle\0")?),
            fermer: std::mem::transmute::<unsafe extern "system" fn() -> isize, Fermer>(f(b"WlanCloseHandle\0")?),
            cartes: std::mem::transmute::<unsafe extern "system" fn() -> isize, Cartes>(f(b"WlanEnumInterfaces\0")?),
            profils: std::mem::transmute::<unsafe extern "system" fn() -> isize, Profils>(f(b"WlanGetProfileList\0")?),
            oublier: std::mem::transmute::<unsafe extern "system" fn() -> isize, Oublier>(f(b"WlanDeleteProfile\0")?),
            liberer: std::mem::transmute::<unsafe extern "system" fn() -> isize, Liberer>(f(b"WlanFreeMemory\0")?),
        })
    }
}

/// Une session WLAN ouverte, refermée toute seule
struct Session<'a> {
    w: &'a Wlan,
    h: HANDLE,
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        // SAFETY : notre session, refermée une seule fois
        unsafe {
            (self.w.fermer)(self.h, std::ptr::null());
        }
    }
}

/// `Ok(None)` : pas de Wi-Fi sur ce PC (pas d'API, pas de service). `Err` : une autre erreur de Windows.
fn session(w: &Wlan) -> Result<Option<Session<'_>>, String> {
    let mut version = 0u32;
    let mut h = HANDLE::default();
    // SAFETY : version 2 de l'API (Windows Vista et après) ; la session est refermée par Session
    let r = unsafe { (w.ouvrir)(2, std::ptr::null(), &mut version, &mut h) };
    match r {
        0 => Ok(Some(Session { w, h })),
        c if SANS_SERVICE.contains(&c) => Ok(None),
        c => Err(format!("WlanOpenHandle : erreur {c}")),
    }
}

/// Les cartes Wi-Fi et leurs Wi-Fi enregistrés (`Ok(None)` : pas de Wi-Fi sur ce PC)
pub fn lire() -> Result<Option<Vec<CarteWifi>>, String> {
    let Some(w) = wlan() else { return Ok(None) };
    let Some(s) = session(&w)? else { return Ok(None) };
    let mut liste: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
    // SAFETY : la liste est rendue par Windows, lue dans ses limites (dwNumberOfItems), puis libérée par WlanFreeMemory
    unsafe {
        let r = (w.cartes)(s.h, std::ptr::null(), &mut liste);
        if r != 0 {
            return Err(format!("WlanEnumInterfaces : erreur {r}"));
        }
        let n = (*liste).dwNumberOfItems as usize;
        let infos = std::slice::from_raw_parts((*liste).InterfaceInfo.as_ptr(), n);
        let mut cartes = Vec::new();
        let mut erreur = None;
        for i in infos {
            match profils(&w, &s, &i.InterfaceGuid) {
                Ok(p) => cartes.push(CarteWifi { id: guid_texte(&i.InterfaceGuid), profils: p }),
                Err(e) => erreur = Some(e),
            }
        }
        (w.liberer)(liste.cast());
        if let Some(e) = erreur {
            return Err(e);
        }
        Ok(if cartes.is_empty() { None } else { Some(cartes) })
    }
}

fn profils(w: &Wlan, s: &Session, carte: &GUID) -> Result<Vec<String>, String> {
    let mut liste: *mut WLAN_PROFILE_INFO_LIST = std::ptr::null_mut();
    // SAFETY : comme pour la liste des cartes
    unsafe {
        let r = (w.profils)(s.h, carte, std::ptr::null(), &mut liste);
        if r != 0 {
            return Err(format!("WlanGetProfileList : erreur {r}"));
        }
        let n = (*liste).dwNumberOfItems as usize;
        let noms = std::slice::from_raw_parts((*liste).ProfileInfo.as_ptr(), n)
            .iter()
            .map(|p| texte(&p.strProfileName))
            .filter(|p| !p.is_empty())
            .collect();
        (w.liberer)(liste.cast());
        Ok(noms)
    }
}

/// Le GUID d'une carte, depuis son texte (« 0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0 »)
fn guid(t: &str) -> Option<GUID> {
    let h: String = t.chars().filter(|c| *c != '-').collect();
    if h.len() != 32 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(GUID::from_u128(u128::from_str_radix(&h, 16).ok()?))
}

/// Fait oublier ce Wi-Fi à cette carte
pub fn oublier(carte: &str, profil: &str) -> Result<(), String> {
    let w = wlan().ok_or("wlanapi.dll absente")?;
    let s = session(&w)?.ok_or("service WLAN arrêté")?;
    let g = guid(carte).ok_or("carte Wi-Fi inconnue")?;
    let nom = HSTRING::from(profil);
    // SAFETY : session ouverte, GUID et nom valables pendant l'appel
    let r = unsafe { (w.oublier)(s.h, &g, PCWSTR(nom.as_ptr()), std::ptr::null()) };
    if r == 0 { Ok(()) } else { Err(format!("WlanDeleteProfile : erreur {r}")) }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn guid_aller_retour() {
        let g = GUID::from_u128(0x0f1e2d3c_4b5a_6978_8796_a5b4c3d2e1f0);
        assert_eq!(guid(&guid_texte(&g)), Some(g));
        assert_eq!(guid("pas un guid"), None);
        assert_eq!(guid("0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1fz"), None);
    }
}
