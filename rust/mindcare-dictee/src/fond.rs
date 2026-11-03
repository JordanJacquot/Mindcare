//! La dictée sur un fil à part, pour les pages : le fil fait avancer l'écoute et l'écriture toutes les 80 ms (les
//! minuteries du PowerShell, 80 et 100 ms) seulement pendant une dictée ; le reste du temps il dort.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::dictee::{Appui, Dictee, Etat};

/// L'intervalle entre deux étapes (`$script:DicteeMinuterie`)
pub const INTERVALLE: Duration = Duration::from_millis(80);

struct Partage {
    dictee: Mutex<(Dictee, bool)>,
    reveil: Condvar,
}

/// La dictée et son fil. Lâchée (Mindcare se ferme), le fil s'arrête et la dictée aussi (whisper arrêté, son effacé).
pub struct DicteeEnFond {
    partage: Arc<Partage>,
    fil: Option<JoinHandle<()>>,
}

fn tenir(p: &Partage) -> MutexGuard<'_, (Dictee, bool)> {
    p.dictee.lock().unwrap_or_else(|e| e.into_inner())
}

impl DicteeEnFond {
    pub fn lancer(dictee: Dictee) -> DicteeEnFond {
        let partage = Arc::new(Partage { dictee: Mutex::new((dictee, false)), reveil: Condvar::new() });
        let p = partage.clone();
        let fil = std::thread::Builder::new()
            .name("dictee".into())
            .spawn(move || {
                let mut g = tenir(&p);
                loop {
                    if g.1 {
                        return;
                    }
                    if g.0.occupee() {
                        drop(g);
                        std::thread::sleep(INTERVALLE);
                        g = tenir(&p);
                        if !g.1 {
                            g.0.etape();
                        }
                    } else {
                        g = p.reveil.wait(g).unwrap_or_else(|e| e.into_inner());
                    }
                }
            })
            .ok();
        DicteeEnFond { partage, fil }
    }

    /// Le bouton Parler
    pub fn appui(&self) -> Appui {
        let r = tenir(&self.partage).0.appui();
        self.partage.reveil.notify_all();
        r
    }

    /// Ce que la page montre
    pub fn etat(&self) -> Etat {
        tenir(&self.partage).0.etat()
    }

    /// La phrase écrite, une seule fois
    pub fn prendre_phrase(&self) -> Option<String> {
        tenir(&self.partage).0.prendre_phrase()
    }

    /// Le message affiché est lu
    pub fn effacer_message(&self) {
        tenir(&self.partage).0.effacer_message();
    }

    /// On quitte l'accueil : tout s'arrête, sans suite
    pub fn annuler(&self) {
        tenir(&self.partage).0.annuler();
    }

    /// Les lignes pour le journal de Mindcare
    pub fn prendre_journal(&self) -> Vec<String> {
        tenir(&self.partage).0.prendre_journal()
    }
}

impl Drop for DicteeEnFond {
    fn drop(&mut self) {
        {
            let mut g = tenir(&self.partage);
            g.1 = true;
            g.0.annuler();
        }
        self.partage.reveil.notify_all();
        if let Some(f) = self.fil.take() {
            let _ = f.join();
        }
    }
}
