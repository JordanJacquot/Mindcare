# Mindcare : les tests du diagnostic (ce que Windows a noté, les textes, la carte de Mindset, le journal et le rapport).
# Mindcare est chargée comme Mindcare.ps1 le fait, mais sa fenêtre ne s'affiche jamais, et tout se fait sur de fausses
# données : rien n'est lu ni modifié sur le PC (ni ses journaux, ni ses disques), rien ne va sur Internet.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File .\outils\tester-diagnostic.ps1
#
# Code de sortie : 0 si tout est bon, 1 sinon (la CI de GitHub, .github\workflows\tests.yml).

$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$test   = Join-Path $racine '_test\diagnostic'
if (Test-Path -LiteralPath $test) { Remove-Item -LiteralPath $test -Recurse -Force }
New-Item -ItemType Directory -Force -Path $test | Out-Null
$script:Ok = $true
function Write-Resultat([string]$Nom, [bool]$Bon, [string]$Detail = '') {
    if ($Bon) { Write-Host "OK     $Nom" -ForegroundColor Green } else { Write-Host "ÉCHEC  $Nom $Detail" -ForegroundColor Red; $script:Ok = $false }
}

# ---------------------------------------------------------------------------
# Mindcare chargée sans fenêtre à l'écran (comme Mindcare.ps1, sans evenements.ps1 : rien n'est lancé, ni l'assistant,
# ni la lecture des journaux de Windows)
# ---------------------------------------------------------------------------
Get-ChildItem env: | Where-Object { $_.Name -like 'MINDCARE_*' } | ForEach-Object { Remove-Item "env:$($_.Name)" }
$env:MINDCARE_TEST = '1'; $env:MINDCARE_DONNEES = $test
$EnTest = $true
$AppVersion = 'test'; $AppNom = 'Mindcare'; $AppId = 'mindcare'; $AppDescription = 'Test'
$AppDir = Join-Path $racine 'Mindcare\fichiers'
$ModulesDir = Join-Path $AppDir 'modules'
$MainScript = Join-Path $AppDir 'Mindcare.ps1'
$AppExe = Join-Path $racine 'Mindcare\Mindset.exe'
Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase, System.Drawing, System.Windows.Forms
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $ModulesDir 'natif.cs'), [Text.Encoding]::UTF8))
foreach ($m in 'mindset', 'donnees', 'themes', 'interface', 'systeme', 'enquete', 'images', 'journaux', 'historique', 'ia', 'dictee', 'contenu') { . (Join-Path $ModulesDir "$m.ps1") }
$script:Donnees = Read-Enquetes
$Journal = Join-Path $test 'journal.txt'
function Get-Journal { if (Test-Path -LiteralPath $Journal) { [IO.File]::ReadAllText($Journal, [Text.Encoding]::UTF8) } else { '' } }

# Les textes affichés dans la fenêtre (sans l'afficher)
function Get-TextesEcran($El = $ui.Contenu) {
    if ($El -is [System.Windows.Controls.TextBlock]) { $El.Text }
    if ($El -is [System.Windows.Controls.Panel]) { foreach ($c in $El.Children) { Get-TextesEcran $c } }
    elseif ($El -is [System.Windows.Controls.Decorator]) { if ($El.Child) { Get-TextesEcran $El.Child } }
    elseif ($El -is [System.Windows.Controls.ContentControl]) { if ($El.Content -is [System.Windows.UIElement]) { Get-TextesEcran $El.Content } else { [string]$El.Content } }
}
function Get-Bouton([string]$Texte, $El = $ui.Contenu) {
    if ($El -is [System.Windows.Controls.Button] -and [string]$El.Content -eq $Texte) { return $El }
    $enfants = if ($El -is [System.Windows.Controls.Panel]) { @($El.Children) } elseif ($El -is [System.Windows.Controls.Decorator]) { @($El.Child) } elseif ($El -is [System.Windows.Controls.ContentControl] -and $El.Content -is [System.Windows.UIElement]) { @($El.Content) } else { @() }
    foreach ($c in $enfants) { if ($c) { $b = Get-Bouton $Texte $c; if ($b) { return $b } } }
}
function Invoke-Clic([string]$Texte) {
    $b = Get-Bouton $Texte
    if (-not $b) { throw "Bouton « $Texte » absent" }
    $b.RaiseEvent((New-Object System.Windows.RoutedEventArgs ([System.Windows.Controls.Primitives.ButtonBase]::ClickEvent)))
}
# Laisse tourner la fenêtre (ses minuteries) jusqu'à ce que Fini soit vrai, MaxMs au plus
function Wait-Fenetre([int]$MaxMs, [scriptblock]$Fini = { $false }) {
    $chrono = [Diagnostics.Stopwatch]::StartNew()
    while ($chrono.ElapsedMilliseconds -lt $MaxMs -and -not (& $Fini)) {
        $frame = New-Object System.Windows.Threading.DispatcherFrame
        $t = New-Object System.Windows.Threading.DispatcherTimer; $t.Interval = [TimeSpan]::FromMilliseconds(100)
        $t.Add_Tick({ param($s) $s.Stop(); $frame.Continue = $false }.GetNewClosure())
        $t.Start(); [System.Windows.Threading.Dispatcher]::PushFrame($frame)
    }
}
# Une nouvelle enquête, comme un clic sur une panne
function Start-EnqueteTest([string]$Panne, [int[]]$Choix) {
    $script:PanneEnCours = Get-Panne $script:Donnees $Panne; $script:ChoixFaits = @($Choix); $script:DiagId = New-DiagId
    $script:PreRempli = $null; $script:ReparateurOuvert = $false
}
# Une lecture des journaux toute neuve : une fausse lecture (une pause, puis des indices inventés), jamais les vrais
function Reset-Journaux([int]$PauseMs) {
    Stop-LectureJournaux
    $script:Journaux = [hashtable]::Synchronized(@{}); $script:JournauxTache = $null; $script:JournauxAbandon = $null; $script:JournauxNonLus = $null; $script:JournauxNotes = $null
    if ($PauseMs) { Start-LectureJournaux "Start-Sleep -Milliseconds $PauseMs; @{ 'testmemoire-erreurs' = [pscustomobject]@{ Vu = `$true; Texte = 'Le test de mémoire du 3 octobre a trouvé des erreurs'; TexteOk = ''; Affiche = `$true } }" }
}

