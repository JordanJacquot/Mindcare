//! Le protocole `pack` (http://pack.localhost/<fichier>) : les images et polices du thème choisi, et rien d'autre
//! (docs\regles-tauri.md du dépôt Mindset, règle 6). Toute la lecture est celle de suite-commun
//! (`fichier_pack::nom_depuis_adresse` puis `fichier_pack::lire`) ; ici, seulement la réponse : le fichier avec le
//! type reconnu à ses premiers octets, ou « introuvable » (404, sans rien dire de plus) pour tout le reste. Le
//! programme (étape 4.4) l'enregistre avec `register_uri_scheme_protocol("pack", ...)`.
//!
//! Ici aussi, ce que l'écran reçoit d'un thème personnalisé (`Get-PackVue` du kit, vue.ps1) : les adresses de ses
//! images et de sa police sous `http://pack.localhost/`, et rien d'autre (jamais un chemin du disque).

use std::path::Path;

use serde_json::{Map, Value, json};
use suite_commun::fichier_pack::{self, Refus};
use suite_commun::themes::{Alignement, Pack, Planche};
use tauri::http::{Response, StatusCode, header};

/// L'adresse des fichiers du pack, vue de la page (le protocole `pack` sous Windows)
pub const ADRESSE_PACK: &str = "http://pack.localhost/";
/// L'origine des pages de l'appli : la seule à qui les fichiers du pack sont permis d'une autre origine (la 3D en
/// fait des textures, la police est chargée par @font-face : tous deux le demandent)
const ORIGINE_PAGES: &str = "http://tauri.localhost";

/// L'adresse d'un fichier du pack pour la page : `http://pack.localhost/<nom codé>`. Tout ce qui n'est pas une
/// lettre ou un chiffre ASCII, `-`, `_` ou `.` est codé (`%XX` en UTF-8), comme `[uri]::EscapeDataString` : le kit
/// n'accepte rien d'autre dans une adresse (`urlSure`), et le protocole décode une fois (`nom_depuis_adresse`).
pub fn adresse(fichier: &str) -> String {
    let mut a = String::from(ADRESSE_PACK);
    for o in fichier.bytes() {
        if o.is_ascii_alphanumeric() || matches!(o, b'-' | b'_' | b'.') {
            a.push(o as char);
        } else {
            a.push_str(&format!("%{o:02X}"));
        }
    }
    a
}

fn planche(p: &Planche) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("url".into(), json!(adresse(&p.image.fichier)));
    m.insert("images".into(), json!(p.images));
    m.insert("delai".into(), json!(p.delai));
    m
}

/// Ce que le kit reçoit d'un thème personnalisé (`pack` de son état), comme `Get-PackVue` (vue.ps1) : logo, fond,
/// mascotte, dessin du bas, police et phrase d'accueil (`soir` : « Bonsoir » au lieu de « Bonjour »). Le bloc
/// « Console » n'est pas passé (vue.ps1 ne le passe pas non plus).
pub fn pack_ecran(p: &Pack, soir: bool) -> Value {
    let mut m = Map::new();
    if let Some(l) = &p.logo {
        let mut logo = planche(l);
        logo.insert("lueur".into(), p.logo_lueur.as_ref().map_or(Value::Null, |c| json!(c)));
        m.insert("logo".into(), Value::Object(logo));
    }
    if let Some(f) = &p.fond {
        let cote = |a: Alignement, debut: &str, fin: &str| match a {
            Alignement::Debut => json!(debut),
            Alignement::Fin => json!(fin),
            Alignement::Centre => Value::Null,
        };
        m.insert(
            "fond".into(),
            json!({
                "url": adresse(&f.image.fichier),
                "opacite": f.opacite,
                "x": cote(f.x, "gauche", "droite"),
                "y": cote(f.y, "haut", "bas"),
            }),
        );
    }
    if let Some(ma) = &p.mascotte {
        let mut mascotte = planche(&ma.planche);
        mascotte.insert("miroir".into(), json!(ma.miroir));
        m.insert("mascotte".into(), Value::Object(mascotte));
    }
    if let Some(b) = &p.bas {
        let mut bas = planche(&b.planche);
        bas.insert("hauteur".into(), json!(b.hauteur));
        m.insert("bas".into(), Value::Object(bas));
    }
    if let Some(po) = &p.police {
        m.insert(
            "police".into(),
            json!({ "url": adresse(&po.fichier.fichier), "partout": po.partout, "pixel": p.pixel }),
        );
    }
    if let Some(phrase) = p.phrase_accueil(soir) {
        m.insert("phrase".into(), json!(phrase));
    }
    Value::Object(m)
}

