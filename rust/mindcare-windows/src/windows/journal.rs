//! Le journal Système, par l'API des journaux de Windows (wevtapi), comme Get-WinEvent : une requête XPath sur le
//! canal « System », les plus récents d'abord, avec le texte de chaque événement (`EvtFormatMessage`) et ses valeurs
//! (`.Properties`).

use std::collections::HashMap;

use windows::Win32::System::EventLog::{
    EVT_HANDLE, EVT_VARIANT, EvtClose, EvtCreateRenderContext, EvtFormatMessage, EvtFormatMessageEvent, EvtNext,
    EvtOpenPublisherMetadata, EvtQuery, EvtQueryChannelPath, EvtQueryForwardDirection, EvtQueryReverseDirection,
    EvtRender, EvtRenderContextSystem, EvtRenderContextUser, EvtRenderEventValues,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};

use super::{local_de_filetime, utc_de_local};
use crate::moment::Moment;
use crate::source::{Evenement, Filtre, Mesure};

/// ERROR_NO_MORE_ITEMS : la requête n'a plus d'événement.
const PLUS_RIEN: u32 = 259;
/// ERROR_INSUFFICIENT_BUFFER : le tampon est trop petit, la taille voulue est rendue.
const TROP_PETIT: u32 = 122;

/// Une poignée de l'API des journaux, fermée en fin de vie.
struct Poignee(EVT_HANDLE);

impl Drop for Poignee {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY : poignée ouverte par wevtapi, fermée une seule fois
            let _ = unsafe { EvtClose(self.0) };
        }
    }
}

fn code_win32(e: &windows::core::Error) -> u32 {
    let h = e.code().0 as u32;
    if h & 0xFFFF_0000 == 0x8007_0000 { h & 0xFFFF } else { h }
}

fn message(e: &windows::core::Error) -> String {
    let m = e.message();
    if m.is_empty() { format!("erreur 0x{:08X}", e.code().0) } else { m }
}

/// Le texte d'un PCWSTR (vide s'il est nul).
fn texte_pcwstr(p: PCWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY : texte terminé par un zéro, écrit par wevtapi dans le tampon encore vivant
    unsafe { p.to_string() }.unwrap_or_default()
}

/// Les valeurs d'un événement (`EvtRender` dans un contexte), dans un tampon aligné.
fn rendre(contexte: &Poignee, evenement: EVT_HANDLE) -> Mesure<(Vec<u64>, usize)> {
    let mut utilise = 0u32;
    let mut nombre = 0u32;
    // SAFETY : premier appel sans tampon, pour connaître la taille
    let r =
        unsafe { EvtRender(Some(contexte.0), evenement, EvtRenderEventValues.0, 0, None, &mut utilise, &mut nombre) };
    match r {
        Ok(()) => return Ok((Vec::new(), 0)),
        Err(e) if code_win32(&e) == TROP_PETIT => {}
        Err(e) => return Err(message(&e)),
    }
    let mut tampon = vec![0u64; (utilise as usize).div_ceil(8) + 1];
    // SAFETY : tampon de la taille demandée (en octets), aligné sur 8 octets comme EVT_VARIANT
    unsafe {
        EvtRender(
            Some(contexte.0),
            evenement,
            EvtRenderEventValues.0,
            (tampon.len() * 8) as u32,
            Some(tampon.as_mut_ptr().cast()),
            &mut utilise,
            &mut nombre,
        )
    }
    .map_err(|e| message(&e))?;
    Ok((tampon, nombre as usize))
}

/// Les EVT_VARIANT d'un tampon rendu.
fn variants(tampon: &[u64], nombre: usize) -> &[EVT_VARIANT] {
    if nombre == 0 || tampon.is_empty() {
        return &[];
    }
    let max = tampon.len() * 8 / std::mem::size_of::<EVT_VARIANT>();
    // SAFETY : EvtRender a écrit `nombre` EVT_VARIANT au début du tampon (aligné, assez grand : vérifié par `max`)
    unsafe { std::slice::from_raw_parts(tampon.as_ptr().cast::<EVT_VARIANT>(), nombre.min(max)) }
}

const ARRAY: u32 = 128;

