# Teste l'assistant de Mindcare (modules\ia.ps1 et sa partie dans contenu.ps1) sans la vraie IA : un faux llama-server
# (un petit programme qui attend, compilé ici), des faux fichiers servis par un petit serveur sur ce PC (127.0.0.1),
# tout dans _test\assistant. Rien n'est téléchargé, rien n'est modifié sur le PC, aucune fenêtre ne s'ouvre.
#
#   powershell -NoProfile -STA -ExecutionPolicy Bypass -File .\outils\tester-assistant.ps1
#   (-Modules <dossier> : essayer d'autres modules, par exemple une ancienne version, pour voir les tests échouer)
#
# Code de sortie : 0 si tout est bon, 1 sinon.

param([string]$Modules)
$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
if (-not $Modules) { $Modules = Join-Path $racine 'Mindcare\fichiers\modules' }
$test = Join-Path $racine '_test\assistant'
Remove-Item -LiteralPath $test -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $test | Out-Null
$script:Ok = $true
function Write-Resultat([string]$Nom, [bool]$Bon, [string]$Detail = '') {
    if ($Bon) { Write-Host "OK     $Nom" -ForegroundColor Green } else { Write-Host "ÉCHEC  $Nom $Detail" -ForegroundColor Red; $script:Ok = $false }
}
function Test-Essai([string]$Nom, [scriptblock]$Essai) {
    try { $r = & $Essai; Write-Resultat $Nom ([bool]$r.Bon) "($($r.Vu))" } catch { Write-Resultat $Nom $false "(erreur : $($_.Exception.Message))" }
}

Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase, System.IO.Compression, System.IO.Compression.FileSystem

# Ce que Mindcare fournit d'habitude à ia.ps1 et contenu.ps1 (journal en mémoire, interface réduite au nécessaire)
$EnTest = $true
$DataDir = Join-Path $test 'donnees'
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
$script:Journal = New-Object Collections.Generic.List[string]
function Write-Log([string]$Texte) { $script:Journal.Add($Texte) }
function Get-Brush([string]$Hex) { [System.Windows.Media.BrushConverter]::new().ConvertFromString($Hex) }
function New-Text([string]$Texte, [double]$Taille = 14, [string]$Couleur = '#EEEBF7', [string]$Graisse = 'Normal') { $t = New-Object System.Windows.Controls.TextBlock; $t.Text = $Texte; $t }
function New-Button([string]$Texte, [string]$Style) { $b = New-Object System.Windows.Controls.Button; $b.Content = $Texte; $b }
function Set-BulleMascotte([string]$Texte) { $script:Bulle = $Texte }
function New-ChargementTheme([double]$Taille) { $null }
function Stop-ChargementTheme {}
function Protect-Action([scriptblock]$Action) { $Action }
function Get-Panne($Donnees, $Id) { $null }
foreach ($m in 'ia', 'contenu') { . (Join-Path $Modules "$m.ps1") }
try { Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $racine 'Mindcare\fichiers\modules\natif.cs'), [Text.Encoding]::UTF8)) } catch {}   # (le faux assistant s'arrête avec ce test)
$script:Donnees = [pscustomobject]@{ Pannes = @() }
$IaFichiersVrais = $IaFichiers

# Les textes affichés dans la zone de l'assistant
function Get-TexteZone { @($ui.ZoneIa.Children | ForEach-Object { $_.Child } | ForEach-Object { if ($_ -is [System.Windows.Controls.TextBlock]) { $_.Text } else { @($_.Children | Where-Object { $_ -is [System.Windows.Controls.TextBlock] } | ForEach-Object { $_.Text }) } }) -join ' | ' }
# Fait tourner les minuteries de la fenêtre jusqu'à ce que la condition soit vraie (ou le délai passé)
function Wait-Dispatcher([scriptblock]$Fini, [int]$Secondes = 10) {
    $limite = (Get-Date).AddSeconds($Secondes)
    while (-not (& $Fini) -and (Get-Date) -lt $limite) {
        $frame = New-Object System.Windows.Threading.DispatcherFrame
        [void][System.Windows.Threading.Dispatcher]::CurrentDispatcher.BeginInvoke([System.Windows.Threading.DispatcherPriority]::Background, [Action]{ $frame.Continue = $false })
        [System.Windows.Threading.Dispatcher]::PushFrame($frame)
        Start-Sleep -Milliseconds 50
    }
}
# Attend la fin d'une installation (en arrière-plan)
function Wait-Installation([int]$Secondes = 30) {
    $limite = (Get-Date).AddSeconds($Secondes)
    while ($script:IaDl -and -not $script:IaDl.Fini -and (Get-Date) -lt $limite) { Start-Sleep -Milliseconds 100 }
    if ($script:IaDl -and -not $script:IaDl.Fini) { try { $script:IaDl.PS.Stop() } catch {}; return $false }
    $true
}

