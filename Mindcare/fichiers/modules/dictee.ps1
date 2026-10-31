# Mindcare : la dictée. On appuie sur le micro de la barre de l'accueil, on parle, et la phrase s'écrit dans la barre :
# on peut la corriger avant d'appuyer sur Entrée. Whisper (whisper.cpp, modèle « small », en français) écrit ce qui a été
# dit, sur ce PC : rien ne part sur Internet, et le son est effacé dès que la phrase est écrite.
#
# Installé à la demande (environ 200 Mo, une seule fois) dans le dossier de données de Mindcare, avec le téléchargement de
# l'assistant (ia.ps1, Start-Telechargement). Les mêmes fichiers que la dictée d'Evermind. Whisper n'est lancé que le temps
# d'écrire une phrase (quelques secondes) : rien ne reste allumé ensuite. Il est arrêté avec Mindcare (natif.cs).
# Le micro : celui choisi dans les réglages de Mindset (suite.json, par son nom), sinon celui de Windows.
# L'affichage (le bouton micro, les messages) est dans contenu.ps1. Chargé par Mindcare.ps1 après ia.ps1.

$DicteeDir    = Join-Path $DataDir 'dictee'
$DicteeExe    = Join-Path $DicteeDir 'whisper-cli.exe'
$DicteeModele = Join-Path $DicteeDir 'ggml-small-q5_1.bin'
$DicteeFichiers = @(
    @{ Nom = 'programme de dictée'; Url = 'https://github.com/ggml-org/whisper.cpp/releases/download/v1.9.2/whisper-bin-x64.zip'
       Sha256 = '49DCC16DE826F20BD53D44F947A1AE49DFA81F86CAD67A64D80820CB192D674A'; Taille = 8194445
       Extraire = '^(whisper-cli\.exe|whisper\.dll|ggml[^/]*\.dll)$'; Garder = $DicteeExe }
    @{ Nom = 'français de la dictée'; Url = 'https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-small-q5_1.bin'
       Sha256 = 'AE85E4A935D7A567BD102FE55AFC16BB595BDB618E11B2FC7591BC08120411BB'; Taille = 190085487; Garder = $DicteeModele }
)
$DicteeDll = 'whisper.dll', 'ggml.dll', 'ggml-base.dll'   # (sans elles whisper-cli ne démarre pas : une installation coupée est à réparer)
# Les mots qu'on s'attend à entendre aident Whisper (sans accents : ils passent mal sur la ligne de commande)
$DicteeAttendu = "Mon ordinateur ne demarre plus, l'ecran reste noir, le PC s'eteint quand je joue, le wifi coupe, la souris, le clavier, Windows."
$script:Dictee = $null       # l'écoute en cours : { Micro, Debut }
$script:DicteeEcrit = $null  # Whisper écrit la phrase : { Process, Wav, Sortie, Debut }
$script:DicteeDl = $null     # le téléchargement (voir Start-Telechargement)

function Test-DicteeMoteur {
    if (-not (Test-Path -LiteralPath $DicteeExe)) { return $false }
    -not @($DicteeDll | Where-Object { -not (Test-Path -LiteralPath (Join-Path $DicteeDir $_)) }).Count
}
function Test-DicteeInstallee { (Test-DicteeMoteur) -and (Test-Path -LiteralPath $DicteeModele) }
function Get-DicteeManquants { @($DicteeFichiers | Where-Object { if ($_.Extraire) { -not (Test-DicteeMoteur) } else { -not (Test-Path -LiteralPath $_.Garder) } }) }
function Get-DicteeTaille { [math]::Ceiling(((Get-DicteeManquants | ForEach-Object { $_.Taille }) | Measure-Object -Sum).Sum / 1MB) }

function Start-DicteeInstall {
    if ($script:DicteeDl -and -not $script:DicteeDl.Fini) { return }
    $script:DicteeDl = Start-Telechargement (Get-DicteeManquants) $DicteeDir 50MB
    if (-not $script:DicteeDl.Fini) { Write-Log "Installation de la dictée ($([math]::Round($script:DicteeDl.Total / 1MB)) Mo)" }
}

# Le numéro du micro choisi dans Mindset, ou 0xFFFFFFFF (celui de Windows) s'il n'y en a pas, ou s'il est débranché
function Get-DicteeMicro {
    $nom = [string](Read-Suite 3).Micro   # (pris plus d'une demi-seconde : celui de Windows, cette fois)
    if (-not $nom) { return [uint32]::MaxValue }
    $i = [array]::IndexOf(@([MindcareMicro]::Noms()), $nom)
    if ($i -ge 0) { return [uint32]$i }
    Write-Log "Dictée : le micro « $nom » choisi dans Mindset n'est pas branché, celui de Windows à la place"
    [uint32]::MaxValue
}
function New-DicteeMicro { New-Object MindcareMicro }   # (les tests le remplacent par un faux micro)
function Test-DicteeMicroPresent { [MindcareMicro]::HasDevice() }