# ---------------------------------------------------------------------------
# 7. Ce que Windows a noté est lu en arrière-plan : la fenêtre ne se fige jamais, et l'enquête ne change pas en route
# ---------------------------------------------------------------------------
# a. Lecture lente (4 s) : l'enquête attend sans figer la fenêtre, puis s'en sert (le test de mémoire raté : trouvé)
Reset-Journaux 4000
Start-EnqueteTest 'redemarre' 1, 0   # (« sur ce PC » : ce que Windows a noté compte)
$chrono = [Diagnostics.Stopwatch]::StartNew(); Show-Etape; $duree = $chrono.ElapsedMilliseconds
$attente = @(Get-TextesEcran) -contains 'Mindcare regarde ce que Windows a noté...'
Write-Resultat "journaux lents : la fenêtre ne se fige pas ($duree ms) et le dit" ($duree -lt 2500 -and $attente) "($duree ms ; écran d'attente : $attente)"
Wait-Fenetre 15000 { @(Get-TextesEcran) -contains 'TROUVÉ' }
$g = $script:Historique[0]
Write-Resultat 'journaux lents : l''enquête reprend toute seule avec ce que Windows a noté' (@(Get-TextesEcran) -contains 'TROUVÉ' -and $g.Suspect -eq 'memoire') "(écran : $(@(Get-TextesEcran | Select-Object -First 3) -join ' / '))"

# b. Lecture trop longue : abandonnée au bout du délai, l'enquête continue sans elle et ne change plus ensuite
Reset-Journaux 3500
$AttenteJournauxMax = 1
Start-EnqueteTest 'redemarre' 1, 0
Show-Etape
Wait-Fenetre 6000 { @(Get-TextesEcran) -notcontains 'Mindcare regarde ce que Windows a noté...' }
$avant = @(Get-TextesEcran) -join '|'
$sansEux = $avant -notmatch 'TROUVÉ' -and $avant -notmatch 'Mindcare regarde'
Wait-Fenetre 8000 { $script:JournauxTache.Res.IsCompleted }
Wait-Fenetre 500
Show-Etape
$apres = @(Get-TextesEcran) -join '|'
Write-Resultat 'journaux trop longs : l''enquête continue sans eux, puis ne change plus' ($sansEux -and $script:JournauxTache.Res.IsCompleted -and $apres -eq $avant -and (Get-Indices).Count -eq 0) "(sans eux : $sansEux ; même écran ensuite : $($apres -eq $avant))"
Write-Resultat 'journaux trop longs : la cause est au journal' ((Get-Journal) -match 'Journaux de Windows non lus : trop longs') ''
$AttenteJournauxMax = 30

# c. Partie ailleurs pendant l'attente (Accueil) : la fin de la lecture ne la ramène pas dans l'enquête
Reset-Journaux 1500
Start-EnqueteTest 'redemarre' 1, 0
Show-Etape
Invoke-Clic 'Accueil'
$accueil = @(Get-TextesEcran) -join '|'
Wait-Fenetre 4000 { $script:JournauxTache.Res.IsCompleted }
Wait-Fenetre 600
Write-Resultat 'journaux lents : partie à l''accueil entre-temps, elle y reste' ((@(Get-TextesEcran) -join '|') -eq $accueil -and $accueil -notmatch 'Mindcare regarde') ''
Reset-Journaux 0

# ---------------------------------------------------------------------------
# 8. Les textes de ce que Windows a noté : sur un faux PC (de faux événements, de faux disques, rien du vrai PC)
# ---------------------------------------------------------------------------
# (de vrais objets .NET, comme ceux de Get-WinEvent : un [pscustomobject] seul n'a pas de .Count dans PowerShell 5.1)
Add-Type -TypeDefinition 'public class FauxValeur { public object Value; } public class FauxEvenement { public string ProviderName; public int Id; public System.DateTime TimeCreated; public string Message; public FauxValeur[] Properties; }'
function New-FauxEvenement([string]$Source, [int]$Id, [string]$Message, [string]$Valeur) {
    $e = New-Object FauxEvenement; $e.ProviderName = $Source; $e.Id = $Id; $e.TimeCreated = (Get-Date).AddDays(-2); $e.Message = $Message
    $v = New-Object FauxValeur; $v.Value = $Valeur; $e.Properties = @($v)
    $e
}
# (Sonde : la date du plus ancien événement du journal Système, ou « illisible » : le journal ne se lit pas)
function Get-IndicesFauxPC($Evenements, $Sonde = (Get-Date).AddDays(-400)) {
    & {
        function Get-WinEvent {
            [CmdletBinding()] param([string]$LogName, [int]$MaxEvents, [switch]$Oldest, $FilterHashtable)
            if ($FilterHashtable -or $LogName -ne 'System' -or -not $Oldest) { throw 'Le test a voulu lire les vrais journaux de Windows' }   # (jamais le vrai PC)
            if ($Sonde -eq 'illisible') { throw 'Accès refusé (faux journal illisible)' }
            $e = New-Object FauxEvenement; $e.TimeCreated = $Sonde; $e
        }
        function Get-EvenementsSysteme([hashtable]$Filtre, [int]$Max = 200) { @($Evenements | Where-Object { $_.ProviderName -eq $Filtre.ProviderName -and @($Filtre.Id) -contains $_.Id }) }
        function Get-Disk { [pscustomobject]@{ Number = 0; BusType = 'NVMe' }; [pscustomobject]@{ Number = 2; BusType = 'USB' } }
        function Get-Partition { [pscustomobject]@{ DiskNumber = 0; DriveLetter = [char]'C' }; [pscustomobject]@{ DiskNumber = 2; DriveLetter = [char]'E' } }
        function Get-PhysicalDisk { [pscustomobject]@{ FriendlyName = 'Clé USB'; BusType = 'USB'; HealthStatus = 'Warning' }; [pscustomobject]@{ FriendlyName = 'SSD du PC'; BusType = 'NVMe'; HealthStatus = 'Healthy' } }
        function Get-PSDrive { [pscustomobject]@{ Free = 200GB; Used = 300GB } }
        function Get-CimInstance { [pscustomobject]@{ LastBootUpTime = (Get-Date).AddDays(-2) } }
        $racineWindows = $env:SystemRoot
        try { $env:SystemRoot = Join-Path $test 'pas-de-windows'; Get-IndicesWindows }   # (pas de rapport de batterie du vrai PC)
        finally { $env:SystemRoot = $racineWindows }
    }
}
$usb = @(
    (New-FauxEvenement 'disk' 7 'Le périphérique \Device\Harddisk2\DR2 a un bloc défectueux.' '\Device\Harddisk2\DR2')
    (New-FauxEvenement 'disk' 153 "L'opération d'E/S à l'adresse de bloc logique 0x1000 pour le disque 2 (nom PDO : \Device\00000039) a été retentée." '\Device\Harddisk2\DR2')
    (New-FauxEvenement 'Ntfs' 55 'Une altération a été découverte dans la structure du système de fichiers sur le volume E:.' 'E:')
    (New-FauxEvenement 'Microsoft-Windows-Kernel-Processor-Power' 37 'La vitesse du processeur 0 est limitée par le microprogramme du système.' '0')
)
$i = Get-IndicesFauxPC $usb
Write-Resultat 'textes : le processeur ralenti n''est pas mis sur le dos de la chaleur seule' ($i.freinage.Vu -and $i.freinage.Texte -match "économiser l'énergie" -and $i.freinage.Texte -notmatch '\(souvent la chaleur\)') "($($i.freinage.Texte))"
Write-Resultat 'textes : les erreurs d''une clé USB ne sont pas celles du disque du PC' (-not $i.disque.Vu -and $i.disque.Texte -notmatch 'Clé USB') "($($i.disque.Texte))"
$i = Get-IndicesFauxPC ($usb + (New-FauxEvenement 'disk' 7 'Le périphérique \Device\Harddisk0\DR0 a un bloc défectueux.' '\Device\Harddisk0\DR0'))
Write-Resultat 'textes : une erreur du disque du PC compte' ($i.disque.Vu -and $i.disque.Texte -match 'Le disque du PC a signalé 1 erreur ') "($($i.disque.Texte))"

