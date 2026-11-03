//! `ConvertFrom-Dictee` : ce que whisper a écrit, nettoyé. Sans les bruits notés entre crochets, parenthèses ou
//! étoiles (« [Musique] », « (rires) »), ni les phrases qu'il invente sur un silence (« Sous-titrage ST' 501 »,
//! « Merci d'avoir regardé »...). Les mêmes expressions que le PowerShell.

use std::sync::OnceLock;

use regex::Regex;

struct Motifs {
    bruits: Regex,
    blancs: Regex,
    inventees: Regex,
    mot: Regex,
}

fn motifs() -> &'static Motifs {
    static M: OnceLock<Motifs> = OnceLock::new();
    M.get_or_init(|| Motifs {
        bruits: Regex::new(r"\[[^\]]*\]|\([^)]*\)|\*[^*]*\*").expect("motif des bruits"),
        blancs: Regex::new(r"\s+").expect("motif des blancs"),
        // (« . » de .NET : tout sauf un retour à la ligne ; il n'en reste plus après les blancs)
        inventees: Regex::new(r"(?i)sous-titr|amara\.org|merci d.avoir regard|abonnez-vous").expect("motif inventé"),
        mot: Regex::new(r"\w").expect("motif d'un mot"),
    })
}

/// La phrase à mettre dans la recherche, ou `""` s'il n'y a rien de sensé.
pub fn nettoyer(brut: &str) -> String {
    let m = motifs();
    let t = m.bruits.replace_all(brut, " ");
    let t = m.blancs.replace_all(&t, " ");
    let t = t.trim();
    if m.inventees.is_match(t) || !m.mot.is_match(t) {
        return String::new();
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::nettoyer;

    /// Les cas de tester-dictee.ps1 (partie 6, et la phrase de la partie 2)
    #[test]
    fn cas_du_powershell() {
        assert_eq!(nettoyer(" [Musique] Mon PC s'éteint quand je joue. \r\n"), "Mon PC s'éteint quand je joue.");
        for rien in ["", "[BLANK_AUDIO]", "Sous-titrage ST' 501", "..."] {
            assert_eq!(nettoyer(rien), "", "« {rien} »");
        }
        assert_eq!(nettoyer("le wifi coupe"), "le wifi coupe");
    }

    #[test]
    fn bruits_et_phrases_inventees() {
        assert_eq!(nettoyer("(rires) l'écran *bip* reste\tnoir [bruit]"), "l'écran reste noir");
        assert_eq!(nettoyer("Merci d’avoir regardé !"), "");
        assert_eq!(nettoyer("ABONNEZ-VOUS"), "");
        assert_eq!(nettoyer("sous-titres réalisés par amara.org"), "");
        // (un crochet sans fin n'est pas un bruit : il reste, comme en .NET)
        assert_eq!(nettoyer("le [clavier"), "le [clavier");
        // (sur plusieurs lignes : les crochets prennent aussi les retours à la ligne)
        assert_eq!(nettoyer("la [musique\nforte] souris"), "la souris");
        assert_eq!(nettoyer("  é  "), "é");
        assert_eq!(nettoyer("_"), "_");
    }
}
