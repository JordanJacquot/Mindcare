//! Les 4 réparations de l'Aide de Mindcare, une fois la demande vérifiée (`mindcare_aide_regles::demandes`), faites
//! avec les appels de Windows du trait [`Systeme`] (le vrai Windows dans l'exe livré, un faux PC dans les tests).
//! Rien ici ne vient de la demande : aucune action ne prend de paramètre. Chaque étape va au journal de l'Aide, que
//! Mindcare recopie dans le sien. Un échec rend une phrase simple, montrée telle quelle à la personne.

use crate::systeme::{Entree, Systeme};
use mindcare_aide_regles::demandes::{Demande, VERSION};
use mindcare_aide_regles::{cartes, maj};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// Ce que l'Aide sait d'elle-même, pour « aide.bonjour »
pub struct EtatAide {
    pub admin: bool,
    pub compte: String,
    pub pid: u32,
}

/// `ip.renouveler` : le temps maximum pour toutes les cartes
pub const LIMITE_IP: Duration = Duration::from_secs(90);
/// `maj.vider` : l'attente entre deux essais
pub const ENTRE_DEUX_ESSAIS: Duration = Duration::from_secs(2);

/// Les phrases d'échec (pour des gens pas à l'aise avec un PC)
pub mod phrases {
    pub const SANS_WIFI: &str = "Ce PC n'a pas de Wi-Fi : il n'y a rien à oublier.";
    pub const WIFI_ILLISIBLE: &str = "Mindcare n'a pas pu lire les Wi-Fi enregistrés sur ce PC.";
    pub const WIFI_RESTES: &str =
        "Windows n'a pas voulu oublier certains Wi-Fi (souvent ceux réglés par une entreprise ou une école).";
    pub const SANS_CARTE: &str =
        "Mindcare n'a trouvé aucune connexion réglée en automatique sur ce PC : il n'y a pas d'adresse à redemander.";
    pub const CARTES_ILLISIBLES: &str = "Mindcare n'a pas pu lire les connexions de ce PC.";
    pub const BOX: &str =
        "La box n'a pas donné de nouvelle adresse. Éteins ta box, attends une minute, rallume-la, puis réessaie.";
    pub const ENERGIE: &str = "Windows n'a pas pu remettre les réglages d'alimentation d'origine.";
    pub const INTERNE: &str = "L'Aide de Mindcare a eu un problème. Redémarre le PC, puis réessaie.";
}

/// Fait la demande. `noter` : une ligne pour le journal de l'Aide.
pub fn faire(d: Demande, s: &dyn Systeme, etat: &EtatAide, noter: &dyn Fn(&str)) -> Result<Value, String> {
    match d {
        Demande::Bonjour => {
            Ok(json!({ "Version": VERSION, "Admin": etat.admin, "Pid": etat.pid, "Compte": etat.compte }))
        }
        // (le journal est rendu par le serveur, qui le tient)
        Demande::Journal => Ok(json!([])),
        Demande::WifiOublier => wifi_oublier(s, noter),
        Demande::IpRenouveler => ip_renouveler(s, noter),
        Demande::EnergieParDefaut => energie_par_defaut(s, noter),
        Demande::MajVider => maj_vider(s, noter),
    }
}

/// `wifi.oublier` : chaque Wi-Fi enregistré, sur chaque carte Wi-Fi, oublié par Windows. Aucune copie gardée : la
/// personne retape le mot de passe de son Wi-Fi (Mindcare le dit avant).
fn wifi_oublier(s: &dyn Systeme, noter: &dyn Fn(&str)) -> Result<Value, String> {
    let cartes = match s.wifi() {
        Ok(Some(c)) if !c.is_empty() => c,
        Ok(_) => {
            noter("wifi.oublier : pas de Wi-Fi sur ce PC (ni service ni carte Wi-Fi)");
            return Err(phrases::SANS_WIFI.into());
        }
        Err(e) => {
            noter(&format!("wifi.oublier : Wi-Fi illisible ({e})"));
            return Err(phrases::WIFI_ILLISIBLE.into());
        }
    };
    let mut supprimes = 0;
    let mut restes = Vec::new();
    for c in &cartes {
        noter(&format!("wifi.oublier : carte {}, {} Wi-Fi enregistré(s)", c.id, c.profils.len()));
        for p in &c.profils {
            match s.wifi_oublier(&c.id, p) {
                Ok(()) => {
                    supprimes += 1;
                    noter(&format!("wifi.oublier : « {p} » oublié"));
                }
                Err(e) => {
                    noter(&format!("wifi.oublier : « {p} » gardé par Windows ({e})"));
                    restes.push(p.clone());
                }
            }
        }
    }
    if !restes.is_empty() {
        return Err(phrases::WIFI_RESTES.into());
    }
    Ok(json!({ "supprimes": supprimes, "restes": restes, "wifi": true }))
}

