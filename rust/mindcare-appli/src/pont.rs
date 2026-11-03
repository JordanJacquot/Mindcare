//! Le pont entre le kit d'écrans de la suite (ui\kit.js, dessiné dans la page) et l'appli, comme vue.ps1 le fait pour
//! les applis PowerShell :
//!
//! - [`lire_message`] : ce que la page envoie par la commande `action` (un geste, l'écran montré, « Retour » depuis
//!   l'accueil...), vérifié champ par champ ; tout le reste est refusé ;
//! - [`Habillage`] : ce que vue.ps1 ajoute de lui-même à l'état (`Send-Vue`) : le thème de la suite (`Get-ThemeVue`),
//!   les couleurs d'un thème personnalisé (`Get-CouleursVue`) et ses images (`Get-PackVue`, adresses
//!   `http://pack.localhost/...` du protocole `pack`) ;
//! - [`demander_menu`] : « Retour » sur l'accueil, dans Mindset, ramène à sa console (`Request-MenuMindset`) ;
//! - la construction d'essai seulement : [`etat_essai`], l'accueil des maquettes (jamais dans l'exe livré).
//!
//! Les pages elles-mêmes (l'état du kit : accueil, pages) sont écrites ailleurs (src\pages).

use std::path::Path;

use mindcare_vie::mindset::AFFICHER_PAGE;
use mindcare_vie::{Disque, Systeme};
use serde_json::{Map, Value, json};
use suite_commun::themes::Theme;

/// Un message plus gros (en JSON) est refusé : un geste du kit fait quelques centaines d'octets, une recherche tapée
/// quelques milliers au plus.
pub const TAILLE_MAX: usize = 16 * 1024;
/// La profondeur la plus grande de `valeur` (le kit envoie un texte, un nombre, un oui ou non, ou `{ valeur, oui }`)
pub const PROFONDEUR_MAX: usize = 4;
/// Le texte d'une erreur de la page, coupé à cette longueur (le kit coupe aussi)
pub const ERREUR_MAX: usize = 300;

/// Un message du kit, vérifié (kit.js : « Messages envoyés à l'appli »).
#[derive(Debug, Clone, PartialEq)]
pub enum MessagePage {
    /// La page est chargée et le kit prêt (`{ type: 'pret' }`)
    Pret,
    /// Un geste : l'id d'une tuile, d'un bouton, d'une bascule, d'un champ... avec sa valeur, sur cet écran
    Action { id: String, valeur: Value, ecran: String },
    /// L'écran montré a changé (« accueil » ou l'id d'une page)
    Ecran { id: String },
    /// « Retour » depuis l'accueil : dans Mindset, retour à sa console
    Menu,
    /// Une erreur de la page (pour le journal)
    Erreur { texte: String },
    /// La réponse à une question posée par l'appli (`{ kit: 'question', id, ... }`) : l'id du bouton choisi
    Reponse { id: String, valeur: String },
}

/// Pourquoi un message est refusé (pour le journal ; la page n'en sait rien)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refus {
    /// Pas un objet JSON
    PasUnObjet,
    /// Plus de [`TAILLE_MAX`] octets
    TropGros,
    /// `type` absent ou inconnu
    Type,
    /// Un champ manque, ou n'a pas le bon type, ou est mal écrit
    Champ(&'static str),
    /// Un champ que ce type de message n'a pas
    ChampEnTrop,
    /// `valeur` plus profonde que [`PROFONDEUR_MAX`]
    TropProfond,
}

impl Refus {
    pub fn raison(self) -> String {
        match self {
            Refus::PasUnObjet => "pas un objet".into(),
            Refus::TropGros => "trop gros".into(),
            Refus::Type => "type inconnu".into(),
            Refus::Champ(c) => format!("champ {c} absent ou mal écrit"),
            Refus::ChampEnTrop => "champ en trop".into(),
            Refus::TropProfond => "valeur trop profonde".into(),
        }
    }
}

