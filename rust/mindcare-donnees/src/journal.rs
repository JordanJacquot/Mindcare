//! Le journal de Mindcare (journal.txt, dans le dossier de données) : une ligne datée par événement (`Write-Log`,
//! modules\donnees.ps1). Il sert à aider à distance : chaque échec utile pour comprendre un problème y est noté.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::moment::Moment;

/// L'heure locale : donnée par l'appelant (la lecture de Windows), une horloge figée dans les tests.
pub type Horloge = Arc<dyn Fn() -> Moment + Send + Sync>;

/// Une horloge arrêtée sur ce moment (les tests).
pub fn horloge_figee(m: Moment) -> Horloge {
    Arc::new(move || m)
}

/// Au-delà de cette taille, le journal est raccourci au démarrage.
pub const TAILLE_MAX: u64 = 1024 * 1024;
/// Les lignes gardées quand il est raccourci.
pub const LIGNES_GARDEES: usize = 3000;

/// Le journal : son fichier et l'horloge qui date ses lignes.
#[derive(Clone)]
pub struct Journal {
    fichier: PathBuf,
    horloge: Horloge,
}

impl Journal {
    /// Le journal de ce fichier (journal.txt), raccourci s'il dépasse 1 Mo (seulement ses 3000 dernières lignes),
    /// comme au chargement de donnees.ps1. Une erreur n'empêche rien.
    pub fn ouvrir(fichier: &Path, horloge: Horloge) -> Journal {
        raccourcir(fichier);
        Journal { fichier: fichier.to_path_buf(), horloge }
    }

    /// Le fichier du journal.
    pub fn fichier(&self) -> &Path {
        &self.fichier
    }

    /// L'heure qu'il est, selon l'horloge du journal.
    pub fn maintenant(&self) -> Moment {
        (self.horloge)()
    }

    /// `Write-Log` : une ligne « 2026-10-30T14:05:00 message », ajoutée à la fin (UTF-8 sans BOM). Une erreur est
    /// ignorée (le journal ne doit jamais rien bloquer).
    pub fn ecrire(&self, message: &str) {
        let ligne = format!("{} {message}\r\n", self.maintenant().iso());
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.fichier)
            .and_then(|mut f| f.write_all(ligne.as_bytes()));
    }
}

/// Les lignes d'un texte comme `[IO.File]::ReadAllLines` : coupées à « \r\n », « \r » ou « \n », sans ligne vide
/// après un dernier retour à la ligne.
pub fn lignes(texte: &str) -> Vec<&str> {
    let mut l = Vec::new();
    let mut debut = 0;
    let o = texte.as_bytes();
    let mut i = 0;
    while i < o.len() {
        match o[i] {
            b'\r' => {
                l.push(&texte[debut..i]);
                i += if o.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
                debut = i;
            }
            b'\n' => {
                l.push(&texte[debut..i]);
                i += 1;
                debut = i;
            }
            _ => i += 1,
        }
    }
    if debut < o.len() {
        l.push(&texte[debut..]);
    }
    l
}

/// Raccourcit le journal s'il dépasse 1 Mo : ses 3000 dernières lignes, réécrites en UTF-8 sans BOM.
pub fn raccourcir(fichier: &Path) {
    let Ok(m) = std::fs::metadata(fichier) else { return };
    if !m.is_file() || m.len() <= TAILLE_MAX {
        return;
    }
    let Ok(octets) = std::fs::read(fichier) else { return };
    let texte = crate::fichiers::decoder(&octets);
    let toutes = lignes(&texte);
    let gardees = &toutes[toutes.len().saturating_sub(LIGNES_GARDEES)..];
    let mut sortie = String::new();
    for l in gardees {
        sortie.push_str(l);
        sortie.push_str("\r\n");
    }
    let _ = std::fs::write(fichier, sortie.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lignes_comme_readalllines() {
        assert_eq!(lignes("a\r\nb\nc\rd"), vec!["a", "b", "c", "d"]);
        assert_eq!(lignes("a\r\n"), vec!["a"]);
        assert_eq!(lignes("a\r\n\r\n"), vec!["a", ""]);
        assert!(lignes("").is_empty());
    }
}
