# L'assistant et la dictée de Mindcare en Rust, de bout en bout, avec la CONSTRUCTION D'ESSAI (étape 6.4) et des FAUX :
# un faux serveur de téléchargement sur ce PC (127.0.0.1, dans ce script), le faux llama-server et le faux whisper des
# tests des crates (servis par ce faux serveur, puis lancés par Mindcare comme les vrais : sans fenêtre, arrêtés avec
# elle), et le faux micro de la construction d'essai (le vrai micro n'est JAMAIS ouvert). Rien n'est pris sur Internet.
# À lancer SEULEMENT sur un bureau Windows à part (outils\sur-bureau.ps1) :
#
#   & '.\outils\sur-bureau.ps1' -Script '.\outils\essai-aides.ps1' -Arguments "-Exe <Mindset.exe d'essai> -FauxLlama <faux-llama-server.exe> -FauxWhisper <faux-whisper.exe> -Resultats <dossier>" -Secondes 900
#
# Pour chaque thème : installer l'assistant et la dictée depuis le faux serveur, une phrase hésitante (« Pour être
# sûr »), une phrase sûre (l'enquête tout de suite, avec les réponses déjà données), une réponse illisible, une phrase
# dictée ; puis un deuxième lancement avec un ancien assistant (à mettre à jour) et une dictée coupée (à réparer, puis
# en erreur : son fichier arrive abîmé). Les captures vont dans <Resultats>\<thème>\NN-nom.png, avec
# <Resultats>\index.txt (fichier, écran, ce qu'on doit y voir).
#
# Rien du vrai PC : un dossier d'essai dans %TEMP% (données, faux %LOCALAPPDATA%, Temp), les verrou et signal d'ESSAI
# (Local\MindcareEssai-...), jamais ceux de la vraie Mindcare ni de Mindset ; seuls les processus de l'essai sont arrêtés.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$FauxLlama,
    [Parameter(Mandatory = $true)][string]$FauxWhisper,
    [Parameter(Mandatory = $true)][string]$Resultats,
    [string[]]$Themes = @('neon', 'crepuscule', 'terminal')
)
$ErrorActionPreference = 'Stop'
$echecs = New-Object System.Collections.Generic.List[string]
function Ok([string]$t) { Write-Host "OK : $t" }
function Echec([string]$t) { Write-Host "ÉCHEC : $t"; $echecs.Add($t) }
function Verifier([bool]$c, [string]$t) { if ($c) { Ok $t } else { Echec $t } }

Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class EssaiAides
{
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int x, y; }
    public static IntPtr[] Enfants(IntPtr parent, string classe)
    {
        var l = new List<IntPtr>();
        EnumChildWindows(parent, delegate (IntPtr h, IntPtr x) { if (Classe(h) == classe) l.Add(h); return true; }, IntPtr.Zero);
        return l.ToArray();
    }
    // La molette (WM_MOUSEWHEEL, sur la page seulement : rien n'est envoyé au vrai bureau) : la page défile
    public static void Molette(IntPtr h, int x, int y, int crans)
    {
        var p = new POINT(); p.x = x; p.y = y; ClientToScreen(h, ref p);
        IntPtr pos = (IntPtr)((p.y << 16) | (p.x & 0xFFFF));
        PostMessage(h, 0x0200, IntPtr.Zero, (IntPtr)((y << 16) | (x & 0xFFFF)));
        for (int i = 0; i < Math.Abs(crans); i++) PostMessage(h, 0x020A, (IntPtr)((crans < 0 ? -120 : 120) << 16), pos);
    }
    public static string Classe(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
    public static string Titre(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
    public static IntPtr[] Fenetres(int pid)
    {
        var l = new List<IntPtr>();
        EnumWindows(delegate (IntPtr h, IntPtr x) { int p; GetWindowThreadProcessId(h, out p); if (p == pid) l.Add(h); return true; }, IntPtr.Zero);
        return l.ToArray();
    }
}
'@

foreach ($f in $Exe, $FauxLlama, $FauxWhisper) { if (-not (Test-Path -LiteralPath $f -PathType Leaf)) { Write-Output "ÉCHEC : introuvable : $f"; exit 1 } }
$Exe = (Resolve-Path -LiteralPath $Exe).Path
$texte = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Exe))
if (-not $texte.Contains('actions.demande') -or -not $texte.Contains('MINDCARE_ESSAI_DEPOT')) { Write-Output "ÉCHEC : $Exe n'est pas la construction d'essai de l'étape 6.4 : rien n'est lancé."; exit 1 }
$depotGit = Split-Path $PSScriptRoot -Parent
New-Item -ItemType Directory -Force -Path $Resultats | Out-Null
$Resultats = (Resolve-Path -LiteralPath $Resultats).Path
$index = New-Object System.Collections.Generic.List[string]
$utf8 = New-Object Text.UTF8Encoding($false)
$racineEssai = Join-Path $env:TEMP "mindcare-essai-aides-$PID"
if (Test-Path -LiteralPath $racineEssai) { Remove-Item -LiteralPath $racineEssai -Recurse -Force }
New-Item -ItemType Directory -Force -Path $racineEssai | Out-Null

