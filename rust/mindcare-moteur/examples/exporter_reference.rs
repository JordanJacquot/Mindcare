//! La référence du test d'équivalence de Mindcare Mobile (son fichier app/src/test/resources/reference-pc.json) : des
//! milliers d'enquêtes au hasard et de recherches jouées par ce moteur, plus les groupes de chaque famille et le tri des
//! pannes selon le PC (« PourPc »). Le test Kotlin du téléphone (EquivalenceTest) rejoue tout et doit trouver
//! exactement les mêmes résultats.
//!
//! Les enquêtes et les recherches sont les mêmes que celles de l'ancien export en PowerShell (outils\exporter-reference.ps1
//! du téléphone, qui chargeait modules\enquete.ps1) : même graine, même hasard (celui de .NET, `System.Random`), mêmes
//! parcours. Lancé par ce script du téléphone, ou à la main :
//!
//! ```text
//! cargo run --offline --locked -p mindcare-moteur --example exporter_reference -- \
//!     --catalogue <enquetes.json> --sortie <reference-pc.json> [--par-panne 40] [--graine 20261019]
//! ```

use std::path::PathBuf;

use mindcare_moteur::ps::{champ, parcours, tableau, texte};
use mindcare_moteur::{Catalogue, Enquete, MARQUES, Objet, ProfilPc, TypePc, chercher, chercher_pour, enquete};
use mindcare_moteur::{titre_verdict, verdict::minuscule};
use serde_json::{Value, json};

/// Le hasard de .NET avec une graine (`New-Object System.Random $Graine`, l'algorithme soustractif de Knuth) : les
/// mêmes tirages que l'ancien export en PowerShell, donc les mêmes parcours.
struct HasardDotNet {
    table: [i32; 56],
    suivant: usize,
    suivant_p: usize,
}

impl HasardDotNet {
    const MBIG: i32 = i32::MAX;
    const MSEED: i32 = 161_803_398;

    fn new(graine: i32) -> HasardDotNet {
        let mut table = [0i32; 56];
        let soustraction = if graine == i32::MIN { i32::MAX } else { graine.abs() };
        let mut mj = Self::MSEED - soustraction;
        table[55] = mj;
        let mut mk = 1i32;
        for i in 1..55 {
            let ii = (21 * i) % 55;
            table[ii] = mk;
            mk = mj - mk;
            if mk < 0 {
                mk += Self::MBIG;
            }
            mj = table[ii];
        }
        for _ in 1..5 {
            for i in 1..56 {
                table[i] = table[i].wrapping_sub(table[1 + (i + 30) % 55]);
                if table[i] < 0 {
                    table[i] += Self::MBIG;
                }
            }
        }
        HasardDotNet { table, suivant: 0, suivant_p: 21 }
    }

    fn echantillon(&mut self) -> f64 {
        let mut a = self.suivant + 1;
        if a >= 56 {
            a = 1;
        }
        let mut b = self.suivant_p + 1;
        if b >= 56 {
            b = 1;
        }
        let mut r = self.table[a].wrapping_sub(self.table[b]);
        if r == Self::MBIG {
            r -= 1;
        }
        if r < 0 {
            r += Self::MBIG;
        }
        self.table[a] = r;
        self.suivant = a;
        self.suivant_p = b;
        f64::from(r) * (1.0 / f64::from(Self::MBIG))
    }

    /// `Next(max)` : un nombre de 0 à max - 1 (0 si max vaut 0).
    fn next(&mut self, max: usize) -> usize {
        let max = i32::try_from(max).expect("trop grand");
        // (comme .NET : la partie entière, toujours entre 0 et max - 1)
        let n = (self.echantillon() * f64::from(max)) as i32;
        usize::try_from(n).unwrap_or(0)
    }
}

fn id(o: &Objet) -> String {
    texte(champ(o, "Id"))
}

/// Une enquête rejouée, sous la forme de la référence (Get-Cas de l'ancien export).
fn cas(cat: &Catalogue, panne: &Objet, choix: &[i64]) -> Value {
    let e: Enquete = enquete(cat, panne, choix, None);
    let verdict = match &e.verdict {
        Some(v) => {
            format!("{}|{}|{}|{}", v.genre.nom(), v.suspect.clone().unwrap_or_default(), v.detail, v.restants.join(","))
        }
        None => String::new(),
    };
    json!({
        "Panne": id(panne),
        "Entree": choix,
        "Etape": e.etape.as_ref().map(id).unwrap_or_default(),
        "Choix": e.choix,
        "Restants": e.restants,
        "Ecartes": e.ecartes.iter().map(|x| format!("{}|{}", x.id, x.raison)).collect::<Vec<_>>(),
        "Notes": e.notes,
        "Preuves": e.preuves,
        "Faits": e.faits.iter().map(|f| format!("{}|{}", f.court, f.reponse)).collect::<Vec<_>>(),
        "Verdict": verdict,
        "Titre": titre_verdict(cat, e.verdict.as_ref()),
        "AVenir": e.a_venir,
    })
}