# Le journal Système illisible (service arrêté, stratégie, journal abîmé) : avec un filtre, Windows répond « aucun
# événement », comme un PC sans souci. Jamais pris pour une bonne nouvelle : aucun indice du journal, rien d'écarté.
$i = Get-IndicesFauxPC @() 'illisible'
Write-Resultat 'journal illisible : aucun indice tiré du journal, et la raison notée' (-not $i.ContainsKey('majredemarrage') -and -not $i.ContainsKey('ecransbleus') -and -not $i.ContainsKey('coupures') -and -not $i.ContainsKey('disque') -and $i.journal.TexteOk -match "n'a pas pu être lu" -and $i.ContainsKey('placelibre')) "(clés : $(@($i.Keys) -join ', '))"
$e = Get-Enquete $script:Donnees (Get-Panne $script:Donnees 'redemarre') @(1, 0, 0) $i
Write-Resultat 'journal illisible : la piste « mises à jour » n''est pas écartée' (@($e.Ecartes | ForEach-Object { $_.Id }) -notcontains 'majnuit') "(écartés : $(@($e.Ecartes | ForEach-Object { $_.Id }) -join ', '))"
# (contre-épreuve : ce que faisait l'ancien code, un « aucun redémarrage » sans preuve, écarte bien la piste)
$ancien = @{ majredemarrage = [pscustomobject]@{ Vu = $false; Texte = ''; TexteOk = 'Aucun redémarrage pour les mises à jour ces 30 jours'; Affiche = $true } }
$e = Get-Enquete $script:Donnees (Get-Panne $script:Donnees 'redemarre') @(1, 0, 0) $ancien
Write-Resultat 'journal illisible (contre-épreuve) : l''ancien « aucun redémarrage » écartait la piste' (@($e.Ecartes | ForEach-Object { $_.Id }) -contains 'majnuit') ''
$sauveIndices = $script:Journaux.Indices; $sauveTache = $script:JournauxTache
$script:JournauxTache = @{ Res = [pscustomobject]@{ IsCompleted = $true } }; $script:Journaux.Indices = $i
$e = Get-Enquete $script:Donnees (Get-Panne $script:Donnees 'redemarre') @(1, 0, 0) $i
$rt = Get-ResumeTexte (Get-Panne $script:Donnees 'redemarre') $e 'Essai'
$script:Journaux.Indices = $sauveIndices; $script:JournauxTache = $sauveTache
Write-Resultat 'journal illisible : écrit dans le rapport pour le réparateur' ($rt -match "À savoir : Le journal de Windows n'a pas pu être lu") ''
# Le journal ne remonte qu'à 10 jours : « depuis le ... » au lieu de « ces 30 jours », et ce qu'il n'a pas vu n'écarte rien
$i = Get-IndicesFauxPC @() (Get-Date).AddDays(-10)
Write-Resultat 'journal court : « depuis le » au lieu de « ces 30 jours »' ($i.ecransbleus.TexteOk -match '^Aucun écran bleu depuis le ' -and $i.majredemarrage.Partiel -and $i.journal.TexteOk -match "ne remonte qu'au") "($($i.ecransbleus.TexteOk))"
$e = Get-Enquete $script:Donnees (Get-Panne $script:Donnees 'redemarre') @(1, 0, 0) $i
Write-Resultat 'journal court : la piste « mises à jour » n''est pas écartée, le texte reste' (@($e.Ecartes | ForEach-Object { $_.Id }) -notcontains 'majnuit' -and @($e.Auto | Where-Object { $_.Texte -match 'mises à jour depuis le' }).Count -eq 1) "(écartés : $(@($e.Ecartes | ForEach-Object { $_.Id }) -join ', '))"
# (contre-épreuve : un journal assez long qui n'a rien vu écarte bien la piste, comme avant)
$i = Get-IndicesFauxPC @()
$e = Get-Enquete $script:Donnees (Get-Panne $script:Donnees 'redemarre') @(1, 0, 0) $i
Write-Resultat 'journal de plus de 30 jours (contre-épreuve) : rien vu, la piste est écartée' (@($e.Ecartes | ForEach-Object { $_.Id }) -contains 'majnuit' -and $i.majredemarrage.TexteOk -match 'ces 30 jours' -and -not $i.ContainsKey('journal')) ''