# Un faux assistant installé dans un dossier : llama-server (faux, qui attend 60 s), ses .dll et un modèle
$csc = Join-Path $env:windir 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
function New-FauxAssistant([string]$Dossier, [switch]$PasUnProgramme, [switch]$SansDll) {
    New-Item -ItemType Directory -Force -Path $Dossier | Out-Null
    $exe = Join-Path $Dossier 'llama-server.exe'
    if ($PasUnProgramme) { Set-Content -LiteralPath $exe -Value 'faux' }
    else {
        $src = Join-Path $Dossier 'faux-serveur.cs'
        [IO.File]::WriteAllText($src, 'static class F { static void Main() { System.Threading.Thread.Sleep(60000); } }')
        $null = & $csc /nologo /target:winexe "/out:$exe" $src
    }
    if (-not $SansDll) { foreach ($d in 'llama.dll', 'ggml.dll') { Set-Content -LiteralPath (Join-Path $Dossier $d) -Value 'faux' } }
    Set-Content -LiteralPath (Join-Path $Dossier 'mindcare-test.gguf') -Value 'faux'
    $script:IaServeur = $exe; $script:IaModele = Join-Path $Dossier 'mindcare-test.gguf'
}
$mesProcessus = New-Object Collections.Generic.List[int]

# Un petit serveur sur ce PC (127.0.0.1) qui donne les fichiers d'un dossier, comme GitHub donne ceux de l'assistant
$depot = Join-Path $test 'depot'
New-Item -ItemType Directory -Force -Path $depot | Out-Null
$ecoute = New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback, 0)
$ecoute.Start()
$portDepot = $ecoute.LocalEndpoint.Port
$serveur = [PowerShell]::Create()
[void]$serveur.AddScript({
    param($Ecoute, $Dossier)
    while ($true) {
        $c = $Ecoute.AcceptTcpClient()
        try {
            $s = $c.GetStream(); $lu = New-Object IO.StreamReader($s)
            $ligne = $lu.ReadLine(); while ($lu.ReadLine()) {}
            $octets = [IO.File]::ReadAllBytes((Join-Path $Dossier ([uri]::UnescapeDataString(($ligne -split ' ')[1].TrimStart('/')))))
            $tete = [Text.Encoding]::ASCII.GetBytes("HTTP/1.1 200 OK`r`nContent-Length: $($octets.Length)`r`nConnection: close`r`n`r`n")
            $s.Write($tete, 0, $tete.Length); $s.Write($octets, 0, $octets.Length); $s.Flush()
        } catch {} finally { $c.Close() }
    }
}).AddArgument($ecoute).AddArgument($depot)
$serveurTache = $serveur.BeginInvoke()
function New-FichierDepot([string]$Nom, [byte[]]$Octets, [string]$Garder, [string]$Extraire) {
    [IO.File]::WriteAllBytes((Join-Path $depot $Nom), $Octets)
    $h = (Get-FileHash -LiteralPath (Join-Path $depot $Nom) -Algorithm SHA256).Hash
    $f = @{ Nom = "fichier $Nom"; Url = "http://127.0.0.1:$portDepot/$Nom"; Sha256 = $h; Taille = $Octets.Length; Garder = $Garder }
    if ($Extraire) { $f.Extraire = $Extraire }
    $f
}
$script:IaPause = 0   # (pas d'attente entre deux essais de téléchargement : le test va vite)

