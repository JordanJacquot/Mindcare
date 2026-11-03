//! Le faux whisper-cli des tests (comme celui de outils\tester-dictee.ps1), copié par chaque test dans son propre
//! dossier de dictée sous le nom whisper-cli.exe. Il note ses arguments dans `arguments.txt` (à côté de lui), puis
//! suit `consigne.txt` (à côté de lui aussi, pour que les tests tournent en même temps sans se gêner) :
//! - « attendre » : il attend 60 secondes sans rien écrire (whisper bloqué) ;
//! - « lancer <dossier> » : il joue Mindcare : il lance `<dossier>\whisper-cli.exe` comme la dictée (sans fenêtre,
//!   relié à lui par le Job Object), note son numéro dans `pid.txt` (à côté de lui), puis s'arrête aussitôt ; le
//!   programme lancé doit s'arrêter avec lui ;
//! - « code N <phrase> » : il écrit la phrase et sort avec le code N ;
//! - sinon : il écrit la consigne telle quelle dans le fichier -of + « .txt », comme whisper.
//!
//! Il n'écrit que dans son dossier et dans le fichier -of, et ne lance que le faux whisper d'un dossier d'essai.

use std::path::PathBuf;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ici = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)).unwrap_or_default();
    let _ = std::fs::write(ici.join("arguments.txt"), a.join("\n"));
    let consigne = std::fs::read_to_string(ici.join("consigne.txt")).unwrap_or_default();
    #[cfg(windows)]
    if let Some(dossier) = consigne.strip_prefix("lancer ") {
        use mindcare_dictee::Lanceur;
        let dossier = PathBuf::from(dossier);
        let lance = mindcare_dictee::windows::LanceurWindows.lancer(&dossier.join("whisper-cli.exe"), &[], &dossier);
        let note = match lance {
            Ok(l) => format!("{} {}", l.processus.numero(), if l.relie.is_ok() { "relie" } else { "seul" }),
            Err(e) => format!("0 {e}"),
        };
        let _ = std::fs::write(ici.join("pid.txt"), note);
        std::process::exit(0); // (sans attendre le programme lancé : il doit s'arrêter avec nous)
    }
    if consigne == "attendre" {
        std::thread::sleep(std::time::Duration::from_secs(60));
        return;
    }
    let (code, phrase) = match consigne.strip_prefix("code ").and_then(|r| r.split_once(' ')) {
        Some((c, p)) => (c.parse().unwrap_or(1), p.to_string()),
        None => (0, consigne),
    };
    if let Some(i) = a.iter().position(|x| x == "-of")
        && let Some(sortie) = a.get(i + 1)
    {
        let _ = std::fs::write(format!("{sortie}.txt"), phrase.as_bytes());
    }
    std::process::exit(code);
}
