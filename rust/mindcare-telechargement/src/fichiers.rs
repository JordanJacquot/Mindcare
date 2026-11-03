//! Les fichiers à télécharger (`$IaFichiers`, `$DicteeFichiers`) et ce qui est déjà installé : `Test-IaMoteur`,
//! `Test-IaIncomplete`, `Get-IaManquants`, `Get-IaModeleLance`, `Remove-IaAnciens` et leurs équivalents de la dictée.
//!
//! Les adresses, tailles, empreintes et motifs d'extraction sont exactement ceux du PowerShell : un test les relit
//! dans Mindcare\fichiers\modules\ia.ps1 et dictee.ps1 et les compare.

use std::fs;
use std::path::{Path, PathBuf};

/// Un mégaoctet (1MB de PowerShell)
pub const MO: u64 = 1024 * 1024;

/// Un fichier à télécharger (une ligne de `$IaFichiers`)
#[derive(Clone, Debug, PartialEq)]
pub struct Fichier {
    /// Son nom pour les gens (« moteur de l'assistant »), montré pendant l'installation et dans les messages
    pub nom: String,
    pub url: String,
    /// L'empreinte SHA-256 attendue, en hexadécimal (majuscules ou minuscules)
    pub sha256: String,
    /// Sa taille en octets
    pub taille: u64,
    /// Un zip dont on n'extrait que ces fichiers ; `None` : le fichier est gardé tel quel
    pub extraire: Option<Extraire>,
    /// Le fichier gardé (le programme pour un zip, sinon le fichier lui-même)
    pub garder: PathBuf,
}

impl Fichier {
    /// Le nom du fichier en cours de téléchargement : le nom de l'adresse, plus « .telechargement »
    /// (`[IO.Path]::GetFileName($f.Url) + '.telechargement'`). Il reste sur le disque après une coupure : la reprise
    /// continue là où il s'est arrêté.
    pub fn nom_temporaire(&self) -> String {
        let nom = self.url.rsplit(['/', '\\']).next().unwrap_or_default();
        format!("{nom}.telechargement")
    }
}

/// Les fichiers à extraire d'un zip, par leur nom (sans le dossier du zip). Le motif de PowerShell,
/// `^(llama-server\.exe|llama\.dll|ggml[^/]*\.dll)$`, est fait de noms exacts et de motifs « ggml...dll », dans cet
/// ordre. Comme `-match`, sans tenir compte des majuscules.
#[derive(Clone, Debug, PartialEq)]
pub struct Extraire(pub Vec<Motif>);

/// Un morceau du motif d'extraction
#[derive(Clone, Debug, PartialEq)]
pub enum Motif {
    /// Un nom exact (« llama-server.exe »)
    Nom(String),
    /// Un début et une fin (« ggml », « .dll ») : `ggml[^/]*\.dll`
    DebutFin(String, String),
}

impl Extraire {
    /// Ce nom (le dernier morceau du chemin dans le zip) est-il à extraire ?
    pub fn voulu(&self, nom: &str) -> bool {
        let n = nom.to_ascii_lowercase();
        self.0.iter().any(|m| match m {
            Motif::Nom(v) => v.to_ascii_lowercase() == n,
            Motif::DebutFin(d, f) => {
                let (d, f) = (d.to_ascii_lowercase(), f.to_ascii_lowercase());
                n.len() >= d.len() + f.len() && n.starts_with(&d) && n.ends_with(&f) && !n.contains('/')
            }
        })
    }

    /// Le motif écrit comme dans le PowerShell (un test le compare au vrai)
    pub fn motif_ps(&self) -> String {
        let echapper = |s: &str| s.replace('.', "\\.");
        let morceaux: Vec<String> = self
            .0
            .iter()
            .map(|m| match m {
                Motif::Nom(n) => echapper(n),
                Motif::DebutFin(d, f) => format!("{}[^/]*{}", echapper(d), echapper(f)),
            })
            .collect();
        format!("^({})$", morceaux.join("|"))
    }
}

/// Un ensemble à installer : l'assistant (dossier « ia ») ou la dictée (dossier « dictee »)
#[derive(Clone, Debug, PartialEq)]
pub struct Paquet {
    /// Le dossier (dans le dossier de données de Mindcare)
    pub dossier: PathBuf,
    pub fichiers: Vec<Fichier>,
    /// Le programme (llama-server.exe, whisper-cli.exe)
    pub programme: PathBuf,
    /// Les .dll sans lesquelles le programme ne démarre pas (`$IaMoteurDll`, `$DicteeDll`) : une installation coupée
    /// pendant l'extraction peut avoir laissé le programme sans elles
    pub dll: Vec<String>,
    /// Le modèle de cette version de Mindcare
    pub modele: PathBuf,
    /// Les anciens modèles (début et fin de leur nom, « mindcare- » et « .gguf ») : l'un d'eux sert en attendant la
    /// mise à jour, ils sont effacés après. `None` pour la dictée (pas d'ancien modèle).
    pub anciens: Option<(String, String)>,
    /// La place gardée en plus sur le disque (200 Mo pour l'assistant, 50 Mo pour la dictée)
    pub marge: u64,
}

