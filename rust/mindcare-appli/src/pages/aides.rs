//! L'assistant et la dictée dans les pages (étape 6.4 du plan « Mindcare en Rust ») : le bouton Parler, les lignes
//! « La dictée » et « L'assistant » de « Raconte ton problème », « Demander à l'assistant », et la page « Pour être
//! sûr ». Les textes et les règles sont ceux de Mindcare PowerShell (modules\contenu.ps1 : `Update-ZoneDictee`,
//! `Update-BandeauIa`, `Request-Ia`, `Show-PourEtreSur`), la place des lignes celle de la maquette validée.
//!
//! Comme le reste des pages : rien n'est lancé ici. Les pages lisent une photo de l'assistant, de la dictée et de leurs
//! installations ([`Aides`], prise par effets.rs), et rendent des effets (installer, parler, demander...).

use serde_json::{Value, json};

use mindcare_assistant::Choix;
use mindcare_assistant::Etat as EtatAssistant;
use mindcare_assistant::decision::pour_etre_sur as pes;
use mindcare_dictee::Etat as EtatDictee;
use mindcare_telechargement::Etat as EtatInstallation;

use super::Etat;

/// Les ids des actions de l'assistant et de la dictée.
pub mod ids {
    /// Le bouton Parler (en haut à droite de « Raconte ton problème »)
    pub const PARLER: &str = "parler";
    /// Installer, réparer, mettre à jour ou réessayer l'assistant
    pub const INSTALLER_IA: &str = "installer-ia";
    /// Installer, réparer ou réessayer la dictée
    pub const INSTALLER_DICTEE: &str = "installer-dictee";
    /// « Demander à l'assistant » (Entrée dans la recherche, `Request-Ia`)
    pub const DEMANDER: &str = "entree";
    /// Une réponse de « Pour être sûr » (valeur : son numéro)
    pub const IDEE: &str = "idee";
    /// « Aucune de ces phrases »
    pub const AUCUNE: &str = "aucune";
}

/// L'id de la page « Pour être sûr ».
pub const POUR_ETRE_SUR: &str = "pourEtreSur";

/// Les textes (ceux du PowerShell ; sinon ceux de la maquette validée).
pub mod textes {
    /// Le bouton Parler, et pendant l'écoute (`Set-BoutonMicroAllume`)
    pub const PARLER: &str = "Parler";
    pub const ARRETER: &str = "Arrêter d'écouter";
    /// `Show-DicteeEcoute`
    pub const DICTEE_ECOUTE: &str = "Je t'écoute. Dis ce qui ne va pas : j'écris ta phrase dès que tu te tais.";
    /// `Show-DicteeEcrit`
    pub const DICTEE_ECRIT: &str = "J'écris ce que tu as dit...";
    /// `Set-DicteeTexte`
    pub const DICTEE_COMPRIS: &str = "Voilà ce que j'ai compris. Corrige si besoin, puis appuie sur Entrée.";
    /// La dictée prête (la maquette : le bouton s'appelle « Parler », plus « le micro à droite de la barre »)
    pub const DICTEE_PRETE: &str = "La dictée est prête : appuie sur « Parler » et dis ce qui ne va pas.";
    /// Parler sans la dictée installée (la maquette)
    pub const INSTALLE_D_ABORD: &str = "Installe d'abord la dictée";
    /// Pendant que l'assistant lit la phrase (`Request-Ia`)
    pub const LIT: &str = "Je lis ta phrase...";
    /// Le bouton qui pose la phrase (la maquette)
    pub const DEMANDER: &str = "Demander à l'assistant";
    /// L'indice du champ quand l'assistant est installé (`Show-Accueil`)
    pub const INDICE_ASSISTANT: &str =
        "Raconte ton problème avec tes mots, puis appuie sur Entrée : « mon pc s'éteint quand je joue »...";
    /// Les noms des lignes
    pub const LIGNE_DICTEE: &str = "La dictée";
    pub const LIGNE_ASSISTANT: &str = "L'assistant";

    fn sans_point(m: &str) -> &str {
        m.trim_end_matches('.')
    }

    pub fn ia_en_cours(pourcent: u8, etape: &str) -> String {
        format!("Installation de l'assistant : {pourcent} % ({etape}). Tu peux continuer à te servir de Mindcare.")
    }
    pub fn ia_erreur(m: &str) -> String {
        format!("L'assistant n'a pas pu s'installer : {}.", sans_point(m))
    }
    pub fn ia_reparer(mo: u64) -> String {
        format!(
            "L'installation de l'assistant n'est pas allée jusqu'au bout (Mindcare a peut-être été fermée pendant) : répare-la, environ {mo} Mo, une seule fois."
        )
    }
    pub fn ia_mettre_a_jour(mo: u64) -> String {
        format!(
            "Une nouvelle version de l'assistant comprend mieux tes phrases : mets-la à jour (environ {mo} Mo, une seule fois). L'assistant actuel marche en attendant."
        )
    }
    pub fn ia_installer(mo: u64) -> String {
        format!(
            "Mindcare peut comprendre tes phrases (« mon pc s'éteint quand je joue ») : installe son assistant, environ {mo} Mo, une seule fois. Il marche ensuite sans internet."
        )
    }
    pub fn dictee_en_cours(pourcent: u8, etape: &str) -> String {
        format!("Installation de la dictée : {pourcent} % ({etape}). Tu peux continuer à te servir de Mindcare.")
    }
    pub fn dictee_erreur(m: &str) -> String {
        format!("La dictée n'a pas pu s'installer : {}.", sans_point(m))
    }
    pub fn dictee_installer(mo: u64) -> String {
        format!(
            "Pour parler au lieu d'écrire, installe la dictée : environ {mo} Mo, une seule fois. Elle marche ensuite sans internet, et ta voix n'est jamais gardée."
        )
    }

