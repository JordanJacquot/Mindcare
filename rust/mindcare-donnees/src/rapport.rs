//! Le rapport pour le réparateur : un texte simple (la panne, la réponse, les preuves, les réponses, le PC).
//! `Get-InfosPC`, `Get-ResumeTexte`, `Save-Resume` (modules\historique.ps1) et les textes de « Pour le réparateur »
//! (`Add-Reparateur`, modules\contenu.ps1 : seulement ses textes, l'écran viendra plus tard).

use std::path::{Path, PathBuf};

use crate::historique::IndicesGardes;
use crate::journal::Journal;
use crate::moment::{Moment, format_moment};
use mindcare_moteur::ps::{champ, egal, texte, vrai};
use mindcare_moteur::{Catalogue, Enquete, Objet, TypeVerdict};

/// Ce que Windows dit du PC (Win32_ComputerSystem et Win32_OperatingSystem), lu par l'autre crate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LecturePc {
    pub fabricant: Option<String>,
    pub modele: Option<String>,
    /// Le nom de Windows (Caption)
    pub systeme: Option<String>,
}

/// `Get-InfosPC` sans la lecture : « Fabricant Modèle, Windows ». Une lecture ratée (son message) donne un texte vide
/// et une ligne au journal.
pub fn infos_pc(lecture: Result<&LecturePc, &str>, journal: &Journal) -> String {
    match lecture {
        Ok(l) => {
            let t = |x: &Option<String>| x.as_deref().unwrap_or_default().trim().to_string();
            format!("{} {}, {}", t(&l.fabricant), t(&l.modele), t(&l.systeme))
        }
        Err(e) => {
            journal.ecrire(&format!("Modèle du PC illisible : {e}"));
            String::new()
        }
    }
}

/// Ce dont le rapport a besoin en plus de l'enquête (lu sur le PC seulement pour une enquête faite sur lui, « cepc »).
#[derive(Debug, Clone, Default)]
pub struct SurLePc<'a> {
    /// `Get-InfosPC`
    pub infos: &'a str,
    /// `Get-Barrettes` : « 8 Go Kingston KF3200 (DIMM1) »...
    pub barrettes: &'a [String],
    /// `Get-Indices` : ce que Windows a noté aujourd'hui (le journal illisible ou trop court : « À savoir »)
    pub indices_du_jour: Option<&'a IndicesGardes>,
}

/// Vrai ou faux d'une liste de textes, comme `if (@(...))`.
fn liste_vraie(l: &[String]) -> bool {
    match l {
        [] => false,
        [x] => !x.is_empty(),
        _ => true,
    }
}

/// `Get-ResumeTexte` : le rapport de l'enquête, lignes séparées par « \r\n ».
#[allow(clippy::too_many_arguments)]
pub fn texte_rapport(
    cat: &Catalogue,
    panne: &Objet,
    e: &Enquete,
    titre: &str,
    maintenant: &Moment,
    pc: &SurLePc,
    version: &str,
) -> String {
    let mut l: Vec<String> = vec!["Mindcare : rapport de l'enquête".into(), String::new()];
    l.push(format!("Date : {}", format_moment(maintenant, true)));
    l.push(format!("Panne : {}", texte(champ(panne, "Titre"))));
    let cepc = e.notes.iter().any(|n| egal(n, "cepc"));
    if cepc {
        if !pc.infos.is_empty() {
            l.push(format!("Ordinateur : {}", pc.infos));
        }
        if liste_vraie(pc.barrettes) {
            l.push(format!("Mémoire : {}", pc.barrettes.join(" ; ")));
        }
    }
    l.push(String::new());
    l.push(format!("Réponse : {titre}"));
    if liste_vraie(&e.preuves) {
        l.push(String::new());
        l.push("Comment on le sait :".into());
        l.extend(e.preuves.iter().map(|p| format!("- {p}")));
    }
    let suspect = |id: &str| cat.suspect(id);
    let ecartes: Vec<String> = e
        .ecartes
        .iter()
        .filter(|x| !suspect(&x.id).is_some_and(|s| vrai(champ(s, "Urgent"))))
        .map(|x| {
            let t = texte(suspect(&x.id).and_then(|s| champ(s, "Titre")));
            if x.raison.is_empty() { t } else { format!("{t} ({})", x.raison) }
        })
        .collect();
    if liste_vraie(&ecartes) {
        l.push(String::new());
        l.push("Écartés :".into());
        l.extend(ecartes.iter().map(|x| format!("- {x}")));
    }
    if let Some(v) = &e.verdict
        && v.genre == TypeVerdict::Reparateur
    {
        let noms: Vec<String> =
            v.restants.iter().map(|id| texte(suspect(id).and_then(|s| champ(s, "Titre")))).collect();
        l.push(String::new());
        l.push(format!("Il reste : {}", noms.join(", ")));
    }
    if !e.auto.is_empty() {
        l.push(String::new());
        l.push("Ce que Mindcare a vérifié sur ce PC (ce que Windows a noté, la place libre, la batterie...) :".into());
        l.extend(e.auto.iter().map(|a| format!("- {}", a.texte)));
    }
    // (le journal de Windows illisible, ou trop court : le réparateur doit savoir que ses indices n'ont pas tous servi)
    if cepc
        && let Some(j) = pc.indices_du_jour.and_then(|i| i.get("journal"))
        && !j.texte_ok.is_empty()
    {
        l.push(String::new());
        l.push(format!("À savoir : {}.", j.texte_ok));
    }
    l.push(String::new());
    l.push("Les réponses :".into());
    for f in &e.faits {
        l.push(format!("- {}", f.texte));
        l.push(format!("  {}", f.reponse));
    }
    l.push(String::new());
    l.push(format!("Fait avec Mindcare {version}."));
    l.join("\r\n")
}

