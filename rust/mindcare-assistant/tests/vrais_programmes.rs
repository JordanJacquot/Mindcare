//! Les essais avec de vrais programmes, par le vrai lanceur (sans fenêtre, Job Object, clé de BCryptGenRandom).
//! Le faux llama-server est CE programme de test, copié sous le nom llama-server.exe dans un dossier d'essai (jamais
//! le llama-server de Jordan) : lancé avec les arguments de llama-server, il joue le faux serveur (ou plante, selon
//! le fichier faux-mode.txt posé à côté). Lancé avec `--role-mindcare`, il joue une fausse Mindcare, que l'essai tue
//! pour voir llama-server s'arrêter avec elle. Chaque programme lancé l'est avec CREATE_NO_WINDOW, et s'arrête tout
//! seul au bout de 2 minutes au plus. Le test ne tue que ses propres processus.

mod commun;

#[cfg(windows)]
mod essais {
    use std::io::{BufRead, BufReader, Write};
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use crate::commun::{Comportement, Essai, FauxServeur, Journal, attendre, installer, repond};
    use mindcare_assistant::windows::{LanceurWindows, SANS_FENETRE, cle_aleatoire};
    use mindcare_assistant::{Assistant, Delais, Etat, Horloge, HorlogeSimulee, Question, Reglages};
    use mindcare_telechargement::Paquet;

    const SECURITE: Duration = Duration::from_secs(120);