/// Une valeur d'événement écrite comme PowerShell l'écrit dans `-join ' '` (les listes : leurs éléments séparés par
/// une espace ; les dates : jj/MM/aaaa HH:mm:ss, l'écriture française de ce PC).
fn texte_variant(v: &EVT_VARIANT) -> String {
    let n = v.Count as usize;
    let liste = v.Type & ARRAY != 0;
    // SAFETY : chaque champ de l'union n'est lu que pour le type que Windows a écrit dans `Type` ; les pointeurs
    // désignent `Count` éléments dans le tampon rendu, encore vivant
    unsafe {
        let a = &v.Anonymous;
        match (v.Type & !ARRAY, liste) {
            (0, _) => String::new(),
            (1, false) | (35, false) => texte_pcwstr(a.StringVal),
            (2, false) => a.AnsiStringVal.to_string().unwrap_or_default(),
            (3, false) => a.SByteVal.to_string(),
            (4, false) => a.ByteVal.to_string(),
            (5, false) => a.Int16Val.to_string(),
            (6, false) => a.UInt16Val.to_string(),
            (7, false) => a.Int32Val.to_string(),
            (8, false) | (20, false) => a.UInt32Val.to_string(),
            (9, false) => a.Int64Val.to_string(),
            (10, false) | (21, false) => a.UInt64Val.to_string(),
            (11, false) => a.SingleVal.to_string().replace('.', ","),
            (12, false) => a.DoubleVal.to_string().replace('.', ","),
            (13, false) => if a.BooleanVal.as_bool() { "True" } else { "False" }.to_string(),
            (14, _) => {
                std::slice::from_raw_parts(a.BinaryVal, n).iter().map(u8::to_string).collect::<Vec<_>>().join(" ")
            }
            (15, false) => {
                let g = &*a.GuidVal;
                format!(
                    "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                    g.data1,
                    g.data2,
                    g.data3,
                    g.data4[0],
                    g.data4[1],
                    g.data4[2],
                    g.data4[3],
                    g.data4[4],
                    g.data4[5],
                    g.data4[6],
                    g.data4[7]
                )
            }
            (16, false) => a.SizeTVal.to_string(),
            (17, false) => local_de_filetime(a.FileTimeVal).map(date_fr).unwrap_or_default(),
            (19, false) => texte_sid(a.SidVal),
            (1, true) => (0..n).map(|i| texte_pcwstr(PCWSTR((*a.StringArr.add(i)).0))).collect::<Vec<_>>().join(" "),
            (4, true) => {
                std::slice::from_raw_parts(a.ByteArr, n).iter().map(u8::to_string).collect::<Vec<_>>().join(" ")
            }
            (6, true) => {
                std::slice::from_raw_parts(a.UInt16Arr, n).iter().map(u16::to_string).collect::<Vec<_>>().join(" ")
            }
            (8, true) | (20, true) => {
                std::slice::from_raw_parts(a.UInt32Arr, n).iter().map(u32::to_string).collect::<Vec<_>>().join(" ")
            }
            (10, true) | (21, true) => {
                std::slice::from_raw_parts(a.UInt64Arr, n).iter().map(u64::to_string).collect::<Vec<_>>().join(" ")
            }
            _ => String::new(),
        }
    }
}

fn date_fr(m: Moment) -> String {
    let (a, mo, j) = m.date();
    let (h, mi, s, _) = m.heure();
    format!("{j:02}/{mo:02}/{a:04} {h:02}:{mi:02}:{s:02}")
}

fn texte_sid(sid: windows::Win32::Security::PSID) -> String {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    let mut p = PWSTR::null();
    // SAFETY : SID rendu par wevtapi ; le texte alloué par Windows est libéré avec LocalFree
    unsafe {
        if ConvertSidToStringSidW(sid, &mut p).is_err() || p.is_null() {
            return String::new();
        }
        let t = p.to_string().unwrap_or_default();
        LocalFree(Some(HLOCAL(p.0.cast())));
        t
    }
}

/// Le texte d'un événement (`.Message`), par la description de son fournisseur (vide si Windows ne sait pas l'écrire).
fn texte_evenement(fournisseurs: &mut HashMap<String, Option<Poignee>>, nom: &str, evenement: EVT_HANDLE) -> String {
    let meta = fournisseurs.entry(nom.to_string()).or_insert_with(|| {
        let h = HSTRING::from(nom);
        // SAFETY : nom valide ; la poignée rendue est fermée par Poignee
        unsafe { EvtOpenPublisherMetadata(None, &h, PCWSTR::null(), 0, 0) }.ok().map(Poignee)
    });
    let Some(meta) = meta else { return String::new() };
    let mut utilise = 0u32;
    // SAFETY : premier appel sans tampon, pour connaître la taille
    let r = unsafe {
        EvtFormatMessage(Some(meta.0), Some(evenement), 0, None, EvtFormatMessageEvent.0, None, &mut utilise)
    };
    match r {
        Ok(()) => return String::new(),
        Err(e) if code_win32(&e) == TROP_PETIT => {}
        Err(_) => return String::new(),
    }
    let mut tampon = vec![0u16; utilise as usize + 1];
    // SAFETY : tampon de la taille demandée (en caractères)
    let r = unsafe {
        EvtFormatMessage(
            Some(meta.0),
            Some(evenement),
            0,
            None,
            EvtFormatMessageEvent.0,
            Some(&mut tampon),
            &mut utilise,
        )
    };
    if r.is_err() {
        return String::new();
    }
    let fin = tampon.iter().position(|c| *c == 0).unwrap_or(tampon.len());
    String::from_utf16_lossy(&tampon[..fin])
}