    /// Les boutons des lignes (`Update-BandeauIa`, `Update-ZoneDictee`)
    pub const INSTALLER: &str = "Installer";
    pub const REPARER: &str = "Réparer";
    pub const METTRE_A_JOUR: &str = "Mettre à jour";
    pub const REESSAYER: &str = "Réessayer";
}

/// « Pour être sûr » : la phrase, et ce qu'on voit pour chaque panne proposée (`Show-PourEtreSur`).
#[derive(Debug, Clone, PartialEq)]
pub struct PourEtreSur {
    pub phrase: String,
    pub choix: Vec<Choix>,
}

/// Ce que les pages savent de l'assistant et de la dictée : une photo prise par effets.rs à chaque affichage (et à
/// chaque pas de la minuterie), plus ce que les pages décident elles-mêmes. Sans installation (`None`) : l'appli n'a ni
/// assistant ni dictée, et « Raconte ton problème » reste celle de l'étape 5.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Aides {
    pub installation_assistant: Option<EtatInstallation>,
    pub assistant: Option<EtatAssistant>,
    pub installation_dictee: Option<EtatInstallation>,
    /// L'état de la dictée (pendant l'écoute, le niveau du son est toujours 0 : la page n'est pas redessinée à chaque
    /// son)
    pub dictee: Option<EtatDictee>,
    /// L'assistant lit la phrase posée (`$script:IaAttente`)
    pub lit: bool,
    /// Le dernier essai n'a rien donné (`Show-EchecIa`) : jusqu'à ce qu'on écrive autre chose
    pub echec: bool,
    /// Le message de la dictée qui reste (« Voilà ce que j'ai compris... », « La dictée est prête... »)
    pub message_dictee: Option<String>,
    /// La question « Pour être sûr » en cours
    pub pour_etre_sur: Option<PourEtreSur>,
}

impl Aides {
    /// L'appli a l'assistant et la dictée (le programme, pas les tests de l'étape 5).
    pub fn branchees(&self) -> bool {
        self.installation_assistant.is_some() || self.installation_dictee.is_some()
    }

    /// L'assistant peut lire une phrase (installé, à jour ou pas : un ancien marche en attendant).
    pub fn assistant_installe(&self) -> bool {
        self.assistant.as_ref().is_some_and(|e| *e != EtatAssistant::PasInstalle)
    }

    /// La dictée écoute ou écrit (`Test-DicteeOccupee`).
    pub fn dictee_occupee(&self) -> bool {
        matches!(self.dictee, Some(EtatDictee::Ecoute { .. } | EtatDictee::Ecrit))
    }

    /// La dictée écoute (`Test-DicteeEcoute`).
    pub fn dictee_ecoute(&self) -> bool {
        matches!(self.dictee, Some(EtatDictee::Ecoute { .. }))
    }
}

/// Une ligne de « Raconte ton problème », avec son bouton et sa barre.
fn ligne(
    icone: &str,
    couleur: &str,
    nom: &str,
    note: String,
    bouton: Option<(&str, &str)>,
    barre: Option<u8>,
) -> Value {
    let mut l = json!({ "type": "ligne", "icone": icone, "couleur": couleur, "texte": nom, "note": note });
    if let Some((id, texte)) = bouton {
        l["bouton"] = json!({ "id": id, "texte": texte });
    }
    if let Some(b) = barre {
        l["barre"] = json!(b);
    }
    l
}

