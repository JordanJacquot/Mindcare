//! Le PC reconnu, avec des valeurs inventées (jamais celles du vrai PC) : les marques ramenées à des noms simples, le
//! type (boîtier, Windows, batterie), la marque qui compte (celle de la carte mère pour un PC monté), la version de
//! Windows, les tailles, la fiche du rapport, et « dans le doute, on ne sait pas ».

use mindcare_moteur::pc::FichePc;
use mindcare_moteur::{MARQUES, ProfilPc, TypePc};
use mindcare_windows::pc::{genre_du_pc, remplissage, taille, version_windows};
use mindcare_windows::{
    Barrette, Batterie, Disque, DisqueBrut, DisquePhysique, Evenement, Filtre, InfosPc, Mesure, Moment, Partition,
    PcBrut, PcDetecte, Place, SourceWindows, detecter, marque_simple,
};

const GO: u64 = 1 << 30;

#[test]
fn les_marques_en_noms_simples() {
    let cas = [
        ("Hewlett-Packard", "HP"),
        ("HP", "HP"),
        ("HP Inc.", "HP"),
        ("Dell Inc.", "Dell"),
        ("Alienware", "Dell"),
        ("LENOVO", "Lenovo"),
        ("ASUSTeK COMPUTER INC.", "ASUS"),
        ("ASUSTeK Computer Inc.", "ASUS"),
        ("Micro-Star International Co., Ltd.", "MSI"),
        ("MSI", "MSI"),
        ("Gigabyte Technology Co., Ltd.", "Gigabyte"),
        ("ASRock", "ASRock"),
        ("Acer", "Acer"),
        ("TOSHIBA", "Toshiba"),
        ("Dynabook Inc.", "Toshiba"),
        ("SAMSUNG ELECTRONICS CO., LTD.", "Samsung"),
        ("HUAWEI", "Huawei"),
        ("MEDION", "Medion"),
        ("FUJITSU", "Fujitsu"),
        ("Razer", "Razer"),
        ("Apple Inc.", "Apple"),
        ("LG Electronics", "LG"),
        ("Sony Corporation", "Sony"),
        ("VAIO Corporation", "Sony"),
        ("TIMI", "Xiaomi"),
        ("Intel Corporation", "Intel"),
        ("BIOSTAR Group", "Biostar"),
        ("EVGA", "EVGA"),
        ("  hp  ", "HP"),
    ];
    for (brut, simple) in cas {
        assert_eq!(marque_simple(brut, "").as_deref(), Some(simple), "{brut}");
        assert!(MARQUES.contains(&simple), "{simple} absent de MARQUES");
    }
    // Microsoft : seulement la Surface ; une machine virtuelle Hyper-V n'a pas de marque
    assert_eq!(marque_simple("Microsoft Corporation", "Surface Laptop 5").as_deref(), Some("Microsoft Surface"));
    assert_eq!(marque_simple("Microsoft Corporation", "Virtual Machine"), None);
    // (des mots qui contiennent une marque sans l'être)
    assert_eq!(marque_simple("Tracer Systems", "").as_deref(), Some("Tracer Systems"));
    assert_eq!(marque_simple("Shpelldell", "").as_deref(), Some("Shpelldell"));
    // Une marque inconnue garde son nom, sans espaces en trop
    assert_eq!(marque_simple("  Wortmann   AG ", "").as_deref(), Some("Wortmann AG"));
}

#[test]
fn les_textes_de_remplissage_sont_inconnus() {
    for t in [
        "To Be Filled By O.E.M.",
        "To be filled by O.E.M.",
        "System manufacturer",
        "System Product Name",
        "System Version",
        "Default string",
        "OEM",
        "O.E.M.",
        "Not Applicable",
        "Not Specified",
        "Unknown",
        "None",
        "Base Board Manufacturer",
        "Type1ProductConfigId",
        "xxxxxxxx",
        "0",
        "",
        "   ",
        "N/A",
        "Standard PC",
    ] {
        assert!(remplissage(t), "« {t} »");
        assert_eq!(marque_simple(t, ""), None, "« {t} »");
    }
    for t in ["VMware, Inc.", "innotek GmbH", "QEMU", "Xen", "Parallels Software International Inc."] {
        assert_eq!(marque_simple(t, ""), None, "« {t} » (virtuel)");
    }
    assert!(!remplissage("HP"));
    assert!(!remplissage("B550 AORUS ELITE"));
}

