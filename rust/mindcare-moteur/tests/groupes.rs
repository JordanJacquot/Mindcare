//! Les groupes de pannes d'une famille (les cartes dépliables) : le vrai catalogue est bien rangé, le filtre du profil
//! fait disparaître un groupe vidé, et la validation refuse un groupe mal écrit.

mod commun;

use commun::catalogue;
use mindcare_moteur::ps::{champ, texte};
use mindcare_moteur::{Catalogue, ProfilPc, TypePc, verifier, verifier_groupes};
use serde_json::{Value, json};

fn ids(g: &mindcare_moteur::Groupe) -> Vec<String> {
    g.pannes.iter().map(|p| texte(champ(p, "Id"))).collect()
}

#[test]
fn le_vrai_catalogue_est_range() {
    let cat = catalogue();
    assert_eq!(verifier_groupes(cat), Vec::<String>::new());
    let mut vues = 0;
    for f in cat.familles() {
        let fid = texte(champ(f, "Id"));
        let gs = cat.groupes(&fid);
        assert!((3..=6).contains(&gs.len()), "{fid} : {} groupes", gs.len());
        for g in &gs {
            assert!(!g.id.is_empty(), "{fid} : une panne sans groupe");
            assert!((2..=8).contains(&g.pannes.len()), "{} : {} pannes", g.id, g.pannes.len());
            assert!(!g.nom.is_empty() && !g.exemple.is_empty() && !g.icone.is_empty(), "{}", g.id);
            let image = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../Mindcare/fichiers/images/{}.svg", g.icone));
            assert!(image.is_file(), "{} : image {} absente", g.id, g.icone);
            vues += g.pannes.len();
        }
        // Rien de connu : les mêmes groupes, rien de caché
        let t = cat.groupes_pour(&fid, &ProfilPc::inconnu());
        assert_eq!((t.groupes, t.cachees), (gs, 0), "{fid}");
    }
    assert_eq!(vues, cat.pannes().len());
    // L'exemple validé par Jordan
    let noms: Vec<String> = cat.groupes("allumage").iter().map(|g| g.nom.clone()).collect();
    assert_eq!(
        noms,
        [
            "Il ne s'allume pas",
            "Il s'éteint ou redémarre",
            "La veille et l'arrêt",
            "Un message au démarrage",
            "Après un accident"
        ]
    );
}

#[test]
fn un_groupe_vide_par_le_filtre_disparait() {
    let cat = catalogue();
    let tour = ProfilPc { genre: Some(TypePc::Fixe), marque: Some("ASUS".into()) };
    let t = cat.groupes_pour("batterie", &tour);
    assert_eq!(t.groupes.len(), 1);
    assert_eq!(t.groupes[0].id, "batterie-danger");
    assert_eq!(ids(&t.groupes[0]), ["etincelles"]);
    assert_eq!(t.cachees, 10);

    let portable_hp = ProfilPc { genre: Some(TypePc::Portable), marque: Some("HP".into()) };
    let t = cat.groupes_pour("voyants", &portable_hp);
    let g: Vec<(&str, Vec<String>)> = t.groupes.iter().map(|g| (g.id.as_str(), ids(g))).collect();
    assert_eq!(
        g,
        [
            ("voyants-bips", vec!["bipsdemarrage".to_string(), "voyantclignote".to_string()]),
            ("voyants-marque", vec!["capslockhp".to_string()]),
        ]
    );
    assert_eq!(t.cachees, 4);
}

fn petit(familles: Value, pannes: Value) -> String {
    json!({
        "Familles": familles,
        "Suspects": { "s": { "Titre": "S", "Trouve": "T", "Pourquoi": "P", "AFaire": "A", "Pro": "R" } },
        "Tests": {},
        "Pannes": pannes
    })
    .to_string()
}

fn famille(groupes: Value) -> Value {
    json!({ "Id": "f", "Titre": "F", "Phrase": "P", "Icone": "i", "Groupes": groupes })
}

fn panne(id: &str, groupe: Value) -> Value {
    json!({ "Id": id, "Famille": "f", "Groupe": groupe, "Titre": "T", "Phrase": "P", "Suspects": ["s"],
        "Etapes": [{ "Id": "e", "Type": "question", "Texte": "T", "Court": "C",
            "Reponses": [{ "Texte": "a" }, { "Texte": "b" }] }] })
}

fn probleme(familles: Value, pannes: Value) -> Vec<String> {
    let j = petit(familles, pannes);
    let cat = Catalogue::lire(&j).expect("lisible");
    // (les groupes ne changent rien à la validation de PowerShell)
    assert_eq!(verifier(&cat, &j), Vec::<String>::new());
    verifier_groupes(&cat)
}

#[test]
fn la_validation_refuse_un_groupe_mal_ecrit() {
    let g = |id: &str| json!({ "Id": id, "Nom": "N", "Exemple": "E", "Icone": "i" });
    let bon =
        probleme(json!([famille(json!([g("a"), g("b")]))]), json!([panne("p", json!("a")), panne("q", json!("b"))]));
    assert_eq!(bon, Vec::<String>::new());

    let cas = [
        (json!([famille(Value::Null)]), json!([panne("p", json!("a"))]), "Famille f : Groupes manquant"),
        (json!([famille(json!([]))]), json!([panne("p", json!("a"))]), "Famille f : Groupes manquant"),
        (json!([famille(json!([g("a")]))]), json!([panne("p", Value::Null)]), "Panne p : Groupe manquant"),
        (json!([famille(json!([g("a")]))]), json!([panne("p", json!(""))]), "Panne p : Groupe manquant"),
        (
            json!([famille(json!([g("a")]))]),
            json!([panne("p", json!("a")), panne("q", json!("z"))]),
            "Panne q : groupe \u{ab} z \u{bb} absent de sa famille",
        ),
        (
            json!([famille(json!([g("a"), g("b")]))]),
            json!([panne("p", json!("a"))]),
            "Famille f, groupe b : aucune panne",
        ),
        (json!([famille(json!([g("a"), g("a")]))]), json!([panne("p", json!("a"))]), "Famille f, groupe a : en double"),
        (
            json!([famille(json!([{ "Id": "a", "Nom": "", "Exemple": "E", "Icone": "i" }]))]),
            json!([panne("p", json!("a"))]),
            "Famille f, groupe a : Id, Nom, Exemple ou Icone manquant",
        ),
    ];
    for (f, p, attendu) in cas {
        let pb = probleme(f, p);
        assert!(pb.iter().any(|x| x == attendu), "attendu « {attendu} », obtenu {pb:?}");
    }
}

#[test]
fn une_panne_sans_groupe_n_est_jamais_perdue() {
    let g = json!({ "Id": "a", "Nom": "N", "Exemple": "E", "Icone": "i" });
    let j = petit(json!([famille(json!([g]))]), json!([panne("p", json!("a")), panne("q", json!("z"))]));
    let cat = Catalogue::lire(&j).expect("lisible");
    let gs = cat.groupes("f");
    assert_eq!(gs.len(), 2);
    assert_eq!((gs[1].id.as_str(), gs[1].nom.as_str(), ids(&gs[1])), ("", "Autres pannes", vec!["q".to_string()]));
    // Une famille sans groupes : toutes ses pannes dans « Autres pannes »
    let j = petit(json!([famille(Value::Null)]), json!([panne("p", json!("a"))]));
    let cat = Catalogue::lire(&j).expect("lisible");
    let gs = cat.groupes("f");
    assert_eq!((gs.len(), ids(&gs[0])), (1, vec!["p".to_string()]));
}
