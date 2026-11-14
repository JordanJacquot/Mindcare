//! Construction de MindcareAide.exe : ses ressources Windows écrites ici sans aucun outil ni bibliothèque de plus,
//! reprises de celles du relais (rust\mindcare-relais\build.rs), sans icône :
//!
//! - le manifeste (aide.manifest : droits « asInvoker », jamais de demande de Windows) ;
//! - la fiche de version : description et produit « Mindcare », version celle de Mindcare (le Cargo.toml de
//!   mindcare-appli : une seule source, la même que version.json, Mindset.exe et Mindcare.exe). Le contrôle du zip
//!   (outils\verifier-zip-rust.ps1) la vérifie.
//!
//! Le tout va dans un fichier .res, que l'éditeur de liens de Microsoft (link.exe) accepte tel quel. Le runtime C de
//! Microsoft est rangé dans l'exe, comme pour Mindset.exe et Mindcare.exe : aucune VCRUNTIME140.dll à installer.

use std::path::{Path, PathBuf};

fn main() {
    let ici = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let manifeste = ici.join("aide.manifest");
    let cargo_appli = ici.join("..").join("mindcare-appli").join("Cargo.toml");
    for f in [&manifeste, &cargo_appli] {
        println!("cargo:rerun-if-changed={}", f.display());
    }
    println!("cargo:rerun-if-changed=build.rs");
    // Seulement pour Windows avec l'éditeur de liens de Microsoft (la suite n'est construite que comme ça)
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let version = version_mindcare(&cargo_appli);
    let mut res = Res::default();
    res.vide();
    // Le manifeste : ressource 24, numéro 1 (celui que Windows lit au lancement)
    res.ajouter(24, 1, 0x0409, &std::fs::read(&manifeste).expect("manifeste illisible"));
    res.ajouter(16, 1, 0x0000, &version_info(&version));
    let sortie = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("aide.res");
    std::fs::write(&sortie, &res.octets).expect("écriture du .res impossible");
    println!("cargo:rustc-link-arg-bins={}", sortie.display());
    // Le runtime C dans l'exe (les mêmes réglages que le relais et que static_vcruntime de tauri-build)
    for l in [
        "/NODEFAULTLIB:libvcruntimed.lib",
        "/NODEFAULTLIB:vcruntime.lib",
        "/NODEFAULTLIB:vcruntimed.lib",
        "/NODEFAULTLIB:libcmtd.lib",
        "/NODEFAULTLIB:msvcrt.lib",
        "/NODEFAULTLIB:msvcrtd.lib",
        "/NODEFAULTLIB:libucrt.lib",
        "/NODEFAULTLIB:libucrtd.lib",
        "/DEFAULTLIB:libcmt.lib",
        "/DEFAULTLIB:libvcruntime.lib",
        "/DEFAULTLIB:ucrt.lib",
    ] {
        println!("cargo:rustc-link-arg-bins={l}");
    }
}

/// La version de Mindcare (ligne `version = "x.y.z"` de la section [package] du Cargo.toml de mindcare-appli), en
/// quatre nombres. Arrêt de la construction si elle est introuvable ou mal écrite.
fn version_mindcare(cargo: &Path) -> [u16; 4] {
    let texte = std::fs::read_to_string(cargo).expect("Cargo.toml de mindcare-appli illisible");
    let mut section = "";
    let mut trouvee = None;
    for ligne in texte.lines() {
        let l = ligne.trim();
        if l.starts_with('[') {
            section = l;
            continue;
        }
        if section == "[package]"
            && let Some(reste) = l.strip_prefix("version = \"")
        {
            trouvee = reste.split('"').next();
            break;
        }
    }
    let texte = trouvee.expect("version introuvable dans le Cargo.toml de mindcare-appli");
    let n: Vec<u16> = texte.split('.').map(|x| x.parse().expect("version non valable")).collect();
    assert!((2..=4).contains(&n.len()), "version non valable : {texte}");
    let mut v = [0u16; 4];
    v[..n.len()].copy_from_slice(&n);
    v
}

/// Un fichier .res : une suite de ressources, chacune avec son en-tête, alignées sur 4 octets
#[derive(Default)]
struct Res {
    octets: Vec<u8>,
}

