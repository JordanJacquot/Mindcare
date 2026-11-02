//! La fenêtre principale vue par Windows : accrochée dans Mindset ou non, visible ou non (elle et ses parents),
//! montrée sans prendre la main. Et [`FenetreTauri`], le trait `Fenetre` de mindcare-vie rempli avec la vraie
//! fenêtre (ces appels, et ceux de Tauri pour la montrer au premier plan et changer sa taille minimale). Et la
//! taille minimale gardée quand Mindset change la taille par `SetWindowPos` ([`garder_taille_minimale`]).

use mindcare_vie::{Fenetre, Niveau, Taille};
use tauri::{LogicalSize, WebviewWindow};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetParent, GetWindowLongPtrW, IsIconic, IsWindow, IsWindowVisible, MINMAXINFO, SW_SHOWNA, SWP_NOSIZE,
    SendMessageW, ShowWindow, WINDOWPOS, WM_GETMINMAXINFO, WM_WINDOWPOSCHANGING, WS_CHILD, WS_MINIMIZE,
};

use super::hwnd;

/// Le numéro de la sous-classe de [`garder_taille_minimale`] (« MCMI »).
const SOUS_CLASSE_TAILLE: usize = 0x4D43_4D49;

/// Garde la taille minimale de la fenêtre quand un AUTRE programme change sa taille (`SetWindowPos`) : Mindset,
/// qui la rend à 150 x 120 puis la libère (natif.cs, `Redimensionner` et `Liberer` ; contrat-appli, partie 7).
///
/// WPF (Mindcare PowerShell, le témoin) respecte sa taille minimale partout. Tao, sous Tauri, répond lui-même à
/// `WM_WINDOWPOSCHANGING` sans passer par `DefWindowProc` (tao 0.37.2, event_loop.rs:1061-1146), or c'est
/// `DefWindowProc` qui demande `WM_GETMINMAXINFO` et borne la taille : la taille minimale de Tauri ne tenait donc qu'à
/// la souris. Cette sous-classe, appelée avant tao, borne chaque nouvelle taille d'une fenêtre de premier niveau à
/// celle que tao donne dans `WM_GETMINMAXINFO` (rien une fois accrochée : `WS_CHILD`, et Mindcare n'y a plus de
/// taille minimale). `false` si Windows l'a refusée. À appeler une fois, sur le fil de la fenêtre.
pub fn garder_taille_minimale(h: usize) -> bool {
    // SAFETY : une fenêtre de ce programme, sur ce fil (exigé par SetWindowSubclass) ; la procédure ne garde
    // aucune donnée (dwrefdata 0) et vit autant que le programme
    existe(h) && unsafe { SetWindowSubclass(hwnd(h), Some(taille_minimale), SOUS_CLASSE_TAILLE, 0) }.as_bool()
}

/// La procédure de [`garder_taille_minimale`] : tout le reste passe tel quel au suivant (tao).
unsafe extern "system" fn taille_minimale(
    h: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _sous_classe: usize,
    _donnees: usize,
) -> LRESULT {
    if message == WM_WINDOWPOSCHANGING && lparam.0 != 0 {
        // SAFETY : pour WM_WINDOWPOSCHANGING, lparam est un WINDOWPOS que Windows laisse modifier pendant l'appel ;
        // GetWindowLongPtrW est une lecture
        let (pos, style) = unsafe { (&mut *(lparam.0 as *mut WINDOWPOS), GetWindowLongPtrW(h, GWL_STYLE) as u32) };
        // (comme DefWindowProc : ni une taille inchangée, ni une fenêtre réduite ; ici, ni une fenêtre accrochée)
        if !pos.flags.contains(SWP_NOSIZE) && style & (WS_CHILD.0 | WS_MINIMIZE.0) == 0 {
            let mut mm = MINMAXINFO::default();
            // SAFETY : la fenêtre de ce fil (appel direct) ; mm vit pendant l'appel ; tao n'y remplit que la taille
            // minimale (rien : 0, aucune borne)
            unsafe { SendMessageW(h, WM_GETMINMAXINFO, None, Some(LPARAM(&mut mm as *mut MINMAXINFO as isize))) };
            pos.cx = pos.cx.max(mm.ptMinTrackSize.x);
            pos.cy = pos.cy.max(mm.ptMinTrackSize.y);
        }
    }
    // SAFETY : le message tel que reçu, au suivant de la chaîne
    unsafe { DefSubclassProc(h, message, wparam, lparam) }
}

/// Au plus tant de niveaux de parents (une chaîne plus longue n'arrive pas ; c'est une garde contre une boucle).
const NIVEAUX_MAX: usize = 64;

