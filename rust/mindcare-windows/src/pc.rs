//! Le PC reconnu : portable ou fixe, sa marque, sa carte mère, son processeur, sa mémoire, son disque et sa version
//! de Windows. Les règles, sans aucun appel à Windows : elles reçoivent les valeurs brutes de WMI ([`PcBrut`], lues
//! par le module `windows`) et en font un [`PcDetecte`], son profil pour trier les pannes
//! ([`mindcare_moteur::ProfilPc`]) et sa fiche pour le rapport ([`mindcare_moteur::pc::FichePc`]).
//!
//! La règle d'or : dans le doute, on ne sait pas (rien). Un profil inconnu montre toutes les pannes ; un profil faux
//! en cacherait à tort. Ainsi deux mesures qui se contredisent sur le type (le boîtier dit « tour », Windows dit
//! « portable ») donnent un type inconnu, et une marque de remplissage (« To Be Filled By O.E.M. ») une marque
//! inconnue.
//!
//! Vie privée : rien qui désigne la personne ou la machine. Ni nom d'utilisateur, ni nom du PC, ni numéro de série
//! (les requêtes WMI ne les demandent même pas : voir `windows::REQUETES_PC`).

use mindcare_moteur::pc::FichePc;
use mindcare_moteur::{ProfilPc, TypePc};

use crate::source::{Place, SourceWindows};

/// Ce que WMI dit du PC, tel quel (un texte vide ou rien : pas lu, ou illisible).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PcBrut {
    /// Win32_SystemEnclosure.ChassisTypes (3 : bureau, 7 : tour, 9 : portable, 10 : notebook...).
    pub boitiers: Vec<u16>,
    /// Win32_ComputerSystem.PCSystemType (1 : bureau, 2 : portable, 3 : station de travail, 8 : tablette...).
    pub genre_systeme: Option<u16>,
    /// Une batterie dans Win32_Battery (rien : pas lu).
    pub batterie: Option<bool>,
    /// Win32_ComputerSystem.Manufacturer et Model.
    pub fabricant: String,
    pub modele: String,
    /// Win32_ComputerSystemProduct.Version (Lenovo y écrit le nom parlant : « ThinkPad T14 Gen 2i »).
    pub version_produit: String,
    /// Win32_BaseBoard.Manufacturer et Product.
    pub carte_fabricant: String,
    pub carte_modele: String,
    /// Win32_Processor.Name (le premier).
    pub processeur: String,
    /// Win32_PhysicalMemory.Capacity de chaque barrette (rien : taille inconnue).
    pub barrettes: Vec<Option<u64>>,
    /// Le disque de Windows (MSFT_PhysicalDisk).
    pub disque: Option<DisqueBrut>,
    /// La place du disque de Windows.
    pub place: Option<Place>,
    /// Win32_OperatingSystem.Caption et BuildNumber.
    pub systeme: String,
    pub build: String,
}

/// Le disque où est Windows (MSFT_PhysicalDisk : MediaType, BusType, Size).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisqueBrut {
    /// 3 : disque dur, 4 : SSD, 0 : non précisé.
    pub media: Option<u16>,
    /// 17 : NVMe, 11 : SATA, 7 : USB...
    pub bus: Option<u16>,
    pub taille: Option<u64>,
}

