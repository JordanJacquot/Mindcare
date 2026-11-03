//! Le seul module qui parle à Windows (et le seul où le code « unsafe » est permis) :
//! - [`WinHttp`] : le téléchargement par WinHTTP, la bibliothèque de Windows (https compris, avec les réglages de
//!   proxy du PC). Rien à installer, aucune bibliothèque de plus. Les mêmes délais que le PowerShell : 30 secondes
//!   pour se connecter, 60 secondes sans rien recevoir ;
//! - [`PlaceWindows`] : la place libre par `GetDiskFreeSpaceExW` (`[IO.DriveInfo]::AvailableFreeSpace`).
//!
//! Aucun programme n'est lancé, aucune fenêtre.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::path::Path;

use windows::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ACCESS_TYPE_NO_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle,
    WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::core::{HSTRING, PCWSTR};

use crate::telechargement::{Disque, Reponse, Transport};

/// Une poignée WinHTTP, fermée quand on la lâche. Gardée comme un nombre : les poignées WinHTTP peuvent servir
/// depuis n'importe quel fil.
struct Poignee(usize);

impl Poignee {
    fn de(p: *mut c_void) -> Result<Poignee, String> {
        if p.is_null() { Err(windows::core::Error::from_thread().message()) } else { Ok(Poignee(p as usize)) }
    }
    fn p(&self) -> *mut c_void {
        self.0 as *mut c_void
    }
}

impl Drop for Poignee {
    fn drop(&mut self) {
        // SAFETY : une poignée ouverte par WinHttpOpen, WinHttpConnect ou WinHttpOpenRequest, fermée une seule fois
        let _ = unsafe { WinHttpCloseHandle(self.p()) };
    }
}

/// Le réseau par WinHTTP
pub struct WinHttp {
    session: Poignee,
}

/// Une adresse découpée : https ?, l'hôte, le port, le chemin
fn decouper(url: &str) -> Result<(bool, String, u16, String), String> {
    let (securise, reste) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(format!("adresse inattendue : {url}"));
    };
    let (hote_port, chemin) = match reste.find('/') {
        Some(i) => (&reste[..i], &reste[i..]),
        None => (reste, "/"),
    };
    let (hote, port) = match hote_port.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| format!("adresse inattendue : {url}"))?),
        None => (hote_port, if securise { 443 } else { 80 }),
    };
    if hote.is_empty() {
        return Err(format!("adresse inattendue : {url}"));
    }
    Ok((securise, hote.to_string(), port, chemin.to_string()))
}

impl WinHttp {
    /// Avec les réglages de proxy du PC (ceux de Windows, comme les navigateurs)
    pub fn nouveau() -> Result<WinHttp, String> {
        WinHttp::ouvrir(WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY)
    }

    /// Sans proxy (les tests, avec leur faux serveur sur 127.0.0.1 : aucune recherche de proxy sur le réseau)
    pub fn sans_proxy() -> Result<WinHttp, String> {
        WinHttp::ouvrir(WINHTTP_ACCESS_TYPE_NO_PROXY)
    }

    fn ouvrir(acces: WINHTTP_ACCESS_TYPE) -> Result<WinHttp, String> {
        // SAFETY : des chaînes terminées par un zéro, vivantes pendant l'appel
        let session =
            Poignee::de(unsafe { WinHttpOpen(&HSTRING::from("Mindcare"), acces, PCWSTR::null(), PCWSTR::null(), 0) })?;
        // (résolution du nom, connexion, envoi : 30 s ; rien reçu pendant 60 s : coupure)
        // SAFETY : une session ouverte juste avant
        unsafe { WinHttpSetTimeouts(session.p(), 30_000, 30_000, 30_000, 60_000) }.map_err(|e| e.message())?;
        Ok(WinHttp { session })
    }
}

/// Une réponse WinHTTP (la demande garde sa connexion ouverte)
struct ReponseWinHttp {
    demande: Poignee,
    _connexion: Poignee,
    statut: u16,
}