/// `ip.renouveler` : pour chaque carte réglée en automatique avec une box (`cartes::a_renouveler`), l'adresse rendue
/// puis redemandée (toujours redemandée, même si la rendre a raté), 90 s au plus en tout.
fn ip_renouveler(s: &dyn Systeme, noter: &dyn Fn(&str)) -> Result<Value, String> {
    let debut = Instant::now();
    let avant = s.cartes().map_err(|e| {
        noter(&format!("ip.renouveler : cartes illisibles ({e})"));
        phrases::CARTES_ILLISIBLES.to_string()
    })?;
    let choisies: Vec<&cartes::Carte> = avant.iter().filter(|c| cartes::a_renouveler(c)).collect();
    for c in avant.iter().filter(|c| !cartes::a_renouveler(c)) {
        noter(&format!("ip.renouveler : « {} » laissée telle quelle ({})", c.nom, c.description));
    }
    if choisies.is_empty() {
        noter("ip.renouveler : aucune carte réglée en automatique avec une box");
        return Err(phrases::SANS_CARTE.into());
    }
    let mut faits = Vec::new();
    for c in &choisies {
        match s.ip_rendre(c.index) {
            Ok(()) => noter(&format!("ip.renouveler : « {} » a rendu son adresse", c.nom)),
            Err(e) => {
                noter(&format!("ip.renouveler : « {} » n'a pas rendu son adresse ({e}), redemandée quand même", c.nom))
            }
        }
        let reste = LIMITE_IP.saturating_sub(debut.elapsed());
        let r = if reste.is_zero() {
            Err(format!("plus de {} s", LIMITE_IP.as_secs()))
        } else {
            s.ip_redemander(c.index, reste)
        };
        match &r {
            Ok(()) => noter(&format!("ip.renouveler : « {} » a redemandé une adresse", c.nom)),
            Err(e) => noter(&format!("ip.renouveler : « {} » n'a pas eu de nouvelle adresse ({e})", c.nom)),
        }
        faits.push((*c, r.is_ok()));
    }
    let apres = s.cartes().unwrap_or_default();
    let mut tout = true;
    let mut rendu = Vec::new();
    for (c, redemandee) in faits {
        let nouvelles = apres.iter().find(|a| a.index == c.index).map(|a| a.adresses.clone()).unwrap_or_default();
        let ok = redemandee && cartes::adresse_recue(&nouvelles);
        tout &= ok;
        noter(&format!(
            "ip.renouveler : « {} » : {} avant, {} après",
            c.nom,
            c.adresses.join(" "),
            if nouvelles.is_empty() { "aucune".to_string() } else { nouvelles.join(" ") }
        ));
        rendu.push(json!({ "nom": c.nom, "avant": c.adresses, "apres": nouvelles, "ok": ok }));
    }
    if !tout {
        return Err(phrases::BOX.into());
    }
    Ok(json!({ "cartes": rendu }))
}

/// `energie.par-defaut` : `powercfg -restoredefaultschemes` (les modes d'origine de Windows, « Utilisation normale »
/// actif ; les modes ajoutés à la main ou par une autre appli sont effacés).
fn energie_par_defaut(s: &dyn Systeme, noter: &dyn Fn(&str)) -> Result<Value, String> {
    let avant = s.plan_actif().unwrap_or_default();
    let code = s.powercfg_par_defaut().map_err(|e| {
        noter(&format!("energie.par-defaut : powercfg non lancé ({e})"));
        phrases::ENERGIE.to_string()
    })?;
    let apres = s.plan_actif().unwrap_or_default();
    noter(&format!("energie.par-defaut : powercfg code {code}, mode actif {avant} avant, {apres} après"));
    if code != 0 {
        return Err(phrases::ENERGIE.into());
    }
    Ok(json!({ "actif_apres": apres, "code": code }))
}

