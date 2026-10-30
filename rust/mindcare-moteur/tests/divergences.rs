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