# ------------------------------------------------------------------------------------------------ le faux serveur
# Les fichiers donnés : les mêmes noms que les vrais (le nom de l'adresse donne celui du fichier en cours), des
# contenus faux. Le moteur de l'assistant : le faux llama-server sous le nom llama-server.exe, ses .dll ; celui de la
# dictée : le faux whisper sous le nom whisper-cli.exe, ses .dll. Les modèles : quelques Mo au hasard (l'avancement se
# voit : le serveur donne 64 Ko toutes les 40 ms).
$depot = Join-Path $racineEssai 'depot'
New-Item -ItemType Directory -Force -Path $depot | Out-Null
function New-Zip([string]$Nom, [hashtable]$Entrees) {
    $chemin = Join-Path $depot $Nom
    $z = [IO.Compression.ZipFile]::Open($chemin, 'Create')
    try {
        foreach ($k in $Entrees.Keys) {
            $e = $z.CreateEntry($k)
            $s = $e.Open(); try { $o = $Entrees[$k]; $s.Write($o, 0, $o.Length) } finally { $s.Dispose() }
        }
    } finally { $z.Dispose() }
}
function Get-Octets([int]$N, [int]$Graine) { $r = New-Object Random $Graine; $o = New-Object byte[] $N; $r.NextBytes($o); , $o }
$faux = { param($t) , [Text.Encoding]::ASCII.GetBytes($t) }
New-Zip 'llama-b11541-bin-win-vulkan-x64.zip' @{
    'llama-b11541/llama-server.exe' = [IO.File]::ReadAllBytes($FauxLlama)
    'llama-b11541/llama.dll' = (& $faux 'faux llama'); 'llama-b11541/ggml.dll' = (& $faux 'faux ggml')
    'llama-b11541/ggml-base.dll' = (& $faux 'faux ggml-base'); 'llama-b11541/LICENSE' = (& $faux 'pas voulu')
}
[IO.File]::WriteAllBytes((Join-Path $depot 'mindcare-0.6b-v3-q8_0.gguf'), (Get-Octets (6MB) 3))
New-Zip 'whisper-bin-x64.zip' @{
    'Release/whisper-cli.exe' = [IO.File]::ReadAllBytes($FauxWhisper)
    'Release/whisper.dll' = (& $faux 'faux whisper'); 'Release/ggml.dll' = (& $faux 'faux ggml'); 'Release/ggml-base.dll' = (& $faux 'faux ggml-base')
}
[IO.File]::WriteAllBytes((Join-Path $depot 'ggml-small-q5_1.bin'), (Get-Octets (3MB) 4))

