//! Repris de D:\Projets\Nevermind\rust\nevermind-aide\src\windows\processus.rs (plan de l'Aide de Mindcare,
//! 14/11/2026), seulement ce dont l'Aide de Mindcare se sert : le chemin et le compte d'un processus (qui appelle, et
//! Mindcare est-il ouvert), la liste des processus, et le compte de l'Aide elle-même.

use super::outils::{MemoireLocale, Poignee};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Security::Authorization::{ConvertSidToStringSidW, ConvertStringSidToSidW};
use windows::Win32::Security::{CheckTokenMembership, GetTokenInformation, PSID, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::core::{BOOL, HSTRING, PWSTR};

fn ouvrir(pid: i64) -> Option<Poignee> {
    let pid = u32::try_from(pid).ok()?;
    // SAFETY : simple ouverture en lecture limitée, la poignée est refermée par Poignee
    unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok().map(Poignee) }
}

/// Le chemin complet du programme d'un processus (vide si illisible)
pub fn chemin(pid: i64) -> String {
    let Some(h) = ouvrir(pid) else { return String::new() };
    let mut b = vec![0u16; 32768];
    let mut n = b.len() as u32;
    // SAFETY : tampon et taille cohérents
    match unsafe { QueryFullProcessImageNameW(h.0, PROCESS_NAME_WIN32, PWSTR(b.as_mut_ptr()), &mut n) } {
        Ok(()) => String::from_utf16_lossy(&b[..n as usize]),
        Err(_) => String::new(),
    }
}

/// Un SID en texte (« S-1-5-18 »)
pub fn sid_texte(sid: PSID) -> Option<String> {
    let mut p = PWSTR::null();
    // SAFETY : `sid` est un SID valable ; le texte rendu est libéré par MemoireLocale
    unsafe {
        ConvertSidToStringSidW(sid, &mut p).ok()?;
        let _m = MemoireLocale(p.0.cast());
        p.to_string().ok()
    }
}

/// Le compte (SID) d'un jeton
fn compte_du_jeton(jeton: HANDLE) -> Option<String> {
    let mut n = 0u32;
    // SAFETY : premier appel pour connaître la taille, second avec un tampon aligné de cette taille, qui contient alors
    // un TOKEN_USER rempli par Windows
    unsafe {
        let _ = GetTokenInformation(jeton, TokenUser, None, 0, &mut n);
        if n == 0 {
            return None;
        }
        let mut b = vec![0u64; (n as usize).div_ceil(8)];
        GetTokenInformation(jeton, TokenUser, Some(b.as_mut_ptr().cast()), n, &mut n).ok()?;
        let u = &*(b.as_ptr() as *const TOKEN_USER);
        sid_texte(u.User.Sid)
    }
}

/// Le compte d'un processus (SID), vide si Windows ne le dit pas
pub fn compte(pid: i64) -> String {
    let Some(h) = ouvrir(pid) else { return String::new() };
    let mut j = HANDLE::default();
    // SAFETY : lecture seule du jeton d'un processus ouvert, refermé par Poignee
    if unsafe { OpenProcessToken(h.0, TOKEN_QUERY, &mut j) }.is_err() {
        return String::new();
    }
    let j = Poignee(j);
    compte_du_jeton(j.0).unwrap_or_default()
}

/// Les numéros des processus en cours
pub fn liste() -> Vec<i64> {
    let mut r = Vec::new();
    // SAFETY : instantané refermé par Poignee ; structure initialisée avec sa taille
    unsafe {
        let Ok(h) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return r };
        let h = Poignee(h);
        let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(h.0, &mut e).is_ok();
        while ok {
            r.push(i64::from(e.th32ProcessID));
            ok = Process32NextW(h.0, &mut e).is_ok();
        }
    }
    r
}

/// Le compte de l'Aide, et si elle a les droits administrateur (membre actif du groupe Administrateurs)
pub fn moi() -> (String, bool) {
    let mut j = HANDLE::default();
    // SAFETY : jeton de ce processus, refermé par Poignee
    let compte = unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut j).is_ok() {
            let j = Poignee(j);
            compte_du_jeton(j.0).unwrap_or_default()
        } else {
            String::new()
        }
    };
    let mut admin = false;
    let mut sid = PSID::default();
    // SAFETY : SID créé par Windows et libéré par MemoireLocale ; vérification sur le jeton de ce fil/processus
    unsafe {
        if ConvertStringSidToSidW(&HSTRING::from("S-1-5-32-544"), &mut sid).is_ok() {
            let _m = MemoireLocale(sid.0);
            let mut membre = BOOL::default();
            admin = CheckTokenMembership(None, sid, &mut membre).is_ok() && membre.as_bool();
        }
    }
    (compte, admin)
}

#[cfg(test)]
mod essais {
    use super::*;

    /// Le vrai Windows, sans droits, sur ce processus de test lui-même (rien d'autre n'est lu ni touché)
    #[test]
    fn ce_processus_est_lu() {
        let pid = i64::from(std::process::id());
        assert!(liste().contains(&pid));
        let c = chemin(pid);
        let exe = std::env::current_exe().unwrap();
        assert!(c.eq_ignore_ascii_case(&exe.to_string_lossy()), "{c}");
        let (moi_compte, _) = moi();
        assert!(moi_compte.starts_with("S-1-5-"), "{moi_compte}");
        assert_eq!(compte(pid), moi_compte);
        assert_eq!(chemin(-1), "");
        assert_eq!(compte(-1), "");
    }
}