/// Le nom du fichier du rapport : « Mindcare, diagnostic du 30-10-2026 14h05 ».
pub fn nom_rapport(m: &Moment) -> String {
    format!("Mindcare, diagnostic du {}-{:02}-{:04} {:02}h{:02}", m.jour, m.mois, m.annee, m.heure, m.minute)
}

/// `Save-Resume` sans l'ouverture : enregistre le rapport dans `dossier` (le Bureau ; le dossier de données pendant
/// les tests), en UTF-8 avec BOM (le Bloc-notes lit bien les accents). Deux rapports dans la même minute : le premier
/// est gardé (« (2) », « (3) »...). Rend son chemin.
pub fn enregistrer_rapport(dossier: &Path, texte_rapport: &str, journal: &Journal) -> std::io::Result<PathBuf> {
    let nom = nom_rapport(&journal.maintenant());
    let mut f = dossier.join(format!("{nom}.txt"));
    let mut n = 2;
    while f.is_file() {
        f = dossier.join(format!("{nom} ({n}).txt"));
        n += 1;
    }
    let mut octets = vec![0xEF, 0xBB, 0xBF];
    octets.extend_from_slice(texte_rapport.as_bytes());
    std::fs::write(&f, octets)?;
    journal.ecrire(&format!("Rapport enregistré : {}", f.display()));
    Ok(f)
}

/// Les textes de « Pour le réparateur » (`Add-Reparateur`).
pub mod reparateur {
    pub const TITRE: &str = "Pour le réparateur";
    pub const EXPLICATION: &str = "Le rapport de cette enquête (la panne, la réponse, les preuves, tes réponses et le modèle du PC), à montrer à un réparateur ou à envoyer à quelqu'un qui peut t'aider.";
    pub const BOUTON_ENREGISTRER: &str = "Enregistrer le rapport sur le Bureau";
    pub const BOUTON_COPIER: &str = "Copier le rapport";
    pub const ECHEC_ENREGISTRER: &str =
        "Le rapport n'a pas pu être enregistré sur le Bureau. Essaie plutôt « Copier le rapport ».";
    pub const COPIE: &str = "Le rapport est copié : tu peux le coller dans un message ou un e-mail.";
    pub const ECHEC_COPIE: &str = "Le rapport n'a pas pu être copié. Essaie plutôt de l'enregistrer sur le Bureau.";

    /// Le rapport est enregistré sur le Bureau (le nom de son fichier).
    pub fn enregistre(nom_fichier: &str) -> String {
        format!("C'est fait : le fichier « {nom_fichier} » est sur le Bureau.")
    }

    /// La ligne du journal quand l'enregistrement rate.
    pub fn journal_echec_enregistrer(erreur: &str) -> String {
        format!("Rapport non enregistré : {erreur}")
    }

    /// La ligne du journal quand la copie rate.
    pub fn journal_echec_copie(erreur: &str) -> String {
        format!("Rapport non copié : {erreur}")
    }
}
