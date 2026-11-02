//! Le verrou et le signal d'affichage (contrat-appli, partie 3 ; Mindcare.ps1:40-54).
//!
//! - `Local\Mindcare-Instance` : `New-Object Mutex($true, nom, [ref]$cree)` (Mindcare.ps1:42), gardé jusqu'à la fin
//!   du processus (Mindset le cherche par `Mutex.OpenExisting`, applis.ps1:111-120) ;
//! - `Local\Mindcare-Afficher` : `EventWaitHandle($false, AutoReset, nom)` (Mindcare.ps1:54), levé par un second
//!   lancement ou par Mindset (installations.ps1:153-161), regardé à chaque tour (`WaitOne(0)`, systeme.ps1:113).
//!
//! Les tests (en bas) n'utilisent que des noms `Local\MindcareEssai-<pid>-...`, jamais ceux de Mindcare.

use std::time::Duration;

use mindcare_vie::AttenteVerrou;
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, OpenEventW, ReleaseMutex, SetEvent, WaitForSingleObject,
};
use windows::core::HSTRING;

use super::handle;

/// Le verrou nommé, pris à sa création s'il n'existait pas. Son handle est fermé quand il est lâché.
#[derive(Debug)]
pub struct Verrou {
    h: usize,
}

impl Verrou {
    /// Crée le verrou et le prend (`CreateMutexW(.., TRUE, nom)`). Rend aussi `true` s'il vient d'être créé, `false`
    /// s'il existait déjà (tenu par une autre copie : il n'est pas à nous, il peut être attendu). `None` si Windows
    /// refuse (le verrou d'un programme lancé en administrateur, par exemple).
    pub fn creer(nom: &str) -> Option<(Verrou, bool)> {
        let nom = HSTRING::from(nom);
        // SAFETY : nom valide pendant l'appel (HSTRING terminé par un zéro) ; aucun attribut de sécurité ; le handle
        // rendu est gardé par Verrou, qui le ferme une fois. GetLastError est lu juste après, sur le même fil.
        let (h, deja) = unsafe {
            let h = CreateMutexW(None, true, &nom).ok()?;
            (h, GetLastError() == ERROR_ALREADY_EXISTS)
        };
        Some((Verrou { h: h.0 as usize }, !deja))
    }

    /// Attend le verrou tenu par une autre copie (`WaitOne(5000)`, Mindcare.ps1:47). Obtenu (libre ou abandonné), il
    /// est à nous, sur ce fil.
    pub fn attendre(&self, delai: Duration) -> AttenteVerrou {
        let ms = u32::try_from(delai.as_millis()).unwrap_or(u32::MAX - 1);
        // SAFETY : handle de mutex valide, gardé par self tant qu'il vit
        let r = unsafe { WaitForSingleObject(handle(self.h), ms) };
        match r {
            WAIT_OBJECT_0 => AttenteVerrou::Libre,
            WAIT_ABANDONED => AttenteVerrou::Abandonne,
            // (délai dépassé, ou un échec : toujours tenu)
            _ => AttenteVerrou::Tenu,
        }
    }

    /// Rend le verrou (s'il est à nous sur ce fil) ; il est fermé quand il est lâché, à la fin de cette fonction
    /// (Mindcare.ps1:92, avant une relance).
    pub fn liberer(self) {
        // SAFETY : handle de mutex valide ; ReleaseMutex échoue sans dommage s'il n'est pas à ce fil
        unsafe {
            let _ = ReleaseMutex(handle(self.h));
        }
    }
}

impl Drop for Verrou {
    fn drop(&mut self) {
        // SAFETY : handle rendu par CreateMutexW, fermé une seule fois (ici)
        unsafe {
            let _ = CloseHandle(handle(self.h));
        }
    }
}

/// Le signal d'affichage, à remise automatique. Son handle est fermé quand il est lâché.
#[derive(Debug)]
pub struct Signal {
    h: usize,
}

impl Signal {
    /// Crée le signal, ou ouvre celui qui existe déjà sous ce nom (`EventWaitHandle(false, AutoReset, nom)`).
    pub fn creer(nom: &str) -> Option<Signal> {
        let nom = HSTRING::from(nom);
        // SAFETY : nom valide pendant l'appel ; aucun attribut de sécurité ; handle gardé par Signal, fermé une fois
        let h = unsafe { CreateEventW(None, false, false, &nom) }.ok()?;
        Some(Signal { h: h.0 as usize })
    }

