//! `energie.par-defaut` : `powercfg -restoredefaultschemes` (le programme de Windows, par son chemin dans System32,
//! sans fenêtre, avec un environnement neuf), et le mode d'alimentation actif (`PowerGetActiveScheme`).

use super::outils::{MemoireLocale, guid_texte};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows::Win32::System::Power::PowerGetActiveScheme;
use windows::core::GUID;

/// Aucune fenêtre de console pour le programme lancé
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// powercfg a ce temps pour finir ; au-delà, il est arrêté
const LIMITE: Duration = Duration::from_secs(30);

/// Lance `powercfg -restoredefaultschemes` et rend son code de sortie
pub fn par_defaut(system32: &str, environnement: &[(String, String)]) -> Result<i32, String> {
    let mut enfant = Command::new(format!("{system32}\\powercfg.exe"))
        .arg("-restoredefaultschemes")
        .env_clear()
        .envs(environnement.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .current_dir(system32)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("powercfg non lancé ({e})"))?;
    let debut = Instant::now();
    loop {
        match enfant.try_wait() {
            Ok(Some(s)) => return s.code().ok_or_else(|| "powercfg sans code de sortie".to_string()),
            Ok(None) => {}
            Err(e) => return Err(format!("powercfg : {e}")),
        }
        if debut.elapsed() >= LIMITE {
            let _ = enfant.kill();
            let _ = enfant.wait();
            return Err(format!("powercfg trop long (plus de {} s), arrêté", LIMITE.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Le mode d'alimentation actif (son GUID)
pub fn actif() -> Option<String> {
    let mut g: *mut GUID = std::ptr::null_mut();
    // SAFETY : Windows rend un GUID alloué par LocalAlloc, lu puis libéré par MemoireLocale
    unsafe {
        if PowerGetActiveScheme(None, &mut g).0 != 0 || g.is_null() {
            return None;
        }
        let _m = MemoireLocale(g.cast());
        Some(guid_texte(&*g))
    }
}
