# Essai du kit d'écrans dans la fenêtre Tauri de Mindcare (étape 5.1, avec les vraies pages depuis l'étape 5.3), avec la
# CONSTRUCTION D'ESSAI. À lancer SEULEMENT sur un bureau Windows à part (outils\sur-bureau.ps1). Captures : accueil
# (3D), puis une page ; un clic simulé (messages de souris et de clavier envoyés à la fenêtre de WebView2) doit
# arriver au journal par la commande action.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Resultats
)
$ErrorActionPreference = 'Stop'
$echecs = New-Object System.Collections.Generic.List[string]
function Ok([string]$t) { Write-Output "OK : $t" }
function Echec([string]$t) { Write-Output "ÉCHEC : $t"; $echecs.Add($t) }
function Verifier([bool]$c, [string]$t) { if ($c) { Ok $t } else { Echec $t } }

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class EssaiKit
{
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
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
    public static int Largeur(IntPtr h) { RECT r; GetClientRect(h, out r); return r.r - r.l; }
    public static int Hauteur(IntPtr h) { RECT r; GetClientRect(h, out r); return r.b - r.t; }
    public static void Clic(IntPtr h, int x, int y)
    {
        IntPtr pos = (IntPtr)((y << 16) | (x & 0xFFFF));
        PostMessage(h, 0x0200, IntPtr.Zero, pos);          // WM_MOUSEMOVE
        PostMessage(h, 0x0201, (IntPtr)1, pos);            // WM_LBUTTONDOWN (MK_LBUTTON)
        PostMessage(h, 0x0202, IntPtr.Zero, pos);          // WM_LBUTTONUP
    }
    public static void Touche(IntPtr h, int vk)
    {
        PostMessage(h, 0x0100, (IntPtr)vk, (IntPtr)1);                       // WM_KEYDOWN
        PostMessage(h, 0x0101, (IntPtr)vk, (IntPtr)unchecked((int)0xC0000001)); // WM_KEYUP
    }
}
'@

