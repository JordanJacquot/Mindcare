//! Le presse-papiers et le Bureau de la personne (étape 5.3) : « Copier le rapport », une commande à taper copiée, et
//! « Enregistrer le rapport sur le Bureau ». Seulement dans l'exe livré : la construction d'essai et les tests en
//! ont des faux dans leur dossier d'essai (src\effets.rs), jamais ceux de la personne.

use std::path::PathBuf;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Shell::{FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

/// Le presse-papiers est peut-être pris par un autre programme : autant d'essais (comme `Clipboard.SetText` de .NET).
const ESSAIS: u32 = 10;
const PAUSE: std::time::Duration = std::time::Duration::from_millis(50);

/// `[Environment]::GetFolderPath('Desktop')` : le Bureau de la personne (déplacé par OneDrive compris).
pub fn bureau() -> Result<PathBuf, String> {
    // SAFETY : GUID constant ; le texte rendu par Windows est lu puis rendu par CoTaskMemFree, une seule fois
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None).map_err(|e| e.message())?;
        let t = p.to_string();
        CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
        t.map(PathBuf::from).map_err(|e| e.to_string())
    }
}

/// Le texte en UTF-16 terminé par un zéro (CF_UNICODETEXT).
fn utf16(texte: &str) -> Vec<u16> {
    texte.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Copie ce texte dans le presse-papiers (`[System.Windows.Clipboard]::SetText`).
pub fn copier(texte: &str) -> Result<(), String> {
    let mut ouvert = Err("presse-papiers indisponible".to_string());
    for _ in 0..ESSAIS {
        // SAFETY : aucun propriétaire (None) ; fermé plus bas à chaque ouverture réussie
        ouvert = unsafe { OpenClipboard(None) }.map_err(|e| e.message());
        if ouvert.is_ok() {
            break;
        }
        std::thread::sleep(PAUSE);
    }
    ouvert?;
    let r = remplir(&utf16(texte));
    // SAFETY : ouvert juste au-dessus par ce fil
    let _ = unsafe { CloseClipboard() };
    r
}

/// Le presse-papiers ouvert : vidé, puis le texte posé dans un bloc de mémoire que Windows garde ensuite.
fn remplir(t: &[u16]) -> Result<(), String> {
    let octets = std::mem::size_of_val(t);
    // SAFETY : le presse-papiers est ouvert par ce fil ; le bloc alloué a la taille du texte, il est verrouillé le
    // temps de la copie (taille exacte, aucune superposition) ; donné au presse-papiers, il appartient à Windows,
    // sinon il est rendu ici.
    unsafe {
        EmptyClipboard().map_err(|e| e.message())?;
        let bloc: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, octets).map_err(|e| e.message())?;
        let p = GlobalLock(bloc) as *mut u16;
        if p.is_null() {
            let _ = GlobalFree(Some(bloc));
            return Err("mémoire du presse-papiers non verrouillée".into());
        }
        std::ptr::copy_nonoverlapping(t.as_ptr(), p, t.len());
        let _ = GlobalUnlock(bloc);
        if let Err(e) = SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(bloc.0))) {
            let _ = GlobalFree(Some(bloc));
            return Err(e.message());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn texte_termine_par_zero() {
        assert_eq!(super::utf16("é"), vec![0xE9, 0]);
        assert_eq!(super::utf16(""), vec![0]);
    }
}