/// Des milliers d'enquêtes au hasard : pour chaque panne, le début, puis `par_panne` fois une enquête jusqu'au bout
/// (une fois sur dix : « Je ne sais pas »), un arrêt en route, et de temps en temps des réponses en trop ou hors liste.
fn enquetes(cat: &Catalogue, par_panne: usize, hasard: &mut HasardDotNet) -> Vec<Value> {
    let mut tous = Vec::new();
    for p in cat.pannes() {
        tous.push(cas(cat, p, &[]));
        for k in 0..par_panne {
            let mut choix: Vec<i64> = Vec::new();
            for _ in 0..60 {
                let e = enquete(cat, p, &choix, None);
                let Some(etape) = &e.etape else { break };
                let c = if hasard.next(10) == 0 {
                    -1
                } else {
                    // (`@($e.Etape.Reponses).Count`)
                    i64::try_from(hasard.next(tableau(champ(etape, "Reponses")).len())).expect("numéro")
                };
                choix.push(c);
            }
            tous.push(cas(cat, p, &choix));
            if choix.len() > 1 {
                let n = hasard.next(choix.len() - 1) + 1;
                tous.push(cas(cat, p, &choix[..n]));
            }
            if k % 10 == 0 {
                let mut trop = choix.clone();
                trop.extend([0, 5]);
                tous.push(cas(cat, p, &trop));
            }
        }
    }
    tous
}

/// Les phrases de tous les jours de la recherche.
const PHRASES: [&str; 20] = [
    "il redémarre",
    "mon pc fait des bips",
    "le wifi se coupe toutes les 5 minutes",
    "écran bleu MEMORY_MANAGEMENT",
    "0x1A",
    "j'ai plus de son",
    "le son grésille",
    "le pc démarre sur le bios",
    "ma webcam marche pas",
    "l'ordi est super lent",
    "ça chauffe",
    "Le PC  ne   démarre PAS !",
    "œil",
    "ÉCRAN NOIR",
    "clé usb",
    "a",
    "",
    "   ",
    "zzzz",
    "mémoire insuffisante",
];

/// Les recherches : les noms, phrases et mots du catalogue, des morceaux de noms, les familles, et des phrases de tous
/// les jours.
fn requetes(cat: &Catalogue) -> Vec<String> {
    let mut r = Vec::new();
    for p in cat.pannes() {
        let titre = texte(champ(p, "Titre"));
        r.push(titre.clone());
        r.push(texte(champ(p, "Phrase")));
        for m in parcours(champ(p, "Mots")) {
            r.push(texte(Some(m)));
        }
        // (`-split ' '`, puis le 2e et le 3e mot)
        let mots: Vec<&str> = titre.split(' ').collect();
        if mots.len() > 2 {
            r.push(mots[1..3].join(" "));
        }
    }
    for f in cat.familles() {
        r.push(texte(champ(f, "Titre")));
    }
    r.extend(PHRASES.iter().map(|q| q.to_string()));
    r
}

/// Les groupes de chaque famille (les cartes dépliables), avec leurs pannes, dans l'ordre affiché par le PC.
fn groupes(cat: &Catalogue) -> Vec<Value> {
    cat.familles()
        .iter()
        .map(|f| {
            let fid = id(f);
            let gs: Vec<Value> = cat
                .groupes(&fid)
                .iter()
                .map(|g| {
                    json!({ "Id": g.id, "Nom": g.nom, "Exemple": g.exemple, "Icone": g.icone,
                            "Pannes": g.pannes.iter().map(|p| id(p)).collect::<Vec<_>>() })
                })
                .collect();
            json!({ "Famille": fid, "Groupes": gs })
        })
        .collect()
}

/// Les profils de PC à essayer : rien de connu, portable, fixe ; chaque marque citée par « PourPc », seule et avec
/// chaque genre ; une marque que le catalogue ne cite pas ; une marque écrite en minuscules.
fn profils(cat: &Catalogue) -> Vec<ProfilPc> {
    let mut citees: Vec<String> = Vec::new();
    for p in cat.pannes() {
        let marques = champ(p, "PourPc").and_then(Value::as_object).and_then(|o| champ(o, "Marque"));
        for m in parcours(marques) {
            let m = texte(Some(m));
            if !m.is_empty() && !citees.contains(&m) {
                citees.push(m);
            }
        }
    }
    let genres = [None, Some(TypePc::Portable), Some(TypePc::Fixe)];
    let mut l: Vec<ProfilPc> = genres.iter().map(|g| ProfilPc { genre: *g, marque: None }).collect();
    for m in &citees {
        for g in genres {
            l.push(ProfilPc { genre: g, marque: Some(m.clone()) });
        }
    }
    if let Some(autre) = MARQUES.iter().find(|m| !citees.iter().any(|c| c.eq_ignore_ascii_case(m))) {
        l.push(ProfilPc { genre: Some(TypePc::Portable), marque: Some((*autre).to_string()) });
    }
    if let Some(m) = citees.first() {
        l.push(ProfilPc { genre: None, marque: Some(m.to_lowercase()) });
        l.push(ProfilPc { genre: Some(TypePc::Fixe), marque: Some(format!(" {} ", minuscule(m))) });
    }
    l
}

