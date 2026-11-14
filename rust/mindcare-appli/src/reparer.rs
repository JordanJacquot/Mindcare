//! « Le faire pour moi » : ce que Mindcare lance pour chaque action de la liste fermée ([`Reparation`], champ
//! « Reparer » des tests du catalogue), et le travail de fond qui les déroule.
//!
//! - [`gestes`] : pour chaque action, les programmes lancés, toujours les mêmes, avec leurs arguments écrits ici et
//!   nulle part ailleurs (aucun texte venu du catalogue ou de la page n'arrive jusqu'à un programme). Les 4 actions
//!   qui demandent les droits administrateur n'ont aucun geste : elles passent par l'Aide de Mindcare ;
//! - [`Executeur`] : ce qui lance vraiment un programme. L'exe livré a le vrai (src\windows\reparer.rs : sans fenêtre,
//!   chemin absolu, limite de temps, code de sortie lu) ; la construction d'essai et les tests ont des faux, qui ne
//!   lancent jamais rien ;
//! - [`AideAdmin`] : l'Aide de Mindcare (MindcareAide.exe, la tâche SYSTEM `\Mindset\Mindcare (aide)` créée par
//!   l'installateur de Mindset), pour les 4 actions qui demandent les droits administrateur ([`action_aide`] : un nom
//!   de la liste fermée de l'Aide, jamais un paramètre). L'exe livré a la vraie (src\windows\aide.rs : état de la tâche,
//!   réveil par `schtasks /Run`, canal vérifié) ; la construction d'essai et les tests ont des fausses. Sans tâche,
//!   pas d'Aide : jamais de demande de Windows (décision de Jordan du 14/11/2026), la page montre les gestes ;
//! - [`EnFond`] : l'action déroulée sur un fil à part, jamais sur celui de la fenêtre ; le programme regarde où elle
//!   en est à chaque pas de sa minuterie.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mindcare_aide_regles::demandes::Demande;
pub use mindcare_moteur::Reparation;

/// La demande faite à l'Aide pour une action qui demande les droits administrateur (aucune pour les autres).
pub fn action_aide(r: Reparation) -> Option<Demande> {
    match r {
        Reparation::ProfilWifi => Some(Demande::WifiOublier),
        Reparation::AdresseIp => Some(Demande::IpRenouveler),
        Reparation::Alimentation => Some(Demande::EnergieParDefaut),
        Reparation::MisesAJour => Some(Demande::MajVider),
        _ => None,
    }
}

/// Le temps laissé à l'Aide pour répondre à une action (après son réveil) : un peu plus que ce qu'elle se donne
/// elle-même (ip.renouveler : 90 s au plus ; maj.vider : deux services de 60 s au plus, trois essais).
pub fn delai_aide(d: Demande) -> Duration {
    Duration::from_secs(match d {
        Demande::WifiOublier => 15,
        Demande::IpRenouveler => 120,
        Demande::EnergieParDefaut => 20,
        Demande::MajVider => 180,
        Demande::Bonjour | Demande::Journal => 5,
    })
}

/// L'Aide de Mindcare, vue de Mindcare.
pub trait AideAdmin: Send + Sync {
    /// Est-elle là ? Sa tâche SYSTEM pour ce compte (`mindcare_aide_regles::tache::etat` : « systeme ») et
    /// MindcareAide.exe à côté de Mindset.exe. Peut prendre un moment (schtasks) : jamais sur le fil de la fenêtre.
    fn presente(&self) -> bool;
    /// Fait faire cette action à l'Aide (réveillée par sa tâche si besoin), en attendant sa réponse au plus `delai` :
    /// son compte rendu, ou la phrase simple de l'échec. Rend aussi les lignes de son journal pas encore recopiées.
    fn faire(&self, d: Demande, delai: Duration) -> (Result<serde_json::Value, String>, Vec<String>);
}

/// Pas d'Aide (tant que le programme n'en a pas branché une) : les 4 actions restent sans bouton.
pub struct SansAide;