$ecoute = New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback, 0)
$ecoute.Start()
$port = $ecoute.LocalEndpoint.Port
$serveur = [PowerShell]::Create()
[void]$serveur.AddScript({
    param($Ecoute, $Dossier)
    while ($true) {
        $c = $Ecoute.AcceptTcpClient()
        try {
            $s = $c.GetStream(); $lu = New-Object IO.StreamReader($s)
            $ligne = $lu.ReadLine(); while ($lu.ReadLine()) {}
            $f = Join-Path $Dossier ([uri]::UnescapeDataString(($ligne -split ' ')[1].TrimStart('/')))
            if (-not (Test-Path -LiteralPath $f -PathType Leaf)) {
                $t = [Text.Encoding]::ASCII.GetBytes("HTTP/1.1 404 Non`r`nContent-Length: 0`r`nConnection: close`r`n`r`n"); $s.Write($t, 0, $t.Length)
            } else {
                $octets = [IO.File]::ReadAllBytes($f)
                $t = [Text.Encoding]::ASCII.GetBytes("HTTP/1.1 200 OK`r`nContent-Length: $($octets.Length)`r`nConnection: close`r`n`r`n")
                $s.Write($t, 0, $t.Length)
                for ($i = 0; $i -lt $octets.Length; $i += 65536) { $s.Write($octets, $i, [math]::Min(65536, $octets.Length - $i)); $s.Flush(); Start-Sleep -Milliseconds 40 }
            }
            $s.Flush()
        } catch {} finally { $c.Close() }
    }
}).AddArgument($ecoute).AddArgument($depot)
[void]$serveur.BeginInvoke()
Write-Host "Faux serveur sur 127.0.0.1:$port"

# depot.json : les fichiers de chaque paquet, dans l'ordre de mindcare-telechargement (moteur, puis modèle)
function Get-Entree([string]$Nom, [switch]$Abime) {
    $f = Join-Path $depot $Nom
    $h = (Get-FileHash -LiteralPath $f -Algorithm SHA256).Hash
    if ($Abime) { $h = ('0' * 64) }
    [ordered]@{ url = "http://127.0.0.1:$port/$Nom"; sha256 = $h; taille = (Get-Item -LiteralPath $f).Length }
}
function Write-Depot([string]$Chemin, [switch]$DicteeAbimee) {
    $j = [ordered]@{
        assistant = @((Get-Entree 'llama-b11541-bin-win-vulkan-x64.zip'), (Get-Entree 'mindcare-0.6b-v3-q8_0.gguf'))
        dictee = @((Get-Entree 'whisper-bin-x64.zip'), (Get-Entree 'ggml-small-q5_1.bin' -Abime:$DicteeAbimee))
    }
    [IO.File]::WriteAllText($Chemin, ($j | ConvertTo-Json -Depth 5), $utf8)
}
$depotBon = Join-Path $racineEssai 'depot.json'; Write-Depot $depotBon
$depotAbime = Join-Path $racineEssai 'depot-abime.json'; Write-Depot $depotAbime -DicteeAbimee

# Les phrases (celles des juges de mindcare-assistant) et les réponses du faux llama-server
$phraseSure = "mon pc portable s'éteint d'un coup, et les lumières de la maison vacillent"
$phraseHesitante = "mon pc s'éteint d'un coup"
$phraseIllisible = "mon pc fait un bruit illisible"
$regles = @(
    [ordered]@{ contient = 'lumières'; panne = 'seteint'; reponses = [ordered]@{ type = 0; maison = 0; surbatterie = 1 }; confiance = 0.97 },
    [ordered]@{ contient = 'illisible'; brut = 'pas du json'; lenteur = 300 },
    [ordered]@{ contient = "éteint d'un coup"; panne = 'seteint'; reponses = [ordered]@{ maison = 0 }; confiance = 0.6 }
)
$phraseDictee = "Mon PC redémarre tout seul quand je joue."

# ------------------------------------------------------------------------------------------------ l'appli d'essai
$script:p = $null; $script:theme = ''; $script:dossierTheme = ''; $script:donnees = $null; $script:racine = $null
$nomEssai = "aides-$PID"