fn nom(s: &str) -> String {
    s.to_string()
}

impl Paquet {
    /// L'assistant (`$IaFichiers`), dans le dossier « ia » des données de Mindcare
    pub fn assistant(donnees: &Path) -> Paquet {
        let dossier = donnees.join("ia");
        let programme = dossier.join("llama-server.exe");
        let modele = dossier.join("mindcare-0.6b-v3-q8_0.gguf");
        Paquet {
            fichiers: vec![
                Fichier {
                    nom: nom("moteur de l'assistant"),
                    url: nom(
                        "https://github.com/ggml-org/llama.cpp/releases/download/b11541/llama-b11541-bin-win-vulkan-x64.zip",
                    ),
                    sha256: nom("37BBA511D120222F13C3D520DD875F739A530CC124E78F7CBCE0026DE0173D8A"),
                    taille: 33_464_528,
                    extraire: Some(Extraire(vec![
                        Motif::Nom(nom("llama-server.exe")),
                        Motif::Nom(nom("llama-server-impl.dll")),
                        Motif::Nom(nom("llama.dll")),
                        Motif::Nom(nom("llama-common.dll")),
                        Motif::DebutFin(nom("ggml"), nom(".dll")),
                        Motif::Nom(nom("mtmd.dll")),
                        Motif::Nom(nom("libomp.dll")),
                    ])),
                    garder: programme.clone(),
                },
                Fichier {
                    nom: nom("cerveau de l'assistant"),
                    url: nom(
                        "https://github.com/JordanJacquot/Mindcare/releases/download/ia-v3/mindcare-0.6b-v3-q8_0.gguf",
                    ),
                    sha256: nom("C9598811845651239AFCEFFEC641B89D01B96F5DD9D9721334D22562D58E6670"),
                    taille: 639_446_912,
                    extraire: None,
                    garder: modele.clone(),
                },
            ],
            programme,
            dll: vec![nom("llama.dll"), nom("ggml.dll")],
            modele,
            anciens: Some((nom("mindcare-"), nom(".gguf"))),
            marge: 200 * MO,
            dossier,
        }
    }

    /// La dictée (`$DicteeFichiers`), dans le dossier « dictee » des données de Mindcare
    pub fn dictee(donnees: &Path) -> Paquet {
        let dossier = donnees.join("dictee");
        let programme = dossier.join("whisper-cli.exe");
        let modele = dossier.join("ggml-small-q5_1.bin");
        Paquet {
            fichiers: vec![
                Fichier {
                    nom: nom("programme de dictée"),
                    url: nom("https://github.com/ggml-org/whisper.cpp/releases/download/v1.9.2/whisper-bin-x64.zip"),
                    sha256: nom("49DCC16DE826F20BD53D44F947A1AE49DFA81F86CAD67A64D80820CB192D674A"),
                    taille: 8_194_445,
                    extraire: Some(Extraire(vec![
                        Motif::Nom(nom("whisper-cli.exe")),
                        Motif::Nom(nom("whisper.dll")),
                        Motif::DebutFin(nom("ggml"), nom(".dll")),
                    ])),
                    garder: programme.clone(),
                },
                Fichier {
                    nom: nom("français de la dictée"),
                    url: nom(
                        "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-small-q5_1.bin",
                    ),
                    sha256: nom("AE85E4A935D7A567BD102FE55AFC16BB595BDB618E11B2FC7591BC08120411BB"),
                    taille: 190_085_487,
                    extraire: None,
                    garder: modele.clone(),
                },
            ],
            programme,
            dll: vec![nom("whisper.dll"), nom("ggml.dll"), nom("ggml-base.dll")],
            modele,
            anciens: None,
            marge: 50 * MO,
            dossier,
        }
    }

    /// Le programme et toutes ses .dll sont là (`Test-IaMoteur`, `Test-DicteeMoteur`)
    pub fn moteur_present(&self) -> bool {
        if !self.programme.is_file() {
            return false;
        }
        let dossier = self.programme.parent().unwrap_or(Path::new("."));
        self.dll.iter().all(|d| dossier.join(d).exists())
    }

