//! Les demandes que l'Aide de Mindcare accepte : une liste FERMÉE, sans aucun paramètre (décision de Jordan du
//! 14/11/2026). Mindcare ne donne qu'un nom d'action ; ce que l'Aide fait pour chacune est écrit dans MindcareAide.exe
//! et nulle part ailleurs. Tout ce qui n'est pas exactement prévu ici est refusé : une action inconnue, un nom écrit
//! autrement (majuscules), un paramètre (même vide autrement que `{}`), un champ en plus.
//!
//! L'échange (comme l'Aide de Nevermind) : une ligne JSON `{"a":"<action>"}` (ou `{"a":"<action>","p":{}}`), une ligne
//! JSON en réponse `{"ok":true,"r":...}` ou `{"ok":false,"e":"<phrase simple>"}`.

use serde_json::{Value, json};

/// La version du protocole de l'Aide de Mindcare (rendue par `aide.bonjour`)
pub const VERSION: i32 = 1;

/// Une demande acceptée
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demande {
    /// Qui est l'Aide (version, compte, droits)
    Bonjour,
    /// Les lignes de son journal (gardé en mémoire), que Mindcare recopie dans le sien
    Journal,
    /// Faire oublier tous les Wi-Fi enregistrés
    WifiOublier,
    /// Rendre puis redemander l'adresse des cartes réseau réglées en automatique
    IpRenouveler,
    /// Remettre les réglages d'alimentation d'origine (`powercfg -restoredefaultschemes`)
    EnergieParDefaut,
    /// Mettre de côté les mises à jour téléchargées (dossier SoftwareDistribution)
    MajVider,
}

impl Demande {
    /// Toutes les demandes acceptées
    pub const TOUTES: [Demande; 6] = [
        Demande::Bonjour,
        Demande::Journal,
        Demande::WifiOublier,
        Demande::IpRenouveler,
        Demande::EnergieParDefaut,
        Demande::MajVider,
    ];

    /// Son nom sur le canal
    pub fn nom(self) -> &'static str {
        match self {
            Demande::Bonjour => "aide.bonjour",
            Demande::Journal => "aide.journal",
            Demande::WifiOublier => "wifi.oublier",
            Demande::IpRenouveler => "ip.renouveler",
            Demande::EnergieParDefaut => "energie.par-defaut",
            Demande::MajVider => "maj.vider",
        }
    }

    /// Une réparation (elle change le PC) : une seule à la fois
    pub fn reparation(self) -> bool {
        !matches!(self, Demande::Bonjour | Demande::Journal)
    }

    /// La demande de ce nom (exactement) ; aucune pour un autre nom
    pub fn depuis_nom(nom: &str) -> Option<Demande> {
        Demande::TOUTES.into_iter().find(|d| d.nom() == nom)
    }

    /// La ligne envoyée par Mindcare pour cette demande (sans « \r\n »)
    pub fn ligne(self) -> String {
        json!({ "a": self.nom() }).to_string()
    }
}

/// Le début de chaque refus (le reste dit pourquoi)
pub const REFUS: &str = "Demande refusée";

/// La demande de cette ligne, vérifiée, ou le message du refus (rendu tel quel dans `e`)
pub fn verifier(ligne: &str) -> Result<Demande, String> {
    let d: Value = serde_json::from_str(ligne).map_err(|_| format!("{REFUS} : elle est illisible."))?;
    let Value::Object(o) = d else { return Err(format!("{REFUS} : elle est illisible.")) };
    if o.keys().any(|k| k != "a" && k != "p") {
        return Err(format!("{REFUS} : elle a un champ en trop."));
    }
    let Some(Value::String(a)) = o.get("a") else { return Err(format!("{REFUS} : elle ne dit pas quoi faire.")) };
    let Some(demande) = Demande::depuis_nom(a) else {
        return Err(format!("{REFUS} : l'Aide de Mindcare ne connaît pas cette action."));
    };
    match o.get("p") {
        None | Some(Value::Null) => Ok(demande),
        Some(Value::Object(p)) if p.is_empty() => Ok(demande),
        Some(_) => Err(format!("{REFUS} : cette action ne prend aucun paramètre.")),
    }
}

/// Une réparation est déjà en cours (une seule à la fois)
pub const OCCUPEE: &str = "Une réparation est déjà en cours. Attends qu'elle soit finie, puis réessaie.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_liste_fermee() {
        for d in Demande::TOUTES {
            assert_eq!(verifier(&d.ligne()), Ok(d), "{d:?}");
            assert_eq!(verifier(&format!("{{\"a\":\"{}\",\"p\":{{}}}}", d.nom())), Ok(d));
            assert_eq!(verifier(&format!("{{\"a\":\"{}\",\"p\":null}}", d.nom())), Ok(d));
        }
        assert_eq!(Demande::TOUTES.iter().filter(|d| d.reparation()).count(), 4);
        assert_eq!(Demande::WifiOublier.ligne(), r#"{"a":"wifi.oublier"}"#);
    }

    #[test]
    fn tout_le_reste_est_refuse() {
        for ligne in [
            r#"{"a":"wifi.supprimer"}"#,
            r#"{"a":"WIFI.OUBLIER"}"#,
            r#"{"a":" wifi.oublier"}"#,
            r#"{"a":"reg.ecrire","p":{"Cle":"HKLM:\\X"}}"#,
            r#"{"a":"processus.fermer"}"#,
            r#"{"a":""}"#,
        ] {
            let e = verifier(ligne).unwrap_err();
            assert!(e.starts_with(REFUS) && e.contains("ne connaît pas"), "{ligne} : {e}");
        }
        for ligne in [
            r#"{"a":"wifi.oublier","p":{"Profil":"Box"}}"#,
            r#"{"a":"ip.renouveler","p":{"Carte":3}}"#,
            r#"{"a":"maj.vider","p":"C:\\Windows"}"#,
            r#"{"a":"energie.par-defaut","p":[]}"#,
            r#"{"a":"maj.vider","p":0}"#,
        ] {
            let e = verifier(ligne).unwrap_err();
            assert!(e.contains("aucun paramètre"), "{ligne} : {e}");
        }
        for ligne in ["", "pas du json", "[]", "\"wifi.oublier\"", r#"{"a":3}"#, r#"{}"#] {
            assert!(verifier(ligne).unwrap_err().starts_with(REFUS), "{ligne}");
        }
        assert!(verifier(r#"{"a":"wifi.oublier","x":1}"#).unwrap_err().contains("champ en trop"));
    }
}
