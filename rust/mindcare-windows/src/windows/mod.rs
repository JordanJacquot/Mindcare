//! Le seul module qui parle à Windows (et le seul où le code « unsafe » est permis). Il remplit le trait
//! [`SourceWindows`] avec les vraies lectures ; les règles (le reste de la bibliothèque) n'en savent rien.
//!
//! Tout se lit sans droits administrateur et sans rien modifier :
//! - le journal Système par l'API des journaux de Windows (`EvtQuery`, ce que fait Get-WinEvent) : [`journal`] ;
//! - les disques, la mémoire, le démarrage et le modèle du PC par WMI (ce que font Get-Disk, Get-PhysicalDisk et
//!   Get-CimInstance) : [`wmi`] ;
//! - la place libre par `GetDiskFreeSpaceExW` (les mêmes nombres que Win32_LogicalDisk, que lit Get-PSDrive) ;
//! - la batterie par le rapport de Windows (`powercfg /batteryreport /xml`, comme Mindcare PowerShell) : un fichier
//!   écrit dans un dossier temporaire, lu puis effacé tout de suite.
//!
//! Une lecture qui demande des droits qu'on n'a pas échoue proprement (`Err`) : c'est une mesure ratée (constat 20).

#![allow(unsafe_code)]

pub mod journal;
pub mod wmi;

use std::os::windows::process::CommandExt;
use std::path::PathBuf;

use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::SystemInformation::{GetSystemTimeAsFileTime, GetSystemWindowsDirectoryW};
use windows::Win32::System::Time::{
    FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime, TzSpecificLocalTimeToSystemTime,
};
use windows::core::HSTRING;

use crate::moment::{Moment, TICKS_PAR_SECONDE};
use crate::source::{
    Barrette, Batterie, Disque, DisquePhysique, Evenement, Filtre, Horloge, InfosPc, Mesure, Partition, Place,
    SourceWindows,
};
use wmi::Valeur;

/// Un programme lancé sans fenêtre (CREATE_NO_WINDOW).
const SANS_FENETRE: u32 = 0x0800_0000;

/// Un instant à l'heure locale, depuis une date FILETIME (heure universelle, en ticks depuis 1601).
pub(crate) fn local_de_filetime(ft: u64) -> Option<Moment> {
    let f = FILETIME { dwLowDateTime: ft as u32, dwHighDateTime: (ft >> 32) as u32 };
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    // SAFETY : structures valides, passées par référence, remplies par Windows
    unsafe {
        FileTimeToSystemTime(&f, &mut utc).ok()?;
        SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).ok()?;
    }
    let reste = (ft % TICKS_PAR_SECONDE as u64) as u32;
    moment_de_systemtime(&local, reste)
}

fn moment_de_systemtime(s: &SYSTEMTIME, ticks: u32) -> Option<Moment> {
    Moment::nouveau(
        i64::from(s.wYear),
        u32::from(s.wMonth),
        u32::from(s.wDay),
        u32::from(s.wHour),
        u32::from(s.wMinute),
        u32::from(s.wSecond),
        ticks,
    )
}

/// Un instant à l'heure locale écrit en heure universelle, pour les requêtes au journal (« 2026-10-01T10:00:00.000Z »).
pub(crate) fn utc_de_local(m: Moment) -> Option<String> {
    let (a, mo, j) = m.date();
    let (h, mi, s, t) = m.heure();
    let local = SYSTEMTIME {
        wYear: u16::try_from(a).ok()?,
        wMonth: mo as u16,
        wDayOfWeek: 0,
        wDay: j as u16,
        wHour: h as u16,
        wMinute: mi as u16,
        wSecond: s as u16,
        wMilliseconds: (t / 10_000) as u16,
    };
    let mut u = SYSTEMTIME::default();
    // SAFETY : structures valides, passées par référence, remplies par Windows
    unsafe { TzSpecificLocalTimeToSystemTime(None, &local, &mut u).ok()? };
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        u.wYear, u.wMonth, u.wDay, u.wHour, u.wMinute, u.wSecond, u.wMilliseconds
    ))
}

/// L'heure locale du PC (Get-Date).
pub struct HorlogeWindows;