$Exe = (Resolve-Path -LiteralPath $Exe).Path
$texte = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Exe))
if (-not $texte.Contains('MINDCARE_ESSAI_NOM')) { Write-Output "ÉCHEC : $Exe n'est pas la construction d'essai : rien n'est lancé."; exit 1 }
$racine = Join-Path $env:TEMP "mindcare-essai-kit-$PID"
if (Test-Path -LiteralPath $racine) { Remove-Item -LiteralPath $racine -Recurse -Force }
$programmes = Join-Path $racine 'Programmes\Mindcare'
$donnees = Join-Path $racine 'Donnees\Mindcare'
$local = Join-Path $racine 'Local'
New-Item -ItemType Directory -Force -Path $programmes, $donnees, (Join-Path $local 'Mindset') | Out-Null
$exeEssai = Join-Path $programmes 'Mindset.exe'
Copy-Item -LiteralPath $Exe -Destination $exeEssai
# (le catalogue des enquêtes, à côté du programme comme dans le dossier construit)
Copy-Item -LiteralPath (Join-Path (Split-Path $PSScriptRoot -Parent) 'Mindcare\fichiers\enquetes.json') -Destination (Join-Path $programmes 'enquetes.json')
New-Item -ItemType Directory -Force -Path $Resultats | Out-Null
$nomEssai = "kit-$PID"
$j = [ordered]@{ theme = 'neon'; micro = ''; casque = ''; applis = [ordered]@{ mindcare = $programmes }; modifie = (Get-Date -Format s) }
[IO.File]::WriteAllText((Join-Path $local 'Mindset\suite.json'), ($j | ConvertTo-Json -Depth 4), (New-Object Text.UTF8Encoding($false)))
$journal = Join-Path $donnees 'journal.txt'
function Get-Journal { if (Test-Path -LiteralPath $journal) { @([IO.File]::ReadAllLines($journal, [Text.Encoding]::UTF8)) } else { @() } }
function Compter([string]$Motif) { @(Get-Journal | Where-Object { $_ -like "*$Motif*" }).Count }
function Attendre([scriptblock]$Condition, [int]$Ms) {
    $t = [Diagnostics.Stopwatch]::StartNew()
    while ($t.ElapsedMilliseconds -lt $Ms) { if (& $Condition) { return $true }; Start-Sleep -Milliseconds 100 }
    [bool](& $Condition)
}
function Capturer([string]$Nom) {
    $c = Join-Path $donnees 'capture.png'
    if (Test-Path -LiteralPath $c) { Remove-Item -LiteralPath $c -Force }
    [IO.File]::WriteAllText((Join-Path $donnees 'capture.demande'), '1')
    $ok = Attendre { Test-Path -LiteralPath $c } 5000
    Verifier $ok "capture $Nom"
    if ($ok) { Start-Sleep -Milliseconds 300; Copy-Item -LiteralPath $c -Destination (Join-Path $Resultats $Nom) -Force; Write-Output "Capture : $(Join-Path $Resultats $Nom)" }
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
try {
    $vu = Attendre { @([EssaiKit]::Fenetres($p.Id) | Where-Object { [EssaiKit]::Classe($_) -ceq 'Tauri Window' -and [EssaiKit]::Titre($_) -ceq 'Mindcare' }).Count -ge 1 } 20000
    Verifier $vu 'fenêtre « Mindcare »'
    $f = @([EssaiKit]::Fenetres($p.Id) | Where-Object { [EssaiKit]::Classe($_) -ceq 'Tauri Window' -and [EssaiKit]::Titre($_) -ceq 'Mindcare' })[0]
    Verifier (Attendre { (Compter 'Écran prêt') -ge 1 } 20000) 'le kit est prêt (message « pret » reçu par la commande action)'
    # Le signal d'ESSAI (Local\MindcareEssai-...) la montre
    $s = [Threading.EventWaitHandle]::OpenExisting("Local\MindcareEssai-$nomEssai-Afficher"); [void]$s.Set(); $s.Dispose()
    Verifier (Attendre { [EssaiKit]::IsWindowVisible($f) } 5000) 'montrée par le signal d''essai'
    Start-Sleep -Milliseconds 3500
    Capturer 'kit-accueil.png'

    # La fenêtre de WebView2 où arrivent la souris et le clavier
    $cible = $null
    [void](Attendre { $script:cible = @([EssaiKit]::Enfants($f, 'Chrome_RenderWidgetHostHWND'))[0]; $null -ne $script:cible } 5000)
    Verifier ($null -ne $cible) 'fenêtre de rendu de WebView2 trouvée'
    if ($cible) {
        $l = [EssaiKit]::Largeur($cible); $h = [EssaiKit]::Hauteur($cible)
        Write-Output "Zone de rendu : $l x $h"
        # 1. Un clic au milieu : la tuile du milieu (« Raconte ton problème », qui a sa page) l'ouvre
        $avant = Compter 'Écran : montre raconte'
        [EssaiKit]::Clic($cible, [int]($l / 2), [int]($h / 2))
        Verifier (Attendre { (Compter 'Écran : montre raconte') -gt $avant } 5000) 'clic sur la tuile du milieu : page « raconte » (message « ecran » au journal)'
        Start-Sleep -Milliseconds 1500
        Capturer 'kit-page-raconte.png'
        # 2. Échap : retour à l'accueil ; flèche droite, Entrée : la tuile « Ça concerne quoi ? » ouvre la page des familles
        #    (étape 5.3 : les vraies pages, elle a sa page)
        $accueils = Compter 'Écran : montre accueil'
        [EssaiKit]::Touche($cible, 0x1B)
        Verifier (Attendre { (Compter 'Écran : montre accueil') -gt $accueils } 5000) 'Échap : retour à l''accueil (journal)'
        Start-Sleep -Milliseconds 1200
        [EssaiKit]::Touche($cible, 0x27)
        Start-Sleep -Milliseconds 700
        [EssaiKit]::Touche($cible, 0x0D)
        $okAction = Attendre { (Compter 'Écran : montre familles') -ge 1 } 5000
        if (-not $okAction) {
            # (si le clavier n'arrive pas : un clic sur la tuile choisie)
            [EssaiKit]::Clic($cible, [int]($l / 2), [int]($h / 2))
            $okAction = Attendre { (Compter 'Écran : montre familles') -ge 1 } 5000
        }
        Verifier $okAction 'la tuile « Ça concerne quoi ? » ouvre les familles (message « ecran » au journal)'
        Start-Sleep -Milliseconds 800
        Capturer 'kit-familles.png'
        # 3. Échap : retour à l'accueil, puis Échap sur l'accueil : le message « menu »
        $accueils = Compter 'Écran : montre accueil'
        [EssaiKit]::Touche($cible, 0x1B)
        Verifier (Attendre { (Compter 'Écran : montre accueil') -gt $accueils } 5000) 'Échap : retour à l''accueil (journal)'
        Start-Sleep -Milliseconds 1200
        [EssaiKit]::Touche($cible, 0x1B)
        Verifier (Attendre { (Compter 'Écran : retour au menu de Mindset demandé') -ge 1 } 5000) 'Échap sur l''accueil : message « menu » (journal)'
    }
}
finally {
    [IO.File]::WriteAllText((Join-Path $donnees 'quitter.demande'), "$PID")
    if (-not $p.WaitForExit(8000)) { try { $p.Kill() } catch {} }
    Start-Sleep -Milliseconds 1500
    Get-Journal | Set-Content -LiteralPath (Join-Path $Resultats 'journal-kit.txt') -Encoding UTF8
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