/// La fenêtre existe encore (`IsWindow`).
pub fn existe(h: usize) -> bool {
    // SAFETY : IsWindow accepte n'importe quelle valeur (une fenêtre détruite ou nulle rend FALSE)
    h != 0 && unsafe { IsWindow(Some(hwnd(h))) }.as_bool()
}

/// Le parent de la fenêtre (`GetParent`), 0 s'il n'y en a pas.
fn parent(h: usize) -> usize {
    // SAFETY : lecture seule ; une fenêtre sans parent rend une erreur, lue comme 0
    unsafe { GetParent(hwnd(h)) }.map(|p| p.0 as usize).unwrap_or(0)
}

/// Accrochée dans Mindset : `WS_CHILD` ou un parent (`EstEnfant` de Mindcare PowerShell, natif.cs:79-82).
pub fn est_accrochee(h: usize) -> bool {
    if !existe(h) {
        return false;
    }
    // SAFETY : lecture seule du style d'une fenêtre qui existe
    let style = unsafe { GetWindowLongPtrW(hwnd(h), GWL_STYLE) };
    (style as u32) & WS_CHILD.0 != 0 || parent(h) != 0
}

/// Visible (`IsWindowVisible` : elle et tous ses parents ont `WS_VISIBLE`).
pub fn est_visible(h: usize) -> bool {
    // SAFETY : lecture seule ; une fenêtre détruite rend FALSE
    existe(h) && unsafe { IsWindowVisible(hwnd(h)) }.as_bool()
}

/// La fenêtre puis chacun de ses parents (`GetParent`), avec leur état : visible (`IsWindowVisible`) et réduite
/// (`IsIconic`). Vide si la fenêtre n'existe pas.
pub fn niveaux(h: usize) -> Vec<Niveau> {
    let mut l = Vec::new();
    let mut x = h;
    while existe(x) && l.len() < NIVEAUX_MAX {
        // SAFETY : lectures seules sur une fenêtre qui existe
        let (visible, reduite) = unsafe { (IsWindowVisible(hwnd(x)).as_bool(), IsIconic(hwnd(x)).as_bool()) };
        l.push(Niveau { visible, reduite });
        x = parent(x);
    }
    l
}

/// Se montre sans prendre la main ni bouger, là où Mindset l'a mise (`ShowWindow(SW_SHOWNA)`, systeme.ps1:29-34).
pub fn montrer_sans_activer(h: usize) {
    if existe(h) {
        // SAFETY : la fenêtre de ce programme, qui existe ; aucune activation
        unsafe {
            let _ = ShowWindow(hwnd(h), SW_SHOWNA);
        }
    }
}

/// La vraie fenêtre principale, pour la vie de l'appli (mindcare-vie, trait `Fenetre`).
pub struct FenetreTauri {
    fenetre: WebviewWindow,
    h: usize,
    /// Le journal (une ligne si la veille de la page n'a pas pu être changée)
    journal: Box<dyn Fn(&str) + Send + Sync>,
}

impl FenetreTauri {
    /// `h` : le `HWND` de la fenêtre (`WebviewWindow::hwnd`).
    pub fn nouvelle(fenetre: WebviewWindow, h: usize, journal: impl Fn(&str) + Send + Sync + 'static) -> FenetreTauri {
        FenetreTauri { fenetre, h, journal: Box::new(journal) }
    }

    /// Le `HWND` de la fenêtre, comme un nombre.
    pub fn hwnd(&self) -> usize {
        self.h
    }
}

impl Fenetre for FenetreTauri {
    fn existe(&self) -> bool {
        existe(self.h)
    }

    fn est_accrochee(&self) -> bool {
        est_accrochee(self.h)
    }

    fn est_visible(&self) -> bool {
        est_visible(self.h)
    }

    fn montrer_sans_activer(&mut self) {
        montrer_sans_activer(self.h);
    }

    fn montrer_et_activer(&mut self) {
        // systeme.ps1:57-60 : montrée, rendue si réduite, puis au premier plan (une demande de l'utilisateur : le
        // second lancement qu'il vient de faire)
        let _ = self.fenetre.show();
        if self.fenetre.is_minimized().unwrap_or(false) {
            let _ = self.fenetre.unminimize();
        }
        let _ = self.fenetre.set_focus();
    }

    fn poser_taille_minimale(&mut self, taille: Option<Taille>) {
        let t = taille.map(|t| LogicalSize::new(t.largeur, t.hauteur));
        let _ = self.fenetre.set_min_size(t);
    }

    fn niveaux(&self) -> Vec<Niveau> {
        niveaux(self.h)
    }