    /// Il manque seulement une partie (installation coupée) : à réparer (`Test-IaIncomplete`)
    pub fn incomplet(&self) -> bool {
        self.programme.exists() && !self.moteur_present()
    }

    /// Le modèle à lancer : celui de cette version, sinon un plus ancien déjà installé, le plus récent
    /// (`Get-IaModeleLance`). Il sert en attendant la mise à jour.
    pub fn modele_lance(&self) -> Option<PathBuf> {
        if self.modele.exists() {
            return Some(self.modele.clone());
        }
        let mut anciens = self.anciens_modeles();
        anciens.sort_by_key(|(_, quand)| *quand);
        anciens.pop().map(|(p, _)| p)
    }

    /// Le moteur et un modèle sont là (`Test-IaInstallee`, `Test-DicteeInstallee`)
    pub fn installe(&self) -> bool {
        self.moteur_present() && self.modele_lance().is_some()
    }

    /// Le moteur et le modèle de cette version sont là (`Test-IaAJour`)
    pub fn a_jour(&self) -> bool {
        self.moteur_present() && self.modele.exists()
    }

    /// Les fichiers à télécharger (`Get-IaManquants`) : le zip si le moteur n'est pas complet, les autres s'ils
    /// manquent
    pub fn manquants(&self) -> Vec<Fichier> {
        self.fichiers
            .iter()
            .filter(|f| if f.extraire.is_some() { !self.moteur_present() } else { !f.garder.exists() })
            .cloned()
            .collect()
    }

    /// La taille de ce qui manque, en Mo arrondis au-dessus (`Get-DicteeTaille`)
    pub fn taille_manquants_mo(&self) -> u64 {
        self.manquants().iter().map(|f| f.taille).sum::<u64>().div_ceil(MO)
    }

    /// Les modèles du dossier dont le nom va avec `anciens` (celui de cette version compris), avec leur date
    fn anciens_modeles(&self) -> Vec<(PathBuf, std::time::SystemTime)> {
        let Some((debut, fin)) = &self.anciens else { return Vec::new() };
        let (debut, fin) = (debut.to_ascii_lowercase(), fin.to_ascii_lowercase());
        let Ok(lus) = fs::read_dir(&self.dossier) else { return Vec::new() };
        lus.flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                let md = e.metadata().ok()?;
                (md.is_file() && n.starts_with(&debut) && n.ends_with(&fin))
                    .then(|| (e.path(), md.modified().unwrap_or(std::time::UNIX_EPOCH)))
            })
            .collect()
    }

    /// Après une mise à jour : les anciens modèles sont effacés (640 Mo chacun), seulement si celui de cette version
    /// est là (`Remove-IaAnciens`). Rend les lignes du journal. L'appelant arrête d'abord llama-server, qui peut
    /// encore lire l'ancien.
    pub fn effacer_anciens(&self) -> Vec<String> {
        let mut journal = Vec::new();
        if !self.modele.exists() {
            return journal;
        }
        let garde = self.modele.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase());
        for (p, _) in self.anciens_modeles() {
            let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if Some(n.to_ascii_lowercase()) == garde {
                continue;
            }
            match fs::remove_file(&p) {
                Ok(()) => journal.push(format!("Ancien modèle effacé : {n}")),
                Err(e) => journal.push(format!("Ancien modèle non effacé : {e}")),
            }
        }
        journal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_voulus_comme_le_motif_de_powershell() {
        let ia = Paquet::assistant(Path::new("C:\\d"));
        let x = ia.fichiers[0].extraire.as_ref().unwrap();
        for oui in [
            "llama-server.exe",
            "LLAMA.DLL",
            "ggml.dll",
            "ggml-vulkan.dll",
            "ggml-cpu-x64.dll",
            "mtmd.dll",
            "libomp.dll",
        ] {
            assert!(x.voulu(oui), "{oui}");
        }
        for non in
            ["llama-cli.exe", "ggml.dll.bak", "ggml", "xggml.dll", "readme.md", "llama-server.exe.old", "ggm.dll", ""]
        {
            assert!(!x.voulu(non), "{non}");
        }
    }

    #[test]
    fn nom_temporaire_comme_powershell() {
        let ia = Paquet::assistant(Path::new("C:\\d"));
        assert_eq!(ia.fichiers[0].nom_temporaire(), "llama-b11541-bin-win-vulkan-x64.zip.telechargement");
        assert_eq!(ia.fichiers[1].nom_temporaire(), "mindcare-0.6b-v3-q8_0.gguf.telechargement");
    }
}