# Le rapport : son titre dit bien ce qu'il contient (pas seulement 30 jours : aussi le test de mémoire, la batterie...)
$jx = @{ 'testmemoire-erreurs' = [pscustomobject]@{ Vu = $true; Texte = 'Le test de mémoire du 3 octobre a trouvé des erreurs'; TexteOk = ''; Affiche = $true } }
$e = Get-Enquete $script:Donnees (Get-Panne $script:Donnees 'redemarre') @(1, 0) $jx
$rt = Get-ResumeTexte (Get-Panne $script:Donnees 'redemarre') $e (Get-VerdictTitre $script:Donnees $null $e.Verdict)
Write-Resultat 'rapport : le titre de ce qui a été vérifié sur le PC' ($rt -match 'Ce que Mindcare a vérifié sur ce PC' -and $rt -notmatch '30 derniers jours' -and $rt -match 'Le test de mémoire du 3 octobre') ''

# ---------------------------------------------------------------------------
# 9. La carte de Mindset (etat.json) : la panne, puis la réponse sur sa ligne, jamais coupée au milieu de la consigne
# ---------------------------------------------------------------------------
function Get-CarteDe([string]$Panne, $Suspect, [string]$Titre) {
    $script:Historique = @([pscustomobject]@{ Id = 'carte'; Panne = $Panne; Choix = @(0); Debut = '2026-10-19T19:29:00'; Maj = '2026-10-19T19:29:00'; Fini = $true; Suspect = [string]$Suspect; Titre = $Titre })
    Get-EtatCarte
}
$c = Get-CarteDe 'pubs' $null (Get-VerdictTitre $script:Donnees $null ([pscustomobject]@{ Type = 'rien' }))
$lignes = @($c.Lignes | ForEach-Object { $_.Texte })
Write-Resultat 'carte de Mindset : « aucune piste » en entier, sans « : » en double' ($lignes -contains 'Aucune piste trouvée : montre le rapport à un réparateur' -and -not @($lignes | Where-Object { $_.Length -gt 80 -or $_ -match ' : .* : ' })) "($($lignes -join ' / '))"
# Toutes les pannes, toutes leurs réponses : chaque ligne tient dans les 80 caractères de Mindset, coupée sur un mot entier
$trop = @()
foreach ($p in $script:Donnees.Pannes) {
    $cas = @(foreach ($s in $p.Suspects) { , @($s, [string](Get-Suspect $script:Donnees $s).Trouve) })
    $cas += , @($null, (Get-VerdictTitre $script:Donnees $p ([pscustomobject]@{ Type = 'reparateur'; Restants = @($p.Suspects) })))
    foreach ($k in $cas) {
        foreach ($l in @((Get-CarteDe $p.Id $k[0] $k[1]).Lignes)) {
            $t = [string]$l.Texte
            if ($t.Length -gt 80) { $trop += "$($p.Id) : $t"; continue }
            if (-not $t.EndsWith('...')) { continue }
            $debut = $t.Substring(0, $t.Length - 3)   # (coupée : ce qui reste doit finir sur un mot entier)
            if (-not @(@([string]$p.Titre, [string]$k[1]) | Where-Object { $_.StartsWith($debut) -and $_.Length -gt $debut.Length -and $_[$debut.Length] -match '[\s,:;(]' })) { $trop += "$($p.Id) : $t" }
        }
    }
}
Write-Resultat "carte de Mindset : chaque ligne tient sans couper un mot ($(@($script:Donnees.Pannes).Count) pannes)" (-not $trop) "($(@($trop | Select-Object -First 3) -join ' ; '))"
$script:Historique = @()

# ---------------------------------------------------------------------------
# 10. Le journal et le rapport
# ---------------------------------------------------------------------------
# Ouvrir « Pour le réparateur » ne note pas la réponse une deuxième fois
Start-EnqueteTest 'redemarre' 1, 1, 1, 1, 1, 1, 0
$avant = ([regex]::Matches((Get-Journal), 'Réponse redemarre : trouve memoire')).Count
Show-Etape
Invoke-Clic 'Pour le réparateur'
$n = ([regex]::Matches((Get-Journal), 'Réponse redemarre : trouve memoire')).Count - $avant
Write-Resultat 'journal : « Pour le réparateur » ne note pas la réponse deux fois' ($n -eq 1 -and @(Get-TextesEcran) -contains 'Enregistrer le rapport sur le Bureau') "($n fois)"
# Une réponse « aucune piste » : pas de double espace dans le journal
Start-EnqueteTest 'pubs' 0, 1, 1
Show-Etape
Write-Resultat 'journal : réponse « aucune piste » sans double espace' ((Get-Journal) -match 'Réponse pubs : rien \(réponses 0,1,1\)') ''
# Deux rapports dans la même minute : le premier est gardé
$f1 = Save-Resume 'premier'; $f2 = Save-Resume 'deuxième'
Write-Resultat 'rapport : deux dans la même minute, le premier est gardé' ($f1 -ne $f2 -and [IO.File]::ReadAllText($f1, [Text.Encoding]::UTF8) -eq 'premier' -and [IO.File]::ReadAllText($f2, [Text.Encoding]::UTF8) -eq 'deuxième') "($([IO.Path]::GetFileName($f1)), $([IO.Path]::GetFileName($f2)))"

