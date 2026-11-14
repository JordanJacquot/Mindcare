//! Les règles de l'Aide de Mindcare, de l'extérieur : qui elle sert, son canal, qui peut lui parler, qui Mindcare
//! écoute, l'état de sa tâche et sa liste fermée. Rien n'est touché sur le PC.

use mindcare_aide_regles::demandes::{Demande, verifier};
use mindcare_aide_regles::{canal, client, serveur, sid, tache};

const ELODIE: &str = "S-1-5-21-1111111111-2222222222-3333333333-1001";
const BRUNO: &str = "S-1-5-21-1111111111-2222222222-3333333333-1002";
const DOSSIER: &str = "C:\\Mindset\\Programmes\\Mindcare";

#[test]
fn l_aide_ne_sert_que_le_compte_inscrit_dans_sa_tache() {
    let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let recu = sid::lire_arguments(&args(&["-Sid", ELODIE]));
    assert_eq!(sid::compte_servi(sid::SYSTEME, recu.as_deref()), Ok(ELODIE.to_string()));
    // En SYSTEM sans -Sid, ou avec autre chose qu'un compte d'utilisateur : elle ne démarre pas
    for a in [&[][..], &["-Sid", "S-1-5-18"], &["-Sid", "S-1-5-32-544"], &["-Sid", "S-1-5-21-1-2-3-4 -Sid"]] {
        let recu = sid::lire_arguments(&args(a));
        assert!(sid::compte_servi(sid::SYSTEME, recu.as_deref()).is_err(), "{a:?}");
    }
    // Sous un compte : jamais pour un autre
    assert!(sid::compte_servi(BRUNO, Some(ELODIE)).is_err());
}

#[test]
fn le_canal_et_le_verrou_sont_ceux_de_mindcare() {
    let s = canal::suffixe(DOSSIER).unwrap();
    assert_eq!(canal::nom_canal(ELODIE, &s), format!("Mindcare-Aide-{ELODIE}-{s}"));
    assert_eq!(canal::nom_verrou(ELODIE, &s), format!("Global\\Mindcare-Aide-{ELODIE}-{s}"));
    assert_ne!(canal::nom_canal(ELODIE, &s), canal::nom_canal(BRUNO, &s));
    // (le même calcul que celui de Nevermind, avec un autre début : SHA-256 de « c:\mindset\programmes\mindcare »)
    assert_eq!(canal::suffixe("C:\\Mindset\\Programmes\\Mindcare\\").unwrap(), s);
}

#[test]
fn seul_mindset_exe_du_dossier_de_mindcare_peut_parler_a_l_aide() {
    assert!(client::accepte("C:\\Mindset\\Programmes\\Mindcare\\Mindset.exe", DOSSIER));
    assert!(client::accepte("c:\\mindset\\programmes\\mindcare\\MINDSET.EXE", DOSSIER));
    for etranger in [
        "C:\\Mindset\\Programmes\\Mindcare\\Mindcare.exe",
        "C:\\Mindset\\Programmes\\Mindcare\\MindcareAide.exe",
        "C:\\Mindset\\Programmes\\Nevermind\\Mindset.exe",
        "C:\\Mindset\\Programmes\\Mindset\\Mindset.exe",
        "C:\\Users\\Elodie\\Downloads\\Mindset.exe",
        "C:\\Mindset\\Programmes\\Mindcare\\sous\\Mindset.exe",
        "C:\\Mindset\\Programmes\\Mindcare\\x\\..\\Mindset.exe",
        "\\\\?\\C:\\Mindset\\Programmes\\Mindcare\\Mindset.exe",
        "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        "",
    ] {
        assert!(!client::accepte(etranger, DOSSIER), "{etranger}");
    }
    assert!(!client::accepte("C:\\x\\Mindset.exe", ""));
}

