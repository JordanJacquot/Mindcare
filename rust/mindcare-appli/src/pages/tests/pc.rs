//! Le PC dans les pages (étape 8) : le tri des pannes (familles, une famille, la recherche, l'accueil), la bande
//! « Adapté à ton PC », « Tout afficher » pour cette fois, l'écran « Ton PC », le choix d'un autre PC gardé dans les
//! réglages, et les boutons de la réponse dans le pied. Chaque compte est refait ici à part, en lisant « PourPc »
//! dans le catalogue sans passer par le moteur.

use serde_json::{Value, json};

use mindcare_donnees::{ChoixPc, Donnees, horloge_figee};
use mindcare_moteur::pc::FichePc;
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{Objet, ProfilPc, TypePc, chercher};

use super::super::ton_pc::{GENRE, MARQUE, textes as x};
use super::{Banc, blocs, cat, maintenant, pannes_depliees, t, toutes_acceptees, verifier_kit};
use crate::pages::{Effet, ecrans, ids};

/// Ce que « PourPc » d'une panne demande (type, marques), lu ici sans le moteur.
fn pour(p: &Objet) -> (Option<String>, Vec<String>) {
    let Some(Value::Object(o)) = champ(p, "PourPc") else { return (None, Vec::new()) };
    let genre = o.get("Type").and_then(Value::as_str).map(str::to_string);
    let marques = o
        .get("Marque")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    (genre, marques)
}

/// Cette panne est-elle cachée pour ce genre (« portable », « fixe ») et cette marque ?
fn cachee(p: &Objet, genre: &str, marque: &str) -> bool {
    let (g, m) = pour(p);
    g.is_some_and(|g| g != genre) || (!m.is_empty() && !m.iter().any(|x| x == marque))
}

fn fiche_fixe() -> FichePc {
    FichePc {
        genre: Some("PC fixe monté".into()),
        modele: None,
        carte_mere: Some("ASUS PRIME B550-PLUS".into()),
        processeur: Some("AMD Ryzen 5 5600X 6-Core Processor".into()),
        memoire: Some("16 Go (2 barrettes)".into()),
        disque: Some("SSD NVMe de 931 Go, 412 Go libres".into()),
        windows: Some("Windows 11, version 24H2".into()),
    }
}

fn fixe_asus() -> ProfilPc {
    ProfilPc { genre: Some(TypePc::Fixe), marque: Some("ASUS".into()) }
}

/// Un banc dont le PC est reconnu : un PC fixe monté ASUS.
fn banc_fixe() -> Banc {
    let mut b = Banc::nouveau(None);
    let effets = b.etat.pc_reconnu(fixe_asus(), fiche_fixe());
    assert_eq!(effets, vec![Effet::Afficher { ecran: None }]);
    b.executer(effets);
    b
}

fn bande(page: &Value) -> Option<Value> {
    let l = blocs(page, "bande");
    assert!(l.len() <= 1, "une seule bande : {l:?}");
    l.first().map(|b| (*b).clone())
}

/// « N pannes correspondent » : le nombre du titre de la recherche.
fn ids_lignes(page: &Value) -> Vec<String> {
    blocs(page, "ligne").iter().filter_map(|l| l["id"].as_str()).map(str::to_string).collect()
}

/// Sans PC reconnu : rien de caché, ni pastille, ni écran « Ton PC », ni bande (comme avant l'étape 8).
#[test]
fn sans_detection_rien_n_est_cache() {
    let mut b = Banc::nouveau(None);
    b.message(json!({ "type": "pret" }));
    b.propre();
    assert!(b.json.get("pc").is_none());
    assert!(b.page(ecrans::TON_PC).is_none());
    assert!(bande(b.page(ecrans::FAMILLES).unwrap()).is_none());
    assert_eq!(b.json["accueil"]["tuiles"][1]["sous"], format!("15 familles, {} pannes", cat().pannes().len()));
    b.action("famille:batterie", Value::Null, ecrans::FAMILLES);
    assert_eq!(
        pannes_depliees(b.page(ecrans::FAMILLE).unwrap()).len(),
        super::super::familles::pannes_de(cat(), "batterie").len()
    );
}

