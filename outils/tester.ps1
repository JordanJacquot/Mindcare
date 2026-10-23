# Teste Mindcare automatiquement : l'appli lancée hors de l'écran, avec des données à part (_test), dans plusieurs
# situations, avec une capture d'écran pour chacune. Rien n'est modifié sur le PC, et les vraies applis ne sont jamais
# touchées (le mode test n'utilise ni le verrou ni le dossier de données de la vraie Mindcare).
#
#   .\outils\tester.ps1                tous les tests (publier.ps1 le lance avant chaque publication)
#   .\outils\tester.ps1 -SansFenetre   seulement ce qui n'ouvre aucune fenêtre, ne lit rien de ce PC et ne va pas sur
#                                      Internet (la CI de GitHub, .github\workflows\tests.yml)
#
# Ajoute ici un scénario pour chaque fonction de l'appli. Code de sortie : 0 si tout est bon, 1 sinon.

param([int]$Delai = 60, [switch]$SansFenetre)
$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$main   = Join-Path $racine 'Mindcare\fichiers\Mindcare.ps1'
$exe    = Join-Path $racine 'Mindcare\Mindset.exe'    # le programme (même nom pour toute la suite : une ligne « Mindset »)
$relais = Join-Path $racine 'Mindcare\Mindcare.exe'   # le relais vers Mindset.exe (ce que les anciennes versions de Mindset lancent)
$test   = Join-Path $racine '_test'
New-Item -ItemType Directory -Force -Path $test | Out-Null
$script:Ok = $true

function Write-Resultat([string]$Nom, [bool]$Bon, [string]$Detail = '') {
    if ($Bon) { Write-Host "OK     $Nom" -ForegroundColor Green } else { Write-Host "ÉCHEC  $Nom $Detail" -ForegroundColor Red; $script:Ok = $false }
}

