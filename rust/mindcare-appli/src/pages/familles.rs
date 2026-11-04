//! « Ça concerne quoi ? » (les familles de pannes, en deux colonnes de listes groupées) et une famille (ses pannes,
//! en cartes dépliables par groupe).

use serde_json::{Value, json};

use mindcare_moteur::ps::{champ, champ_v, egal, parcours, texte};
use mindcare_moteur::{Catalogue, Objet};

use super::actions::ids;
use super::outils::t;
use super::raconte::PANNE;
use super::{Etat, ecrans};

/// Le début de l'id d'une famille à ouvrir (« famille:plantages »).
pub const FAMILLE: &str = "famille:";

/// Le bouton de la bande « Adapté à ton PC ».
pub const TOUT_AFFICHER: &str = "Tout afficher";

/// Les pannes d'une famille, dans l'ordre du catalogue.
pub fn pannes_de<'a>(cat: &'a Catalogue, famille: &str) -> Vec<&'a Objet> {
    cat.pannes().iter().filter(|p| egal(&t(p, "Famille"), famille)).collect()
}

/// L'icône d'une famille (un dessin au trait du kit, 20 px dans sa pastille) : par son Id ; une famille inconnue a
/// celle d'une liste. (Les images des familles, `Icone` du catalogue, restent celles du téléphone : décentrées et
/// trop détaillées pour une pastille de 36 px.)
pub fn icone_famille(id: &str) -> &'static str {
    match id {
        "allumage" => "marche",
        "voyants" => "voyants",
        "demarrage" => "demarrage",
        "ecran" => "moniteur",
        "plantages" => "eclair",
        "jeux" => "manette",
        "lenteur" => "horloge",
        "bruits" => "thermo",
        "son" => "son",
        "internet" => "wifi",
        "peripheriques" => "souris",
        "batterie" => "batterie",
        "virus" => "bouclier",
        "fichiers" => "dossier",
        "windows" => "windows",
        _ => "liste",
    }
}

/// L'icône du kit pour une image du catalogue (« Icone » d'un groupe : une image de Mindcare\fichiers\images, faite
/// pour le téléphone) : un dessin au trait qui dit la même chose ; une image inconnue : aucune (la carte prend alors
/// l'icône de sa famille).
pub fn icone_image(image: &str) -> Option<&'static str> {
    if let Some(famille) = image.strip_prefix("famille-") {
        return Some(icone_famille(famille)).filter(|i| *i != "liste");
    }
    Some(match image {
        "interrupteur" | "bouton-15s" => "marche",
        "redemarrer" => "actu",
        "prise-lampe" => "eclair",
        "lampe" => "ampoule",
        "tour-voyant" => "voyants",
        "stockage" => "disque",
        "ecran-bleu" => "alerte",
        "ecran-signal" => "moniteur",
        "mise-a-jour" => "sauve",
        "portable-chargeur" => "batterie",
        "branchement" => "cable",
        "grille-poussiere" => "thermo",
        _ => return None,
    })
}

pub fn page_familles(etat: &Etat, cat: &Catalogue) -> Value {
    let elements: Vec<Value> = cat
        .familles()
        .iter()
        .map(|f| {
            let id = t(f, "Id");
            let texte = if pannes_de(cat, &id).is_empty() { "Arrive bientôt".to_string() } else { t(f, "Phrase") };
            json!({ "id": format!("{FAMILLE}{id}"), "titre": t(f, "Titre"), "texte": texte,
                    "icone": icone_famille(&id), "couleur": "P" })
        })
        .collect();
    let mut blocs = Vec::new();
    // (seulement quand la détection du PC a caché des pannes qui ne le concernent pas)
    if let Some(texte) = etat.adapte_au_pc.as_deref().filter(|x| !x.trim().is_empty()) {
        blocs.push(json!({ "type": "bande", "texte": texte,
                           "bouton": { "id": ids::TOUT_AFFICHER, "texte": TOUT_AFFICHER } }));
    }
    // (la première famille choisie à l'ouverture, pas le bouton de la bande)
    let focus = elements.first().map(|e| format!("l:{}", e["id"].as_str().unwrap_or_default()));
    blocs.push(json!({ "type": "groupe", "colonnes": 2, "elements": elements }));
    json!({
        "focus": focus,
        "titre": "Ça concerne quoi ?",
        "sous": "Choisis ce qui ressemble le plus à ce qui se passe.",
        "blocs": blocs,
    })
}

