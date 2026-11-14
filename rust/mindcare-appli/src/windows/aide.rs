//! L'Aide de Mindcare vue de Mindcare (exe livré seulement : la construction d'essai et les tests ont des fausses).
//! Repris de Nevermind (D:\Projets\Nevermind\rust\nevermind-noyau\src\aide.rs et src\windows\canal.rs, côté client),
//! sans la demande de Windows (décision de Jordan du 14/11/2026) :
//!
//! - présente : MindcareAide.exe à côté de Mindset.exe, et sa tâche `\Mindset\Mindcare (aide)` lue par
//!   `schtasks /Query /XML` (sans fenêtre) : en SYSTEM, ce programme-là, pour ce compte (`-Sid`), active
//!   (`mindcare_aide_regles::tache::etat`) ;
//! - réveil : `schtasks /Run` (sans fenêtre), seulement si c'est bien cette tâche-là, puis 20 s au plus pour qu'elle
//!   écoute ;
//! - le canal `Mindcare-Aide-<sid>-<suffixe>` : connexion anonyme (l'Aide ne prend pas l'identité de Mindcare), celui
//!   qui répond vérifié (`mindcare_aide_regles::serveur::accepte` : un canal créé par le système ou les
//!   administrateurs, sinon MindcareAide.exe de ce dossier), une ligne envoyée (`{"a":"<action>"}`), une ligne lue ;
//! - après chaque action, le journal de l'Aide (`aide.journal`) : les lignes pas encore recopiées.

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use mindcare_aide_regles::demandes::Demande;
use mindcare_aide_regles::{canal, serveur, tache};
use serde_json::Value;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, GENERIC_READ, GENERIC_WRITE, HANDLE, HLOCAL, LocalFree,
};
use windows::Win32::Globalization::{CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS, MultiByteToWideChar};
use windows::Win32::Security::Authorization::{ConvertSidToStringSidW, GetSecurityInfo, SE_KERNEL_OBJECT};
use windows::Win32::Security::{
    GetTokenInformation, LookupAccountNameW, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SID_NAME_USE,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_NONE, FlushFileBuffers, OPEN_EXISTING, ReadFile,
    SECURITY_ANONYMOUS, SECURITY_SQOS_PRESENT, WriteFile,
};
use windows::Win32::System::IO::CancelIoEx;
use windows::Win32::System::Pipes::{GetNamedPipeServerProcessId, WaitNamedPipeW};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};

use crate::reparer::AideAdmin;

/// CREATE_NO_WINDOW : un programme lancé par Mindcare n'ouvre jamais de fenêtre
const SANS_FENETRE: u32 = 0x0800_0000;
/// L'attente de l'Aide réveillée par sa tâche
const ATTENTE_TACHE: Duration = Duration::from_secs(20);
/// Le temps pour se connecter au canal, et pour lire le journal de l'Aide
const CONNEXION_MS: u32 = 3000;
const JOURNAL_MS: u32 = 5000;

/// Ce que dit Mindcare quand l'Aide ne peut pas être jointe (le détail va au journal)
pub const NE_REPOND_PAS: &str = "L'Aide de Mindcare ne répond pas. Redémarre le PC, puis réessaie.";
/// Ce que dit Mindcare quand la tâche de l'Aide n'est pas (ou plus) la bonne
pub const PAS_INSTALLEE: &str = "L'Aide de Mindcare n'est pas prête sur ce PC. Ouvre Mindset et mets Mindcare à jour.";

/// Une poignée de Windows, refermée toute seule
struct Poignee(HANDLE);

impl Drop for Poignee {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY : la poignée nous appartient et n'est refermée qu'ici
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Une mémoire donnée par Windows (LocalAlloc), rendue toute seule
struct MemoireLocale(*mut core::ffi::c_void);

impl Drop for MemoireLocale {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY : mémoire allouée par Windows avec LocalAlloc, rendue une seule fois
            unsafe {
                let _ = LocalFree(Some(HLOCAL(self.0)));
            }
        }
    }
}

