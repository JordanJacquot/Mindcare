//! Le protocole `pack` (http://pack.localhost/<fichier>) : les images et polices du thème choisi, et rien d'autre
//! (docs\regles-tauri.md du dépôt Mindset, règle 6). Toute la lecture est celle de suite-commun
//! (`fichier_pack::nom_depuis_adresse` puis `fichier_pack::lire`) ; ici, seulement la réponse : le fichier avec le
//! type reconnu à ses premiers octets, ou « introuvable » (404, sans rien dire de plus) pour tout le reste. Le
//! programme (étape 4.4) l'enregistre avec `register_uri_scheme_protocol("pack", ...)`.

use std::path::Path;

use suite_commun::fichier_pack::{self, Refus};
use tauri::http::{Response, StatusCode, header};

/// La réponse à une demande du protocole `pack`. `dossier_pack` : le dossier du pack du thème choisi (aucun : le
/// thème n'a pas de pack, tout est introuvable) ; `chemin` : le chemin de l'adresse demandée (`/fond.png`). Rend
/// aussi la raison d'un refus, pour le journal (la page n'en sait rien).
pub fn repondre(dossier_pack: Option<&Path>, chemin: &str) -> (Response<Vec<u8>>, Option<Refus>) {
    let lu = match (dossier_pack, fichier_pack::nom_depuis_adresse(chemin)) {
        (Some(dossier), Some(nom)) => fichier_pack::lire(dossier, &nom),
        (None, _) => Err(Refus::Dossier),
        (_, None) => Err(Refus::Nom),
    };
    let base =
        Response::builder().header(header::CACHE_CONTROL, "no-store").header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    let (reponse, refus) = match lu {
        Ok(f) => (base.status(StatusCode::OK).header(header::CONTENT_TYPE, f.type_contenu).body(f.octets), None),
        Err(r) => (base.status(StatusCode::NOT_FOUND).body(Vec::new()), Some(r)),
    };
    // (un en-tête mal formé n'arrive pas : tous sont écrits ici ou viennent de suite-commun)
    (reponse.unwrap_or_else(|_| Response::new(Vec::new())), refus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Un dossier d'essai neuf sous le dossier temporaire (`<temp>\mindcare-pack-<pid>-<nom>\packs\essai`), effacé
    /// à la fin.
    struct Essai(PathBuf);
    impl Essai {
        fn new(nom: &str) -> Essai {
            let racine = std::env::temp_dir().join(format!("mindcare-pack-{}-{nom}", std::process::id()));
            let _ = std::fs::remove_dir_all(&racine);
            std::fs::create_dir_all(racine.join("packs").join("essai")).expect("dossier d'essai");
            Essai(racine)
        }
        fn pack(&self) -> PathBuf {
            self.0.join("packs").join("essai")
        }
    }
    impl Drop for Essai {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Les premiers octets d'un PNG, et de quoi faire 12 octets au moins
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn une_image_du_pack_est_servie_avec_son_type() {
        let e = Essai::new("image");
        std::fs::write(e.pack().join("fond.png"), PNG).unwrap();
        let (r, refus) = repondre(Some(&e.pack()), "/fond.png");
        assert_eq!(refus, None);
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(r.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(r.body().as_slice(), PNG);
    }

    #[test]
    fn tout_le_reste_est_introuvable_sans_contenu() {
        let e = Essai::new("refus");
        std::fs::write(e.pack().join("fond.png"), PNG).unwrap();
        std::fs::write(e.0.join("packs").join("voisin.png"), PNG).unwrap();
        std::fs::write(e.pack().join("faux.png"), b"pas une image du tout").unwrap();
        for (chemin, attendu) in [
            ("/..%5Cvoisin.png", Refus::Nom),
            ("/../voisin.png", Refus::Nom),
            ("/C:%5CWindows%5Cwin.ini", Refus::Nom),
            ("/fond.png?x=1", Refus::Nom),
            ("/faux.png", Refus::Type),
            ("/absent.png", Refus::Introuvable),
            ("/notes.txt", Refus::Extension),
        ] {
            let (r, refus) = repondre(Some(&e.pack()), chemin);
            assert_eq!(r.status(), StatusCode::NOT_FOUND, "{chemin}");
            assert!(r.body().is_empty(), "{chemin}");
            assert_eq!(refus, Some(attendu), "{chemin}");
        }
        // Sans pack (thème intégré) : rien n'est servi
        let (r, refus) = repondre(None, "/fond.png");
        assert_eq!(r.status(), StatusCode::NOT_FOUND);
        assert_eq!(refus, Some(Refus::Dossier));
    }
}