#[test]
fn mindcare_ne_parle_qu_a_l_aide() {
    let exe = "C:\\Mindset\\Programmes\\Mindcare\\MindcareAide.exe";
    assert!(serveur::accepte("", "S-1-5-18", exe));
    assert!(serveur::accepte("", "S-1-5-32-544", exe));
    assert!(serveur::accepte(exe, ELODIE, exe));
    // Un canal créé par un programme sans droits, qui n'est pas l'Aide : refusé
    assert!(!serveur::accepte("C:\\Users\\Elodie\\faux.exe", ELODIE, exe));
    assert!(!serveur::accepte("", ELODIE, exe));
    assert!(!serveur::accepte("C:\\Mindset\\Programmes\\Nevermind\\NevermindAide.exe", ELODIE, exe));
}

fn xml(compte: &str, commande: &str, arguments: &str, active: bool) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\r\n  <Principals>\r\n    <Principal id=\"Author\">\r\n      <UserId>{compte}</UserId>\r\n      <RunLevel>HighestAvailable</RunLevel>\r\n    </Principal>\r\n  </Principals>\r\n  <Settings>\r\n    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>\r\n    <Enabled>{active}</Enabled>\r\n  </Settings>\r\n  <Actions Context=\"Author\">\r\n    <Exec>\r\n      <Command>{commande}</Command>\r\n      <Arguments>{arguments}</Arguments>\r\n    </Exec>\r\n  </Actions>\r\n</Task>"
    )
}

#[test]
fn l_etat_de_la_tache() {
    let exe = format!("{DOSSIER}\\{}", tache::EXE);
    let commande = format!("\"{exe}\"");
    let rien = |_: &str| None;
    let systeme = |n: &str| (n == "SYSTEM").then(|| "S-1-5-18".to_string());
    let sid = format!("-Sid {ELODIE}");
    assert_eq!(tache::etat(&xml("S-1-5-18", &commande, &sid, true), &exe, ELODIE, &rien), "systeme");
    assert_eq!(tache::etat(&xml("SYSTEM", &commande, &sid, true), &exe, ELODIE, &systeme), "systeme");
    assert_eq!(tache::etat("", &exe, ELODIE, &rien), "");
    for (x, quoi) in [
        (xml("S-1-5-18", &commande, &sid, false), "désactivée"),
        (xml(ELODIE, &commande, &sid, true), "sous le compte"),
        (xml("S-1-5-18", &commande, &format!("-Sid {BRUNO}"), true), "pour un autre compte"),
        (xml("S-1-5-18", "\"C:\\Users\\Elodie\\MindcareAide.exe\"", &sid, true), "un autre programme"),
        (xml("S-1-5-18", &format!("\"{DOSSIER}\\Mindset.exe\""), &sid, true), "pas l'Aide"),
        (xml("S-1-5-18", &commande, &format!("{sid} -Autre"), true), "arguments en plus"),
        ("pas du xml".to_string(), "illisible"),
    ] {
        assert_eq!(tache::etat(&x, &exe, ELODIE, &systeme), "autre", "{quoi}");
    }
    assert_eq!(tache::TACHE, "\\Mindset\\Mindcare (aide)");
}

#[test]
fn liste_fermee_sans_parametre() {
    let noms: Vec<&str> = Demande::TOUTES.iter().map(|d| d.nom()).collect();
    assert_eq!(
        noms,
        ["aide.bonjour", "aide.journal", "wifi.oublier", "ip.renouveler", "energie.par-defaut", "maj.vider"]
    );
    assert_eq!(verifier(r#"{"a":"maj.vider"}"#), Ok(Demande::MajVider));
    assert!(verifier(r#"{"a":"maj.vider","p":{"Dossier":"C:\\Users"}}"#).is_err());
    assert!(verifier(r#"{"a":"service.regler","p":{}}"#).is_err());
    // Les actions de l'Aide de Nevermind ne sont jamais acceptées par celle de Mindcare
    for a in ["reg.ecrire", "dns.regler", "pare-feu.bloquer", "processus.fermer", "carte.relancer", "lecture.pc"] {
        assert!(verifier(&format!("{{\"a\":\"{a}\"}}")).is_err(), "{a}");
    }
}