/// Un PC fixe ASUS : les pannes de portable et celles des autres marques sont cachées partout, la bande le dit avec
/// le bon nombre sur chaque page, et seulement là où une panne est cachée.
#[test]
fn pc_fixe_cache_les_pannes_de_portable() {
    let mut b = banc_fixe();
    b.propre();
    let toutes = cat().pannes();
    let n = toutes.iter().filter(|p| cachee(p, "fixe", "ASUS")).count();
    let par_genre = toutes.iter().filter(|p| pour(p).0.as_deref() == Some("portable")).count();
    assert!(n > par_genre && par_genre > 0, "des pannes de portable et d'autres marques dans le catalogue");
    // L'accueil compte les pannes montrées
    assert_eq!(b.json["accueil"]["tuiles"][1]["sous"], format!("15 familles, {} pannes", toutes.len() - n));
    // Les familles : la bande en haut, avec le compte de tout le catalogue
    let familles = b.page(ecrans::FAMILLES).unwrap().clone();
    assert_eq!(familles["blocs"][0]["type"], "bande");
    assert_eq!(
        familles["blocs"][0]["texte"],
        format!("Adapté à ton PC fixe ASUS : {n} pannes de PC portable ou d'autres marques sont cachées.")
    );
    assert_eq!(familles["blocs"][0]["bouton"], json!({ "id": ids::TOUT_AFFICHER, "texte": "Tout afficher" }));
    assert_eq!(familles["focus"], "l:famille:allumage", "la première famille choisie, pas le bouton de la bande");
    // Chaque famille : seulement ses pannes qui concernent ce PC ; la bande seulement si l'une est cachée
    let mut vues = 0;
    for f in cat().familles() {
        let id = t(f, "Id");
        b.action(&format!("famille:{id}"), Value::Null, ecrans::FAMILLES);
        b.propre();
        let page = b.page(ecrans::FAMILLE).unwrap().clone();
        let siennes = super::super::familles::pannes_de(cat(), &id);
        let mut attendues: Vec<String> =
            siennes.iter().filter(|p| !cachee(p, "fixe", "ASUS")).map(|p| format!("panne:{}", t(p, "Id"))).collect();
        let mut montrees = pannes_depliees(&page);
        attendues.sort();
        montrees.sort();
        assert_eq!(montrees, attendues, "famille {id}");
        let caches = siennes.iter().filter(|p| cachee(p, "fixe", "ASUS")).count();
        match bande(&page) {
            None => assert_eq!(caches, 0, "famille {id} : des pannes cachées sans la bande"),
            Some(bd) => {
                vues += 1;
                assert!(caches > 0, "famille {id} : la bande sans panne cachée");
                let n = bd["texte"].as_str().unwrap();
                assert!(n.starts_with("Adapté à ton PC fixe ASUS : "), "{n}");
                let compte = if caches == 1 { "1 panne ".to_string() } else { format!("{caches} pannes ") };
                assert!(n.contains(&compte), "famille {id} : {n}");
            }
        }
        assert!(toutes_acceptees(&b.etat, &b.json).is_empty());
    }
    assert!(vues >= 3, "la bande sur plusieurs familles");
    // (la batterie : seulement des pannes de portable cachées, et une qui reste)
    b.action("famille:batterie", Value::Null, ecrans::FAMILLES);
    let page = b.page(ecrans::FAMILLE).unwrap().clone();
    let portables =
        super::super::familles::pannes_de(cat(), "batterie").iter().filter(|p| cachee(p, "fixe", "ASUS")).count();
    assert_eq!(
        bande(&page).unwrap()["texte"],
        format!("Adapté à ton PC fixe ASUS : {portables} pannes de PC portable sont cachées.")
    );
    // (contre-épreuve : Internet n'a aucune panne propre à un genre de PC, pas de bande)
    b.action("famille:internet", Value::Null, ecrans::FAMILLES);
    assert!(bande(b.page(ecrans::FAMILLE).unwrap()).is_none());

    // La recherche : les mêmes pannes que le moteur, sans celles cachées, la bande seulement si l'une l'est
    let mut avec = (0, 0);
    for requete in ["batterie", "wifi", "écran bleu", "le son grésille", "clavier", "bip"] {
        b.action("recherche", json!(requete), ecrans::RACONTE);
        b.propre();
        let page = b.page(ecrans::RACONTE).unwrap().clone();
        let trouvees = chercher(cat(), requete);
        let attendues: Vec<String> = trouvees
            .iter()
            .filter(|p| !cachee(p, "fixe", "ASUS"))
            .take(8)
            .map(|p| format!("panne:{}", t(p, "Id")))
            .collect();
        assert_eq!(ids_lignes(&page), attendues, "« {requete} »");
        let caches = trouvees.iter().filter(|p| cachee(p, "fixe", "ASUS")).count();
        assert_eq!(bande(&page).is_some(), caches > 0, "« {requete} »");
        if caches > 0 {
            avec.0 += 1
        } else {
            avec.1 += 1
        }
        // (le champ reste le premier : il garde le clavier)
        assert_eq!(page["blocs"][0]["type"], "champ");
    }
    assert!(avec.0 > 0 && avec.1 > 0, "des recherches avec et sans panne cachée : {avec:?}");
}

