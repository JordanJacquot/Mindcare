//! `maj.vider` : le dossier des mises à jour, regardé sans jamais suivre un lien, renommé, et le `.old` d'une fois
//! d'avant supprimé. Un lien (jonction, lien symbolique) n'est jamais suivi : il est retiré lui-même, jamais ce vers
//! quoi il pointe (`std::fs::remove_dir_all` de Rust ne suit pas les liens sous Windows).

use crate::systeme::Entree;
use std::os::windows::fs::MetadataExt;

/// FILE_ATTRIBUTE_REPARSE_POINT
const LIEN: u32 = 0x400;

pub fn entree(chemin: &str) -> Entree {
    match std::fs::symlink_metadata(chemin) {
        Err(_) => Entree::Absente,
        Ok(m) if m.file_attributes() & LIEN != 0 => Entree::Lien,
        Ok(m) if m.is_dir() => Entree::Dossier,
        Ok(_) => Entree::Fichier,
    }
}

pub fn supprimer(chemin: &str) -> Result<(), String> {
    let r = match entree(chemin) {
        Entree::Absente => return Ok(()),
        // (un lien vers un dossier se retire comme un dossier vide, sans toucher à ce vers quoi il pointe)
        Entree::Lien => std::fs::remove_dir(chemin).or_else(|_| std::fs::remove_file(chemin)),
        Entree::Fichier => std::fs::remove_file(chemin),
        Entree::Dossier => std::fs::remove_dir_all(chemin),
    };
    r.map_err(|e| e.to_string())
}

pub fn renommer(de: &str, vers: &str) -> Result<(), String> {
    std::fs::rename(de, vers).map_err(|e| e.to_string())
}

#[cfg(test)]
mod essais {
    use super::*;

    /// Dans un dossier temporaire de l'essai seulement (jamais le vrai SoftwareDistribution)
    #[test]
    fn un_dossier_de_l_essai() {
        let base = std::env::temp_dir().join(format!("mindcare-aide-essai-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let d = base.join("SoftwareDistribution");
        std::fs::create_dir_all(d.join("Download")).unwrap();
        std::fs::write(d.join("Download").join("x.cab"), b"x").unwrap();
        let (d, old) = (d.to_string_lossy().into_owned(), format!("{}.old", d.to_string_lossy()));
        assert_eq!(entree(&d), Entree::Dossier);
        assert_eq!(entree(&old), Entree::Absente);
        renommer(&d, &old).unwrap();
        assert_eq!((entree(&d), entree(&old)), (Entree::Absente, Entree::Dossier));
        supprimer(&old).unwrap();
        assert_eq!(entree(&old), Entree::Absente);
        assert!(supprimer(&old).is_ok());
        std::fs::write(&d, b"un fichier").unwrap();
        assert_eq!(entree(&d), Entree::Fichier);
        let _ = std::fs::remove_dir_all(&base);
    }
}