/// Les textes d'une famille.
pub mod textes {
    pub const SOUS: &str = "Déplie la carte qui ressemble le plus";
    pub const TOUTES: &str = "Toutes les pannes";
    pub const AUTRES: &str = "Autres pannes";
    pub const AUTRES_EXEMPLE: &str = "Ce qui ne rentre dans aucune carte";
    pub const RACONTE: &str = "Pas trouvé ? Raconte ton problème avec tes mots";
    pub const BIENTOT: &str = "Arrive bientôt : cette famille n'a pas encore de panne connue.";
}

/// Un groupe de pannes d'une famille (« Groupes » du catalogue : Id, Titre ou Nom, Exemple, Icone).
struct Groupe {
    id: String,
    titre: String,
    exemple: String,
    icone: String,
}

/// Les groupes d'une famille, dans l'ordre du catalogue (aucun : la famille n'en a pas encore).
fn groupes(f: &Objet) -> Vec<Groupe> {
    parcours(champ(f, "Groupes"))
        .into_iter()
        .map(|g| {
            let c = |n: &str| texte(champ_v(Some(g), n));
            let titre = Some(c("Titre")).filter(|x| !x.is_empty()).unwrap_or_else(|| c("Nom"));
            Groupe { id: c("Id"), titre, exemple: c("Exemple"), icone: c("Icone") }
        })
        .filter(|g| !g.id.is_empty() && !g.titre.is_empty())
        .collect()
}

/// Une famille et ses pannes, en cartes dépliables (une par groupe de pannes, la première ouverte ; une famille sans
/// groupes : une seule carte « Toutes les pannes ») ; rien pour une famille inconnue.
pub fn page_famille(cat: &Catalogue, id: &str) -> Option<Value> {
    let f = cat.famille(id)?;
    let fid = t(f, "Id");
    let pannes = pannes_de(cat, &fid);
    let secours = icone_famille(&fid);
    let ligne = |p: &Objet| json!({ "id": format!("{PANNE}{}", t(p, "Id")), "texte": t(p, "Titre") });
    let gs = groupes(f);
    let mut cartes: Vec<Value> = Vec::new();
    for g in &gs {
        let siennes: Vec<Value> = pannes.iter().filter(|p| egal(&t(p, "Groupe"), &g.id)).map(|p| ligne(p)).collect();
        if siennes.is_empty() {
            continue;
        }
        let icone = icone_image(&g.icone).unwrap_or(secours);
        cartes.push(json!({ "id": g.id, "titre": g.titre, "texte": g.exemple, "icone": icone, "secours": secours,
                            "nombre": siennes.len(), "pannes": siennes }));
    }
    // (les pannes sans groupe connu : une carte de plus, ou la seule carte quand la famille n'a pas de groupes)
    let seules: Vec<Value> =
        pannes.iter().filter(|p| !gs.iter().any(|g| egal(&t(p, "Groupe"), &g.id))).map(|p| ligne(p)).collect();
    if !seules.is_empty() {
        let (titre, exemple) = if cartes.is_empty() {
            (textes::TOUTES, t(f, "Phrase"))
        } else {
            (textes::AUTRES, textes::AUTRES_EXEMPLE.into())
        };
        cartes.push(json!({ "id": "autres", "titre": titre, "texte": exemple, "icone": secours,
                            "nombre": seules.len(), "pannes": seules }));
    }
    let mut page = json!({
        "titre": t(f, "Titre"), "parent": ecrans::FAMILLES, "icone": secours, "sous": textes::SOUS,
    });
    let mut blocs = Vec::new();
    if let Some(premiere) = cartes.first_mut() {
        premiere["ouvert"] = json!(true);
        page["focus"] = json!(format!("l:{}", premiere["pannes"][0]["id"].as_str().unwrap_or_default()));
        blocs.push(json!({ "type": "depliables", "id": "groupes", "elements": cartes }));
    } else {
        blocs.push(json!({ "type": "note", "texte": textes::BIENTOT }));
    }
    blocs.push(json!({ "type": "bouton", "id": ecrans::RACONTE, "texte": textes::RACONTE, "style": "lien" }));
    page["blocs"] = json!(blocs);
    Some(page)
}