impl Horloge for HorlogeWindows {
    fn maintenant(&self) -> Moment {
        // SAFETY : simple lecture de l'heure
        let f = unsafe { GetSystemTimeAsFileTime() };
        let ft = (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
        local_de_filetime(ft).unwrap_or(Moment(0))
    }
}

/// Le dossier de Windows (C:\Windows), lu par Windows.
fn dossier_windows() -> String {
    let mut b = vec![0u16; 1024];
    // SAFETY : tampon et taille cohérents
    let n = unsafe { GetSystemWindowsDirectoryW(Some(&mut b)) } as usize;
    String::from_utf16_lossy(&b[..n.min(b.len())]).trim_end_matches('\\').to_string()
}

/// Une date WMI (« 20261030083015.500000+060 » : heure locale du moment, et son décalage en minutes) à l'heure locale
/// du PC.
pub(crate) fn date_wmi(t: &str) -> Option<Moment> {
    let t = t.trim();
    if t.len() < 25 || !t.is_ascii() {
        return None;
    }
    let n = |a: usize, b: usize| t[a..b].parse::<u32>().ok();
    let ici =
        Moment::nouveau(i64::from(n(0, 4)?), n(4, 6)?, n(6, 8)?, n(8, 10)?, n(10, 12)?, n(12, 14)?, n(15, 21)? * 10)?;
    let decalage: i64 = t[21..25].parse().ok()?;
    // (l'heure universelle, puis l'heure locale de ce PC à cette date, comme Get-CimInstance)
    let utc = ici.plus_minutes(-decalage);
    let ft = utc.0 - Moment::nouveau(1601, 1, 1, 0, 0, 0, 0)?.0;
    local_de_filetime(u64::try_from(ft).ok()?)
}

/// Le vrai Windows de ce PC.
pub struct WindowsReel {
    /// Où le rapport de batterie est écrit le temps de le lire (le dossier temporaire de Windows, comme PowerShell ;
    /// les essais en donnent un à eux).
    pub dossier_temp: PathBuf,
}

impl Default for WindowsReel {
    fn default() -> Self {
        WindowsReel { dossier_temp: std::env::temp_dir() }
    }
}

fn texte(v: Option<&Valeur>) -> String {
    match v {
        Some(Valeur::Texte(t)) => t.clone(),
        Some(Valeur::Entier(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn entier(v: Option<&Valeur>) -> Option<i64> {
    match v {
        Some(Valeur::Entier(n)) => Some(*n),
        Some(Valeur::Texte(t)) => t.trim().parse().ok(),
        _ => None,
    }
}

/// La première batterie d'un rapport de batterie (XML de powercfg) : rien si le rapport n'en cite aucune.
pub fn batterie_du_rapport(octets: &[u8]) -> Mesure<Option<Batterie>> {
    let xml = if octets.starts_with(&[0xFF, 0xFE]) {
        let u: Vec<u16> = octets[2..].as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        String::from_utf16_lossy(&u)
    } else {
        String::from_utf8_lossy(octets.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(octets)).into_owned()
    };
    if !xml.contains("<BatteryReport") {
        return Err("rapport de batterie illisible".to_string());
    }
    let Some(i) = xml.find("<Batteries") else { return Ok(None) };
    let reste = &xml[i..];
    let fin = reste.find("</Batteries>").unwrap_or(reste.len());
    let batteries = &reste[..fin];
    let debut = batteries.match_indices("<Battery").find(|(j, _)| {
        batteries[j + 8..].starts_with('>')
            || batteries[j + 8..].starts_with(' ')
            || batteries[j + 8..].starts_with('/')
    });
    let Some((j, _)) = debut else { return Ok(None) };
    let b = &batteries[j..];
    let b = &b[..b.find("</Battery>").unwrap_or(b.len())];
    let valeur = |nom: &str| {
        let ouvre = format!("<{nom}>");
        b.find(&ouvre)
            .and_then(|k| {
                let v = &b[k + ouvre.len()..];
                v.find(&format!("</{nom}>")).map(|f| v[..f].to_string())
            })
            .unwrap_or_default()
    };
    Ok(Some(Batterie { origine: valeur("DesignCapacity"), pleine: valeur("FullChargeCapacity") }))
}

impl SourceWindows for WindowsReel {
    fn plus_ancien(&self) -> Mesure<Option<Moment>> {
        journal::plus_ancien()
    }

    fn evenements(&self, filtre: &Filtre) -> Mesure<Vec<Evenement>> {
        journal::evenements(filtre)
    }

    fn disques(&self) -> Mesure<Vec<Disque>> {
        let l = wmi::requete(wmi::STOCKAGE, "SELECT Number, BusType FROM MSFT_Disk", &["Number", "BusType"])?;
        Ok(l.iter()
            .map(|d| Disque {
                numero: entier(d.first()).and_then(|n| u32::try_from(n).ok()).unwrap_or(0),
                bus: entier(d.get(1)).and_then(|n| u16::try_from(n).ok()),
            })
            .collect())
    }

    fn partitions(&self) -> Mesure<Vec<Partition>> {
        let l = wmi::requete(
            wmi::STOCKAGE,
            "SELECT DiskNumber, DriveLetter FROM MSFT_Partition",
            &["DiskNumber", "DriveLetter"],
        )?;
        Ok(l.iter()
            .map(|p| Partition {
                disque: entier(p.first()).and_then(|n| u32::try_from(n).ok()).unwrap_or(0),
                lettre: entier(p.get(1)).and_then(|n| u16::try_from(n).ok()).unwrap_or(0),
            })
            .collect())
    }

    fn disques_physiques(&self) -> Mesure<Vec<DisquePhysique>> {
        let l = wmi::requete(
            wmi::STOCKAGE,
            "SELECT FriendlyName, BusType, HealthStatus FROM MSFT_PhysicalDisk",
            &["FriendlyName", "BusType", "HealthStatus"],
        )?;
        Ok(l.iter()
            .map(|d| DisquePhysique {
                nom: texte(d.first()),
                bus: entier(d.get(1)).and_then(|n| u16::try_from(n).ok()),
                sante: entier(d.get(2)).and_then(|n| u16::try_from(n).ok()),
            })
            .collect())
    }

    fn lecteur_systeme(&self) -> String {
        std::env::var("SystemDrive").unwrap_or_default()
    }

    fn place(&self, lecteur: &str) -> Mesure<Place> {
        let racine = HSTRING::from(format!("{lecteur}:\\"));
        let (mut taille, mut libre) = (0u64, 0u64);
        // SAFETY : chemin valide, nombres remplis par Windows
        unsafe { GetDiskFreeSpaceExW(&racine, None, Some(&mut taille), Some(&mut libre)) }.map_err(|e| e.message())?;
        Ok(Place { libre, taille })
    }

    fn batterie(&self) -> Mesure<Option<Batterie>> {
        let f = self.dossier_temp.join(format!("mindcare-batterie-{}.xml", std::process::id()));
        let exe = format!(r"{}\System32\powercfg.exe", dossier_windows());
        let lance = std::process::Command::new(exe)
            .args(["/batteryreport", "/xml", "/output"])
            .arg(&f)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(SANS_FENETRE)
            .status();
        let lu = std::fs::read(&f);
        let _ = std::fs::remove_file(&f);
        if let Err(e) = lance {
            return Err(format!("powercfg : {e}"));
        }
        batterie_du_rapport(&lu.map_err(|e| format!("rapport de batterie absent ({e})"))?)
    }

    fn demarrage(&self) -> Mesure<Moment> {
        let l = wmi::requete(wmi::CIMV2, "SELECT LastBootUpTime FROM Win32_OperatingSystem", &["LastBootUpTime"])?;
        let t = texte(l.first().and_then(|o| o.first()));
        date_wmi(&t).ok_or_else(|| format!("date de démarrage illisible ({t})"))
    }

    fn barrettes(&self) -> Mesure<Vec<Barrette>> {
        let l = wmi::requete(
            wmi::CIMV2,
            "SELECT Capacity, Manufacturer, PartNumber, DeviceLocator FROM Win32_PhysicalMemory",
            &["Capacity", "Manufacturer", "PartNumber", "DeviceLocator"],
        )?;
        Ok(l.iter()
            .map(|b| Barrette {
                capacite: entier(b.first()).and_then(|n| u64::try_from(n).ok()),
                fabricant: texte(b.get(1)),
                reference: texte(b.get(2)),
                emplacement: texte(b.get(3)),
            })
            .collect())
    }

    fn infos_pc(&self) -> Mesure<InfosPc> {
        let cs = wmi::requete(
            wmi::CIMV2,
            "SELECT Manufacturer, Model FROM Win32_ComputerSystem",
            &["Manufacturer", "Model"],
        )?;
        let os = wmi::requete(wmi::CIMV2, "SELECT Caption FROM Win32_OperatingSystem", &["Caption"])?;
        let cs = cs.first().cloned().unwrap_or_default();
        Ok(InfosPc {
            fabricant: texte(cs.first()),
            modele: texte(cs.get(1)),
            systeme: texte(os.first().and_then(|o| o.first())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapport_de_batterie() {
        let xml = br#"<?xml version="1.0"?><BatteryReport xmlns="x"><Batteries><Battery><Id>A</Id><DesignCapacity>50000</DesignCapacity><FullChargeCapacity>41000</FullChargeCapacity></Battery><Battery><DesignCapacity>1</DesignCapacity></Battery></Batteries></BatteryReport>"#;
        let b = batterie_du_rapport(xml).expect("lisible").expect("une batterie");
        assert_eq!((b.origine.as_str(), b.pleine.as_str()), ("50000", "41000"));
        assert_eq!(batterie_du_rapport(b"<BatteryReport><Batteries /></BatteryReport>"), Ok(None));
        assert!(batterie_du_rapport(b"pas du xml").is_err());
    }
}
