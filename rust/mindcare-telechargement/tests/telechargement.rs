//! Les téléchargements avec WinHTTP et un faux serveur sur 127.0.0.1 (aucun vrai Internet), les cas des tests du
//! PowerShell (outils\tester-assistant.ps1, « Bug 5 » et « Bug 11 ») et ceux de l'étape 6.1.

#![cfg(windows)]

mod commun;

use std::fs;
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use commun::{Action, Essai, FauxServeur, Place, fichier, octets, tous_les_fichiers, zip};
use mindcare_telechargement::windows::WinHttp;
use mindcare_telechargement::{
    Avancement, Etat, Extraire, Fichier, Fin, Installation, MO, Motif, Paquet, Telechargement, Transport,
};

fn reseau() -> Arc<dyn Transport> {
    Arc::new(WinHttp::sans_proxy().expect("WinHTTP"))
}

/// Télécharge et attend la fin (30 s au plus), sans pause entre deux essais
fn telecharger(fichiers: Vec<Fichier>, dossier: &Path, marge: u64, place: Option<u64>) -> Avancement {
    let mut t = Telechargement::lancer(fichiers, dossier, marge, reseau(), &Place(place), Duration::ZERO);
    assert!(t.attendre(Duration::from_secs(30)), "téléchargement trop long");
    t.avancement()
}

/// Ouvre un fichier sans le partager (comme un antivirus, ou une .dll encore chargée)
fn bloquer(p: &Path) -> fs::File {
    fs::OpenOptions::new().read(true).share_mode(0).open(p).expect("bloquer")
}

fn moteur_zip() -> Extraire {
    Extraire(vec![
        Motif::Nom("llama-server.exe".into()),
        Motif::Nom("llama.dll".into()),
        Motif::DebutFin("ggml".into(), ".dll".into()),
    ])
}

/// Un faux assistant : son dossier, le zip du moteur (et son contenu) et un modèle, sur le faux serveur
fn paquet(dossier: &Path, serveur: &FauxServeur, zip_moteur: &[u8], modele: &[u8]) -> Paquet {
    let programme = dossier.join("llama-server.exe");
    let fichier_modele = dossier.join("mindcare-0.6b-v3-q8_0.gguf");
    Paquet {
        dossier: dossier.to_path_buf(),
        fichiers: vec![
            fichier(serveur, "moteur.zip", zip_moteur, &programme, Some(moteur_zip())),
            fichier(serveur, "mindcare-0.6b-v3-q8_0.gguf", modele, &fichier_modele, None),
        ],
        programme,
        dll: vec!["llama.dll".into(), "ggml.dll".into()],
        modele: fichier_modele,
        anciens: Some(("mindcare-".into(), ".gguf".into())),
        marge: 0,
    }
}

#[test]
fn le_fichier_arrive_quand_tout_va_bien() {
    let o = Arc::new(octets(300_000, 1));
    let s = {
        let o = o.clone();
        FauxServeur::lancer(move |_, _| Action::fichier(&o))
    };
    let e = Essai::nouveau("complet");
    let garder = e.0.join("ia").join("modele.gguf");
    let f = fichier(&s, "modele.gguf", &o, &garder, None);
    let a = telecharger(vec![f], &e.0.join("ia"), 0, None);
    assert_eq!(a.erreur, "");
    assert!(a.fini && !a.annule);
    assert_eq!((a.recu, a.total, a.pourcent()), (300_000, 300_000, 100));
    assert_eq!(fs::read(&garder).unwrap(), *o);
    assert!(!e.0.join("ia").join("modele.gguf.telechargement").exists(), "le fichier en cours est renommé");
    assert_eq!(s.demandes().len(), 1);
    assert_eq!(s.demandes()[0].depuis, None);
}