/// « Tout afficher » : tout est montré pour cette fois (rien n'est gardé, la pastille ne change pas), la bande
/// propose de revenir au tri ; « Adapter à mon PC » y revient.
#[test]
fn tout_afficher_pour_cette_fois() {
    let mut b = banc_fixe();
    let effets = b.action(ids::TOUT_AFFICHER, Value::Null, ecrans::FAMILLES);
    assert!(effets.contains(&Effet::Afficher { ecran: None }), "{effets:?}");
    assert!(!effets.iter().any(|e| matches!(e, Effet::ChoisirPc(_))), "rien n'est gardé");
    b.propre();
    assert_eq!(b.json["accueil"]["tuiles"][1]["sous"], format!("15 familles, {} pannes", cat().pannes().len()));
    let bd = bande(b.page(ecrans::FAMILLES).unwrap()).unwrap();
    assert_eq!(bd["texte"], x::TOUT_MONTRE);
    assert_eq!(bd["bouton"]["id"], ids::ADAPTER);
    assert_eq!(bd["bouton"]["texte"], x::ADAPTER_MON_PC);
    assert_eq!(b.json["pc"]["texte"], "PC fixe ASUS");
    assert_eq!(b.donnees.choix_pc(), ChoixPc::MonPc);
    b.action("famille:batterie", Value::Null, ecrans::FAMILLES);
    assert_eq!(
        pannes_depliees(b.page(ecrans::FAMILLE).unwrap()).len(),
        super::super::familles::pannes_de(cat(), "batterie").len()
    );
    // (contre-épreuve : une famille sans panne cachée n'a pas de bande, même après « Tout afficher »)
    b.action("famille:internet", Value::Null, ecrans::FAMILLES);
    assert!(bande(b.page(ecrans::FAMILLE).unwrap()).is_none());
    // Adapter à mon PC : le tri revient
    b.action(ids::ADAPTER, Value::Null, ecrans::FAMILLES);
    assert_ne!(b.json["accueil"]["tuiles"][1]["sous"], format!("15 familles, {} pannes", cat().pannes().len()));
    assert_eq!(bande(b.page(ecrans::FAMILLES).unwrap()).unwrap()["bouton"]["id"], ids::TOUT_AFFICHER);
    b.propre();
}