#[test]
fn portable_ou_fixe() {
    use TypePc::{Fixe, Portable};
    // Le boîtier, Windows et la batterie d'accord
    assert_eq!(genre_du_pc(&[10], Some(2), Some(true)), Some(Portable));
    assert_eq!(genre_du_pc(&[3], Some(1), Some(false)), Some(Fixe));
    assert_eq!(genre_du_pc(&[7], None, None), Some(Fixe));
    assert_eq!(genre_du_pc(&[31], None, None), Some(Portable));
    assert_eq!(genre_du_pc(&[13], None, None), Some(Fixe));
    // Une tour avec un onduleur branché en USB (une « batterie ») reste une tour
    assert_eq!(genre_du_pc(&[3], Some(1), Some(true)), Some(Fixe));
    // Windows seul, la batterie seule
    assert_eq!(genre_du_pc(&[2], Some(2), None), Some(Portable));
    assert_eq!(genre_du_pc(&[], Some(8), None), Some(Portable));
    assert_eq!(genre_du_pc(&[], None, Some(true)), Some(Portable));
    // Pas de batterie ne prouve rien (un portable sans sa batterie)
    assert_eq!(genre_du_pc(&[], None, Some(false)), None);
    // Rien de sûr : inconnu (et des mesures qui se contredisent : inconnu)
    assert_eq!(genre_du_pc(&[1], Some(0), None), None);
    assert_eq!(genre_du_pc(&[], None, None), None);
    assert_eq!(genre_du_pc(&[3], Some(2), None), None);
    assert_eq!(genre_du_pc(&[9, 3], None, None), None);
    // Deux contre un : la majorité
    assert_eq!(genre_du_pc(&[3], Some(2), Some(true)), Some(Portable));
}

#[test]
fn la_version_de_windows() {
    assert_eq!(version_windows("Microsoft Windows 11 Famille", "26100").as_deref(), Some("Windows 11, version 24H2"));
    assert_eq!(version_windows("Microsoft Windows 11 Pro", "26200").as_deref(), Some("Windows 11, version 25H2"));
    assert_eq!(version_windows("Microsoft Windows 11 Famille", "22631").as_deref(), Some("Windows 11, version 23H2"));
    assert_eq!(version_windows("Microsoft Windows 10 Famille", "19045").as_deref(), Some("Windows 10, version 22H2"));
    assert_eq!(version_windows("", "27999").as_deref(), Some("Windows 11"));
    assert_eq!(version_windows("Microsoft Windows 8.1", "9600").as_deref(), Some("Windows 8.1"));
    assert_eq!(version_windows("", ""), None);
}

#[test]
fn les_tailles() {
    assert_eq!(taille(16 * GO), "16 Go");
    assert_eq!(taille(512_110_190_592), "476 Go");
    assert_eq!(taille(2_000_398_934_016), "1,8 To");
    assert_eq!(taille(1024 * GO), "1 To");
    assert_eq!(taille(GO * 3 / 2), "1,5 Go");
    assert_eq!(taille(8 * GO), "8 Go");
}

fn portable_hp() -> PcBrut {
    PcBrut {
        boitiers: vec![10],
        genre_systeme: Some(2),
        batterie: Some(true),
        fabricant: "HP".into(),
        modele: "HP Pavilion Laptop 15-eh1xxx".into(),
        version_produit: "Type1ProductConfigId".into(),
        carte_fabricant: "HP".into(),
        carte_modele: "87B2".into(),
        processeur: "AMD Ryzen 5 5500U with Radeon Graphics         ".into(),
        barrettes: vec![Some(8 * GO), Some(8 * GO)],
        disque: Some(DisqueBrut { media: Some(4), bus: Some(17), taille: Some(512_110_190_592) }),
        place: Some(Place { libre: 120 * GO, taille: 510 * GO }),
        systeme: "Microsoft Windows 11 Famille".into(),
        build: "26100".into(),
    }
}

