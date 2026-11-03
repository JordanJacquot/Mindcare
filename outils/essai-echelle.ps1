# Essai de l'échelle de Mindcare en Rust dans un panneau de Mindset plus petit que sa fenêtre (étape 5.4,
# `Update-EchelleMindset` de systeme.ps1:42-48), avec la CONSTRUCTION D'ESSAI. À lancer SEULEMENT sur un bureau Windows
# à part (outils\sur-bureau.ps1) :
#
#   & '.\outils\sur-bureau.ps1' -Script '.\outils\essai-echelle.ps1' -Arguments "-Exe <Mindset.exe d'essai> -Resultats <dossier>"
#
# Une fenêtre Windows Forms joue le panneau de Mindset : la fenêtre de Mindcare y est accrochée exactement comme le
# fait Mindset (natif.cs, `MSIntegration.Accrocher`, `Redimensionner`, `Liberer`, recopiés ici), à 700 x 500, puis
# 1300 x 900, puis 700 x 500 de nouveau, puis libérée. À chaque taille : l'échelle notée au journal par la construction
# d'essai (« Essai : échelle ... pour L x H », L x H en pixels logiques) doit être celle du PowerShell,
# min(1, L / 900, H / 620) arrondie au millième vers le bas, et une capture de la fenêtre est gardée pour être regardée
# (tout le contenu visible et lisible, pas de barre de défilement horizontale, pas de texte coupé).
#
# Rien du vrai PC : un dossier d'essai dans %TEMP%, les verrou et signal d'ESSAI (Local\MindcareEssai-...).
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Resultats
)
$ErrorActionPreference = 'Stop'
$echecs = New-Object System.Collections.Generic.List[string]
# (Write-Host : les fonctions qui rendent une valeur ne mélangent pas leurs messages à leur résultat)
function Ok([string]$t) { Write-Host "OK : $t" }
function Echec([string]$t) { Write-Host "ÉCHEC : $t"; $echecs.Add($t) }
function Verifier([bool]$c, [string]$t) { if ($c) { Ok $t } else { Echec $t } }

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class EssaiEchelle
{
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr h);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int l, t, r, b; }
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    public static string Classe(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
    public static string Titre(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
    public static IntPtr[] Fenetres(int pid)
    {
        var l = new List<IntPtr>();
        EnumWindows(delegate (IntPtr h, IntPtr x) { int p; GetWindowThreadProcessId(h, out p); if (p == pid) l.Add(h); return true; }, IntPtr.Zero);
        return l.ToArray();
    }
    public static IntPtr[] Enfants(IntPtr parent, string classe)
    {
        var l = new List<IntPtr>();
        EnumChildWindows(parent, delegate (IntPtr h, IntPtr x) { if (Classe(h) == classe) l.Add(h); return true; }, IntPtr.Zero);
        return l.ToArray();
    }
    public static IntPtr[] Tous(IntPtr parent)
    {
        var l = new List<IntPtr>();
        EnumChildWindows(parent, delegate (IntPtr h, IntPtr x) { l.Add(h); return true; }, IntPtr.Zero);
        return l.ToArray();
    }
    public static int Largeur(IntPtr h) { RECT r; GetClientRect(h, out r); return r.r - r.l; }
    public static int Hauteur(IntPtr h) { RECT r; GetClientRect(h, out r); return r.b - r.t; }
    public static void Touche(IntPtr h, int vk)
    {
        PostMessage(h, 0x0100, (IntPtr)vk, (IntPtr)1);                       // WM_KEYDOWN
        PostMessage(h, 0x0101, (IntPtr)vk, (IntPtr)unchecked((int)0xC0000001)); // WM_KEYUP
    }

    // --- Recopié de Mindset (fichiers\modules\natif.cs, MSIntegration), sans changement ---
    [DllImport("user32.dll", SetLastError = true)] static extern IntPtr SetParent(IntPtr child, IntPtr parent);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] static extern IntPtr GetWindowLongPtr(IntPtr h, int i);
    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")] static extern IntPtr SetWindowLongPtr(IntPtr h, int i, IntPtr v);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hh, uint flags);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    const int GWL_STYLE = -16, GWL_EXSTYLE = -20;
    const long WS_CHILD = 0x40000000, WS_POPUP = 0x80000000, WS_CAPTION = 0x00C00000, WS_THICKFRAME = 0x00040000,
               WS_SYSMENU = 0x00080000, WS_MINIMIZEBOX = 0x00020000, WS_MAXIMIZEBOX = 0x00010000, WS_VISIBLE = 0x10000000;
    const long WS_EX_APPWINDOW = 0x00040000, WS_EX_WINDOWEDGE = 0x100, WS_EX_DLGMODALFRAME = 0x1, WS_EX_CLIENTEDGE = 0x200, WS_EX_STATICEDGE = 0x20000;
    const uint SWP_NOZORDER = 0x4, SWP_FRAMECHANGED = 0x20, SWP_NOACTIVATE = 0x10, SWP_SHOWWINDOW = 0x40, SWP_NOMOVE = 0x2, SWP_NOSIZE = 0x1;
    public static long[] Accrocher(IntPtr fenetre, IntPtr parent, int largeur, int hauteur)
    {
        long style = GetWindowLongPtr(fenetre, GWL_STYLE).ToInt64();
        long ex = GetWindowLongPtr(fenetre, GWL_EXSTYLE).ToInt64();
        ShowWindow(fenetre, 9);
        long nouveau = (style & ~(WS_POPUP | WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX)) | WS_CHILD | WS_VISIBLE;
        long nouveauEx = ex & ~(WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_DLGMODALFRAME | WS_EX_CLIENTEDGE | WS_EX_STATICEDGE);
        SetWindowLongPtr(fenetre, GWL_STYLE, new IntPtr(nouveau));
        SetWindowLongPtr(fenetre, GWL_EXSTYLE, new IntPtr(nouveauEx));
        if (SetParent(fenetre, parent) == IntPtr.Zero && Marshal.GetLastWin32Error() != 0)
        {
            SetWindowLongPtr(fenetre, GWL_STYLE, new IntPtr(style));
            SetWindowLongPtr(fenetre, GWL_EXSTYLE, new IntPtr(ex));
            return null;
        }
        SetWindowPos(fenetre, IntPtr.Zero, 0, 0, largeur, hauteur, SWP_NOZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
        return new long[] { style, ex };
    }
    public static void Redimensionner(IntPtr fenetre, int largeur, int hauteur)
    {
        SetWindowPos(fenetre, IntPtr.Zero, 0, 0, largeur, hauteur, SWP_NOZORDER | SWP_NOACTIVATE);
    }
    public static void Liberer(IntPtr fenetre, long style, long ex)
    {
        if (!IsWindow(fenetre)) return;
        ShowWindow(fenetre, 0);
        SetParent(fenetre, IntPtr.Zero);
        SetWindowLongPtr(fenetre, GWL_STYLE, new IntPtr(style & ~WS_VISIBLE));
        SetWindowLongPtr(fenetre, GWL_EXSTYLE, new IntPtr(ex));
        SetWindowPos(fenetre, IntPtr.Zero, 0, 0, 0, 0, SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_NOACTIVATE);
    }
}
'@

$Exe = (Resolve-Path -LiteralPath $Exe).Path
$texte = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Exe))
if (-not $texte.Contains('MINDCARE_ESSAI_NOM')) { Write-Output "ÉCHEC : $Exe n'est pas la construction d'essai : rien n'est lancé."; exit 1 }
$racine = Join-Path $env:TEMP "mindcare-essai-echelle-$PID"
if (Test-Path -LiteralPath $racine) { Remove-Item -LiteralPath $racine -Recurse -Force }
$programmes = Join-Path $racine 'Programmes\Mindcare'
$donnees = Join-Path $racine 'Donnees\Mindcare'
$local = Join-Path $racine 'Local'
New-Item -ItemType Directory -Force -Path $programmes, $donnees, (Join-Path $local 'Mindset') | Out-Null
$exeEssai = Join-Path $programmes 'Mindset.exe'
Copy-Item -LiteralPath $Exe -Destination $exeEssai
Copy-Item -LiteralPath (Join-Path (Split-Path $PSScriptRoot -Parent) 'Mindcare\fichiers\enquetes.json') -Destination (Join-Path $programmes 'enquetes.json')
New-Item -ItemType Directory -Force -Path $Resultats | Out-Null
$nomEssai = "echelle-$PID"
$j = [ordered]@{ theme = 'neon'; micro = ''; casque = ''; applis = [ordered]@{ mindcare = $programmes }; modifie = (Get-Date -Format s) }
[IO.File]::WriteAllText((Join-Path $local 'Mindset\suite.json'), ($j | ConvertTo-Json -Depth 4), (New-Object Text.UTF8Encoding($false)))
$journal = Join-Path $donnees 'journal.txt'
# Le journal lu sans gêner l'appli qui y écrit (FileShare.ReadWrite)
function Get-Journal {
    if (-not (Test-Path -LiteralPath $journal)) { return @() }
    $f = New-Object IO.FileStream($journal, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    try { $l = (New-Object IO.StreamReader($f, [Text.Encoding]::UTF8)).ReadToEnd() } finally { $f.Dispose() }
    @($l -split "`r`n" | Where-Object { $_ })
}
function Compter([string]$Motif) { @(Get-Journal | Where-Object { $_ -like "*$Motif*" }).Count }
# (les messages de Windows de ce fil passent pendant l'attente : le panneau est une fenêtre de ce fil)
function Attendre([scriptblock]$Condition, [int]$Ms) {
    $t = [Diagnostics.Stopwatch]::StartNew()
    while ($t.ElapsedMilliseconds -lt $Ms) { [Windows.Forms.Application]::DoEvents(); if (& $Condition) { return $true }; Start-Sleep -Milliseconds 100 }
    [bool](& $Condition)
}
function Patienter([int]$Ms) { [void](Attendre { $false } $Ms) }
function Capturer([string]$Nom) {
    $c = Join-Path $donnees 'capture.png'
    if (Test-Path -LiteralPath $c) { Remove-Item -LiteralPath $c -Force }
    [IO.File]::WriteAllText((Join-Path $donnees 'capture.demande'), '1')
    $ok = Attendre { Test-Path -LiteralPath $c } 5000
    Verifier $ok "capture $Nom"
    if ($ok) {
        Patienter 300
        Copy-Item -LiteralPath $c -Destination (Join-Path $Resultats $Nom) -Force
        $ligne = @(Get-Journal | Where-Object { $_ -like '*Essai : capture de*' })[-1]
        Write-Output "Capture : $(Join-Path $Resultats $Nom) ($ligne)"
    }
}
# La dernière échelle notée par la construction d'essai : @{ Echelle; Largeur; Hauteur } (pixels logiques)
function Get-Echelle {
    $l = @(Get-Journal | Where-Object { $_ -match 'Essai : échelle ([0-9.]+) pour ([0-9]+) x ([0-9]+)' })
    if (-not $l.Count) { return $null }
    [void]($l[-1] -match 'Essai : échelle ([0-9.]+) pour ([0-9]+) x ([0-9]+)')
    $c = [Globalization.CultureInfo]::InvariantCulture
    @{ Echelle = [double]::Parse($Matches[1], $c); Largeur = [double]$Matches[2]; Hauteur = [double]$Matches[3]; Ligne = $l[-1] }
}
# Ce que donnerait Update-EchelleMindset (systeme.ps1:45), arrondi au millième vers le bas
function Get-EchelleAttendue([double]$L, [double]$H, [bool]$Dans) {
    if (-not $Dans -or $L -le 0 -or $H -le 0) { return 1.0 }
    [math]::Floor([math]::Min(1.0, [math]::Min($L / 900, $H / 620)) * 1000) / 1000
}
# Attend que l'échelle notée corresponde à la place actuelle de la fenêtre (taille lue par Windows, ramenée à son DPI)
function Verifier-Echelle([string]$Quand, [bool]$Dans) {
    $dpi = [EssaiEchelle]::GetDpiForWindow($f); if (-not $dpi) { $dpi = 96 }
    $L = [math]::Round([EssaiEchelle]::Largeur($f) * 96 / $dpi); $H = [math]::Round([EssaiEchelle]::Hauteur($f) * 96 / $dpi)
    $attendue = Get-EchelleAttendue $L $H $Dans
    $ok = Attendre { $e = Get-Echelle; ($null -eq $e -and $attendue -eq 1.0) -or ($null -ne $e -and [math]::Abs($e.Echelle - $attendue) -lt 0.0011) } 5000
    $e = Get-Echelle
    Verifier $ok "$Quand : place $L x $H (DPI $dpi), échelle attendue $attendue, notée : $(if ($e) { $e.Ligne } else { 'aucune (1)' })"
    $attendue
}

$psi = New-Object Diagnostics.ProcessStartInfo $exeEssai
$psi.Arguments = '-Demarrage'
$psi.UseShellExecute = $false
$psi.WorkingDirectory = $programmes
$psi.EnvironmentVariables['MINDCARE_DONNEES'] = $donnees
$psi.EnvironmentVariables['MINDCARE_ESSAI_LOCAL'] = $local
$psi.EnvironmentVariables['MINDCARE_ESSAI_NOM'] = $nomEssai
$p = [Diagnostics.Process]::Start($psi)
Write-Output "Lancée cachée : processus $($p.Id)"
$panneau = $null
try {
    $vu = Attendre { @([EssaiEchelle]::Fenetres($p.Id) | Where-Object { [EssaiEchelle]::Classe($_) -ceq 'Tauri Window' -and [EssaiEchelle]::Titre($_) -ceq 'Mindcare' }).Count -ge 1 } 20000
    Verifier $vu 'fenêtre « Mindcare »'
    $f = @([EssaiEchelle]::Fenetres($p.Id) | Where-Object { [EssaiEchelle]::Classe($_) -ceq 'Tauri Window' -and [EssaiEchelle]::Titre($_) -ceq 'Mindcare' })[0]
    Verifier (Attendre { (Compter 'Écran prêt') -ge 1 } 20000) 'le kit est prêt'
    Write-Output "DPI de la fenêtre : $([EssaiEchelle]::GetDpiForWindow($f))"
    # Montrée d'abord par le signal d'ESSAI (Local\MindcareEssai-...) : accrochée avant d'avoir jamais été montrée,
    # WebView2 n'a pas de fenêtre « Chrome_RenderWidgetHostHWND » et les messages de clavier envoyés par l'essai
    # n'arrivent pas (vu à l'essai du 03/11/2026). L'échelle, elle, n'en dépend pas : elle est vérifiée pareil.
    $s = [Threading.EventWaitHandle]::OpenExisting("Local\MindcareEssai-$nomEssai-Afficher"); [void]$s.Set(); $s.Dispose()
    Verifier (Attendre { [EssaiEchelle]::IsWindowVisible($f) } 5000) 'montrée par le signal d''essai'
    $cible = $null
    [void](Attendre { $script:cible = @([EssaiEchelle]::Enfants($f, 'Chrome_RenderWidgetHostHWND'))[0]; $null -ne $script:cible } 8000)
    Verifier ($null -ne $cible) 'fenêtre de rendu de WebView2 trouvée (le clavier)'
    Write-Output "Libre, à sa taille : $([EssaiEchelle]::Largeur($f)) x $([EssaiEchelle]::Hauteur($f))"

    # Le panneau de Mindset : une fenêtre Windows Forms montrée (sur ce bureau à part seulement), plus grande que tout
    $panneau = New-Object Windows.Forms.Form
    $panneau.FormBorderStyle = 'None'; $panneau.ShowInTaskbar = $false; $panneau.StartPosition = 'Manual'
    $panneau.Location = New-Object Drawing.Point(0, 0); $panneau.Size = New-Object Drawing.Size(1600, 1100)
    $panneau.BackColor = [Drawing.Color]::Black
    $panneau.Show()
    Patienter 300

    # --- 1. Accrochée à 700 x 500 ------------------------------------------------------------------------------------
    $styles = [EssaiEchelle]::Accrocher($f, $panneau.Handle, 700, 500)
    Verifier ($null -ne $styles -and [EssaiEchelle]::GetParent($f) -eq $panneau.Handle) 'accrochée dans le panneau (Accrocher de natif.cs)'
    Verifier (Attendre { (Compter 'Affichée dans Mindset') -ge 1 } 5000) 'la vie de l''appli la voit dans Mindset (journal)'
    $e1 = Verifier-Echelle 'accrochée à 700 x 500' $true
    Verifier ($e1 -lt 1.0) "réduite à 700 x 500 ($e1)"
    Patienter 2500
    Capturer 'echelle-700x500-accueil.png'
    if ($cible) {
        Write-Output "Zone de rendu : $([EssaiEchelle]::Largeur($cible)) x $([EssaiEchelle]::Hauteur($cible))"
        # flèche droite, Entrée : « Ça concerne quoi ? » (la page des familles, du texte et des cartes)
        [EssaiEchelle]::Touche($cible, 0x27); Patienter 700
        [EssaiEchelle]::Touche($cible, 0x0D)
        Verifier (Attendre { (Compter 'Écran : montre familles') -ge 1 } 5000) 'page des familles ouverte au clavier'
        Patienter 1200
        Capturer 'echelle-700x500-familles.png'
    }

    # --- 2. 1300 x 900 -----------------------------------------------------------------------------------------------
    [EssaiEchelle]::Redimensionner($f, 1300, 900)
    $e2 = Verifier-Echelle 'panneau de 1300 x 900' $true
    Patienter 1200
    Capturer 'echelle-1300x900-familles.png'

    # --- 3. 700 x 500 de nouveau, une page de texte (« Raconte ton problème ») -----------------------------------------
    [EssaiEchelle]::Redimensionner($f, 700, 500)
    $e3 = Verifier-Echelle 'de nouveau 700 x 500' $true
    Verifier ($e3 -eq $e1) "même échelle qu'au début ($e3)"
    if ($cible) {
        $accueils = Compter 'Écran : montre accueil'
        [EssaiEchelle]::Touche($cible, 0x1B)
        Verifier (Attendre { (Compter 'Écran : montre accueil') -gt $accueils } 5000) 'Échap : retour à l''accueil'
        Patienter 1200
        [EssaiEchelle]::Touche($cible, 0x25); Patienter 700
        [EssaiEchelle]::Touche($cible, 0x0D)
        Verifier (Attendre { (Compter 'Écran : montre raconte') -ge 1 } 5000) 'page « raconte » ouverte au clavier'
        Patienter 1200
        Capturer 'echelle-700x500-raconte.png'
    }

    # --- 4. Très petit : 300 x 200 -----------------------------------------------------------------------------------
    [EssaiEchelle]::Redimensionner($f, 300, 200)
    [void](Verifier-Echelle 'panneau de 300 x 200' $true)
    Patienter 1000
    Capturer 'echelle-300x200-raconte.png'

    # --- 5. Libérée (Mindset la rend, cachée) : échelle 1 ------------------------------------------------------------
    [EssaiEchelle]::Liberer($f, $styles[0], $styles[1])
    Verifier (Attendre { (Compter 'Plus affichée dans Mindset') -ge 1 } 5000) 'libérée : la vie de l''appli le voit (journal)'
    $e5 = Verifier-Echelle 'libérée' $false
    Verifier ($e5 -eq 1.0) 'libérée : échelle 1'
    # montrée dans sa propre fenêtre (sur ce bureau à part), sans prendre la main : à sa taille, échelle 1
    [void][EssaiEchelle]::ShowWindow($f, 8)   # SW_SHOWNA
    Patienter 1500
    Write-Output "Libérée, montrée : $([EssaiEchelle]::Largeur($f)) x $([EssaiEchelle]::Hauteur($f))"
    Verifier ((Get-Echelle).Echelle -eq 1.0) 'libérée et montrée : toujours 1'
    Capturer 'echelle-liberee-raconte.png'
}
catch { Echec "erreur du script : $_ $($_.InvocationInfo.PositionMessage)" }
finally {
    [IO.File]::WriteAllText((Join-Path $donnees 'quitter.demande'), "$PID")
    if (-not $p.WaitForExit(8000)) { try { $p.Kill() } catch {} }
    if ($panneau) { $panneau.Dispose() }
    Start-Sleep -Milliseconds 1500
    Get-Journal | Set-Content -LiteralPath (Join-Path $Resultats 'journal-echelle.txt') -Encoding UTF8
    Write-Output '--- Journal ---'
    Get-Journal | ForEach-Object { Write-Output "  $_" }
    $restants = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $exeEssai })
    $restants += @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" | Where-Object { $_.CommandLine -like "*$racine*" })
    Verifier ($restants.Count -eq 0) "plus aucun processus de l'essai"
    foreach ($r in $restants) { try { Stop-Process -Id $r.ProcessId -Force -ErrorAction Stop } catch {} }
    try { Remove-Item -LiteralPath $racine -Recurse -Force -ErrorAction Stop } catch { Write-Output "Dossier d'essai pas encore effacé : $racine" }
}
if ($echecs.Count) { Write-Output "$($echecs.Count) échec(s)."; exit 1 }
Write-Output 'Tout est bon.'
exit 0