/// L'écran « Ton PC » de mon PC : ce que Mindcare a reconnu, expliqué ; le choix d'un autre PC ; Retour ramène là où
/// l'on était.
#[test]
fn ecran_ton_pc() {
    let mut b = banc_fixe();
    assert_eq!(b.json["pc"], json!({ "id": ids::TON_PC, "texte": "PC fixe ASUS", "icone": "pc" }));
    b.action(ids::TON_PC, Value::Null, ecrans::FAMILLES);
    b.propre();
    assert_eq!(b.ecran.as_deref(), Some(ecrans::TON_PC));
    let page = b.page(ecrans::TON_PC).unwrap().clone();
    assert_eq!(page["titre"], "Ton PC");
    assert_eq!(page["parent"], ecrans::FAMILLES, "Retour : là d'où on vient");
    assert_eq!(page["focus"], format!("l:{GENRE}portable"));
    assert!(bande(&page).is_none(), "pas d'autre PC choisi");
    let lignes: Vec<(String, String)> =
        blocs(&page, "ligne").iter().map(|l| (texte(l.get("texte")), texte(l.get("note")))).collect();
    let f = fiche_fixe();
    for v in [&f.genre, &f.carte_mere, &f.processeur, &f.memoire, &f.disque, &f.windows] {
        assert!(lignes.iter().any(|(t, _)| Some(t) == v.as_ref()), "{v:?} manque : {lignes:?}");
    }
    assert!(lignes.iter().any(|(t, n)| t == "ASUS" && n.contains("carte mère")), "{lignes:?}");
    // (chaque mot technique expliqué)
    for (mot, explication) in
        [("carte mère", "où tout se branche"), ("processeur", "cerveau"), ("mémoire vive", "garde")]
    {
        assert!(lignes.iter().any(|(_, n)| n.to_lowercase().contains(mot) && n.contains(explication)), "{mot}");
    }
    assert!(!lignes.iter().any(|(t, _)| t.is_empty()), "pas de ligne vide (le modèle d'un PC monté n'existe pas)");
    let genres: Vec<String> =
        blocs(&page, "groupe")[0]["elements"].as_array().unwrap().iter().map(|e| texte(e.get("id"))).collect();
    assert_eq!(genres, [format!("{GENRE}portable"), format!("{GENRE}fixe"), format!("{GENRE}inconnu")]);
    assert!(toutes_acceptees(&b.etat, &b.json).is_empty());
    // Ouvert depuis l'accueil : pas de parent (Retour : l'accueil), le fil dit seulement « Ton PC »
    b.action(ids::TON_PC, Value::Null, ecrans::ACCUEIL);
    let page = b.page(ecrans::TON_PC).unwrap();
    assert!(page.get("parent").is_none());
    assert_eq!(page["chemin"], json!(["Ton PC"]));
    // (ouvert depuis lui-même : l'écran d'avant est gardé)
    b.action(ids::TON_PC, Value::Null, ecrans::RACONTE);
    b.action(ids::TON_PC, Value::Null, ecrans::TON_PC);
    assert_eq!(b.page(ecrans::TON_PC).unwrap()["parent"], ecrans::RACONTE);
}

