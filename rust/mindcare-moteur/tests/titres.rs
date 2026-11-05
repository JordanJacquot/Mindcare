//! La phrase de la réponse quand une réponse a confirmé le suspect avec un détail (05/11/2026) : sans répéter ce que
//! le détail précise (« La batterie (ou le chargeur) lâche : la batterie » devient « La batterie lâche »). Tous les
//! détails du catalogue sont ici, avec la phrase attendue, relue un par un ; Mindcare Mobile a la même liste
//! (TelephoneTest) et doit écrire exactement les mêmes phrases.

mod commun;

use commun::catalogue;
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{TypeVerdict, Verdict, titre_affiche, titre_avec_detail, titre_verdict};
use serde_json::Value;

/// Le suspect, le détail, et la phrase attendue.
#[rustfmt::skip]
const ATTENDUS: [(&str, &str, &str); 31] = [
    ("aeration", "il est enfermé", "L'air ne circule pas autour du PC : il est enfermé"),
    ("arnaque", "ils ont eu accès au PC ou à des codes", "Danger : c'est une arnaque au faux support (ils ont eu accès au PC ou à des codes)"),
    ("arnaque", "rien de grave si rien n'a été fait", "Danger : c'est une arnaque au faux support (rien de grave si rien n'a été fait)"),
    ("batterie", "la batterie", "La batterie lâche"),
    ("batterie", "le chargeur ou la prise de charge", "Le chargeur ou la prise de charge lâche"),
    ("batteriegonflee", "dans le doute, traite-la comme gonflée", "Danger : la batterie a gonflé (dans le doute, traite-la comme gonflée)"),
    ("bluetoothson", "la distance ou les ondes autour", "La liaison Bluetooth était mal faite : la distance ou les ondes autour"),
    ("chargeur", "il chauffe mais sans danger visible", "Le chargeur ne donne plus de courant : il chauffe mais sans danger visible"),
    ("courant", "la prise", "Le courant n'arrive pas jusqu'au PC : la prise"),
    ("courant", "le câble ou la multiprise", "Le courant n'arrive pas jusqu'au PC : le câble ou la multiprise"),
    ("danger", "débranche et éloigne-toi tout de suite", "Danger : une pièce a peut-être grillé (débranche et éloigne-toi tout de suite)"),
    ("danger", "le chargeur", "Danger : une pièce a peut-être grillé (le chargeur)"),
    ("ecranalim", "il suffisait de l'allumer ou de rebrancher son câble", "L'écran lui-même était éteint ou débranché : il suffisait de l'allumer ou de rebrancher son câble"),
    ("ecranportable", "la dalle est cassée", "L'écran du portable (ou son câble) est abîmé : la dalle est cassée"),
    ("energie", "le portable marche sur batterie", "Le PC était bridé pour économiser l'énergie : le portable marche sur batterie"),
    ("localiser", "cherche-le avec « Localiser mon appareil »", "Windows peut localiser (et protéger) le portable perdu : cherche-le avec « Localiser mon appareil »"),
    ("localiser", "verrouille-le et change tes mots de passe", "Windows peut localiser (et protéger) le portable perdu : verrouille-le et change tes mots de passe"),
    ("majbloquee", "la dernière mise à jour", "Une mise à jour de Windows était en train de s'installer : la dernière mise à jour"),
    ("materiel", "une pièce mal remise", "Une pièce mal remise"),
    ("memoire", "celle avec laquelle il ne démarre pas", "Une barrette de mémoire (RAM) est abîmée : celle avec laquelle il ne démarre pas"),
    ("pilote", "le pilote de la carte graphique manque", "Le pilote de la carte graphique manque"),
    ("prise", "l'électricité de la maison", "Le courant coupe : l'électricité de la maison"),
    ("prise", "la multiprise", "Le courant coupe : la multiprise"),
    ("reinit", "choisis « Conserver mes fichiers »", "Réinitialiser Windows peut régler les problèmes que rien d'autre ne règle : choisis « Conserver mes fichiers »"),
    ("reinit", "choisis « Supprimer tout » (et « Nettoyer le lecteur »)", "Réinitialiser Windows peut régler les problèmes que rien d'autre ne règle : choisis « Supprimer tout » (et « Nettoyer le lecteur »)"),
    ("surchauffe", "la poussière", "Il chauffe trop : la poussière"),
    ("surchauffe", "un ventilateur ne tourne plus", "Il chauffe trop : un ventilateur ne tourne plus"),
    ("verrou", "Verr Num", "Une touche de verrouillage était activée : Verr Num"),
    ("wifioff", "le Bluetooth ou le mode avion", "Le Wi-Fi était désactivé (ou le mode avion activé) : le Bluetooth ou le mode avion"),
    ("windowsabime", "réparé par l'outil de redémarrage", "Windows est abîmé : réparé par l'outil de redémarrage"),
    ("windowsabime", "réparé par la restauration", "Windows est abîmé : réparé par la restauration"),
];