/// La ligne « La dictée » (`Update-ZoneDictee`, et la maquette : toujours montrée).
pub fn ligne_dictee(a: &Aides) -> Option<Value> {
    use textes as x;
    let inst = a.installation_dictee.as_ref()?;
    let (note, bouton, barre) = match inst {
        EtatInstallation::EnCours { pourcent, etape } => (x::dictee_en_cours(*pourcent, etape), None, Some(*pourcent)),
        EtatInstallation::Erreur(m) => (x::dictee_erreur(m), Some(x::REESSAYER), None),
        EtatInstallation::AReparer { taille_mo } => (x::dictee_installer(*taille_mo), Some(x::REPARER), None),
        EtatInstallation::AMettreAJour { taille_mo } | EtatInstallation::Rien { taille_mo } => {
            (x::dictee_installer(*taille_mo), Some(x::INSTALLER), None)
        }
        EtatInstallation::Pret => {
            let note = match &a.dictee {
                Some(EtatDictee::Ecoute { .. }) => x::DICTEE_ECOUTE.to_string(),
                Some(EtatDictee::Ecrit | EtatDictee::PhrasePrete(_)) => x::DICTEE_ECRIT.to_string(),
                Some(EtatDictee::Erreur(m)) => m.clone(),
                _ => a.message_dictee.clone().unwrap_or_else(|| x::DICTEE_PRETE.to_string()),
            };
            (note, None, None)
        }
    };
    Some(ligne("son", "S", x::LIGNE_DICTEE, note, bouton.map(|b| (ids::INSTALLER_DICTEE, b)), barre))
}

/// La ligne « L'assistant » (`Update-BandeauIa`, `Show-EchecIa` ; la maquette : toujours montrée).
pub fn ligne_assistant(a: &Aides) -> Option<Value> {
    use textes as x;
    let inst = a.installation_assistant.as_ref()?;
    // (l'assistant marche : à jour, ou un ancien en attendant la mise à jour)
    let marche = |defaut: String| -> String {
        if a.lit {
            x::LIT.to_string()
        } else if a.echec {
            mindcare_assistant::MESSAGE_ECHEC.to_string()
        } else {
            defaut
        }
    };
    let (note, bouton, barre) = match inst {
        EtatInstallation::EnCours { pourcent, etape } => (x::ia_en_cours(*pourcent, etape), None, Some(*pourcent)),
        EtatInstallation::Erreur(m) => (x::ia_erreur(m), Some(x::REESSAYER), None),
        EtatInstallation::AReparer { taille_mo } => (x::ia_reparer(*taille_mo), Some(x::REPARER), None),
        EtatInstallation::AMettreAJour { taille_mo } => {
            (marche(x::ia_mettre_a_jour(*taille_mo)), Some(x::METTRE_A_JOUR), None)
        }
        EtatInstallation::Rien { taille_mo } => (x::ia_installer(*taille_mo), Some(x::INSTALLER), None),
        EtatInstallation::Pret => {
            let etat = match &a.assistant {
                // (une erreur passée est dite seulement juste après la phrase : l'assistant est relancé à la suivante)
                Some(EtatAssistant::Erreur { .. }) | None => EtatAssistant::Repos.texte(),
                Some(e) => e.texte(),
            };
            (marche(etat), None, None)
        }
    };
    Some(ligne("aide", "T", x::LIGNE_ASSISTANT, note, bouton.map(|b| (ids::INSTALLER_IA, b)), barre))
}

/// Le bouton Parler, en haut à droite de « Raconte ton problème » (seulement avec la dictée).
pub fn actions(a: &Aides) -> Option<Value> {
    a.installation_dictee.as_ref()?;
    let texte = if a.dictee_ecoute() { textes::ARRETER } else { textes::PARLER };
    Some(json!([{ "id": ids::PARLER, "texte": texte, "icone": "son" }]))
}

/// « Demander à l'assistant » : il est installé, il y a une phrase, et il n'en lit pas déjà une.
pub fn bouton_demander(etat: &Etat) -> Option<Value> {
    let a = &etat.aides;
    (a.assistant_installe() && !a.lit && !etat.recherche.trim().is_empty())
        .then(|| json!({ "type": "bouton", "id": ids::DEMANDER, "texte": textes::DEMANDER }))
}

/// La page « Pour être sûr » (`Show-PourEtreSur`) : ce qu'on a écrit, puis une question sur ce qu'on voit (le titre et la
/// phrase de chaque panne), jamais une liste de pannes à deviner ; « Aucune de ces phrases » ramène aux questions
/// guidées (« Ça concerne quoi ? »).
pub fn page_pour_etre_sur(p: &PourEtreSur) -> Value {
    let options: Vec<Value> = p
        .choix
        .iter()
        .enumerate()
        .map(|(i, c)| json!({ "id": i.to_string(), "titre": c.titre, "texte": c.phrase }))
        .collect();
    json!({
        "titre": "Pour être sûr",
        "parent": super::ecrans::RACONTE,
        // (la maquette validée ; le PowerShell : « Plusieurs pannes y ressemblent... », sous la phrase)
        "sous": "Plusieurs pannes ressemblent à ta phrase : une question pour choisir la bonne enquête.",
        "icone": "aide", "couleur": "T",
        "module": { "icone": "aide", "couleur": "T", "titre": pes::CE_QUE_TU_AS_ECRIT, "texte": format!("« {} »", p.phrase) },
        "blocs": [
            { "type": "titre", "texte": pes::QUESTION },
            { "type": "cartes", "id": ids::IDEE, "options": options },
            { "type": "bouton", "id": ids::AUCUNE, "texte": pes::AUCUNE, "style": "second" },
        ],
    })
}