function Test-DicteeEcoute { [bool]$script:Dictee }
function Test-DicteeOccupee { [bool]($script:Dictee -or $script:DicteeEcrit) }

# Le bouton micro : écouter, ou arrêter d'écouter (deuxième appui). Renvoie ce qui se passe, pour l'affichage :
# 'installer' (pas encore installée), 'ecoute', 'fin' (deuxième appui), 'occupee' (Whisper écrit encore), ou un message d'erreur
function Start-Dictee {
    if ($script:Dictee) { Stop-Dictee; return 'fin' }
    if ($script:DicteeEcrit) { return 'occupee' }
    if (-not (Test-DicteeInstallee)) { return 'installer' }
    $micro = New-DicteeMicro
    $err = -1
    try { if (Test-DicteeMicroPresent) { $err = $micro.Start((Get-DicteeMicro)) } } catch { Write-Log "Dictée : micro ($_)" }
    if ($err -ne 0) {
        Write-Log "Dictée : micro indisponible (code $err)"
        try { $micro.Stop() } catch {}
        return 'Je n''ai pas accès au micro. Vérifie qu''il est branché. Dans les paramètres de Windows (Confidentialité, Microphone), les applications de bureau doivent avoir le droit de l''utiliser.'
    }
    $script:Dictee = @{ Micro = $micro; Debut = [Diagnostics.Stopwatch]::StartNew() }
    if (-not $script:DicteeMinuterie) {
        try {
            $t = New-Object System.Windows.Threading.DispatcherTimer
            $t.Interval = [TimeSpan]::FromMilliseconds(80)
            $action = { Step-Dictee }
            if (Get-Command Protect-Action -ErrorAction SilentlyContinue) { $action = Protect-Action $action }
            $t.Add_Tick($action)
            $script:DicteeMinuterie = $t
        } catch {}
    }
    if ($script:DicteeMinuterie) { $script:DicteeMinuterie.Start() }
    Write-Log 'Dictée : écoute'
    'ecoute'
}

# Toutes les 80 ms pendant l'écoute : on s'arrête une seconde après la fin de la phrase, après 7 secondes sans rien entendre,
# ou au bout de 25 secondes. Micro débranché pendant l'écoute : plus aucun son n'arrive, arrêt au bout de 3 secondes.
function Step-Dictee {
    $d = $script:Dictee
    if (-not $d) { if ($script:DicteeMinuterie) { $script:DicteeMinuterie.Stop() }; return }
    $m = $d.Micro
    Update-DicteeNiveau ([double]$m.Level)
    $sansSon = $d.Debut.ElapsedMilliseconds - $m.Millis
    if ($sansSon -gt 3000) { $d.MicroPerdu = $true; Stop-Dictee; return }
    $fin = ($m.HeardVoice -and $m.MillisSinceVoice -gt 1000) -or (-not $m.HeardVoice -and $m.Millis -gt 7000) -or $m.Millis -gt 25000 -or $d.Debut.ElapsedMilliseconds -gt 30000
    if ($fin) { Stop-Dictee }
}

# Fin de l'écoute : Whisper écrit ce qui a été dit (en arrière-plan), sinon un message
function Stop-Dictee {
    $d = $script:Dictee
    if (-not $d) { return }
    $script:Dictee = $null
    if ($script:DicteeMinuterie) { $script:DicteeMinuterie.Stop() }
    try { $d.Micro.Stop() } catch {}
    if (-not $d.Micro.HeardVoice) {
        Write-Log "Dictée : $(if ($d.MicroPerdu) { 'plus aucun son ne vient du micro' } else { 'rien entendu' })"
        Show-DicteeFin $(if ($d.MicroPerdu) { 'Je n''entends plus le micro. Vérifie qu''il est bien branché, puis réessaie.' } else { 'Je n''ai rien entendu. Appuie sur le micro et parle un peu plus fort.' })
        return
    }
    $wav = Join-Path $env:TEMP "mindcare-$PID-dictee.wav"
    try { $d.Micro.SaveWav($wav) } catch { Write-Log "Dictée : son non gardé ($_)"; Remove-DicteeFichier $wav; Show-DicteeFin 'La dictée n''a pas marché cette fois. Réessaie, ou écris ta phrase.'; return }
    Start-DicteeEcriture $wav
}

# Le son n'est jamais gardé : effacé dès que Whisper l'a lu (ou s'il n'a pas pu)
function Remove-DicteeFichier([string]$Fichier) { if ($Fichier) { try { [IO.File]::Delete($Fichier) } catch {} } }

function Get-DicteeFils { [math]::Max(2, [math]::Min(8, [int]([Environment]::ProcessorCount / 2))) }

