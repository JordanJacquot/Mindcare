# Essai réel de la fenêtre de Mindcare en Rust (étape 4.4), avec la CONSTRUCTION D'ESSAI de Mindset.exe (option cargo
# « essai ») et un dossier d'essai sous %TEMP%. À lancer SEULEMENT sur un bureau Windows à part :
#
#   & '.\outils\sur-bureau.ps1' -Script '.\outils\essai-fenetre.ps1' -Arguments "-Exe '<Mindset.exe d'essai>' -Resultats '<dossier>'"
#
# Le script refuse un exe qui n'est pas la construction d'essai (il ne lancerait jamais l'exe livré : une copie qui
# se croit gérée ouvrirait le vrai Mindset du PC). La construction d'essai ne lit pas HKLM, ne lance qu'elle-même, et
# n'utilise que le dossier d'essai et des noms de verrou et de signal d'essai (Local\MindcareEssai-...).
#
# Ce qui est vérifié, dans l'ordre :
#  1. lancée cachée (-Demarrage) : une fenêtre de classe « Tauri Window », titre exactement « Mindcare », sans
#     propriétaire, cachée ; les réglages de WebView2 coupés (journal) ; la page passe à « caché » (journal) ;
#  2. EBWebView créé seulement sous <essai>\Donnees\WebView2-Tauri\Mindcare (ni dans le vrai %LOCALAPPDATA%\Mindset,
#     ni dans le dossier par défaut de Tauri, ni ailleurs dans l'essai) ;
#  3. un second lancement caché quitte sans rien montrer (et sans toucher à la première) ;
#  4. le signal d'affichage la montre ; la page revient « à l'écran » (journal) ; une capture faite par l'exe ;
#  5. cachée de nouveau : la page repasse à « caché » (journal) ;
#  6. quitter.demande : elle quitte en moins de 6 s ;
#  7. relancée, WM_CLOSE (le bouton « Fermer » de Mindset) : elle quitte ;
#  8. relancée, un nouveau thème dans suite.json : elle se relance cachée (nouveau processus), qui quitte à son tour
#     par quitter.demande ;
#  9. plus aucun processus de l'essai.
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
public static class EssaiFenetre
{
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    public static string Classe(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
    public static string Titre(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
    public static IntPtr Proprietaire(IntPtr h) { return GetWindow(h, 4); }   // GW_OWNER
    // Les fenêtres de premier niveau d'un processus (sur le bureau de ce fil : le bureau à part)
    public static IntPtr[] Fenetres(int pid)
    {
        var l = new List<IntPtr>();
        EnumWindows(delegate (IntPtr h, IntPtr x) { int p; GetWindowThreadProcessId(h, out p); if (p == pid) l.Add(h); return true; }, IntPtr.Zero);
        return l.ToArray();
    }
}
'@

# --- Le dossier d'essai ----------------------------------------------------------------------------------------------
$Exe = (Resolve-Path -LiteralPath $Exe).Path
$octets = [IO.File]::ReadAllBytes($Exe)
$texte = [Text.Encoding]::ASCII.GetString($octets)
if (-not $texte.Contains('MINDCARE_ESSAI_NOM')) {
    Write-Output "ÉCHEC : $Exe n'est pas la construction d'essai (option cargo « essai ») : rien n'est lancé."
    exit 1
}
$racine = Join-Path $env:TEMP "mindcare-essai-fenetre-$PID"
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
$nomEssai = "fenetre-$PID"
$suite = Join-Path $local 'Mindset\suite.json'
function Write-Suite([string]$Theme) {
    $j = [ordered]@{ theme = $Theme; micro = ''; casque = ''; applis = [ordered]@{ mindcare = $programmes }; modifie = (Get-Date -Format s) }
    [IO.File]::WriteAllText($suite, ($j | ConvertTo-Json -Depth 4), (New-Object Text.UTF8Encoding($false)))
}
Write-Suite 'crepuscule'
$journal = Join-Path $donnees 'journal.txt'
# Le journal lu sans gêner l'appli qui y écrit (FileShare.ReadWrite : ReadAllLines le bloquerait en écriture le temps
# de la lecture, et la ligne écrite à ce moment-là serait perdue)
function Get-Journal {
    if (-not (Test-Path -LiteralPath $journal)) { return @() }
    $f = New-Object IO.FileStream($journal, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    try { $l = (New-Object IO.StreamReader($f, [Text.Encoding]::UTF8)).ReadToEnd() } finally { $f.Dispose() }
    @($l -split "`r`n" | Where-Object { $_ })
}
function Compter([string]$Motif) { @(Get-Journal | Where-Object { $_ -like "*$Motif*" }).Count }

# Ce qui existe déjà hors de l'essai (comparé à la fin : rien ne doit y apparaître)
$horsEssai = @(
    (Join-Path $env:LOCALAPPDATA 'Mindset\WebView2-Tauri\Mindcare'),
    (Join-Path $env:LOCALAPPDATA 'JordanJacquot.Mindcare'),
    (Join-Path $env:LOCALAPPDATA 'Mindset\Mindcare\ecran-webview')
)
$avant = @{}
foreach ($d in $horsEssai) { $avant[$d] = if (Test-Path -LiteralPath $d) { (Get-Item -LiteralPath $d).LastWriteTimeUtc } else { $null } }

function Lancer([string[]]$Arguments) {
    $psi = New-Object Diagnostics.ProcessStartInfo $exeEssai
    $psi.Arguments = ($Arguments -join ' ')
    $psi.UseShellExecute = $false
    $psi.WorkingDirectory = $programmes
    $psi.EnvironmentVariables['MINDCARE_DONNEES'] = $donnees
    $psi.EnvironmentVariables['MINDCARE_ESSAI_LOCAL'] = $local
    $psi.EnvironmentVariables['MINDCARE_ESSAI_NOM'] = $nomEssai
    [Diagnostics.Process]::Start($psi)
}
function Attendre([scriptblock]$Condition, [int]$Ms) {
    $t = [Diagnostics.Stopwatch]::StartNew()
    while ($t.ElapsedMilliseconds -lt $Ms) { if (& $Condition) { return $true }; Start-Sleep -Milliseconds 100 }
    [bool](& $Condition)
}
function Get-Principale([int]$IdProcessus) {
    @([EssaiFenetre]::Fenetres($IdProcessus) | Where-Object { [EssaiFenetre]::Classe($_) -ceq 'Tauri Window' -and [EssaiFenetre]::Titre($_) -ceq 'Mindcare' })
}
$tousLesPid = New-Object System.Collections.Generic.List[int]

# --- 1. Lancée cachée ------------------------------------------------------------------------------------------------
$p1 = Lancer @('-Demarrage')
$tousLesPid.Add($p1.Id)
Write-Output "Lancée cachée : processus $($p1.Id)"
$vu = Attendre { (Get-Principale $p1.Id).Count -ge 1 } 20000
Verifier $vu 'une fenêtre « Tauri Window » titrée exactement « Mindcare »'
$f = (Get-Principale $p1.Id) | Select-Object -First 1
if ($f) {
    $toutes = [EssaiFenetre]::Fenetres($p1.Id) | ForEach-Object { "$([EssaiFenetre]::Classe($_)) « $([EssaiFenetre]::Titre($_)) » visible=$([EssaiFenetre]::IsWindowVisible($_))" }
    Write-Output "Fenêtres du processus : $($toutes -join ' ; ')"
    Verifier ((Get-Principale $p1.Id).Count -eq 1) 'une seule fenêtre principale'
    Verifier ([EssaiFenetre]::Proprietaire($f) -eq [IntPtr]::Zero) 'sans propriétaire'
    Verifier (-not [EssaiFenetre]::IsWindowVisible($f)) 'cachée au départ (-Demarrage)'
}
Verifier (Attendre { (Compter 'Écran : menu du clic droit, raccourcis du navigateur, saisie automatique, mots de passe, outils de développement et zoom coupés') -ge 1 } 15000) 'réglages de WebView2 coupés et relus (journal)'
Verifier (Attendre { (Compter 'Écran : caché, en pause') -ge 1 } 15000) 'la page cachée passe à « caché » (journal)'

# --- 2. EBWebView seulement sous le dossier d'essai ------------------------------------------------------------------
$attendu = Join-Path $racine 'Donnees\WebView2-Tauri\Mindcare\EBWebView'
Verifier (Attendre { Test-Path -LiteralPath $attendu } 10000) "EBWebView créé sous $attendu"
$autres = @(Get-ChildItem -LiteralPath $racine -Recurse -Directory -Filter 'EBWebView' -ErrorAction SilentlyContinue | Where-Object { $_.FullName -ne $attendu })
Verifier ($autres.Count -eq 0) "aucun autre EBWebView dans l'essai ($($autres.FullName -join ', '))"

# --- 3. Second lancement caché ---------------------------------------------------------------------------------------
$p2 = Lancer @('-Demarrage')
$tousLesPid.Add($p2.Id)
$sorti = $p2.WaitForExit(12000)
Verifier $sorti "le second lancement caché quitte (verrou tenu, attente de 5 s)"
if ($sorti) { Verifier ((Get-Principale $p2.Id).Count -eq 0) 'le second lancement ne montre rien' }
Verifier (-not $p1.HasExited) 'la première copie tourne toujours'
if ($f) { Verifier (-not [EssaiFenetre]::IsWindowVisible($f)) 'la première reste cachée' }

# --- 4. Le signal d'affichage la montre ------------------------------------------------------------------------------
$visiblesAvant = Compter "Écran : de nouveau à l'écran"
$signal = [Threading.EventWaitHandle]::OpenExisting("Local\MindcareEssai-$nomEssai-Afficher")
[void]$signal.Set(); $signal.Dispose()
if ($f) { Verifier (Attendre { [EssaiFenetre]::IsWindowVisible($f) } 5000) "le signal d'affichage la montre" }
Verifier (Attendre { (Compter "Écran : de nouveau à l'écran") -gt $visiblesAvant } 8000) "la page revient « à l'écran » (journal)"
Start-Sleep -Milliseconds 1500
$capture = Join-Path $donnees 'capture.png'
[IO.File]::WriteAllText((Join-Path $donnees 'capture.demande'), '1')
Verifier (Attendre { Test-Path -LiteralPath $capture } 5000) 'capture faite par la construction d''essai'
if (Test-Path -LiteralPath $capture) {
    Start-Sleep -Milliseconds 300
    Copy-Item -LiteralPath $capture -Destination (Join-Path $Resultats 'capture-fenetre.png') -Force
    Write-Output "Capture : $(Join-Path $Resultats 'capture-fenetre.png') ($((Get-Item -LiteralPath $capture).Length) octets)"
}

# --- 5. Cachée de nouveau : la page en pause -------------------------------------------------------------------------
$cachesAvant = Compter 'Écran : caché, en pause'
if ($f) { [void][EssaiFenetre]::ShowWindow($f, 0) }   # SW_HIDE (sur le bureau à part)
Verifier (Attendre { (Compter 'Écran : caché, en pause') -gt $cachesAvant } 8000) 'cachée de nouveau : la page repasse à « caché » (journal)'

# --- 6. quitter.demande ----------------------------------------------------------------------------------------------
$t = [Diagnostics.Stopwatch]::StartNew()
[IO.File]::WriteAllText((Join-Path $donnees 'quitter.demande'), "$PID")
$sorti = $p1.WaitForExit(6000)
Verifier $sorti "quitter.demande : elle quitte en moins de 6 s ($($t.ElapsedMilliseconds) ms)"
Verifier (-not (Test-Path -LiteralPath (Join-Path $donnees 'quitter.demande'))) 'quitter.demande effacé'
if ($f) { Verifier (-not [EssaiFenetre]::IsWindow($f)) 'sa fenêtre a disparu' }

# --- 7. WM_CLOSE -----------------------------------------------------------------------------------------------------
$p3 = Lancer @('-Demarrage')
$tousLesPid.Add($p3.Id)
$ok = Attendre { (Get-Principale $p3.Id).Count -ge 1 } 20000
Verifier $ok 'relancée : sa fenêtre est là'
if ($ok) {
    Start-Sleep -Milliseconds 1500
    $f3 = (Get-Principale $p3.Id) | Select-Object -First 1
    $t = [Diagnostics.Stopwatch]::StartNew()
    [void][EssaiFenetre]::PostMessage($f3, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)   # WM_CLOSE
    Verifier ($p3.WaitForExit(6000)) "WM_CLOSE : elle quitte ($($t.ElapsedMilliseconds) ms)"
}

# --- 8. Nouveau thème : relance cachée -------------------------------------------------------------------------------
$p4 = Lancer @('-Demarrage')
$tousLesPid.Add($p4.Id)
$ok = Attendre { (Get-Principale $p4.Id).Count -ge 1 } 20000
Verifier $ok 'relancée pour le thème : sa fenêtre est là'
if ($ok) {
    Start-Sleep -Milliseconds 3000   # (la date de suite.json est notée au premier tour)
    Write-Suite 'rubis'
    Verifier ($p4.WaitForExit(8000)) 'nouveau thème : la copie quitte pour se relancer'
    $relance = $null
    [void](Attendre {
        $script:relance = @(Get-CimInstance Win32_Process -Filter "Name = 'Mindset.exe'" | Where-Object { $_.ExecutablePath -eq $exeEssai -and $_.ProcessId -ne $p4.Id -and -not $tousLesPid.Contains([int]$_.ProcessId) }) | Select-Object -First 1
        $null -ne $script:relance
    } 10000)
    Verifier ($null -ne $relance) 'une nouvelle copie a été lancée'
    if ($relance) {
        $tousLesPid.Add([int]$relance.ProcessId)
        Verifier ($relance.CommandLine -match '-Demarrage') "relancée cachée (ligne : $($relance.CommandLine))"
        Verifier (Attendre { (Get-Principale ([int]$relance.ProcessId)).Count -ge 1 } 20000) 'la copie relancée a sa fenêtre'
        $fr = (Get-Principale ([int]$relance.ProcessId)) | Select-Object -First 1
        if ($fr) { Verifier (-not [EssaiFenetre]::IsWindowVisible($fr)) 'la copie relancée est cachée' }
        Verifier ((Compter 'Thème de la suite changé : rubis, relance') -ge 1) 'relance notée au journal'
        Start-Sleep -Milliseconds 1500
        [IO.File]::WriteAllText((Join-Path $donnees 'quitter.demande'), "$PID")
        $pr = Get-Process -Id ([int]$relance.ProcessId) -ErrorAction SilentlyContinue
        Verifier ((-not $pr) -or $pr.WaitForExit(6000)) 'la copie relancée quitte par quitter.demande'
    }
}

# --- 9. Plus aucun processus de l'essai ------------------------------------------------------------------------------
Start-Sleep -Milliseconds 2000
$restants = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $exeEssai })
$restants += @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" | Where-Object { $_.CommandLine -like "*$racine*" })
Verifier ($restants.Count -eq 0) "plus aucun processus de l'essai ($(@($restants | ForEach-Object { "$($_.Name) $($_.ProcessId)" }) -join ', '))"
foreach ($r in $restants) { try { Stop-Process -Id $r.ProcessId -Force -ErrorAction Stop } catch {} }

# Rien n'est apparu hors de l'essai
foreach ($d in $horsEssai) {
    $maintenant = if (Test-Path -LiteralPath $d) { (Get-Item -LiteralPath $d).LastWriteTimeUtc } else { $null }
    Verifier ($maintenant -eq $avant[$d]) "rien de créé ni changé hors de l'essai : $d"
}

# Le journal, gardé avec les résultats
Get-Journal | Set-Content -LiteralPath (Join-Path $Resultats 'journal-essai.txt') -Encoding UTF8
Write-Output '--- Journal de l''essai ---'
Get-Journal | ForEach-Object { Write-Output "  $_" }
Start-Sleep -Milliseconds 1000
try { Remove-Item -LiteralPath $racine -Recurse -Force -ErrorAction Stop } catch { Write-Output "Dossier d'essai pas encore effacé : $racine" }

if ($echecs.Count) { Write-Output "$($echecs.Count) échec(s)."; exit 1 }
Write-Output 'Tout est bon.'
exit 0
