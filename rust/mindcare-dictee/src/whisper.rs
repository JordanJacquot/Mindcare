//! whisper-cli, sans Windows : ses arguments (`Start-DicteeEcriture`), le nombre de fils (`Get-DicteeFils`), les
//! fichiers de voix dans le dossier temporaire, et ceux laissés par une fermeture forcée (constat 25n).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Les mots qu'on s'attend à entendre aident whisper (`$DicteeAttendu`, sans accents : ils passent mal sur la ligne de
/// commande)
pub const ATTENDU: &str = "Mon ordinateur ne demarre plus, l'ecran reste noir, le PC s'eteint quand je joue, le wifi coupe, la souris, le clavier, Windows.";

/// Au-delà, un fichier de voix est forcément orphelin, même si un programme porte encore le numéro de processus de son
/// nom (une écoute dure 30 secondes au plus, whisper 60 secondes au plus)
pub const ORPHELIN_APRES: Duration = Duration::from_secs(10 * 60);

/// `Get-DicteeFils` : la moitié des processeurs, entre 2 et 8. `[int]` de PowerShell arrondit une moitié au nombre pair
/// (7 / 2 = 3,5 donne 4 ; 5 / 2 = 2,5 donne 2).
pub fn fils(processeurs: usize) -> usize {
    let k = processeurs / 2;
    let moitie = if processeurs % 2 == 1 && k % 2 == 1 { k + 1 } else { k };
    moitie.clamp(2, 8)
}

/// Le son enregistré : `<temp>\mindcare-<pid>-dictee.wav`
pub fn fichier_son(temp: &Path, pid: u32) -> PathBuf {
    temp.join(format!("mindcare-{pid}-dictee.wav"))
}

/// Ce que whisper écrit, sans son « .txt » (l'argument -of) : `<temp>\mindcare-<pid>-dictee`
pub fn sortie(temp: &Path, pid: u32) -> PathBuf {
    temp.join(format!("mindcare-{pid}-dictee"))
}

/// Le fichier texte que whisper écrit (-otxt ajoute « .txt » à -of)
pub fn sortie_txt(temp: &Path, pid: u32) -> PathBuf {
    temp.join(format!("mindcare-{pid}-dictee.txt"))
}

/// Les arguments de whisper-cli, dans l'ordre du PowerShell : le modèle, le son, en français, sans les heures ni les
/// messages de progression, `fils` fils, le texte dans `sortie`.txt, et les mots attendus
pub fn arguments(modele: &Path, son: &Path, fils: usize, sortie: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = Vec::with_capacity(16);
    a.push("-m".into());
    a.push(modele.into());
    a.push("-f".into());
    a.push(son.into());
    for x in ["-l", "fr", "-nt", "-np", "-t"] {
        a.push(x.into());
    }
    a.push(fils.to_string().into());
    a.push("-otxt".into());
    a.push("-of".into());
    a.push(sortie.into());
    a.push("--prompt".into());
    a.push(ATTENDU.into());
    a
}

/// Le numéro de processus d'un fichier de voix (`mindcare-<pid>-dictee.wav` ou `.txt`, majuscules permises comme
/// Windows), sinon rien
pub fn pid_du_fichier(nom: &str) -> Option<u32> {
    let n = nom.to_ascii_lowercase();
    let reste = n.strip_prefix("mindcare-")?;
    let chiffres = reste.strip_suffix("-dictee.wav").or_else(|| reste.strip_suffix("-dictee.txt"))?;
    if chiffres.is_empty() || chiffres.len() > 10 || !chiffres.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    chiffres.parse().ok()
}

/// Constat 25n : au démarrage, les fichiers de voix laissés dans le dossier temporaire par une fermeture forcée sont
/// effacés. Un fichier est orphelin s'il porte notre numéro (on vient de démarrer : il n'est pas à nous), si son
/// processus n'est plus là, ou s'il a plus de 10 minutes. Celui d'une autre Mindcare encore ouverte, récent, est
/// laissé. Rend le nombre de fichiers effacés.
pub fn effacer_orphelins(temp: &Path, pid: u32, vivant: &dyn Fn(u32) -> bool, maintenant: SystemTime) -> usize {
    let Ok(lus) = std::fs::read_dir(temp) else { return 0 };
    let mut effaces = 0;
    for e in lus.flatten() {
        let Some(p) = pid_du_fichier(&e.file_name().to_string_lossy()) else { continue };
        let Ok(md) = e.metadata() else { continue };
        if !md.is_file() {
            continue;
        }
        let vieux =
            md.modified().ok().and_then(|m| maintenant.duration_since(m).ok()).is_some_and(|age| age > ORPHELIN_APRES);
        if (p == pid || vieux || !vivant(p)) && std::fs::remove_file(e.path()).is_ok() {
            effaces += 1;
        }
    }
    effaces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fils_comme_powershell() {
        // [math]::Max(2, [math]::Min(8, [int]($n / 2))), mesuré dans Windows PowerShell 5.1
        let attendu = [(1, 2), (2, 2), (3, 2), (4, 2), (5, 2), (6, 3), (7, 4), (8, 4), (9, 4), (10, 5), (11, 6)];
        for (n, f) in attendu {
            assert_eq!(fils(n), f, "{n} processeurs");
        }
        for (n, f) in [(12, 6), (13, 6), (14, 7), (15, 8), (16, 8), (17, 8), (32, 8), (0, 2)] {
            assert_eq!(fils(n), f, "{n} processeurs");
        }
    }

    #[test]
    fn arguments_comme_powershell() {
        let a = arguments(Path::new(r"C:\d\ggml-small-q5_1.bin"), Path::new(r"C:\t\son.wav"), 4, Path::new(r"C:\t\s"));
        let a: Vec<String> = a.iter().map(|x| x.to_string_lossy().into_owned()).collect();
        assert_eq!(
            a,
            [
                "-m",
                r"C:\d\ggml-small-q5_1.bin",
                "-f",
                r"C:\t\son.wav",
                "-l",
                "fr",
                "-nt",
                "-np",
                "-t",
                "4",
                "-otxt",
                "-of",
                r"C:\t\s",
                "--prompt",
                ATTENDU
            ]
        );
        assert!(ATTENDU.is_ascii());
    }

    #[test]
    fn noms_des_fichiers_de_voix() {
        assert_eq!(pid_du_fichier("mindcare-1234-dictee.wav"), Some(1234));
        assert_eq!(pid_du_fichier("Mindcare-77-Dictee.TXT"), Some(77));
        for autre in [
            "mindcare-batterie-12.xml",
            "mindcare--dictee.wav",
            "mindcare-12-dictee.wav.bak",
            "mindcare-1a-dictee.wav",
            "mindcare-99999999999-dictee.wav",
            "evermind-12-dictee.wav",
            "mindcare-12-dictee",
        ] {
            assert_eq!(pid_du_fichier(autre), None, "{autre}");
        }
    }
}