function Start-DicteeEcriture([string]$Wav) {
    $sortie = Join-Path $env:TEMP "mindcare-$PID-dictee"
    Remove-DicteeFichier "$sortie.txt"
    $si = New-Object Diagnostics.ProcessStartInfo $DicteeExe
    $si.Arguments = "-m `"$DicteeModele`" -f `"$Wav`" -l fr -nt -np -t $(Get-DicteeFils) -otxt -of `"$sortie`" --prompt `"$DicteeAttendu`""
    $si.UseShellExecute = $false; $si.CreateNoWindow = $true; $si.WorkingDirectory = $DicteeDir
    try { $p = [Diagnostics.Process]::Start($si) }
    catch { Write-Log "Dictée : Whisper ne démarre pas ($($_.Exception.Message))"; Remove-DicteeFichier $Wav; Show-DicteeFin 'La dictée n''a pas pu démarrer. Réessaie, ou écris ta phrase.'; return }
    # (arrêté avec Mindcare, même fermée de force)
    try { if (-not [MindcareNatif]::ArreterAvecMindcare($p.Handle)) { Write-Log "Dictée : pas reliée à Mindcare ($([MindcareNatif]::Erreur))" } } catch {}
    $script:DicteeEcrit = @{ Process = $p; Wav = $Wav; Sortie = "$sortie.txt"; Debut = [Diagnostics.Stopwatch]::StartNew() }
    Show-DicteeEcrit
    if (-not $script:DicteeSuivi) {
        try {
            $t = New-Object System.Windows.Threading.DispatcherTimer
            $t.Interval = [TimeSpan]::FromMilliseconds(100)
            $action = { Step-DicteeEcriture }
            if (Get-Command Protect-Action -ErrorAction SilentlyContinue) { $action = Protect-Action $action }
            $t.Add_Tick($action)
            $script:DicteeSuivi = $t
        } catch {}
    }
    if ($script:DicteeSuivi) { $script:DicteeSuivi.Start() }
}

# Whisper a fini ? (60 secondes au plus) La phrase va dans la barre de recherche
function Step-DicteeEcriture {
    $e = $script:DicteeEcrit
    if (-not $e) { if ($script:DicteeSuivi) { $script:DicteeSuivi.Stop() }; return }
    if (-not $e.Process.HasExited) {
        if ($e.Debut.Elapsed.TotalSeconds -lt 60) { return }
        try { $e.Process.Kill() } catch {}
        Write-Log 'Dictée : Whisper trop long, arrêté'
    }
    $script:DicteeEcrit = $null
    if ($script:DicteeSuivi) { $script:DicteeSuivi.Stop() }
    Remove-DicteeFichier $e.Wav
    $brut = ''
    try { if (Test-Path -LiteralPath $e.Sortie) { $brut = [IO.File]::ReadAllText($e.Sortie, [Text.Encoding]::UTF8) } } catch {}
    Remove-DicteeFichier $e.Sortie
    $texte = ConvertFrom-Dictee $brut
    $code = try { $e.Process.ExitCode } catch { '?' }
    # (la phrase n'est pas écrite dans le journal, seulement sa longueur, comme pour l'assistant)
    Write-Log "Dictée : $([math]::Round($e.Debut.Elapsed.TotalSeconds, 1)) s, $($texte.Length) caractères$(if ($code -ne 0) { " (code $code)" })"
    if (-not $texte) { Show-DicteeFin 'Je n''ai pas compris ce que tu as dit. Réessaie en parlant plus près du micro, ou écris ta phrase.'; return }
    Set-DicteeTexte $texte
}

# Ce que Whisper a écrit, nettoyé : sans les bruits notés entre crochets ou parenthèses, ni les phrases qu'il invente sur
# un silence (« Sous-titrage ST' 501 », « Merci d'avoir regardé »...)
function ConvertFrom-Dictee([string]$Brut) {
    $t = ($Brut -replace '\[[^\]]*\]|\([^)]*\)|\*[^*]*\*', ' ' -replace '\s+', ' ').Trim()
    if ($t -match '(?i)sous-titr|amara\.org|merci d.avoir regard|abonnez-vous') { return '' }
    if ($t -notmatch '\w') { return '' }
    $t
}

# Mindcare se ferme, ou on quitte l'accueil : on arrête d'écouter et Whisper, sans suite
function Stop-DicteeSansSuite {
    $d = $script:Dictee; $script:Dictee = $null
    if ($script:DicteeMinuterie) { try { $script:DicteeMinuterie.Stop() } catch {} }
    if ($d) { try { $d.Micro.Stop() } catch {} }
    $e = $script:DicteeEcrit; $script:DicteeEcrit = $null
    if ($script:DicteeSuivi) { try { $script:DicteeSuivi.Stop() } catch {} }
    if ($e) {
        try { if (-not $e.Process.HasExited) { $e.Process.Kill(); [void]$e.Process.WaitForExit(2000) } } catch {}
        Remove-DicteeFichier $e.Wav; Remove-DicteeFichier $e.Sortie
    }
}

function Stop-Dictees {
    Stop-DicteeSansSuite
    try { if ($script:DicteeDl -and $script:DicteeDl.PS) { $script:DicteeDl.PS.Stop() } } catch {}
}