impl Res {
    /// La ressource vide qui ouvre tout fichier .res
    fn vide(&mut self) {
        self.octets.extend_from_slice(&0u32.to_le_bytes());
        self.octets.extend_from_slice(&32u32.to_le_bytes());
        self.octets.extend_from_slice(&[0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0]);
        self.octets.extend_from_slice(&[0u8; 16]);
    }

    /// Une ressource de type et de numéro donnés (langue : 0 neutre, 0x0409...)
    fn ajouter(&mut self, type_: u16, numero: u16, langue: u16, donnees: &[u8]) {
        let o = &mut self.octets;
        o.extend_from_slice(&(donnees.len() as u32).to_le_bytes());
        o.extend_from_slice(&32u32.to_le_bytes()); // taille de l'en-tête (type et nom en numéros)
        o.extend_from_slice(&[0xFF, 0xFF]);
        o.extend_from_slice(&type_.to_le_bytes());
        o.extend_from_slice(&[0xFF, 0xFF]);
        o.extend_from_slice(&numero.to_le_bytes());
        o.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
        o.extend_from_slice(&0x1030u16.to_le_bytes()); // MOVEABLE | PURE | DISCARDABLE
        o.extend_from_slice(&langue.to_le_bytes());
        o.extend_from_slice(&0u32.to_le_bytes()); // Version
        o.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
        o.extend_from_slice(donnees);
        while !o.len().is_multiple_of(4) {
            o.push(0);
        }
    }
}

/// Un bloc de VERSIONINFO : longueur, longueur de la valeur, type (1 texte, 0 octets), clé, valeur, puis les enfants
fn bloc(cle: &str, valeur: &[u8], longueur_valeur: u16, texte: bool, enfants: &[Vec<u8>]) -> Vec<u8> {
    let mut b = vec![0u8; 6];
    b[2..4].copy_from_slice(&longueur_valeur.to_le_bytes());
    b[4..6].copy_from_slice(&u16::from(texte).to_le_bytes());
    for c in cle.encode_utf16().chain([0]) {
        b.extend_from_slice(&c.to_le_bytes());
    }
    while !b.len().is_multiple_of(4) {
        b.push(0);
    }
    b.extend_from_slice(valeur);
    for e in enfants {
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        b.extend_from_slice(e);
    }
    let n = b.len() as u16;
    b[0..2].copy_from_slice(&n.to_le_bytes());
    b
}

/// Un texte des informations de version (« FileDescription » = « Mindcare »)
fn chaine(cle: &str, valeur: &str) -> Vec<u8> {
    let mut v = Vec::new();
    let mut n = 0u16;
    for c in valeur.encode_utf16().chain([0]) {
        v.extend_from_slice(&c.to_le_bytes());
        n += 1;
    }
    bloc(cle, &v, n, true, &[])
}

fn version_info(v: &[u16; 4]) -> Vec<u8> {
    let texte = format!("{}.{}.{}.{}", v[0], v[1], v[2], v[3]);
    let haut = (u32::from(v[0]) << 16) | u32::from(v[1]);
    let bas = (u32::from(v[2]) << 16) | u32::from(v[3]);
    let mut fixe = Vec::new();
    for x in [0xFEEF_04BDu32, 0x0001_0000, haut, bas, haut, bas, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0] {
        fixe.extend_from_slice(&x.to_le_bytes());
    }
    let table = bloc(
        "000004b0",
        &[],
        0,
        true,
        &[
            chaine(
                "Comments",
                "La partie administrateur de Mindcare (tâche de Mindset), quatre réparations de Windows.",
            ),
            chaine("FileDescription", "Mindcare"),
            chaine("FileVersion", &texte),
            chaine("InternalName", "MindcareAide.exe"),
            chaine("OriginalFilename", "MindcareAide.exe"),
            chaine("ProductName", "Mindcare"),
            chaine("ProductVersion", &texte),
        ],
    );
    let textes = bloc("StringFileInfo", &[], 0, true, &[table]);
    let traduction = bloc("Translation", &[0x00, 0x00, 0xB0, 0x04], 4, false, &[]);
    let langues = bloc("VarFileInfo", &[], 0, true, &[traduction]);
    bloc("VS_VERSION_INFO", &fixe, fixe.len() as u16, false, &[textes, langues])
}
