# Teste la dictée de Mindcare (modules\dictee.ps1 et sa partie dans contenu.ps1) sans le vrai micro : un faux micro
# (on règle ce qu'il « entend »), un faux whisper-cli (un petit programme compilé ici qui écrit une phrase connue) et,
# si Whisper est déjà sur ce PC (dossier de la dictée d'Evermind, lu sans rien y changer), un vrai essai : une phrase
# dite par la voix de Windows dans un fichier, écrite par le vrai Whisper copié dans _test\dictee.
# Rien n'est téléchargé, rien n'est modifié sur le PC, aucune fenêtre ne s'ouvre, le vrai micro n'est jamais ouvert.
#
#   powershell -NoProfile -STA -ExecutionPolicy Bypass -File .\outils\tester-dictee.ps1
#
# Code de sortie : 0 si tout est bon, 1 sinon.

$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$Modules = Join-Path $racine 'Mindcare\fichiers\modules'
$test = Join-Path $racine '_test\dictee'
Remove-Item -LiteralPath $test -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $test | Out-Null
$script:Ok = $true
function Write-Resultat([string]$Nom, [bool]$Bon, [string]$Detail = '') {
    if ($Bon) { Write-Host "OK     $Nom" -ForegroundColor Green } else { Write-Host "ÉCHEC  $Nom $Detail" -ForegroundColor Red; $script:Ok = $false }
}

Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase

# Ce que Mindcare fournit d'habitude (journal en mémoire, interface réduite au nécessaire)
$EnTest = $true
$DataDir = Join-Path $test 'donnees'
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
$script:Journal = New-Object Collections.Generic.List[string]
function Write-Log([string]$Texte) { $script:Journal.Add($Texte) }
function Get-Brush([string]$Hex) { [System.Windows.Media.BrushConverter]::new().ConvertFromString($Hex) }
function New-Text([string]$Texte, [double]$Taille = 14, [string]$Couleur = '#EEEBF7', [string]$Graisse = 'Normal') { $t = New-Object System.Windows.Controls.TextBlock; $t.Text = $Texte; $t }
function New-Button([string]$Texte, [string]$Style) { $b = New-Object System.Windows.Controls.Button; $b.Content = $Texte; $b }
function Set-BulleMascotte([string]$Texte) { $script:Bulle = $Texte }
function Protect-Action([scriptblock]$Action) { $Action }
$script:SuiteMicro = ''
function Read-Suite { @{ Micro = $script:SuiteMicro } }
foreach ($m in 'ia', 'dictee', 'contenu') { . (Join-Path $Modules "$m.ps1") }
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $Modules 'natif.cs'), [Text.Encoding]::UTF8))

# Un faux micro : le test règle ce qu'il entend (Millis, HeardVoice, MillisSinceVoice) ; SaveWav copie un son préparé
$script:FauxSon = $null
$script:CodeMicro = 0
function New-DicteeMicro {
    $m = [pscustomobject]@{ Level = 0.2; Millis = 0; HeardVoice = $false; MillisSinceVoice = 0; Demarre = $null; Arrete = $false; Code = $script:CodeMicro }
    $m | Add-Member ScriptMethod Start { param($Micro) $this.Demarre = $Micro; $this.Code }
    $m | Add-Member ScriptMethod Stop { $this.Arrete = $true }
    $m | Add-Member ScriptMethod SaveWav { param($Chemin) if ($script:FauxSon) { Copy-Item -LiteralPath $script:FauxSon -Destination $Chemin } else { [IO.File]::WriteAllBytes($Chemin, [byte[]](1..44)) } }
    $script:DernierMicro = $m
    $m
}
function Test-DicteeMicroPresent { $true }

# L'interface : une barre de recherche et sa zone, comme l'accueil
$ui = @{}
$ui.Contenu = New-Object System.Windows.Controls.StackPanel
$ui.ChampRecherche = New-Object System.Windows.Controls.Border
$ui.Recherche = New-Object System.Windows.Controls.TextBox
$ui.ChampRecherche.Child = $ui.Recherche
[void]$ui.Contenu.Children.Add($ui.ChampRecherche)
$ui.BoutonMicro = New-BoutonMicro
$ui.ZoneDictee = New-Object System.Windows.Controls.StackPanel
[void]$ui.Contenu.Children.Add($ui.ZoneDictee)
function Get-TexteZone { @($ui.ZoneDictee.Children | ForEach-Object { $_.Child } | ForEach-Object { @($_.Children | Where-Object { $_ -is [System.Windows.Controls.TextBlock] } | ForEach-Object { $_.Text }) }) -join ' | ' }
function Wait-Dispatcher([scriptblock]$Fini, [int]$Secondes = 10) {
    $limite = (Get-Date).AddSeconds($Secondes)
    while (-not (& $Fini) -and (Get-Date) -lt $limite) {
        $frame = New-Object System.Windows.Threading.DispatcherFrame
        [void][System.Windows.Threading.Dispatcher]::CurrentDispatcher.BeginInvoke([System.Windows.Threading.DispatcherPriority]::Background, [Action]{ $frame.Continue = $false })
        [System.Windows.Threading.Dispatcher]::PushFrame($frame)
        Start-Sleep -Milliseconds 50
    }
}
function Reset-Dictee { Stop-DicteeSansSuite; $script:DicteeMessage = $null; $script:DicteeProposee = $false; $ui.Recherche.Text = ''; $script:Journal.Clear() }