#[test]
fn coupure_puis_reprise_la_ou_il_s_est_arrete() {
    let o = Arc::new(octets(200_000, 2));
    let coupure = 80_000;
    let s = {
        let o = o.clone();
        FauxServeur::lancer(move |_, n| Action::Donner {
            octets: o.clone(),
            reprise: true,
            couper_apres: (n == 0).then_some(coupure),
            lent: None,
        })
    };
    let e = Essai::nouveau("reprise");
    let garder = e.0.join("modele.gguf");
    let a = telecharger(vec![fichier(&s, "modele.gguf", &o, &garder, None)], &e.0, 0, None);
    assert_eq!(a.erreur, "");
    assert_eq!(fs::read(&garder).unwrap(), *o);
    let d = s.demandes();
    assert_eq!(d.len(), 2, "{d:?}");
    assert_eq!(d[0].depuis, None);
    // (la deuxième demande ne redemande que la suite)
    assert!(d[1].depuis.is_some_and(|x| x > 0 && x <= coupure as u64), "{d:?}");
}

#[test]
fn serveur_sans_reprise_recommence_au_debut() {
    let o = Arc::new(octets(150_000, 3));
    let s = {
        let o = o.clone();
        FauxServeur::lancer(move |_, n| Action::Donner {
            octets: o.clone(),
            reprise: false,
            couper_apres: (n == 0).then_some(50_000),
            lent: None,
        })
    };
    let e = Essai::nouveau("sans-reprise");
    let garder = e.0.join("modele.gguf");
    let a = telecharger(vec![fichier(&s, "modele.gguf", &o, &garder, None)], &e.0, 0, None);
    assert_eq!(a.erreur, "");
    assert_eq!(fs::read(&garder).unwrap(), *o, "pas de morceau en double");
}

#[test]
fn mauvaise_empreinte_refusee_et_fichier_efface() {
    let o = Arc::new(octets(50_000, 4));
    let s = {
        let o = o.clone();
        FauxServeur::lancer(move |_, _| Action::fichier(&o))
    };
    let e = Essai::nouveau("empreinte");
    let garder = e.0.join("modele.gguf");
    let mut f = fichier(&s, "modele.gguf", &o, &garder, None);
    f.sha256 = commun::sha256(b"un autre fichier");
    let a = telecharger(vec![f], &e.0, 0, None);
    assert_eq!(
        a.erreur,
        "le fichier « fichier modele.gguf » arrive abîmé (deux fois de suite). Un antivirus le modifie peut-être pendant le téléchargement"
    );
    assert!(!garder.exists());
    assert!(!e.0.join("modele.gguf.telechargement").exists(), "le fichier abîmé est effacé");
    assert_eq!(s.demandes().len(), 2, "téléchargé une deuxième fois avant d'abandonner");
}

#[test]
fn zip_seuls_les_fichiers_voulus_et_rien_hors_du_dossier() {
    let gros = octets(120_000, 5);
    let z = zip(
        &[
            ("llama/llama-server.exe", b"faux llama-server", true),
            ("llama/llama.dll", &gros, true),
            ("llama/ggml.dll", b"faux ggml", false),
            ("llama/GGML-Vulkan.dll", b"faux vulkan", true),
            ("llama/autre.txt", b"pas voulu", true),
            ("llama/llama-cli.exe", b"pas voulu", true),
            ("../../dehors.dll", b"pas voulu", true),
            ("..\\..\\ggml-remonte.dll", b"voulu, mais dans le dossier", true),
            ("C:\\Windows\\ggml-absolu.dll", b"voulu, mais dans le dossier", false),
            ("/ggml-racine.dll", b"voulu, mais dans le dossier", true),
            ("llama/..", b"rien", false),
            ("llama/sous/", b"", false),
        ],
        false,
    );
    let s = {
        let z = Arc::new(z.clone());
        FauxServeur::lancer(move |_, _| Action::fichier(&z))
    };
    let e = Essai::nouveau("zip");
    let dossier = e.0.join("donnees").join("ia");
    let f = fichier(&s, "moteur.zip", &z, &dossier.join("llama-server.exe"), Some(moteur_zip()));
    let a = telecharger(vec![f], &dossier, 0, None);
    assert_eq!(a.erreur, "");
    let noms: Vec<String> = tous_les_fichiers(&e.0)
        .iter()
        .map(|p| p.strip_prefix(&dossier).expect("hors du dossier").display().to_string())
        .collect();
    let mut attendus = vec![
        "llama-server.exe",
        "llama.dll",
        "ggml.dll",
        "GGML-Vulkan.dll",
        "ggml-remonte.dll",
        "ggml-absolu.dll",
        "ggml-racine.dll",
    ];
    attendus.sort_by_key(|n| n.to_ascii_lowercase());
    let mut vus = noms.clone();
    vus.sort_by_key(|n| n.to_ascii_lowercase());
    assert_eq!(vus, attendus, "seuls les fichiers voulus, tous dans le dossier, et le zip effacé");
    assert_eq!(fs::read(dossier.join("llama.dll")).unwrap(), gros);
    assert_eq!(fs::read(dossier.join("ggml-remonte.dll")).unwrap(), b"voulu, mais dans le dossier");
}

