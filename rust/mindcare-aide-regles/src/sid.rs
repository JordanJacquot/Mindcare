//! Copié de D:\Projets\Nevermind\rust\aide-regles\src\sid.rs (plan de l'Aide de Mindcare, 14/11/2026).
//!
//! Pour qui l'Aide travaille.
//!
//! - En SYSTEM (la tâche `\Mindset\Mindcare (aide)`, créée par l'installateur de Mindset) : seulement l'utilisateur
//!   inscrit dans la tâche (`-Sid`), par l'administrateur qui l'a créée ; sans `-Sid` valable, l'Aide ne démarre pas.
//! - Lancée autrement, sous le compte d'un utilisateur : le SID reçu doit être celui de son propre compte, sinon elle
//!   ne démarre pas. (Mindcare ne la lance jamais ainsi : pas de demande de Windows, décision de Jordan du 14/11 ;
//!   la règle reste celle de Nevermind, pour qu'un lancement à la main ne serve jamais un autre compte.)

/// Le compte du système
pub const SYSTEME: &str = "S-1-5-18";

/// Un SID de compte d'utilisateur : `^S-1-5-21-\d+-\d+-\d+-\d+\z` (majuscules comprises)
pub fn sid_utilisateur(s: &str) -> bool {
    let Some(r) = s.strip_prefix("S-1-5-21-") else {
        return false;
    };
    let parties: Vec<&str> = r.split('-').collect();
    parties.len() == 4 && parties.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Le compte servi, ou la raison du refus de démarrer. `compte` : le compte sous lequel l'Aide tourne ;
/// `sid_recu` : la valeur de `-Sid` sur la ligne de commande.
pub fn compte_servi(compte: &str, sid_recu: Option<&str>) -> Result<String, String> {
    if compte == SYSTEME {
        return match sid_recu {
            Some(s) if sid_utilisateur(s) => Ok(s.to_string()),
            _ => Err("en SYSTEM sans -Sid valable".into()),
        };
    }
    match sid_recu {
        None => Ok(compte.to_string()),
        Some(s) if s == compte => Ok(s.to_string()),
        Some(s) => Err(format!("-Sid {s} n'est pas le compte qui fait tourner l'Aide ({compte})")),
    }
}

/// La valeur de `-Sid` dans les arguments (« -Sid », « /Sid », « --sid »... suivis de la valeur ; la dernière
/// compte)
pub fn lire_arguments(args: &[String]) -> Option<String> {
    let mut sid = None;
    for i in 0..args.len().saturating_sub(1) {
        if args[i].trim_matches(['-', '/']).eq_ignore_ascii_case("Sid") {
            sid = Some(args[i + 1].clone());
        }
    }
    sid
}

#[cfg(test)]
mod tests {
    use super::*;

    const ELODIE: &str = "S-1-5-21-1111111111-2222222222-3333333333-1001";

    #[test]
    fn forme_du_sid() {
        assert!(sid_utilisateur(ELODIE));
        assert!(!sid_utilisateur("S-1-5-18"));
        assert!(!sid_utilisateur("s-1-5-21-1-2-3-4"));
        assert!(!sid_utilisateur("S-1-5-21-1-2-3-4-5"));
        assert!(!sid_utilisateur("S-1-5-21-1-2-3-4\n"));
        assert!(!sid_utilisateur("S-1-5-21-1-2-3-"));
        assert!(!sid_utilisateur("pas-un-sid"));
    }

    #[test]
    fn en_systeme() {
        assert_eq!(compte_servi(SYSTEME, Some(ELODIE)), Ok(ELODIE.into()));
        assert!(compte_servi(SYSTEME, None).is_err());
        assert!(compte_servi(SYSTEME, Some("S-1-5-18")).is_err());
        assert!(compte_servi(SYSTEME, Some("pas-un-sid")).is_err());
    }

    #[test]
    fn sous_un_compte() {
        assert_eq!(compte_servi(ELODIE, Some(ELODIE)), Ok(ELODIE.into()));
        assert_eq!(compte_servi(ELODIE, None), Ok(ELODIE.into()));
        // Un administrateur qui la lancerait pour un autre compte : refusé
        assert!(compte_servi("S-1-5-21-1111111111-2222222222-3333333333-500", Some(ELODIE)).is_err());
        assert!(compte_servi(ELODIE, Some("S-1-5-18")).is_err());
        assert!(compte_servi(ELODIE, Some("")).is_err());
    }

    #[test]
    fn arguments() {
        let a = |v: &[&str]| lire_arguments(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(a(&["-Sid", ELODIE]), Some(ELODIE.into()));
        assert_eq!(a(&["/sid", "x", "--SID", "y"]), Some("y".into()));
        assert_eq!(a(&["-Sid"]), None);
        assert_eq!(a(&[]), None);
    }
}
