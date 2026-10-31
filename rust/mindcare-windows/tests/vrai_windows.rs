//! Un essai sur le VRAI Windows de ce PC, en lecture seule : le module Windows du Rust lit le journal Système, WMI, la
//! place et la batterie, puis Get-IndicesWindows (le vrai journaux.ps1) fait de même dans un PowerShell caché, et les
//! deux doivent trouver les mêmes indices. Pas lancé par `cargo test` (il dépend de la machine) :
//!
//!   cargo test -p mindcare-windows --test vrai_windows -- --ignored --nocapture
//!
//! Rien n'est écrit hors d'un dossier temporaire créé par l'essai (le rapport de batterie de powercfg, des deux côtés),
//! effacé à la fin. Aucune fenêtre (PowerShell et powercfg sont lancés sans fenêtre). Rien ne demande de droits
//! administrateur.
#![cfg(windows)]

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::time::Instant;

use mindcare_windows::windows::{HorlogeWindows, WindowsReel};
use mindcare_windows::{Horloge, IndicesWindows, JOURS, SourceWindows, barrettes, indices_windows};
use serde_json::Value;

const SANS_FENETRE: u32 = 0x0800_0000;

const SCRIPT: &str = r#"param([string]$Journaux, [string]$Sortie)
function Write-Log { }
$EnTest = $false
. $Journaux
$sw = [Diagnostics.Stopwatch]::StartNew()
$i = Get-IndicesWindows
$ms = $sw.ElapsedMilliseconds
$indices = [ordered]@{}
foreach ($k in @($i.Keys | Sort-Object)) { $v = $i[$k]; $indices[$k] = [ordered]@{ Vu = $v.Vu; Texte = $v.Texte; TexteOk = $v.TexteOk; Affiche = $v.Affiche; Partiel = $v.Partiel } }
$b = @(Get-Barrettes)
$cs = Get-CimInstance Win32_ComputerSystem; $os = Get-CimInstance Win32_OperatingSystem
$r = [ordered]@{ Duree = $ms; Indices = $indices; Barrettes = $b; Fabricant = [string]$cs.Manufacturer; Modele = [string]$cs.Model; Systeme = [string]$os.Caption }
[IO.File]::WriteAllText($Sortie, (ConvertTo-Json -InputObject $r -Depth 6), (New-Object Text.UTF8Encoding($false)))
"#;

/// Les requêtes au journal que fait Get-IndicesWindows (pour mesurer leur temps).
fn indices_journal(source: &WindowsReel, maintenant: mindcare_windows::Moment) -> usize {
    let depuis = maintenant.plus_jours(-JOURS);
    let requetes: [(&str, &[u32], &[u8]); 9] = [
        ("Microsoft-Windows-Kernel-Power", &[41], &[]),
        ("Microsoft-Windows-WER-SystemErrorReporting", &[1001], &[]),
        ("Microsoft-Windows-WHEA-Logger", &[], &[1, 2]),
        ("Microsoft-Windows-MemoryDiagnostics-Results", &[1101, 1102], &[]),
        ("disk", &[7, 51, 153], &[]),
        ("Ntfs", &[55], &[]),
        ("Display", &[4101], &[]),
        ("User32", &[1074], &[]),
        ("Microsoft-Windows-Kernel-Processor-Power", &[37], &[]),
    ];
    requetes
        .iter()
        .map(|(f, ids, niveaux)| {
            let filtre = mindcare_windows::Filtre {
                fournisseur: f.to_string(),
                ids: ids.to_vec(),
                niveaux: niveaux.to_vec(),
                depuis,
                max: 200,
            };
            source.evenements(&filtre).map(|l| l.len()).unwrap_or(0)
        })
        .sum()
}

fn journaux_ps1() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(r"..\..\Mindcare\fichiers\modules\journaux.ps1")
}

fn indices_powershell(v: &Value) -> IndicesWindows {
    v.as_object()
        .expect("Indices")
        .iter()
        .map(|(k, i)| {
            let t = |c: &str| i[c].as_str().unwrap_or_default().to_string();
            let b = |c: &str| i[c].as_bool().unwrap_or_default();
            let ind = mindcare_windows::IndiceWindows {
                vu: b("Vu"),
                texte: t("Texte"),
                texte_ok: t("TexteOk"),
                affiche: b("Affiche"),
                partiel: b("Partiel"),
            };
            (k.clone(), ind)
        })
        .collect()
}