function Preparer([string]$Theme, [string]$Nom) {
    $script:racine = Join-Path $racineEssai "$Theme-$Nom"
    if (Test-Path -LiteralPath $script:racine) { Remove-Item -LiteralPath $script:racine -Recurse -Force }
    $script:programmes = Join-Path $script:racine 'Programmes\Mindcare'
    $script:donnees = Join-Path $script:racine 'Donnees\Mindcare'
    $script:local = Join-Path $script:racine 'Local'
    New-Item -ItemType Directory -Force -Path $script:programmes, $script:donnees, (Join-Path $script:local 'Mindset') | Out-Null
    $script:exeEssai = Join-Path $script:programmes 'Mindset.exe'
    Copy-Item -LiteralPath $Exe -Destination $script:exeEssai
    Copy-Item -LiteralPath (Join-Path $depotGit 'Mindcare\fichiers\enquetes.json') -Destination (Join-Path $script:programmes 'enquetes.json')
    $j = [ordered]@{ theme = $Theme; micro = ''; casque = ''; applis = [ordered]@{ mindcare = $script:programmes }; modifie = (Get-Date -Format s) }
    [IO.File]::WriteAllText((Join-Path $script:local 'Mindset\suite.json'), ($j | ConvertTo-Json -Depth 4), $utf8)
}

function Get-Journal {
    $journal = Join-Path $script:donnees 'journal.txt'
    if (-not (Test-Path -LiteralPath $journal)) { return @() }
    $f = New-Object IO.FileStream($journal, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    try { $l = (New-Object IO.StreamReader($f, [Text.Encoding]::UTF8)).ReadToEnd() } finally { $f.Dispose() }
    @($l -split "`r`n" | Where-Object { $_ })
}
function Compter([string]$Motif) { @(Get-Journal | Where-Object { $_ -like "*$Motif*" }).Count }
function Attendre([scriptblock]$Condition, [int]$Ms) {
    $t = [Diagnostics.Stopwatch]::StartNew()
    while ($t.ElapsedMilliseconds -lt $Ms) { if (& $Condition) { return $true }; Start-Sleep -Milliseconds 100 }
    [bool](& $Condition)
}
function Attendre-Journal([string]$Motif, [int]$Ms, [string]$Quoi) { Verifier (Attendre { (Compter $Motif) -gt 0 } $Ms) "$Quoi (journal : « $Motif »)" }

function Lancer([string]$Quoi, [string]$Depot) {
    $psi = New-Object Diagnostics.ProcessStartInfo $script:exeEssai
    $psi.Arguments = '-Demarrage'
    $psi.UseShellExecute = $false
    $psi.WorkingDirectory = $script:programmes
    $psi.EnvironmentVariables['MINDCARE_DONNEES'] = $script:donnees
    $psi.EnvironmentVariables['MINDCARE_ESSAI_LOCAL'] = $script:local
    $psi.EnvironmentVariables['MINDCARE_ESSAI_NOM'] = $nomEssai
    $psi.EnvironmentVariables['MINDCARE_ESSAI_DEPOT'] = $Depot
    $script:p = [Diagnostics.Process]::Start($psi)
    Write-Host "$Quoi : processus $($script:p.Id)"
    $pid2 = $script:p.Id
    $vu = Attendre { @([EssaiAides]::Fenetres($pid2) | Where-Object { [EssaiAides]::Classe($_) -ceq 'Tauri Window' -and [EssaiAides]::Titre($_) -ceq 'Mindcare' }).Count -ge 1 } 20000
    Verifier $vu "$Quoi : fenêtre « Mindcare »"
    $f = @([EssaiAides]::Fenetres($pid2) | Where-Object { [EssaiAides]::Classe($_) -ceq 'Tauri Window' -and [EssaiAides]::Titre($_) -ceq 'Mindcare' })[0]
    Verifier (Attendre { (Compter 'Écran prêt') -gt 0 } 20000) "$Quoi : le kit est prêt"
    $s = [Threading.EventWaitHandle]::OpenExisting("Local\MindcareEssai-$nomEssai-Afficher"); [void]$s.Set(); $s.Dispose()
    Verifier (Attendre { [EssaiAides]::IsWindowVisible($f) } 5000) "$Quoi : montrée par le signal d'essai"
    $script:cible = [IntPtr]::Zero
    [void](Attendre { $script:cible = @([EssaiAides]::Enfants($f, 'Chrome_RenderWidgetHostHWND'))[0]; $null -ne $script:cible -and $script:cible -ne [IntPtr]::Zero } 5000)
}

# Les processus de l'essai encore là (Mindcare, sa copie de llama-server, whisper), par leur chemin
function Get-Restants { @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($racineEssai, [StringComparison]::OrdinalIgnoreCase) }) }

