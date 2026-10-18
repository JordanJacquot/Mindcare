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

# 4. Les pannes (questions.json), sans fenêtre : le fichier, puis des parcours dont on connaît la bonne conclusion
$AppDir = Join-Path $racine 'Mindcare\fichiers'
. (Join-Path $AppDir 'modules\diagnostic.ps1')
$pannes = Read-Pannes
$pb = @(Test-Pannes $pannes ([IO.File]::ReadAllText($QuestionsFichier, [Text.Encoding]::UTF8)))
Write-Resultat 'questions.json (aucune impasse, aucune boucle, rien d''inconnu)' (-not $pb) "($($pb -join ' ; '))"
# Chaque cas : la panne, les choix (numéro de réponse, -1 pour « Je ne sais pas »), la cause attendue en premier
# (vide : rien de clair), et les causes qui ne doivent pas apparaître du tout
$cas = @(
    @{ Nom = 'le cousin (PC fixe, coupé au bout de 5 min)'; Panne = 'extinction'; Choix = 1, 1, 0, 0, -1, 0, 2, 3; Premiere = 'surchauffe'; Jamais = @('batterie') }
    @{ Nom = 'odeur de brûlé'; Panne = 'extinction'; Choix = @(0); Premiere = 'danger' }
    @{ Nom = 'tout en « Je ne sais pas »'; Panne = 'extinction'; Choix = -1, -1, -1, -1, -1, -1, -1, -1; Premiere = '' }
    @{ Nom = 'portable qui lâche sur batterie'; Panne = 'extinction'; Choix = 1, 0, 0, 0, 3, 1, 2, -1, 3; Premiere = 'batterie'; Jamais = @('alimentation') }
    @{ Nom = 'écran bleu après une mise à jour'; Panne = 'extinction'; Choix = 1, 1, 1, 3, 1, 2, 2, 0; Premiere = 'windows' }
    @{ Nom = 'mise en veille'; Panne = 'extinction'; Choix = 1, 1, 3, 2, 1, 2, 2, 3; Premiere = 'veille' }
    @{ Nom = 'la maison qui coupe aussi'; Panne = 'extinction'; Choix = 1, 1, 0, 3, 1, 2, 3, -1; Premiere = 'prise' }
    @{ Nom = 'portable tombé par terre'; Panne = 'extinction'; Choix = 1, 0, 1, 0, -1, -1, 2, 2, 1; Premiere = 'materiel'; Jamais = @('alimentation') }
    @{ Nom = 'PC fixe, coupure en jeu, ventilateurs muets'; Panne = 'extinction'; Choix = 1, 1, 0, 1, 2, 1, 2, 3; Premiere = 'surchauffe'; Jamais = @('batterie') }
    @{ Nom = 'portable sans voyant de charge'; Panne = 'rien'; Choix = 1, 0, 0, 1, 0, 2; Premiere = 'chargeur'; Jamais = @('alimentation', 'interrupteur') }
    @{ Nom = 'tour, interrupteur sur O'; Panne = 'rien'; Choix = 1, 0, 1, 0, 0, 1, 3; Premiere = 'interrupteur'; Jamais = @('chargeur') }
    @{ Nom = 'une lumière, mais écran noir'; Panne = 'rien'; Choix = 1, 1; Premiere = 'ecran' }
    @{ Nom = 'tour après un orage, tout essayé'; Panne = 'rien'; Choix = 1, 0, 1, 0, 1, 0, 0; Premiere = 'alimentation'; Jamais = @('chargeur') }
    @{ Nom = 'prise morte'; Panne = 'rien'; Choix = 1, 0, 1, 1, 2, 1, 3; Premiere = 'courant' }
    @{ Nom = 'jamais vidé de son électricité'; Panne = 'rien'; Choix = 1, 0, -1, 1, 3; Premiere = 'residuelle' }
    @{ Nom = 'écran noir avec la flèche de la souris'; Panne = 'ecran-noir'; Choix = 1, 2, 1, 1, 2, 1, 1, 0; Premiere = 'pilote' }
    @{ Nom = 'pas de signal, câble en haut'; Panne = 'ecran-noir'; Choix = 1, 0, 1, 1, 1, 0, 1, 3; Premiere = 'cablecarte'; Jamais = @('eclairage', 'ecranportable') }
    @{ Nom = 'écran éteint'; Panne = 'ecran-noir'; Choix = 1, 0, 1, 1, 0, 2, 1, 3; Premiere = 'ecranalim' }
    @{ Nom = 'il dormait'; Panne = 'ecran-noir'; Choix = 1, 3, 0; Premiere = 'veille' }
    @{ Nom = 'portable, image devinée à la lampe'; Panne = 'ecran-noir'; Choix = 1, 0, 1, 0, 0, 1, 3; Premiere = 'eclairage'; Jamais = @('cable', 'ecranalim', 'cablecarte') }
    @{ Nom = 'des bips au démarrage'; Panne = 'ecran-noir'; Choix = 1, 0, 1, 1, 2, 2, 0, 2; Premiere = 'memoire' }
    @{ Nom = 'portable tombé, plus d''image'; Panne = 'ecran-noir'; Choix = 1, 0, 1, 0, 1, 1, 1; Premiere = 'materiel'; Jamais = @('cable') }
    @{ Nom = 'nouvelle manette'; Panne = 'ecran-bleu'; Choix = 0, 0, 2, 1, 1, 2; Premiere = 'appareil' }
    @{ Nom = 'écran bleu à chaque démarrage'; Panne = 'ecran-bleu'; Choix = 1, 3, 3, 1, 1, 2; Premiere = 'bloque' }
    @{ Nom = 'écrans bleus depuis l''ouverture du PC'; Panne = 'ecran-bleu'; Choix = 0, 3, 1, 1, 1, 0; Premiere = 'memoire' }
    @{ Nom = 'disque qui fait des clics'; Panne = 'ecran-bleu'; Choix = 0, 3, 1, 1, 0, 2; Premiere = 'disque' }
    @{ Nom = 'jeu qui fait chauffer'; Panne = 'ecran-bleu'; Choix = 0, 3, 0, 0, 1, 2; Premiere = 'surchauffe' }
    @{ Nom = 'écrans bleus après une mise à jour'; Panne = 'ecran-bleu'; Choix = 0, 2, 1, 1, 1, 2; Premiere = 'pilote' }
    @{ Nom = 'jamais redémarré'; Panne = 'lent'; Choix = 3, 0, 1, 1, 1, 1, 1, 0, 1; Premiere = 'redemarrage' }
    @{ Nom = 'pubs depuis un programme'; Panne = 'lent'; Choix = 2, 1, 0, 0, 1, 1, 1, 0, 1; Premiere = 'indesirables' }
    @{ Nom = 'vieux PC à disque dur'; Panne = 'lent'; Choix = 0, 1, 1, 1, 1, 1, 0, 2, 1; Premiere = 'disquevieux' }
    @{ Nom = 'disque plein'; Panne = 'lent'; Choix = 1, 1, 1, 1, 0, 1, 1, 0, 1; Premiere = 'disqueplein' }
    @{ Nom = 'trop de pages ouvertes'; Panne = 'lent'; Choix = 1, 1, 2, 1, 1, 1, 1, 0, 1; Premiere = 'memoire' }
    @{ Nom = 'mise à jour en cours'; Panne = 'lent'; Choix = 3, 1, -1, 1, 1, 1, 1, 0, 0; Premiere = 'maj' }
)
# Chaque panne a ses cas connus
foreach ($p in $pannes.Pannes) { Write-Resultat "cas connus pour la panne $($p.Id)" (@($cas | Where-Object { $_.Panne -eq $p.Id }).Count -ge 3) }
foreach ($k in $cas) {
    $p = Get-Panne $pannes $k.Panne
    $e = Get-Parcours $p $k.Choix
    $conc = @(Get-Conclusion $p $e)
    $ids = @($conc | ForEach-Object { [string]$_.Cause.Id })
    $premiere = if ($ids) { $ids[0] } else { '' }
    $bon = -not $e.Question -and $premiere -eq $k.Premiere -and -not @($ids | Where-Object { $k.Jamais -contains $_ })
    Write-Resultat "cas : $($k.Nom)" $bon "(fini : $(-not $e.Question), causes : $($ids -join ', '))"
}
# Des centaines de parcours tirés au hasard (toujours les mêmes) : chacun finit, sans cause écartée par une réponse
$mauvais = @()
$hasard = New-Object Random 42
foreach ($p in $pannes.Pannes) {
    for ($t = 0; $t -lt 500; $t++) {
        $choix = @(); $e = Get-Parcours $p @()
        while ($e.Question -and $choix.Count -lt 40) { $choix += ($hasard.Next(-1, $e.Question.Reponses.Count)); $e = Get-Parcours $p $choix }
        $conc = @(Get-Conclusion $p $e)
        $ecartee = @($conc | Where-Object { foreach ($s in $_.Cause.Sauf) { if ($e.Notes -contains $s) { $true } } })
        if ($e.Question -or $ecartee -or $conc.Count -gt 3) { $mauvais += "$($p.Id):$($choix -join ',')" }
    }
}
Write-Resultat 'parcours au hasard (500 par panne)' (-not $mauvais) "($($mauvais | Select-Object -First 3))"