#[test]
#[ignore = "lit le vrai Windows de ce PC (lecture seule) : -- --ignored"]
fn vrai_windows_comme_powershell() {
    let dossier = std::env::temp_dir().join(format!("mindcare-essai-windows-{}", std::process::id()));
    let (temp_rust, temp_ps) = (dossier.join("rust"), dossier.join("powershell"));
    std::fs::create_dir_all(&temp_rust).expect("dossier temporaire");
    std::fs::create_dir_all(&temp_ps).expect("dossier temporaire");

    // Le Rust
    let source = WindowsReel { dossier_temp: temp_rust.clone() };
    let debut = Instant::now();
    let maintenant = HorlogeWindows.maintenant();
    let rust = indices_windows(&source, maintenant, JOURS);
    let duree_rust = debut.elapsed();
    let debut = Instant::now();
    let barrettes_rust = barrettes(&source);
    let infos = source.infos_pc();
    let duree_rust_reste = debut.elapsed();
    // (où passe le temps : chaque lecture seule, une seconde fois)
    let chrono = |quoi: &str, f: &dyn Fn()| {
        let d = Instant::now();
        f();
        println!("    {quoi:<34} {} ms", d.elapsed().as_millis());
    };
    println!("  Détail du Rust (seconde lecture) :");
    chrono("sonde du journal", &|| drop(source.plus_ancien()));
    chrono("9 requêtes au journal (indices)", &|| {
        let _ = indices_journal(&source, maintenant);
    });
    chrono("disques, partitions, santé (WMI)", &|| {
        drop((source.disques(), source.partitions(), source.disques_physiques()));
    });
    chrono("place libre", &|| drop(source.place("C")));
    chrono("batterie (powercfg)", &|| drop(source.batterie()));
    chrono("démarrage (WMI)", &|| drop(source.demarrage()));

    // PowerShell caché, avec le vrai journaux.ps1
    let script = dossier.join("lire.ps1");
    let mut octets = vec![0xEF, 0xBB, 0xBF];
    octets.extend_from_slice(SCRIPT.as_bytes());
    std::fs::write(&script, octets).expect("script");
    let sortie = dossier.join("powershell.json");
    let debut = Instant::now();
    let fin = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-Journaux")
        .arg(journaux_ps1())
        .arg("-Sortie")
        .arg(&sortie)
        .env("TEMP", &temp_ps)
        .env("TMP", &temp_ps)
        .stdin(std::process::Stdio::null())
        .creation_flags(SANS_FENETRE)
        .output()
        .expect("powershell");
    let duree_ps_totale = debut.elapsed();
    let texte = std::fs::read_to_string(&sortie)
        .unwrap_or_else(|e| panic!("PowerShell n'a rien écrit ({e}) : {}", String::from_utf8_lossy(&fin.stderr)));
    let ps: Value = serde_json::from_str(texte.trim_start_matches('\u{feff}')).expect("JSON de PowerShell");
    let powershell = indices_powershell(&ps["Indices"]);
    let restes = [
        std::fs::read_dir(&temp_rust).map(|d| d.count()).unwrap_or(0),
        std::fs::read_dir(&temp_ps).map(|d| d.count()).unwrap_or(0),
    ];
    let _ = std::fs::remove_dir_all(&dossier);

    println!("Lecture de Windows de ce PC :");
    println!(
        "  Rust       : {} ms (Get-IndicesWindows), {} ms de plus pour les barrettes et le modèle",
        duree_rust.as_millis(),
        duree_rust_reste.as_millis()
    );
    println!(
        "  PowerShell : {} ms (Get-IndicesWindows), {} ms en tout avec le démarrage de PowerShell",
        ps["Duree"],
        duree_ps_totale.as_millis()
    );
    for (k, i) in &rust {
        println!("  {k:<20} vu={:<5} {}", i.vu, if i.vu { &i.texte } else { &i.texte_ok });
    }
    println!("  barrettes  : Rust {barrettes_rust:?} / PowerShell {}", ps["Barrettes"]);
    println!("  PC         : Rust {infos:?} / PowerShell {} {}, {}", ps["Fabricant"], ps["Modele"], ps["Systeme"]);
    assert_eq!(restes, [0, 0], "le rapport de batterie doit être effacé");

    let mut ecarts = Vec::new();
    let mut noms: Vec<&String> = rust.keys().chain(powershell.keys()).collect();
    noms.sort();
    noms.dedup();
    for n in noms {
        if rust.get(n) != powershell.get(n) {
            ecarts.push(format!("{n} :\n    PowerShell {:?}\n    Rust       {:?}", powershell.get(n), rust.get(n)));
        }
    }
    let ps_barrettes: Vec<String> = ps["Barrettes"]
        .as_array()
        .map(|a| a.iter().filter_map(|b| b.as_str().map(str::to_string)).collect())
        .unwrap_or_else(|| ps["Barrettes"].as_str().map(|b| vec![b.to_string()]).unwrap_or_default());
    if barrettes_rust.clone().unwrap_or_default() != ps_barrettes {
        ecarts.push(format!("barrettes : PowerShell {ps_barrettes:?} / Rust {barrettes_rust:?}"));
    }
    let infos = infos.unwrap_or_default();
    if (infos.fabricant.as_str(), infos.modele.as_str(), infos.systeme.as_str())
        != (
            ps["Fabricant"].as_str().unwrap_or("?"),
            ps["Modele"].as_str().unwrap_or("?"),
            ps["Systeme"].as_str().unwrap_or("?"),
        )
    {
        ecarts.push(format!(
            "modèle du PC : PowerShell {} {} {} / Rust {infos:?}",
            ps["Fabricant"], ps["Modele"], ps["Systeme"]
        ));
    }
    assert!(ecarts.is_empty(), "{} écart(s) avec PowerShell :\n{}", ecarts.len(), ecarts.join("\n"));
    println!("Tout est pareil.");
}
