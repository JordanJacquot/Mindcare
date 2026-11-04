//! Le PC dont on regarde les pannes (étape 8) : la pastille « Ton PC » de la barre du haut, le tri des pannes (les
//! familles, une famille, la recherche), la bande « Adapté à ton PC » et l'écran « Ton PC ».
//!
//! - Le PC reconnu arrive de effets.rs (la détection lue sur un fil à part au démarrage : [`Etat::pc_reconnu`]). Tant
//!   qu'il n'est pas reconnu, rien n'est caché et la pastille n'est pas montrée (sauf si un autre PC est choisi).
//! - Le choix « pour un autre PC » (portable, PC fixe ou je ne sais pas, puis la marque) est gardé dans les réglages
//!   (effet [`Effet::ChoisirPc`], `Donnees::choisir_pc`) jusqu'à « Revenir à mon PC ».
//! - « Tout afficher » (la bande) montre toutes les pannes **pour cette fois** : rien n'est gardé, la pastille dit
//!   toujours le bon PC, et la bande propose de revenir au tri (« Adapter à mon PC »). Au prochain lancement, le tri
//!   revient tout seul : c'est le plus simple pour quelqu'un qui a cliqué sans trop savoir.
//!
//! La règle d'or du moteur reste : ce qu'on ne sait pas ne cache rien (`mindcare_moteur::pc`).

use serde_json::{Value, json};

use mindcare_donnees::ChoixPc;
use mindcare_moteur::pc::{FichePc, marque_connue};
use mindcare_moteur::{MARQUES, Objet, ProfilPc, TypePc, concerne};

use super::actions::ids;
use super::{Effet, Etat, ecrans};

/// Le PC reconnu par Mindcare (la détection est finie, réussie ou pas).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PcReconnu {
    /// Ce qui trie les pannes (rien de connu : la détection a raté, tout est montré)
    pub profil: ProfilPc,
    /// Ce qui est montré dans « Ton PC » et écrit dans le rapport pour le réparateur
    pub fiche: FichePc,
}

/// Les textes de l'écran « Ton PC » et de la bande.
pub mod textes {
    pub const TITRE: &str = "Ton PC";
    pub const SOUS: &str = "Mindcare montre seulement les pannes qui peuvent arriver à ce PC.";
    pub const SOUS_AUTRE: &str = "Mindcare montre seulement les pannes qui peuvent arriver au PC choisi.";
    pub const RECONNU: &str = "Ce que Mindcare a reconnu sur ce PC";
    pub const EN_COURS: &str = "Mindcare regarde encore ce PC, ça ne prend qu'un instant.";
    pub const PAS_RECONNU: &str = "Mindcare n'a pas pu reconnaître ce PC : toutes les pannes sont montrées.";
    pub const AUTRE: &str = "Tu regardes une panne pour un autre PC ?";
    pub const AUTRE_NOTE: &str =
        "Pour aider un ami ou un proche : choisis son genre de PC, puis sa marque. Mindcare s'en souviendra.";
    pub const PORTABLE: &str = "Un portable";
    pub const PORTABLE_NOTE: &str = "Il se replie, avec une batterie";
    pub const FIXE: &str = "Un PC fixe";
    pub const FIXE_NOTE: &str = "Une tour, ou un écran tout en un";
    pub const NSP: &str = "Je ne sais pas";
    pub const NSP_NOTE: &str = "Toutes les pannes seront montrées";
    pub const CHOISI: &str = "Choisi";
    pub const REVENIR: &str = "Revenir à mon PC";
    pub const MARQUE_TITRE_PORTABLE: &str = "Quelle est la marque du portable ?";
    pub const MARQUE_TITRE_FIXE: &str = "Quelle est la marque du PC fixe ?";
    pub const MARQUE_TITRE: &str = "Quelle est la marque du PC ?";
    pub const MARQUE_SOUS: &str = "Elle est écrite sur le PC, souvent sous l'écran, sur le dessus ou sur une étiquette dessous. Pour un PC fixe monté pièce par pièce, c'est la marque de la carte mère.";
    pub const MARQUE_NSP_NOTE: &str = "Les pannes de toutes les marques seront montrées";
    pub const TOUT_AFFICHER: &str = "Tout afficher";
    pub const ADAPTER_MON_PC: &str = "Adapter à mon PC";
    pub const ADAPTER_CHOISI: &str = "Adapter au PC choisi";
    pub const TOUT_MONTRE: &str = "Toutes les pannes sont affichées, même celles qui ne concernent pas ton PC.";
    pub const TOUT_MONTRE_AUTRE: &str =
        "Toutes les pannes sont affichées, même celles qui ne concernent pas le PC choisi.";
}
use textes as x;