/// Le PC reconnu. Tout ce qui n'a pas pu être reconnu est rien.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PcDetecte {
    pub genre: Option<TypePc>,
    /// La marque qui compte pour les pannes : celle de la carte mère pour un PC monté, celle du PC sinon.
    pub marque: Option<String>,
    /// Un PC monté (une carte mère ASUS, MSI... dans une tour sans marque).
    pub monte: bool,
    /// La marque et le modèle du PC (« HP », « Pavilion 15-eh1xxx »).
    pub marque_pc: Option<String>,
    pub modele_pc: Option<String>,
    /// La marque et le modèle de la carte mère.
    pub marque_carte_mere: Option<String>,
    pub modele_carte_mere: Option<String>,
    pub processeur: Option<String>,
    /// La mémoire installée, en octets, et le nombre de barrettes.
    pub memoire: Option<u64>,
    pub barrettes: usize,
    /// « SSD NVMe », « SSD », « disque dur ».
    pub disque_genre: Option<&'static str>,
    pub disque_taille: Option<u64>,
    pub disque_libre: Option<u64>,
    /// « Windows 11, version 24H2 ».
    pub windows: Option<String>,
    /// Une machine virtuelle (VMware, VirtualBox, Hyper-V...) : ni type ni marque.
    pub virtuel: bool,
}

/// Les fabricants de cartes mères : une de ces marques sur la carte mère d'un PC fixe sans autre marque, c'est un PC
/// monté.
const FABRICANTS_DE_CARTES: [&str; 6] = ["ASUS", "MSI", "Gigabyte", "ASRock", "Biostar", "EVGA"];

/// Les marques de PC « de marque » (leurs pannes propres : codes des voyants, touches du BIOS...).
const MARQUES_DE_PC: [&str; 15] = [
    "HP",
    "Dell",
    "Lenovo",
    "Acer",
    "Microsoft Surface",
    "Apple",
    "Toshiba",
    "Samsung",
    "Huawei",
    "Medion",
    "Fujitsu",
    "Sony",
    "Xiaomi",
    "Razer",
    "LG",
];

/// Un texte de remplissage laissé par le fabricant (« To Be Filled By O.E.M. », « System manufacturer »...), ou vide.
pub fn remplissage(t: &str) -> bool {
    let t = t.trim().to_lowercase();
    if t.is_empty() || t.chars().all(|c| !c.is_alphanumeric() || c == 'x' || c == '0') {
        return true;
    }
    const EXACTS: [&str; 16] = [
        "oem",
        "o.e.m.",
        "n/a",
        "na",
        "none",
        "unknown",
        "invalid",
        "default",
        "not specified",
        "not applicable",
        "not available",
        "manufacturer",
        "product name",
        "type1productconfigid",
        "standard pc",
        "undefined",
    ];
    const MORCEAUX: [&str; 9] = [
        "o.e.m",
        "filled by",
        "default string",
        "system manufacturer",
        "system product",
        "system version",
        "base board",
        "baseboard manufacturer",
        "chassis manufacturer",
    ];
    EXACTS.contains(&t.as_str()) || MORCEAUX.iter().any(|m| t.contains(m))
}