/// Les événements suivants d'une requête (rien : il n'y en a plus).
fn suivants(requete: &Poignee, combien: usize) -> Mesure<Vec<Poignee>> {
    let mut lot = vec![0isize; combien.clamp(1, 64)];
    let mut rendus = 0u32;
    // SAFETY : tableau de poignées de la taille annoncée ; chaque poignée rendue est fermée par Poignee
    match unsafe { EvtNext(requete.0, &mut lot, u32::MAX, 0, &mut rendus) } {
        Ok(()) => Ok(lot[..(rendus as usize).min(lot.len())].iter().map(|h| Poignee(EVT_HANDLE(*h))).collect()),
        Err(e) if code_win32(&e) == PLUS_RIEN => Ok(Vec::new()),
        Err(e) => Err(message(&e)),
    }
}

fn ouvrir(xpath: &str, sens: u32) -> Mesure<Poignee> {
    let (canal, q) = (HSTRING::from("System"), HSTRING::from(xpath));
    // SAFETY : textes valides ; la poignée rendue est fermée par Poignee
    unsafe { EvtQuery(None, &canal, &q, EvtQueryChannelPath.0 | sens) }.map(Poignee).map_err(|e| message(&e))
}

/// La sonde : l'événement le plus ancien du journal Système (`Get-WinEvent -LogName System -MaxEvents 1 -Oldest`).
pub fn plus_ancien() -> Mesure<Option<Moment>> {
    let requete = ouvrir("*", EvtQueryForwardDirection.0)?;
    let Some(e) = suivants(&requete, 1)?.into_iter().next() else { return Ok(None) };
    // SAFETY : contexte des valeurs « System », fermé par Poignee
    let contexte = Poignee(unsafe { EvtCreateRenderContext(None, EvtRenderContextSystem.0) }.map_err(|e| message(&e))?);
    let (tampon, n) = rendre(&contexte, e.0)?;
    let v = variants(&tampon, n);
    // SAFETY : TimeCreated (n° 8) est une FILETIME (type 17), vérifié avant la lecture
    let ft = v.get(8).filter(|x| x.Type == 17).map(|x| unsafe { x.Anonymous.FileTimeVal });
    Ok(ft.and_then(local_de_filetime))
}

/// Le XPath d'un filtre, comme Get-WinEvent le construit depuis -FilterHashtable.
pub fn xpath(f: &Filtre) -> Mesure<String> {
    let mut morceaux = vec![format!("Provider[@Name='{}']", f.fournisseur.replace('\'', "&apos;"))];
    if !f.ids.is_empty() {
        morceaux.push(format!("({})", f.ids.iter().map(|i| format!("EventID={i}")).collect::<Vec<_>>().join(" or ")));
    }
    if !f.niveaux.is_empty() {
        morceaux.push(format!("({})", f.niveaux.iter().map(|n| format!("Level={n}")).collect::<Vec<_>>().join(" or ")));
    }
    let depuis = utc_de_local(f.depuis).ok_or("date de début illisible")?;
    morceaux.push(format!("TimeCreated[@SystemTime>='{depuis}']"));
    Ok(format!("*[System[{}]]", morceaux.join(" and ")))
}

/// Les événements du journal Système qui répondent au filtre, les plus récents d'abord.
pub fn evenements(f: &Filtre) -> Mesure<Vec<Evenement>> {
    let requete = ouvrir(&xpath(f)?, EvtQueryReverseDirection.0)?;
    // SAFETY : contextes des valeurs « System » et des valeurs de l'événement, fermés par Poignee
    let systeme = Poignee(unsafe { EvtCreateRenderContext(None, EvtRenderContextSystem.0) }.map_err(|e| message(&e))?);
    let utilisateur =
        Poignee(unsafe { EvtCreateRenderContext(None, EvtRenderContextUser.0) }.map_err(|e| message(&e))?);
    let mut fournisseurs = HashMap::new();
    let mut l = Vec::new();
    while l.len() < f.max {
        let lot = suivants(&requete, f.max - l.len())?;
        if lot.is_empty() {
            break;
        }
        for e in lot {
            let (tampon, n) = rendre(&systeme, e.0)?;
            let v = variants(&tampon, n);
            // SAFETY : chaque valeur n'est lue que si son type est celui attendu (1 : texte, 6 : u16, 4 : octet,
            // 17 : FILETIME)
            let (fournisseur, id, niveau, ft) = unsafe {
                (
                    v.first().filter(|x| x.Type == 1).map(|x| texte_pcwstr(x.Anonymous.StringVal)).unwrap_or_default(),
                    v.get(2).filter(|x| x.Type == 6).map_or(0, |x| u32::from(x.Anonymous.UInt16Val)),
                    v.get(4).filter(|x| x.Type == 4).map_or(0, |x| x.Anonymous.ByteVal),
                    v.get(8).filter(|x| x.Type == 17).map(|x| x.Anonymous.FileTimeVal),
                )
            };
            let (tampon_u, nu) = rendre(&utilisateur, e.0).unwrap_or_default();
            let proprietes = variants(&tampon_u, nu).iter().map(texte_variant).collect();
            let message = texte_evenement(&mut fournisseurs, &fournisseur, e.0);
            l.push(Evenement {
                id,
                fournisseur,
                niveau,
                date: ft.and_then(local_de_filetime).unwrap_or(Moment(0)),
                message,
                proprietes,
            });
        }
    }
    l.truncate(f.max);
    Ok(l)
}