/// La réponse à une demande du protocole `pack`. `dossier_pack` : le dossier du pack du thème choisi (aucun : le
/// thème n'a pas de pack, tout est introuvable) ; `chemin` : le chemin de l'adresse demandée (`/fond.png`). Rend
/// aussi la raison d'un refus, pour le journal (la page n'en sait rien).
pub fn repondre(dossier_pack: Option<&Path>, chemin: &str) -> (Response<Vec<u8>>, Option<Refus>) {
    let lu = match (dossier_pack, fichier_pack::nom_depuis_adresse(chemin)) {
        (Some(dossier), Some(nom)) => fichier_pack::lire(dossier, &nom),
        (None, _) => Err(Refus::Dossier),
        (_, None) => Err(Refus::Nom),
    };
    let base = Response::builder()
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, ORIGINE_PAGES);
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

    /// Le pack.json d'un pack d'essai (les noms de Mindset, packs.ps1)
    const PACK_JSON: &str = r##"{ "Name": "Essai", "Base": "neon", "Hello": "{0} toi",
        "Logo": "logo.png", "LogoFrames": 4, "LogoDelay": 120, "LogoGlow": "#FF2EB5",
        "Background": "fond.png", "BackgroundOpacity": 0.5, "BackgroundAlign": "haut droite" }"##;

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
        // (seulement pour les pages de l'appli, jamais « * »)
        assert_eq!(r.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "http://tauri.localhost");
        assert_eq!(r.body().as_slice(), PNG);
    }

    #[test]
    fn adresses_du_pack_codees_et_relues() {
        assert_eq!(adresse("fond.png"), "http://pack.localhost/fond.png");
        assert_eq!(adresse("logo bleu (2).png"), "http://pack.localhost/logo%20bleu%20%282%29.png");
        assert_eq!(adresse("étoile'.png"), "http://pack.localhost/%C3%A9toile%27.png");
        // Le protocole retrouve le même nom
        for nom in ["fond.png", "logo bleu (2).png", "étoile'.png", "a~b.png"] {
            let a = adresse(nom);
            let chemin = &a["http://pack.localhost".len()..];
            assert_eq!(fichier_pack::nom_depuis_adresse(chemin).as_deref(), Some(nom), "{a}");
            // Le kit (urlSure) n'accepte que lettres, chiffres, _ % - . / après l'adresse du pack
            assert!(chemin[1..].bytes().all(|o| o.is_ascii_alphanumeric() || b"_%-.".contains(&o)), "{a}");
        }
    }

    /// Un pack d'essai écrit sur le disque, lu par suite-commun, puis donné au kit : seulement des adresses du pack
    #[test]
    fn pack_pour_le_kit_comme_vue_ps1() {
        let e = Essai::new("kit");
        std::fs::write(e.pack().join("fond.png"), PNG).unwrap();
        std::fs::write(e.pack().join("logo.png"), PNG).unwrap();
        std::fs::write(e.pack().join("pack.json"), PACK_JSON).unwrap();
        let t = suite_commun::themes::charger(Some("pack:essai"), &[e.0.join("packs")]);
        let p = t.pack.as_ref().unwrap_or_else(|| panic!("pack non lu : {:?}", t.raisons));
        let v = pack_ecran(p, true);
        assert_eq!(v["phrase"], "Bonsoir toi");
        assert_eq!(v["logo"]["url"], "http://pack.localhost/logo.png");
        assert_eq!(v["logo"]["images"], 4);
        assert_eq!(v["logo"]["delai"], 120);
        assert_eq!(v["logo"]["lueur"], "#FF2EB5");
        assert_eq!(v["fond"]["url"], "http://pack.localhost/fond.png");
        assert_eq!(v["fond"]["opacite"], 0.5);
        assert_eq!(v["fond"]["x"], "droite");
        assert_eq!(v["fond"]["y"], "haut");
        assert!(v.get("mascotte").is_none() && v.get("police").is_none() && v.get("bas").is_none());
        // Aucun chemin du disque dans ce que voit la page
        let texte = v.to_string();
        assert!(!texte.contains(&*e.0.to_string_lossy()) && !texte.contains('\\'), "{texte}");
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