# Un faux whisper-cli : il note ses arguments et écrit la phrase de FAUX_WHISPER dans le fichier -of (.txt), ou attend
$csc = Join-Path $env:windir 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
function New-FauxWhisper([string]$Dossier) {
    New-Item -ItemType Directory -Force -Path $Dossier | Out-Null
    $src = Join-Path $Dossier 'faux.cs'
    [IO.File]::WriteAllText($src, @'
using System; using System.IO; using System.Text;
static class F { static int Main(string[] a) {
    File.WriteAllText(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "arguments.txt"), string.Join("\n", a));
    string t = Environment.GetEnvironmentVariable("FAUX_WHISPER") ?? "";
    if (t == "attendre") { System.Threading.Thread.Sleep(60000); return 0; }
    for (int i = 0; i < a.Length - 1; i++) if (a[i] == "-of") File.WriteAllText(a[i + 1] + ".txt", t, new UTF8Encoding(false));
    return 0; } }
'@)
    $null = & $csc /nologo /target:exe "/out:$(Join-Path $Dossier 'whisper-cli.exe')" $src
    foreach ($d in 'whisper.dll', 'ggml.dll', 'ggml-base.dll') { Set-Content -LiteralPath (Join-Path $Dossier $d) -Value 'faux' }
    Set-Content -LiteralPath (Join-Path $Dossier 'ggml-small-q5_1.bin') -Value 'faux'
}

# 1. Pas installée : le micro propose l'installation (environ 190 Mo), sans rien télécharger
$r = Start-Dictee
if ($r -eq 'installer') { $script:DicteeProposee = $true; Update-ZoneDictee }
Write-Resultat 'pas installée : le micro propose de l''installer (environ 190 Mo)' ($r -eq 'installer' -and (Get-TexteZone) -match 'installe la dictée : environ 190 Mo' -and -not $script:DicteeDl) "($r ; $(Get-TexteZone))"
Write-Resultat 'pas installée : les fichiers manquants sont le programme et le français' ((Get-DicteeManquants).Count -eq 2 -and (Get-DicteeTaille) -eq 190) "($((Get-DicteeManquants).Count) ; $(Get-DicteeTaille))"
# Installation coupée (programme sans ses .dll) : à réparer
New-Item -ItemType Directory -Force -Path $DicteeDir | Out-Null
Set-Content -LiteralPath $DicteeExe -Value 'faux'
Update-ZoneDictee
$bouton = @($ui.ZoneDictee.Children[0].Child.Children | Where-Object { $_ -is [System.Windows.Controls.Button] })[0]
Write-Resultat 'installation coupée : le bouton dit « Réparer »' ($bouton -and $bouton.Content -eq 'Réparer' -and -not (Test-DicteeInstallee)) "($($bouton.Content))"
Remove-Item -LiteralPath $DicteeDir -Recurse -Force
Reset-Dictee

New-FauxWhisper $DicteeDir
Write-Resultat 'installée : programme, .dll et français trouvés' (Test-DicteeInstallee) ''