# Lance l'appli (le script, le programme avec -Programme, ou son relais avec -ViaRelais ; pas « -Exe » ni « -Relais » : ils cacheraient $exe et $relais) dans une situation donnée et renvoie le processus, ses données et sa capture
# (-Dossier : les données d'un autre scénario, gardées telles quelles, pour enchaîner plusieurs lancements)
function Start-Scenario([string]$Nom, [hashtable]$Env = @{}, [switch]$Programme, [switch]$ViaRelais, [string]$Dossier) {
    $donnees = Join-Path $test "donnees-$(if ($Dossier) { $Dossier } else { $Nom })"
    if (-not $Dossier -and (Test-Path -LiteralPath $donnees)) { Remove-Item -LiteralPath $donnees -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $donnees | Out-Null
    $capture = Join-Path $test "capture-$Nom.png"
    Remove-Item -LiteralPath $capture -ErrorAction SilentlyContinue
    Get-ChildItem env: | Where-Object { $_.Name -like 'MINDCARE_*' } | ForEach-Object { Remove-Item "env:$($_.Name)" }
    $env:MINDCARE_TEST = '1'; $env:MINDCARE_DONNEES = $donnees
    foreach ($k in $Env.Keys) { Set-Item "env:$k" $Env[$k] }
    $err = Join-Path $test "erreurs-$Nom.txt"
    $p = if ($Programme -or $ViaRelais) { Start-Process $(if ($ViaRelais) { $relais } else { $exe }) -ArgumentList '-Demarrage' -PassThru }
         else { Start-Process powershell.exe -ArgumentList '-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', "`"$main`"" -PassThru -WindowStyle Hidden -RedirectStandardError $err }
    Get-ChildItem env: | Where-Object { $_.Name -like 'MINDCARE_*' } | ForEach-Object { Remove-Item "env:$($_.Name)" }
    @{ P = $p; Donnees = $donnees; Capture = $capture; Erreurs = $err }
}

function Get-Journal($S) { $j = Join-Path $S.Donnees 'journal.txt'; if (Test-Path -LiteralPath $j) { [IO.File]::ReadAllText($j, [Text.Encoding]::UTF8) } else { '' } }
function Get-Erreurs($S) { if (Test-Path -LiteralPath $S.Erreurs) { ((Get-Content -LiteralPath $S.Erreurs -Raw) + '').Trim() } else { '' } }

# Une fenêtre qui s'affiche, se capture et se ferme toute seule, sans erreur PowerShell
function Test-Capture([string]$Nom, [hashtable]$Env = @{}, [string]$Dossier) {
    $env2 = @{ MINDCARE_CAPTURE = (Join-Path $test "capture-$Nom.png") } + $Env
    $s = Start-Scenario $Nom $env2 -Dossier $Dossier
    if (-not $s.P.WaitForExit($Delai * 1000)) { $s.P.Kill(); Write-Resultat $Nom $false "(ne s'est pas fermée en $Delai s)"; return $s }
    $e = Get-Erreurs $s
    Write-Resultat $Nom ((Test-Path -LiteralPath $s.Capture) -and -not $e) $(if ($e) { "(erreurs : $($e.Substring(0, [math]::Min(300, $e.Length))))" } else { '(pas de capture)' })
    $s
}

if (-not $SansFenetre) {
    # 1. La fenêtre, dans le thème Néon puis dans un autre thème de la suite
    $s = Test-Capture 'fenetre'
    Test-Capture 'theme-crepuscule' @{ MINDCARE_THEME = 'crepuscule' } | Out-Null
    # Les autres thèmes de base de la suite
    foreach ($th in 'terminal', 'arcade', 'rubis') { Test-Capture "theme-$th" @{ MINDCARE_THEME = $th; MINDCARE_TEST_PARCOURS = 'redemarre:1,1' } | Out-Null }
    # Un thème perso de test (images dessinées ici, police de Windows : rien d'un vrai thème, rien dans le dépôt), complet :
    # couleurs propres, police pixel partout, image de fond, logo qui lévite, mascotte et dessin du bas animés, phrase
    Add-Type -AssemblyName System.Drawing
    $pk = Join-Path $test 'donnees-pack'
    if (Test-Path -LiteralPath $pk) { Remove-Item -LiteralPath $pk -Recurse -Force }
    $pd = Join-Path $pk 'packs\test'
    New-Item -ItemType Directory -Force -Path $pd | Out-Null
    function New-ImageTest([string]$Nom, [int]$L, [int]$H, [int]$Images, [string]$Couleur) {
        $b = New-Object Drawing.Bitmap ($L * $Images), $H
        $g = [Drawing.Graphics]::FromImage($b); $g.Clear([Drawing.Color]::Transparent)
        for ($k = 0; $k -lt $Images; $k++) { $g.FillEllipse((New-Object Drawing.SolidBrush ([Drawing.ColorTranslator]::FromHtml($Couleur))), ($k * $L + 4 + $k * 3), 4, ($L - 12), ($H - 8)) }
        $g.Dispose(); $b.Save((Join-Path $pd $Nom), [Drawing.Imaging.ImageFormat]::Png); $b.Dispose()
    }
    New-ImageTest 'logo.png' 64 64 1 '#FFD166'
    New-ImageTest 'mascotte.png' 64 64 4 '#3DDC97'
    New-ImageTest 'bas.png' 48 24 2 '#FF8A00'
    $fond = New-Object Drawing.Bitmap 640, 360
    $g = [Drawing.Graphics]::FromImage($fond); $g.FillRectangle((New-Object Drawing.Drawing2D.LinearGradientBrush ([Drawing.Point]::new(0, 0)), ([Drawing.Point]::new(640, 360)), ([Drawing.Color]::DarkGreen), ([Drawing.Color]::DarkOrange)), 0, 0, 640, 360); $g.Dispose()
    $fond.Save((Join-Path $pd 'fond.jpg'), [Drawing.Imaging.ImageFormat]::Jpeg); $fond.Dispose()
    Copy-Item -LiteralPath "$env:WINDIR\Fonts\consola.ttf" -Destination (Join-Path $pd 'police.ttf')
    $reglage = [ordered]@{ Name = 'Test'; Colors = @{ P = '#3FA66B'; S = '#E0773A'; T = '#F2C66D'; NH = 24; NS = 0.45 }; Hello = '{0} ! Ceci est un thème de test.'
        Font = 'police.ttf'; FontScope = 'tout'; Logo = 'logo.png'; LogoGlow = '#FFF4D6'; Background = 'fond.jpg'; BackgroundOpacity = 0.5; BackgroundAlign = 'bas-droite'
        Mascotte = @{ File = 'mascotte.png'; Frames = 4; Delay = 120 }; Footer = @{ File = 'bas.png'; Frames = 2; Delay = 200; Height = 24 } }
    [IO.File]::WriteAllText((Join-Path $pd 'pack.json'), (ConvertTo-Json $reglage -Depth 4), (New-Object Text.UTF8Encoding($false)))
    $s = Test-Capture 'theme-perso' @{ MINDCARE_THEME = 'pack:test' } -Dossier 'pack'
    Write-Resultat 'thème perso : posé (journal)' ((Get-Journal $s) -match 'Thème perso : test' -and (Get-Journal $s) -notmatch 'visuels non posés') '(pas de « Thème perso : test » dans le journal)'
    Test-Capture 'theme-perso-question' @{ MINDCARE_THEME = 'pack:test'; MINDCARE_TEST_PARCOURS = 'redemarre:1,1' } -Dossier 'pack' | Out-Null
    $s = Test-Capture 'theme-perso-absent' @{ MINDCARE_THEME = 'pack:nexistepas' }
    Write-Resultat 'thème perso absent : Néon, sans erreur' ((Get-Journal $s) -match 'introuvable : Néon') '(pas de retour au Néon dans le journal)'

    # 2. La carte vivante pour Mindset : etat.json au format commun
    $etat = Join-Path $s.Donnees 'etat.json'
    $bon = $false
    try { $j = [IO.File]::ReadAllText($etat, [Text.Encoding]::UTF8) | ConvertFrom-Json; $bon = ($j.Version -eq 1) -and (@($j.Lignes).Count -ge 1) -and (@($j.Lignes).Count -le 3) -and (@($j.Scores).Count -le 2) } catch {}
    Write-Resultat 'carte vivante (etat.json)' $bon
    # 3. Une copie hors de Mindset : la fenêtre « s'utilise avec Mindset »
    Test-Capture 'blocage' @{ MINDCARE_TEST_BLOCAGE = '1' } | Out-Null
}

# 4. Les enquêtes (enquetes.json), sans fenêtre : le fichier, puis des enquêtes dont on connaît la bonne réponse
$AppDir = Join-Path $racine 'Mindcare\fichiers'
. (Join-Path $AppDir 'modules\enquete.ps1')
$donnees = Read-Enquetes
$pb = @(Test-Enquetes $donnees ([IO.File]::ReadAllText($EnquetesFichier, [Text.Encoding]::UTF8)))
Write-Resultat 'enquetes.json (rien d''inconnu ni de manquant)' (-not $pb) "($($pb -join ' ; '))"
function New-Indice([bool]$Vu, [string]$Texte = '') { [pscustomobject]@{ Vu = $Vu; Texte = $Texte; TexteOk = 'rien de noté'; Affiche = $true } }
# Chaque cas : la panne, les réponses (numéro, -1 pour « Je ne sais pas »), les indices de Windows éventuels (inventés),
# et la réponse attendue (Type : trouve, elimination, reparateur ; Suspect, ou Restants pour le réparateur)
$cas = @(
    # Cas réels de Jordan (octobre 2026) : son PC et celui de son cousin redémarraient tout seuls sur le Bureau au bout de
    # quelques minutes, sans chauffer : une barrette de mémoire abîmée sur les deux
    @{ Nom = 'Jordan et son cousin (RAM abîmée)'; Panne = 'redemarre'; Choix = 1, 1, 1, 1, 1, 1, 0; Type = 'trouve'; Suspect = 'memoire' }
    @{ Nom = 'Jordan, sur son PC, test de mémoire déjà raté'; Panne = 'redemarre'; Choix = 1, 0; Indices = @{ 'testmemoire-erreurs' = (New-Indice $true 'Le test de mémoire a trouvé des erreurs') }; Type = 'trouve'; Suspect = 'memoire' }
    @{ Nom = 'redémarre, mémoire sans erreur : alimentation par élimination'; Panne = 'redemarre'; Choix = 1, 1, 1, 1, 1, 1, 1; Type = 'elimination'; Suspect = 'alimentation' }
    @{ Nom = 'redémarre la nuit, une mise à jour notée'; Panne = 'redemarre'; Choix = 1, 0, 0; Indices = @{ majredemarrage = (New-Indice $true 'redémarrage pour les mises à jour') }; Type = 'trouve'; Suspect = 'majnuit' }
    @{ Nom = 'redémarre après un écran bleu, réglé par les mises à jour'; Panne = 'redemarre'; Choix = 1, 1, 1, 0, 1, 1, 1, 0; Type = 'trouve'; Suspect = 'pilote' }
    @{ Nom = 'redémarre en chauffant, réglé par le nettoyage'; Panne = 'redemarre'; Choix = 1, 1, 1, 1, 0, 1, 1, 0; Type = 'trouve'; Suspect = 'surchauffe' }
    @{ Nom = 'ventilateurs muets'; Panne = 'redemarre'; Choix = 1, 1, 1, 1, 2; Type = 'trouve'; Suspect = 'surchauffe' }
    @{ Nom = 'après un choc'; Panne = 'redemarre'; Choix = 1, 1, 1, 1, 1, 0; Type = 'trouve'; Suspect = 'materiel' }
    @{ Nom = 'odeur de brûlé'; Panne = 'redemarre'; Choix = @(0); Type = 'trouve'; Suspect = 'danger' }
    @{ Nom = 'tout en « Je ne sais pas » : au réparateur'; Panne = 'redemarre'; Choix = 1, 1, -1, -1, -1, -1, -1, -1, -1; Type = 'reparateur' }
    @{ Nom = 'la maison coupe aussi'; Panne = 'seteint'; Choix = 1, 1, 0; Type = 'trouve'; Suspect = 'prise' }
    @{ Nom = 'portable qui lâche sur batterie'; Panne = 'seteint'; Choix = 1, 0, 1, 0; Type = 'trouve'; Suspect = 'batterie' }
    @{ Nom = 'multiprise : branché au mur, il tient'; Panne = 'seteint'; Choix = 1, 1, 1, 1, 1, 0, 0; Type = 'trouve'; Suspect = 'prise' }
    @{ Nom = 'PC fixe au mur, frais : alimentation par élimination'; Panne = 'seteint'; Choix = 1, 1, 1, 1, 1, 1; Type = 'elimination'; Suspect = 'alimentation' }
    @{ Nom = 'écran bleu depuis une nouvelle manette'; Panne = 'ecranbleu'; Choix = 0, 1, 6, 0, 0; Type = 'trouve'; Suspect = 'appareil' }
    @{ Nom = 'écran bleu à chaque démarrage : au réparateur'; Panne = 'ecranbleu'; Choix = 1, 1, 3, 1, 1, 1; Type = 'reparateur'; Restants = @('windowsabime', 'disque') }
    @{ Nom = 'écran bleu, Windows dit que le disque est malade'; Panne = 'ecranbleu'; Choix = 0, 0; Indices = @{ disque = (New-Indice $true 'Windows dit qu''un disque n''est pas en bonne santé') }; Type = 'trouve'; Suspect = 'disque' }
    @{ Nom = 'jeu : écran noir qui revient, réglé par le pilote'; Panne = 'jeu'; Choix = 1, 1, 1, 2, 1, 0; Type = 'trouve'; Suspect = 'pilote' }
    @{ Nom = 'jeu : des traits bizarres, frais : la carte graphique'; Panne = 'jeu'; Choix = 1, 1, 1, 3, 1; Type = 'elimination'; Suspect = 'gpu' }
    @{ Nom = 'se fige avec des clics : le disque'; Panne = 'fige'; Choix = 1, 1, 0; Type = 'trouve'; Suspect = 'disque' }
    @{ Nom = 'un seul jeu plante, réinstallé : réglé'; Panne = 'programme'; Choix = 1, 0, 0; Type = 'trouve'; Suspect = 'leprogramme' }
    @{ Nom = 'écran bleu MEMORY_MANAGEMENT, test de mémoire raté'; Panne = 'ecranbleu'; Choix = 0, 1, 0, 0; Type = 'trouve'; Suspect = 'memoire' }
    @{ Nom = 'écran bleu : code lu par Windows (pilote), réglé par les mises à jour'; Panne = 'ecranbleu'; Choix = 0, 0, 3, 1, 0; Indices = @{ 'bsod-pilote' = (New-Indice $true 'Code d''écran bleu noté par Windows : DRIVER_IRQL_NOT_LESS_OR_EQUAL') }; Type = 'trouve'; Suspect = 'pilote' }
    @{ Nom = 'écran bleu VIDEO_TDR_FAILURE, frais, pilote sans effet : la carte graphique'; Panne = 'ecranbleu'; Choix = 0, 1, 2, 1, 1; Type = 'elimination'; Suspect = 'gpu' }
    @{ Nom = 'rien ne s''allume : interrupteur sur O'; Panne = 'rien'; Choix = 1, 1, 0; Type = 'trouve'; Suspect = 'interrupteur' }
    @{ Nom = 'écran noir, pas de signal, image sur la prise du haut : la carte graphique'; Panne = 'ecrannoir'; Choix = 1, 1, 1, 1, 1, 1, 1, 1, 0; Type = 'trouve'; Suspect = 'gpu' }
    @{ Nom = 'tout est lent, le disque est plein'; Panne = 'toutlent'; Choix = 1, 1, 1, 0; Type = 'trouve'; Suspect = 'disqueplein' }
    @{ Nom = 'plus d''internet nulle part, la box redémarrée'; Panne = 'plusinternet'; Choix = 0, 0; Type = 'trouve'; Suspect = 'box' }
    @{ Nom = 'le Wi-Fi a disparu : le mode avion'; Panne = 'wifiabsent'; Choix = @(0); Type = 'trouve'; Suspect = 'wifioff' }
    @{ Nom = 'plus de son : le son était coupé'; Panne = 'plusdeson'; Choix = @(0); Type = 'trouve'; Suspect = 'soncoupe' }
    @{ Nom = 'son qui grésille (le cas de Jordan) : le service audio'; Panne = 'songresille'; Choix = @(1, 0); Type = 'trouve'; Suspect = 'serviceaudio' }
    @{ Nom = 'son qui grésille, rien de réglable : le casque'; Panne = 'songresille'; Choix = @(1, 1, 1, 1, 1, 1, 1); Type = 'trouve'; Suspect = 'hautparleurs' }
    @{ Nom = 'son trop fort : le volume d''une appli'; Panne = 'sonfort'; Choix = @(0); Type = 'trouve'; Suspect = 'volumeappli' }
    @{ Nom = 'Wi-Fi qui coupe (le cas du copain) : le Wi-Fi enregistré'; Panne = 'wificoupe'; Choix = @(1, 1, 0); Type = 'trouve'; Suspect = 'profilwifi' }
    @{ Nom = 'un seul site : le carnet d''adresses'; Panne = 'unsite'; Choix = @(1, 0); Type = 'trouve'; Suspect = 'dns' }
    @{ Nom = 'démarre sur le BIOS : une touche coincée'; Panne = 'biosboucle'; Choix = @(0); Type = 'trouve'; Suspect = 'touchecoincee' }
    @{ Nom = 'démarre sur le BIOS : aucun disque'; Panne = 'biosboucle'; Choix = @(1, 2); Type = 'elimination'; Suspect = 'disquenondetecte' }
    @{ Nom = 'démarre sur le BIOS : la pile'; Panne = 'biosboucle'; Choix = @(1, 1, 0); Type = 'trouve'; Suspect = 'pilebios' }
    @{ Nom = 'mot de passe oublié : compte Microsoft'; Panne = 'mdpoublie'; Choix = @(1, 0); Type = 'trouve'; Suspect = 'comptemicrosoft' }
    @{ Nom = 'barre des tâches figée : l''Explorateur'; Panne = 'barretaches'; Choix = @(0); Type = 'trouve'; Suspect = 'explorateur' }
    @{ Nom = 'le micro : le mauvais micro choisi'; Panne = 'micro'; Choix = 1, 0; Type = 'trouve'; Suspect = 'microchoix' }
    @{ Nom = 'l''imprimante hors ligne'; Panne = 'imprimante'; Choix = 1, 1, 0; Type = 'trouve'; Suspect = 'imprimantehorsligne' }
    @{ Nom = 'la batterie gonfle : urgent'; Panne = 'gonfle'; Choix = @(0); Type = 'trouve'; Suspect = 'batteriegonflee' }
    @{ Nom = 'faux support Microsoft'; Panne = 'arnaquesupport'; Choix = @(0); Type = 'trouve'; Suspect = 'arnaque' }
    @{ Nom = 'fichier supprimé, ni corbeille ni sauvegarde : un spécialiste'; Panne = 'fichiersupprime'; Choix = 1, 1; Type = 'elimination'; Suspect = 'perdu' }
)
foreach ($k in $cas) {
    $p = Get-Panne $donnees $k.Panne
    $e = Get-Enquete $donnees $p $k.Choix $k.Indices
    $v = $e.Verdict
    $bon = $v -and $v.Type -eq $k.Type -and (-not $k.Suspect -or $v.Suspect -eq $k.Suspect) -and (-not $k.Restants -or (@($v.Restants | Sort-Object) -join ',') -eq (@($k.Restants | Sort-Object) -join ','))
    Write-Resultat "cas : $($k.Nom)" $bon "(réponse : $(if ($v) { "$($v.Type) $($v.Suspect) $(@($v.Restants) -join ',')" } else { "pas finie, étape $($e.Etape.Id)" }))"
}
$ailleurs = Get-Enquete $donnees (Get-Panne $donnees 'redemarre') @(1, 1) @{ 'testmemoire-erreurs' = (New-Indice $true 'erreurs') }
Write-Resultat 'indices de Windows ignorés si Mindcare est ouvert sur un autre appareil' ($ailleurs.Etape -and -not $ailleurs.Auto) "(étape : $($ailleurs.Etape.Id))"
$av = @(foreach ($c in @(@(), @(1), @(1, 1, 1, 1, 1))) { $e = Get-Enquete $donnees (Get-Panne $donnees 'songresille') $c; "$(@($e.Faits).Count + 1)/$(@($e.Faits).Count + 1 + $e.AVenir)" })
Write-Resultat 'avancement « étape N sur M » (son qui grésille)' (($av -join ' ') -eq '1/8 2/7 6/7') "($($av -join ' '))"
# Des centaines d'enquêtes au hasard (toujours les mêmes) : chacune finit, avec une réponse
$mauvais = @(); $types = @{}
$hasard = New-Object Random 42
foreach ($p in $donnees.Pannes) {
    for ($t = 0; $t -lt 300; $t++) {
        $choix = @(); $e = Get-Enquete $donnees $p @()
        while ($e.Etape -and $choix.Count -lt 40) { $choix += $hasard.Next(-1, @($e.Etape.Reponses).Count); $e = Get-Enquete $donnees $p $choix }
        if ($e.Etape -or -not $e.Verdict) { $mauvais += "$($p.Id):$($choix -join ',')" } else { $types[$e.Verdict.Type] = 1 + [int]$types[$e.Verdict.Type] }
    }
}
Write-Resultat "enquêtes au hasard (300 par panne : $(($types.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Name) $($_.Value)" }) -join ', '))" (-not $mauvais) "($($mauvais | Select-Object -First 3))"
# La recherche par mots (plusieurs bonnes réponses possibles : séparées par |)
foreach ($r in @(@('il redémarre tout seul', 'redemarre'), @('écran bleu', 'ecranbleu'), @('mon pc se fige', 'fige'), @('mon jeu plante', 'programme|jeu'), @('il chauffe et plante en jeu', 'jeu'),
                 @('bip', 'bipsdemarrage'), @('wifi', 'wifiabsent|wificoupe'), @('mon imprimante n''imprime plus', 'imprimante'), @('la batterie gonfle', 'gonfle'), @('des pubs partout', 'pubs'),
                 @('pas de son', 'plusdeson'), @('le son est hyper fort', 'sonfort'), @('le wifi se coupe toutes les 5 minutes', 'wificoupe'), @('le pc démarre sur le bios', 'biosboucle'), @('ma webcam marche pas', 'webcam'), @('j''ai oublié mon mot de passe', 'mdpoublie'), @('no bootable device', 'nodisque'), @('le pc chauffe', 'chauffe'), @('micro discord', 'micro'), @('clé usb pas reconnue', 'usbnonreconnu'), @('MEMORY_MANAGEMENT', 'ecranbleu'), @('0x1A', 'ecranbleu'))) {
    $trouvees = @(Find-Pannes $donnees $r[0])
    Write-Resultat "recherche « $($r[0]) »" ($trouvees -and ($r[1] -split '\|') -contains $trouvees[0].Id) "(trouvé : $(@($trouvees | Select-Object -First 4 | ForEach-Object { $_.Id }) -join ', '))"
}
# Chaque suspect de chaque panne peut être identifié (trouvé, ou tout le reste écarté) par au moins une suite de réponses
function Get-SuspectsTrouvables($Panne, $Indices) {
    $trouves = @{}; $file = New-Object 'Collections.Generic.Queue[object]'; $file.Enqueue(@()); $n = 0
    while ($file.Count -and $n -lt 4000 -and $trouves.Count -lt @($Panne.Suspects).Count) {
        $choix = [int[]]$file.Dequeue(); $n++
        $e = Get-Enquete $donnees $Panne $choix $Indices
        if (-not $e.Etape) { if ($e.Verdict.Type -in 'trouve', 'elimination') { $trouves[[string]$e.Verdict.Suspect] = $true }; continue }
        for ($k = 0; $k -lt @($e.Etape.Reponses).Count; $k++) { $file.Enqueue(@($choix) + $k) }
    }
    $trouves
}
$tousVus = @{}; foreach ($i in $IndicesConnus) { $tousVus[$i] = New-Indice $true $i }
$introuvables = @()
foreach ($p in $donnees.Pannes) {
    $t = Get-SuspectsTrouvables $p $null
    $manque = @($p.Suspects | Where-Object { -not $t.ContainsKey([string]$_) })
    if ($manque) { $t2 = Get-SuspectsTrouvables $p $tousVus; $manque = @($manque | Where-Object { -not $t2.ContainsKey([string]$_) }) }
    foreach ($m in $manque) { $introuvables += "$($p.Id) : $m" }
}
Write-Resultat "chaque suspect peut être identifié ($(@($donnees.Pannes).Count) pannes)" (-not $introuvables) "(jamais identifiés : $($introuvables -join ' ; '))"

# Les images : chaque image citée (étapes, familles) existe, se dessine et n'utilise que ce que images.ps1 comprend
Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase
function Get-Brush([string]$Hex) { [System.Windows.Media.BrushConverter]::new().ConvertFromString($Hex) }   # (thème Néon)
. (Join-Path $AppDir 'modules\images.ps1')
$citees = @(@($donnees.Familles | ForEach-Object { [string]$_.Icone }) + @($donnees.Tests.PSObject.Properties | ForEach-Object { [string]$_.Value.Image }) + @($donnees.Pannes | ForEach-Object { $_.Etapes } | ForEach-Object { [string]$_.Image }) | Where-Object { $_ } | Select-Object -Unique)
$pbImages = @()
foreach ($nom in $citees) {
    try { $v = ConvertFrom-Svg $nom; if (-not $v) { $pbImages += "$nom introuvable" } elseif ($v.Child.Children.Count -lt 1) { $pbImages += "$nom vide" } }
    catch { $pbImages += "$nom : $($_.Exception.Message)" }
}
$enAttente = @()
foreach ($f in Get-ChildItem -LiteralPath $ImagesDir -Filter '*.svg') {
    if ($citees -notcontains $f.BaseName) { $enAttente += $f.BaseName }
    foreach ($m in [regex]::Matches([IO.File]::ReadAllText($f.FullName), '<([A-Za-z]+)')) {
        if ('svg', 'g', 'rect', 'circle', 'ellipse', 'line', 'polyline', 'polygon', 'path', 'text' -notcontains $m.Groups[1].Value) { $pbImages += "$($f.Name) : <$($m.Groups[1].Value)> pas compris" }
    }
}
Write-Resultat "images ($($citees.Count) citées ; en attente d'une famille : $($enAttente.Count))" (-not $pbImages) "($($pbImages -join ' ; '))"
# La vraie lecture des journaux, sur ce PC, en lecture seule (rien n'est modifié) : tous les indices, dans un temps raisonnable
if (-not $SansFenetre) {
    . (Join-Path $AppDir 'modules\journaux.ps1')
    $chrono = [Diagnostics.Stopwatch]::StartNew()
    $vrais = $null; try { $vrais = Get-IndicesWindows } catch { $erreurLecture = "$_" }
    $manquants = @($IndicesConnus | Where-Object { -not $vrais -or -not $vrais.ContainsKey($_) })
    Write-Resultat "lecture des journaux de Windows ($([math]::Round($chrono.Elapsed.TotalSeconds, 1)) s, lecture seule)" ($vrais -and -not $manquants -and $chrono.Elapsed.TotalSeconds -lt 20) "(manquants : $($manquants -join ', ') $erreurLecture)"

    # 5. Les écrans : l'accueil, une famille, la recherche, les étapes de l'enquête, les réponses
    Test-Capture 'famille' @{ MINDCARE_TEST_FAMILLE = 'plantages' } | Out-Null
    Test-Capture 'famille-internet' @{ MINDCARE_TEST_FAMILLE = 'internet' } | Out-Null
    Test-Capture 'accueil' | Out-Null
}
# Le garde-fou de l'assistant : une réponse réglée d'avance n'est gardée que si la phrase en parle
$DataDirSauve = $DataDir; $DataDir = $test
. (Join-Path $AppDir 'modules\ia.ps1')
$DataDir = $DataDirSauve
$pSet = Get-Panne $donnees 'seteint'
$gardes = @(foreach ($cas in @(@("mon ordi s'éteint tout seul d'un coup comme une coupure de courant", @{ clics = 1; 'maj-en-cours' = 1 }, ''),
                              @("mon pc fixe s'éteint d'un coup quand je joue", @{ type = 1; maison = 0 }, 'type'),
                              @("ça s'éteint et toutes les lumières de la maison aussi", @{ maison = 0 }, 'maison'),
                              @("il est sur une multiprise et il s'éteint", @{ multiprise = 0 }, 'multiprise'))) {
    $g = (@((Select-PreRempli $donnees $pSet $cas[1] $cas[0]).Keys) | Sort-Object) -join ','
    if ($g -ne $cas[2]) { "« $($cas[0]) » : $g au lieu de $($cas[2])" }
})
Write-Resultat "assistant : réponses réglées d'avance seulement si la phrase en parle" (-not $gardes) "($($gardes -join ' ; '))"
# L'assistant s'arrête avec Mindcare, même quand Mindset l'arrête de force (mise à jour, désinstallation) : un faux
# assistant (une pause de PowerShell) lancé par un faux Mindcare qui le relie comme Start-Ia, puis arrêté de force
$relie = Join-Path $test 'assistant-relie.txt'
Remove-Item -LiteralPath $relie -ErrorAction SilentlyContinue
$fauxMindcare = Join-Path $test 'faux-mindcare.ps1'
[IO.File]::WriteAllText($fauxMindcare, @'
param([string]$Natif, [string]$Sortie)
try {
    Add-Type -TypeDefinition ([IO.File]::ReadAllText($Natif, [Text.Encoding]::UTF8))
    $p = Start-Process powershell.exe -ArgumentList '-NoProfile', '-Command', 'Start-Sleep -Seconds 60' -WindowStyle Hidden -PassThru
    [IO.File]::WriteAllText($Sortie, "$($p.Id) $([MindcareNatif]::ArreterAvecMindcare($p.Handle))")
} catch { [IO.File]::WriteAllText($Sortie, "0 $_") }
Start-Sleep -Seconds 60
'@, (New-Object Text.UTF8Encoding($true)))
$fm = Start-Process powershell.exe -ArgumentList '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$fauxMindcare`"", '-Natif', "`"$(Join-Path $AppDir 'modules\natif.cs')`"", '-Sortie', "`"$relie`"" -WindowStyle Hidden -PassThru
$fin = (Get-Date).AddSeconds(60)
while (-not (Test-Path -LiteralPath $relie) -and (Get-Date) -lt $fin) { Start-Sleep -Milliseconds 300 }
$lu = if (Test-Path -LiteralPath $relie) { [IO.File]::ReadAllText($relie) } else { '' }
$idAssistant = [int](($lu -split ' ')[0] -as [int])
$avant = $idAssistant -and [bool](Get-Process -Id $idAssistant -ErrorAction SilentlyContinue)
try { $fm.Kill() } catch {}
Start-Sleep -Seconds 2
$apres = $idAssistant -and [bool](Get-Process -Id $idAssistant -ErrorAction SilentlyContinue)
if ($apres) { Stop-Process -Id $idAssistant -Force -ErrorAction SilentlyContinue }
Write-Resultat 'assistant : arrêté avec Mindcare, même fermée de force' ($avant -and -not $apres -and $lu -match ' True$') "(relié : $lu ; encore lancé après : $apres)"
# L'assistant tourne sous le nom Mindset.exe (une seule ligne « Mindset » dans le Gestionnaire des tâches) : une copie de
# llama-server.exe dans son dossier, refaite quand llama-server.exe change, et llama-server.exe si la copie est
# impossible. Essai avec un faux llama-server (un petit programme qui attend, compilé ici : rien n'est téléchargé), lancé
# et arrêté par Start-Ia et Stop-Ia comme dans Mindcare.
$iaCopie = Join-Path $test 'ia-copie'
Remove-Item -LiteralPath $iaCopie -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $iaCopie | Out-Null
$csc = Join-Path $env:windir 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
function New-FauxServeur([int]$Attente) {
    $src = Join-Path $iaCopie 'faux-serveur.cs'
    [IO.File]::WriteAllText($src, "static class F { static void Main() { System.Threading.Thread.Sleep($Attente); } }")
    $null = & $csc /nologo /target:winexe "/out:$(Join-Path $iaCopie 'llama-server.exe')" $src
}
function Get-InfosProcessus([int]$Id) { Get-CimInstance Win32_Process -Filter "ProcessId = $Id" -ErrorAction SilentlyContinue }
if (-not (Get-Command Write-Log -ErrorAction SilentlyContinue)) { function Write-Log([string]$Texte) { $script:JournalIa += @($Texte) } }
$script:JournalIa = @()
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $AppDir 'modules\natif.cs'), [Text.Encoding]::UTF8))
$IaServeurSauve = $IaServeur; $IaModeleSauve = $IaModele
$IaServeur = Join-Path $iaCopie 'llama-server.exe'; $IaModele = Join-Path $iaCopie 'mindcare-test.gguf'
Set-Content -LiteralPath $IaModele -Value 'faux'
$copie = Join-Path $iaCopie 'Mindset.exe'
$essais = @()
# 1. Une installation sans copie (comme toutes celles d'avant) : la copie est faite, l'assistant tourne sous Mindset.exe
New-FauxServeur 60000
Start-Ia
$pr = if ($script:Ia.Process) { Get-InfosProcessus $script:Ia.Process.Id }
$essais += [pscustomobject]@{ Nom = 'copie faite'; Bon = ($pr -and $pr.Name -eq 'Mindset.exe' -and $pr.ExecutablePath -ieq $copie -and (Get-FileHash $copie).Hash -eq (Get-FileHash $IaServeur).Hash); Vu = "$($pr.Name) $($pr.ExecutablePath)" }
$idIa = if ($script:Ia.Process) { $script:Ia.Process.Id } else { 0 }
Stop-Ia
$essais += [pscustomobject]@{ Nom = 'arrêté par Stop-Ia'; Bon = ($idIa -and -not (Get-Process -Id $idIa -ErrorAction SilentlyContinue)); Vu = "processus $idIa" }
# 2. llama-server.exe mis à jour : la copie est refaite
New-FauxServeur 60001
Start-Ia
$pr = if ($script:Ia.Process) { Get-InfosProcessus $script:Ia.Process.Id }
$essais += [pscustomobject]@{ Nom = 'copie refaite'; Bon = ($pr -and $pr.ExecutablePath -ieq $copie -and (Get-FileHash $copie).Hash -eq (Get-FileHash $IaServeur).Hash); Vu = "$($pr.Name) $($pr.ExecutablePath)" }
Stop-Ia
# 3. Copie impossible (fichier verrouillé) : llama-server.exe, comme avant
New-FauxServeur 60002
$verrou = [IO.File]::Open($copie, 'Open', 'Read', 'None')
try {
    Start-Ia
    $pr = if ($script:Ia.Process) { Get-InfosProcessus $script:Ia.Process.Id }
    $essais += [pscustomobject]@{ Nom = 'secours llama-server.exe'; Bon = ($pr -and $pr.Name -eq 'llama-server.exe' -and ($script:JournalIa -join ' ') -match 'copie Mindset\.exe impossible'); Vu = "$($pr.Name)" }
    Stop-Ia
} finally { $verrou.Dispose() }
$IaServeur = $IaServeurSauve; $IaModele = $IaModeleSauve
$rates = @($essais | Where-Object { -not $_.Bon } | ForEach-Object { "$($_.Nom) : $($_.Vu)" })
Write-Resultat "assistant : lancé sous le nom Mindset.exe (copie de llama-server.exe), sinon llama-server.exe" (-not $rates -and $essais.Count -eq 4) "($($rates -join ' ; '))"
# Aucun envoi automatique sur Internet (CODE_SIGNING.md, confidentialité) : seulement les adresses connues, et le
# téléchargement de l'assistant ne part que d'un clic. Un nouvel envoi doit être ajouté ici ET dans CODE_SIGNING.md.
$connues = 'https://github.com/ggml-org/llama.cpp/releases/download/', 'https://github.com/JordanJacquot/Mindcare/releases/download/', 'https://github.com/JordanJacquot/Mindset/releases/latest', 'http://127.0.0.1:'
$reseau = @()
foreach ($f in Get-ChildItem -LiteralPath $AppDir -Recurse -File | Where-Object { $_.Extension -in '.ps1', '.cs' }) {
    $n = 0
    foreach ($l in [IO.File]::ReadAllLines($f.FullName, [Text.Encoding]::UTF8)) {
        $n++
        foreach ($m in [regex]::Matches($l, 'https?://[^\s''"]+')) {
            if ($m.Value -match '^http://schemas\.microsoft\.com/') { continue }   # (espaces de noms XAML, pas des connexions)
            if (-not @($connues | Where-Object { $m.Value.StartsWith($_) }).Count) { $reseau += "$($f.Name) ligne $n : $($m.Value)" }
        }
        if ($l -match 'WebClient|Net\.Http\.|HttpClient|BitsTransfer|Sockets\.(TcpClient|UdpClient)|Resolve-DnsName|Test-Connection|Test-NetConnection|Net\.NetworkInformation') { $reseau += "$($f.Name) ligne $n : $($l.Trim())" }
        if ($l -match 'Start-IaInstall' -and $l -notmatch '^\s*function ' -and $l -notmatch 'Add_Click') { $reseau += "$($f.Name) ligne $n : installation de l'assistant hors d'un clic" }
    }
}
Write-Resultat 'aucun envoi automatique sur Internet (adresses connues, assistant sur un clic)' (-not $reseau) "($($reseau -join ' ; '))"
# Le nom dans la barre des tâches : la fenêtre porte l'identité de Mindcare (le raccourci du menu Démarrer de même
# identité, écrit par Mindset, donne le nom « Mindcare » au bouton ; sans lui, Windows afficherait « Mindset », le nom de
# son hôte), et l'épingle relance son programme avec le nom « Mindcare » et son icône. Le vrai code d'interface.ps1
# (Get-LaunchCommand et l'ouverture de la fenêtre) sur une fenêtre de test hors de l'écran, jamais montrée ; ses
# propriétés sont relues avec SHGetPropertyStoreForWindow.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MCTestFenetre
{
    [StructLayout(LayoutKind.Sequential, Pack = 4)] struct PK { public Guid f; public uint p; }
    [StructLayout(LayoutKind.Explicit)] struct PV { [FieldOffset(0)] public ushort vt; [FieldOffset(8)] public IntPtr p; [FieldOffset(16)] public long x; }
    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IPropertyStore { void GetCount(out uint c); void GetAt(uint i, out PK k); void GetValue(ref PK k, out PV v); }
    [DllImport("ole32.dll")] static extern int PropVariantClear(ref PV v);
    [DllImport("shell32.dll", PreserveSig = false)] static extern void SHGetPropertyStoreForWindow(IntPtr h, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out IPropertyStore s);
    // { identité, commande de relance, icône de relance, nom de relance }
    public static string[] Lire(IntPtr h)
    {
        Guid iid = typeof(IPropertyStore).GUID; IPropertyStore s;
        SHGetPropertyStoreForWindow(h, ref iid, out s);
        try
        {
            string[] r = new string[4]; uint[] pids = { 5, 2, 3, 4 };
            for (int i = 0; i < 4; i++)
            {
                PK k = new PK { f = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"), p = pids[i] };
                PV v; s.GetValue(ref k, out v);
                try { r[i] = v.vt == 31 && v.p != IntPtr.Zero ? Marshal.PtrToStringUni(v.p) : ""; } finally { PropVariantClear(ref v); }
            }
            return r;
        }
        finally { Marshal.ReleaseComObject(s); }
    }
}
'@
$tok = $null; $errI = $null
$astI = [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $AppDir 'modules\interface.ps1'), [ref]$tok, [ref]$errI)
Invoke-Expression ($astI.Find({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Get-LaunchCommand' }, $true).Extent.Text)
$ouverture = $astI.Find({ param($n) $n -is [System.Management.Automation.Language.InvokeMemberExpressionAst] -and $n.Member.Value -eq 'Add_SourceInitialized' }, $true).Arguments[0].ScriptBlock.GetScriptBlock()
$AppNomSauve = $AppNom; $AppExeSauve = $AppExe; $MainScriptSauve = $MainScript; $icoSauve = $ico; $WindowSauve = $Window
$AppNom = [regex]::Match([IO.File]::ReadAllText($main, [Text.Encoding]::UTF8), "(?m)^\`$AppNom\s*=\s*'([^']+)'").Groups[1].Value
$ico = Join-Path $AppDir 'Mindcare.ico'
$MainScript = $main
$fenetreEssais = @()
foreach ($avecProgramme in $true, $false) {
    $AppExe = Join-Path $test $(if ($avecProgramme) { 'fenetre\Mindset.exe' } else { 'fenetre\absent.exe' })
    if ($avecProgramme) { New-Item -ItemType Directory -Force -Path (Split-Path $AppExe) | Out-Null; [IO.File]::WriteAllText($AppExe, 'programme') }
    $Window = New-Object System.Windows.Window
    $Window.WindowStartupLocation = 'Manual'; $Window.Left = -10000; $Window.Top = 0; $Window.Width = 200; $Window.Height = 100
    $Window.ShowActivated = $false; $Window.ShowInTaskbar = $false
    $h = (New-Object System.Windows.Interop.WindowInteropHelper $Window).EnsureHandle()
    & $ouverture
    $pf = [MCTestFenetre]::Lire($h)
    $Window.Close()
    $relance = if ($avecProgramme) { "`"$AppExe`"" } else { "`"$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe`" -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$main`"" }
    if ($pf[0] -cne 'JordanJacquot.Mindcare') { $fenetreEssais += "identité « $($pf[0]) »" }
    if ($pf[1] -ne $relance) { $fenetreEssais += "relance « $($pf[1]) » au lieu de « $relance »" }
    if ($pf[3] -cne 'Mindcare' -or $pf[2] -ne "$ico,0") { $fenetreEssais += "nom « $($pf[3]) », icône « $($pf[2]) »" }
}
Remove-Item -LiteralPath (Join-Path $test 'fenetre') -Recurse -Force -ErrorAction SilentlyContinue
$AppNom = $AppNomSauve; $AppExe = $AppExeSauve; $MainScript = $MainScriptSauve; $ico = $icoSauve; $Window = $WindowSauve
Write-Resultat 'barre des tâches : identité JordanJacquot.Mindcare, épingle vers Mindset.exe (PowerShell sans lui), nom « Mindcare » et son icône' (-not $errI -and -not $fenetreEssais) "($($fenetreEssais -join ' ; '))"
if (-not $SansFenetre) {
    # L'assistant (ia.ps1) : sa carte « J'ai compris », puis une enquête où la phrase a déjà répondu à des questions
    $faux = '{"panne":"seteint","reponses":{"type":1,"maison":0}}'
    $s = Test-Capture 'ia-comprise' @{ MINDCARE_TEST_RECHERCHE = 'mon pc s''éteint d''un coup quand je joue'; MINDCARE_TEST_IA = $faux }
    Write-Resultat "assistant : la carte « J'ai compris » (journal)" ((Get-Journal $s) -match 'Assistant : .+ -> seteint') '(pas de proposition dans le journal)'
    $s = Test-Capture 'ia-enquete' @{ MINDCARE_TEST_RECHERCHE = 'mon pc fixe s''éteint d''un coup, et les lumières de la maison aussi'; MINDCARE_TEST_IA = $faux; MINDCARE_TEST_IA_CHOISIR = '1' }
    Write-Resultat "assistant : l'enquête commence après les questions déjà réglées par la phrase" ((Get-Journal $s) -match 'Enquête : seteint \(déjà su') '(pas de réponses pré-remplies dans le journal)'
    # Un ancien assistant installé (v1) : l'accueil propose de le mettre à jour, et l'ancien sert en attendant
    $ancien = Join-Path $test 'donnees-ia-maj\ia'
    Remove-Item -LiteralPath (Join-Path $test 'donnees-ia-maj') -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $ancien | Out-Null
    Set-Content -LiteralPath (Join-Path $ancien 'llama-server.exe') -Value 'faux' ; Set-Content -LiteralPath (Join-Path $ancien 'mindcare-0.6b-v1-q8_0.gguf') -Value 'faux'
    $s = Test-Capture 'ia-maj' @{} -Dossier 'ia-maj'
    Write-Resultat "assistant : mise à jour proposée quand un ancien modèle est installé" ((Get-Journal $s) -match 'nouvelle version est proposée') '(pas de mise à jour proposée, ou ancien modèle non utilisé)'
    # Le vrai assistant (seulement sur le PC qui l'entraîne : le modèle et llama-server du dossier d'entraînement)
    $entr = Join-Path $env:LOCALAPPDATA 'Mindcare-entrainement'
    $vraiModele = Get-ChildItem -LiteralPath (Join-Path $entr 'modeles') -Filter 'mindcare-0.6b-*-q8_0.gguf' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1
    if ($vraiModele -and (Test-Path -LiteralPath (Join-Path $entr 'llama\llama-server.exe'))) {
        # (llama.cpp recopié dans _test : Mindcare y fait sa copie Mindset.exe, rien n'est écrit dans le dossier d'entraînement)
        $moteur = Join-Path $test 'ia-vraie-moteur'
        Remove-Item -LiteralPath $moteur -Recurse -Force -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force -Path $moteur | Out-Null
        Get-ChildItem -LiteralPath (Join-Path $entr 'llama') -File | Where-Object { $_.Extension -in '.exe', '.dll' -and $_.Name -ne 'Mindset.exe' } | Copy-Item -Destination $moteur
        $deMoteur = { @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith("$moteur\", [StringComparison]::OrdinalIgnoreCase) }) }
        $s = Test-Capture 'ia-vraie' @{ MINDCARE_TEST_RECHERCHE = 'mon ordi s''éteint tout seul d''un coup comme une coupure de courant'; MINDCARE_IA_SERVEUR = (Join-Path $moteur 'llama-server.exe'); MINDCARE_IA_MODELE = $vraiModele.FullName; MINDCARE_CAPTURE_DELAI = '25000' }
        Write-Resultat "assistant : le vrai modèle propose une panne ($($vraiModele.Name))" ((Get-Journal $s) -match 'Assistant : .+ -> \w+') "(journal : $((Get-Journal $s) -split "`n" | Select-String 'Assistant' | Select-Object -Last 2))"
        Write-Resultat "assistant : le vrai lancé sous le nom Mindset.exe" ((Get-Journal $s) -match 'Assistant lancé \(.+, Mindset\.exe\)') "(journal : $((Get-Journal $s) -split "`n" | Select-String 'Assistant lancé' | Select-Object -Last 1))"
        Start-Sleep -Seconds 1
        $restants = & $deMoteur
        Write-Resultat "assistant : arrêté à la fermeture de Mindcare" (-not $restants) "(encore lancé : $($restants.Name -join ', '))"
        if ($restants) { $restants | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue } }
    }
    Test-Capture 'enquete-son' @{ MINDCARE_TEST_PARCOURS = 'songresille:1' } | Out-Null
    Test-Capture 'enquete-commande' @{ MINDCARE_TEST_PARCOURS = 'wificoupe:1,1' } | Out-Null
    Test-Capture 'enquete-interrupteur' @{ MINDCARE_TEST_PARCOURS = 'rien:1,1' } | Out-Null
    Test-Capture 'enquete-code' @{ MINDCARE_TEST_PARCOURS = 'ecranbleu:0,1' } | Out-Null
    Test-Capture 'recherche' @{ MINDCARE_TEST_RECHERCHE = 'il redémarre' } | Out-Null
    Test-Capture 'recherche-rien' @{ MINDCARE_TEST_RECHERCHE = 'wifi' } | Out-Null
    Test-Capture 'enquete-question' @{ MINDCARE_TEST_PARCOURS = 'redemarre:1,1' } | Out-Null
    Test-Capture 'enquete-test' @{ MINDCARE_TEST_PARCOURS = 'redemarre:1,1,1,1,1,1' } | Out-Null
    Test-Capture 'enquete-image' @{ MINDCARE_TEST_PARCOURS = 'seteint:1,0,1' } | Out-Null
    $s = Test-Capture 'verdict-trouve' @{ MINDCARE_TEST_PARCOURS = 'redemarre:1,1,1,1,1,1,0' }
    Write-Resultat 'réponse trouvée (journal)' ((Get-Journal $s) -match 'Réponse redemarre : trouve memoire') '(pas de « trouve memoire » dans le journal)'
    Test-Capture 'verdict-elimination' @{ MINDCARE_TEST_PARCOURS = 'redemarre:1,1,1,1,1,1,1' } | Out-Null
    Test-Capture 'verdict-urgent' @{ MINDCARE_TEST_PARCOURS = 'redemarre:0' } | Out-Null
    Test-Capture 'verdict-reparateur' @{ MINDCARE_TEST_PARCOURS = 'ecranbleu:1,1,3,1,1,1' } | Out-Null
    $jx = '{"testmemoire-erreurs":{"Vu":true,"Texte":"Le test de mémoire du 18 octobre a trouvé des erreurs"},"testmemoire-ok":{"Vu":false},"majredemarrage":{"Vu":false,"TexteOk":"Aucun redémarrage pour les mises à jour ces 30 jours"}}'
    $s = Test-Capture 'verdict-journaux' @{ MINDCARE_TEST_PARCOURS = 'redemarre:1,0'; MINDCARE_TEST_JOURNAUX = $jx; MINDCARE_TEST_BARRETTES = '16 Go Corsair CMK16GX4M2B3200C16 (DIMM 1)|16 Go Corsair CMK16GX4M2B3200C16 (DIMM 2)' }
    Write-Resultat 'réponse grâce à ce que Windows a noté (journal)' ((Get-Journal $s) -match 'Réponse redemarre : trouve memoire') '(pas de « trouve memoire » dans le journal)'
    Test-Capture 'verdict-rapport' @{ MINDCARE_TEST_PARCOURS = 'redemarre:1,1,1,1,1,1,0'; MINDCARE_TEST_REPARATEUR = '1' } | Out-Null
    Test-Capture 'images' @{ MINDCARE_TEST_GALERIE = '1' } | Out-Null

    # 6. Les enquêtes gardées : 10 au plus, la dernière en tête, la carte de Mindset, le rapport, la reprise
    $hist = Join-Path $test 'donnees-historique'
    if (Test-Path -LiteralPath $hist) { Remove-Item -LiteralPath $hist -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $hist | Out-Null
    $vieux = @(for ($i = 1; $i -le 10; $i++) { [ordered]@{ Id = "vieux-$i"; Panne = 'fige'; Choix = @(1); Debut = "2026-01-$('{0:d2}' -f $i)T10:00:00"; Maj = "2026-01-$('{0:d2}' -f $i)T10:00:00"; Fini = $false; Suspect = ''; Titre = '' } })
    [IO.File]::WriteAllText((Join-Path $hist 'diagnostics.json'), (ConvertTo-Json -InputObject ([ordered]@{ Version = 2; Diagnostics = $vieux }) -Depth 5), (New-Object Text.UTF8Encoding($false)))
    function Read-Etat { try { [IO.File]::ReadAllText((Join-Path $hist 'etat.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json } catch { $null } }
    function Read-Gardes { try { @(([IO.File]::ReadAllText((Join-Path $hist 'diagnostics.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json).Diagnostics) } catch { @() } }
    $s = Test-Capture 'historique-reponse' @{ MINDCARE_TEST_PARCOURS = 'redemarre:1,1,1,1,1,1,0'; MINDCARE_TEST_RESUME = '1' } -Dossier 'historique'
    $g = Read-Gardes; $et = Read-Etat
    Write-Resultat 'enquête finie gardée (la plus récente en tête, 10 au plus)' ($g.Count -eq 10 -and $g[0].Panne -eq 'redemarre' -and $g[0].Fini -and $g[0].Suspect -eq 'memoire' -and $g[1].Id -eq 'vieux-1') "($($g.Count) gardées, en tête : $($g[0].Panne))"
    Write-Resultat 'carte de Mindset : la dernière réponse' ($et -and @($et.Lignes)[0].Texte -eq 'Il redémarre tout seul : Une barrette de mémoire (RAM) est abîmée' -and @($et.Lignes)[0].Etat -eq 'attention' -and @($et.Lignes | Where-Object { $_.Texte.Length -gt 80 }).Count -eq 0) "($(@($et.Lignes)[0].Texte))"
    $resume = @(Get-ChildItem -LiteralPath $hist -Filter 'Mindcare, diagnostic du *.txt')
    $rt = if ($resume) { [IO.File]::ReadAllText($resume[0].FullName, [Text.Encoding]::UTF8) } else { '' }
    Write-Resultat 'rapport pour le réparateur' ($rt -match 'Réponse : Une barrette de mémoire' -and $rt -match 'Comment on le sait :' -and $rt -match 'Le test de mémoire de Windows a trouvé des erreurs' -and $rt -match 'Écartés :' -and $rt -match 'Les réponses :' -and $rt.IndexOfAny([char[]](0x2013, 0x2014)) -lt 0) "($($resume.Count) fichier)"
    Test-Capture 'historique-pas-finie' @{ MINDCARE_TEST_PARCOURS = 'seteint:1,1' } -Dossier 'historique' | Out-Null
    $g = Read-Gardes; $et = Read-Etat
    Write-Resultat 'enquête pas finie gardée' ($g.Count -eq 10 -and $g[0].Panne -eq 'seteint' -and -not $g[0].Fini -and (@($g[0].Choix) -join ',') -eq '1,1' -and $g[1].Panne -eq 'redemarre') "(en tête : $($g[0].Panne), $(@($g[0].Choix) -join ','))"
    Write-Resultat 'carte de Mindset : pas finie' ($et -and @($et.Lignes)[0].Texte -eq "Il s'éteint d'un coup : enquête pas finie") "($(@($et.Lignes)[0].Texte))"
    Test-Capture 'historique-accueil' @{} -Dossier 'historique' | Out-Null
    $s = Test-Capture 'historique-reprise' @{ MINDCARE_TEST_REPRENDRE = '1' } -Dossier 'historique'
    $g = Read-Gardes
    Write-Resultat 'reprise de l''enquête pas finie' ((Get-Journal $s) -match "Reprise de l'enquête .* \(seteint\)" -and $g[0].Panne -eq 'seteint' -and $g.Count -eq 10) '(pas de reprise dans le journal)'

    # 7. Lancée cachée par Mindset, puis « Tout quitter » : elle doit quitter d'elle-même, vite
    function Test-Quitter([string]$Nom, [switch]$Programme, [switch]$ViaRelais) {
        $s = Start-Scenario $Nom @{ MINDCARE_TEST_DEMARRAGE = '1' } -Programme:$Programme -ViaRelais:$ViaRelais
        $p = $s.P
        if ($ViaRelais) {
            # Le relais Mindcare.exe lance Mindset.exe de son dossier puis se ferme aussitôt ; c'est Mindset.exe qui tourne
            $relaisParti = $p.WaitForExit(10000)
            $enfant = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($p.Id)" -ErrorAction SilentlyContinue | Where-Object { $_.ExecutablePath -ieq $exe })
            Write-Resultat "$Nom : le relais se ferme et lance Mindset.exe" ($relaisParti -and $enfant.Count -eq 1) "(relais fermé : $relaisParti ; lancés : $($enfant.Count))"
            if (-not $relaisParti) { $p.Kill() }
            if ($enfant.Count -ne 1) { return }
            $p = Get-Process -Id $enfant[0].ProcessId -ErrorAction SilentlyContinue
            if (-not $p) { Write-Resultat $Nom $false '(Mindset.exe déjà fermé)'; return }
        }
        if ($Programme -or $ViaRelais) {
            # Le nom que voit le Gestionnaire des tâches : Mindset.exe, du dossier de Mindcare
            $info = Get-CimInstance Win32_Process -Filter "ProcessId = $($p.Id)" -ErrorAction SilentlyContinue
            Write-Resultat "$Nom : le processus s'appelle Mindset.exe" ($info -and $info.Name -eq 'Mindset.exe' -and $info.ExecutablePath -ieq $exe) "($($info.Name) $($info.ExecutablePath))"
        }
        $fin = (Get-Date).AddSeconds($Delai)
        while ((Get-Date) -lt $fin -and (Get-Journal $s) -notmatch 'Démarrage ') { Start-Sleep -Milliseconds 300 }
        Start-Sleep -Seconds 2
        [IO.File]::WriteAllText((Join-Path $s.Donnees 'quitter.demande'), 'test')
        $parti = $p.WaitForExit(8000)
        if (-not $parti) { $p.Kill() }
        Write-Resultat $Nom ($parti -and (Get-Journal $s) -match 'Fermeture demandée par Mindset') $(if (-not $parti) { '(encore ouverte 8 s après la demande : Mindset la fermerait de force)' } else { '(pas de trace dans le journal)' })
    }
    Test-Quitter 'quitter (script)'
    # 8. Le vrai programme : son processus doit disparaître (sinon Mindset ne pourrait plus la rouvrir), lancé
    # directement (nouvelles versions de Mindset) ou par le relais Mindcare.exe (anciennes versions, raccourcis)
    if ((Test-Path -LiteralPath $exe) -and (Test-Path -LiteralPath $relais)) {
        Test-Quitter 'quitter (programme)' -Programme
        Test-Quitter 'quitter (relais)' -ViaRelais
    } else { Write-Host "(pas de $exe ou de $relais : lance construire.ps1 pour tester le programme)" -ForegroundColor Yellow }
}

if ($script:Ok) { Write-Host "`nTout est bon. Captures : $test" -ForegroundColor Green; exit 0 }
Write-Host "`nDes tests ont échoué. Captures et erreurs : $test" -ForegroundColor Red
exit 1
