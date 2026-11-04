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

/// Une lueur que la page accepte : « #RRGGBB » ou « #AARRGGBB » (`Get-CouleurGlow` de themes.ps1). suite-commun
/// l'a déjà vérifiée à la lecture ; comme pour les couleurs (`pont::couleurs_json`), rien d'autre n'entre dans
/// l'état de la page, quoi que dise le pack.
pub fn lueur_valable(c: &str) -> bool {
    (c.len() == 7 || c.len() == 9) && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())
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
        let lueur = p.logo_lueur.as_deref().filter(|c| lueur_valable(c));
        logo.insert("lueur".into(), lueur.map_or(Value::Null, |c| json!(c)));
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

    /// Les 13 packs inventés de outils\tester.ps1 (« thème perso : couleurs, police et lueur contrôlées », Mindcare
    /// 0.15.2, `Test-CouleursPack` et `Get-CouleurGlow` de themes.ps1), lus comme le programme les lit
    /// (`themes::charger`), puis posés dans l'état de la page (`Habillage`, comme `Send-Vue`) : les bonnes couleurs
    /// passent ; une couleur mal écrite laisse le thème de base, avec une ligne au journal ; une police ou une lueur
    /// piégées sont écartées ; et rien du piège n'arrive jamais à la page.
    ///
    /// Un seul changement : T vaut ici « #A0C4FF » au lieu de « #0A2A66 ». suite-commun vérifie EN PLUS du PowerShell
    /// que le texte reste lisible (`verifier_lisibilite`), et le bleu nuit de tester.ps1 se lit mal sur le fond
    /// (contraste 1,4 pour 2 demandé) : avec lui, toutes les couleurs seraient refusées pour cette raison, et le test
    /// ne prouverait plus rien sur l'écriture des couleurs. Cet écart voulu est vérifié à part, à la fin.
    #[test]
    fn packs_inventes_de_tester_ps1() {
        let piege = r#""/><Button xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Content="piege"/><Border Tag=""#;
        let bonnes = json!({ "P": "#1E5BFF", "S": "#7FB2FF", "T": "#A0C4FF", "NH": 220, "NS": 0.6,
            "Map": { "0C0920": "0A1230", "#06050F": "#050A1A" } });
        let avec = |changes: Value| {
            let mut c = bonnes.clone();
            for (k, v) in changes.as_object().expect("objet") {
                c[k] = v.clone();
            }
            c
        };
        // (id, pack.json, thème attendu : « pack » pour ses propres couleurs, police et lueur à écarter)
        let cas = [
            ("essai-bleu", json!({ "Name": "Essai bleu", "Colors": bonnes, "LogoGlow": "#FFD6E4FF" }), "pack", false),
            (
                "essai-rouge-mot",
                json!({ "Base": "crepuscule", "Colors": avec(json!({ "P": "red" })) }),
                "crepuscule",
                false,
            ),
            ("essai-court", json!({ "Colors": avec(json!({ "S": "#0F0" })) }), "neon", false),
            (
                "essai-guillemets",
                json!({ "Base": "terminal", "Colors": avec(json!({ "T": "#FF0000\" Foreground=\"Red" })) }),
                "terminal",
                false,
            ),
            ("essai-balise", json!({ "Colors": avec(json!({ "P": format!("#00FF00{piege}") })) }), "neon", false),
            ("essai-retour-ligne", json!({ "Colors": avec(json!({ "P": "#00FF00\n" })) }), "neon", false),
            (
                "essai-map-cle",
                json!({ "Base": "arcade", "Colors": avec(json!({ "Map": { format!("0C0920{piege}"): "112233" } })) }),
                "arcade",
                false,
            ),
            (
                "essai-map-valeur",
                json!({ "Colors": avec(json!({ "Map": { "0C0920": format!("112233{piege}") } })) }),
                "neon",
                false,
            ),
            ("essai-map-texte", json!({ "Colors": avec(json!({ "Map": "rouge" })) }), "neon", false),
            ("essai-nombre", json!({ "Colors": avec(json!({ "NS": -2 })) }), "neon", false),
            ("essai-teinte", json!({ "Colors": avec(json!({ "NH": "NaN" })) }), "neon", false),
            (
                "essai-police",
                json!({ "Colors": bonnes, "Font": format!("police{piege}.ttf"), "FontScope": "tout",
                    "LogoGlow": format!("#FFFFFF{piege}") }),
                "pack",
                true,
            ),
            ("essai-police-dossier", json!({ "Colors": bonnes, "Font": "..\\police.ttf" }), "pack", true),
        ];
        let e = Essai::new("couleurs");
        // (une vraie police hors du dossier du pack : jamais reprise)
        std::fs::write(e.0.join("packs").join("police.ttf"), b"pas une police").unwrap();
        let mut rates = Vec::new();
        for (id, json_pack, attendu, police) in &cas {
            let dossier = e.0.join("packs").join(id);
            std::fs::create_dir_all(&dossier).unwrap();
            // (un logo, pour que la lueur passe vraiment par ce que reçoit la page)
            let mut json_pack = json_pack.clone();
            json_pack["Logo"] = json!("logo.png");
            std::fs::write(dossier.join("logo.png"), PNG).unwrap();
            std::fs::write(dossier.join("pack.json"), serde_json::to_string_pretty(&json_pack).unwrap()).unwrap();
            let t = suite_commun::themes::charger(Some(&format!("pack:{id}")), &[e.0.join("packs")]);
            let Some(p) = t.pack.as_ref() else {
                rates.push(format!("{id} : pas lu ({:?})", t.raisons));
                continue;
            };
            let propres = *attendu == "pack";
            let attendu_id = if propres { format!("pack-{id}") } else { (*attendu).to_string() };
            if t.id != attendu_id {
                rates.push(format!("{id} : thème {} au lieu de {attendu_id} ({:?})", t.id, t.raisons));
            }
            // (refusées pour leur écriture, jamais pour la lisibilité : c'est bien ce contrôle-là qui joue)
            let note = t
                .raisons
                .iter()
                .filter(|r| r.starts_with(&format!("thème {id} : couleurs refusées")) && !r.contains("se lit mal"))
                .count();
            if propres == (note > 0) || t.raisons.len() != note {
                rates.push(format!("{id} : journal {:?}", t.raisons));
            }
            if *police && (p.police.is_some() || p.logo_lueur.is_some()) {
                rates.push(format!("{id} : police {:?}, lueur {:?} gardées", p.police, p.logo_lueur));
            }
            // L'état de la page : le thème du kit, et les couleurs du pack seulement si elles sont à lui
            let h = crate::pont::Habillage::depuis(&t, false);
            let mut etat = json!({ "app": { "nom": "Mindcare" } });
            h.poser(&mut etat);
            let lueur = &etat["pack"]["logo"]["lueur"];
            if !(lueur.is_null() || lueur.as_str().is_some_and(lueur_valable)) || (*police && !lueur.is_null()) {
                rates.push(format!("{id} : lueur dans la page {lueur}"));
            }
            let theme_kit = if propres { "neon" } else { *attendu };
            if etat["theme"] != theme_kit || h.couleurs.is_some() != propres {
                rates.push(format!("{id} : état de la page {etat}"));
            }
            if let Some(c) = &h.couleurs {
                let ok = c.len() == 5 && c.values().all(|v| v.as_str().is_some_and(crate::pont::couleur_valable));
                if !ok {
                    rates.push(format!("{id} : couleurs de la page {c:?}"));
                }
            }
            let texte = etat.to_string();
            if texte.contains("piege") || texte.contains("Foreground") || texte.contains("red") {
                rates.push(format!("{id} : le piège est dans l'état de la page : {texte}"));
            }
            if *id == "essai-bleu" {
                let (a, b, c) = (t.traduire("#00E5FF"), t.traduire("#0C0920"), t.traduire("#06050F"));
                if (a.as_str(), b.as_str(), c.as_str()) != ("#1E5BFF", "#0A1230", "#050A1A")
                    || p.logo_lueur.as_deref() != Some("#FFD6E4FF")
                    || etat["couleurs"]["P"] != "#1E5BFF"
                    || etat["pack"]["logo"]["lueur"] != "#FFD6E4FF"
                {
                    rates.push(format!("essai-bleu : couleurs du pack non reprises ({a} {b} {c}, {:?})", p.logo_lueur));
                }
            }
        }
        // Le pack « essai-bleu » de tester.ps1 tel quel (T « #0A2A66 ») : PowerShell garde ses couleurs, suite-commun
        // les refuse parce que le texte se lirait mal (écart voulu), et l'écran reste sur le thème de base.
        let dossier = e.0.join("packs").join("essai-bleu-nuit");
        std::fs::create_dir_all(&dossier).unwrap();
        let nuit = avec(json!({ "T": "#0A2A66" }));
        std::fs::write(dossier.join("pack.json"), json!({ "Colors": nuit, "LogoGlow": "#FFD6E4FF" }).to_string())
            .unwrap();
        let t = suite_commun::themes::charger(Some("pack:essai-bleu-nuit"), &[e.0.join("packs")]);
        if t.id != "neon" || !t.raisons.iter().any(|r| r.contains("couleurs refusées") && r.contains("se lit mal")) {
            rates.push(format!("essai-bleu-nuit : thème {} ({:?})", t.id, t.raisons));
        }
        assert_eq!(t.pack.as_ref().and_then(|p| p.logo_lueur.as_deref()), Some("#FFD6E4FF"));
        assert!(rates.is_empty(), "{}", rates.join("\n"));
    }
}