# ---------------------------------------------------------------------------
# 11. Une enquête reprise (revue du 25/10, constat 16) : chaque réponse retourne à la question qui porte son Id, même si
# ce que Windows a noté a changé entre-temps ; une enquête finie se revoit avec ce que Windows avait noté ce jour-là
# ---------------------------------------------------------------------------
# (les indices de Windows prennent la valeur voulue : une lecture finie, sans jamais lire le vrai PC)
function Set-IndicesTest($Indices) { Stop-LectureJournaux; $script:Journaux = [hashtable]::Synchronized(@{ Indices = $Indices }); $script:JournauxTache = @{ Res = [pscustomobject]@{ IsCompleted = $true } }; $script:JournauxAbandon = $null; $script:JournauxNotes = $true }
function New-IndiceTest([string]$Id, [bool]$Vu) { [pscustomobject]@{ Vu = $Vu; Texte = "$Id vu"; TexteOk = "$Id rien"; Affiche = $true; Partiel = $false } }
$connus = @($script:Donnees.Pannes | ForEach-Object { Get-Etapes $script:Donnees $_ } | Where-Object { $_.Type -eq 'auto' } | ForEach-Object { [string]$_.Indice } | Select-Object -Unique)
$hasard = New-Object Random 7
$essais = 0; $decalesAvant = 0; $decales = 0; $exemple = ''
foreach ($p in $script:Donnees.Pannes) {
    $ets = @(Get-Etapes $script:Donnees $p)
    $autos = @($ets | Where-Object { $_.Type -eq 'auto' } | ForEach-Object { [string]$_.Indice } | Select-Object -Unique)
    if (-not $autos -or -not @($ets | Where-Object { $_.Id -eq 'cepc' })) { continue }
    for ($k = 0; $k -lt 40; $k++) {
        $A = @{}; foreach ($x in $connus) { $A[$x] = New-IndiceTest $x ($hasard.Next(2) -eq 1) }
        $B = $A.Clone(); $chg = $autos[$hasard.Next($autos.Count)]; $B[$chg] = New-IndiceTest $chg (-not $A[$chg].Vu)
        # Une enquête au hasard (« sur ce PC » : oui ; pas de danger), avec ce que Windows notait le jour A
        $choix = @()
        for ($n = 0; $n -lt 30; $n++) {
            $e = Get-Enquete $script:Donnees $p $choix $A
            if (-not $e.Etape) { break }
            $c = if ($e.Etape.Id -eq 'cepc') { [array]::IndexOf(@($e.Etape.Reponses | ForEach-Object { $_.Note }), 'cepc') } elseif ($e.Etape.Id -eq 'danger') { @($e.Etape.Reponses).Count - 1 } else { $hasard.Next(@($e.Etape.Reponses).Count) }
            $choix += $c
        }
        $ea = Get-Enquete $script:Donnees $p $choix $A
        $gardees = @{}; foreach ($r in (Get-Paires $ea)) { $gardees[$r.Etape] = $r.Reponse }
        # Reprise le jour B, comme Mindcare : l'enquête gardée, rouverte par Open-Diagnostic
        Set-IndicesTest $B
        $script:Historique = @([pscustomobject]@{ Id = "essai-$essais"; Panne = $p.Id; Choix = @($choix); Reponses = @(Get-Paires $ea); Fini = $false; Suspect = ''; Titre = '' })
        Open-Diagnostic "essai-$essais"
        $eb = Get-Enquete $script:Donnees $p $script:ChoixFaits $B
        $faux = @(for ($j = 0; $j -lt @($eb.Faits).Count; $j++) { $id = [string]@($eb.Faits)[$j].Etape.Id; if ($gardees.ContainsKey($id) -and $gardees[$id] -ne @($eb.Choix)[$j]) { $id } })
        if ($faux) { $decales++; if (-not $exemple) { $exemple = "$($p.Id) : $($faux -join ', ')" } }
        # (contre-épreuve : rejouées par leur place, comme avant)
        $ep = Get-Enquete $script:Donnees $p $choix $B
        if (@(for ($j = 0; $j -lt @($ep.Faits).Count; $j++) { $id = [string]@($ep.Faits)[$j].Etape.Id; if ($gardees.ContainsKey($id) -and $gardees[$id] -ne @($ep.Choix)[$j]) { $id } }).Count) { $decalesAvant++ }
        $essais++
    }
}
Write-Resultat "reprise : $essais enquêtes rejouées avec un indice changé, aucune réponse sur une autre question" ($essais -ge 400 -and $decales -eq 0) "($decales décalées, par exemple $exemple)"
Write-Resultat "reprise (contre-épreuve) : rejouées par leur place, $decalesAvant réponses tombaient sur une autre question" ($decalesAvant -gt 0) ''
# Une enquête finie (le test de mémoire raté : la mémoire) revue alors que Windows ne note plus rien : même réponse
$jourA = @{ 'testmemoire-erreurs' = [pscustomobject]@{ Vu = $true; Texte = 'Le test de mémoire du 3 octobre a trouvé des erreurs'; TexteOk = ''; Affiche = $true; Partiel = $false } }
Set-IndicesTest $jourA
$script:Historique = @()
Start-EnqueteTest 'redemarre' 1, 0
Show-Etape
$finie = $script:Historique[0]
Set-IndicesTest @{}
Open-Diagnostic ([string]$finie.Id)
Write-Resultat 'reprise : une enquête finie se revoit avec la même réponse' ($finie.Fini -and $finie.Suspect -eq 'memoire' -and @(Get-TextesEcran) -contains 'TROUVÉ' -and $script:Historique[0].Suspect -eq 'memoire') "(gardée : $($finie.Suspect) ; écran : $(@(Get-TextesEcran | Select-Object -First 2) -join ' / '))"
# (le lendemain : relue du fichier diagnostics.json)
Read-Historique
Open-Diagnostic ([string]$finie.Id)
Write-Resultat 'reprise : une enquête finie, relue du fichier, garde sa réponse' ($script:Historique[0].PSObject.Properties['Indices'] -and @(Get-TextesEcran) -contains 'TROUVÉ') ''
# (contre-épreuve : sans les indices gardés, comme avant, la réponse changeait toute seule)
$script:Historique = @($finie | Select-Object * -ExcludeProperty Indices)
Open-Diagnostic ([string]$finie.Id)
Write-Resultat 'reprise (contre-épreuve) : sans les indices gardés, la réponse changeait' (@(Get-TextesEcran) -notcontains 'TROUVÉ') ''
# L'ancien format (les numéros seulement) se relit toujours
$script:Historique = @([pscustomobject]@{ Id = 'ancien'; Panne = 'redemarre'; Choix = @(1, 1); Fini = $false; Suspect = ''; Titre = '' })
Set-IndicesTest @{}
Open-Diagnostic 'ancien'
Write-Resultat 'reprise : l''ancien format (les numéros seulement) se relit' ((@($script:ChoixFaits) -join ',') -eq '1,1' -and $script:PanneEnCours.Id -eq 'redemarre') "(choix : $(@($script:ChoixFaits) -join ','))"
Reset-Journaux 0