/// Le mot `m` est-il au début d'un mot de `t` (déjà en minuscules) ?
fn mot(t: &str, m: &str) -> bool {
    t.match_indices(m).any(|(i, _)| {
        let avant = t[..i].chars().last().is_none_or(|c| !c.is_alphanumeric());
        let apres = t[i + m.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        avant && apres
    })
}

/// Une machine virtuelle, d'après le fabricant et le modèle.
pub fn virtuel(fabricant: &str, modele: &str) -> bool {
    let f = fabricant.to_lowercase();
    let m = modele.to_lowercase();
    ["vmware", "innotek", "virtualbox", "qemu", "xen", "parallels", "bochs", "red hat"].iter().any(|x| f.contains(x))
        || m.contains("virtual machine")
        || m.contains("virtualbox")
        || m.starts_with("vmware")
        || m.starts_with("kvm")
}

/// La marque en nom simple (« Hewlett-Packard » : « HP », « ASUSTeK COMPUTER INC. » : « ASUS »...), un des noms de
/// [`mindcare_moteur::MARQUES`]. Une marque inconnue garde son nom (sans espaces autour) ; un texte de remplissage
/// donne rien. `modele` départage Microsoft (seule la Surface est une marque de PC ; « Virtual Machine » : rien).
pub fn marque_simple(brut: &str, modele: &str) -> Option<String> {
    if remplissage(brut) || virtuel(brut, modele) {
        return None;
    }
    let t = brut.trim().to_lowercase();
    let connue = |n: &str| Some(n.to_string());
    if t.contains("hewlett") || mot(&t, "hp") || t.starts_with("hpinc") {
        return connue("HP");
    }
    if mot(&t, "dell") || t.contains("alienware") {
        return connue("Dell");
    }
    if t.contains("lenovo") {
        return connue("Lenovo");
    }
    if t.contains("asustek") || t.starts_with("asus") {
        return connue("ASUS");
    }
    if t.contains("micro-star") || t.contains("micro star") || mot(&t, "msi") {
        return connue("MSI");
    }
    if t.contains("gigabyte") {
        return connue("Gigabyte");
    }
    if t.contains("asrock") {
        return connue("ASRock");
    }
    if mot(&t, "acer") {
        return connue("Acer");
    }
    if t.contains("microsoft") {
        return if modele.to_lowercase().contains("surface") { connue("Microsoft Surface") } else { None };
    }
    let table: [(&[&str], &str); 13] = [
        (&["toshiba", "dynabook"], "Toshiba"),
        (&["samsung"], "Samsung"),
        (&["huawei"], "Huawei"),
        (&["medion"], "Medion"),
        (&["fujitsu"], "Fujitsu"),
        (&["razer"], "Razer"),
        (&["apple"], "Apple"),
        (&["lg electronics"], "LG"),
        (&["sony", "vaio"], "Sony"),
        (&["xiaomi", "timi"], "Xiaomi"),
        (&["intel"], "Intel"),
        (&["biostar"], "Biostar"),
        (&["evga"], "EVGA"),
    ];
    for (mots, nom) in table {
        if mots.iter().any(|m| mot(&t, m)) {
            return connue(nom);
        }
    }
    if t == "lg" {
        return connue("LG");
    }
    Some(brut.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Le vote du boîtier (ChassisTypes) : portable, fixe, ou rien (autre, inconnu, ou des types qui se contredisent).
fn vote_boitier(boitiers: &[u16]) -> Option<TypePc> {
    let un = |c: u16| match c {
        8 | 9 | 10 | 11 | 14 | 30 | 31 | 32 => Some(TypePc::Portable),
        3..=7 | 13 | 15 | 16 | 17 | 23 | 24 | 35 | 36 => Some(TypePc::Fixe),
        _ => None,
    };
    let votes: Vec<TypePc> = boitiers.iter().filter_map(|c| un(*c)).collect();
    match votes.first() {
        Some(v) if votes.iter().all(|x| x == v) => Some(*v),
        _ => None,
    }
}

/// Le vote de Windows (PCSystemType).
fn vote_systeme(genre: Option<u16>) -> Option<TypePc> {
    match genre? {
        2 | 8 => Some(TypePc::Portable),
        1 | 3 => Some(TypePc::Fixe),
        _ => None,
    }
}

/// Portable ou fixe, d'après le boîtier, Windows et la batterie. Une batterie vote « portable » (son absence ne vote
/// pas : un onduleur branché en USB est aussi une batterie, et un portable peut avoir la sienne retirée). La majorité
/// l'emporte ; à égalité, on ne sait pas.
pub fn genre_du_pc(boitiers: &[u16], genre_systeme: Option<u16>, batterie: Option<bool>) -> Option<TypePc> {
    let votes =
        [vote_boitier(boitiers), vote_systeme(genre_systeme), batterie.and_then(|b| b.then_some(TypePc::Portable))];
    let portable = votes.iter().filter(|v| **v == Some(TypePc::Portable)).count();
    let fixe = votes.iter().filter(|v| **v == Some(TypePc::Fixe)).count();
    match portable.cmp(&fixe) {
        std::cmp::Ordering::Greater => Some(TypePc::Portable),
        std::cmp::Ordering::Less => Some(TypePc::Fixe),
        std::cmp::Ordering::Equal => None,
    }
}

/// La version de Windows d'après son numéro de build (« Windows 11, version 24H2 ») ; le nom de Windows (Caption)
/// quand le build est inconnu.
pub fn version_windows(caption: &str, build: &str) -> Option<String> {
    let n: Option<u32> = build.trim().parse().ok();
    let nom = match n {
        Some(b) if b >= 22000 => "Windows 11",
        Some(b) if b >= 10240 => "Windows 10",
        _ => {
            let c = caption.trim().trim_start_matches("Microsoft").trim();
            return if c.is_empty() { None } else { Some(c.to_string()) };
        }
    };
    let version = match n? {
        10240 => "1507",
        10586 => "1511",
        14393 => "1607",
        15063 => "1703",
        16299 => "1709",
        17134 => "1803",
        17763 => "1809",
        18362 => "1903",
        18363 => "1909",
        19041 => "2004",
        19042 => "20H2",
        19043 => "21H1",
        19044 => "21H2",
        19045 => "22H2",
        22000 => "21H2",
        22621 => "22H2",
        22631 => "23H2",
        26100 => "24H2",
        26200 => "25H2",
        _ => "",
    };
    Some(if version.is_empty() { nom.to_string() } else { format!("{nom}, version {version}") })
}

/// Une taille pour les gens, comme Windows la montre (en Go de 1024 Mo) : « 16 Go », « 476 Go », « 1,8 To ».
pub fn taille(octets: u64) -> String {
    let go = octets as f64 / (1u64 << 30) as f64;
    if go >= 1000.0 {
        let to = go / 1024.0;
        let t = format!("{to:.1}").replace('.', ",");
        format!("{} To", t.trim_end_matches(",0"))
    } else if go >= 10.0 {
        // (comme l'Explorateur : un SSD de 512 Go du commerce fait « 476 Go »)
        format!("{} Go", go.floor() as u64)
    } else {
        let t = format!("{go:.1}").replace('.', ",");
        format!("{} Go", t.trim_end_matches(",0"))
    }
}

/// Un texte lu, sans ses espaces en trop, rien si c'est du remplissage.
fn propre(t: &str) -> Option<String> {
    if remplissage(t) { None } else { Some(t.split_whitespace().collect::<Vec<_>>().join(" ")) }
}

/// « marque modèle », sans répéter la marque si le modèle commence déjà par elle.
fn avec_marque(marque: Option<&str>, modele: Option<&str>) -> Option<String> {
    match (marque, modele) {
        (Some(m), Some(x)) if x.to_lowercase().starts_with(&m.to_lowercase()) => Some(x.to_string()),
        (Some(m), Some(x)) => Some(format!("{m} {x}")),
        (Some(m), None) => Some(m.to_string()),
        (None, Some(x)) => Some(x.to_string()),
        (None, None) => None,
    }
}

impl PcDetecte {
    /// Le PC reconnu d'après les valeurs brutes de WMI.
    pub fn depuis(b: &PcBrut) -> PcDetecte {
        let est_virtuel = virtuel(&b.fabricant, &b.modele) || virtuel(&b.carte_fabricant, &b.carte_modele);
        let genre = if est_virtuel { None } else { genre_du_pc(&b.boitiers, b.genre_systeme, b.batterie) };
        let marque_pc = marque_simple(&b.fabricant, &b.modele);
        let marque_cm = if est_virtuel { None } else { marque_simple(&b.carte_fabricant, &b.carte_modele) };
        // (Lenovo écrit le nom parlant dans la version du produit, et un code dans le modèle)
        let modele_pc = match (marque_pc.as_deref(), propre(&b.version_produit)) {
            (Some("Lenovo"), Some(v)) if !v.eq_ignore_ascii_case("lenovo") => Some(v),
            _ => propre(&b.modele),
        };
        let cm_de_cartes = marque_cm.as_deref().is_some_and(|m| FABRICANTS_DE_CARTES.contains(&m));
        let pc_de_marque = marque_pc.as_deref().is_some_and(|m| MARQUES_DE_PC.contains(&m));
        let monte = !est_virtuel && genre != Some(TypePc::Portable) && cm_de_cartes && !pc_de_marque;
        let marque = if est_virtuel {
            None
        } else if monte {
            marque_cm.clone()
        } else {
            marque_pc.clone().or_else(|| marque_cm.clone())
        };
        let tailles: Vec<u64> = b.barrettes.iter().flatten().copied().collect();
        let memoire = if tailles.is_empty() { None } else { Some(tailles.iter().sum()) };
        let disque_genre = b.disque.as_ref().and_then(|d| match (d.media, d.bus) {
            (_, Some(17)) => Some("SSD NVMe"),
            (Some(4), _) => Some("SSD"),
            (Some(3), _) => Some("disque dur"),
            _ => None,
        });
        PcDetecte {
            genre,
            marque,
            monte,
            marque_pc,
            modele_pc,
            marque_carte_mere: marque_cm,
            modele_carte_mere: propre(&b.carte_modele),
            processeur: propre(&b.processeur),
            memoire,
            barrettes: b.barrettes.len(),
            disque_genre,
            disque_taille: b.disque.as_ref().and_then(|d| d.taille).or(b.place.map(|p| p.taille)).filter(|t| *t > 0),
            disque_libre: b.place.map(|p| p.libre),
            windows: version_windows(&b.systeme, &b.build),
            virtuel: est_virtuel,
        }
    }

    /// Le profil pour trier les pannes (type et marque qui compte).
    pub fn profil(&self) -> ProfilPc {
        ProfilPc { genre: self.genre, marque: self.marque.clone() }
    }

    /// La pastille « Ton PC » : « Portable HP », « PC fixe ASUS »... (« Tous les PC » si rien n'est reconnu).
    pub fn libelle_court(&self) -> String {
        self.profil().libelle_court()
    }

    /// Les lignes du PC pour le rapport du réparateur (sans nom d'utilisateur ni numéro de série).
    pub fn fiche(&self) -> FichePc {
        let modele = if self.monte { None } else { avec_marque(self.marque_pc.as_deref(), self.modele_pc.as_deref()) };
        let memoire = self.memoire.map(|m| match self.barrettes {
            0 | 1 => taille(m),
            n => format!("{} ({n} barrettes)", taille(m)),
        });
        let disque = match (self.disque_genre, self.disque_taille, self.disque_libre) {
            (None, None, None) => None,
            (g, t, l) => {
                let mut x = g.map(|g| g.to_string()).unwrap_or_else(|| "Disque".to_string());
                if let Some(t) = t {
                    x.push_str(&format!(" de {}", taille(t)));
                }
                if let Some(l) = l {
                    x.push_str(&format!(", {} libres", taille(l)));
                }
                Some(x[..1].to_uppercase() + &x[1..])
            }
        };
        FichePc {
            genre: if self.virtuel {
                Some("Machine virtuelle".to_string())
            } else {
                self.genre.map(|g| if self.monte { "PC fixe monté".to_string() } else { g.libelle().to_string() })
            },
            modele,
            carte_mere: avec_marque(self.marque_carte_mere.as_deref(), self.modele_carte_mere.as_deref()),
            processeur: self.processeur.clone(),
            memoire,
            disque,
            windows: self.windows.clone(),
        }
    }
}

/// Reconnaît le PC (en lecture seule, sans fenêtre, à faire hors du fil de la fenêtre : quelques requêtes WMI). Une
/// lecture ratée : un PC inconnu (toutes les pannes montrées), et la raison.
pub fn detecter(source: &dyn SourceWindows) -> (PcDetecte, Option<String>) {
    match source.materiel() {
        Ok(b) => (PcDetecte::depuis(&b), None),
        Err(e) => (PcDetecte::default(), Some(e)),
    }
}