impl Transport for WinHttp {
    fn ouvrir(&self, url: &str, depuis: u64) -> Result<Box<dyn Reponse>, String> {
        let (securise, hote, port, chemin) = decouper(url)?;
        // SAFETY : une session ouverte, une chaîne vivante pendant l'appel
        let connexion = Poignee::de(unsafe { WinHttpConnect(self.session.p(), &HSTRING::from(hote), port, 0) })?;
        let drapeaux = if securise { WINHTTP_FLAG_SECURE } else { WINHTTP_OPEN_REQUEST_FLAGS(0) };
        // SAFETY : une connexion ouverte, des chaînes vivantes pendant l'appel, pas de liste de types acceptés
        let demande = Poignee::de(unsafe {
            WinHttpOpenRequest(
                connexion.p(),
                &HSTRING::from("GET"),
                &HSTRING::from(chemin),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                drapeaux,
            )
        })?;
        // (la reprise : seulement la suite du fichier. WinHTTP garde cet en-tête si GitHub redirige.)
        let entetes: Vec<u16> =
            if depuis > 0 { format!("Range: bytes={depuis}-\r\n").encode_utf16().collect() } else { Vec::new() };
        // SAFETY : une demande ouverte ; les en-têtes vivent pendant l'appel ; pas de corps
        unsafe {
            WinHttpSendRequest(demande.p(), (!entetes.is_empty()).then_some(&entetes[..]), None, 0, 0, 0)
                .map_err(|e| e.message())?;
            WinHttpReceiveResponse(demande.p(), std::ptr::null_mut()).map_err(|e| e.message())?;
        }
        let mut statut = 0u32;
        let mut longueur = std::mem::size_of::<u32>() as u32;
        // SAFETY : un nombre de 4 octets, sa taille donnée
        unsafe {
            WinHttpQueryHeaders(
                demande.p(),
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some(&mut statut as *mut u32 as *mut c_void),
                &mut longueur,
                std::ptr::null_mut(),
            )
        }
        .map_err(|e| e.message())?;
        if statut != 200 && statut != 206 {
            return Err(format!("réponse {statut} du serveur"));
        }
        Ok(Box::new(ReponseWinHttp { demande, _connexion: connexion, statut: statut as u16 }))
    }
}

impl Reponse for ReponseWinHttp {
    fn statut(&self) -> u16 {
        self.statut
    }

    fn lire(&mut self, tampon: &mut [u8]) -> Result<usize, String> {
        let mut lu = 0u32;
        let n = tampon.len().min(u32::MAX as usize) as u32;
        // SAFETY : le tampon est à nous, de `n` octets au moins
        unsafe { WinHttpReadData(self.demande.p(), tampon.as_mut_ptr() as *mut c_void, n, &mut lu) }
            .map_err(|e| e.message())?;
        Ok(lu as usize)
    }
}

/// La place libre par Windows
pub struct PlaceWindows;

impl Disque for PlaceWindows {
    fn place_libre(&self, dossier: &Path) -> Option<u64> {
        // (le dossier n'existe peut-être pas encore : le premier dossier parent qui existe, sur le même disque)
        let existant = dossier.ancestors().find(|p| p.is_dir())?;
        let mut libre = 0u64;
        // SAFETY : un chemin terminé par un zéro, un nombre à nous
        unsafe { GetDiskFreeSpaceExW(&HSTRING::from(existant.as_os_str()), Some(&mut libre), None, None) }.ok()?;
        Some(libre)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses_decoupees() {
        assert_eq!(
            decouper("https://github.com/a/b.zip").unwrap(),
            (true, "github.com".into(), 443, "/a/b.zip".into())
        );
        assert_eq!(decouper("http://127.0.0.1:8123/x").unwrap(), (false, "127.0.0.1".into(), 8123, "/x".into()));
        assert!(decouper("ftp://x/y").is_err());
        assert!(decouper("http://:80/y").is_err());
    }

    #[test]
    fn place_libre_lue() {
        assert!(PlaceWindows.place_libre(&std::env::temp_dir().join("pas-encore-la\\sous")).is_some_and(|p| p > 0));
    }
}