try {
    # -----------------------------------------------------------------------------------------------------------------
    # Bug 6 : l'assistant n'est plus chargé (carte graphique) à l'ouverture de Mindcare, seulement quand on s'en sert,
    # et il est arrêté après quelques minutes sans question
    # -----------------------------------------------------------------------------------------------------------------
    Test-Essai "assistant : pas lancé à l'ouverture de Mindcare (Initialize-Contenu)" {
        $ast = [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $Modules 'contenu.ps1'), [ref]$null, [ref]$null)
        $f = $ast.Find({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Initialize-Contenu' }, $true)
        $appels = @($f.FindAll({ param($n) $n -is [System.Management.Automation.Language.CommandAst] -and $n.GetCommandName() -eq 'Start-Ia' }, $true))
        @{ Bon = ($f -and -not $appels.Count); Vu = "$($appels.Count) appel(s) à Start-Ia" }
    }
    New-FauxAssistant (Join-Path $test 'ia-repos')
    Test-Essai "assistant : lancé à la première question s'il ne tourne pas" {
        $tache = Start-IaQuestion 'mon pc fait du bruit' @('bruit')
        if ($script:Ia.Process) { $mesProcessus.Add($script:Ia.Process.Id) }
        if ($tache -and $tache.PS) { [void]$tache.PS.BeginStop($null, $null) }
        @{ Bon = ($tache -and (Test-IaLancee)); Vu = "tâche : $([bool]$tache), lancé : $(Test-IaLancee)" }
    }
    Test-Essai "assistant : gardé tant qu'il sert, arrêté après $IaRepos minutes sans question" {
        $id = if ($script:Ia.Process) { $script:Ia.Process.Id } else { 0 }
        $garde = -not (Stop-IaSiInutile (Get-Date).AddMinutes(1)) -and (Test-IaLancee)
        $arrete = (Stop-IaSiInutile (Get-Date).AddMinutes($IaRepos + 1)) -and -not (Test-IaLancee)
        @{ Bon = ($id -and $garde -and $arrete -and -not (Get-Process -Id $id -ErrorAction SilentlyContinue)); Vu = "gardé à 1 min : $garde, arrêté après : $arrete" }
    }
    Test-Essai "assistant : relancé tout seul à la question suivante" {
        $tache = Start-IaQuestion 'mon pc fait du bruit' @('bruit')
        if ($script:Ia.Process) { $mesProcessus.Add($script:Ia.Process.Id) }
        if ($tache -and $tache.PS) { [void]$tache.PS.BeginStop($null, $null) }
        $bon = $tache -and (Test-IaLancee)
        Stop-Ia
        @{ Bon = $bon; Vu = "lancé : $bon" }
    }

    # -----------------------------------------------------------------------------------------------------------------
    # Bug 4 : quand l'assistant échoue, l'utilisateur le voit (et le journal dit pourquoi)
    # -----------------------------------------------------------------------------------------------------------------
    Test-Essai "assistant : la vraie cause d'un échec est gardée (llama-server injoignable)" {
        $ps = [PowerShell]::Create(); [void]$ps.AddScript({ throw 'Impossible de se connecter au serveur distant' })
        $tache = @{ PS = $ps; Res = $ps.BeginInvoke(); Debut = Get-Date; Attente = 40 }
        $limite = (Get-Date).AddSeconds(10); while (-not $tache.Res.IsCompleted -and (Get-Date) -lt $limite) { Start-Sleep -Milliseconds 50 }
        $r = Get-IaReponse $tache
        @{ Bon = ($r -and $r.Erreur -match 'connecter'); Vu = "erreur : « $($r.Erreur) »" }
    }
    $ui = @{ ZoneIa = New-Object System.Windows.Controls.StackPanel; Recherche = New-Object System.Windows.Controls.TextBox }
    $ui.Recherche.Text = "mon pc s'éteint quand je joue"
    Test-Essai "assistant : réponse illisible, l'utilisateur voit un message (pas un bandeau vide)" {
        $env:MINDCARE_TEST_IA = 'pas du json'
        try {
            $script:IaCarte = $null; $script:IaAttente = $null; $ui.ZoneIa.Children.Clear()
            Request-Ia
            Wait-Dispatcher { -not $script:IaAttente } 10
        } finally { Remove-Item env:MINDCARE_TEST_IA -ErrorAction SilentlyContinue }
        $vu = Get-TexteZone
        @{ Bon = (-not $script:IaAttente -and $vu -match "n'a pas réussi à lire ta phrase"); Vu = "zone : « $vu »" }
    }
    New-FauxAssistant (Join-Path $test 'ia-casse') -PasUnProgramme
    Test-Essai "assistant : installé mais il ne démarre pas, l'utilisateur voit un message" {
        $script:IaCarte = $null; $script:IaAttente = $null; $ui.ZoneIa.Children.Clear(); $script:Ia.Process = $null
        Request-Ia
        $vu = Get-TexteZone
        @{ Bon = ($vu -match "n'a pas réussi à lire ta phrase" -and ($script:Journal -join ' ') -match 'lancement impossible'); Vu = "zone : « $vu »" }
    }

    # -----------------------------------------------------------------------------------------------------------------
    # Bug 5 : disque plein ou fichier bloqué ne sont pas appelés « connexion instable »
    # -----------------------------------------------------------------------------------------------------------------
    $IaDir = Join-Path $DataDir 'ia'
    $petit = New-FichierDepot 'modele.gguf' ([Text.Encoding]::ASCII.GetBytes('faux modèle ' * 1000)) (Join-Path $IaDir 'modele.gguf')
    Test-Essai "installation : pas assez de place, dit avant de télécharger" {
        $IaFichiers = @($petit); $script:IaDl = $null
        $placeVraie = ${function:Get-PlaceLibre}
        ${function:Get-PlaceLibre} = { param($Dossier) 100MB }
        try { Start-IaInstall } finally { ${function:Get-PlaceLibre} = $placeVraie }
        $fini = Wait-Installation 20
        @{ Bon = ($fini -and $script:IaDl.Erreur -match 'place sur le disque' -and -not (Test-Path -LiteralPath $petit.Garder)); Vu = "erreur : « $($script:IaDl.Erreur) »" }
    }
    Test-Essai "installation : fichier bloqué (antivirus), pas « connexion instable »" {
        Remove-Item -LiteralPath $IaDir -Recurse -Force -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force -Path $IaDir | Out-Null
        $tmp = Join-Path $IaDir 'modele.gguf.telechargement'
        [IO.File]::WriteAllBytes($tmp, [byte[]]@())
        $verrou = [IO.File]::Open($tmp, 'Open', 'Read', 'None')
        try { $IaFichiers = @($petit); $script:IaDl = $null; Start-IaInstall; $fini = Wait-Installation 20 } finally { $verrou.Dispose() }
        @{ Bon = ($fini -and $script:IaDl.Erreur -match 'antivirus le bloque' -and $script:IaDl.Erreur -notmatch 'connexion'); Vu = "fini : $fini, erreur : « $($script:IaDl.Erreur) »" }
    }
    Test-Essai "installation : une vraie coupure reste « connexion instable »" {
        $l = New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback, 0); $l.Start(); $ferme = $l.LocalEndpoint.Port; $l.Stop()
        $coupe = $petit.Clone(); $coupe.Url = "http://127.0.0.1:$ferme/modele.gguf"
        $IaFichiers = @($coupe); $script:IaDl = $null; Start-IaInstall
        $fini = Wait-Installation 60
        @{ Bon = ($fini -and $script:IaDl.Erreur -match 'connexion instable'); Vu = "fini : $fini, erreur : « $($script:IaDl.Erreur) »" }
    }
    Test-Essai "installation : le fichier arrive quand tout va bien" {
        Remove-Item -LiteralPath $IaDir -Recurse -Force -ErrorAction SilentlyContinue
        $IaFichiers = @($petit); $script:IaDl = $null; Start-IaInstall
        $fini = Wait-Installation 20
        @{ Bon = ($fini -and -not $script:IaDl.Erreur -and (Test-Path -LiteralPath $petit.Garder)); Vu = "fini : $fini, erreur : « $($script:IaDl.Erreur) »" }
    }

    # -----------------------------------------------------------------------------------------------------------------
    # Bug 11 : une installation interrompue (llama-server.exe là, ses .dll pas toutes) se voit, et se répare
    # -----------------------------------------------------------------------------------------------------------------
    $IaFichiers = $IaFichiersVrais
    New-FauxAssistant (Join-Path $test 'ia-coupee') -SansDll
    Set-Content -LiteralPath (Join-Path $test 'ia-coupee\llama.dll') -Value 'faux'   # (ggml.dll manque)
    Test-Essai "installation coupée : le moteur compte comme manquant, Mindcare propose de réparer" {
        $IaServeur = Join-Path $test 'ia-coupee\llama-server.exe'
        $manque = @(Get-IaManquants | Where-Object { $_.Extraire })
        $script:IaDl = $null; $script:IaCarte = $null
        Update-BandeauIa
        $vu = Get-TexteZone
        @{ Bon = ($manque.Count -eq 1 -and -not (Test-IaInstallee) -and (Test-IaIncomplete) -and $vu -match "pas allée jusqu'au bout"); Vu = "manquants : $($manque.Count), installé : $(Test-IaInstallee), bandeau : « $vu »" }
    }
    # Un faux moteur (zip : llama-server.exe d'abord, puis ses .dll), extrait alors qu'une .dll est bloquée : Mindcare
    # « fermée » au milieu de l'extraction. llama-server.exe ne doit pas être là sans ses .dll.
    $zip = Join-Path $test 'moteur.zip'
    $z = [IO.Compression.ZipFile]::Open($zip, 'Create')
    foreach ($n in 'llama-server.exe', 'llama.dll', 'ggml.dll', 'autre.txt') { $e = $z.CreateEntry("llama/$n"); $w = New-Object IO.StreamWriter($e.Open()); $w.Write("faux $n"); $w.Dispose() }
    $z.Dispose()
    $IaDir = Join-Path $DataDir 'ia-moteur'
    New-Item -ItemType Directory -Force -Path $IaDir | Out-Null
    $IaServeur = Join-Path $IaDir 'llama-server.exe'
    $moteur = New-FichierDepot 'moteur.zip' ([IO.File]::ReadAllBytes($zip)) $IaServeur '^(llama-server\.exe|llama\.dll|ggml\.dll)$'
    Test-Essai "installation coupée pendant l'extraction : pas de llama-server.exe sans ses .dll" {
        $bloquee = Join-Path $IaDir 'ggml.dll'
        [IO.File]::WriteAllText($bloquee, 'ancienne')
        $verrou = [IO.File]::Open($bloquee, 'Open', 'Read', 'None')
        try { $IaFichiers = @($moteur); $script:IaDl = $null; Start-IaInstall; $fini = Wait-Installation 20 } finally { $verrou.Dispose() }
        @{ Bon = ($fini -and $script:IaDl.Erreur -and -not (Test-Path -LiteralPath $IaServeur) -and -not (Test-IaMoteur)); Vu = "fini : $fini, llama-server.exe là : $(Test-Path -LiteralPath $IaServeur), erreur : « $($script:IaDl.Erreur) »" }
    }
    Test-Essai "installation coupée : « Réparer » la finit (moteur complet, rien de moitié écrit)" {
        $IaFichiers = @($moteur); $script:IaDl = $null; Start-IaInstall
        $fini = Wait-Installation 20
        $restes = @(Get-ChildItem -LiteralPath $IaDir -Filter '*.extraction')
        @{ Bon = ($fini -and -not $script:IaDl.Erreur -and (Test-IaMoteur) -and -not $restes.Count -and -not (Test-Path -LiteralPath (Join-Path $IaDir 'autre.txt'))); Vu = "fini : $fini, erreur : « $($script:IaDl.Erreur) », moteur : $(Test-IaMoteur), restes : $($restes.Count)" }
    }
} finally {
    try { Stop-Ia } catch {}
    foreach ($id in $mesProcessus) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }   # (seulement les faux assistants de ce test)
    try { $ecoute.Stop() } catch {}
    try { [void]$serveur.BeginStop($null, $null) } catch {}
}

if ($script:Ok) { Write-Host "`nAssistant : tout est bon." -ForegroundColor Green; exit 0 }
Write-Host "`nAssistant : au moins un test a échoué." -ForegroundColor Red
exit 1