# ---------------------------------------------------------------------------
# 12. Ce qu'on écrit à l'assistant ne va pas dans journal.txt (revue du 25/10, constat 26) : seulement sa longueur
# ---------------------------------------------------------------------------
Show-Accueil
$secret = 'ma voisine Ginette a renversé du thé, il s''éteint'
$ui.Recherche.Text = $secret
$env:MINDCARE_TEST_IA = '{"panne":"seteint","reponses":{}}'
try { $script:IaAttente = $null; Request-Ia; Wait-Fenetre 8000 { (Get-Journal) -match 'Assistant : .+ -> seteint' } }
finally { Remove-Item env:MINDCARE_TEST_IA -ErrorAction SilentlyContinue }
Write-Resultat 'assistant : la phrase tapée n''est pas écrite dans le journal, seulement sa longueur' ((Get-Journal) -match "Assistant : \($($secret.Length) caractères\) -> seteint" -and (Get-Journal) -notmatch 'Ginette') "($(((Get-Journal) -split "`n" | Select-String 'Assistant :' | Select-Object -Last 1)))"

# Un historique vide (coupure pendant l'écriture) : la copie de secours est reprise, et l'enregistrement suivant ne la perd pas
$fj = Join-Path $env:TEMP "mindcare-essai-vide-$PID.json"
try {
    [IO.File]::WriteAllText("$fj.bak", '[{"Panne":"ecranbleu"},{"Panne":"pubs"}]', [Text.Encoding]::UTF8)
    [IO.File]::WriteAllText($fj, '', [Text.Encoding]::UTF8)
    $lu = @(Read-JsonFile $fj)
    Save-JsonFile $fj $lu
    $relu = @(Read-JsonFile $fj)
    Write-Resultat 'données : fichier vide, la copie de secours est reprise et gardée' ($lu.Count -eq 2 -and $relu.Count -eq 2 -and $relu[1].Panne -eq 'pubs') "($($lu.Count) puis $($relu.Count))"
} finally { Remove-Item -LiteralPath $fj, "$fj.bak", "$fj.tmp" -Force -ErrorAction SilentlyContinue; Get-ChildItem -Path "$fj.abime-*" -ErrorAction SilentlyContinue | Remove-Item -Force }

# ---------------------------------------------------------------------------
# 13. Un fichier tenu un instant par un autre programme (revue du 25/10, constat 19) : l'antivirus ou OneDrive le
# gardent ouvert une demi-seconde au démarrage. Il n'est ni abîmé ni mis de côté, et la dernière enquête n'est pas perdue.
# ---------------------------------------------------------------------------
# (un autre « programme » : le fichier ouvert sans partage, relâché par un autre fil d'exécution au bout de Ms)
Add-Type -TypeDefinition 'public static class VerrouTest { public static void LibererApres(System.IDisposable d, int ms) { new System.Threading.Thread(() => { System.Threading.Thread.Sleep(ms); d.Dispose(); }).Start(); } }'
function Lock-Fichier([string]$Chemin, [int]$Ms = 0) {
    $fs = [IO.File]::Open($Chemin, 'Open', 'Read', 'None')
    if ($Ms) { [VerrouTest]::LibererApres($fs, $Ms) }
    $fs
}
function Get-Abimes([string]$Chemin) { @(Get-ChildItem -Path "$Chemin.abime-*", "$Chemin.bak.abime-*" -ErrorAction SilentlyContinue) }
$utf8 = New-Object Text.UTF8Encoding($false)

# a. Read-JsonFile : tenu 700 ms, il est relu (la bonne version, pas la sauvegarde), et rien n'est mis de côté
$fj = Join-Path $test 'pris.json'
[IO.File]::WriteAllText("$fj.bak", '{"N":1}', $utf8); [IO.File]::WriteAllText($fj, '{"N":2}', $utf8)
try {
    $avant = Get-Journal
    $null = Lock-Fichier $fj 700
    $chrono = [Diagnostics.Stopwatch]::StartNew(); $j = Read-JsonFile $fj; $duree = $chrono.ElapsedMilliseconds
    $illisible = (Get-Journal).Substring($avant.Length) -match 'Fichier illisible'
    Write-Resultat "fichier pris 700 ms : relu après une courte attente ($duree ms), jamais dit abîmé" ($j.N -eq 2 -and -not (Get-Abimes $fj) -and -not $illisible -and $duree -lt 3000) "(lu : N = $($j.N) ; mis de côté : $((Get-Abimes $fj).Count) ; « illisible » au journal : $illisible)"
} catch { Write-Resultat 'fichier pris 700 ms : relu après une courte attente' $false "($_)" }

# b. Tenu plus longtemps que les essais : la sauvegarde est montrée, rien n'est mis de côté, rien n'est écrit (la bonne
# version n'est pas écrasée), puis, une fois le fichier libre, il est relu avant d'enregistrer
Wait-Fenetre 900   # (le verrou du a est relâché)
try {
    $fs = Lock-Fichier $fj
    try {
        $j = Read-JsonFile $fj
        $refuse = try { Save-JsonFile $fj ([ordered]@{ N = 3 }); $false } catch { $true }
    } finally { $fs.Dispose() }
    $brut = [IO.File]::ReadAllText($fj, [Text.Encoding]::UTF8) | ConvertFrom-Json
    $r = Get-FichierRelu $fj
    Write-Resultat 'fichier pris longtemps : sauvegarde montrée, rien écrit, rien mis de côté, relu une fois libre' ($j.N -eq 1 -and $refuse -and $brut.N -eq 2 -and -not (Get-Abimes $fj) -and $r.Libre -and $r.Relu -and $r.Contenu.N -eq 2) "(lu : $($j.N) ; écriture refusée : $refuse ; dans le fichier : $($brut.N) ; relu : $($r.Contenu.N))"
} catch { Write-Resultat 'fichier pris longtemps : relu une fois libre' $false "($_)" }

# c. Le cas de la revue : diagnostics.json tenu au lancement, la dernière enquête n'est pas perdue
function New-EnqueteGardee([string]$Id, [string]$Maj) { [ordered]@{ Id = $Id; Panne = 'seteint'; Choix = @(1, 1); Reponses = @([ordered]@{ Etape = 'danger'; Reponse = 1 }, [ordered]@{ Etape = 'type'; Reponse = 1 }); Debut = $Maj; Maj = $Maj; Fini = $false; Suspect = ''; Titre = '' } }
function Set-HistoriqueTest([object[]]$Fichier, [object[]]$Bak) {
    Remove-Item -LiteralPath $HistoriqueFichier, "$HistoriqueFichier.bak" -Force -ErrorAction SilentlyContinue
    Get-Abimes $HistoriqueFichier | Remove-Item -Force
    [IO.File]::WriteAllText($HistoriqueFichier, (ConvertTo-Json -InputObject ([ordered]@{ Version = 2; Diagnostics = $Fichier }) -Depth 6), $utf8)
    if ($Bak) { [IO.File]::WriteAllText("$HistoriqueFichier.bak", (ConvertTo-Json -InputObject ([ordered]@{ Version = 2; Diagnostics = $Bak }) -Depth 6), $utf8) }
}
function Get-IdsGardes { @(([IO.File]::ReadAllText($HistoriqueFichier, [Text.Encoding]::UTF8) | ConvertFrom-Json).Diagnostics | ForEach-Object { [string]$_.Id }) }
$derniere = New-EnqueteGardee 'derniere' '2026-10-24T20:00:00'; $vieille = New-EnqueteGardee 'vieille' '2026-10-20T20:00:00'
try {
    Set-HistoriqueTest @($derniere, $vieille) @($vieille)
    $null = Lock-Fichier $HistoriqueFichier 700
    Read-Historique
    $lus = @($script:Historique | ForEach-Object { [string]$_.Id }) -join ','
    Start-EnqueteTest 'seteint' 1; Show-Etape   # (une nouvelle enquête enregistrée ensuite)
    $ids = Get-IdsGardes
    Write-Resultat 'diagnostics.json pris 700 ms au lancement : la dernière enquête est gardée' ($lus -eq 'derniere,vieille' -and $ids -contains 'derniere' -and $ids.Count -eq 3 -and -not (Get-Abimes $HistoriqueFichier)) "(lues : $lus ; dans le fichier : $($ids -join ','))"
} catch { Write-Resultat 'diagnostics.json pris 700 ms au lancement : la dernière enquête est gardée' $false "($_)" }

# d. Pris plus longtemps que les essais au lancement (seule la sauvegarde est lue), puis libre : l'enregistrement suivant
# relit le fichier et garde la dernière enquête avec la nouvelle
try {
    Set-HistoriqueTest @($derniere, $vieille) @($vieille)
    $fs = Lock-Fichier $HistoriqueFichier
    try { Read-Historique; $lus = @($script:Historique | ForEach-Object { [string]$_.Id }) -join ',' } finally { $fs.Dispose() }
    Start-EnqueteTest 'seteint' 1; Show-Etape
    $ids = Get-IdsGardes
    Write-Resultat 'diagnostics.json pris longtemps au lancement : relu avant d''enregistrer, rien de perdu' ($lus -eq 'vieille' -and $ids.Count -eq 3 -and $ids[0] -eq $script:DiagId -and $ids[1] -eq 'derniere' -and $ids[2] -eq 'vieille' -and -not (Get-Abimes $HistoriqueFichier)) "(lues : $lus ; dans le fichier : $($ids -join ','))"
} catch { Write-Resultat 'diagnostics.json pris longtemps au lancement : rien de perdu' $false "($_)" }

# e. suite.json (mindset.ps1) tenu 700 ms : lu quand même (avant : illisible, Mindcare se croyait hors de Mindset)
$sauveSuite = $SuiteFile
try {
    $SuiteFile = Join-Path $test 'suite.json'
    [IO.File]::WriteAllText($SuiteFile, '{"theme":"rubis","applis":{"mindcare":"C:\\Program Files\\Mindset\\Mindcare"}}', $utf8)
    $null = Lock-Fichier $SuiteFile 700
    $s = Read-Suite
    Write-Resultat 'suite.json pris 700 ms : lu quand même' ($s.Lisible -and $s.Theme -eq 'rubis' -and $s.Dossier -eq 'C:\Program Files\Mindset\Mindcare') "(lisible : $($s.Lisible) ; thème : $($s.Theme))"
    # Encore pris après les essais : « pris », pas « illisible » ; une copie dans un dossier de Mindset reste gérée
    Wait-Fenetre 900
    $fs = Lock-Fichier $SuiteFile
    try { $s = Read-Suite 2 } finally { $fs.Dispose() }
    Write-Resultat 'suite.json pris longtemps : « pris », la copie de Mindset reste gérée, une autre copie non' ($s.Pris -and -not $s.Lisible -and (Test-GereeParMindset $s 'C:\Program Files\Mindset\Mindcare') -and (Test-GereeParMindset $s 'D:\Mindset\Programmes\Mindcare') -and -not (Test-GereeParMindset $s 'C:\Users\x\Bureau\Mindcare')) "(pris : $($s.Pris))"
} catch { Write-Resultat 'suite.json pris : lu quand même' $false "($_)" }
finally { $SuiteFile = $sauveSuite }

# ---------------------------------------------------------------------------
# 14. Reprendre une enquête pas finie dont une question a changé dans le catalogue (revue du 25/10, constat 22) : les
# réponses encore valables sont gardées et rejouées ; s'il n'y en a aucune à rejouer, l'enquête reste telle quelle
# ---------------------------------------------------------------------------
function Get-Garde([string]$Id) { @(([IO.File]::ReadAllText($HistoriqueFichier, [Text.Encoding]::UTF8) | ConvertFrom-Json).Diagnostics | Where-Object { $_.Id -eq $Id }) | Select-Object -First 1 }
function Get-TextePaires($D) { @($D.Reponses | ForEach-Object { "$($_.Etape)=$($_.Reponse)" }) -join ',' }
function New-Reprise([string]$Id, [object[]]$Paires) {
    $d = [ordered]@{ Id = $Id; Panne = 'seteint'; Choix = @($Paires | ForEach-Object { $_[1] }); Reponses = @($Paires | ForEach-Object { [ordered]@{ Etape = $_[0]; Reponse = $_[1] } }); Debut = '2026-10-24T20:00:00'; Maj = '2026-10-24T20:05:00'; Fini = $false; Suspect = ''; Titre = '' }
    Set-HistoriqueTest @($d)
    Read-Historique
}
Set-IndicesTest @{}
# a. La première question a changé (son Id n'existe plus) : rien n'est rejoué, l'enquête reste telle quelle dans le fichier
try {
    New-Reprise 'reprise-a' @(@('danger-ancien', 1), @('type', 1), @('maison', 1))
    $avant = [IO.File]::ReadAllText($HistoriqueFichier, [Text.Encoding]::UTF8)
    Open-Diagnostic 'reprise-a'
    $g = Get-Garde 'reprise-a'
    Write-Resultat 'reprise : première question changée, l''enquête reste telle quelle' ($g -and (Get-TextePaires $g) -eq 'danger-ancien=1,type=1,maison=1' -and $g.Debut -eq '2026-10-24T20:00:00' -and [IO.File]::ReadAllText($HistoriqueFichier, [Text.Encoding]::UTF8) -eq $avant -and $script:Historique.Count -eq 1) "(gardée : $(if ($g) { Get-TextePaires $g } else { 'effacée' }))"
    # La nouvelle question est posée ; une fois répondue, les réponses suivantes encore valables sont rejouées
    Add-Choix 1
    $g = Get-Garde 'reprise-a'
    Write-Resultat 'reprise : la nouvelle question répondue, les suivantes sont rejouées, la date de début gardée' ((@($script:ChoixFaits) -join ',') -eq '1,1,1' -and [string]$script:PanneEnCours.Id -eq 'seteint' -and $g -and (Get-TextePaires $g) -eq 'danger=1,type=1,maison=1' -and $g.Debut -eq '2026-10-24T20:00:00') "(choix : $(@($script:ChoixFaits) -join ',') ; gardée : $(if ($g) { Get-TextePaires $g } else { 'effacée' }))"
} catch { Write-Resultat 'reprise : première question changée' $false "($_)" }
# b. Aucune réponse valable (des questions qui n'existent plus, une réponse hors limites) : gardée telle quelle
try {
    New-Reprise 'reprise-b' @(@('disparue-1', 0), @('disparue-2', 1), @('danger', 7))
    Open-Diagnostic 'reprise-b'
    $g = Get-Garde 'reprise-b'
    Write-Resultat 'reprise : aucune réponse valable, l''enquête reste telle quelle' ($g -and (Get-TextePaires $g) -eq 'disparue-1=0,disparue-2=1,danger=7' -and $g.Maj -eq '2026-10-24T20:05:00') "(gardée : $(if ($g) { Get-TextePaires $g } else { 'effacée' }))"
} catch { Write-Resultat 'reprise : aucune réponse valable' $false "($_)" }
# c. Une question du milieu a changé : les réponses d'avant sont rejouées, celles d'après (encore valables) restent gardées
# pour plus tard, la réponse à la question disparue est laissée
try {
    New-Reprise 'reprise-c' @(@('danger', 1), @('type-ancien', 1), @('maison', 1), @('chaleur', 2))
    Open-Diagnostic 'reprise-c'
    $g = Get-Garde 'reprise-c'
    Write-Resultat 'reprise : question du milieu changée, les réponses suivantes valables restent gardées' ((@($script:ChoixFaits) -join ',') -eq '1' -and $g -and (Get-TextePaires $g) -eq 'danger=1,maison=1,chaleur=2' -and $g.Debut -eq '2026-10-24T20:00:00') "(choix : $(@($script:ChoixFaits) -join ',') ; gardée : $(if ($g) { Get-TextePaires $g } else { 'effacée' }))"
    Read-Historique; Open-Diagnostic 'reprise-c'   # (le lendemain : relue du fichier, puis la question changée répondue)
    Add-Choix 1
    Write-Resultat 'reprise : relue du fichier, les réponses gardées sont rejouées ensuite' ((@($script:ChoixFaits) -join ',') -eq '1,1,1,2') "(choix : $(@($script:ChoixFaits) -join ','))"
} catch { Write-Resultat 'reprise : question du milieu changée' $false "($_)" }
# d. (inchangé) Une nouvelle enquête ramenée à zéro réponse n'est toujours pas gardée
try {
    Set-HistoriqueTest @(); Read-Historique
    Start-EnqueteTest 'seteint' 1; Show-Etape
    $id = $script:DiagId; $garde = [bool](Get-Garde $id)
    $script:ChoixFaits = @(); Show-Etape
    Write-Resultat 'nouvelle enquête ramenée à zéro réponse : retirée, comme avant' ($garde -and -not (Get-Garde $id) -and -not $script:Historique.Count) "(gardée d'abord : $garde)"
} catch { Write-Resultat 'nouvelle enquête ramenée à zéro réponse' $false "($_)" }
Remove-Item -LiteralPath $HistoriqueFichier, "$HistoriqueFichier.bak" -Force -ErrorAction SilentlyContinue
$script:Historique = @()
Stop-LectureJournaux
Get-ChildItem env: | Where-Object { $_.Name -like 'MINDCARE_*' } | ForEach-Object { Remove-Item "env:$($_.Name)" }
if ($script:Ok) { Write-Host 'Tout est bon.' -ForegroundColor Green; exit 0 }
Write-Host 'Des tests ont échoué.' -ForegroundColor Red
exit 1
