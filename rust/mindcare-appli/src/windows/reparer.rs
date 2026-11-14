//! « Le faire pour moi » pour de vrai (l'exe livré seulement : la construction d'essai et les tests ont des faux, qui
//! ne lancent jamais rien) :
//!
//! - [`ExecuteurWindows`] : lance un programme de la liste fermée (src\reparer.rs) par son chemin absolu (System32,
//!   ou winget dans `%LOCALAPPDATA%\Microsoft\WindowsApps`), sans fenêtre (CREATE_NO_WINDOW), sans rien à lire ni à
//!   écrire, attend sa fin au plus le temps permis (au-delà, il est arrêté) et lit son code de sortie. Toujours appelé
//!   sur le fil de fond de l'action, jamais sur celui de la fenêtre ;
//! - [`ouvrir_windows_update`] : la page Windows Update des Paramètres (`ms-settings:windowsupdate`), quand la
//!   personne clique sur « Ouvrir Windows Update ».

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::w;

use crate::reparer::{Executeur, Geste, Programme};

/// Aucune fenêtre de console pour le programme lancé.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Le temps entre deux regards sur le programme lancé.
const PAS: Duration = Duration::from_millis(200);

/// Le dossier System32 de Windows (`GetSystemDirectoryW`).
fn system32() -> Result<PathBuf, String> {
    let mut b = vec![0u16; 1024];
    // SAFETY : le tampon et sa taille vont ensemble ; Windows y écrit au plus sa taille
    let n = unsafe { GetSystemDirectoryW(Some(&mut b)) } as usize;
    if n == 0 || n >= b.len() {
        return Err("dossier System32 introuvable".into());
    }
    Ok(PathBuf::from(String::from_utf16_lossy(&b[..n])))
}

/// Le chemin absolu du programme.
fn chemin(p: Programme) -> Result<PathBuf, String> {
    match p {
        Programme::Systeme(exe) => Ok(system32()?.join(exe)),
        Programme::Winget => {
            let local = std::env::var_os("LOCALAPPDATA").filter(|x| !x.is_empty()).ok_or("LOCALAPPDATA absent")?;
            Ok(PathBuf::from(local).join("Microsoft").join("WindowsApps").join("winget.exe"))
        }
    }
}

/// Le vrai lanceur des gestes de « Le faire pour moi ».
pub struct ExecuteurWindows;

impl Executeur for ExecuteurWindows {
    fn lancer(&self, g: &Geste) -> Result<i32, String> {
        let exe = chemin(g.programme)?;
        let nom = exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // (winget est un raccourci d'appli de Windows : il existe sans être un fichier ordinaire, d'où
        // symlink_metadata)
        if std::fs::symlink_metadata(&exe).is_err() {
            return Err(format!("{nom} introuvable"));
        }
        let mut enfant = Command::new(&exe)
            .args(g.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("{nom} non lancé ({e})"))?;
        let debut = Instant::now();
        loop {
            match enfant.try_wait() {
                Ok(Some(s)) => return s.code().ok_or_else(|| format!("{nom} sans code de sortie")),
                Ok(None) => {}
                Err(e) => return Err(format!("{nom} : {e}")),
            }
            if debut.elapsed() >= g.delai {
                let _ = enfant.kill();
                let _ = enfant.wait();
                return Err(format!("{nom} trop long (plus de {} s), arrêté", g.delai.as_secs()));
            }
            std::thread::sleep(PAS);
        }
    }
}

/// Ouvre la page Windows Update des Paramètres.
pub fn ouvrir_windows_update() -> Result<(), String> {
    // SAFETY : textes constants terminés par un zéro, aucune fenêtre parente
    let r = unsafe { ShellExecuteW(None, w!("open"), w!("ms-settings:windowsupdate"), None, None, SW_SHOWNORMAL) };
    // (plus de 32 : ouvert ; sinon le code d'erreur de ShellExecute)
    let code = r.0 as isize;
    if code > 32 { Ok(()) } else { Err(format!("Paramètres non ouverts (code {code})")) }
}
