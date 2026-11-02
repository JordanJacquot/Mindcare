//! Mindset, le portail de la suite (modules\mindset.ps1) : son programme, donné par son installation, et l'ouverture
//! d'une de ses pages.

use std::path::{Path, PathBuf};

use mindcare_donnees::dossier::chemin_normal;
use suite_commun::chemins::joindre;

use crate::monde::{Disque, Systeme};

/// La page de Mindset sur GitHub, quand Mindset n'est pas installé (mindset.ps1:14).
pub const MINDSET_PAGE: &str = "https://github.com/JordanJacquot/Mindset/releases/latest";
/// La page de Mindcare dans Mindset (mindset.ps1:99).
pub const PAGE_MINDCARE: &str = "appli:mindcare";
/// Le fichier où Mindset lit la page à ouvrir, dans `%LOCALAPPDATA%\Mindset` (mindset.ps1:92-94).
pub const AFFICHER_PAGE: &str = "afficher-page.txt";

/// `Get-MindsetExe` (mindset.ps1:77-83) : `Mindset.exe` du dossier d'installation de Mindset, lu dans HKLM
/// (modifiable seulement par un administrateur), jamais dans suite.json. `None` s'il est introuvable.
pub fn mindset_exe(systeme: &mut dyn Systeme, disque: &dyn Disque) -> Option<PathBuf> {
    // mindset.ps1:79 : ([string]InstallLocation).Trim().Trim('"'), puis Get-CheminNormal
    let brut = systeme.emplacement_mindset()?;
    let d = chemin_normal(brut.trim().trim_matches('"'))?;
    // mindset.ps1:80 : seulement s'il y a bien un fichier Mindset.exe
    let exe = PathBuf::from(joindre(&d, "Mindset.exe"));
    disque.est_fichier(&exe).then_some(exe)
}

/// `Open-PageMindset` (mindset.ps1:88-98) : écrit la page dans `afficher-page.txt` puis lance Mindset (s'il tourne
/// déjà, il se ramène et lit le fichier). `false` si Mindset est introuvable ou si quelque chose a échoué.
/// `dossier_mindset` : `%LOCALAPPDATA%\Mindset`.
pub fn ouvrir_page_mindset(
    page: &str,
    dossier_mindset: &Path,
    systeme: &mut dyn Systeme,
    disque: &mut dyn Disque,
) -> bool {
    // mindset.ps1:89-90
    let Some(exe) = mindset_exe(systeme, disque) else { return false };
    // mindset.ps1:93 : le dossier créé s'il manque
    if disque.creer_dossier(dossier_mindset).is_err() {
        return false;
    }
    // mindset.ps1:94 : UTF-8 sans BOM
    if disque.ecrire_texte(&dossier_mindset.join(AFFICHER_PAGE), page).is_err() {
        return false;
    }
    // mindset.ps1:95 : Start-Process dans le dossier de Mindset
    let dossier = exe.parent().map(Path::to_path_buf);
    systeme.lancer(&exe, &[], dossier.as_deref()).is_ok()
}