fn tour_montee_asus() -> PcBrut {
    PcBrut {
        boitiers: vec![3],
        genre_systeme: Some(1),
        batterie: Some(false),
        fabricant: "System manufacturer".into(),
        modele: "System Product Name".into(),
        version_produit: "System Version".into(),
        carte_fabricant: "ASUSTeK COMPUTER INC.".into(),
        carte_modele: "ROG STRIX B550-F GAMING".into(),
        processeur: "AMD Ryzen 7 5800X 8-Core Processor".into(),
        barrettes: vec![Some(16 * GO), Some(16 * GO)],
        disque: Some(DisqueBrut { media: Some(3), bus: Some(11), taille: Some(2_000_398_934_016) }),
        place: Some(Place { libre: 900 * GO, taille: 1862 * GO }),
        systeme: "Microsoft Windows 10 Professionnel".into(),
        build: "19045".into(),
    }
}

#[test]
fn un_portable_hp() {
    let d = PcDetecte::depuis(&portable_hp());
    assert_eq!(d.profil(), ProfilPc { genre: Some(TypePc::Portable), marque: Some("HP".into()) });
    assert_eq!(d.libelle_court(), "Portable HP");
    assert!(!d.monte);
    assert_eq!(
        d.fiche(),
        FichePc {
            genre: Some("Portable".into()),
            modele: Some("HP Pavilion Laptop 15-eh1xxx".into()),
            carte_mere: Some("HP 87B2".into()),
            processeur: Some("AMD Ryzen 5 5500U with Radeon Graphics".into()),
            memoire: Some("16 Go (2 barrettes)".into()),
            disque: Some("SSD NVMe de 476 Go, 120 Go libres".into()),
            windows: Some("Windows 11, version 24H2".into()),
        }
    );
}

#[test]
fn une_tour_montee_prend_la_marque_de_sa_carte_mere() {
    let d = PcDetecte::depuis(&tour_montee_asus());
    assert_eq!(d.profil(), ProfilPc { genre: Some(TypePc::Fixe), marque: Some("ASUS".into()) });
    assert_eq!(d.libelle_court(), "PC fixe ASUS");
    assert!(d.monte);
    assert_eq!(d.marque_pc, None);
    let f = d.fiche();
    assert_eq!(f.genre.as_deref(), Some("PC fixe monté"));
    assert_eq!(f.modele, None);
    assert_eq!(f.carte_mere.as_deref(), Some("ASUS ROG STRIX B550-F GAMING"));
    assert_eq!(f.disque.as_deref(), Some("Disque dur de 1,8 To, 900 Go libres"));
    assert_eq!(f.windows.as_deref(), Some("Windows 10, version 22H2"));

    // La carte mère MSI qui écrit aussi son nom comme fabricant du PC : un PC monté MSI
    let mut b = tour_montee_asus();
    b.fabricant = "Micro-Star International Co., Ltd.".into();
    b.modele = "MS-7C56".into();
    b.carte_fabricant = "Micro-Star International Co., Ltd.".into();
    let d = PcDetecte::depuis(&b);
    assert!(d.monte);
    assert_eq!(d.marque.as_deref(), Some("MSI"));
    // Un PC de marque (HP) avec une carte mère d'un fabricant de cartes : c'est HP qui compte
    b.fabricant = "HP".into();
    let d = PcDetecte::depuis(&b);
    assert!(!d.monte);
    assert_eq!(d.marque.as_deref(), Some("HP"));
    // Un portable ASUS (le fabricant du PC et de la carte mère) : ASUS, pas « monté »
    let mut b = portable_hp();
    b.fabricant = "ASUSTeK COMPUTER INC.".into();
    b.modele = "VivoBook_ASUSLaptop X515EA".into();
    b.carte_fabricant = "ASUSTeK COMPUTER INC.".into();
    let d = PcDetecte::depuis(&b);
    assert_eq!(d.libelle_court(), "Portable ASUS");
    assert!(!d.monte);
}