/// Le début de l'id d'un genre de PC choisi (« pc-genre:portable », « pc-genre:fixe », « pc-genre:inconnu »).
pub const GENRE: &str = "pc-genre:";
/// Le début de l'id d'une marque choisie (« pc-marque:HP » ; « pc-marque:inconnue » : je ne sais pas).
pub const MARQUE: &str = "pc-marque:";
/// La marque « je ne sais pas » dans l'id.
pub const MARQUE_INCONNUE: &str = "inconnue";
const GENRE_INCONNU: &str = "inconnu";

/// La première lettre en minuscule, sauf pour « PC » : « Portable HP » devient « portable HP ».
fn dans_la_phrase(libelle: &str) -> String {
    if libelle.starts_with("PC") {
        return libelle.to_string();
    }
    let mut c = libelle.chars();
    match c.next() {
        Some(p) => p.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

impl Etat {
    /// Le profil choisi (le PC reconnu, ou l'autre PC choisi), sans tenir compte de « Tout afficher ». Rien de connu
    /// tant que le PC n'est pas reconnu : rien n'est caché.
    pub fn profil_choisi(&self) -> ProfilPc {
        self.choix_pc.profil(self.pc.as_ref().map(|p| &p.profil))
    }

    /// Le profil qui trie les pannes montrées (« Tout afficher » : rien de connu, tout est montré).
    pub fn profil_actif(&self) -> ProfilPc {
        if self.tout_afficher { ProfilPc::inconnu() } else { self.profil_choisi() }
    }

    /// Les pannes à montrer parmi celles-ci (dans l'ordre reçu), et celles que le profil choisi cache (même avec
    /// « Tout afficher » : la bande en parle).
    pub(crate) fn trier<'a>(&self, pannes: Vec<&'a Objet>) -> (Vec<&'a Objet>, Vec<&'a Objet>) {
        let profil = self.profil_choisi();
        let cachees: Vec<&Objet> = pannes.iter().copied().filter(|p| !concerne(p, &profil)).collect();
        let montrees =
            if self.tout_afficher { pannes } else { pannes.into_iter().filter(|p| concerne(p, &profil)).collect() };
        (montrees, cachees)
    }

    /// La pastille « Ton PC » : « Portable HP », « PC pas reconnu », « Autre : PC fixe ASUS » (court, comme la
    /// maquette validée : la barre du haut doit garder la place des onglets). Rien tant que le PC n'est pas reconnu
    /// (sauf si un autre PC est choisi).
    pub fn libelle_pastille(&self) -> Option<String> {
        match (&self.choix_pc, &self.pc) {
            (ChoixPc::MonPc, None) => None,
            (ChoixPc::MonPc, Some(p)) if p.profil.est_inconnu() => Some("PC pas reconnu".to_string()),
            (ChoixPc::MonPc, Some(p)) => Some(p.profil.libelle_court()),
            (ChoixPc::Autre(p), _) => Some(format!("Autre : {}", p.libelle_court())),
        }
    }

    /// La bande « Adapté à ton PC » d'une page où ces pannes sont cachées (rien si aucune) : « Adapté à ton PC fixe
    /// ASUS : 21 pannes de PC portable sont cachées. » et « Tout afficher » ; après « Tout afficher » : « Toutes les
    /// pannes sont affichées... » et « Adapter à mon PC ».
    pub(crate) fn bande(&self, cachees: &[&Objet]) -> Option<Value> {
        if cachees.is_empty() {
            return None;
        }
        let autre = self.choix_pc.pour_un_autre();
        if self.tout_afficher {
            let (texte, bouton) =
                if autre { (x::TOUT_MONTRE_AUTRE, x::ADAPTER_CHOISI) } else { (x::TOUT_MONTRE, x::ADAPTER_MON_PC) };
            return Some(json!({ "type": "bande", "texte": texte, "bouton": { "id": ids::ADAPTER, "texte": bouton } }));
        }
        Some(json!({ "type": "bande", "texte": self.phrase_cachees(cachees),
                     "bouton": { "id": ids::TOUT_AFFICHER, "texte": x::TOUT_AFFICHER } }))
    }

    /// « Adapté à ton PC fixe ASUS : 21 pannes de PC portable sont cachées. »
    fn phrase_cachees(&self, cachees: &[&Objet]) -> String {
        let profil = self.profil_choisi();
        let libelle = dans_la_phrase(&profil.libelle_court());
        let debut = if self.choix_pc.pour_un_autre() {
            format!("Adapté au PC choisi ({libelle})")
        } else {
            format!("Adapté à ton {libelle}")
        };
        // (cachée pour le genre de PC, ou seulement pour la marque)
        let genre_seul = ProfilPc { genre: profil.genre, marque: None };
        let par_genre = cachees.iter().filter(|p| !concerne(p, &genre_seul)).count();
        let autre_genre = match profil.genre {
            Some(TypePc::Fixe) => "de PC portable",
            Some(TypePc::Portable) => "de PC fixe",
            None => "d'un autre genre de PC",
        };
        let quoi = if par_genre == cachees.len() {
            autre_genre.to_string()
        } else if par_genre == 0 {
            "d'autres marques".to_string()
        } else {
            format!("{autre_genre} ou d'autres marques")
        };
        let n = cachees.len();
        if n == 1 {
            format!("{debut} : 1 panne {quoi} est cachée.")
        } else {
            format!("{debut} : {n} pannes {quoi} sont cachées.")
        }
    }

    /// Le PC est reconnu (la détection est finie : `profil` rien de connu si elle a raté). La page est redessinée.
    pub fn pc_reconnu(&mut self, profil: ProfilPc, fiche: FichePc) -> Vec<Effet> {
        self.pc = Some(PcReconnu { profil, fiche });
        vec![Effet::rafraichir()]
    }

    /// Les actions de « Ton PC » (la pastille, la bande, le genre et la marque choisis, revenir à mon PC). Rien : ce
    /// n'en est pas une.
    pub(crate) fn action_pc(&mut self, id: &str, ecran: &str) -> Option<Vec<Effet>> {
        if let Some(g) = id.strip_prefix(GENRE) {
            let genre = match g {
                GENRE_INCONNU => None,
                autre => match TypePc::depuis(autre) {
                    Some(t) => Some(t),
                    None => return Some(vec![Effet::Inconnue(id.to_string())]),
                },
            };
            self.genre_choisi = Some(genre);
            return Some(vec![Effet::afficher(ecrans::TON_PC_MARQUE)]);
        }
        if let Some(m) = id.strip_prefix(MARQUE) {
            let Some(genre) = self.genre_choisi else { return Some(vec![Effet::afficher(ecrans::TON_PC)]) };
            let marque = match m {
                MARQUE_INCONNUE => None,
                autre => match marque_connue(autre) {
                    Some(m) => Some(m.to_string()),
                    None => return Some(vec![Effet::Inconnue(id.to_string())]),
                },
            };
            let choix = ChoixPc::Autre(ProfilPc { genre, marque });
            self.genre_choisi = None;
            self.tout_afficher = false;
            self.choix_pc = choix.clone();
            return Some(vec![Effet::ChoisirPc(choix), Effet::afficher(ecrans::TON_PC)]);
        }
        match id {
            ids::TON_PC => {
                // (Retour ramène là où l'on était)
                if ecran != ecrans::TON_PC && ecran != ecrans::TON_PC_MARQUE {
                    self.avant_pc = Some(ecran.to_string()).filter(|e| !e.is_empty());
                }
                self.genre_choisi = None;
                Some(vec![Effet::afficher(ecrans::TON_PC)])
            }
            ids::MON_PC => {
                self.genre_choisi = None;
                self.tout_afficher = false;
                self.choix_pc = ChoixPc::MonPc;
                Some(vec![Effet::ChoisirPc(ChoixPc::MonPc), Effet::rafraichir()])
            }
            ids::TOUT_AFFICHER => {
                self.tout_afficher = true;
                Some(vec![Effet::Journal(JOURNAL_TOUT_AFFICHER.into()), Effet::rafraichir()])
            }
            ids::ADAPTER => {
                self.tout_afficher = false;
                Some(vec![Effet::Journal(JOURNAL_ADAPTER.into()), Effet::rafraichir()])
            }
            _ => None,
        }
    }
}

/// Les lignes du journal de « Tout afficher » et de « Adapter à mon PC ».
pub const JOURNAL_TOUT_AFFICHER: &str = "Adapté à ton PC : toutes les pannes affichées (pour cette fois)";
pub const JOURNAL_ADAPTER: &str = "Adapté à ton PC : pannes de nouveau triées";

/// Une ligne de ce qui est reconnu : la valeur en gras, ce que c'est en dessous (sans mot technique sans explication).
fn ligne(icone: &str, valeur: &Option<String>, note: &str) -> Option<Value> {
    let v = valeur.as_deref().map(str::trim).filter(|v| !v.is_empty())?;
    Some(json!({ "type": "ligne", "icone": icone, "couleur": "P", "texte": v, "note": note }))
}

/// La page « Ton PC » : à gauche ce que Mindcare a reconnu, à droite le choix d'un autre PC.
pub fn page(etat: &Etat, parent: Option<&str>) -> Value {
    let autre = match &etat.choix_pc {
        ChoixPc::Autre(p) => Some(p.clone()),
        ChoixPc::MonPc => None,
    };
    let mut blocs = Vec::new();
    if let Some(p) = &autre {
        blocs.push(json!({ "type": "bande",
            "texte": format!("Tu regardes les pannes d'un autre PC : {}.", dans_la_phrase(&p.libelle_court())),
            "bouton": { "id": ids::MON_PC, "texte": x::REVENIR } }));
    }

    // À gauche : ce que Mindcare a reconnu
    let mut gauche = vec![json!({ "type": "titre", "texte": x::RECONNU })];
    match &etat.pc {
        None => gauche.push(json!({ "type": "note", "texte": x::EN_COURS })),
        Some(pc) => {
            let f = &pc.fiche;
            let monte = f.genre.as_deref() == Some("PC fixe monté");
            let genre_icone = if pc.profil.genre == Some(TypePc::Portable) { "portable" } else { "pc" };
            let genre_note = if monte { "Un PC fixe monté pièce par pièce" } else { "Le genre de PC" };
            let marque_note = if monte { "La marque, celle de la carte mère" } else { "La marque" };
            let lignes: Vec<Value> = [
                ligne(genre_icone, &f.genre, genre_note),
                ligne("etoile", &pc.profil.marque, marque_note),
                ligne("pc", &f.modele, "Le modèle"),
                ligne("puce", &f.carte_mere, "La carte mère : la grande carte où tout se branche"),
                ligne("jauge", &f.processeur, "Le processeur : le cerveau du PC, il fait tous les calculs"),
                ligne("memoire", &f.memoire, "La mémoire vive : elle garde les programmes ouverts"),
                ligne("disque", &f.disque, "Le disque : là où sont rangés Windows et tes fichiers"),
                ligne("windows", &f.windows, "La version de Windows"),
            ]
            .into_iter()
            .flatten()
            .collect();
            if lignes.is_empty() || pc.profil.est_inconnu() {
                gauche.push(json!({ "type": "note", "texte": x::PAS_RECONNU }));
            }
            gauche.extend(lignes);
        }
    }

    // À droite : un autre PC (le genre ; la marque sur l'écran suivant)
    let choisi = autre.as_ref().map(|p| p.genre);
    let element = |id: &str, titre: &str, texte: &str, icone: &str, genre: Option<TypePc>| {
        let mut e = json!({ "id": format!("{GENRE}{id}"), "titre": titre, "texte": texte, "icone": icone,
                            "couleur": "S" });
        if choisi == Some(genre) {
            e["etiquette"] = json!(x::CHOISI);
        }
        e
    };
    let droite = vec![
        json!({ "type": "titre", "texte": x::AUTRE }),
        json!({ "type": "groupe", "elements": [
            element(TypePc::Portable.nom(), x::PORTABLE, x::PORTABLE_NOTE, "portable", Some(TypePc::Portable)),
            element(TypePc::Fixe.nom(), x::FIXE, x::FIXE_NOTE, "pc", Some(TypePc::Fixe)),
            element(GENRE_INCONNU, x::NSP, x::NSP_NOTE, "question", None),
        ] }),
        json!({ "type": "petit", "texte": x::AUTRE_NOTE }),
    ];
    blocs.push(json!({ "type": "colonnes", "elements": [{ "blocs": gauche }, { "blocs": droite }] }));

    let focus =
        if autre.is_some() { format!("o:{}", ids::MON_PC) } else { format!("l:{GENRE}{}", TypePc::Portable.nom()) };
    let mut p = json!({
        "titre": x::TITRE,
        "sous": if autre.is_some() { x::SOUS_AUTRE } else { x::SOUS },
        "icone": if etat.profil_choisi().genre == Some(TypePc::Portable) { "portable" } else { "pc" },
        "focus": focus,
        "blocs": blocs,
    });
    match parent {
        Some(par) => p["parent"] = json!(par),
        None => p["chemin"] = json!([x::TITRE]),
    }
    p
}

/// La page de la marque, après le genre choisi : « Je ne sais pas » puis les marques connues, en deux colonnes.
pub fn page_marque(genre: Option<TypePc>) -> Value {
    let titre = match genre {
        Some(TypePc::Portable) => x::MARQUE_TITRE_PORTABLE,
        Some(TypePc::Fixe) => x::MARQUE_TITRE_FIXE,
        None => x::MARQUE_TITRE,
    };
    let mut elements =
        vec![json!({ "id": format!("{MARQUE}{MARQUE_INCONNUE}"), "titre": x::NSP, "texte": x::MARQUE_NSP_NOTE })];
    elements.extend(MARQUES.iter().map(|m| json!({ "id": format!("{MARQUE}{m}"), "titre": m })));
    json!({
        "titre": titre,
        "sous": x::MARQUE_SOUS,
        "parent": ecrans::TON_PC,
        "chemin": [x::TITRE, titre],
        "focus": format!("l:{MARQUE}{MARQUE_INCONNUE}"),
        "blocs": [{ "type": "groupe", "colonnes": 2, "elements": elements }],
    })
}
