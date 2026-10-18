# Teste Mindcare automatiquement : l'appli lancée hors de l'écran, avec des données à part (_test), dans plusieurs
# situations, avec une capture d'écran pour chacune. Rien n'est modifié sur le PC, et les vraies applis ne sont jamais
# touchées (le mode test n'utilise ni le verrou ni le dossier de données de la vraie Mindcare).
#
#   .\outils\tester.ps1        tous les tests (publier.ps1 le lance avant chaque publication)
#
# Ajoute ici un scénario pour chaque fonction de l'appli. Code de sortie : 0 si tout est bon, 1 sinon.

param([int]$Delai = 60)
$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$main   = Join-Path $racine 'Mindcare\fichiers\Mindcare.ps1'
$exe    = Join-Path $racine 'Mindcare\Mindcare.exe'
$test   = Join-Path $racine '_test'
New-Item -ItemType Directory -Force -Path $test | Out-Null
$script:Ok = $true

function Write-Resultat([string]$Nom, [bool]$Bon, [string]$Detail = '') {
    if ($Bon) { Write-Host "OK     $Nom" -ForegroundColor Green } else { Write-Host "ÉCHEC  $Nom $Detail" -ForegroundColor Red; $script:Ok = $false }
}

# Lance l'appli (le script, ou le programme avec -Programme ; pas « -Exe » : il cacherait $exe) dans une situation donnée et renvoie le processus, ses données et sa capture
function Start-Scenario([string]$Nom, [hashtable]$Env = @{}, [switch]$Programme) {
    $donnees = Join-Path $test "donnees-$Nom"
    if (Test-Path -LiteralPath $donnees) { Remove-Item -LiteralPath $donnees -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $donnees | Out-Null
    $capture = Join-Path $test "capture-$Nom.png"
    Remove-Item -LiteralPath $capture -ErrorAction SilentlyContinue
    Get-ChildItem env: | Where-Object { $_.Name -like 'MINDCARE_*' } | ForEach-Object { Remove-Item "env:$($_.Name)" }
    $env:MINDCARE_TEST = '1'; $env:MINDCARE_DONNEES = $donnees
    foreach ($k in $Env.Keys) { Set-Item "env:$k" $Env[$k] }
    $err = Join-Path $test "erreurs-$Nom.txt"
    $p = if ($Programme) { Start-Process $exe -ArgumentList '-Demarrage' -PassThru }
         else { Start-Process powershell.exe -ArgumentList '-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', "`"$main`"" -PassThru -WindowStyle Hidden -RedirectStandardError $err }
    Get-ChildItem env: | Where-Object { $_.Name -like 'MINDCARE_*' } | ForEach-Object { Remove-Item "env:$($_.Name)" }
    @{ P = $p; Donnees = $donnees; Capture = $capture; Erreurs = $err }
}

function Get-Journal($S) { $j = Join-Path $S.Donnees 'journal.txt'; if (Test-Path -LiteralPath $j) { [IO.File]::ReadAllText($j, [Text.Encoding]::UTF8) } else { '' } }
function Get-Erreurs($S) { if (Test-Path -LiteralPath $S.Erreurs) { ((Get-Content -LiteralPath $S.Erreurs -Raw) + '').Trim() } else { '' } }

# Une fenêtre qui s'affiche, se capture et se ferme toute seule, sans erreur PowerShell
function Test-Capture([string]$Nom, [hashtable]$Env = @{}) {
    $env2 = @{ MINDCARE_CAPTURE = (Join-Path $test "capture-$Nom.png") } + $Env
    $s = Start-Scenario $Nom $env2
    if (-not $s.P.WaitForExit($Delai * 1000)) { $s.P.Kill(); Write-Resultat $Nom $false "(ne s'est pas fermée en $Delai s)"; return $s }
    $e = Get-Erreurs $s
    Write-Resultat $Nom ((Test-Path -LiteralPath $s.Capture) -and -not $e) $(if ($e) { "(erreurs : $($e.Substring(0, [math]::Min(300, $e.Length))))" } else { '(pas de capture)' })
    $s
}

# 1. La fenêtre, dans le thème Néon puis dans un autre thème de la suite
$s = Test-Capture 'fenetre'
Test-Capture 'theme-crepuscule' @{ MINDCARE_THEME = 'crepuscule' } | Out-Null
# 2. La carte vivante pour Mindset : etat.json au format commun
$etat = Join-Path $s.Donnees 'etat.json'
$bon = $false
try { $j = [IO.File]::ReadAllText($etat, [Text.Encoding]::UTF8) | ConvertFrom-Json; $bon = ($j.Version -eq 1) -and (@($j.Lignes).Count -ge 1) -and (@($j.Lignes).Count -le 3) -and (@($j.Scores).Count -le 2) } catch {}
Write-Resultat 'carte vivante (etat.json)' $bon
# 3. Une copie hors de Mindset : la fenêtre « s'utilise avec Mindset »
Test-Capture 'blocage' @{ MINDCARE_TEST_BLOCAGE = '1' } | Out-Null

# 4. Lancée cachée par Mindset, puis « Tout quitter » : elle doit quitter d'elle-même, vite
function Test-Quitter([string]$Nom, [switch]$Programme) {
    $s = Start-Scenario $Nom @{ MINDCARE_TEST_DEMARRAGE = '1' } -Programme:$Programme
    $fin = (Get-Date).AddSeconds($Delai)
    while ((Get-Date) -lt $fin -and (Get-Journal $s) -notmatch 'Démarrage ') { Start-Sleep -Milliseconds 300 }
    Start-Sleep -Seconds 2
    [IO.File]::WriteAllText((Join-Path $s.Donnees 'quitter.demande'), 'test')
    $parti = $s.P.WaitForExit(8000)
    if (-not $parti) { $s.P.Kill() }
    Write-Resultat $Nom ($parti -and (Get-Journal $s) -match 'Fermeture demandée par Mindset') $(if (-not $parti) { '(encore ouverte 8 s après la demande : Mindset la fermerait de force)' } else { '(pas de trace dans le journal)' })
}
Test-Quitter 'quitter (script)'
# 5. Le vrai programme : son processus doit disparaître (sinon Mindset ne pourrait plus la rouvrir)
if (Test-Path -LiteralPath $exe) { Test-Quitter 'quitter (programme)' -Programme } else { Write-Host "(pas de $exe : lance construire.ps1 pour tester le programme)" -ForegroundColor Yellow }

if ($script:Ok) { Write-Host "`nTout est bon. Captures : $test" -ForegroundColor Green; exit 0 }
Write-Host "`nDes tests ont échoué. Captures et erreurs : $test" -ForegroundColor Red
exit 1