/// Un autre PC choisi (« pour un ami ») : le genre, puis la marque ; gardé dans les réglages ; la pastille, la bande
/// et le tri le suivent ; « Revenir à mon PC » ramène le PC reconnu.
#[test]
fn autre_pc_choisi_et_garde() {
    let mut b = banc_fixe();
    b.action(ids::TON_PC, Value::Null, ecrans::FAMILLES);
    // (contre-épreuve : une marque sans genre choisi ne choisit rien)
    let e = b.action(&format!("{MARQUE}HP"), Value::Null, ecrans::TON_PC);
    assert!(!e.iter().any(|x| matches!(x, Effet::ChoisirPc(_))), "{e:?}");
    assert!(b.page(ecrans::TON_PC_MARQUE).is_none());
    // Le genre : l'écran de la marque
    b.action(&format!("{GENRE}portable"), Value::Null, ecrans::TON_PC);
    b.propre();
    assert_eq!(b.ecran.as_deref(), Some(ecrans::TON_PC_MARQUE));
    let marques = b.page(ecrans::TON_PC_MARQUE).unwrap().clone();
    assert_eq!(marques["titre"], "Quelle est la marque du portable ?");
    assert_eq!(marques["parent"], ecrans::TON_PC);
    let el = blocs(&marques, "groupe")[0]["elements"].as_array().unwrap().clone();
    assert_eq!(el.len(), 1 + mindcare_moteur::MARQUES.len());
    assert_eq!(el[0]["titre"], "Je ne sais pas");
    assert!(toutes_acceptees(&b.etat, &b.json).is_empty());
    // (une marque inconnue : refusée)
    assert!(matches!(
        b.etat.clone().agir(
            cat(),
            &json!({ "type": "action", "id": format!("{MARQUE}Inconnue2"), "ecran": ecrans::TON_PC_MARQUE }),
            &mut String::new
        )[..],
        [Effet::Inconnue(_)]
    ));
    // La marque : le choix est gardé, et on revient à « Ton PC »
    let e = b.action(&format!("{MARQUE}HP"), Value::Null, ecrans::TON_PC_MARQUE);
    let voulu = ChoixPc::Autre(ProfilPc { genre: Some(TypePc::Portable), marque: Some("HP".into()) });
    assert!(e.contains(&Effet::ChoisirPc(voulu.clone())), "{e:?}");
    assert_eq!(b.ecran.as_deref(), Some(ecrans::TON_PC));
    assert_eq!(b.donnees.choix_pc(), voulu);
    let relu = Donnees::ouvrir(&b.dossier, horloge_figee(maintenant()), false);
    assert_eq!(relu.choix_pc(), voulu, "gardé dans les réglages");
    b.propre();
    assert!(b.page(ecrans::TON_PC_MARQUE).is_none());
    assert_eq!(b.json["pc"], json!({ "id": ids::TON_PC, "texte": "Autre : Portable HP", "icone": "portable" }));
    let page = b.page(ecrans::TON_PC).unwrap().clone();
    assert_eq!(page["sous"], x::SOUS_AUTRE);
    let bd = bande(&page).unwrap();
    assert_eq!(bd["texte"], "Tu regardes les pannes d'un autre PC : portable HP.");
    assert_eq!(bd["bouton"], json!({ "id": ids::MON_PC, "texte": "Revenir à mon PC" }));
    assert_eq!(page["focus"], format!("o:{}", ids::MON_PC));
    let el = blocs(&page, "groupe")[0]["elements"].as_array().unwrap().clone();
    assert_eq!(el[0]["etiquette"], "Choisi");
    assert!(el[1].get("etiquette").is_none());
    // (ce que Mindcare a reconnu sur CE PC reste montré)
    assert!(blocs(&page, "ligne").iter().any(|l| l["texte"] == "PC fixe monté"));
    // Le tri suit l'autre PC : les pannes de PC fixe et des autres marques cachées, celles de portable montrées
    let n = cat().pannes().iter().filter(|p| cachee(p, "portable", "HP")).count();
    let familles = b.page(ecrans::FAMILLES).unwrap().clone();
    assert_eq!(
        bande(&familles).unwrap()["texte"],
        format!("Adapté au PC choisi (portable HP) : {n} pannes de PC fixe ou d'autres marques sont cachées.")
    );
    b.action("famille:batterie", Value::Null, ecrans::FAMILLES);
    assert_eq!(
        pannes_depliees(b.page(ecrans::FAMILLE).unwrap()).len(),
        super::super::familles::pannes_de(cat(), "batterie").len()
    );
    // « Tout afficher » puis un nouveau choix : le tri revient
    b.action(ids::TOUT_AFFICHER, Value::Null, ecrans::FAMILLES);
    assert_eq!(bande(b.page(ecrans::FAMILLES).unwrap()).unwrap()["bouton"]["texte"], x::ADAPTER_CHOISI);
    // Revenir à mon PC
    let e = b.action(ids::MON_PC, Value::Null, ecrans::TON_PC);
    assert!(e.contains(&Effet::ChoisirPc(ChoixPc::MonPc)));
    assert_eq!(b.donnees.choix_pc(), ChoixPc::MonPc);
    assert_eq!(b.json["pc"]["texte"], "PC fixe ASUS");
    assert!(bande(b.page(ecrans::TON_PC).unwrap()).is_none());
    assert!(
        bande(b.page(ecrans::FAMILLES).unwrap()).unwrap()["texte"]
            .as_str()
            .unwrap()
            .starts_with("Adapté à ton PC fixe ASUS")
    );
    b.propre();
}

