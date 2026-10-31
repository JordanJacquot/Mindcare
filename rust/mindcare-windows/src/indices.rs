//! Les règles de la lecture de Windows : le portage de Get-IndicesWindows, Select-ErreursDisque, Get-Quantite et
//! Get-Barrettes (modules\journaux.ps1). Ce que Windows a noté sur ce PC (son journal « Système », ces 30 derniers
//! jours) et quelques mesures (place libre, batterie, dernier redémarrage), pour départager les pistes quand Mindcare
//! est ouvert sur le PC en panne.
//!
//! Un indice : vu ou pas, le texte de ce qui a été vu, celui de la bonne nouvelle, « affiché » et « partiel » (le
//! journal ne remonte pas assez loin : s'il n'a rien vu, ça ne prouve rien).
//!
//! **Divergence voulue (constat 20)** : PowerShell rend « pas vu » quand une mesure rate (une requête au journal qui
//! échoue, la santé des disques, la place, la batterie, le démarrage illisibles), ce qui écarte des pistes à tort. Ici,
//! une mesure ratée qui n'a rien vu ne donne pas d'indice du tout : le moteur n'écarte rien. Ce qu'une mesure a vu
//! reste vu (une erreur de disque comptée reste une erreur, même si une autre lecture a raté). Tout le reste répond
//! exactement comme PowerShell (tests\juges).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::moment::Moment;
use crate::motifs::{arrondi, code_hex, contient_un, double, lettre_lecteur, nombre, numero_disque, quantite};
use crate::source::{Barrette, Evenement, Filtre, SourceWindows};

/// Les jours de journal lus (`Get-IndicesWindows -Jours 30`).
pub const JOURS: i64 = 30;

/// Au plus autant d'événements par requête (`Get-EvenementsSysteme -Max 200`).
const MAX_EVENEMENTS: usize = 200;

/// Ce que Windows a noté pour un indice.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndiceWindows {
    pub vu: bool,
    /// Ce qui a été vu.
    pub texte: String,
    /// Rien de vu : la bonne nouvelle (ou vide).
    pub texte_ok: String,
    /// Montré dans la liste de ce qui a été vérifié.
    pub affiche: bool,
    /// Le journal ne remonte pas assez loin : ce qu'il n'a pas vu ne prouve rien.
    pub partiel: bool,
}

/// Les indices, par nom (« coupures », « disque »... et « journal » quand le journal n'a pas servi en entier).
pub type IndicesWindows = BTreeMap<String, IndiceWindows>;

/// Les codes d'écran bleu les plus courants, et ce qu'ils accusent le plus souvent (`$script:CodesEcranBleu`).
const CODES_ECRAN_BLEU: [(i32, &str, &str); 22] = [
    (0x139, "KERNEL_SECURITY_CHECK_FAILURE", "pilote"),
    (0x1E, "KMODE_EXCEPTION_NOT_HANDLED", "pilote"),
    (0x154, "UNEXPECTED_STORE_EXCEPTION", "disque"),
    (0x119, "VIDEO_SCHEDULER_INTERNAL_ERROR", "pilote"),
    (0x1A, "MEMORY_MANAGEMENT", "memoire"),
    (0x50, "PAGE_FAULT_IN_NONPAGED_AREA", "memoire"),
    (0x4E, "PFN_LIST_CORRUPT", "memoire"),
    (0x12B, "FAULTY_HARDWARE_CORRUPTED_PAGE", "memoire"),
    (0xA, "IRQL_NOT_LESS_OR_EQUAL", "pilote"),
    (0xD1, "DRIVER_IRQL_NOT_LESS_OR_EQUAL", "pilote"),
    (0x116, "VIDEO_TDR_FAILURE", "pilote"),
    (0x3B, "SYSTEM_SERVICE_EXCEPTION", "pilote"),
    (0x7E, "SYSTEM_THREAD_EXCEPTION_NOT_HANDLED", "pilote"),
    (0x133, "DPC_WATCHDOG_VIOLATION", "pilote"),
    (0x9F, "DRIVER_POWER_STATE_FAILURE", "pilote"),
    (0x7A, "KERNEL_DATA_INPAGE_ERROR", "disque"),
    (0x24, "NTFS_FILE_SYSTEM", "disque"),
    (0xEF, "CRITICAL_PROCESS_DIED", "disque"),
    (0x7B, "INACCESSIBLE_BOOT_DEVICE", "disque"),
    (0xF4, "CRITICAL_OBJECT_TERMINATION", "disque"),
    (0x124, "WHEA_UNCORRECTABLE_ERROR", "materiel"),
    (0x101, "CLOCK_WATCHDOG_TIMEOUT", "materiel"),
];