/// Un id du kit (tuile, bouton, page, question...) : 1 à 60 lettres, chiffres, `_`, `:`, `.` ou `-`, comme vue.ps1
/// (`^[\w:.-]{1,60}$`).
pub fn id_valable(id: &str) -> bool {
    let n = id.chars().count();
    (1..=60).contains(&n) && id.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '.' | '-'))
}

fn profondeur(v: &Value) -> usize {
    match v {
        Value::Array(l) => 1 + l.iter().map(profondeur).max().unwrap_or(0),
        Value::Object(m) => 1 + m.values().map(profondeur).max().unwrap_or(0),
        _ => 0,
    }
}

/// Lit un message de la page : un objet `{ type, ... }` avec seulement les champs de son type, chacun du bon type.
pub fn lire_message(message: &Value) -> Result<MessagePage, Refus> {
    let Value::Object(m) = message else { return Err(Refus::PasUnObjet) };
    // (un objet se réécrit toujours en JSON)
    if serde_json::to_vec(message).map_or(usize::MAX, |o| o.len()) > TAILLE_MAX {
        return Err(Refus::TropGros);
    }
    let type_ = m.get("type").and_then(Value::as_str).ok_or(Refus::Type)?;
    let permis: &[&str] = match type_ {
        "pret" | "menu" => &["type"],
        "action" => &["type", "id", "valeur", "ecran"],
        "ecran" => &["type", "id"],
        "erreur" => &["type", "texte"],
        "reponse" => &["type", "id", "valeur"],
        _ => return Err(Refus::Type),
    };
    if m.keys().any(|k| !permis.contains(&k.as_str())) {
        return Err(Refus::ChampEnTrop);
    }
    let id = |champ: &'static str| -> Result<String, Refus> {
        m.get(champ).and_then(Value::as_str).filter(|i| id_valable(i)).map(str::to_string).ok_or(Refus::Champ(champ))
    };
    Ok(match type_ {
        "pret" => MessagePage::Pret,
        "menu" => MessagePage::Menu,
        "ecran" => MessagePage::Ecran { id: id("id")? },
        "reponse" => MessagePage::Reponse { id: id("id")?, valeur: id("valeur")? },
        "erreur" => {
            let t = m.get("texte").and_then(Value::as_str).ok_or(Refus::Champ("texte"))?;
            MessagePage::Erreur { texte: t.chars().take(ERREUR_MAX).collect() }
        }
        _ => {
            let valeur = m.get("valeur").cloned().unwrap_or(Value::Null);
            if profondeur(&valeur) > PROFONDEUR_MAX {
                return Err(Refus::TropProfond);
            }
            // (le kit envoie toujours l'écran d'où vient le geste)
            MessagePage::Action { id: id("id")?, valeur, ecran: id("ecran")? }
        }
    })
}

/// Un texte venu de la page, coupé pour le journal
fn court(t: &str) -> String {
    t.chars().take(120).collect()
}