function Quitter {
    if (-not $script:p -or $script:p.HasExited) { return }
    [IO.File]::WriteAllText((Join-Path $script:donnees 'quitter.demande'), "$PID")
    if (-not $script:p.WaitForExit(15000)) { try { $script:p.Kill() } catch {}; Echec 'l''appli ne quitte pas sur quitter.demande' }
    # (llama-server et whisper s'arrêtent avec elle : Stop-Contenu, puis les Job Objects)
    Verifier (Attendre { (Get-Restants).Count -eq 0 } 8000) "$($script:theme) : plus rien de l'essai ne tourne après la fermeture ($(@(Get-Restants | ForEach-Object { $_.Name }) -join ', '))"
    $l = @("--- Processus $($script:p.Id) ---") + @(Get-Journal)
    [IO.File]::AppendAllText((Join-Path $script:dossierTheme 'journal.txt'), (($l -join "`r`n") + "`r`n"), $utf8)
}

function Envoyer([object[]]$Messages, [int]$Pause = 700) {
    $lignes = @($Messages | ForEach-Object { $_ | ConvertTo-Json -Compress -Depth 5 })
    $tmp = Join-Path $script:donnees 'actions.tmp'
    $dest = Join-Path $script:donnees 'actions.demande'
    [IO.File]::WriteAllText($tmp, ($lignes -join "`n"), $utf8)
    Move-Item -LiteralPath $tmp -Destination $dest -Force
    if (-not (Attendre { -not (Test-Path -LiteralPath $dest) } 5000)) { Echec "actions non lues : $($lignes -join ' ; ')" }
    Start-Sleep -Milliseconds $Pause
}
function Action([string]$Id, $Valeur = $null, [string]$Ecran = 'raconte') { [ordered]@{ type = 'action'; id = $Id; valeur = $Valeur; ecran = $Ecran } }
function VaRaconte { Envoyer @((Action 'accueil' $null 'enquete'), (Action 'raconte' $null 'accueil')) 900 }

$script:n = 0
function Capturer([string]$Nom, [string]$Ecran, [string]$AVoir, [int]$Attente = 900) {
    $script:n++
    $fichier = '{0:D2}-{1}.png' -f $script:n, $Nom
    $c = Join-Path $script:donnees 'capture.png'
    if (Test-Path -LiteralPath $c) { Remove-Item -LiteralPath $c -Force }
    Start-Sleep -Milliseconds $Attente
    [IO.File]::WriteAllText((Join-Path $script:donnees 'capture.demande'), '1')
    $ok = Attendre { Test-Path -LiteralPath $c } 5000
    Verifier $ok "capture $($script:theme)\$fichier"
    if ($ok) {
        Start-Sleep -Milliseconds 300
        Copy-Item -LiteralPath $c -Destination (Join-Path $script:dossierTheme $fichier) -Force
        $index.Add("$($script:theme)\$fichier | $Ecran | $AVoir")
    }
}