#[test]
fn lenovo_et_son_nom_parlant() {
    let mut b = portable_hp();
    b.fabricant = "LENOVO".into();
    b.modele = "20W0005UFR".into();
    b.version_produit = "ThinkPad T14 Gen 2i".into();
    b.carte_fabricant = "LENOVO".into();
    let d = PcDetecte::depuis(&b);
    assert_eq!(d.marque.as_deref(), Some("Lenovo"));
    assert_eq!(d.fiche().modele.as_deref(), Some("Lenovo ThinkPad T14 Gen 2i"));
}

#[test]
fn dans_le_doute_on_ne_sait_pas() {
    // Rien de lu : tout est inconnu, la fiche est vide
    let d = PcDetecte::depuis(&PcBrut::default());
    assert!(d.profil().est_inconnu());
    assert_eq!(d.libelle_court(), "Tous les PC");
    assert_eq!(d.fiche(), FichePc::default());
    // Une tour sans aucune marque : fixe, marque inconnue
    let mut b = tour_montee_asus();
    b.carte_fabricant = "To Be Filled By O.E.M.".into();
    let d = PcDetecte::depuis(&b);
    assert_eq!(d.profil(), ProfilPc { genre: Some(TypePc::Fixe), marque: None });
    // Une machine virtuelle : ni type ni marque
    let mut b = portable_hp();
    b.fabricant = "VMware, Inc.".into();
    b.modele = "VMware7,1".into();
    let d = PcDetecte::depuis(&b);
    assert!(d.virtuel);
    assert!(d.profil().est_inconnu());
    assert_eq!(d.fiche().genre.as_deref(), Some("Machine virtuelle"));
}

/// Un faux Windows qui ne sait rien d'autre que le matériel.
struct Faux(Mesure<PcBrut>);

impl SourceWindows for Faux {
    fn plus_ancien(&self) -> Mesure<Option<Moment>> {
        Ok(None)
    }
    fn evenements(&self, _: &Filtre) -> Mesure<Vec<Evenement>> {
        Ok(Vec::new())
    }
    fn disques(&self) -> Mesure<Vec<Disque>> {
        Ok(Vec::new())
    }
    fn partitions(&self) -> Mesure<Vec<Partition>> {
        Ok(Vec::new())
    }
    fn disques_physiques(&self) -> Mesure<Vec<DisquePhysique>> {
        Ok(Vec::new())
    }
    fn lecteur_systeme(&self) -> String {
        "C:".into()
    }
    fn place(&self, _: &str) -> Mesure<Place> {
        Err("non".into())
    }
    fn batterie(&self) -> Mesure<Option<Batterie>> {
        Ok(None)
    }
    fn demarrage(&self) -> Mesure<Moment> {
        Err("non".into())
    }
    fn barrettes(&self) -> Mesure<Vec<Barrette>> {
        Ok(Vec::new())
    }
    fn infos_pc(&self) -> Mesure<InfosPc> {
        Ok(InfosPc::default())
    }
    fn materiel(&self) -> Mesure<PcBrut> {
        self.0.clone()
    }
}

#[test]
fn detecter_par_la_source() {
    let (d, e) = detecter(&Faux(Ok(portable_hp())));
    assert_eq!((d.libelle_court().as_str(), e), ("Portable HP", None));
    // Une lecture ratée : un PC inconnu (tout est montré), et la raison
    let (d, e) = detecter(&Faux(Err("WMI illisible".into())));
    assert!(d.profil().est_inconnu());
    assert_eq!(e.as_deref(), Some("WMI illisible"));
}

#[cfg(windows)]
#[test]
fn les_requetes_ne_lisent_rien_de_personnel() {
    for (wql, champs) in mindcare_windows::windows::REQUETES_PC {
        let tout = format!("{wql} {}", champs.join(" ")).to_lowercase();
        for interdit in
            ["serial", "identifyingnumber", "uuid", "username", "user", "name from win32_computersystem", "*"]
        {
            assert!(!tout.contains(interdit), "« {wql} » lit « {interdit} »");
        }
    }
}
