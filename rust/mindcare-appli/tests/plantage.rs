//! Une panique sur le fil de la fenêtre (audit Rust, petit de Mindcare) : tao l'attrape dans un rappel de la fenêtre et
//! la relance à la sortie de sa boucle (`panic::resume_unwind`, tao 0.37.2 event_loop.rs:268). La boucle protégée la
//! note au journal et rend la main : `lancer()` appelle ensuite `finir()` dans tous les cas (llama-server, whisper et
//! le micro arrêtés). Sans fenêtre : la boucle est remplacée par ce que tao fait d'une panique.

use std::path::PathBuf;

use mindcare_appli::programme::boucle_protegee;
use mindcare_donnees::{Journal, Moment, horloge_figee};

fn journal(nom: &str) -> (Journal, PathBuf) {
    let dossier = std::env::temp_dir().join(format!("mindcare-plantage-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    std::fs::create_dir_all(&dossier).unwrap();
    let fichier = dossier.join("journal.txt");
    (Journal::ouvrir(&fichier, horloge_figee(Moment::lire("2026-11-08T03:00:00").unwrap())), dossier)
}

fn texte(dossier: &std::path::Path) -> String {
    std::fs::read_to_string(dossier.join("journal.txt")).unwrap_or_default()
}

#[test]
fn panique_de_la_fenetre_notee_et_la_fin_propre_suit() {
    let (j, dossier) = journal("tao");
    // (ce que fait tao : la panique d'un rappel, gardée, puis relancée à la sortie de la boucle ; arriver à la ligne
    // d'après, c'est ce qui permet à `lancer()` d'appeler `finir()`)
    boucle_protegee(&j, || {
        let gardee = std::panic::catch_unwind(|| panic!("index hors limites")).unwrap_err();
        std::panic::resume_unwind(gardee);
    });
    let t = texte(&dossier);
    assert!(t.contains("Fermeture : plantage (index hors limites)"), "{t}");
    // Une panique avec un texte construit (String)
    let n = 3;
    boucle_protegee(&j, || panic!("page {n} illisible"));
    assert!(texte(&dossier).contains("Fermeture : plantage (page 3 illisible)"));
    // Une boucle qui finit normalement : rien au journal
    let (j, dossier2) = journal("normal");
    boucle_protegee(&j, || {});
    assert!(!texte(&dossier2).contains("plantage"));
    let _ = std::fs::remove_dir_all(&dossier);
    let _ = std::fs::remove_dir_all(&dossier2);
}