    fn page_visible(&mut self, visible: bool) {
        if let Err(e) = super::webview2::page_visible(&self.fenetre, visible) {
            (self.journal)(&format!("Veille de l'écran : {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Sur de vraies fenêtres Windows (la classe « STATIC », sans rien dedans), créées sur un BUREAU WINDOWS À PART
    //! (CreateDesktop, propre à ce fil) : même montrées, elles n'apparaissent jamais à l'écran et ne prennent jamais
    //! le premier plan. Détruites à la fin, le fil revient à son bureau, et le bureau d'essai est fermé.

    use super::*;
    use windows::Win32::Foundation::{GENERIC_ALL, HWND};
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS, GetThreadDesktop, SetThreadDesktop,
    };
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SW_SHOWMINNOACTIVE, SetParent, SetWindowLongPtrW, WINDOW_EX_STYLE,
        WINDOW_STYLE, WS_OVERLAPPEDWINDOW,
    };
    use windows::core::{HSTRING, PCWSTR, w};

    /// Une fenêtre « STATIC » jamais montrée à sa création, sur le bureau du fil
    fn creer(style: WINDOW_STYLE, parent: Option<HWND>) -> usize {
        // SAFETY : classe système, textes constants, aucune donnée de création ; la fenêtre est détruite à la fin
        let h = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("MindcareEssai"),
                style,
                0,
                0,
                200,
                150,
                parent,
                None,
                None,
                None,
            )
        }
        .expect("fenêtre d'essai");
        h.0 as usize
    }

    /// Fait tourner `essai` sur un bureau Windows à part, dans un fil neuf (sans fenêtre : il peut changer de bureau).
    fn sur_un_bureau_a_part(essai: impl FnOnce() + Send + 'static) {
        std::thread::spawn(move || {
            // (un nom par essai : les essais tournent en même temps, chacun sur son bureau)
            static NUMERO: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let n = NUMERO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let nom = HSTRING::from(format!("MindcareEssai-{}-fenetre-{n}", std::process::id()));
            // SAFETY : nom valide ; bureau neuf, jamais affiché (seul SwitchDesktop l'afficherait, jamais appelé)
            let bureau =
                unsafe { CreateDesktopW(&nom, PCWSTR::null(), None, DESKTOP_CONTROL_FLAGS(0), GENERIC_ALL.0, None) }
                    .expect("bureau d'essai");
            // SAFETY : ce fil n'a encore aucune fenêtre ; il revient à son bureau avant de fermer celui d'essai
            unsafe {
                let avant = GetThreadDesktop(GetCurrentThreadId()).expect("bureau du fil");
                SetThreadDesktop(bureau).expect("passer sur le bureau d'essai");
                essai();
                SetThreadDesktop(avant).expect("revenir au bureau d'avant");
                let _ = CloseDesktop(bureau);
            }
        })
        .join()
        .expect("essai sur le bureau à part");
    }

    fn detruire(h: usize) {
        // SAFETY : une fenêtre d'essai de ce fil
        unsafe {
            let _ = DestroyWindow(hwnd(h));
        }
    }

    #[test]
    fn accrochee_comme_par_mindset() {
        sur_un_bureau_a_part(|| {
            let panneau = creer(WS_OVERLAPPEDWINDOW, None);
            let appli = creer(WS_OVERLAPPEDWINDOW, None);
            assert!(existe(appli));
            assert!(!est_accrochee(appli), "fenêtre de premier niveau : pas accrochée");
            // Ce que fait Mindset (natif.cs:81-98) : WS_CHILD, puis SetParent dans son panneau
            // SAFETY : fenêtres d'essai de ce fil
            unsafe {
                let style = GetWindowLongPtrW(hwnd(appli), GWL_STYLE);
                SetWindowLongPtrW(hwnd(appli), GWL_STYLE, style | WS_CHILD.0 as isize);
            }
            // (entre les deux appels de Mindset : WS_CHILD suffit, sans parent encore)
            assert!(est_accrochee(appli), "WS_CHILD seul : accrochée");
            // SAFETY : fenêtres d'essai de ce fil
            unsafe { SetParent(hwnd(appli), Some(hwnd(panneau))).expect("SetParent") };
            assert!(est_accrochee(appli));
            assert_eq!(niveaux(appli).len(), 2, "la fenêtre puis le panneau");
            // Une fenêtre créée enfant (WS_CHILD) dans le panneau : accrochée aussi. (Sans WS_CHILD, GetParent rend le
            // propriétaire et non le parent : comme EstEnfant, natif.cs:81, c'est WS_CHILD que pose Mindset qui compte.)
            let enfant = creer(WS_CHILD, Some(hwnd(panneau)));
            assert!(est_accrochee(enfant));
            for h in [enfant, appli, panneau] {
                detruire(h);
            }
            assert!(!existe(appli) && !est_accrochee(appli) && niveaux(appli).is_empty());
        });
    }