fn message(e: &windows::core::Error) -> String {
    let m = e.message();
    let m = m.trim();
    if m.is_empty() { format!("erreur 0x{:08x}", e.code().0) } else { m.to_string() }
}

/// Un SID en texte (« S-1-5-18 »)
fn sid_texte(sid: PSID) -> Option<String> {
    let mut p = PWSTR::null();
    // SAFETY : `sid` est un SID valable ; le texte rendu est libéré par MemoireLocale
    unsafe {
        ConvertSidToStringSidW(sid, &mut p).ok()?;
        let _m = MemoireLocale(p.0.cast());
        p.to_string().ok()
    }
}

/// Le SID du compte qui fait tourner Mindcare (vide si illisible)
fn sid_utilisateur() -> String {
    let mut j = HANDLE::default();
    // SAFETY : jeton de ce processus en lecture, refermé par Poignee ; tampon aligné de la taille demandée par Windows,
    // qui contient alors un TOKEN_USER
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut j).is_err() {
            return String::new();
        }
        let j = Poignee(j);
        let mut n = 0u32;
        let _ = GetTokenInformation(j.0, TokenUser, None, 0, &mut n);
        if n == 0 {
            return String::new();
        }
        let mut b = vec![0u64; (n as usize).div_ceil(8)];
        if GetTokenInformation(j.0, TokenUser, Some(b.as_mut_ptr().cast()), n, &mut n).is_err() {
            return String::new();
        }
        let u = &*(b.as_ptr() as *const TOKEN_USER);
        sid_texte(u.User.Sid).unwrap_or_default()
    }
}

/// Le SID d'un nom de compte (« SYSTEM »), pour une tâche dont le compte est écrit par son nom
fn traduire_compte(nom: &str) -> Option<String> {
    let n = HSTRING::from(nom);
    let mut taille_sid = 0u32;
    let mut taille_domaine = 0u32;
    let mut usage = SID_NAME_USE::default();
    // SAFETY : premier appel pour les tailles, second avec des tampons de ces tailles
    unsafe {
        let _ = LookupAccountNameW(PCWSTR::null(), &n, None, &mut taille_sid, None, &mut taille_domaine, &mut usage);
        if taille_sid == 0 {
            return None;
        }
        let mut sid = vec![0u64; (taille_sid as usize).div_ceil(8)];
        let mut domaine = vec![0u16; taille_domaine as usize + 1];
        LookupAccountNameW(
            PCWSTR::null(),
            &n,
            Some(PSID(sid.as_mut_ptr().cast())),
            &mut taille_sid,
            Some(PWSTR(domaine.as_mut_ptr())),
            &mut taille_domaine,
            &mut usage,
        )
        .ok()?;
        sid_texte(PSID(sid.as_mut_ptr().cast()))
    }
}

/// Le dossier System32
fn system32() -> String {
    let mut b = vec![0u16; 1024];
    // SAFETY : tampon et taille cohérents
    let n = unsafe { GetSystemDirectoryW(Some(&mut b)) } as usize;
    String::from_utf16_lossy(&b[..n.min(b.len())])
}

/// Une sortie de schtasks (page de code OEM de la console) en texte
fn texte_oem(o: &[u8]) -> String {
    if o.is_empty() {
        return String::new();
    }
    // SAFETY : premier appel pour la taille, second avec un tampon de cette taille
    unsafe {
        let n = MultiByteToWideChar(CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), o, None);
        if n <= 0 {
            return String::from_utf8_lossy(o).into_owned();
        }
        let mut b = vec![0u16; n as usize];
        let n = MultiByteToWideChar(CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), o, Some(&mut b));
        String::from_utf16_lossy(&b[..n.max(0) as usize])
    }
}