#[test]
fn zip64_lu_aussi() {
    let z = zip(&[("bin/llama-server.exe", b"faux", true), ("bin/ggml.dll", b"faux ggml", false)], true);
    let s = {
        let z = Arc::new(z.clone());
        FauxServeur::lancer(move |_, _| Action::fichier(&z))
    };
    let e = Essai::nouveau("zip64");
    let f = fichier(&s, "moteur.zip", &z, &e.0.join("llama-server.exe"), Some(moteur_zip()));
    let a = telecharger(vec![f], &e.0, 0, None);
    assert_eq!(a.erreur, "");
    assert_eq!(fs::read(e.0.join("ggml.dll")).unwrap(), b"faux ggml");
    assert!(e.0.join("llama-server.exe").exists());
}

#[test]
fn zip_abime_refuse_sans_rien_laisser() {
    let mut z = zip(&[("llama-server.exe", &octets(20_000, 9), true)], false);
    // (le contenu compressé abîmé, le sommaire intact : le CRC-32 ou deflate le voit)
    for b in &mut z[60..200] {
        *b ^= 0x5a;
    }
    let s = {
        let z = Arc::new(z.clone());
        FauxServeur::lancer(move |_, _| Action::fichier(&z))
    };
    let e = Essai::nouveau("zip-abime");
    let f = fichier(&s, "moteur.zip", &z, &e.0.join("llama-server.exe"), Some(moteur_zip()));
    let a = telecharger(vec![f], &e.0, 0, None);
    assert!(a.erreur.starts_with("le fichier « fichier moteur.zip » ne s'ouvre pas"), "{}", a.erreur);
    assert!(!e.0.join("llama-server.exe").exists());
    assert!(!e.0.join("llama-server.exe.extraction").exists());
}

#[test]
fn pas_assez_de_place_dit_avant_de_telecharger() {
    let o = Arc::new(octets(1000, 6));
    let s = {
        let o = o.clone();
        FauxServeur::lancer(move |_, _| Action::fichier(&o))
    };
    let e = Essai::nouveau("place");
    let garder = e.0.join("modele.gguf");
    let a = telecharger(vec![fichier(&s, "modele.gguf", &o, &garder, None)], &e.0, 200 * MO, Some(100 * MO));
    assert_eq!(
        a.erreur,
        "il n'y a pas assez de place sur le disque (il faut environ 201 Mo de libre, il en reste 100 Mo). Fais de la place (la corbeille, les vieux téléchargements), puis réessaie"
    );
    assert!(a.fini && !garder.exists());
    assert!(s.demandes().is_empty(), "rien n'est demandé au serveur");
    // Place inconnue (dossier réseau) : on essaie quand même ; assez de place : aussi
    for place in [None, Some(201 * MO)] {
        let a = telecharger(vec![fichier(&s, "modele.gguf", &o, &garder, None)], &e.0, 200 * MO, place);
        assert_eq!(a.erreur, "");
        fs::remove_file(&garder).unwrap();
    }
}

#[test]
fn place_comptee_sans_ce_qui_est_deja_telecharge() {
    let o = Arc::new(octets(3 * MO as usize, 7));
    let s = {
        let o = o.clone();
        FauxServeur::lancer(move |_, _| Action::fichier(&o))
    };
    let e = Essai::nouveau("place-reprise");
    fs::write(e.0.join("modele.gguf.telechargement"), &o[..2 * MO as usize]).unwrap();
    // (il reste 1 Mo à télécharger : 1 Mo de libre suffit, la marge étant nulle)
    let a = telecharger(vec![fichier(&s, "modele.gguf", &o, &e.0.join("modele.gguf"), None)], &e.0, 0, Some(MO));
    assert_eq!(a.erreur, "");
    assert_eq!(s.demandes()[0].depuis, Some(2 * MO));
}