impl AideAdmin for SansAide {
    fn presente(&self) -> bool {
        false
    }
    fn faire(&self, _: Demande, _: Duration) -> (Result<serde_json::Value, String>, Vec<String>) {
        (Err("pas d'Aide de Mindcare sur ce PC".into()), Vec::new())
    }
}

/// Un programme lancé : toujours un programme connu, à un emplacement fixe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Programme {
    /// Un programme de Windows, dans son dossier System32 (le chemin à partir de System32)
    Systeme(&'static str),
    /// winget, le gestionnaire de programmes de Windows (`%LOCALAPPDATA%\Microsoft\WindowsApps\winget.exe`)
    Winget,
}

/// Un programme à lancer pour une action, et ce qu'en dit la page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geste {
    pub programme: Programme,
    pub arguments: &'static [&'static str],
    /// Au-delà, le programme est arrêté et l'action ratée
    pub delai: Duration,
    /// Les codes de sortie qui veulent dire « c'est fait »
    pub reussite: &'static [i32],
}

/// winget : installé (0), déjà installé sans rien de plus récent (0x8A15002B, 0x8A150061), installé avec un
/// redémarrage demandé (3010, 1641).
const WINGET_FAIT: &[i32] = &[0, 0x8A15_002B_u32 as i32, 0x8A15_0061_u32 as i32, 3010, 1641];
/// Un programme de Windows : seulement 0.
const FAIT: &[i32] = &[0];

/// Les options de winget : rien à lire ni à accepter à l'écran.
const VCREDIST_X64: &[&str] = &[
    "install",
    "--id",
    "Microsoft.VCRedist.2015+.x64",
    "--exact",
    "--silent",
    "--accept-package-agreements",
    "--accept-source-agreements",
    "--disable-interactivity",
];
const VCREDIST_X86: &[&str] = &[
    "install",
    "--id",
    "Microsoft.VCRedist.2015+.x86",
    "--exact",
    "--silent",
    "--accept-package-agreements",
    "--accept-source-agreements",
    "--disable-interactivity",
];
const JAVA: &[&str] = &[
    "install",
    "--id",
    "EclipseAdoptium.Temurin.21.JRE",
    "--exact",
    "--silent",
    "--accept-package-agreements",
    "--accept-source-agreements",
    "--disable-interactivity",
];

/// La remise à zéro de l'appli Paramètres (`Get-AppxPackage *immersivecontrolpanel* | Reset-AppxPackage`) : Windows
/// ne la propose qu'à PowerShell. Le texte est fixe ; sans l'appli Paramètres, le code 2 (rien n'est fait).
const PARAMETRES: &[&str] = &[
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "$ErrorActionPreference = 'Stop'; $p = @(Get-AppxPackage -Name '*immersivecontrolpanel*'); if (-not $p.Count) { exit 2 }; $p | Reset-AppxPackage; exit 0",
];

/// Les programmes lancés pour cette action, dans l'ordre. Aucun pour une action qui demande les droits
/// administrateur (l'Aide administrateur n'est pas encore là).
pub fn gestes(r: Reparation) -> Vec<Geste> {
    let windows = |exe, arguments, secondes| Geste {
        programme: Programme::Systeme(exe),
        arguments,
        delai: Duration::from_secs(secondes),
        reussite: FAIT,
    };
    let winget = |arguments| Geste {
        programme: Programme::Winget,
        arguments,
        delai: Duration::from_secs(15 * 60),
        reussite: WINGET_FAIT,
    };
    match r {
        Reparation::Dns => vec![windows("ipconfig.exe", &["/flushdns"], 60)],
        Reparation::CacheStore => vec![windows("WSReset.exe", &[], 3 * 60)],
        Reparation::CacheIcones => vec![windows("ie4uinit.exe", &["-show"], 60)],
        Reparation::VisualCpp => vec![winget(VCREDIST_X64), winget(VCREDIST_X86)],
        Reparation::Java => vec![winget(JAVA)],
        Reparation::Parametres => vec![windows(r"WindowsPowerShell\v1.0\powershell.exe", PARAMETRES, 3 * 60)],
        Reparation::ProfilWifi | Reparation::AdresseIp | Reparation::Alimentation | Reparation::MisesAJour => {
            Vec::new()
        }
    }
}