# 5. Les écrans du diagnostic : une question, une conclusion (celle du cousin), le danger
Test-Capture 'question' @{ MINDCARE_TEST_PARCOURS = 'extinction:1,1' } | Out-Null
$s = Test-Capture 'conclusion' @{ MINDCARE_TEST_PARCOURS = 'extinction:1,1,0,0,-1,0,2,3' }
Write-Resultat 'conclusion affichée (journal)' ((Get-Journal $s) -match 'Conclusion extinction : surchauffe') '(pas de conclusion « surchauffe » dans le journal)'
Test-Capture 'conclusion-bas' @{ MINDCARE_TEST_PARCOURS = 'extinction:1,1,0,0,-1,0,2,3'; MINDCARE_TEST_BAS = '1' } | Out-Null
Test-Capture 'conclusion-danger' @{ MINDCARE_TEST_PARCOURS = 'extinction:0' } | Out-Null
Test-Capture 'conclusion-renvoi' @{ MINDCARE_TEST_PARCOURS = 'rien:1,1' } | Out-Null
Test-Capture 'conclusion-bon-a-savoir' @{ MINDCARE_TEST_PARCOURS = 'ecran-bleu:0,0,2,1,1,2' } | Out-Null

# 6. Lancée cachée par Mindset, puis « Tout quitter » : elle doit quitter d'elle-même, vite
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
# 7. Le vrai programme : son processus doit disparaître (sinon Mindset ne pourrait plus la rouvrir)
if (Test-Path -LiteralPath $exe) { Test-Quitter 'quitter (programme)' -Programme } else { Write-Host "(pas de $exe : lance construire.ps1 pour tester le programme)" -ForegroundColor Yellow }

if ($script:Ok) { Write-Host "`nTout est bon. Captures : $test" -ForegroundColor Green; exit 0 }
Write-Host "`nDes tests ont échoué. Captures et erreurs : $test" -ForegroundColor Red
exit 1