    /// Comme tao (event_loop.rs:1061-1146 et 1783-1811) : `WM_WINDOWPOSCHANGING` répondu sans `DefWindowProc` (aucune
    /// borne), et une taille minimale de 300 x 200 donnée dans `WM_GETMINMAXINFO`
    unsafe extern "system" fn comme_tao(
        h: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _sous_classe: usize,
        _donnees: usize,
    ) -> LRESULT {
        match message {
            WM_WINDOWPOSCHANGING => LRESULT(0),
            WM_GETMINMAXINFO => {
                // SAFETY : pour WM_GETMINMAXINFO, lparam est un MINMAXINFO modifiable pendant l'appel
                let mm = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
                mm.ptMinTrackSize.x = 300;
                mm.ptMinTrackSize.y = 200;
                LRESULT(0)
            }
            // SAFETY : le message tel que reçu, au suivant de la chaîne
            _ => unsafe { DefSubclassProc(h, message, wparam, lparam) },
        }
    }

    /// La taille de la fenêtre, après un `SetWindowPos` comme celui de Mindset (natif.cs, `Redimensionner`)
    fn redimensionner(h: usize, largeur: i32, hauteur: i32) -> (i32, i32) {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos};
        let mut r = RECT::default();
        // SAFETY : fenêtre d'essai de ce fil, sur le bureau à part
        unsafe {
            SetWindowPos(hwnd(h), None, 0, 0, largeur, hauteur, SWP_NOZORDER | SWP_NOACTIVATE).expect("SetWindowPos");
            GetWindowRect(hwnd(h), &mut r).expect("GetWindowRect");
        }
        (r.right - r.left, r.bottom - r.top)
    }

    #[test]
    fn taille_minimale_gardee_quand_un_autre_programme_redimensionne() {
        sur_un_bureau_a_part(|| {
            let appli = creer(WS_OVERLAPPEDWINDOW, None);
            // SAFETY : fenêtre d'essai de ce fil
            assert!(unsafe { SetWindowSubclass(hwnd(appli), Some(comme_tao), 1, 0) }.as_bool());
            // Le défaut : comme tao, rien ne borne un SetWindowPos (Mindset la réduisait à 100 x 100)
            assert_eq!(redimensionner(appli, 100, 100), (100, 100), "sans la sous-classe, aucune borne");
            assert!(garder_taille_minimale(appli));
            assert_eq!(redimensionner(appli, 100, 100), (300, 200), "bornée à la taille minimale de tao");
            assert_eq!(redimensionner(appli, 250, 500), (300, 500), "chaque côté à part");
            assert_eq!(redimensionner(appli, 640, 480), (640, 480), "plus grande : inchangée");
            // Accrochée (WS_CHILD, natif.cs:86) : Mindset lui donne la taille qu'il veut
            // SAFETY : fenêtre d'essai de ce fil
            unsafe {
                let style = GetWindowLongPtrW(hwnd(appli), GWL_STYLE);
                SetWindowLongPtrW(hwnd(appli), GWL_STYLE, style | WS_CHILD.0 as isize);
            }
            assert_eq!(redimensionner(appli, 150, 120), (150, 120), "accrochée : aucune borne");
            detruire(appli);
            assert!(!garder_taille_minimale(appli), "une fenêtre détruite : refusé");
        });
    }

    #[test]
    fn visible_seulement_si_tous_les_niveaux_le_sont() {
        sur_un_bureau_a_part(|| {
            let panneau = creer(WS_OVERLAPPEDWINDOW, None);
            let appli = creer(WS_CHILD, Some(hwnd(panneau)));
            let tous_vus = |h| niveaux(h).iter().all(|n: &Niveau| n.visible && !n.reduite);
            assert!(!est_visible(appli) && !tous_vus(appli), "rien n'est montré");
            montrer_sans_activer(appli);
            assert!(!est_visible(appli), "le panneau est encore caché");
            assert_eq!(niveaux(appli)[1], Niveau { visible: false, reduite: false });
            montrer_sans_activer(panneau);
            assert!(est_visible(appli) && tous_vus(appli));
            // Le panneau réduit (sans être activé) : la page ne se voit plus
            // SAFETY : fenêtre d'essai de ce fil, sur le bureau à part
            unsafe {
                let _ = ShowWindow(hwnd(panneau), SW_SHOWMINNOACTIVE);
            }
            assert!(niveaux(appli)[1].reduite);
            assert!(!tous_vus(appli));
            detruire(panneau);
            assert!(!existe(appli), "détruite avec son parent");
        });
    }
}
