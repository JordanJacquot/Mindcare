# Mindcare en Rust : la mise à jour depuis la version PowerShell ne perd rien. SUR GITHUB SEULEMENT (job « rust » de
# .github\workflows\tests.yml) : le test lance de vrais Mindcare et écrit suite.json dans %LOCALAPPDATA%\Mindset du
# compte (la machine de GitHub est jetée après) ; sur un PC, il refuse de tourner.
# (Repris de outils\tester-maj-rust.ps1 de Nevermind, qui fait la même chose depuis Nevermind 1.15.6.)
#
#   & '.\outils\tester-maj-rust.ps1' -Ancienne <Mindcare.zip de 0.15.3> -Nouvelle <dossier Mindcare en Rust construit>
#
# Comme chez un utilisateur dont Mindset met Mindcare à jour :
#   1. la version PowerShell (le zip publié de 0.15.3) est installée dans <tmp>\Mindset\Programmes\Mindcare, gérée par
#      Mindset (son dossier dans suite.json), avec des données inventées dans <tmp>\Mindset\Donnees\Mindcare
#      (reglages.json, une enquête gardée dans diagnostics.json, un fichier et une image qui ne sont pas à elle) ; elle
#      est lancée cachée (-Demarrage), écrit ses fichiers, puis quitte par quitter.demande, comme quand Mindset installe
#      une mise à jour ;
#   2. ce qu'elle laisse est relevé : chaque fichier, son empreinte, et pour un fichier JSON chaque champ (ou chaque
#      élément d'une liste) ;
#   3. son dossier de programmes est remplacé par Mindcare en Rust (ce que fait Install-Paquet), lancé caché à son tour,
#      puis arrêté par quitter.demande ;
#   4. rien n'est perdu : chaque fichier est encore là, lisible ; un fichier qui n'est pas du JSON est identique ; un JSON
#      garde chaque champ et chaque élément d'avant (des champs en plus sont permis), les valeurs inventées sont
#      exactement les mêmes, l'enquête gardée est toujours là ; le dossier des données n'a pas changé de place (rien de
#      créé dans %LOCALAPPDATA%\Mindset\Mindcare ni dans %LOCALAPPDATA%\Mindset\Mindcare (travail)).
# Ce qui change à chaque lancement n'est pas comparé : etat.json, journal.txt, journal-ancien.txt, les demandes (*.demande),
# un profil WebView2 ; d'une copie de secours (*.bak), seulement qu'elle est encore là (elle est refaite à chaque écriture).
# Code de sortie 1 si un cas échoue.
param(
    [Parameter(Mandatory = $true)][string]$Ancienne,
    [Parameter(Mandatory = $true)][string]$Nouvelle
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or -not $env:RUNNER_TEMP) {
    Write-Host 'Ce test ne tourne que sur GitHub : il lance de vrais Mindcare et écrit suite.json dans %LOCALAPPDATA%\Mindset du compte.' -ForegroundColor Red
    exit 1
}
Add-Type -AssemblyName System.IO.Compression.FileSystem
$base = Join-Path $env:RUNNER_TEMP 'maj-mindcare'
$prog = Join-Path $base 'Mindset\Programmes\Mindcare'
$donnees = Join-Path $base 'Mindset\Donnees\Mindcare'
$local = [Environment]::GetFolderPath('LocalApplicationData')
$suiteDir = Join-Path $local 'Mindset'
$suiteFichier = Join-Path $suiteDir 'suite.json'
$ailleurs = @((Join-Path $local 'Mindset\Mindcare'), (Join-Path $local 'Mindset\Mindcare (travail)'))
$utf8 = New-Object Text.UTF8Encoding($false)
$script:Echecs = @()
$script:Reussis = 0
function Test-Cas([string]$Nom, [scriptblock]$Condition) {
    $detail = ''
    try { $r = @(& $Condition) } catch { $r = @(); $detail = $_.Exception.Message }
    if ($r.Count -and $r[-1] -is [bool] -and $r[-1]) { $script:Reussis++; Write-Host "  OK     $Nom" -ForegroundColor Green }
    else { $script:Echecs += $Nom; Write-Host "  ÉCHEC  $Nom$(if ($detail) { " ($detail)" })" -ForegroundColor Red }
}
function Wait-Jusque([scriptblock]$Condition, [double]$Secondes) {
    $fin = (Get-Date).AddSeconds($Secondes)
    do { try { if (& $Condition) { return $true } } catch {}; Start-Sleep -Milliseconds 250 } while ((Get-Date) -lt $fin)
    $false
}
# Lance Mindset.exe de $prog caché (-Demarrage), attend son etat.json, le laisse travailler, puis l'arrête par
# quitter.demande. Rend $true s'il a tourné et quitté de lui-même.
function Invoke-Tour([string]$Nom) {
    Remove-Item -LiteralPath (Join-Path $donnees 'etat.json') -Force -ErrorAction SilentlyContinue
    $p = Start-Process -FilePath (Join-Path $prog 'Mindset.exe') -ArgumentList '-Demarrage' -WorkingDirectory $prog -PassThru
    $parti = Wait-Jusque { Test-Path -LiteralPath (Join-Path $donnees 'etat.json') } 180
    Test-Cas "$Nom : lancée cachée, elle écrit etat.json dans <tmp>\Mindset\Donnees\Mindcare (le même dossier de données)" { $parti }
    $null = Wait-Jusque { $p.HasExited } 20   # (le temps d'écrire ses fichiers au démarrage)
    [IO.File]::WriteAllText((Join-Path $donnees 'quitter.demande'), [string]$PID, $utf8)
    $quitte = Wait-Jusque { $p.HasExited } 30
    Test-Cas "$Nom : quitter.demande, elle quitte d'elle-même en moins de 30 s" { $quitte }
    if (-not $p.HasExited) { try { $p.Kill(); $null = $p.WaitForExit(5000) } catch {} }
    # (ses autres programmes lancés depuis son dossier, s'il en reste : fermés par leur chemin, comme le fait Mindset)
    Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith("$prog\", [StringComparison]::OrdinalIgnoreCase) } | ForEach-Object { try { $_.Kill() } catch {} }
    Remove-Item -LiteralPath (Join-Path $donnees 'quitter.demande') -Force -ErrorAction SilentlyContinue
    if (-not $parti) {
        $j = Join-Path $donnees 'journal.txt'
        if (Test-Path -LiteralPath $j) { Write-Host "  (fin de journal.txt :)"; Get-Content -LiteralPath $j -Tail 30 -Encoding UTF8 | ForEach-Object { "    $_" } }
    }
}
# Ce que contient le dossier des données : chemin relatif -> @{ Hash; Json (le texte de chaque champ ou élément) }
$volatils = '^(etat\.json|journal\.txt|journal-ancien\.txt|[^\\]+\.demande|[^\\]*webview[^\\]*\\.*)$'
function Get-Releve {
    $r = [ordered]@{}
    foreach ($f in @(Get-ChildItem -LiteralPath $donnees -Recurse -File -Force | Sort-Object FullName)) {
        $rel = $f.FullName.Substring($donnees.Length + 1)
        if ($rel -match $volatils) { continue }
        $e = @{ Hash = (Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash; Json = $null }
        if ($f.Extension -ieq '.json') {
            try {
                $o = [IO.File]::ReadAllText($f.FullName, [Text.Encoding]::UTF8).TrimStart([char]0xFEFF) | ConvertFrom-Json
                $champs = @{}
                if ($o -is [Management.Automation.PSCustomObject]) { foreach ($p in $o.PSObject.Properties) { $champs["champ:$($p.Name)"] = ConvertTo-Json $p.Value -Depth 20 -Compress } }
                elseif ($o -is [array]) { foreach ($x in $o) { $champs["élément:$(ConvertTo-Json $x -Depth 20 -Compress)"] = '' } }
                $e.Json = $champs
            } catch { $e.Json = 'illisible' }
        }
        $r[$rel] = $e
    }
    $r
}
# Ce qu'il y a dans les deux autres dossiers de données possibles (chaque fichier, sa taille et sa date), une ligne par
# fichier ; vide s'ils n'existent pas
function Get-Ailleurs {
    @(foreach ($a in $ailleurs) {
        if (Test-Path -LiteralPath $a) {
            "$a\"
            Get-ChildItem -LiteralPath $a -Recurse -File -Force | Sort-Object FullName | ForEach-Object { "$($_.FullName)|$($_.Length)|$($_.LastWriteTimeUtc.Ticks)" }
        }
    }) -join "`n"
}
# Les numéros des enquêtes gardées dans diagnostics.json (vide s'il est absent ou illisible)
function Get-IdsEnquetes {
    $f = Join-Path $donnees 'diagnostics.json'
    try { @(([IO.File]::ReadAllText($f, [Text.Encoding]::UTF8).TrimStart([char]0xFEFF) | ConvertFrom-Json).Diagnostics | ForEach-Object { [string]$_.Id }) } catch { @() }
}

$avantSuite = if (Test-Path -LiteralPath $suiteFichier) { [IO.File]::ReadAllBytes($suiteFichier) } else { $null }
try {
    if (Test-Path -LiteralPath $base) { Remove-Item -LiteralPath $base -Recurse -Force }
    # (ce qui est déjà dans ces deux dossiers, s'ils existent : le test vérifie à la fin que rien n'y a été ajouté ni changé)
    $avantAilleurs = Get-Ailleurs

    Write-Host ''; Write-Host '1. La version PowerShell, gérée par Mindset, avec des données inventées' -ForegroundColor Cyan
    $zipDir = Join-Path $base 'zip'
    [IO.Compression.ZipFile]::ExtractToDirectory((Resolve-Path -LiteralPath $Ancienne).Path, $zipDir)
    New-Item -ItemType Directory -Force -Path (Split-Path $prog -Parent) | Out-Null
    Move-Item -LiteralPath (Join-Path $zipDir 'Mindcare') -Destination $prog
    $versionAncienne = if ([IO.File]::ReadAllText((Join-Path $prog 'fichiers\Mindcare.ps1'), [Text.Encoding]::UTF8) -match "(?m)^\`$AppVersion = '([\d\.]+)'") { $Matches[1] } else { '' }
    Write-Host "  version PowerShell installée : $versionAncienne"
    New-Item -ItemType Directory -Force -Path $suiteDir, $donnees, (Join-Path $donnees 'images-essai') | Out-Null
    [IO.File]::WriteAllText($suiteFichier, (@{ applis = @{ mindcare = $prog } } | ConvertTo-Json -Compress), $utf8)
    # Les données inventées : des réglages qu'aucune version ne connaît (gardés tels quels par Get-Setting et Set-Setting),
    # une enquête gardée (une vraie panne du catalogue de cette version, pas finie), et deux fichiers qui ne sont pas à
    # Mindcare
    $inventes = [ordered]@{ EssaiMaj = 'garder-moi'; EssaiNombre = 42; EssaiAccents = "é à ü œ ç"; EssaiListe = @('un', 'deux') }
    [IO.File]::WriteAllText((Join-Path $donnees 'reglages.json'), ($inventes | ConvertTo-Json -Depth 5), $utf8)
    $catalogue = [IO.File]::ReadAllText((Join-Path $prog 'fichiers\enquetes.json'), [Text.Encoding]::UTF8).TrimStart([char]0xFEFF) | ConvertFrom-Json
    $panne = [string]@($catalogue.Pannes)[0].Id
    if (-not $panne) { throw "aucune panne dans le catalogue de Mindcare $versionAncienne" }
    $idEnquete = 'essai-maj-0001'
    $quand = (Get-Date).AddDays(-1).ToString('s')
    $enquete = [ordered]@{ Id = $idEnquete; Panne = $panne; Choix = @(0); Reponses = @(); Debut = $quand; Maj = $quand; Fini = $false; Suspect = ''; Titre = '' }
    [IO.File]::WriteAllText((Join-Path $donnees 'diagnostics.json'), (ConvertTo-Json ([ordered]@{ Version = 2; Diagnostics = @($enquete) }) -Depth 6), $utf8)
    [IO.File]::WriteAllText((Join-Path $donnees 'note-essai.txt'), "Un fichier laissé là par l'utilisateur, é à ü`r`n", $utf8)
    [IO.File]::WriteAllBytes((Join-Path $donnees 'images-essai\essai.png'), [byte[]](0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4))
    Invoke-Tour "Mindcare $versionAncienne (PowerShell)"
    $releve = Get-Releve
    Write-Host "  relevé : $($releve.Count) fichiers ($(@($releve.Keys) -join ', '))"
    Test-Cas "Mindcare $versionAncienne a gardé les données inventées (sinon ce test ne prouverait rien)" {
        $p = $releve['reglages.json']
        $p -and $p.Json -is [hashtable] -and $p.Json['champ:EssaiMaj'] -ceq '"garder-moi"' -and $p.Json['champ:EssaiNombre'] -ceq '42' -and
        (@(Get-IdsEnquetes) -contains $idEnquete) -and $releve.Contains('note-essai.txt') -and $releve.Contains('images-essai\essai.png')
    }

    Write-Host ''; Write-Host '2. La mise à jour : Mindcare en Rust à la place, mêmes données' -ForegroundColor Cyan
    Remove-Item -LiteralPath $prog -Recurse -Force
    Copy-Item -LiteralPath ([IO.Path]::GetFullPath($Nouvelle)) -Destination $prog -Recurse
    $vj = [IO.File]::ReadAllText((Join-Path $prog 'version.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json
    Write-Host "  version en Rust installée : $($vj.version)"
    Invoke-Tour "Mindcare $($vj.version) (Rust)"

    Write-Host ''; Write-Host '3. Rien de perdu' -ForegroundColor Cyan
    $apres = Get-Releve
    foreach ($rel in $releve.Keys) {
        $a = $releve[$rel]; $b = $apres[$rel]
        if ($a.Json -is [hashtable]) {
            Test-Cas "$rel : toujours là, lisible, chaque champ et élément d'avant gardé" {
                if (-not $b -or $b.Json -isnot [hashtable]) { throw $(if ($b) { 'illisible ou plus du même genre' } else { 'disparu' }) }
                $perdus = @($a.Json.Keys | Where-Object { -not $b.Json.ContainsKey($_) })
                if ($perdus.Count) { throw "perdus : $($perdus -join ', ')" }
                $true
            }
        } elseif ($rel -match '\.bak$') {
            # (une copie de secours est refaite à chaque écriture du fichier : seulement qu'elle soit encore là)
            Test-Cas "$rel : copie de secours toujours là" { [bool]$b }
        } else {
            Test-Cas "$rel : toujours là, identique" { $b -and $b.Hash -eq $a.Hash }
        }
    }
    Test-Cas 'les valeurs inventées de reglages.json sont exactement les mêmes' {
        $b = $apres['reglages.json'].Json
        $changes = @($inventes.Keys | Where-Object { $b["champ:$_"] -cne $releve['reglages.json'].Json["champ:$_"] })
        if ($changes.Count) { throw "changées : $($changes -join ', ')" }
        $true
    }
    Test-Cas "l'enquête gardée ($idEnquete) est toujours dans diagnostics.json" { @(Get-IdsEnquetes) -contains $idEnquete }
    Test-Cas 'le dossier des données n''a pas changé de place (rien dans %LOCALAPPDATA%\Mindset\Mindcare ni dans « Mindcare (travail) »)' {
        $apresAilleurs = Get-Ailleurs
        if ($apresAilleurs -cne $avantAilleurs) { throw "ajouté ou changé : $(@(Compare-Object @($avantAilleurs -split "`n") @($apresAilleurs -split "`n") | ForEach-Object { $_.InputObject }) -join ', ')" }
        $true
    }
} finally {
    if ($null -ne $avantSuite) { [IO.File]::WriteAllBytes($suiteFichier, $avantSuite) } else { Remove-Item -LiteralPath $suiteFichier -Force -ErrorAction SilentlyContinue }
}

Write-Host ''
if ($script:Echecs.Count) { Write-Host "Mise à jour depuis la version PowerShell : $($script:Echecs.Count) cas raté(s) sur $($script:Reussis + $script:Echecs.Count)." -ForegroundColor Red; exit 1 }
Write-Host "Mise à jour depuis la version PowerShell : $($script:Reussis) cas sur $($script:Reussis), rien de perdu." -ForegroundColor Green
exit 0
