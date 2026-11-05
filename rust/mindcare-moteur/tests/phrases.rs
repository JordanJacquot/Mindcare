//! Des phrases de tous les jours, comme les tapent des gens pas à l'aise avec un PC, et la panne qu'elles doivent
//! trouver dans les premières (05/11/2026). Pour chacune : sa phrase, les pannes attendues (une suffit), sa place avec
//! l'ancien classement (celui de PowerShell) et la place à ne pas dépasser maintenant (3, ou sa place d'aujourd'hui
//! quand le catalogue n'a pas les mots qu'il faudrait).
//!
//! Quatre lots : celui qui a servi à régler le classement (src\recherche.rs), puis trois lots jamais utilisés pour le
//! régler (le premier mis de côté avant tout réglage, les deux autres écrits ensuite) : réussir ses propres phrases ne
//! prouve rien.

mod commun;

use commun::catalogue;
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{Objet, chercher, chercher_comme_powershell};

/// Une phrase tapée, les pannes attendues (séparées par « | »), sa place avant, la place à ne pas dépasser.
type Phrase = (&'static str, &'static str, usize, usize);

/// Le lot qui a servi à régler.
#[rustfmt::skip]
const REGLAGE: &[Phrase] = &[
    ("il s'éteint tout seul", "seteint", 6, 3),
    ("mon pc s'éteint tout seul", "seteint", 6, 3),
    ("il s'éteint d'un coup", "seteint", 1, 3),
    ("écran bleu", "ecranbleu", 1, 3),
    ("plus de son", "plusdeson", 1, 3),
    ("le wifi marche plus", "wifiabsent|wificoupe|plusinternet", 29, 3),
    ("il rame", "toutlent|rameheures|apresmaj", 1, 3),
    ("ça sent le brûlé", "odeurbrule", 1, 3),
    ("mon ordi ne s'allume plus", "rien", 1, 3),
    ("il fait beaucoup de bruit", "ventilobruyant|cliquetis|sifflement|vibration", 15, 3),
    ("l'écran est tout noir", "ecrannoir", 5, 3),
    ("j'ai plus internet", "plusinternet", 3, 3),
    ("le clavier marche plus", "clavier|touches", 2, 3),
    ("la souris bouge plus", "souris", 2, 3),
    ("mon portable charge pas", "necharge|portablecharge", 1, 3),
    ("la batterie dure pas longtemps", "tientpeu", 7, 10),
    ("il chauffe trop", "chauffe", 4, 3),
    ("j'ai renversé du café dessus", "liquide", 2, 3),
    ("l'imprimante marche pas", "imprimante", 1, 3),
    ("il est très lent", "toutlent", 9, 3),
    ("des pubs partout", "pubs", 1, 3),
    ("j'ai oublié mon mot de passe", "mdpoublie", 1, 3),
    ("il redémarre tout seul", "redemarre", 1, 3),
    ("le son grésille", "songresille", 1, 3),
    ("la webcam marche pas", "webcam", 1, 3),
    ("il est tombé par terre", "chute", 1, 3),
    ("le micro marche pas", "micro", 1, 3),
    ("il se bloque", "fige", 9, 3),
    ("le disque est plein", "disqueplein|cpleindvide", 1, 3),
    ("le bluetooth marche pas", "bluetoothperiph|bluetoothaudio|bluetoothdisparu", 1, 3),
];

/// Mis de côté avant tout réglage.
#[rustfmt::skip]
const DE_COTE: &[Phrase] = &[
    ("mon pc s'arrête tout seul", "seteint", 26, 4),
    ("le pc se coupe sans prévenir", "seteint", 16, 5),
    ("y a plus de son sur mon pc", "plusdeson", 2, 3),
    ("écran noir", "ecrannoir", 5, 3),
    ("internet marche plus", "plusinternet", 12, 3),
    ("le ventilateur fait un bruit énorme", "ventilobruyant", 2, 3),
    ("la batterie se décharge vite", "tientpeu", 1, 3),
    ("l'écran clignote", "clignote", 1, 3),
    ("les touches marchent plus", "touches", 1, 3),
    ("ça sent le cramé", "odeurbrule", 1, 3),
    ("il est devenu super lent", "toutlent", 1, 3),
    ("le wifi coupe tout le temps", "wificoupe", 1, 3),
    ("la souris clique toute seule", "doubleclic|cliquesfantomes", 1, 3),
    ("mon pc a pris l'eau", "liquide", 1, 3),
    ("mon ordinateur s'éteint tout seul quand je joue", "seteint|jeu", 2, 3),
];