#[test]
fn une_vraie_coupure_reste_connexion_instable() {
    let ferme = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let e = Essai::nouveau("instable");
    let f = Fichier {
        nom: "fichier modele.gguf".into(),
        url: format!("http://127.0.0.1:{ferme}/modele.gguf"),
        sha256: commun::sha256(b"x"),
        taille: 1,
        extraire: None,
        garder: e.0.join("modele.gguf"),
    };
    let a = telecharger(vec![f], &e.0, 0, None);
    assert_eq!(
        a.erreur,
        "le téléchargement de « fichier modele.gguf » s'est coupé plusieurs fois (connexion instable ?). Réessaie : il reprendra là où il s'est arrêté"
    );
}

#[test]
fn serveur_qui_refuse_compte_comme_une_coupure() {
    let s = FauxServeur::lancer(|_, _| Action::Statut(404));
    let e = Essai::nouveau("404");
    let mut f = fichier(&s, "modele.gguf", b"x", &e.0.join("modele.gguf"), None);
    f.taille = 1;
    let a = telecharger(vec![f], &e.0, 0, None);
    assert!(a.erreur.contains("connexion instable"), "{}", a.erreur);
    assert!(s.demandes().len() >= 4, "réessayé plusieurs fois");
}

#[test]
fn fichier_bloque_par_un_antivirus_pas_connexion_instable() {
    let o = Arc::new(octets(10_000, 8));
    let s = {
        let o = o.clone();
        FauxServeur::lancer(move |_, _| Action::fichier(&o))
    };
    let e = Essai::nouveau("antivirus");
    let tmp = e.0.join("modele.gguf.telechargement");
    fs::write(&tmp, b"").unwrap();
    let verrou = bloquer(&tmp);
    let a = telecharger(vec![fichier(&s, "modele.gguf", &o, &e.0.join("modele.gguf"), None)], &e.0, 0, None);
    drop(verrou);
    assert_eq!(
        a.erreur,
        "Mindcare n'arrive pas à écrire le fichier « fichier modele.gguf » sur le disque (un antivirus le bloque peut-être). Réessaie dans un moment : il reprendra là où il s'est arrêté"
    );
}

#[test]
fn installation_coupee_detectee_a_reparer() {
    let s = FauxServeur::lancer(|_, _| Action::Statut(404));
    let e = Essai::nouveau("coupee");
    let p = paquet(&e.0, &s, &zip(&[("llama-server.exe", b"x", false)], false), b"modele");
    fs::write(&p.programme, b"faux").unwrap();
    fs::write(e.0.join("llama.dll"), b"faux").unwrap(); // (ggml.dll manque)
    fs::write(&p.modele, b"faux").unwrap();
    assert!(!p.moteur_present() && p.incomplet() && !p.installe());
    let manquants = p.manquants();
    assert_eq!(manquants.len(), 1);
    assert!(manquants[0].extraire.is_some(), "le moteur compte comme manquant");
    let i = Installation::nouvelle(p);
    assert!(matches!(i.etat(), Etat::AReparer { taille_mo: 1 }), "{:?}", i.etat());
}