function Parcours-Theme {
    $t = $script:theme
    $script:n = 0
    # --- 1. Tout à installer, depuis le faux serveur
    Preparer $t 'installer'
    Lancer "Thème $t" $depotBon
    VaRaconte
    Verifier ((Compter 'Assistant lancé') -eq 0) "$t : l'assistant n'est pas lancé à l'ouverture"
    Capturer 'raconte-a-installer' 'Raconte ton problème, rien d''installé' 'Le bouton Parler en haut à droite ; « La dictée » : installe la dictée, environ 4 Mo (faux fichiers), bouton Installer ; « L''assistant » : installe son assistant, environ 7 Mo, bouton Installer'
    Envoyer @((Action 'installer-ia'), (Action 'installer-dictee')) 200
    Capturer 'installation-en-cours' 'Les deux installations en cours' '« Installation de l''assistant : N % (moteur ou cerveau de l''assistant). Tu peux continuer... » avec sa barre ; la dictée attend ou avance avec sa barre' 1200
    Attendre-Journal 'Assistant installé' 60000 "$t : l'assistant s'installe"
    Attendre-Journal 'Dictée installée' 60000 "$t : la dictée s'installe"
    Verifier ((Compter 'Installation de l''assistant (') -eq 1 -and (Compter 'Installation de la dictée (') -eq 1) "$t : un seul téléchargement de chacun"
    Start-Sleep -Milliseconds 800
    Capturer 'aides-pretes' 'L''assistant et la dictée installés' '« La dictée est prête : appuie sur « Parler »... », « L''assistant est prêt : raconte ton problème... », l''indice du champ « ... puis appuie sur Entrée »'
    [IO.File]::WriteAllText((Join-Path $script:donnees 'ia\faux-reponses.json'), (ConvertTo-Json @($regles) -Depth 5), $utf8)
    [IO.File]::WriteAllText((Join-Path $script:donnees 'dictee\consigne.txt'), "lent $phraseDictee", $utf8)

    # --- 2. Une phrase hésitante : « Pour être sûr »
    Envoyer @(Action 'recherche' $phraseHesitante) 300
    Capturer 'assistant-se-prepare' 'On écrit : l''assistant se prépare' '« L''assistant se prépare, quelques secondes... », les pannes trouvées, « Demander à l''assistant »' 100
    Attendre-Journal 'Assistant lancé (port' 10000 "$t : l'assistant se lance dès qu'on écrit"
    Start-Sleep -Milliseconds 2500
    Capturer 'phrase-ecrite' 'Une phrase écrite, l''assistant prêt' 'La phrase dans le champ, « L''assistant est prêt... », les pannes trouvées (le bouton « Demander à l''assistant » est sous les 8 pannes, comme dans la maquette)'
    [EssaiAides]::Molette($script:cible, 560, 500, -12); Start-Sleep -Milliseconds 600
    Capturer 'phrase-ecrite-bas' 'Une phrase écrite, défilé vers le bas' 'Le bouton « Demander à l''assistant » sous les pannes trouvées'
    [EssaiAides]::Molette($script:cible, 560, 500, 12); Start-Sleep -Milliseconds 400
    Envoyer @(Action 'entree') 200
    Capturer 'assistant-lit' 'L''assistant lit la phrase' '« Je lis ta phrase... » dans la ligne de l''assistant, la pastille « Je lis ta phrase... », plus de bouton Demander' 150
    Attendre-Journal 'Pour être sûr : ' 15000 "$t : la phrase hésitante donne « Pour être sûr »"
    Capturer 'pour-etre-sur' 'Pour être sûr' 'À gauche « Ce que tu as écrit » et la phrase ; à droite la question, 2 ou 3 cartes (le titre et la phrase de chaque panne), « Aucune de ces phrases »'
    Envoyer @(Action 'idee' '0' 'pourEtreSur') 900
    Verifier ((Compter 'Pour être sûr : seteint') -ge 1 -and (Compter 'Enquête : seteint') -ge 1) "$t : une réponse de « Pour être sûr » lance son enquête"
    Capturer 'enquete-apres-pour-etre-sur' 'L''enquête choisie dans Pour être sûr' 'L''enquête « Il s''éteint d''un coup » : sa question de sécurité d''abord (celle sur la maison, déjà répondue, ne sera pas posée)'

    # --- 3. Une phrase sûre : l'enquête tout de suite, avec les réponses déjà données
    VaRaconte
    Envoyer @(Action 'recherche' $phraseSure) 400
    Envoyer @(Action 'entree') 200
    Attendre-Journal '(sûr à 97 %)' 15000 "$t : la phrase sûre est lue"
    Attendre-Journal 'Enquête : seteint (déjà su : type, maison)' 5000 "$t : l'enquête commence avec les réponses déjà données"
    Verifier ((Compter 'lumières') -eq 0) "$t : la phrase n'est jamais au journal"
    Capturer 'enquete-directe' 'L''enquête lancée par une phrase sûre' 'Directement l''enquête « Il s''éteint d''un coup » (sa question de sécurité d''abord ; type de PC et maison, déjà répondus, ne seront pas posés)'

    # --- 4. Une réponse illisible : le message, la recherche par mots reste
    VaRaconte
    Envoyer @(Action 'recherche' $phraseIllisible) 400
    Envoyer @(Action 'entree') 200
    Attendre-Journal 'Assistant : réponse illisible' 15000 "$t : la réponse illisible est notée"
    Capturer 'assistant-echec' 'L''assistant n''a pas pu lire la phrase' '« L''assistant n''a pas réussi à lire ta phrase cette fois. Regarde les pannes ci-dessous... », les pannes trouvées en dessous'

    # --- 5. La dictée (faux micro : une voix de 0,6 à 2,6 s, puis le silence)
    Envoyer @(Action 'recherche' '') 300
    Envoyer @(Action 'parler') 100
    Capturer 'dictee-ecoute' 'La dictée écoute' 'Le bouton « Arrêter d''écouter » en haut à droite ; « Je t''écoute. Dis ce qui ne va pas : j''écris ta phrase dès que tu te tais. »' 500
    [void](Attendre { (Compter 'Dictée : écoute') -gt 0 } 3000)
    Start-Sleep -Milliseconds 3700
    Capturer 'dictee-ecrit' 'La dictée écrit' '« J''écris ce que tu as dit... », le bouton redevenu « Parler »' 100
    Attendre-Journal 'caractères' 15000 "$t : whisper a écrit la phrase"
    Start-Sleep -Milliseconds 600
    Capturer 'dictee-phrase' 'La phrase dictée dans la recherche' "« $phraseDictee » dans le champ (à corriger) ; « Voilà ce que j'ai compris. Corrige si besoin, puis appuie sur Entrée. » ; les pannes trouvées"
    Verifier ((Compter 'redémarre tout seul') -eq 0) "$t : la phrase dictée n'est pas au journal"
    $sons = @(Get-ChildItem -LiteralPath (Join-Path $script:local 'Temp') -Filter '*dictee*' -ErrorAction SilentlyContinue)
    Verifier ($sons.Count -eq 0) "$t : le son est effacé dès que la phrase est écrite"
    Quitter

    # --- 6. Un ancien assistant (à mettre à jour) et une dictée coupée (à réparer, puis en erreur)
    Preparer $t 'etats'
    $ia = Join-Path $script:donnees 'ia'; $di = Join-Path $script:donnees 'dictee'
    New-Item -ItemType Directory -Force -Path $ia, $di | Out-Null
    Copy-Item -LiteralPath $FauxLlama -Destination (Join-Path $ia 'llama-server.exe')
    foreach ($d in 'llama.dll', 'ggml.dll') { [IO.File]::WriteAllText((Join-Path $ia $d), 'faux') }
    [IO.File]::WriteAllText((Join-Path $ia 'mindcare-0.6b-v2-q8_0.gguf'), 'ancien modèle')
    Copy-Item -LiteralPath $FauxWhisper -Destination (Join-Path $di 'whisper-cli.exe')
    Lancer "Thème $t, états" $depotAbime
    VaRaconte
    Verifier ((Compter 'Assistant lancé') -eq 0) "$t : l'ancien assistant n'est pas lancé à l'ouverture"
    Attendre-Journal 'Assistant : une nouvelle version est proposée' 3000 "$t : la mise à jour est proposée"
    Capturer 'a-mettre-a-jour-et-a-reparer' 'Un ancien assistant, une dictée coupée' '« La dictée » : installe la dictée..., bouton Réparer ; « L''assistant » : « Une nouvelle version de l''assistant comprend mieux tes phrases : mets-la à jour (environ 6 Mo...) », bouton Mettre à jour'
    Envoyer @(Action 'installer-dictee') 300
    Attendre-Journal 'Dictée non installée' 60000 "$t : le fichier abîmé de la dictée est refusé"
    Start-Sleep -Milliseconds 600
    Capturer 'dictee-erreur' 'La dictée n''a pas pu s''installer' '« La dictée n''a pas pu s''installer : le fichier « français de la dictée » arrive abîmé... », bouton Réessayer'
    Envoyer @(Action 'recherche' 'mon pc fait du bruit') 300
    Attendre-Journal 'Assistant lancé (port' 10000 "$t : l'ancien assistant marche en attendant"
    Envoyer @(Action 'installer-ia') 300
    Attendre-Journal 'Ancien modèle effacé : mindcare-0.6b-v2-q8_0.gguf' 60000 "$t : l'ancien modèle est effacé après la mise à jour"
    $j = @(Get-Journal) -join "`n"
    Verifier ($j.IndexOf('Assistant installé') -ge 0 -and $j.IndexOf('Assistant installé') -lt $j.IndexOf('Ancien modèle effacé')) "$t : llama-server arrêté avant d'effacer l'ancien modèle"
    Verifier (-not (Test-Path -LiteralPath (Join-Path $ia 'mindcare-0.6b-v2-q8_0.gguf'))) "$t : l'ancien modèle n'est plus sur le disque"
    Quitter
}