fn code_ecran_bleu(code: i32) -> Option<(&'static str, &'static str)> {
    CODES_ECRAN_BLEU.iter().find(|(c, _, _)| *c == code).map(|(_, n, f)| (*n, *f))
}

/// Le nom du bus d'un disque, comme Storage.types.ps1xml (Get-Disk, Get-PhysicalDisk).
pub fn nom_bus(bus: u16) -> &'static str {
    match bus {
        1 => "SCSI",
        2 => "ATAPI",
        3 => "ATA",
        4 => "1394",
        5 => "SSA",
        6 => "Fibre Channel",
        7 => "USB",
        8 => "RAID",
        9 => "iSCSI",
        10 => "SAS",
        11 => "SATA",
        12 => "SD",
        13 => "MMC",
        14 => "Virtual",
        15 => "File Backed Virtual",
        16 => "Spaces",
        17 => "NVMe",
        18 => "SCM",
        19 => "UFS",
        _ => "Unknown",
    }
}

/// La santé d'un disque physique, comme Storage.types.ps1xml (rien ou un nombre inconnu : « Unknown »).
pub fn nom_sante(sante: Option<u16>) -> &'static str {
    match sante {
        Some(0) => "Healthy",
        Some(1) => "Warning",
        Some(2) => "Unhealthy",
        _ => "Unknown",
    }
}

/// Les erreurs de disque qui concernent les disques du PC lui-même (Select-ErreursDisque) : une clé USB ou un disque
/// externe qui signale des erreurs ne dit rien du disque du PC. `internes` : les numéros des disques du PC ; `lettres` :
/// leurs lecteurs (C, D...). Inconnus (rien), ou une erreur qui ne dit pas son disque : elle compte.
pub fn erreurs_disque<'a>(
    evenements: &'a [Evenement],
    internes: Option<&[i64]>,
    lettres: Option<&[String]>,
) -> Vec<&'a Evenement> {
    evenements
        .iter()
        .filter(|e| {
            let t = format!("{} {}", e.proprietes.join(" "), e.message);
            if e.fournisseur.eq_ignore_ascii_case("Ntfs") {
                match (lettre_lecteur(&t), lettres) {
                    (Some(l), Some(lettres)) => lettres.iter().any(|x| x.eq_ignore_ascii_case(&l.to_string())),
                    _ => true,
                }
            } else {
                match (numero_disque(&t), internes) {
                    // (un numéro trop grand pour [int], « Harddisk99999999999\ » : en PowerShell, l'erreur remonte
                    // jusqu'au try de Start-LectureJournaux et TOUTE la lecture est perdue. Ici l'événement compte,
                    // comme une erreur qui ne dit pas son disque. Hors des juges : jamais vu dans un vrai journal.)
                    (Some(n), Some(internes)) => n.parse::<i32>().map_or(true, |n| internes.contains(&i64::from(n))),
                    _ => true,
                }
            }
        })
        .collect()
}

/// Les barrettes de mémoire, écrites comme Get-Barrettes : « 16 Go Kingston KF3200 (DIMM1) ».
pub fn texte_barrettes(barrettes: &[Barrette]) -> Vec<String> {
    barrettes
        .iter()
        .map(|b| {
            let go = arrondi(b.capacite.unwrap_or(0) as f64 / 1_073_741_824.0);
            let reference = format!("{} {}", b.fabricant.trim(), b.reference.trim()).trim().to_string();
            let mut t = format!("{} Go", nombre(go));
            if !reference.is_empty() {
                t.push(' ');
                t.push_str(&reference);
            }
            if !b.emplacement.is_empty() {
                t.push_str(&format!(" ({})", b.emplacement.trim()));
            }
            t
        })
        .collect()
}