# 2. On parle puis on se tait : Whisper écrit, la phrase va dans la barre, le son est effacé
$env:FAUX_WHISPER = " [Musique] Mon PC s'éteint quand je joue. `r`n"
$r = Start-Dictee
$m = $script:DernierMicro
Write-Resultat 'appui sur le micro : écoute (micro de Windows si rien n''est choisi)' ($r -eq 'ecoute' -and (Test-DicteeEcoute) -and $m.Demarre -eq [uint32]::MaxValue) "($r ; micro $($m.Demarre))"
Show-DicteeEcoute
Write-Resultat 'écoute : le bouton s''allume et la zone dit « Je t''écoute »' ((Get-TexteZone) -match 'Je t.écoute' -and $ui.BoutonMicro.Background.Color.A -gt 0) "($(Get-TexteZone))"
$m.Millis = 2000; $m.HeardVoice = $true; $m.MillisSinceVoice = 400; Step-Dictee
$encore = Test-DicteeEcoute
$m.Millis = 2600; $m.MillisSinceVoice = 1100; Step-Dictee
Write-Resultat 'fin de la phrase : on attend une seconde de silence' ($encore -and -not (Test-DicteeEcoute) -and $m.Arrete -and $script:DicteeEcrit) "(encore : $encore)"
$wav = $script:DicteeEcrit.Wav
Wait-Dispatcher { -not $script:DicteeEcrit } 15
Write-Resultat 'la phrase dite va dans la barre (sans les bruits notés par Whisper)' ($ui.Recherche.Text -eq "Mon PC s'éteint quand je joue.") "($($ui.Recherche.Text))"
Write-Resultat 'puis : « Corrige si besoin, puis appuie sur Entrée »' ((Get-TexteZone) -match 'Corrige si besoin, puis appuie sur Entrée') "($(Get-TexteZone))"
Write-Resultat 'le son et le texte de Whisper sont effacés' (-not (Test-Path -LiteralPath $wav) -and -not @(Get-ChildItem $env:TEMP -Filter "mindcare-$PID-dictee*").Count) ''
$args1 = (Get-Content -LiteralPath (Join-Path $DicteeDir 'arguments.txt')) -join ' '
Write-Resultat 'Whisper : en français, le bon modèle, sans les heures' ($args1 -match '-l fr' -and $args1 -match [regex]::Escape($DicteeModele) -and $args1 -match '-nt') "($args1)"
Write-Resultat 'la phrase n''est pas écrite dans le journal (seulement sa longueur)' (($script:Journal -join ' ') -notmatch 'éteint' -and ($script:Journal -join ' ') -match 'Dictée : [\d,.]+ s, 30 caractères') "($($script:Journal -join ' | '))"
Reset-Dictee

# 3. Deuxième appui sur le micro : on arrête d'écouter tout de suite
$env:FAUX_WHISPER = 'le wifi coupe'
[void](Start-Dictee); $m = $script:DernierMicro; $m.Millis = 1500; $m.HeardVoice = $true
$r = Invoke-BoutonMicro
Write-Resultat 'deuxième appui : fin de l''écoute, la phrase est écrite' (-not (Test-DicteeEcoute) -and $script:DicteeEcrit) ''
Wait-Dispatcher { -not $script:DicteeEcrit } 15
Write-Resultat 'deuxième appui : la phrase arrive dans la barre' ($ui.Recherche.Text -eq 'le wifi coupe') "($($ui.Recherche.Text))"
Reset-Dictee

# 4. Rien entendu en 7 secondes ; micro débranché pendant l'écoute ; micro refusé par Windows
[void](Start-Dictee); $m = $script:DernierMicro; $m.Millis = 7100; Step-Dictee
Write-Resultat 'rien entendu : « Je n''ai rien entendu », Whisper n''est pas lancé' (-not (Test-DicteeOccupee) -and (Get-TexteZone) -match 'rien entendu') "($(Get-TexteZone))"
Reset-Dictee
[void](Start-Dictee); $m = $script:DernierMicro; $script:Dictee.Debut = [Diagnostics.Stopwatch]::StartNew(); Start-Sleep -Milliseconds 3200; $m.Millis = 0; Step-Dictee
Write-Resultat 'micro débranché pendant l''écoute : « Je n''entends plus le micro »' (-not (Test-DicteeOccupee) -and (Get-TexteZone) -match 'entends plus le micro') "($(Get-TexteZone))"
Reset-Dictee
$script:CodeMicro = 4   # (le code de Windows quand le micro est refusé ou occupé)
$r = Start-Dictee
$script:CodeMicro = 0
if ($r -notin 'ecoute', 'installer', 'fin', 'occupee') { Show-DicteeFin $r }
Write-Resultat 'micro refusé : on explique où donner le droit dans Windows' (-not (Test-DicteeEcoute) -and (Get-TexteZone) -match 'Confidentialité, Microphone') "($(Get-TexteZone))"
Reset-Dictee

# 5. Le micro choisi dans Mindset : pris par son nom s'il est branché, sinon celui de Windows
$noms = @([MindcareMicro]::Noms())
if ($noms.Count) {
    $script:SuiteMicro = $noms[-1]
    Write-Resultat 'micro choisi dans Mindset : pris par son nom' ((Get-DicteeMicro) -eq [uint32]($noms.Count - 1)) "($(Get-DicteeMicro))"
} else { Write-Host '(aucun micro sur ce PC : le choix par nom n''est pas testé)' -ForegroundColor Yellow }
$script:SuiteMicro = 'Micro qui n''existe pas'
Write-Resultat 'micro choisi mais débranché : celui de Windows' ((Get-DicteeMicro) -eq [uint32]::MaxValue) ''
$script:SuiteMicro = ''
Reset-Dictee

