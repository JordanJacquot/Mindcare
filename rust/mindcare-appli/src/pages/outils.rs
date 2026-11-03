//! Les petits outils des pages : lire un champ du catalogue en texte, accorder un nombre, les suspects à montrer.

use mindcare_moteur::ps::{champ, parcours, texte, vrai};
use mindcare_moteur::{Catalogue, Objet};

/// Un champ d'un objet du catalogue, en texte (vide s'il manque).
pub fn t(o: &Objet, nom: &str) -> String {
    texte(champ(o, nom))
}

/// Une liste de textes d'un champ du catalogue (Gestes, AFaire, Lignes...).
pub fn textes(o: &Objet, nom: &str) -> Vec<String> {
    parcours(champ(o, nom)).into_iter().map(|x| texte(Some(x))).collect()
}

/// « 1 panne », « 3 pannes ».
pub fn nombre(n: usize, un: &str, plusieurs: &str) -> String {
    if n == 1 { format!("1 {un}") } else { format!("{n} {plusieurs}") }
}

/// Le suspect est-il un danger à vérifier (pas un suspect à montrer) ?
pub fn urgent(cat: &Catalogue, id: &str) -> bool {
    cat.suspect(id).is_some_and(|s| vrai(champ(s, "Urgent")))
}

/// Le titre d'un suspect.
pub fn titre_suspect(cat: &Catalogue, id: &str) -> String {
    cat.suspect(id).map(|s| t(s, "Titre")).unwrap_or_default()
}

/// Les suspects d'une panne à montrer (sans les dangers), dans l'ordre du catalogue.
pub fn suspects_montres(cat: &Catalogue, panne: &Objet) -> Vec<String> {
    textes(panne, "Suspects").into_iter().filter(|s| !urgent(cat, s)).collect()
}

/// Les gestes touchent-ils au matériel (la consigne de sécurité), et pas seulement à Windows ? Le motif de
/// `Show-EtapeEnquete` (`débranch|câble|barrette|boîtier|capot|ventilat|multiprise|ouvr(e|ir) le PC|tournevis|batterie`,
/// sans tenir compte des majuscules).
pub fn gestes_materiel(gestes: &[String]) -> bool {
    const MOTS: [&str; 11] = [
        "débranch",
        "câble",
        "barrette",
        "boîtier",
        "capot",
        "ventilat",
        "multiprise",
        "ouvre le pc",
        "ouvrir le pc",
        "tournevis",
        "batterie",
    ];
    let tout = gestes.join(" ").to_lowercase();
    MOTS.iter().any(|m| tout.contains(m))
}

/// L'image d'une famille ou d'une étape, dans le dossier ui (images\<nom>.svg).
pub fn image(nom: &str) -> String {
    format!("images/{nom}.svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materiel_comme_powershell() {
        assert!(gestes_materiel(&["Débranche le câble".into()]));
        assert!(gestes_materiel(&["Ouvre le PC avec un tournevis".into()]));
        assert!(gestes_materiel(&["Enlève la BATTERIE".into()]));
        assert!(!gestes_materiel(&["Clique sur Démarrer, puis Paramètres".into()]));
        assert!(!gestes_materiel(&[]));
    }

    #[test]
    fn nombres_accordes() {
        assert_eq!(nombre(1, "panne", "pannes"), "1 panne");
        assert_eq!(nombre(0, "panne", "pannes"), "0 pannes");
        assert_eq!(nombre(12, "panne", "pannes"), "12 pannes");
    }
}