/// Ce qui lance vraiment un programme de la liste fermée : sans fenêtre, et en attendant sa fin au plus
/// `geste.delai`. Rend son code de sortie, ou pourquoi il n'a pas pu tourner jusqu'au bout.
pub trait Executeur: Send + Sync {
    fn lancer(&self, geste: &Geste) -> Result<i32, String>;
}

/// Aucun exécuteur (tant que le programme n'en a pas branché un) : chaque action rate proprement.
pub struct SansExecuteur;

impl Executeur for SansExecuteur {
    fn lancer(&self, _: &Geste) -> Result<i32, String> {
        Err("rien pour lancer les gestes".into())
    }
}

/// Où en est une action déroulée : les gestes faits, les lignes du journal pas encore écrites, et la fin.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Avancement {
    pub faits: usize,
    pub journal: Vec<String>,
    pub fin: Option<Result<(), String>>,
}

fn noter(suivi: &Mutex<Avancement>, f: impl FnOnce(&mut Avancement)) {
    f(&mut suivi.lock().unwrap_or_else(|e| e.into_inner()));
}

/// Déroule l'action, geste après geste (le premier raté arrête tout), ou par l'Aide pour une action qui demande les
/// droits administrateur. Tourne sur le fil de l'appelant.
pub fn derouler(r: Reparation, executeur: &dyn Executeur, aide: &dyn AideAdmin, suivi: &Mutex<Avancement>) {
    if let Some(d) = action_aide(r) {
        let ligne = format!("Le faire pour moi ({}) : par l'Aide de Mindcare ({})", r.id(), d.nom());
        noter(suivi, |a| a.journal.push(ligne.clone()));
        let (resultat, journal_aide) = aide.faire(d, delai_aide(d));
        noter(suivi, |a| {
            a.journal.extend(journal_aide.into_iter().map(|l| format!("Aide : {l}")));
            match &resultat {
                Ok(r) => a.journal.push(format!("{ligne}, fini ({})", court(&r.to_string()))),
                Err(e) => a.journal.push(format!("{ligne}, raté ({e})")),
            }
            a.fin = Some(resultat.map(|_| ()));
        });
        return;
    }
    let liste = gestes(r);
    if liste.is_empty() {
        noter(suivi, |a| a.fin = Some(Err("aucun geste pour cette action".into())));
        return;
    }
    for (k, g) in liste.iter().enumerate() {
        let nom = match g.programme {
            Programme::Systeme(exe) => exe.rsplit('\\').next().unwrap_or(exe),
            Programme::Winget => "winget.exe",
        };
        let ligne = format!("Le faire pour moi ({}) : {nom}", r.id());
        match executeur.lancer(g) {
            Ok(code) if g.reussite.contains(&code) => noter(suivi, |a| {
                a.faits = k + 1;
                a.journal.push(format!("{ligne}, fini (code {code})"));
            }),
            Ok(code) => {
                noter(suivi, |a| {
                    a.journal.push(format!("{ligne}, raté (code {code}, 0x{:08X})", code as u32));
                    a.fin = Some(Err(format!("{nom} : code {code}")));
                });
                return;
            }
            Err(e) => {
                noter(suivi, |a| {
                    a.journal.push(format!("{ligne}, raté ({e})"));
                    a.fin = Some(Err(format!("{nom} : {e}")));
                });
                return;
            }
        }
    }
    noter(suivi, |a| a.fin = Some(Ok(())));
}

/// Un compte rendu de l'Aide, coupé pour le journal (300 caractères au plus).
fn court(t: &str) -> String {
    let mut c: String = t.chars().take(300).collect();
    if c.len() < t.len() {
        c.push_str("...");
    }
    c
}

/// Ce qui a changé depuis le dernier regard sur une action déroulée.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Nouvelles {
    /// Les gestes faits, s'il y en a de nouveaux
    pub faits: Option<usize>,
    pub journal: Vec<String>,
    pub fin: Option<Result<(), String>>,
}

/// Une action déroulée sur un fil à part.
pub struct EnFond {
    pub action: Reparation,
    suivi: Arc<Mutex<Avancement>>,
    vus: usize,
}