# 6. Whisper n'écrit rien de sensé (un silence, ses phrases inventées) ; Whisper bloqué ; on quitte pendant qu'il écrit
foreach ($cas in @('', '[BLANK_AUDIO]', 'Sous-titrage ST'' 501', '...')) {
    Write-Resultat "Whisper écrit « $cas » : rien dans la barre" ((ConvertFrom-Dictee $cas) -eq '') "($(ConvertFrom-Dictee $cas))"
}
$env:FAUX_WHISPER = 'attendre'
[void](Start-Dictee); $m = $script:DernierMicro; $m.Millis = 1500; $m.HeardVoice = $true; $m.MillisSinceVoice = 1200; Step-Dictee
$p = $script:DicteeEcrit.Process
Write-Resultat 'Whisper en cours : un nouvel appui sur le micro ne relance rien' ((Start-Dictee) -eq 'occupee') ''
Stop-Dictees
Write-Resultat 'Mindcare se ferme pendant que Whisper écrit : Whisper est arrêté, le son effacé' ($p.HasExited -and -not @(Get-ChildItem $env:TEMP -Filter "mindcare-$PID-dictee*").Count) ''
Reset-Dictee

# 7. Changer d'écran pendant que Whisper écrit : la phrase ne va pas dans une barre qui n'est plus affichée
$ui.Contenu.Children.Remove($ui.ChampRecherche)
Set-DicteeTexte 'phrase perdue'
Write-Resultat 'accueil quitté : la phrase n''est pas mise dans l''ancienne barre' ($ui.Recherche.Text -eq '' -and -not $script:DicteeMessage) "($($ui.Recherche.Text))"
[void]$ui.Contenu.Children.Insert(0, $ui.ChampRecherche)
Reset-Dictee

# 8. Le vrai Whisper (si la dictée d'Evermind est sur ce PC) : une phrase dite par la voix de Windows
$evermind = @('D:\Mindset\Donnees\Evermind\voix', (Join-Path $env:LOCALAPPDATA 'Evermind\voix')) | Where-Object { Test-Path -LiteralPath (Join-Path $_ 'ggml-small-q5_1.bin') } | Select-Object -First 1
$voixWindows = $null
try { Add-Type -AssemblyName System.Speech; $voixWindows = New-Object System.Speech.Synthesis.SpeechSynthesizer; $fr = @($voixWindows.GetInstalledVoices() | Where-Object { $_.VoiceInfo.Culture.Name -like 'fr*' })[0]; if (-not $fr) { $voixWindows = $null } else { $voixWindows.SelectVoice($fr.VoiceInfo.Name) } } catch { $voixWindows = $null }
if ($evermind -and $voixWindows) {
    Remove-Item -LiteralPath $DicteeDir -Recurse -Force
    New-Item -ItemType Directory -Force -Path $DicteeDir | Out-Null
    foreach ($f in @('whisper-cli.exe', 'ggml-small-q5_1.bin') + @(Get-ChildItem -LiteralPath $evermind -Filter '*.dll' | Where-Object { $_.Name -match '^(whisper|ggml)' } | ForEach-Object Name)) { Copy-Item -LiteralPath (Join-Path $evermind $f) -Destination $DicteeDir }
    $script:FauxSon = Join-Path $test 'phrase.wav'
    $format = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo 16000, ([System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen), ([System.Speech.AudioFormat.AudioChannel]::Mono)
    $voixWindows.SetOutputToWaveFile($script:FauxSon, $format)
    $voixWindows.Speak('Mon ordinateur s''éteint tout seul quand je joue.')
    $voixWindows.Dispose()
    [void](Start-Dictee); $m = $script:DernierMicro; $m.Millis = 3000; $m.HeardVoice = $true; $m.MillisSinceVoice = 1200; Step-Dictee
    Wait-Dispatcher { -not $script:DicteeEcrit } 60
    $vu = $ui.Recherche.Text
    Write-Resultat 'vrai Whisper : la phrase dite est écrite dans la barre' ($vu -match '(?i)ordinateur' -and $vu -match '(?i)joue') "($vu ; $($script:Journal -join ' | '))"
    Write-Host "       (Whisper a écrit : « $vu »)" -ForegroundColor DarkGray
    Reset-Dictee
} else { Write-Host '(pas de Whisper ou pas de voix française de Windows sur ce PC : le vrai essai est sauté)' -ForegroundColor Yellow }

Remove-Item Env:FAUX_WHISPER -ErrorAction SilentlyContinue
if ($script:Ok) { Write-Host "`nTout est bon." -ForegroundColor Green; exit 0 }
Write-Host "`nDes tests ont échoué." -ForegroundColor Red
exit 1