    /// Levé depuis la dernière fois (`WaitOne(0)`) : il est alors baissé (remise automatique).
    pub fn recu(&self) -> bool {
        // SAFETY : handle d'événement valide, gardé par self ; attente nulle
        unsafe { WaitForSingleObject(handle(self.h), 0) == WAIT_OBJECT_0 }
    }
}

impl Drop for Signal {
    fn drop(&mut self) {
        // SAFETY : handle rendu par CreateEventW, fermé une seule fois (ici)
        unsafe {
            let _ = CloseHandle(handle(self.h));
        }
    }
}

/// Ouvre le signal d'une autre copie et le lève (Mindcare.ps1:50) : `false` s'il n'existe pas ou si Windows refuse.
pub fn lever(nom: &str) -> bool {
    let nom = HSTRING::from(nom);
    // SAFETY : nom valide pendant l'appel ; seul le droit de lever le signal est demandé ; le handle ouvert est fermé
    // tout de suite
    unsafe {
        let Ok(h) = OpenEventW(EVENT_MODIFY_STATE, false, &nom) else { return false };
        let ok = SetEvent(h).is_ok();
        let _ = CloseHandle(h);
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc::channel;

    /// Un nom unique pour chaque essai : jamais `Local\Mindcare-*` (les vrais noms de Mindcare)
    fn nom(quoi: &str) -> String {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let nom = format!(r"Local\MindcareEssai-{}-{n}-{quoi}", std::process::id());
        assert!(!nom.starts_with(r"Local\Mindcare-"));
        nom
    }

    #[test]
    fn le_premier_cree_le_second_le_trouve_tenu() {
        let n = nom("verrou");
        let (v1, cree1) = Verrou::creer(&n).expect("verrou");
        assert!(cree1, "le premier crée le verrou");
        // Un second lancement (ici un autre fil : le verrou appartient au fil qui l'a pris)
        let n2 = n.clone();
        let r = std::thread::spawn(move || {
            let (v2, cree2) = Verrou::creer(&n2).expect("verrou");
            (cree2, v2.attendre(Duration::from_millis(50)))
        })
        .join()
        .unwrap();
        assert_eq!(r, (false, AttenteVerrou::Tenu));
        drop(v1);
        // Fermé partout : il n'existe plus, le suivant le crée
        let (_v3, cree3) = Verrou::creer(&n).expect("verrou");
        assert!(cree3);
    }

    /// Un autre fil tient le verrou puis le rend (libre), ou s'arrête en le tenant (abandonné).
    fn attente_quand(rendre: bool) -> AttenteVerrou {
        let n = nom(if rendre { "libre" } else { "abandon" });
        let (pret_tx, pret_rx) = channel();
        let (go_tx, go_rx) = channel::<()>();
        let n2 = n.clone();
        let fil = std::thread::spawn(move || {
            let (v, cree) = Verrou::creer(&n2).expect("verrou");
            assert!(cree);
            pret_tx.send(()).unwrap();
            go_rx.recv().unwrap();
            if rendre {
                v.liberer();
            } else {
                // (le handle reste ouvert, le fil s'arrête : le verrou est abandonné)
                std::mem::forget(v);
            }
        });
        pret_rx.recv().unwrap();
        let (moi, cree) = Verrou::creer(&n).expect("verrou");
        assert!(!cree);
        go_tx.send(()).unwrap();
        if !rendre {
            fil.join().unwrap();
        }
        let r = moi.attendre(Duration::from_secs(5));
        moi.liberer();
        r
    }

    #[test]
    fn attente_libre_ou_abandonne() {
        assert_eq!(attente_quand(true), AttenteVerrou::Libre);
        assert_eq!(attente_quand(false), AttenteVerrou::Abandonne);
    }

    #[test]
    fn signal_leve_recu_une_fois() {
        let n = nom("signal");
        assert!(!lever(&n), "un signal qui n'existe pas ne se lève pas");
        let s = Signal::creer(&n).expect("signal");
        assert!(!s.recu());
        assert!(lever(&n));
        assert!(s.recu(), "levé : reçu");
        assert!(!s.recu(), "remise automatique : reçu une seule fois");
        drop(s);
        assert!(!lever(&n), "fermé : il n'existe plus");
    }
}