/// `maj.vider` : voir `mindcare_aide_regles::maj`. Les services sont arrêtés, le dossier renommé (3 essais), puis les
/// services remis comme ils étaient avant (ceux qui tournaient sont relancés), TOUJOURS, même après un échec.
fn maj_vider(s: &dyn Systeme, noter: &dyn Fn(&str)) -> Result<Value, String> {
    let windows = s.dossier_windows();
    let dossier = maj::dossier(&windows);
    let etapes = std::cell::RefCell::new(Vec::<String>::new());
    let etape = |t: String| {
        noter(&format!("maj.vider : {t}"));
        etapes.borrow_mut().push(t);
    };
    match s.entree(&dossier) {
        Entree::Dossier => {}
        Entree::Absente => {
            etape(format!("{dossier} absent, rien à mettre de côté"));
            return Ok(json!({ "etapes": etapes.take(), "renomme_en": Value::Null, "services": services(s) }));
        }
        Entree::Lien | Entree::Fichier => {
            etape(format!("{dossier} n'est pas un dossier normal, laissé tel quel"));
            return Err(maj::LIEN.into());
        }
    }
    // Le .old d'une fois d'avant : supprimé (un reste jetable), sinon un nom daté
    let mut cible = maj::mis_de_cote(&windows);
    if s.entree(&cible) != Entree::Absente {
        match s.supprimer_dossier(&cible) {
            Ok(()) => etape(format!("{cible} d'avant supprimé")),
            Err(e) => {
                etape(format!("{cible} d'avant gardé ({e})"));
                cible = match maj::mis_de_cote_date(&windows, &s.heure_locale()) {
                    Some(c) if s.entree(&c) == Entree::Absente => c,
                    _ => return Err(maj::RENOMMAGE_REFUSE.into()),
                };
            }
        }
    }
    // Ce qui tournait avant (remis ainsi à la fin)
    let avant: Vec<(&str, bool)> = maj::SERVICES
        .iter()
        .map(|n| {
            let t = s.service_tourne(n).unwrap_or_else(|e| {
                etape(format!("service {n} illisible ({e})"));
                false
            });
            (*n, t)
        })
        .collect();
    etape(format!(
        "avant : {}",
        avant
            .iter()
            .map(|(n, t)| format!("{n} {}", if *t { "en marche" } else { "arrêté" }))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    let mut resultat: Result<(), &str> = Err(maj::RENOMMAGE_REFUSE);
    'essais: for essai in 1..=maj::ESSAIS {
        for n in maj::SERVICES {
            match s.service_arreter(n) {
                Ok(()) => etape(format!("essai {essai} : service {n} arrêté")),
                Err(e) => {
                    etape(format!("essai {essai} : service {n} ne s'arrête pas ({e}), abandon"));
                    resultat = Err(maj::SERVICE_BLOQUE);
                    break 'essais;
                }
            }
        }
        match s.renommer(&dossier, &cible) {
            Ok(()) => {
                etape(format!("essai {essai} : {dossier} renommé en {cible}"));
                resultat = Ok(());
                break;
            }
            Err(e) => {
                etape(format!("essai {essai} : renommage refusé ({e})"));
                if essai < maj::ESSAIS {
                    s.pause(ENTRE_DEUX_ESSAIS);
                }
            }
        }
    }
    // TOUJOURS : les services qui tournaient avant sont relancés (dans l'ordre inverse)
    let mut relances = true;
    for (n, tournait) in avant.iter().rev() {
        if !*tournait {
            continue;
        }
        match s.service_demarrer(n) {
            Ok(()) => etape(format!("service {n} relancé")),
            Err(e) => {
                etape(format!("service {n} pas relancé ({e})"));
                relances = false;
            }
        }
    }
    resultat?;
    if !relances {
        return Err(maj::RELANCE_RATEE.into());
    }
    Ok(json!({ "etapes": etapes.take(), "renomme_en": cible, "services": services(s) }))
}

/// L'état des services des mises à jour, pour le compte rendu
fn services(s: &dyn Systeme) -> Value {
    let mut m = serde_json::Map::new();
    for n in maj::SERVICES {
        let v = match s.service_tourne(n) {
            Ok(true) => "en marche",
            Ok(false) => "arrêté",
            Err(_) => "inconnu",
        };
        m.insert(n.to_string(), json!(v));
    }
    Value::Object(m)
}

#[cfg(test)]
mod tests;