#[test]
fn coupee_pendant_l_extraction_pas_de_programme_sans_ses_dll_puis_reparee() {
    let z = zip(
        &[
            ("llama/llama-server.exe", b"faux llama-server", true),
            ("llama/llama.dll", b"faux llama", true),
            ("llama/ggml.dll", b"faux ggml", true),
            ("llama/autre.txt", b"pas voulu", true),
        ],
        false,
    );
    let modele = octets(5000, 10);
    let s = {
        let (z, m) = (Arc::new(z.clone()), Arc::new(modele.clone()));
        FauxServeur::lancer(move |d, _| Action::fichier(if d.chemin == "moteur.zip" { &z } else { &m }))
    };
    let e = Essai::nouveau("extraction");
    let mut i = Installation::nouvelle(paquet(&e.0, &s, &z, &modele));
    i.pause = Duration::ZERO;
    // ggml.dll (l'ancienne) est bloquée : Mindcare « fermée » au milieu de l'extraction
    let bloquee = e.0.join("ggml.dll");
    fs::write(&bloquee, b"ancienne").unwrap();
    let verrou = bloquer(&bloquee);
    i.lancer(reseau(), &Place(None));
    assert!(i.attendre(Duration::from_secs(30)));
    drop(verrou);
    let Some(Fin::Erreur(m)) = i.fin() else { panic!("une erreur attendue : {:?}", i.avancement()) };
    assert!(m.contains("n'arrive pas à installer « fichier moteur.zip »"), "{m}");
    assert!(!i.paquet.programme.exists(), "llama-server.exe n'est pas là sans ses .dll");
    assert!(!i.paquet.moteur_present());
    assert!(matches!(i.etat(), Etat::Erreur(_)));
    assert_eq!(i.fin(), None, "la fin n'est rendue qu'une fois");
    // « Réessayer » : le zip déjà téléchargé resert, l'installation se finit
    let avant = s.demandes().len();
    i.lancer(reseau(), &Place(None));
    assert!(i.attendre(Duration::from_secs(30)));
    assert_eq!(i.fin(), Some(Fin::Installe));
    assert_eq!(i.etat(), Etat::Pret);
    assert_eq!(fs::read(&bloquee).unwrap(), b"faux ggml");
    let restes: Vec<_> = tous_les_fichiers(&e.0)
        .into_iter()
        .filter(|p| {
            let n = p.to_string_lossy();
            n.ends_with(".extraction") || n.ends_with(".telechargement") || n.ends_with("autre.txt")
        })
        .collect();
    assert!(restes.is_empty(), "{restes:?}");
    assert_eq!(
        s.demandes()[avant..].iter().filter(|d| d.chemin == "moteur.zip").count(),
        0,
        "le zip n'est pas retéléchargé"
    );
}

#[test]
fn annule_quand_mindcare_se_ferme_puis_repris() {
    let o = Arc::new(octets(2 * MO as usize, 11));
    let s = {
        let o = o.clone();
        FauxServeur::lancer(move |_, n| Action::Donner {
            octets: o.clone(),
            reprise: true,
            couper_apres: None,
            lent: (n == 0).then_some(Duration::from_millis(15)),
        })
    };
    let e = Essai::nouveau("annule");
    let garder = e.0.join("modele.gguf");
    let f = fichier(&s, "modele.gguf", &o, &garder, None);
    let mut t = Telechargement::lancer(vec![f.clone()], &e.0, 0, reseau(), &Place(None), Duration::ZERO);
    let limite = Instant::now() + Duration::from_secs(10);
    while t.avancement().recu == 0 && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(t.avancement().recu > 0, "le téléchargement a commencé");
    t.annuler();
    let debut = Instant::now();
    assert!(t.attendre(Duration::from_secs(3)), "arrêté vite");
    assert!(debut.elapsed() < Duration::from_secs(3));
    let a = t.avancement();
    assert!(a.annule && a.fini && a.erreur.is_empty(), "{a:?}");
    assert!(!garder.exists());
    let partiel = fs::metadata(e.0.join("modele.gguf.telechargement")).map(|m| m.len()).unwrap_or(0);
    assert!(partiel > 0 && partiel < o.len() as u64, "le début reste pour la reprise ({partiel})");
    // Mindcare rouverte : il reprend là où il s'était arrêté
    let a = telecharger(vec![f], &e.0, 0, None);
    assert_eq!(a.erreur, "");
    assert_eq!(fs::read(&garder).unwrap(), *o);
    assert_eq!(s.demandes().last().unwrap().depuis, Some(partiel));
}

#[test]
fn lache_veut_dire_annule() {
    let o = Arc::new(octets(MO as usize, 12));
    let s = {
        let o = o.clone();
        FauxServeur::lancer(move |_, _| Action::Donner {
            octets: o.clone(),
            reprise: true,
            couper_apres: None,
            lent: Some(Duration::from_millis(20)),
        })
    };
    let e = Essai::nouveau("lache");
    let garder = e.0.join("modele.gguf");
    let t = Telechargement::lancer(
        vec![fichier(&s, "modele.gguf", &o, &garder, None)],
        &e.0,
        0,
        reseau(),
        &Place(None),
        Duration::ZERO,
    );
    std::thread::sleep(Duration::from_millis(200));
    drop(t);
    std::thread::sleep(Duration::from_secs(2));
    assert!(!garder.exists(), "le téléchargement lâché ne continue pas");
}