try {
    foreach ($t in $Themes) {
        $script:theme = $t
        $script:dossierTheme = Join-Path $Resultats $t
        New-Item -ItemType Directory -Force -Path $script:dossierTheme | Out-Null
        Get-ChildItem -LiteralPath $script:dossierTheme -File | Remove-Item -Force
        try { Parcours-Theme }
        catch { Echec "thème $t : erreur du script : $_ ($($_.InvocationInfo.PositionMessage))" }
        finally { Quitter }
    }
}
finally {
    if ($index.Count) {
        $entete = @('Captures de l''assistant et de la dictée de Mindcare en Rust (étape 6.4), avec des faux (serveur, llama-server, whisper, micro), fenêtre de 1100 x 760 sur un bureau Windows à part.', 'fichier | écran | ce qu''on doit y voir', '')
        [IO.File]::WriteAllText((Join-Path $Resultats 'index.txt'), (($entete + @($index | Sort-Object)) -join "`r`n") + "`r`n", (New-Object Text.UTF8Encoding($true)))
    }
    $restants = @(Get-Restants)
    $restants += @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" | Where-Object { $_.CommandLine -like "*$racineEssai*" })
    Verifier ($restants.Count -eq 0) "plus aucun processus de l'essai"
    foreach ($r in $restants) { try { Stop-Process -Id $r.ProcessId -Force -ErrorAction Stop } catch {} }
    try { $ecoute.Stop() } catch {}
    try { [void]$serveur.BeginStop($null, $null) } catch {}
    Start-Sleep -Milliseconds 500
    try { Remove-Item -LiteralPath $racineEssai -Recurse -Force -ErrorAction Stop } catch { Write-Output "Dossier d'essai pas encore effacé : $racineEssai" }
}
if ($echecs.Count) { Write-Output "$($echecs.Count) échec(s)."; exit 1 }
Write-Output 'Tout est bon.'
exit 0