/// Le XML de la tâche (`schtasks /Query /TN <tâche> /XML`, sans fenêtre), vide si elle n'existe pas ou est illisible
fn lire_tache(schtasks: &str) -> String {
    let Ok(s) = std::process::Command::new(schtasks)
        .args(["/Query", "/TN", tache::TACHE, "/XML"])
        .creation_flags(SANS_FENETRE)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return String::new();
    };
    if !s.status.success() || s.stdout.is_empty() {
        return String::new();
    }
    texte_oem(&s.stdout).lines().collect::<Vec<_>>().join("\n")
}

/// `schtasks /Run /TN <tâche>` (sans fenêtre) : vrai si Windows l'a lancée
fn lancer_tache(schtasks: &str) -> bool {
    std::process::Command::new(schtasks)
        .args(["/Run", "/TN", tache::TACHE])
        .creation_flags(SANS_FENETRE)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn chemin_canal(nom: &str) -> HSTRING {
    HSTRING::from(format!("\\\\.\\pipe\\{nom}"))
}

/// L'Aide écoute-t-elle ? (sans s'y connecter ; `WaitNamedPipe`, au moins 1 ms)
fn joignable(nom: &str, ms: u32) -> bool {
    // SAFETY : un nom terminé par un zéro, vivant pendant l'appel
    unsafe { WaitNamedPipeW(&chemin_canal(nom), ms.max(1)).as_bool() }
}

/// Le chemin complet du programme d'un processus (vide si illisible)
fn chemin_processus(pid: u32) -> String {
    // SAFETY : ouverture en lecture limitée, refermée par Poignee ; tampon et taille cohérents
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new() };
        let h = Poignee(h);
        let mut b = vec![0u16; 32768];
        let mut n = b.len() as u32;
        match QueryFullProcessImageNameW(h.0, PROCESS_NAME_WIN32, PWSTR(b.as_mut_ptr()), &mut n) {
            Ok(()) => String::from_utf16_lossy(&b[..n as usize]),
            Err(_) => String::new(),
        }
    }
}

/// Le propriétaire (SID) d'un canal ouvert (vide si illisible)
fn proprietaire(h: HANDLE) -> String {
    let mut o = PSID::default();
    let mut psd = PSECURITY_DESCRIPTOR::default();
    // SAFETY : lecture du propriétaire ; le descripteur rendu est libéré par MemoireLocale
    unsafe {
        let r = GetSecurityInfo(
            h,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut o),
            None,
            None,
            None,
            Some(&mut psd),
        );
        let _m = MemoireLocale(psd.0);
        if r.is_err() {
            return String::new();
        }
        sid_texte(o).unwrap_or_default()
    }
}