/// Les couples (suspect confirmé, détail) du catalogue, dans l'ordre où ils apparaissent, une fois chacun.
fn couples() -> Vec<(String, String)> {
    fn parcourir(v: &Value, l: &mut Vec<(String, String)>) {
        match v {
            Value::Object(o) => {
                let c = o.get("Confirme").and_then(Value::as_str).unwrap_or_default();
                let d = o.get("Detail").and_then(Value::as_str).unwrap_or_default();
                if !c.is_empty() && !d.is_empty() && !l.iter().any(|(a, b)| a == c && b == d) {
                    l.push((c.to_string(), d.to_string()));
                }
                o.values().for_each(|x| parcourir(x, l));
            }
            Value::Array(a) => a.iter().for_each(|x| parcourir(x, l)),
            _ => {}
        }
    }
    let mut l = Vec::new();
    parcourir(&Value::Object(catalogue().json().clone()), &mut l);
    l
}

fn trouve(id: &str) -> String {
    texte(catalogue().suspect(id).and_then(|s| champ(s, "Trouve")))
}

#[test]
fn chaque_detail_du_catalogue_donne_une_phrase_relue() {
    let mut ecarts = Vec::new();
    for (s, d) in couples() {
        let obtenu = titre_avec_detail(&trouve(&s), &d);
        match ATTENDUS.iter().find(|(a, b, _)| *a == s && *b == d) {
            Some((_, _, attendu)) if obtenu != *attendu => {
                ecarts.push(format!("{s} / « {d} » : « {obtenu} », attendu « {attendu} »"));
            }
            Some(_) => {}
            // Un détail ajouté au catalogue depuis : au moins pas deux « : » ni le détail répété
            None if obtenu.matches(" : ").count() > 1 => ecarts.push(format!("{s} / « {d} » : « {obtenu} »")),
            None => {}
        }
    }
    assert!(ecarts.is_empty(), "{}", ecarts.join("\n"));
    assert!(couples().len() >= ATTENDUS.len(), "des détails ont disparu du catalogue : mettre ATTENDUS à jour");
}

#[test]
fn le_titre_affiche_ne_change_que_les_details() {
    let cat = catalogue();
    let v = |genre, suspect: &str, detail: &str| Verdict {
        genre,
        suspect: Some(suspect.to_string()),
        detail: detail.to_string(),
        restants: vec![suspect.to_string()],
    };
    let batterie = v(TypeVerdict::Trouve, "batterie", "la batterie");
    assert_eq!(titre_affiche(cat, Some(&batterie)), "La batterie lâche");
    // Le titre du moteur (Get-VerdictTitre, comparé aux juges et au téléphone) ne change pas
    assert_eq!(titre_verdict(cat, Some(&batterie)), "La batterie (ou le chargeur) lâche : la batterie");
    for sans_detail in [v(TypeVerdict::Trouve, "batterie", ""), v(TypeVerdict::Elimination, "batterie", "")] {
        assert_eq!(titre_affiche(cat, Some(&sans_detail)), titre_verdict(cat, Some(&sans_detail)));
    }
    assert_eq!(titre_affiche(cat, None), "");
}