/// Le tri des pannes pour chaque profil : toutes les pannes, chaque famille rangée en groupes, et quelques recherches
/// (celles gardées, dans l'ordre du PC, et combien sont cachées).
fn tri(cat: &Catalogue) -> Vec<Value> {
    let ids = |l: &[&Objet]| l.iter().map(|p| id(p)).collect::<Vec<_>>();
    let mut requetes_tri: Vec<String> = cat.familles().iter().map(|f| texte(champ(f, "Titre"))).collect();
    requetes_tri.extend(
        ["batterie", "voyant clignote", "écran noir", "il ne s'allume pas", "clavier", "ventilateur"].map(String::from),
    );
    profils(cat)
        .iter()
        .map(|profil| {
            let toutes = cat.pannes_pour(profil);
            let familles: Vec<Value> = cat
                .familles()
                .iter()
                .map(|f| {
                    let fid = id(f);
                    let gf = cat.groupes_pour(&fid, profil);
                    let gs: Vec<Value> =
                        gf.groupes.iter().map(|g| json!({ "Id": g.id, "Pannes": ids(&g.pannes) })).collect();
                    json!({ "Famille": fid, "Groupes": gs, "Cachees": gf.cachees })
                })
                .collect();
            let recherches: Vec<Value> = requetes_tri
                .iter()
                .map(|q| {
                    let t = chercher_pour(cat, q, profil);
                    json!({ "Requete": q, "Pannes": ids(&t.visibles), "Cachees": t.cachees })
                })
                .collect();
            json!({
                "Type": profil.genre.map(TypePc::nom).unwrap_or_default(),
                "Marque": profil.marque.clone().unwrap_or_default(),
                "Pannes": ids(&toutes.visibles),
                "Cachees": toutes.cachees,
                "Familles": familles,
                "Recherches": recherches,
            })
        })
        .collect()
}

fn main() {
    let manifeste = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut chemin = manifeste.join("..").join("..").join("Mindcare").join("fichiers").join("enquetes.json");
    let mut sortie: Option<PathBuf> = None;
    let mut par_panne = 40usize;
    let mut graine = 20_261_019i32;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut valeur = || args.next().unwrap_or_else(|| panic!("{a} : valeur manquante"));
        match a.as_str() {
            "--catalogue" => chemin = PathBuf::from(valeur()),
            "--sortie" => sortie = Some(PathBuf::from(valeur())),
            "--par-panne" => par_panne = valeur().parse().expect("--par-panne : un nombre"),
            "--graine" => graine = valeur().parse().expect("--graine : un nombre"),
            autre => panic!("option inconnue : {autre}"),
        }
    }
    let cat = Catalogue::lire_fichier(&chemin).unwrap_or_else(|e| panic!("{e}"));
    let mut hasard = HasardDotNet::new(graine);
    let cas = enquetes(&cat, par_panne, &mut hasard);
    let recherches: Vec<Value> = requetes(&cat)
        .into_iter()
        .map(|q| {
            let pannes: Vec<String> = chercher(&cat, &q).iter().map(|p| id(p)).collect();
            json!({ "Requete": q, "Pannes": pannes })
        })
        .collect();
    let groupes = groupes(&cat);
    let tri = tri(&cat);
    let resume = format!(
        "Référence (moteur Rust) : {} enquêtes, {} recherches, {} familles en groupes, {} profils de PC",
        cas.len(),
        recherches.len(),
        groupes.len(),
        tri.len()
    );
    let json = json!({ "Graine": graine, "Moteur": "Rust", "Cas": cas, "Recherches": recherches,
                       "Groupes": groupes, "Tri": tri });
    let texte_json = serde_json::to_string(&json).expect("json");
    match sortie {
        Some(s) => {
            std::fs::write(&s, texte_json).unwrap_or_else(|e| panic!("{} : {e}", s.display()));
            println!("{resume} -> {}", s.display());
        }
        None => println!("{texte_json}"),
    }
}

#[cfg(test)]
mod tests {
    use super::HasardDotNet;

    /// Les mêmes tirages que `New-Object System.Random` de .NET (relevés avec Windows PowerShell 5.1).
    #[test]
    fn le_hasard_est_celui_de_dotnet() {
        let mut h = HasardDotNet::new(20_261_019);
        let a: Vec<usize> = (0..8).map(|_| h.next(10)).collect();
        let b: Vec<usize> = (0..8).map(|_| h.next(7)).collect();
        assert_eq!(a, [9, 0, 7, 8, 3, 6, 1, 6]);
        assert_eq!(b, [2, 1, 3, 4, 4, 2, 1, 5]);
        let mut h = HasardDotNet::new(1);
        let c: Vec<usize> = (0..5).map(|_| h.next(1000)).collect();
        assert_eq!(c, [248, 110, 467, 771, 657]);
    }
}