/// Ouvre le canal (réessayé tant qu'il est occupé ou absent, `connexion_ms` au plus)
fn connecter(nom: &str, connexion_ms: u32) -> Result<Poignee, String> {
    let chemin = chemin_canal(nom);
    let fin = Instant::now() + Duration::from_millis(u64::from(connexion_ms));
    loop {
        // SAFETY : un nom terminé par un zéro ; connexion anonyme (l'Aide ne prend pas l'identité de Mindcare)
        let r = unsafe {
            CreateFileW(
                &chemin,
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(SECURITY_SQOS_PRESENT.0 | SECURITY_ANONYMOUS.0),
                None,
            )
        };
        match r {
            Ok(h) => return Ok(Poignee(h)),
            Err(e) => {
                let code = (e.code().0 & 0xFFFF) as u32;
                if code != ERROR_PIPE_BUSY.0 && code != ERROR_FILE_NOT_FOUND.0 {
                    return Err(message(&e));
                }
            }
        }
        let reste = fin.saturating_duration_since(Instant::now());
        if reste.is_zero() {
            return Err("délai de connexion dépassé".into());
        }
        if !joignable(nom, reste.as_millis().min(u128::from(u32::MAX)) as u32) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Lit une ligne (jusqu'à « \n », sans « \r »), 64 Ko au plus. `None` : l'autre a coupé sans rien envoyer.
fn lire_ligne(h: HANDLE) -> Option<String> {
    let mut tout = Vec::new();
    let mut b = [0u8; 4096];
    loop {
        let mut n = 0u32;
        // SAFETY : tampon à nous, de sa taille
        let ok = unsafe { ReadFile(h, Some(&mut b), Some(&mut n), None) }.is_ok();
        if !ok || n == 0 {
            break;
        }
        tout.extend_from_slice(&b[..n as usize]);
        if tout.contains(&b'\n') || tout.len() > 65536 {
            break;
        }
    }
    if tout.is_empty() {
        return None;
    }
    let fin = tout.iter().position(|&o| o == b'\n').unwrap_or(tout.len());
    let mut l = &tout[..fin];
    if l.last() == Some(&b'\r') {
        l = &l[..l.len() - 1];
    }
    Some(String::from_utf8_lossy(l).into_owned())
}

fn ecrire_tout(h: HANDLE, o: &[u8]) -> Result<(), String> {
    let mut fait = 0;
    while fait < o.len() {
        let mut n = 0u32;
        // SAFETY : tampon à nous, de sa taille
        unsafe { WriteFile(h, Some(&o[fait..]), Some(&mut n), None) }.map_err(|e| message(&e))?;
        if n == 0 {
            return Err("canal fermé".into());
        }
        fait += n as usize;
    }
    // SAFETY : notre poignée
    let _ = unsafe { FlushFileBuffers(h) };
    Ok(())
}

/// Une poignée gardée comme un nombre, pour le fil qui lit la réponse
#[derive(Clone, Copy)]
struct Brute(usize);

/// Envoie une ligne à l'Aide et rend sa réponse (une ligne JSON). Une erreur si l'Aide ne répond pas à temps, ou si
/// celui qui répond n'est pas elle (`exe_attendu` : MindcareAide.exe de ce dossier).
pub fn appeler(
    nom: &str,
    exe_attendu: &str,
    ligne: &str,
    connexion_ms: u32,
    reponse_ms: u32,
) -> Result<String, String> {
    let c = connecter(nom, connexion_ms)?;
    let mut pid = 0u32;
    // SAFETY : notre poignée du canal, un nombre à nous
    let serveur = if unsafe { GetNamedPipeServerProcessId(c.0, &mut pid) }.is_ok() {
        chemin_processus(pid)
    } else {
        String::new()
    };
    let proprio = proprietaire(c.0);
    if !serveur::accepte(&serveur, &proprio, exe_attendu) {
        let qui = if serveur.is_empty() {
            format!("inconnu, canal de {}", if proprio.is_empty() { "?" } else { &proprio })
        } else {
            serveur
        };
        return Err(format!("le programme qui répond à la place de l'Aide n'est pas elle ({qui})"));
    }
    ecrire_tout(c.0, format!("{ligne}\r\n").as_bytes())?;
    let brute = Brute(c.0.0 as usize);
    let (tx, rx) = mpsc::channel();
    let fil = std::thread::spawn(move || {
        let h = HANDLE(brute.0 as *mut _);
        let _ = tx.send(lire_ligne(h));
    });
    let r = match rx.recv_timeout(Duration::from_millis(u64::from(reponse_ms.max(1)))) {
        Ok(Some(l)) => Ok(l),
        Ok(None) => Err("l'Aide a coupé l'échange sans répondre".to_string()),
        Err(_) => {
            // SAFETY : notre poignée ; la lecture en cours est abandonnée avant que la poignée soit refermée
            let _ = unsafe { CancelIoEx(c.0, None) };
            Err("l'Aide n'a pas répondu à temps".to_string())
        }
    };
    let _ = fil.join();
    drop(c);
    r
}

/// La vraie Aide de Mindcare, pour la copie de Mindcare de ce dossier et le compte qui la fait tourner
pub struct AideWindows {
    exe: String,
    sid: String,
    canal: String,
    schtasks: String,
    /// La dernière ligne du journal de l'Aide déjà recopiée
    vue: Mutex<Option<String>>,
}

impl AideWindows {
    /// L'Aide de la copie de Mindcare du dossier `racine` (celui de Mindset.exe). `None` : dossier ou compte illisible.
    pub fn nouvelle(racine: &str) -> Option<AideWindows> {
        let suffixe = canal::suffixe(racine)?;
        let sid = sid_utilisateur();
        if !mindcare_aide_regles::sid::sid_utilisateur(&sid) {
            return None;
        }
        let exe = mindcare_aide_regles::chemin::joindre(racine, tache::EXE);
        Some(AideWindows {
            canal: canal::nom_canal(&sid, &suffixe),
            sid,
            exe,
            schtasks: format!("{}\\schtasks.exe", system32()),
            vue: Mutex::new(None),
        })
    }

    fn etat_tache(&self) -> &'static str {
        tache::etat(&lire_tache(&self.schtasks), &self.exe, &self.sid, &traduire_compte)
    }

    /// Les lignes du journal de l'Aide pas encore recopiées
    fn journal(&self) -> Vec<String> {
        let Ok(r) = appeler(&self.canal, &self.exe, &Demande::Journal.ligne(), CONNEXION_MS, JOURNAL_MS) else {
            return Vec::new();
        };
        let lignes: Vec<String> = serde_json::from_str::<Value>(&r)
            .ok()
            .and_then(|v| v.get("r").cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let mut vue = self.vue.lock().unwrap_or_else(|e| e.into_inner());
        let depuis = vue.as_ref().and_then(|d| lignes.iter().rposition(|l| l == d)).map_or(0, |i| i + 1);
        let nouvelles = lignes[depuis.min(lignes.len())..].to_vec();
        if let Some(l) = lignes.last() {
            *vue = Some(l.clone());
        }
        nouvelles
    }
}

impl AideAdmin for AideWindows {
    fn presente(&self) -> bool {
        Path::new(&self.exe).is_file() && self.etat_tache() == "systeme"
    }

    fn faire(&self, d: Demande, delai: Duration) -> (Result<Value, String>, Vec<String>) {
        let mut notes = Vec::new();
        // Réveillée par sa tâche, seulement si c'est bien la tâche SYSTEM de ce compte (jamais une autre)
        if !joignable(&self.canal, 200) {
            let etat = self.etat_tache();
            if etat != "systeme" {
                notes.push(format!("Mindcare : tâche de l'Aide « {etat} », pas lancée"));
                return (Err(PAS_INSTALLEE.into()), notes);
            }
            if !lancer_tache(&self.schtasks) {
                notes.push("Mindcare : la tâche de l'Aide n'a pas démarré".into());
                return (Err(NE_REPOND_PAS.into()), notes);
            }
            let fin = Instant::now() + ATTENTE_TACHE;
            while Instant::now() < fin && !joignable(&self.canal, 200) {
                std::thread::sleep(Duration::from_millis(150));
            }
        }
        let ms = u32::try_from(delai.as_millis()).unwrap_or(u32::MAX);
        let resultat = match appeler(&self.canal, &self.exe, &d.ligne(), CONNEXION_MS, ms) {
            Ok(r) => match serde_json::from_str::<Value>(&r) {
                Ok(j) if j["ok"] == Value::Bool(true) => Ok(j.get("r").cloned().unwrap_or(Value::Null)),
                Ok(j) => Err(j["e"].as_str().map(str::to_string).unwrap_or_else(|| NE_REPOND_PAS.to_string())),
                Err(e) => {
                    notes.push(format!("Mindcare : réponse de l'Aide illisible ({e})"));
                    Err(NE_REPOND_PAS.into())
                }
            },
            Err(e) => {
                notes.push(format!("Mindcare : l'Aide n'a pas été jointe ({e})"));
                Err(NE_REPOND_PAS.into())
            }
        };
        let mut lignes = self.journal();
        lignes.extend(notes);
        (resultat, lignes)
    }
}

#[cfg(test)]
mod essais {
    use super::*;
    use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    };

    /// Un faux serveur créé par le test lui-même (son nom à lui, jamais celui de la vraie Aide), qui répond « ok » :
    /// Mindcare refuse de lui parler, car ce n'est ni un canal du système ni MindcareAide.exe. Rien d'autre n'est
    /// touché sur le PC.
    #[test]
    fn un_canal_qui_n_est_pas_l_aide_est_refuse() {
        let nom = format!("Mindcare-Essai-Faux-{}", std::process::id());
        // SAFETY : un nom terminé par un zéro ; droits par défaut (ceux du compte qui fait le test)
        let h = unsafe {
            CreateNamedPipeW(
                &chemin_canal(&nom),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                4096,
                4096,
                0,
                None,
            )
        };
        assert!(!h.is_invalid());
        let serveur = Poignee(h);
        let brute = Brute(serveur.0.0 as usize);
        let fil = std::thread::spawn(move || {
            let h = HANDLE(brute.0 as *mut _);
            // SAFETY : notre exemplaire du canal, attente bloquante d'un client, puis une réponse écrite
            unsafe {
                let _ = ConnectNamedPipe(h, None);
            }
            let _ = ecrire_tout(h, b"{\"ok\":true,\"r\":\"faux\"}\r\n");
        });
        let exe = r"C:\Mindset\Programmes\Mindcare\MindcareAide.exe";
        let r = appeler(&nom, exe, r#"{"a":"aide.bonjour"}"#, 2000, 2000);
        // (si Mindcare n'a pas ouvert le canal, le fil attend encore un client sans fin : le test s'y connecte pour
        // le libérer. Sur GitHub, en administrateur, il restait bloqué des heures.)
        // SAFETY : un nom terminé par un zéro ; la poignée obtenue est aussitôt refermée
        if let Ok(c) = unsafe {
            CreateFileW(
                &chemin_canal(&nom),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        } {
            drop(Poignee(c));
        }
        let _ = fil.join();
        drop(serveur);
        match r {
            Err(e) => assert!(e.contains("n'est pas elle"), "{e}"),
            // (lancé depuis une console d'administrateur, le canal du test appartient aux Administrateurs, que Mindcare
            // accepte comme l'Aide de Nevermind : ce cas ne se juge qu'en compte normal)
            Ok(_) => assert!(crate::windows::aide::essais::admin(), "un faux canal accepté"),
        }
        // (et un canal absent : une erreur, pas d'attente sans fin)
        let debut = Instant::now();
        assert!(appeler(&format!("{nom}-absent"), exe, "{}", 300, 300).is_err());
        assert!(debut.elapsed() < Duration::from_secs(5));
    }

    /// Ce test tourne-t-il en administrateur ?
    pub fn admin() -> bool {
        use windows::Win32::Security::{TOKEN_ELEVATION, TokenElevation};
        let mut j = HANDLE::default();
        // SAFETY : jeton de ce processus en lecture, refermé par Poignee
        unsafe {
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut j).is_err() {
                return false;
            }
            let j = Poignee(j);
            let mut e = TOKEN_ELEVATION::default();
            let mut n = 0u32;
            GetTokenInformation(
                j.0,
                TokenElevation,
                Some((&mut e as *mut TOKEN_ELEVATION).cast()),
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut n,
            )
            .is_ok()
                && e.TokenIsElevated != 0
        }
    }

    #[test]
    fn le_compte_et_les_noms() {
        let sid = sid_utilisateur();
        assert!(sid.starts_with("S-1-5-"), "{sid}");
        let a = AideWindows::nouvelle(r"C:\Mindset\Programmes\Mindcare");
        if mindcare_aide_regles::sid::sid_utilisateur(&sid) {
            let a = a.unwrap();
            assert_eq!(a.exe, r"C:\Mindset\Programmes\Mindcare\MindcareAide.exe");
            assert!(a.canal.starts_with(&format!("Mindcare-Aide-{sid}-")));
            assert!(a.schtasks.to_lowercase().ends_with(r"\system32\schtasks.exe"));
        }
        assert!(AideWindows::nouvelle("pas un dossier").is_none());
    }
}