/// Écrit après le réglage.
#[rustfmt::skip]
const NEUVES: &[Phrase] = &[
    ("mon écran est bleu avec un message", "ecranbleu", 1, 3),
    ("le son ne marche plus", "plusdeson", 11, 3),
    ("le pc fait un bruit bizarre", "ventilobruyant|cliquetis|sifflement|vibration", 14, 7),
    ("ça chauffe énormément", "chauffe", 1, 3),
    ("il s'allume pas", "rien", 1, 3),
    ("le wifi se déconnecte", "wificoupe", 1, 3),
    ("la batterie charge plus", "necharge", 2, 3),
    ("je n'ai plus de connexion internet", "plusinternet", 1, 3),
    ("le clavier écrit tout seul", "toucherepete", 1, 3),
    ("mon pc est lent", "toutlent", 1, 3),
    ("il plante tout le temps", "programme|fige|ecranbleu|seteint|redemarre", 1, 3),
    ("l'écran reste noir", "ecrannoir", 10, 5),
    ("ça s'éteint sans raison", "seteint", 6, 3),
    ("de la fumée sort du pc", "odeurbrule", 4, 3),
];

/// Écrit après le dernier changement (les mots « devenu » et « temps »).
#[rustfmt::skip]
const TROISIEME: &[Phrase] = &[
    ("mon ordi se met à redémarrer tout seul", "redemarre", 1, 3),
    ("il y a un bip quand j'allume", "bipsdemarrage", 5, 3),
    ("j'entends plus rien dans le casque", "casque|plusdeson", 16, 3),
    ("l'image est floue", "flou", 1, 3),
    ("la souris ne bouge plus du tout", "souris", 2, 3),
    ("le pc ne trouve pas le wifi", "wifiabsent", 3, 3),
    ("le chargeur devient très chaud", "chargeurchauffe", 1, 3),
    ("il met des heures à démarrer", "demarragelong", 1, 5),
    ("un virus sur mon pc", "pubs|programmesinconnus|fauxnettoyage|antivirusoff", 1, 3),
    ("mon pc s'est éteint d'un coup et ne redémarre plus", "seteint|rien|orage", 1, 3),
    ("le pc ne s'éteint jamais", "nesteintpas", 3, 3),
    ("ça coupe tout seul", "seteint|wificoupe|songresille|sansfilcoupe", 7, 3),
];
/// La place de la première panne attendue (1 = en tête ; 0 = pas trouvée).
fn place(pannes: &[&Objet], attendues: &str) -> usize {
    let bonnes: Vec<&str> = attendues.split('|').collect();
    pannes.iter().position(|p| bonnes.contains(&texte(champ(p, "Id")).as_str())).map_or(0, |i| i + 1)
}

fn lots() -> [(&'static str, &'static [Phrase]); 4] {
    [("réglage", REGLAGE), ("mis de côté", DE_COTE), ("écrit après", NEUVES), ("troisième", TROISIEME)]
}

#[test]
fn chaque_phrase_trouve_sa_panne_dans_les_premieres() {
    let cat = catalogue();
    let mut ecarts = Vec::new();
    for (lot, phrases) in lots() {
        for &(phrase, attendues, avant, au_plus) in phrases {
            let ancienne = place(&chercher_comme_powershell(cat, phrase), attendues);
            if ancienne != avant {
                ecarts.push(format!("« {phrase} » ({lot}) : {ancienne}e avec l'ancien classement, {avant}e attendu"));
            }
            let trouvees = chercher(cat, phrase);
            let p = place(&trouvees, attendues);
            if p == 0 || p > au_plus {
                let premieres: Vec<String> = trouvees.iter().take(5).map(|x| texte(champ(x, "Id"))).collect();
                ecarts
                    .push(format!("« {phrase} » ({lot}) : {attendues} est {p}e (au plus {au_plus}e) : {premieres:?}"));
            }
        }
    }
    assert!(ecarts.is_empty(), "{} phrases mal classées :\n{}", ecarts.len(), ecarts.join("\n"));
}

#[test]
fn le_nouveau_classement_fait_mieux_sur_chaque_lot() {
    let cat = catalogue();
    for (lot, phrases) in lots() {
        assert!(phrases.len() >= 10, "{lot} : trop peu de phrases");
        let (mut avant, mut apres) = ((0, 0), (0, 0));
        for &(phrase, attendues, _, _) in phrases {
            let a = place(&chercher_comme_powershell(cat, phrase), attendues);
            let n = place(&chercher(cat, phrase), attendues);
            avant = (avant.0 + usize::from(a == 1), avant.1 + usize::from((1..=3).contains(&a)));
            apres = (apres.0 + usize::from(n == 1), apres.1 + usize::from((1..=3).contains(&n)));
        }
        assert!(
            apres.0 > avant.0 && apres.1 > avant.1,
            "{lot} : en tête {} puis {}, dans les 3 premières {} puis {}",
            avant.0,
            apres.0,
            avant.1,
            apres.1
        );
    }
}

#[test]
fn il_s_eteint_tout_seul() {
    let cat = catalogue();
    for phrase in ["il s'éteint tout seul", "mon pc s'éteint tout seul", "Il s'éteint tout seul !"] {
        let trouvees = chercher(cat, phrase);
        assert_eq!(trouvees.first().map(|p| texte(champ(p, "Id"))).as_deref(), Some("seteint"), "« {phrase} »");
    }
}