/// Les barrettes de ce PC (Get-Barrettes) : illisibles, aucune (PowerShell le note dans son journal).
pub fn barrettes(source: &dyn SourceWindows) -> Mesure<Vec<String>> {
    source.barrettes().map(|b| texte_barrettes(&b))
}

use crate::source::Mesure;

/// La lecture a été arrêtée en route (Stop-LectureJournaux).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arretee;

struct Lecteur<'a> {
    source: &'a dyn SourceWindows,
    arret: Option<&'a AtomicBool>,
    i: IndicesWindows,
}

impl Lecteur<'_> {
    fn ajoute(&mut self, id: &str, vu: bool, texte: String, ok: String, affiche: bool, partiel: bool) {
        self.i.insert(id.to_string(), IndiceWindows { vu, texte, texte_ok: ok, affiche, partiel });
    }

    /// Get-EvenementsSysteme. Rien : la requête a échoué (PowerShell rendait une liste vide).
    fn evenements(
        &self,
        fournisseur: &str,
        ids: &[u32],
        niveaux: &[u8],
        depuis: Moment,
        max: usize,
    ) -> Option<Vec<Evenement>> {
        let filtre =
            Filtre { fournisseur: fournisseur.to_string(), ids: ids.to_vec(), niveaux: niveaux.to_vec(), depuis, max };
        self.source.evenements(&filtre).ok()
    }

    fn continuer(&self) -> Result<(), Arretee> {
        if self.arret.is_some_and(|a| a.load(Ordering::SeqCst)) { Err(Arretee) } else { Ok(()) }
    }
}

/// `$dernier` : « (le 5 octobre) », ou « (le dernier le 5 octobre) » s'il y en a plusieurs.
fn dernier(ev: &[Evenement]) -> String {
    match ev {
        [] => String::new(),
        [e] => format!(" (le {})", e.date.quand()),
        [e, ..] => format!(" (le dernier le {})", e.date.quand()),
    }
}

/// Get-IndicesWindows : ce que Windows a noté sur ce PC, et les mesures. `maintenant` : Get-Date.
pub fn indices_windows(source: &dyn SourceWindows, maintenant: Moment, jours: i64) -> IndicesWindows {
    // (jamais arrêtée : sans drapeau d'arrêt)
    lire(source, maintenant, jours, None).unwrap_or_default()
}

