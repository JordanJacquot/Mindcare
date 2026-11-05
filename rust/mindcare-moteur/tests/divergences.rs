//! Les divergences voulues avec le moteur PowerShell (tests\juges\divergences.json, audit du 25/10).
//!
//! Constat 20 : une mesure ratée (redémarrage, place libre, batterie) doit valoir « inconnu » et n'écarter aucune piste.
//! Le moteur pur n'a rien à changer : un indice absent n'écarte déjà rien. C'est la lecture de Windows (étape 3) qui ne
//! doit pas donner d'indice pour une mesure ratée. Ce test garde les deux réponses : celle d'aujourd'hui (indice raté
//! donné comme « pas vu » : la piste est écartée) et la réponse voulue (indice absent : la piste reste).

mod commun;

use commun::{catalogue, choix, ecart, indices, lire_juge, panne, resultat, verdict_final};
use mindcare_moteur::enquete;

#[test]
fn mesure_ratee_vaut_inconnu() {
    let juge = lire_juge("divergences.json");
    let cat = catalogue();
    let cas = juge["Divergences"].as_array().expect("Divergences");
    let mut ecarts = Vec::new();
    for c in cas {
        let nom = c["Nom"].as_str().unwrap_or_default();
        let p = panne(c["Panne"].as_str().unwrap_or_default());
        let entree = choix(&c["Entree"]);
        let piste = c["Piste"].as_str().unwrap_or_default();
        let aujourd_hui = enquete(cat, p, &entree, indices(&c["IndicesPowerShell"]).as_ref());
        if let Some(d) = ecart(&c["ReponsePowerShell"], &resultat(cat, &aujourd_hui)) {
            ecarts.push(format!("« {nom} », indice raté :\n    {d}"));
        }
        let voulue = enquete(cat, p, &entree, indices(&c["IndicesVoulus"]).as_ref());
        if let Some(d) = ecart(&c["ReponseVoulue"], &resultat(cat, &voulue)) {
            ecarts.push(format!("« {nom} », indice absent :\n    {d}"));
        }
        if !aujourd_hui.ecartes.iter().any(|x| x.id == piste) || !voulue.restants.iter().any(|x| x == piste) {
            ecarts.push(format!(
                "« {nom} » : la piste {piste} devrait être écartée avec l'indice raté et rester sans lui"
            ));
        }
    }
    verdict_final("divergences (constat 20)", cas.len(), &ecarts);
}

/// Écart voulu du 05/11/2026 : la recherche ne classe plus comme Find-Pannes (le PowerShell n'est plus livré, le
/// moteur Rust fait foi). Les pannes trouvées restent exactement celles de PowerShell (tests\recherches.rs le vérifie
/// sur les milliers de recherches du juge) ; seul leur ordre change. Avant, « il s'éteint tout seul » mettait « Il
/// s'éteint d'un coup » 6e, derrière « L'écran s'éteint trop vite » (le mot « seul » de « écran noir tout seul »
/// comptait autant que « éteint »). Les phrases de tous les jours qui mesurent ce changement : tests\phrases.rs.
#[test]
fn recherche_classee_autrement_que_powershell() {
    let cat = catalogue();
    let juge = lire_juge("recherches.json");
    let ids = |l: Vec<&mindcare_moteur::Objet>| -> Vec<String> {
        l.into_iter().map(|p| mindcare_moteur::ps::texte(mindcare_moteur::ps::champ(p, "Id"))).collect()
    };
    let q = "Il s'éteint tout seul";
    let powershell = ids(mindcare_moteur::chercher_comme_powershell(cat, q));
    let rust = ids(mindcare_moteur::chercher(cat, q));
    assert_eq!(powershell.iter().position(|p| p == "seteint"), Some(5), "PowerShell : {powershell:?}");
    assert_eq!(rust.first().map(String::as_str), Some("seteint"), "Rust : {rust:?}");
    // Combien de recherches du juge n'ont plus la même panne en tête (pour garder la mesure sous les yeux)
    let cas = juge["Recherches"].as_array().expect("Recherches");
    let changees = cas
        .iter()
        .filter(|c| {
            let q = c["Q"].as_str().unwrap_or_default();
            ids(mindcare_moteur::chercher(cat, q)).first()
                != c["P"].get(0).and_then(|x| x.as_str()).map(str::to_string).as_ref()
        })
        .count();
    assert!(changees < cas.len() / 4, "{changees} recherches sur {} changent de panne en tête", cas.len());
}
