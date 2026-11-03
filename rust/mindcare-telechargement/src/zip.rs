//! La lecture des zips du moteur de l'assistant et de la dictée, sans bibliothèque de zip : seulement ce qu'il faut
//! pour extraire quelques fichiers (`[IO.Compression.ZipFile]::OpenRead`, `ExtractToFile`).
//!
//! - le sommaire du zip (le « répertoire central »), zip64 compris ;
//! - les fichiers rangés tels quels (méthode 0) ou compressés par deflate (méthode 8), la seule que donnent les
//!   publications de llama.cpp et whisper.cpp ; une autre méthode, ou un fichier chiffré, est refusé ;
//! - chaque fichier vérifié par sa taille et son CRC-32 ;
//! - seul le dernier morceau du nom compte (comme `ZipArchiveEntry.Name`) : un nom « ..\..\x.dll » ou « C:\x.dll »
//!   finit dans le dossier d'installation, jamais ailleurs.
//!
//! Chaque fichier est écrit à côté (« .extraction ») puis renommé : jamais de fichier à moitié écrit sous son vrai
//! nom. Le programme (llama-server.exe, whisper-cli.exe) en dernier : s'il est là, ses .dll aussi, même si Mindcare
//! a été fermée pendant.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use flate2::Crc;
use flate2::read::DeflateDecoder;

use crate::fichiers::Extraire;

/// Un fichier rangé dans le zip (une ligne du sommaire)
#[derive(Clone, Debug, PartialEq)]
pub struct Entree {
    /// Son chemin dans le zip, tel qu'écrit (« llama/llama-server.exe »)
    pub chemin: String,
    pub methode: u16,
    pub drapeaux: u16,
    pub crc: u32,
    pub taille_compressee: u64,
    pub taille: u64,
    /// Où commence son en-tête local dans le zip
    pub decalage: u64,
}

impl Entree {
    /// Son nom sans dossier (le dernier morceau du chemin, séparé par « / » ou « \ »), `None` pour un dossier ou un
    /// nom qui ne peut pas être un fichier (« .. », « . », « C: », un deux-points qui viserait un flux caché)
    pub fn nom(&self) -> Option<&str> {
        let n = self.chemin.rsplit(['/', '\\']).next()?;
        if n.is_empty() || n == "." || n == ".." || n.contains(':') || n.chars().any(|c| (c as u32) < 32) {
            return None;
        }
        Some(n)
    }
}

fn mauvais(texte: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, texte.to_string())
}

fn u16_de(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_de(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u64_de(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3], b[i + 4], b[i + 5], b[i + 6], b[i + 7]])
}