/// Get-IndicesWindows, arrêtable entre deux mesures (la lecture en arrière-plan).
pub fn lire(
    source: &dyn SourceWindows,
    maintenant: Moment,
    jours: i64,
    arret: Option<&AtomicBool>,
) -> Result<IndicesWindows, Arretee> {
    let mut l = Lecteur { source, arret, i: IndicesWindows::new() };
    let depuis = maintenant.plus_jours(-jours);

    // La sonde : le journal Système se lit-il, et depuis quand ? Un journal illisible lu avec un filtre répond « aucun
    // événement », comme un PC sans souci : sans la sonde, ce serait pris pour une bonne nouvelle.
    let (mut illisible, debut) = match source.plus_ancien() {
        Ok(d) => (String::new(), d),
        Err(e) => (e, None),
    };
    if illisible.is_empty() && debut.is_none() {
        illisible = "journal vide".to_string();
    }
    let court = debut.is_some_and(|d| d > depuis);
    let periode = match debut {
        Some(d) if court => format!("depuis le {}", d.quand()),
        _ => format!("ces {jours} jours"),
    };
    if !illisible.is_empty() {
        let ok = format!("Le journal de Windows n'a pas pu être lu ({illisible}) : Mindcare ne s'en est pas servie");
        l.ajoute("journal", false, String::new(), ok, false, false);
    } else if let (true, Some(d)) = (court, debut) {
        let ok =
            format!("Le journal de Windows ne remonte qu'au {} : ce qu'il n'a pas vu n'écarte aucune piste", d.quand());
        l.ajoute("journal", false, String::new(), ok, false, false);
    }

    if illisible.is_empty() {
        l.continuer()?;
        // Redémarrages et coupures brutales (Windows ne s'est pas arrêté normalement)
        if let Some(ev) = l.evenements("Microsoft-Windows-Kernel-Power", &[41], &[], depuis, MAX_EVENEMENTS) {
            let n = ev.len() as i64;
            let t = format!("{} {periode}{}", quantite(n, "redémarrage brutal", "redémarrages brutaux"), dernier(&ev));
            l.ajoute("coupures", n > 0, t, format!("Aucun redémarrage brutal {periode}"), true, court);
        }

        l.continuer()?;
        // Écrans bleus, et ce que leur code accuse
        if let Some(ev) =
            l.evenements("Microsoft-Windows-WER-SystemErrorReporting", &[1001], &[], depuis, MAX_EVENEMENTS)
        {
            let mut noms: Vec<String> = Vec::new();
            let mut par_famille: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
            for e in &ev {
                let t = format!("{} {}", e.proprietes.first().map(String::as_str).unwrap_or(""), e.message);
                let Some(hex) = code_hex(&t) else { continue };
                // ([Convert]::ToInt32(..., 16) : au-delà de 0x7FFFFFFF, un nombre négatif)
                let code = u32::from_str_radix(hex, 16).unwrap_or(0) as i32;
                match code_ecran_bleu(code) {
                    Some((nom, famille)) => {
                        noms.push(nom.to_string());
                        par_famille.entry(famille).or_default().push(nom);
                    }
                    None => noms.push(format!("0x{:X}", code as u32)),
                }
            }
            let mut uniques: Vec<String> = Vec::new();
            for n in noms {
                if !uniques.contains(&n) {
                    uniques.push(n);
                }
            }
            let n = ev.len() as i64;
            let liste = if uniques.is_empty() { String::new() } else { format!(" ({})", uniques.join(", ")) };
            let t = format!("{} {periode}{liste}", quantite(n, "écran bleu", "écrans bleus"));
            l.ajoute("ecransbleus", n > 0, t, format!("Aucun écran bleu {periode}"), true, court);
            for f in ["memoire", "pilote", "disque", "materiel"] {
                let mut codes: Vec<&str> = Vec::new();
                for c in par_famille.get(f).into_iter().flatten() {
                    if !codes.contains(c) {
                        codes.push(c);
                    }
                }
                let t = format!("Code d'écran bleu noté par Windows : {}", codes.join(", "));
                l.ajoute(&format!("bsod-{f}"), !codes.is_empty(), t, String::new(), false, court);
            }
        }

        l.continuer()?;
        // Erreurs matérielles graves, notées par le processeur ou la carte mère
        if let Some(ev) = l.evenements("Microsoft-Windows-WHEA-Logger", &[], &[1, 2], depuis, MAX_EVENEMENTS) {
            let n = ev.len() as i64;
            let t = format!("{} {periode}", quantite(n, "erreur matérielle grave", "erreurs matérielles graves"));
            l.ajoute("materiel", n > 0, t, format!("Aucune erreur matérielle grave {periode}"), true, court);
        }

        l.continuer()?;
        // Le dernier test de la mémoire de Windows (sur 6 mois) : 1101 sans erreur, 1102 avec des erreurs
        let six_mois = maintenant.plus_jours(-180);
        if let Some(ev) = l.evenements("Microsoft-Windows-MemoryDiagnostics-Results", &[1101, 1102], &[], six_mois, 1) {
            let erreurs = ev.first().is_some_and(|e| e.id == 1102);
            let court6 = debut.is_some_and(|d| d > six_mois);
            let (t_erreurs, t_ok) = match ev.first() {
                Some(e) => (
                    format!("Le test de mémoire du {} a trouvé des erreurs", e.date.quand()),
                    format!("Le test de mémoire du {} n'a trouvé aucune erreur", e.date.quand()),
                ),
                None => (String::new(), String::new()),
            };
            l.ajoute("testmemoire-erreurs", erreurs, t_erreurs, String::new(), true, court6);
            l.ajoute("testmemoire-ok", !ev.is_empty() && !erreurs, t_ok, String::new(), true, court6);
        }
    }

    l.continuer()?;
    // Le disque du PC : ses erreurs, et ce que Windows dit de sa santé (les clés USB et disques externes ne comptent pas)
    let (internes, lettres) = match source.disques() {
        Err(_) => (None, None),
        Ok(disques) => {
            let internes: Vec<i64> = disques
                .iter()
                .filter(|d| d.bus.map_or("Unknown", nom_bus) != "USB")
                .map(|d| i64::from(d.numero))
                .collect();
            let lettres = source.partitions().ok().map(|p| {
                p.iter()
                    .filter(|x| internes.contains(&i64::from(x.disque)))
                    .filter_map(|x| char::from_u32(u32::from(x.lettre)).filter(char::is_ascii_alphabetic))
                    .map(|c| c.to_ascii_uppercase().to_string())
                    .collect::<Vec<_>>()
            });
            (Some(internes), lettres)
        }
    };
    let mut echec = false;
    let mut ev: Vec<Evenement> = Vec::new();
    if illisible.is_empty() {
        for (fournisseur, ids) in [("disk", &[7u32, 51, 153][..]), ("Ntfs", &[55][..])] {
            match l.evenements(fournisseur, ids, &[], depuis, MAX_EVENEMENTS) {
                Some(e) => ev.extend(e),
                None => echec = true,
            }
        }
    }
    let ev = erreurs_disque(&ev, internes.as_deref(), lettres.as_deref());
    let malades: Vec<_> = match source.disques_physiques() {
        Ok(d) => {
            d.into_iter().filter(|d| d.bus.map_or("", nom_bus) != "USB" && nom_sante(d.sante) != "Healthy").collect()
        }
        Err(_) => {
            echec = true;
            Vec::new()
        }
    };
    let texte = match malades.first() {
        Some(m) => format!("Windows dit qu'un disque du PC n'est pas en bonne santé ({})", m.nom),
        None => format!("Le disque du PC a signalé {} {periode}", quantite(ev.len() as i64, "erreur", "erreurs")),
    };
    // (journal illisible : seul un disque que Windows dit malade compte ; « en bonne santé » ne se dit pas sans le
    // journal, ni sans la santé des disques : constat 20)
    let vu = !malades.is_empty() || !ev.is_empty();
    if (illisible.is_empty() || !malades.is_empty()) && (vu || !echec) {
        l.ajoute("disque", vu, texte, "Disques en bonne santé".to_string(), true, court);
    }

    if illisible.is_empty() {
        l.continuer()?;
        // Le pilote de la carte graphique qui plante (puis repart)
        if let Some(ev) = l.evenements("Display", &[4101], &[], depuis, MAX_EVENEMENTS) {
            let n = ev.len() as i64;
            let t = format!("Le pilote de la carte graphique a planté {} {periode}", quantite(n, "fois", "fois"));
            l.ajoute("pilotegraphique", n > 0, t, format!("Aucun plantage du pilote graphique {periode}"), true, court);
        }

        l.continuer()?;
        // Redémarrages lancés par les mises à jour de Windows
        if let Some(ev) = l.evenements("User32", &[1074], &[], depuis, MAX_EVENEMENTS) {
            let mots = ["TrustedInstaller", "MoUsoCoreWorker", "UsoClient", "wuauclt", "musnotification"];
            let ev: Vec<Evenement> = ev
                .into_iter()
                .filter(|e| contient_un(e.proprietes.first().map(String::as_str).unwrap_or(""), &mots))
                .collect();
            let n = ev.len() as i64;
            let t = format!(
                "{} pour les mises à jour de Windows{}",
                quantite(n, "redémarrage", "redémarrages"),
                dernier(&ev)
            );
            l.ajoute(
                "majredemarrage",
                n > 0,
                t,
                format!("Aucun redémarrage pour les mises à jour {periode}"),
                true,
                court,
            );
        }

        l.continuer()?;
        // Le processeur ralenti par la carte mère (événement 37 : « vitesse limitée par le microprogramme »)
        if let Some(ev) = l.evenements("Microsoft-Windows-Kernel-Processor-Power", &[37], &[], depuis, MAX_EVENEMENTS) {
            let n = ev.len() as i64;
            let t = format!(
                "Le processeur a été ralenti {} {periode} (souvent pour économiser l'énergie, parfois à cause de la chaleur)",
                quantite(n, "fois", "fois")
            );
            l.ajoute("freinage", n > 0, t, format!("Processeur jamais ralenti {periode}"), true, court);
        }
    }

    l.continuer()?;
    // La place libre sur le disque de Windows (illisible : pas d'indice, constat 20)
    let lecteur = source.lecteur_systeme();
    let place = if lecteur.is_empty() { Err(String::new()) } else { source.place(lecteur.trim_end_matches(':')) };
    if let Ok(p) = place {
        let libre = nombre(arrondi(p.libre as f64 / 1_073_741_824.0));
        let part = arrondi(100.0 * p.libre as f64 / p.taille as f64);
        l.ajoute(
            "placelibre",
            part < 10.0,
            format!("Plus que {libre} Go libres sur le disque {lecteur} ({} %)", nombre(part)),
            format!("{libre} Go libres sur le disque {lecteur}"),
            true,
            false,
        );
    }

    l.continuer()?;
    // L'usure de la batterie (portables) : sa capacité actuelle, comparée à celle d'origine. Aucune batterie dans le
    // rapport : comme PowerShell ; rapport illisible, capacités illisibles ou d'origine inconnue : pas d'indice
    // (constat 20).
    match source.batterie() {
        Ok(None) => l.ajoute("usurebatterie", false, String::new(), String::new(), false, false),
        Ok(Some(b)) => {
            if let (Some(origine), Some(pleine)) = (double(&b.origine), double(&b.pleine))
                && origine > 0.0
            {
                let part = nombre(arrondi(100.0 * pleine / origine));
                let vu = arrondi(100.0 * pleine / origine) < 60.0;
                l.ajoute(
                    "usurebatterie",
                    vu,
                    format!("La batterie ne garde plus que {part} % de sa capacité d'origine"),
                    format!("La batterie garde {part} % de sa capacité d'origine"),
                    true,
                    false,
                );
            }
        }
        Err(_) => {}
    }

    l.continuer()?;
    // Le dernier vrai redémarrage (« Arrêter » ne compte pas ; illisible : pas d'indice, constat 20)
    if let Ok(d) = source.demarrage() {
        let jours = arrondi(maintenant.jours_depuis(d)) as i64;
        let ok = if jours < 1 {
            "Redémarré aujourd'hui".to_string()
        } else {
            format!("Redémarré il y a {}", quantite(jours, "jour", "jours"))
        };
        l.ajoute("allume", jours >= 7, format!("Pas vraiment redémarré depuis {jours} jours"), ok, true, false);
    }
    Ok(l.i)
}

/// Les indices pour le moteur d'enquête (ses `Indices` : sans « affiché »).
pub fn vers_moteur(indices: &IndicesWindows) -> mindcare_moteur::Indices {
    indices
        .iter()
        .map(|(k, v)| {
            let i = mindcare_moteur::Indice {
                vu: v.vu,
                texte: v.texte.clone(),
                texte_ok: v.texte_ok.clone(),
                partiel: v.partiel,
            };
            (k.clone(), i)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_complete() {
        assert_eq!(CODES_ECRAN_BLEU.len(), 22);
        assert_eq!(code_ecran_bleu(0x9F), Some(("DRIVER_POWER_STATE_FAILURE", "pilote")));
        assert_eq!(code_ecran_bleu(0), None);
    }
}