#[test]
fn ancien_modele_garde_en_attendant_puis_efface() {
    let modele = octets(40_000, 13);
    let s = {
        let m = Arc::new(modele.clone());
        FauxServeur::lancer(move |_, _| Action::fichier(&m))
    };
    let e = Essai::nouveau("ancien");
    let p = paquet(&e.0, &s, &zip(&[("llama-server.exe", b"x", false)], false), &modele);
    for n in ["llama-server.exe", "llama.dll", "ggml.dll", "mindcare-0.6b-v1-q8_0.gguf", "autre.gguf"] {
        fs::write(e.0.join(n), b"faux").unwrap();
    }
    // (un téléchargement commencé de la nouvelle version n'est pas un modèle)
    fs::write(e.0.join("mindcare-0.6b-v3-q8_0.gguf.telechargement"), &modele[..1000]).unwrap();
    let ancien = e.0.join("mindcare-0.6b-v1-q8_0.gguf");
    assert!(p.installe() && !p.a_jour());
    assert_eq!(p.modele_lance(), Some(ancien.clone()), "l'ancien sert en attendant");
    assert_eq!(p.effacer_anciens(), Vec::<String>::new(), "rien d'effacé tant que le nouveau n'est pas là");
    let mut i = Installation::nouvelle(p);
    i.pause = Duration::ZERO;
    assert_eq!(i.etat(), Etat::AMettreAJour { taille_mo: 1 });
    i.lancer(reseau(), &Place(None));
    assert!(i.attendre(Duration::from_secs(30)));
    assert_eq!(i.fin(), Some(Fin::Installe));
    assert_eq!(i.etat(), Etat::Pret);
    assert!(ancien.exists(), "l'ancien reste jusqu'à ce que Mindcare l'efface (llama-server arrêté)");
    assert_eq!(i.paquet.modele_lance(), Some(i.paquet.modele.clone()));
    assert_eq!(i.paquet.effacer_anciens(), vec!["Ancien modèle effacé : mindcare-0.6b-v1-q8_0.gguf".to_string()]);
    assert!(!ancien.exists() && i.paquet.modele.exists() && e.0.join("autre.gguf").exists());
    assert_eq!(fs::read(&i.paquet.modele).unwrap(), modele);
}