    /// Le rôle de ce programme : faux llama-server, fausse Mindcare, ou les essais
    pub fn main() -> i32 {
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--port") {
            return faux_llama_server(&args);
        }
        if args.get(1).map(String::as_str) == Some("--role-mindcare") {
            return fausse_mindcare(Path::new(&args[2]));
        }
        if args.iter().any(|a| a == "--list") {
            return 0;
        }
        essais()
    }

    fn faux_llama_server(args: &[String]) -> i32 {
        let exe = std::env::current_exe().unwrap();
        let mode = std::fs::read_to_string(exe.with_file_name("faux-mode.txt")).unwrap_or_default();
        let cle = std::env::var("LLAMA_API_KEY").ok();
        if mode.trim() == "plante" {
            // (comme New-FauxAssistant -Plante de tester-assistant.ps1)
            for l in crate::commun::lignes_plante(cle.as_deref(), &args[1..]) {
                eprintln!("{l}");
            }
            return 3;
        }
        let i = args.iter().position(|a| a == "--port").unwrap();
        let port: u16 = args[i + 1].parse().unwrap();
        let c = Comportement { cle, ..crate::commun::comportement_bruit() };
        let Ok(_serveur) = FauxServeur::demarrer(port, c) else {
            eprintln!("couldn't bind HTTP server socket, hostname: 127.0.0.1, port: {port}");
            return 1;
        };
        println!("faux llama-server prêt sur le port {port}");
        std::thread::sleep(SECURITE);
        0
    }

    /// Une fausse Mindcare : lance l'assistant (le vrai lanceur), dit son port, puis attend d'être tuée
    fn fausse_mindcare(donnees: &Path) -> i32 {
        let a = Assistant::nouveau(
            Reglages { paquet: Paquet::assistant(donnees), delais: Delais::default() },
            Arc::new(LanceurWindows),
            Arc::new(mindcare_assistant::HorlogeVraie),
            Arc::new(|_: &str| {}),
        );
        a.demarrer();
        if !attendre(Duration::from_secs(30), || a.etat() == Etat::Pret) {
            println!("PAS PRET");
            return 1;
        }
        let (port, _) = a.port_et_cle();
        println!("PRET {port}");
        let _ = std::io::stdout().flush();
        std::thread::sleep(SECURITE);
        0
    }

    /// Un faux assistant dont le llama-server.exe est ce programme de test
    fn installer_moi(essai: &Essai, mode: &str) -> Paquet {
        let moi = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let p = installer(&essai.0, &moi);
        std::fs::write(p.dossier.join("faux-mode.txt"), mode).unwrap();
        p
    }

    fn assistant(paquet: Paquet, horloge: Arc<HorlogeSimulee>, journal: &Journal) -> Assistant {
        let d = Delais { sonde: Duration::from_millis(50), ..Delais::default() };
        Assistant::nouveau(
            Reglages { paquet, delais: d },
            Arc::new(LanceurWindows),
            horloge as Arc<dyn Horloge>,
            journal.fonction(),
        )
    }

    fn essai_plante() -> Result<(), String> {
        let e = Essai::nouveau("vrai-plante");
        let horloge = Arc::new(HorlogeSimulee::nouvelle());
        let j = Journal::default();
        let a = assistant(installer_moi(&e, "plante"), horloge.clone(), &j);
        a.demarrer();
        let (_, cle) = a.port_et_cle();
        if !attendre(Duration::from_secs(20), || !a.lance()) {
            return Err("le faux llama-server qui plante tourne encore".into());
        }
        for _ in 0..5 {
            a.demarrer();
        }
        let t = j.texte();
        let verifier = |ok: bool, quoi: &str| if ok { Ok(()) } else { Err(format!("{quoi} ; journal : {t}")) };
        verifier(j.compte("Assistant lancé") == 1, "relancé à chaque lettre")?;
        verifier(t.contains("Mindset.exe)"), "pas lancé depuis sa copie Mindset.exe")?;
        verifier(j.compte("Assistant arrêté tout seul (code 3)") == 1, "son code pas au journal une seule fois")?;
        verifier(t.contains("ligne 23") && !t.contains("ligne 4 |"), "pas ses 20 dernières lignes")?;
        verifier(cle.len() == 64 && cle.chars().all(|c| c.is_ascii_hexdigit()), "clé de 64 chiffres hexadécimaux")?;
        verifier(t.contains(&format!("cle={cle}")), "la clé n'est pas arrivée par l'environnement")?;
        let args = t.split("arguments=").nth(1).unwrap_or_default().lines().next().unwrap_or_default().to_string();
        verifier(args.starts_with("-m ") && !args.contains(&cle), "la clé est dans les arguments")?;
        let empreintes = a.empreintes_calculees();
        horloge.avancer(Duration::from_secs(61));
        a.demarrer();
        let (_, cle2) = a.port_et_cle();
        attendre(Duration::from_secs(20), || !a.lance());
        verifier(j.compte("Assistant lancé") == 2, "pas relancé une minute plus tard")?;
        verifier(a.empreintes_calculees() == empreintes, "une nouvelle empreinte")?;
        verifier(cle2 != cle, "la même clé au second lancement")
    }

    fn essai_question_et_repos() -> Result<(), String> {
        let e = Essai::nouveau("vrai-question");
        let horloge = Arc::new(HorlogeSimulee::nouvelle());
        let j = Journal::default();
        let a = assistant(installer_moi(&e, "normal"), horloge.clone(), &j);
        let Question::EnCours(t) = a.poser("mon pc fait du bruit", &["bruit".to_string()]) else {
            return Err(format!("pas de question ; journal : {}", j.texte()));
        };
        let r = t.attendre();
        if r.panne != "bruit" || !r.erreur.is_empty() {
            return Err(format!("réponse {r:?} ; journal : {}", j.texte()));
        }
        let (port, _) = a.port_et_cle();
        if !a.arreter_si_inutile(horloge.maintenant() + Duration::from_secs(5 * 60 + 1)) {
            return Err("pas arrêté après 5 minutes".into());
        }
        if !attendre(Duration::from_secs(5), || !repond(port)) {
            return Err("arrêté, mais son port répond encore".into());
        }
        // à la fermeture de Mindcare
        a.demarrer();
        if !attendre(Duration::from_secs(20), || a.etat() == Etat::Pret) {
            return Err(format!("pas relancé ; journal : {}", j.texte()));
        }
        let (port, _) = a.port_et_cle();
        drop(a);
        if !attendre(Duration::from_secs(5), || !repond(port)) {
            return Err("Mindcare fermée, son port répond encore".into());
        }
        Ok(())
    }

    fn essai_pas_un_programme() -> Result<(), String> {
        let e = Essai::nouveau("vrai-pas-programme");
        let j = Journal::default();
        let a = assistant(installer(&e.0, b"faux"), Arc::new(HorlogeSimulee::nouvelle()), &j);
        let q = a.poser("mon pc fait du bruit", &["bruit".to_string()]);
        let t = j.texte();
        if !matches!(q, Question::NeDemarrePas { .. }) || !t.contains("Assistant : lancement impossible (") {
            return Err(format!("journal : {t}"));
        }
        Ok(())
    }

    fn essai_mindcare_tuee() -> Result<(), String> {
        let e = Essai::nouveau("vrai-tuee");
        installer_moi(&e, "normal");
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.arg("--role-mindcare").arg(&e.0).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        c.creation_flags(SANS_FENETRE);
        let mut mindcare = c.spawn().map_err(|x| x.to_string())?;
        let sortie = mindcare.stdout.take().unwrap();
        let (envoi, recu) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut l = String::new();
            let _ = BufReader::new(sortie).read_line(&mut l);
            let _ = envoi.send(l);
        });
        let ligne = recu.recv_timeout(Duration::from_secs(40)).unwrap_or_default();
        let port: Option<u16> = ligne.trim().strip_prefix("PRET ").and_then(|p| p.parse().ok());
        let Some(port) = port else {
            let _ = mindcare.kill();
            return Err(format!("la fausse Mindcare n'a pas lancé l'assistant (« {} »)", ligne.trim()));
        };
        if !repond(port) {
            let _ = mindcare.kill();
            return Err("l'assistant ne répond pas avant".into());
        }
        // Mindcare tuée (TerminateProcess, comme Mindset avant une mise à jour) : llama-server s'arrête avec elle
        mindcare.kill().map_err(|x| x.to_string())?;
        let _ = mindcare.wait();
        let debut = Instant::now();
        if !attendre(Duration::from_secs(10), || !repond(port)) {
            return Err("Mindcare tuée, llama-server répond encore".into());
        }
        println!("       (llama-server arrêté {} ms après Mindcare)", debut.elapsed().as_millis());
        Ok(())
    }

    fn essai_cles() -> Result<(), String> {
        let a = cle_aleatoire()?;
        let b = cle_aleatoire()?;
        if a.len() != 64 || a == b {
            return Err(format!("{a} / {b}"));
        }
        Ok(())
    }

    fn essais() -> i32 {
        type UnEssai = fn() -> Result<(), String>;
        let liste: [(&str, UnEssai); 5] = [
            ("vrai programme : deux clés aléatoires différentes, 64 caractères", essai_cles),
            ("vrai programme : il plante, pas relancé à chaque lettre, sa clé par l'environnement", essai_plante),
            ("vrai programme : la question, arrêté après 5 minutes, et à la fermeture", essai_question_et_repos),
            ("vrai programme : pas un programme, lancement impossible au journal", essai_pas_un_programme),
            ("vrai programme : Mindcare tuée, llama-server arrêté avec elle (Job Object)", essai_mindcare_tuee),
        ];
        let mut echecs = 0;
        for (nom, f) in liste {
            match f() {
                Ok(()) => println!("OK     {nom}"),
                Err(e) => {
                    echecs += 1;
                    println!("ÉCHEC  {nom} : {e}");
                }
            }
        }
        if echecs == 0 { 0 } else { 1 }
    }
}

fn main() {
    #[cfg(windows)]
    std::process::exit(essais::main());
}