/// La ligne du journal d'un message (le receveur par défaut, en attendant celui du programme)
pub fn ligne_journal(m: &MessagePage) -> String {
    match m {
        MessagePage::Pret => "Écran prêt".into(),
        MessagePage::Action { id, valeur, ecran } => {
            format!("Écran : action {id} (écran {ecran}, valeur {})", court(&valeur.to_string()))
        }
        MessagePage::Ecran { id } => format!("Écran : montre {id}"),
        MessagePage::Menu => "Écran : retour au menu de Mindset demandé".into(),
        MessagePage::Erreur { texte } => format!("Écran : erreur ({})", court(texte)),
        MessagePage::Reponse { id, valeur } => format!("Écran : réponse {valeur} à la question {id}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Le thème pour l'écran
// ---------------------------------------------------------------------------------------------------------------

/// Une couleur que la page accepte : « #RRGGBB » (celles des thèmes de suite-commun, vérifiées à la lecture).
pub fn couleur_valable(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// Un nom de variable CSS que la page accepte : des lettres et des chiffres (« P », « fond2 »).
pub fn nom_valable(n: &str) -> bool {
    (1..=16).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Des variables CSS du thème en objet JSON ; une variable au nom ou à la couleur mal écrits n'est pas passée.
pub fn couleurs_json(variables: &[(&str, String)]) -> Map<String, Value> {
    let mut couleurs = Map::new();
    for (nom, valeur) in variables {
        if nom_valable(nom) && couleur_valable(valeur) {
            couleurs.insert((*nom).to_string(), json!(valeur));
        }
    }
    couleurs
}

/// Ce que vue.ps1 ajoute de lui-même à l'état (`Send-Vue`) : le thème de la suite, les couleurs et les images d'un
/// thème personnalisé. Posé une fois par le programme ([`crate::Page::habiller`]) ; l'état des pages n'a pas à s'en
/// occuper.
#[derive(Debug, Clone, PartialEq)]
pub struct Habillage {
    /// Un des 5 thèmes intégrés (`data-theme` du kit)
    pub theme: &'static str,
    /// Les couleurs propres d'un pack (`--P`, `--S`...), sinon rien
    pub couleurs: Option<Map<String, Value>>,
    /// Les images d'un thème personnalisé (`pack` du kit), sinon rien
    pub pack: Option<Value>,
}

impl Habillage {
    /// D'après le thème chargé (`soir` : la phrase d'accueil dit « Bonsoir »)
    pub fn depuis(theme: &Theme, soir: bool) -> Habillage {
        Habillage {
            theme: theme.theme_ecran(),
            couleurs: theme.couleurs_ecran().map(|v| couleurs_json(&v)),
            pack: theme.pack.as_ref().map(|p| crate::pack::pack_ecran(p, soir)),
        }
    }

    /// Ajoute à l'état ce qui y manque (comme `Send-Vue` : l'appli peut choisir elle-même). Un ordre (`kit`) ou autre
    /// chose qu'un objet ne change pas.
    pub fn poser(&self, etat: &mut Value) {
        let Value::Object(m) = etat else { return };
        if m.contains_key("kit") {
            return;
        }
        m.entry("theme").or_insert_with(|| json!(self.theme));
        if let Some(c) = &self.couleurs {
            m.entry("couleurs").or_insert_with(|| Value::Object(c.clone()));
        }
        if let Some(p) = &self.pack {
            m.entry("pack").or_insert_with(|| p.clone());
        }
    }
}

/// Le soir, pour la phrase d'accueil d'un thème personnalisé : de 18 h à 5 h (vue.ps1, `Get-PackVue`)
pub fn soir(heure: u32) -> bool {
    !(5..18).contains(&heure)
}

// ---------------------------------------------------------------------------------------------------------------
// Retour à la console de Mindset
// ---------------------------------------------------------------------------------------------------------------

/// Le signal de Mindset : levé, il relit `afficher-page.txt` (mindset.ps1 du squelette, `Request-MenuMindset`)
pub const SIGNAL_MINDSET: &str = r"Local\Mindset-Afficher";
/// La page « menu » de Mindset : sa console
pub const PAGE_MENU: &str = "menu";

/// `Request-MenuMindset` (mindset.ps1 du squelette) : « menu » dans `<dossier_mindset>\afficher-page.txt`, puis le
/// signal de Mindset (rien n'est lancé : il est déjà ouvert, l'appli est dans sa fenêtre). Seulement quand l'appli est
/// affichée dans Mindset (au programme de le vérifier : `Vie::dans_mindset`). La construction d'essai écrit le
/// fichier dans son faux dossier et ne lève JAMAIS le signal (ce serait celui du vrai Mindset du PC). Rend la ligne
/// du journal.
pub fn demander_menu(dossier_mindset: &Path, systeme: &mut dyn Systeme, disque: &mut dyn Disque) -> String {
    let fichier = dossier_mindset.join(AFFICHER_PAGE);
    if let Err(e) = disque.creer_dossier(dossier_mindset).and_then(|()| disque.ecrire_texte(&fichier, PAGE_MENU)) {
        return format!("Retour au menu de Mindset impossible ({e})");
    }
    #[cfg(not(feature = "essai"))]
    systeme.lever_signal(SIGNAL_MINDSET);
    #[cfg(feature = "essai")]
    let _ = systeme;
    "Retour au menu de Mindset demandé".into()
}

// ---------------------------------------------------------------------------------------------------------------
// Construction d'essai : l'accueil des maquettes
// ---------------------------------------------------------------------------------------------------------------

/// L'état d'essai (construction d'essai seulement, jamais livrée) : l'accueil des maquettes validées (tuiles
/// « Raconte ton problème » et « Ça concerne quoi ? », rond « Comment ça marche »), la page « Raconte ton problème »,
/// la page « Comment ça marche », et les 15 familles en jaquettes (pour essayer les images de ui\images). La tuile
/// « Ça concerne quoi ? » n'a pas de page ici : un clic envoie son action (vue dans le journal).
#[cfg(feature = "essai")]
pub fn etat_essai(version: &str) -> Value {
    const FAMILLES: [(&str, &str, &str); 15] = [
        ("allumage", "Allumage", "Rien ne s'allume, des bips, il s'éteint aussitôt"),
        ("voyants", "Voyants, lumières et bips", "Une lumière sur la carte mère, un voyant qui clignote, des bips"),
        ("demarrage", "Démarrage de Windows", "Reste sur le BIOS, réparation en boucle, disque introuvable"),
        ("ecran", "Écran et image", "Noir, pas de signal, clignote, des traits"),
        ("plantages", "Plantages", "Écran bleu, redémarre, se fige"),
        ("jeux", "Jeux vidéo", "Le jeu ne se lance pas, saccade, lague en ligne"),
        ("lenteur", "Lenteur", "Tout rame, démarrage long, jeux qui saccadent"),
        ("bruits", "Bruits et chaleur", "Ventilateur fort, clics, chauffe, odeur"),
        ("son", "Son", "Plus de son, grésille, micro"),
        ("internet", "Internet et Wi-Fi", "Plus d'internet, Wi-Fi absent ou qui coupe"),
        ("peripheriques", "Clavier, souris et USB", "Ne répond plus, clé non reconnue"),
        ("batterie", "Batterie et charge", "Ne charge plus, tient peu"),
        ("virus", "Virus et pubs", "Fenêtres bizarres, navigateur détourné"),
        ("fichiers", "Fichiers et disques", "Disque plein, fichier perdu, disque externe"),
        ("windows", "Windows et compte", "Mot de passe oublié, mises à jour, menu Démarrer, heure"),
    ];
    let couleurs = ["P", "S", "T"];
    let familles: Vec<Value> = FAMILLES
        .iter()
        .enumerate()
        .map(|(i, (id, titre, phrase))| {
            json!({ "id": format!("famille:{id}"), "titre": titre, "sous": phrase,
                    "image": format!("images/famille-{id}.svg"), "couleur": couleurs[i % 3] })
        })
        .collect();
    // (une page jamais ouverte, pour que l'état dépasse 8 Ko : au-delà, le canal de Tauri passe par « fetch » au lieu
    // d'un script, comme ce sera le cas avec les vraies pages)
    let lignes: Vec<Value> = (1..=60)
        .map(|n| json!({ "type": "ligne", "texte": format!("Ligne d'essai {n}"), "note": "Pour un état de plus de 8 Ko" }))
        .collect();
    json!({
        "app": { "nom": crate::NOM_APPLI, "version": version, "logo": "logo.svg" },
        "sons": true,
        "accueil": {
            "tuiles": [
                { "id": "raconte", "nom": "Raconte ton problème", "icone": "loupe", "sous": "Avec tes mots ou ta voix", "couleur": "P" },
                { "id": "familles", "nom": "Ça concerne quoi ?", "icone": "liste", "sous": "15 familles, 300 pannes", "couleur": "S" },
            ],
            "ronds": [{ "id": "aide", "nom": "Comment ça marche", "icone": "aide", "couleur": "T" }],
        },
        "pages": {
            "essai-gros": { "titre": "Essai", "blocs": lignes },
            "raconte": {
                "titre": "Raconte ton problème", "sous": "Avec tes mots : Mindcare trouve la bonne enquête.",
                "icone": "loupe", "couleur": "P",
                "blocs": [
                    { "type": "champ", "id": "recherche", "recherche": true, "valeur": "",
                      "indice": "Par exemple : « bip », « le son grésille », « écran bleu »..." },
                    { "type": "titre", "texte": "Ou choisis ce qui est concerné" },
                    { "type": "grille", "elements": familles },
                ],
            },
            "aide": {
                "titre": "Comment ça marche",
                "sous": "Mindcare trouve pourquoi ton PC est en panne, en répondant à quelques questions simples.",
                "icone": "aide", "couleur": "T",
                "module": { "icone": "aide", "couleur": "T", "titre": "Une enquête",
                            "texte": "Une question à la fois : chaque réponse écarte des suspects, jusqu'à la bonne piste." },
                "blocs": [
                    { "type": "image", "url": "images/ecran-bleu.svg", "hauteur": 160 },
                    { "type": "faq", "elements": [
                        { "question": "Est-ce que Mindcare répare mon PC ?", "icone": "bouclier",
                          "reponse": "Non. Elle t'explique quoi faire, montre les commandes à taper avec un bouton Copier, mais ne lance jamais rien toute seule." },
                        { "question": "Mes réponses sont-elles envoyées ?", "icone": "coeur",
                          "reponse": "Non. Les 10 dernières enquêtes restent sur ce PC." },
                    ] },
                ],
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::io;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn messages_du_kit_lus() {
        assert_eq!(lire_message(&json!({ "type": "pret" })), Ok(MessagePage::Pret));
        assert_eq!(lire_message(&json!({ "type": "menu" })), Ok(MessagePage::Menu));
        assert_eq!(
            lire_message(&json!({ "type": "action", "id": "famille:plantages", "ecran": "accueil" })),
            Ok(MessagePage::Action { id: "famille:plantages".into(), valeur: Value::Null, ecran: "accueil".into() })
        );
        assert_eq!(
            lire_message(
                &json!({ "type": "action", "id": "geste", "valeur": { "valeur": "a", "oui": true }, "ecran": "enquete" })
            ),
            Ok(MessagePage::Action {
                id: "geste".into(),
                valeur: json!({ "valeur": "a", "oui": true }),
                ecran: "enquete".into()
            })
        );
        assert_eq!(
            lire_message(&json!({ "type": "action", "id": "recherche", "valeur": "écran bleu", "ecran": "raconte" })),
            Ok(MessagePage::Action { id: "recherche".into(), valeur: json!("écran bleu"), ecran: "raconte".into() })
        );
        assert_eq!(
            lire_message(&json!({ "type": "ecran", "id": "raconte" })),
            Ok(MessagePage::Ecran { id: "raconte".into() })
        );
        assert_eq!(
            lire_message(&json!({ "type": "reponse", "id": "a1b2c3d4e5", "valeur": "oui" })),
            Ok(MessagePage::Reponse { id: "a1b2c3d4e5".into(), valeur: "oui".into() })
        );
        let long = "x".repeat(500);
        assert_eq!(
            lire_message(&json!({ "type": "erreur", "texte": long })),
            Ok(MessagePage::Erreur { texte: "x".repeat(ERREUR_MAX) })
        );
    }

    /// Contre-épreuve : chaque message mal formé est refusé, avec sa raison.
    #[test]
    fn messages_mal_formes_refuses() {
        for (m, refus) in [
            (json!("pret"), Refus::PasUnObjet),
            (json!(["pret"]), Refus::PasUnObjet),
            (json!({}), Refus::Type),
            (json!({ "type": 3 }), Refus::Type),
            (json!({ "type": "ouvrir" }), Refus::Type),
            (json!({ "type": "pret", "fichier": "C:\\Windows" }), Refus::ChampEnTrop),
            (json!({ "type": "action", "ecran": "accueil" }), Refus::Champ("id")),
            (json!({ "type": "action", "id": "", "ecran": "accueil" }), Refus::Champ("id")),
            (json!({ "type": "action", "id": "<script>", "ecran": "accueil" }), Refus::Champ("id")),
            (json!({ "type": "action", "id": "a b", "ecran": "accueil" }), Refus::Champ("id")),
            (json!({ "type": "action", "id": "x".repeat(61), "ecran": "accueil" }), Refus::Champ("id")),
            (json!({ "type": "action", "id": 7, "ecran": "accueil" }), Refus::Champ("id")),
            (json!({ "type": "action", "id": "ok" }), Refus::Champ("ecran")),
            (json!({ "type": "action", "id": "ok", "ecran": "../x" }), Refus::Champ("ecran")),
            (json!({ "type": "action", "id": "ok", "ecran": "a", "valeur": [[[[[1]]]]] }), Refus::TropProfond),
            (json!({ "type": "action", "id": "ok", "ecran": "a", "valeur": "x".repeat(TAILLE_MAX) }), Refus::TropGros),
            (json!({ "type": "ecran" }), Refus::Champ("id")),
            (json!({ "type": "erreur", "texte": 1 }), Refus::Champ("texte")),
            (json!({ "type": "reponse", "id": "q", "valeur": true }), Refus::Champ("valeur")),
        ] {
            assert_eq!(lire_message(&m), Err(refus), "{m}");
        }
        // (à la limite : 4 niveaux passent)
        assert!(lire_message(&json!({ "type": "action", "id": "ok", "ecran": "a", "valeur": [[[[1]]]] })).is_ok());
    }

    #[test]
    fn habillage_comme_send_vue() {
        let h = Habillage::depuis(&suite_commun::themes::charger(Some("crepuscule"), &[]), false);
        assert_eq!(h.theme, "crepuscule");
        assert_eq!(h.couleurs, None);
        assert_eq!(h.pack, None);
        let mut e = json!({ "app": { "nom": "Mindcare" } });
        h.poser(&mut e);
        assert_eq!(e["theme"], "crepuscule");
        assert!(e.get("couleurs").is_none() && e.get("pack").is_none());
        // Ce que l'appli a choisi elle-même reste ; un ordre n'est pas touché
        let mut e = json!({ "theme": "rubis" });
        h.poser(&mut e);
        assert_eq!(e["theme"], "rubis");
        let mut o = json!({ "kit": "message", "texte": "Bravo" });
        h.poser(&mut o);
        assert_eq!(o, json!({ "kit": "message", "texte": "Bravo" }));
        // Néon par défaut
        assert_eq!(Habillage::depuis(&Theme::neon(), false).theme, "neon");
    }

    #[test]
    fn couleurs_mal_ecrites_ecartees() {
        let v = [
            ("P", "#00E5FF".to_string()),
            ("S", "red".to_string()),
            ("T", "#FF2EB5;background:url(x)".to_string()),
            ("fond", "#80112233".to_string()),
            ("x}y", "#000000".to_string()),
            ("", "#000000".to_string()),
        ];
        assert_eq!(Value::Object(couleurs_json(&v)), json!({ "P": "#00E5FF" }));
    }

    /// L'état d'essai passe par le « fetch » du canal (plus de 8 Ko), et ses images sont celles de ui\images
    #[cfg(feature = "essai")]
    #[test]
    fn etat_d_essai() {
        let e = etat_essai("0.1.0");
        assert!(e.to_string().len() > 8192);
        let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
        for f in e["pages"]["raconte"]["blocs"][2]["elements"].as_array().unwrap() {
            assert!(ui.join(f["image"].as_str().unwrap()).is_file(), "{f}");
        }
    }

    #[test]
    fn le_soir_de_18_h_a_5_h() {
        assert!(soir(18) && soir(23) && soir(0) && soir(4));
        assert!(!soir(5) && !soir(12) && !soir(17));
    }

    /// Un faux Windows : seulement ce que `demander_menu` peut toucher
    #[derive(Default)]
    struct Faux {
        signaux: Vec<String>,
        ecrits: RefCell<Vec<(PathBuf, String)>>,
        refuser: bool,
    }
    impl Systeme for Faux {
        fn creer_verrou(&mut self, _: &str) -> bool {
            unreachable!()
        }
        fn attendre_verrou(&mut self, _: Duration) -> mindcare_vie::AttenteVerrou {
            unreachable!()
        }
        fn liberer_verrou(&mut self) {}
        fn lever_signal(&mut self, nom: &str) {
            self.signaux.push(nom.into());
        }
        fn creer_signal(&mut self, _: &str) {}
        fn signal_recu(&mut self) -> bool {
            false
        }
        fn fermer_signal(&mut self) {}
        fn emplacement_mindset(&mut self) -> Option<String> {
            None
        }
        fn lancer(&mut self, _: &Path, _: &[&str], _: Option<&Path>) -> io::Result<()> {
            panic!("rien n'est lancé pour revenir au menu")
        }
    }
    struct Disq<'a>(&'a Faux);
    impl Disque for Disq<'_> {
        fn existe(&self, _: &Path) -> bool {
            false
        }
        fn est_fichier(&self, _: &Path) -> bool {
            false
        }
        fn effacer(&mut self, _: &Path) -> io::Result<()> {
            Ok(())
        }
        fn date(&self, _: &Path) -> mindcare_vie::Date {
            None
        }
        fn lire_suite(&mut self, _: &Path, _: u32) -> mindcare_donnees::dossier::Suite {
            Default::default()
        }
        fn creer_dossier(&mut self, _: &Path) -> io::Result<()> {
            if self.0.refuser { Err(io::Error::other("refusé")) } else { Ok(()) }
        }
        fn ecrire_texte(&mut self, chemin: &Path, texte: &str) -> io::Result<()> {
            self.0.ecrits.borrow_mut().push((chemin.to_path_buf(), texte.into()));
            Ok(())
        }
        fn ouvrir_donnees(&mut self, _: &Path) {}
        fn ecrire_etat(&mut self) {}
        fn journal(&mut self, _: &str) {}
    }

    #[test]
    fn retour_au_menu_de_mindset() {
        let dossier = PathBuf::from(r"C:\Essai\Local\Mindset");
        let mut s = Faux::default();
        let d = Faux::default();
        let ligne = demander_menu(&dossier, &mut s, &mut Disq(&d));
        assert_eq!(ligne, "Retour au menu de Mindset demandé");
        assert_eq!(*d.ecrits.borrow(), vec![(dossier.join("afficher-page.txt"), "menu".to_string())]);
        // Le signal du vrai Mindset : jamais dans la construction d'essai
        if cfg!(feature = "essai") {
            assert!(s.signaux.is_empty());
        } else {
            assert_eq!(s.signaux, vec![r"Local\Mindset-Afficher".to_string()]);
        }
        // Le fichier ne s'écrit pas : pas de signal non plus
        let mut s = Faux::default();
        let d = Faux { refuser: true, ..Faux::default() };
        assert!(demander_menu(&dossier, &mut s, &mut Disq(&d)).starts_with("Retour au menu de Mindset impossible"));
        assert!(s.signaux.is_empty() && d.ecrits.borrow().is_empty());
    }
}