#[test]
fn etats_rien_puis_en_cours_puis_pret() {
    let modele = octets(MO as usize, 14);
    let z = zip(&[("llama-server.exe", b"x", false), ("llama.dll", b"x", false), ("ggml.dll", b"x", false)], false);
    let s = {
        let (z, m) = (Arc::new(z.clone()), Arc::new(modele.clone()));
        FauxServeur::lancer(move |d, _| Action::Donner {
            octets: if d.chemin == "moteur.zip" { z.clone() } else { m.clone() },
            reprise: true,
            couper_apres: None,
            lent: Some(Duration::from_millis(2)),
        })
    };
    let e = Essai::nouveau("etats");
    let mut i = Installation::nouvelle(paquet(&e.0, &s, &z, &modele));
    assert_eq!(i.etat(), Etat::Rien { taille_mo: 2 });
    assert_eq!(i.lancer(reseau(), &Place(None)), Some(1));
    assert_eq!(i.lancer(reseau(), &Place(None)), None, "pas lancé deux fois");
    let mut vus = Vec::new();
    while i.en_cours() {
        if let Etat::EnCours { pourcent, etape } = i.etat() {
            vus.push((pourcent, etape));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(vus.iter().any(|(_, etape)| etape == "fichier mindcare-0.6b-v3-q8_0.gguf"), "{vus:?}");
    assert!(vus.windows(2).all(|w| w[0].0 <= w[1].0), "le pourcentage ne recule pas");
    assert_eq!(i.etat(), Etat::Pret);
    assert_eq!(i.fin(), Some(Fin::Installe));
}

/// Les fichiers, tailles, empreintes et motifs sont ceux du PowerShell (Mindcare\fichiers\modules)
#[test]
fn catalogue_comme_le_powershell() {
    let modules = Path::new(env!("CARGO_MANIFEST_DIR")).join("..\\..\\Mindcare\\fichiers\\modules");
    let lire = |n: &str| {
        fs::read_to_string(modules.join(n)).expect("module PowerShell").trim_start_matches('\u{feff}').to_string()
    };
    let entre = |t: &str, debut: &str, fin: &str| -> Vec<String> {
        t.match_indices(debut)
            .map(|(i, _)| {
                let r = &t[i + debut.len()..];
                // (dans une chaîne PowerShell entre apostrophes, '' est une apostrophe)
                let mut s = String::new();
                let mut c = r.chars().peekable();
                while let Some(x) = c.next() {
                    if fin == "'" && x == '\'' {
                        if c.peek() == Some(&'\'') {
                            c.next();
                            s.push('\'');
                            continue;
                        }
                        break;
                    }
                    if fin != "'" && !x.is_ascii_digit() {
                        break;
                    }
                    s.push(x);
                }
                s
            })
            .collect()
    };
    let quotes = |ligne: &str| -> Vec<String> {
        ligne.split('#').next().unwrap().split('\'').skip(1).step_by(2).map(String::from).collect()
    };
    for (module, p, variable, dll, marge) in [
        ("ia.ps1", Paquet::assistant(Path::new("C:\\donnees")), "$IaMoteurDll =", "$IaMoteurDll", "200MB"),
        ("dictee.ps1", Paquet::dictee(Path::new("C:\\donnees")), "$DicteeDll =", "$DicteeDll", "50MB"),
    ] {
        let t = lire(module);
        let noms = entre(&t, "Nom = '", "'");
        let urls = entre(&t, "Url = '", "'");
        let shas = entre(&t, "Sha256 = '", "'");
        let tailles: Vec<u64> = entre(&t, "Taille = ", "").iter().map(|x| x.parse().unwrap()).collect();
        let motifs = entre(&t, "Extraire = '", "'");
        assert_eq!(p.fichiers.len(), 2, "{module}");
        assert_eq!(noms, p.fichiers.iter().map(|f| f.nom.clone()).collect::<Vec<_>>(), "{module}");
        assert_eq!(urls, p.fichiers.iter().map(|f| f.url.clone()).collect::<Vec<_>>(), "{module}");
        assert_eq!(shas, p.fichiers.iter().map(|f| f.sha256.clone()).collect::<Vec<_>>(), "{module}");
        assert_eq!(tailles, p.fichiers.iter().map(|f| f.taille).collect::<Vec<_>>(), "{module}");
        assert_eq!(motifs, vec![p.fichiers[0].extraire.as_ref().unwrap().motif_ps()], "{module}");
        assert!(p.fichiers[1].extraire.is_none());
        let ligne = t.lines().find(|l| l.starts_with(variable)).expect(dll);
        assert_eq!(quotes(ligne), p.dll, "{module}");
        assert!(t.contains(&format!(
            "Start-Telechargement ({}) ",
            if module == "ia.ps1" { "Get-IaManquants" } else { "Get-DicteeManquants" }
        )));
        assert!(
            t.lines().any(|l| l.contains("Start-Telechargement (Get-") && l.trim_end().ends_with(marge)),
            "{module} : marge {marge}"
        );
        assert_eq!(p.marge, marge.trim_end_matches("MB").parse::<u64>().unwrap() * MO);
        // les fichiers gardés, dans le bon dossier
        let dossier = if module == "ia.ps1" { "$IaDir" } else { "$DicteeDir" };
        for g in [&p.programme, &p.modele] {
            let n = g.file_name().unwrap().to_string_lossy();
            assert!(t.contains(&format!("Join-Path {dossier} '{n}'")), "{module} : {n}");
            assert_eq!(g.parent().unwrap(), p.dossier);
        }
        assert_eq!(p.fichiers[0].garder, p.programme);
        assert_eq!(p.fichiers[1].garder, p.modele);
    }
    let ia = lire("ia.ps1");
    assert!(ia.contains("-Filter 'mindcare-*.gguf'"), "les anciens modèles");
    assert!(ia.contains("$script:IaPause = 3 "), "la pause de 3 secondes");
    assert_eq!(mindcare_telechargement::PAUSE, Duration::from_secs(3));
    assert_eq!(Paquet::assistant(Path::new("C:\\d")).dossier, Path::new("C:\\d\\ia"));
    assert_eq!(Paquet::dictee(Path::new("C:\\d")).dossier, Path::new("C:\\d\\dictee"));
}