impl EnFond {
    /// Lance l'action sur un fil à part. Un fil impossible à créer : l'action est ratée tout de suite.
    pub fn lancer(action: Reparation, executeur: Arc<dyn Executeur>, aide: Arc<dyn AideAdmin>) -> EnFond {
        let suivi = Arc::new(Mutex::new(Avancement::default()));
        let pour_fil = Arc::clone(&suivi);
        let fil = std::thread::Builder::new()
            .name("mindcare-reparer".into())
            .spawn(move || derouler(action, executeur.as_ref(), aide.as_ref(), &pour_fil));
        if let Err(e) = fil {
            noter(&suivi, |a| a.fin = Some(Err(format!("travail de fond impossible ({e})"))));
        }
        EnFond { action, suivi, vus: 0 }
    }

    /// Ce qui a changé depuis le dernier regard (la fin n'est rendue qu'une fois).
    pub fn nouvelles(&mut self) -> Nouvelles {
        let mut a = self.suivi.lock().unwrap_or_else(|e| e.into_inner());
        let faits = (a.faits != self.vus).then_some(a.faits);
        self.vus = a.faits;
        Nouvelles { faits, journal: std::mem::take(&mut a.journal), fin: a.fin.take() }
    }
}

/// Pour les tests : le vrai catalogue, où chaque panne qui pose un des tests de « Le faire pour moi » (ou test-sfc,
/// test-gaming-services) commence par la question « Mindcare est-il ouvert sur le PC qui a le problème ? » (Ref
/// « cepc »), pour essayer la réponse « Non, sur un autre appareil » (aucune de ces pannes ne la pose aujourd'hui).
#[cfg(test)]
pub(crate) fn catalogue_avec_cepc() -> mindcare_moteur::Catalogue {
    use serde_json::{Value, json};
    let fichier = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Mindcare/fichiers/enquetes.json");
    let texte = std::fs::read_to_string(fichier).expect("enquetes.json");
    let mut j: Value = serde_json::from_str(texte.trim_start_matches('\u{feff}')).expect("enquetes.json lisible");
    let tests: Vec<String> = j["Tests"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(nom, t)| t.get("Reparer").is_some() || mindcare_moteur::reparer::JAMAIS.contains(&nom.as_str()))
        .map(|(nom, _)| nom.clone())
        .collect();
    for p in j["Pannes"].as_array_mut().unwrap() {
        let etapes = p["Etapes"].as_array_mut().unwrap();
        let cite = |r: &str| etapes.iter().any(|e| e["Ref"] == r);
        if tests.iter().any(|t| cite(t)) && !cite("cepc") {
            etapes.insert(0, json!({ "Ref": "cepc" }));
        }
    }
    mindcare_moteur::Catalogue::lire(&j.to_string()).expect("catalogue d'essai")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un faux exécuteur : il note ce qu'on lui demande et rend les codes donnés (jamais rien de lancé).
    struct Faux {
        codes: Mutex<Vec<Result<i32, String>>>,
        lances: Mutex<Vec<Geste>>,
    }

    impl Executeur for Faux {
        fn lancer(&self, g: &Geste) -> Result<i32, String> {
            self.lances.lock().unwrap().push(g.clone());
            self.codes.lock().unwrap().remove(0)
        }
    }

    fn faux(codes: Vec<Result<i32, String>>) -> Faux {
        Faux { codes: Mutex::new(codes), lances: Mutex::new(Vec::new()) }
    }

    #[test]
    fn les_programmes_de_chaque_action() {
        let g = gestes(Reparation::Dns);
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].programme, g[0].arguments), (Programme::Systeme("ipconfig.exe"), &["/flushdns"][..]));
        assert_eq!(gestes(Reparation::CacheStore)[0].programme, Programme::Systeme("WSReset.exe"));
        assert_eq!(gestes(Reparation::CacheIcones)[0].arguments, ["-show"]);
        let vc = gestes(Reparation::VisualCpp);
        assert_eq!(vc.len(), 2);
        assert!(vc[0].arguments.contains(&"Microsoft.VCRedist.2015+.x64"));
        assert!(vc[1].arguments.contains(&"Microsoft.VCRedist.2015+.x86"));
        assert!(gestes(Reparation::Java)[0].arguments.contains(&"EclipseAdoptium.Temurin.21.JRE"));
        for g in vc.iter().chain(&gestes(Reparation::Java)) {
            assert_eq!(g.programme, Programme::Winget);
            for o in
                ["--silent", "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity"]
            {
                assert!(g.arguments.contains(&o), "{o}");
            }
        }
        let p = gestes(Reparation::Parametres);
        assert_eq!(p[0].programme, Programme::Systeme(r"WindowsPowerShell\v1.0\powershell.exe"));
        assert!(p[0].arguments.contains(&"-NonInteractive") && p[0].arguments.contains(&"-NoProfile"));
        assert!(p[0].arguments[3].contains("Reset-AppxPackage") && !p[0].arguments[3].contains("Remove-"));
        for r in Reparation::TOUTES {
            assert_eq!(gestes(r).is_empty(), r.admin(), "{r:?}");
            for g in gestes(r) {
                assert!(g.delai >= Duration::from_secs(30) && g.delai <= Duration::from_secs(15 * 60));
                assert!(g.reussite.contains(&0));
            }
        }
    }

    #[test]
    fn deroule_jusqu_au_premier_rate() {
        let f = faux(vec![Ok(0), Ok(0)]);
        let s = Mutex::new(Avancement::default());
        derouler(Reparation::VisualCpp, &f, &SansAide, &s);
        let a = s.lock().unwrap().clone();
        assert_eq!((a.faits, a.fin), (2, Some(Ok(()))));
        assert_eq!(f.lances.lock().unwrap().len(), 2);
        assert!(a.journal[0].starts_with("Le faire pour moi (visual-cpp) : winget.exe, fini (code 0)"));

        // Déjà installé : c'est fait aussi
        let f = faux(vec![Ok(0x8A15_002B_u32 as i32)]);
        let s = Mutex::new(Avancement::default());
        derouler(Reparation::Java, &f, &SansAide, &s);
        assert_eq!(s.lock().unwrap().fin, Some(Ok(())));

        // Le premier raté arrête tout : le second n'est jamais lancé
        let f = faux(vec![Ok(1), Ok(0)]);
        let s = Mutex::new(Avancement::default());
        derouler(Reparation::VisualCpp, &f, &SansAide, &s);
        let a = s.lock().unwrap().clone();
        assert_eq!(a.faits, 0);
        assert_eq!(a.fin, Some(Err("winget.exe : code 1".into())));
        assert_eq!(f.lances.lock().unwrap().len(), 1);

        // winget absent : un échec propre
        let f = faux(vec![Err("winget.exe introuvable".into())]);
        let s = Mutex::new(Avancement::default());
        derouler(Reparation::Java, &f, &SansAide, &s);
        assert_eq!(s.lock().unwrap().fin, Some(Err("winget.exe : winget.exe introuvable".into())));

        // Une action qui demande les droits administrateur, sans Aide : rien de lancé, raté proprement
        let f = faux(vec![]);
        let s = Mutex::new(Avancement::default());
        derouler(Reparation::MisesAJour, &f, &SansAide, &s);
        assert_eq!(s.lock().unwrap().fin, Some(Err("pas d'Aide de Mindcare sur ce PC".into())));
        assert!(f.lances.lock().unwrap().is_empty());
    }

    #[test]
    fn en_fond_rend_la_fin_une_seule_fois() {
        let mut f = EnFond::lancer(Reparation::Dns, Arc::new(faux(vec![Ok(0)])), Arc::new(SansAide));
        let debut = std::time::Instant::now();
        let mut vu = Nouvelles::default();
        while vu.fin.is_none() && debut.elapsed() < Duration::from_secs(10) {
            let n = f.nouvelles();
            vu.faits = vu.faits.or(n.faits);
            vu.journal.extend(n.journal);
            vu.fin = n.fin;
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(vu.fin, Some(Ok(())));
        assert_eq!(vu.faits, Some(1));
        assert_eq!(vu.journal.len(), 1);
        assert_eq!(f.nouvelles(), Nouvelles::default());
        // Sans exécuteur : raté proprement
        let mut f = EnFond::lancer(Reparation::Dns, Arc::new(SansExecuteur), Arc::new(SansAide));
        let debut = std::time::Instant::now();
        let mut fin = None;
        while fin.is_none() && debut.elapsed() < Duration::from_secs(10) {
            fin = f.nouvelles().fin;
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(fin, Some(Err("ipconfig.exe : rien pour lancer les gestes".into())));
    }

    /// Une fausse Aide : elle note les demandes et rend la réponse donnée, avec deux lignes de son journal (jamais
    /// rien de lancé)
    struct FausseAide {
        demandes: Mutex<Vec<(Demande, Duration)>>,
        reponse: Result<serde_json::Value, String>,
    }

    impl AideAdmin for FausseAide {
        fn presente(&self) -> bool {
            true
        }
        fn faire(&self, d: Demande, delai: Duration) -> (Result<serde_json::Value, String>, Vec<String>) {
            self.demandes.lock().unwrap().push((d, delai));
            (self.reponse.clone(), vec![format!("{} : commencé", d.nom()), format!("{} : fini", d.nom())])
        }
    }

    #[test]
    fn les_actions_admin_passent_par_l_aide() {
        let attendu = [
            (Reparation::ProfilWifi, Demande::WifiOublier, 15),
            (Reparation::AdresseIp, Demande::IpRenouveler, 120),
            (Reparation::Alimentation, Demande::EnergieParDefaut, 20),
            (Reparation::MisesAJour, Demande::MajVider, 180),
        ];
        for r in Reparation::TOUTES {
            assert_eq!(action_aide(r).is_some(), r.admin(), "{r:?}");
        }
        for (r, d, secondes) in attendu {
            assert_eq!(action_aide(r), Some(d));
            let aide = FausseAide { demandes: Mutex::new(Vec::new()), reponse: Ok(serde_json::json!({ "code": 0 })) };
            let f = faux(vec![]);
            let s = Mutex::new(Avancement::default());
            derouler(r, &f, &aide, &s);
            let a = s.lock().unwrap().clone();
            assert_eq!(a.fin, Some(Ok(())), "{r:?}");
            assert_eq!(*aide.demandes.lock().unwrap(), [(d, Duration::from_secs(secondes))]);
            assert!(f.lances.lock().unwrap().is_empty(), "{r:?} : un programme lancé par Mindcare lui-même");
            // Le journal de l'Aide recopié dans celui de Mindcare
            let ligne = format!("Le faire pour moi ({}) : par l'Aide de Mindcare ({})", r.id(), d.nom());
            assert_eq!(a.journal[0], ligne);
            assert_eq!(a.journal[1], format!("Aide : {} : commencé", d.nom()));
            assert_eq!(a.journal[2], format!("Aide : {} : fini", d.nom()));
            assert_eq!(a.journal[3], format!("{ligne}, fini ({{\"code\":0}})"));
        }
        // Ratée : la phrase de l'Aide, telle quelle
        let aide = FausseAide {
            demandes: Mutex::new(Vec::new()),
            reponse: Err("Ce PC n'a pas de Wi-Fi : il n'y a rien à oublier.".into()),
        };
        let s = Mutex::new(Avancement::default());
        derouler(Reparation::ProfilWifi, &faux(vec![]), &aide, &s);
        let a = s.lock().unwrap().clone();
        assert_eq!(a.fin, Some(Err("Ce PC n'a pas de Wi-Fi : il n'y a rien à oublier.".into())));
        assert!(a.journal.last().unwrap().ends_with(", raté (Ce PC n'a pas de Wi-Fi : il n'y a rien à oublier.)"));
        // Les actions sans droits administrateur ne passent jamais par l'Aide
        let aide = FausseAide { demandes: Mutex::new(Vec::new()), reponse: Ok(serde_json::Value::Null) };
        let s = Mutex::new(Avancement::default());
        derouler(Reparation::Dns, &faux(vec![Ok(0)]), &aide, &s);
        assert!(aide.demandes.lock().unwrap().is_empty());
        assert!(!SansAide.presente());
    }
}