/// « Je ne sais pas » au genre et à la marque : rien n'est caché, la pastille le dit.
#[test]
fn autre_pc_je_ne_sais_pas() {
    let mut b = banc_fixe();
    b.action(&format!("{GENRE}inconnu"), Value::Null, ecrans::TON_PC);
    assert_eq!(b.page(ecrans::TON_PC_MARQUE).unwrap()["titre"], "Quelle est la marque du PC ?");
    b.action(&format!("{MARQUE}inconnue"), Value::Null, ecrans::TON_PC_MARQUE);
    assert_eq!(b.donnees.choix_pc(), ChoixPc::Autre(ProfilPc::inconnu()));
    assert_eq!(b.json["pc"]["texte"], "Autre : Tous les PC");
    assert!(bande(b.page(ecrans::FAMILLES).unwrap()).is_none());
    assert_eq!(b.json["accueil"]["tuiles"][1]["sous"], format!("15 familles, {} pannes", cat().pannes().len()));
    // Un portable sans marque : seulement les pannes de PC fixe cachées (pas celles d'une marque)
    b.action(&format!("{GENRE}portable"), Value::Null, ecrans::TON_PC);
    b.action(&format!("{MARQUE}inconnue"), Value::Null, ecrans::TON_PC_MARQUE);
    let n = cat().pannes().iter().filter(|p| pour(p).0.as_deref() == Some("fixe")).count();
    assert_eq!(
        bande(b.page(ecrans::FAMILLES).unwrap()).unwrap()["texte"],
        format!("Adapté au PC choisi (portable) : {n} pannes de PC fixe sont cachées.")
    );
    b.propre();
}

/// La détection a raté : la pastille le dit, rien n'est caché, l'écran l'explique.
#[test]
fn pc_pas_reconnu() {
    let mut b = Banc::nouveau(None);
    let e = b.etat.pc_reconnu(ProfilPc::inconnu(), FichePc::default());
    b.executer(e);
    b.propre();
    assert_eq!(b.json["pc"]["texte"], "PC pas reconnu");
    assert!(bande(b.page(ecrans::FAMILLES).unwrap()).is_none());
    let page = b.page(ecrans::TON_PC).unwrap().clone();
    assert!(blocs(&page, "note").iter().any(|n| n["texte"] == x::PAS_RECONNU));
    assert!(verifier_kit(&b.json).is_empty());
}

/// La réponse : ses trois boutons dans le pied de la page (toujours visibles), plus dans le contenu qui défile.
#[test]
fn reponse_boutons_dans_le_pied() {
    let mut b = Banc::nouveau(Some(Default::default()));
    b.commencer("chute");
    b.repondre(&[2]);
    b.propre();
    let page = b.enquete().clone();
    let pied: Vec<&str> = page["pied"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(pied, [ids::ACCUEIL, ids::REPARATEUR, ids::PRECEDENT]);
    assert!(!page["blocs"].as_array().unwrap().iter().any(|x| x["type"] == "bouton"), "un bouton dans ce qui défile");
    assert_eq!(page["focus"], format!("o:{}", ids::ACCUEIL));
    // (le vérificateur voit un pied mal rempli)
    let mut j = b.json.clone();
    j["pages"]["enquete"]["pied"] = json!([{ "type": "texte", "texte": "x" }]);
    assert!(!verifier_kit(&j).is_empty());
}
