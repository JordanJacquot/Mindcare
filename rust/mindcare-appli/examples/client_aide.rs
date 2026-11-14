//! Le client de l'essai en vrai de l'Aide de Mindcare (outils\essai-aide-reelle.ps1, workflow aide-reelle.yml) :
//! JAMAIS livré, jamais lancé sur un vrai PC. Sur la machine jetable de GitHub, il est mis dans le paquet d'essai à la
//! place de Mindset.exe (le programme que l'Aide accepte : Mindset.exe du dossier de Mindcare), et il se sert du VRAI
//! client de Mindcare (`mindcare_appli::windows::aide::AideWindows` : état de la tâche, réveil par schtasks, canal
//! vérifié, journal recopié).
//!
//! `client_aide <sortie.json> <mode> [valeurs...]`, la réponse écrite en JSON dans `<sortie.json>` :
//! - `actions <nom>...` : `presente()`, puis chaque action de l'Aide (`aide.bonjour`, `maj.vider`...) par `faire()` ;
//! - `brut <canal> <ligne>` : ouvre ce canal sans rien vérifier et envoie la ligne (un appelant qui n'est pas Mindcare,
//!   ou un autre compte) ;
//! - `tache` : `schtasks /Run` et `schtasks /Query` de la tâche de l'Aide, leurs codes de sortie ;
//! - `garder <secondes>` : reste ouvert (l'Aide voit Mindcare ouvert).
//!
//! Aucune fenêtre (programme sans console, schtasks sans fenêtre).

#![windows_subsystem = "windows"]

#[cfg(all(windows, not(feature = "essai")))]
fn main() {
    use mindcare_aide_regles::demandes::Demande;
    use mindcare_appli::reparer::AideAdmin;
    use mindcare_appli::windows::aide::AideWindows;
    use serde_json::{Map, Value, json};
    use std::io::{BufRead, BufReader, Write};
    use std::os::windows::process::CommandExt;
    use std::time::Duration;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(sortie) = args.first().cloned() else { std::process::exit(2) };
    let mode = args.get(1).cloned().unwrap_or_default();
    let valeurs = &args[2.min(args.len())..];
    let racine = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let mut r = Map::new();
    r.insert("racine".into(), json!(racine));
    r.insert("exe".into(), json!(std::env::current_exe().map(|e| e.to_string_lossy().into_owned()).ok()));
    match mode.as_str() {
        "actions" => match AideWindows::nouvelle(&racine) {
            None => {
                r.insert("erreur".into(), json!("AideWindows::nouvelle : rien"));
            }
            Some(a) => {
                r.insert("presente".into(), json!(a.presente()));
                let mut reponses = Map::new();
                let mut journal = Vec::new();
                for nom in valeurs {
                    let Some(d) = Demande::depuis_nom(nom) else {
                        reponses.insert(nom.clone(), json!({ "ok": false, "e": "action inconnue du client" }));
                        continue;
                    };
                    let (res, lignes) = a.faire(d, mindcare_appli::reparer::delai_aide(d) + Duration::from_secs(30));
                    journal.extend(lignes);
                    reponses.insert(
                        nom.clone(),
                        match res {
                            Ok(v) => json!({ "ok": true, "r": v }),
                            Err(e) => json!({ "ok": false, "e": e }),
                        },
                    );
                }
                r.insert("reponses".into(), Value::Object(reponses));
                r.insert("journal".into(), json!(journal));
            }
        },
        "brut" => {
            let (canal, ligne) =
                (valeurs.first().cloned().unwrap_or_default(), valeurs.get(1).cloned().unwrap_or_default());
            match std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\{canal}")) {
                Err(e) => {
                    r.insert("ouvert".into(), json!(false));
                    r.insert("erreur".into(), json!(format!("{e} ({:?})", e.raw_os_error())));
                }
                Ok(mut f) => {
                    r.insert("ouvert".into(), json!(true));
                    let _ = f.write_all(format!("{ligne}\r\n").as_bytes());
                    let mut l = String::new();
                    let lu = BufReader::new(f).read_line(&mut l);
                    r.insert("reponse".into(), json!(l.trim_end()));
                    r.insert("lecture".into(), json!(format!("{lu:?}")));
                }
            }
        }
        "tache" => {
            let schtasks =
                format!(r"{}\System32\schtasks.exe", std::env::var("SystemRoot").unwrap_or(r"C:\Windows".into()));
            for (cle, quoi) in [("lancer", "/Run"), ("lire", "/Query")] {
                let code = std::process::Command::new(&schtasks)
                    .args([quoi, "/TN", mindcare_aide_regles::tache::TACHE])
                    .creation_flags(0x0800_0000)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .ok()
                    .and_then(|s| s.code());
                r.insert(cle.into(), json!(code));
            }
        }
        "garder" => {
            let s = valeurs.first().and_then(|v| v.parse().ok()).unwrap_or(30);
            std::thread::sleep(Duration::from_secs(s));
        }
        _ => {
            r.insert("erreur".into(), json!(format!("mode inconnu « {mode} »")));
        }
    }
    let _ = std::fs::write(&sortie, serde_json::to_vec_pretty(&Value::Object(r)).unwrap_or_default());
}

/// (la construction d'essai et les autres systèmes n'ont pas le vrai client : rien)
#[cfg(not(all(windows, not(feature = "essai"))))]
fn main() {}