/// Le sommaire du zip
pub fn lire_sommaire(f: &mut File) -> io::Result<Vec<Entree>> {
    let longueur = f.metadata()?.len();
    if longueur < 22 {
        return Err(mauvais("zip trop court"));
    }
    // La fin du sommaire : dans les 22 derniers octets, plus un commentaire de 65535 octets au plus
    let fin_lue = longueur.min(22 + 65_535);
    f.seek(SeekFrom::Start(longueur - fin_lue))?;
    let mut fin = vec![0u8; fin_lue as usize];
    f.read_exact(&mut fin)?;
    let pos = (0..=fin.len() - 22)
        .rev()
        .find(|&i| fin[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .ok_or_else(|| mauvais("pas un zip"))?;
    let mut nombre = u64::from(u16_de(&fin, pos + 10));
    let mut taille_sommaire = u64::from(u32_de(&fin, pos + 12));
    let mut debut_sommaire = u64::from(u32_de(&fin, pos + 16));
    if nombre == 0xFFFF || taille_sommaire == 0xFFFF_FFFF || debut_sommaire == 0xFFFF_FFFF {
        // zip64 : son « localisateur » juste avant
        let pos_absolue = longueur - fin_lue + pos as u64;
        if pos_absolue < 20 {
            return Err(mauvais("zip64 abîmé"));
        }
        let mut loc = [0u8; 20];
        f.seek(SeekFrom::Start(pos_absolue - 20))?;
        f.read_exact(&mut loc)?;
        if loc[0..4] != [0x50, 0x4b, 0x06, 0x07] {
            return Err(mauvais("zip64 abîmé"));
        }
        let ou = u64_de(&loc, 8);
        let mut z = [0u8; 56];
        f.seek(SeekFrom::Start(ou))?;
        f.read_exact(&mut z)?;
        if z[0..4] != [0x50, 0x4b, 0x06, 0x06] {
            return Err(mauvais("zip64 abîmé"));
        }
        nombre = u64_de(&z, 32);
        taille_sommaire = u64_de(&z, 40);
        debut_sommaire = u64_de(&z, 48);
    }
    if debut_sommaire.checked_add(taille_sommaire).is_none_or(|b| b > longueur) || taille_sommaire > 64 * 1024 * 1024 {
        return Err(mauvais("sommaire du zip abîmé"));
    }
    let mut s = vec![0u8; taille_sommaire as usize];
    f.seek(SeekFrom::Start(debut_sommaire))?;
    f.read_exact(&mut s)?;
    let mut entrees = Vec::new();
    let mut i = 0usize;
    for _ in 0..nombre {
        if i + 46 > s.len() || s[i..i + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(mauvais("sommaire du zip abîmé"));
        }
        let (n, x, c) = (u16_de(&s, i + 28) as usize, u16_de(&s, i + 30) as usize, u16_de(&s, i + 32) as usize);
        if i + 46 + n + x + c > s.len() {
            return Err(mauvais("sommaire du zip abîmé"));
        }
        let mut e = Entree {
            chemin: String::from_utf8_lossy(&s[i + 46..i + 46 + n]).into_owned(),
            drapeaux: u16_de(&s, i + 8),
            methode: u16_de(&s, i + 10),
            crc: u32_de(&s, i + 16),
            taille_compressee: u64::from(u32_de(&s, i + 20)),
            taille: u64::from(u32_de(&s, i + 24)),
            decalage: u64::from(u32_de(&s, i + 42)),
        };
        // Les tailles et la position en 64 bits (champ supplémentaire 0x0001), dans cet ordre, pour celles qui valent
        // 0xFFFFFFFF
        let mut k = i + 46 + n;
        let fin_extra = k + x;
        while k + 4 <= fin_extra {
            let (id, l) = (u16_de(&s, k), u16_de(&s, k + 2) as usize);
            if k + 4 + l > fin_extra {
                break;
            }
            if id == 1 {
                let mut j = k + 4;
                for champ in [&mut e.taille, &mut e.taille_compressee, &mut e.decalage] {
                    if *champ == 0xFFFF_FFFF && j + 8 <= k + 4 + l {
                        *champ = u64_de(&s, j);
                        j += 8;
                    }
                }
            }
            k += 4 + l;
        }
        entrees.push(e);
        i += 46 + n + x + c;
    }
    Ok(entrees)
}

/// Où commencent les données d'un fichier (après son en-tête local)
fn debut_donnees(f: &mut File, e: &Entree) -> io::Result<u64> {
    let mut t = [0u8; 30];
    f.seek(SeekFrom::Start(e.decalage))?;
    f.read_exact(&mut t)?;
    if t[0..4] != [0x50, 0x4b, 0x03, 0x04] {
        return Err(mauvais("fichier du zip abîmé"));
    }
    Ok(e.decalage + 30 + u64::from(u16_de(&t, 26)) + u64::from(u16_de(&t, 28)))
}

/// Écrit un fichier du zip dans `cible`, en vérifiant sa taille et son CRC-32
fn ecrire_entree(f: &mut File, e: &Entree, cible: &Path, annule: &dyn Fn() -> bool) -> io::Result<()> {
    if e.drapeaux & 1 != 0 {
        return Err(mauvais("fichier du zip chiffré"));
    }
    let debut = debut_donnees(f, e)?;
    f.seek(SeekFrom::Start(debut))?;
    let brut = (&mut *f).take(e.taille_compressee);
    // (jamais plus que la taille annoncée, plus un octet pour voir qu'elle est fausse)
    let mut lecteur: Box<dyn Read + '_> = match e.methode {
        0 => Box::new(brut.take(e.taille + 1)),
        8 => Box::new(DeflateDecoder::new(brut).take(e.taille + 1)),
        m => return Err(mauvais(&format!("méthode de compression {m} inconnue"))),
    };
    let mut sortie = File::create(cible)?;
    let mut crc = Crc::new();
    let mut tampon = vec![0u8; 64 * 1024];
    let mut ecrit = 0u64;
    loop {
        if annule() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "annulé"));
        }
        // (deflate abîmé, fichier tronqué : le zip est abîmé)
        let n = lecteur.read(&mut tampon).map_err(|_| mauvais("fichier du zip abîmé"))?;
        if n == 0 {
            break;
        }
        crc.update(&tampon[..n]);
        sortie.write_all(&tampon[..n])?;
        ecrit += n as u64;
    }
    sortie.sync_all()?;
    if ecrit != e.taille || crc.sum() != e.crc {
        return Err(mauvais("fichier du zip abîmé"));
    }
    Ok(())
}

/// Extrait du zip les seuls fichiers voulus, dans `dossier`, le programme en dernier. Rend leurs noms.
pub fn extraire_voulus(
    zip: &Path,
    dossier: &Path,
    voulus: &Extraire,
    programme: &str,
    annule: &dyn Fn() -> bool,
) -> io::Result<Vec<String>> {
    let mut f = File::open(zip)?;
    let mut choisis: Vec<Entree> =
        lire_sommaire(&mut f)?.into_iter().filter(|e| e.nom().is_some_and(|n| voulus.voulu(n))).collect();
    // (tri stable, comme Sort-Object { $_.Name -eq $exe } : le programme à la fin, les autres dans l'ordre du zip)
    choisis.sort_by_key(|e| e.nom().is_some_and(|n| n.eq_ignore_ascii_case(programme)));
    let mut faits = Vec::new();
    for e in &choisis {
        if annule() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "annulé"));
        }
        let nom = e.nom().unwrap_or_default();
        let cible = dossier.join(nom);
        let a_cote = dossier.join(format!("{nom}.extraction"));
        let r = ecrire_entree(&mut f, e, &a_cote, annule).and_then(|()| fs::rename(&a_cote, &cible));
        if let Err(err) = r {
            let _ = fs::remove_file(&a_cote);
            return Err(err);
        }
        faits.push(nom.to_string());
    }
    Ok(faits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entree(chemin: &str) -> Entree {
        Entree { chemin: chemin.into(), methode: 0, drapeaux: 0, crc: 0, taille_compressee: 0, taille: 0, decalage: 0 }
    }

    #[test]
    fn seul_le_dernier_morceau_du_nom_compte() {
        assert_eq!(entree("llama/llama-server.exe").nom(), Some("llama-server.exe"));
        assert_eq!(entree("../../ggml.dll").nom(), Some("ggml.dll"));
        assert_eq!(entree("..\\..\\Windows\\ggml.dll").nom(), Some("ggml.dll"));
        assert_eq!(entree("C:\\ggml.dll").nom(), Some("ggml.dll"));
        assert_eq!(entree("/ggml.dll").nom(), Some("ggml.dll"));
        for rien in ["llama/", "..", "a/..", ".", "C:", "ggml.dll:flux", ""] {
            assert_eq!(entree(rien).nom(), None, "{rien}");
        }
    }
}
