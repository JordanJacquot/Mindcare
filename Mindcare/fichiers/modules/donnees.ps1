# Mindcare : dossier de données, journal et réglages.
# Chargé par Mindcare.ps1 après mindset.ps1 ($GereParMindset, $AppRacine).

# ---------------------------------------------------------------------------
# Dossier de données
# ---------------------------------------------------------------------------
# Copie gérée par Mindset : déduit de l'emplacement du programme (jamais d'un fichier modifiable) :
#   <X>:\Mindset\Programmes\Mindcare   ->  <X>:\Mindset\Donnees\Mindcare   (Mindset installé sur un autre disque)
#   sinon (C:\Program Files\Mindset\Mindcare)  ->  %LOCALAPPDATA%\Mindset\Mindcare
# Dossier de travail (code source) : %LOCALAPPDATA%\Mindset\Mindcare (travail), pour ne jamais mélanger avec la vraie.
# Tests : MINDCARE_DONNEES.
function Get-DonneesMindset([string]$Racine) {
    $p = if ($Racine) { Split-Path $Racine -Parent } else { $null }
    $m = if ($p) { Split-Path $p -Parent } else { $null }
    if ($m -and (Split-Path $p -Leaf) -ieq 'Programmes' -and (Split-Path $m -Leaf) -ieq 'Mindset') { return [IO.Path]::Combine($m, 'Donnees', $AppNom) }
    Join-Path $env:LOCALAPPDATA "Mindset\$AppNom"
}
$DataDir = if ($EnTest -and $env:MINDCARE_DONNEES) { $env:MINDCARE_DONNEES }
           elseif ($GereParMindset) { Get-DonneesMindset $AppRacine }
           else { Join-Path $env:LOCALAPPDATA "Mindset\$AppNom (travail)" }
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
$LogFile      = Join-Path $DataDir 'journal.txt'
$SettingsFile = Join-Path $DataDir 'reglages.json'
$Utf8NoBom    = New-Object Text.UTF8Encoding($false)
$env:MINDCARE_JOURNAL = $LogFile   # Mindset.exe (outils\lanceur.cs) y écrit les erreurs qu'il voit passer

# Journal : une ligne datée par événement. Gardé petit : au-delà de 1 Mo, seulement les 3000 dernières lignes.
# Il sert à aider à distance : chaque échec utile pour comprendre un problème y est noté.
try {
    $fi = New-Object IO.FileInfo $LogFile
    if ($fi.Exists -and $fi.Length -gt 1MB) {
        [IO.File]::WriteAllLines($LogFile, [string[]]([IO.File]::ReadAllLines($LogFile, [Text.Encoding]::UTF8) | Select-Object -Last 3000), $Utf8NoBom)
    }
} catch {}
function Write-Log([string]$Message) {
    try { [IO.File]::AppendAllText($LogFile, "$(Get-Date -Format s) $Message`r`n", $Utf8NoBom) } catch {}
}

# ---------------------------------------------------------------------------
# Le filet contre les plantages. Une erreur dans un clic ou une minuterie n'allait que dans la console (personne ne la
# voit) ; une erreur qui en sortait (throw, ou pendant une fenêtre ouverte dans un try) fermait Mindcare sans un mot.
# - Chaque clic, minuterie ou événement de l'appli passe par Protect-Action : l'erreur est attrapée là, notée en entier
#   au journal (message, script, ligne), la personne est prévenue une seule fois, et Mindcare continue.
# - Ce qui passerait quand même (Register-FiletErreurs : une erreur de WPF lui-même, ou un code oublié) est noté de la
#   même façon et Mindcare continue. (Sans Protect-Action, une minuterie qui se trompe s'arrêterait pour de bon : WPF ne
#   la relance pas après une erreur. D'où Protect-Action sur chacune.) Seule une erreur grave (mémoire épuisée) ferme
#   Mindcare, avec un message (Mindcare.ps1, après Dispatcher.Run).
# Le démarrage n'est pas concerné (il se fait avant Dispatcher.Run : ses erreurs restent visibles comme avant), ni les
# travaux de fond (ils tournent dans leur propre PowerShell, qui garde ses erreurs).
# ---------------------------------------------------------------------------
$script:Filet = @{ Vues = @{}; Montre = $false; Fatale = '' }

# L'erreur en une ligne pour le journal : message, script et ligne, puis les appels qui y ont mené
function Get-TexteErreur($Erreur) {
    $er = if ($Erreur -is [System.Management.Automation.ErrorRecord]) { $Erreur } else { $null }
    $e = if ($er) { $er.Exception } else { $Erreur }
    while ($e -is [System.Management.Automation.MethodInvocationException] -and $e.InnerException) { $e = $e.InnerException }
    $texte = "$($e.GetType().Name) : $($e.Message)"
    for ($x = $e; $x -and -not $er; $x = $x.InnerException) { if ($x -is [System.Management.Automation.IContainsErrorRecord]) { $er = $x.ErrorRecord } }
    if ($er -and $er.InvocationInfo -and $er.InvocationInfo.ScriptName) { $texte += " ($(Split-Path $er.InvocationInfo.ScriptName -Leaf) ligne $($er.InvocationInfo.ScriptLineNumber) : $($er.InvocationInfo.Line.Trim()))" }
    if ($er -and $er.ScriptStackTrace) { $texte += " ; appels : $(($er.ScriptStackTrace -split "`r?`n" | Where-Object { $_ } | Select-Object -First 4) -join ' < ')" }
    elseif ($e.StackTrace) { $texte += " ; $(($e.StackTrace -split "`r?`n" | Select-Object -First 3 | ForEach-Object { $_.Trim() }) -join ' < ')" }
    $texte
}

# Le message simple, une seule fois par lancement (posé après l'erreur, pour ne pas la bloquer ; jamais en test)
function Show-ProblemeImprevu {
    if ($EnTest) { return }
    [void][System.Windows.Threading.Dispatcher]::CurrentDispatcher.BeginInvoke([action]{
        try { [System.Windows.MessageBox]::Show("Un petit problème est survenu, l'appli continue.`n`nSi ça se répète, utilise « Signaler un problème ».", $AppNom, 'OK', 'Information') | Out-Null } catch {}
    })
}

# Note une erreur imprévue : en entier les 3 premières fois, puis une fois toutes les 100 (une minuterie qui se trompe à
# chaque tour ne remplit pas le journal) ; le message n'est montré qu'une fois
function Write-ErreurImprevue($Erreur, [switch]$SansMessage) {
    try {
        $texte = Get-TexteErreur $Erreur
        $cle = if ($texte.Length -gt 300) { $texte.Substring(0, 300) } else { $texte }
        $n = 1 + [int]$script:Filet.Vues[$cle]; $script:Filet.Vues[$cle] = $n
        if ($n -le 3 -or $n % 100 -eq 0) { Write-Log "Erreur imprévue$(if ($n -gt 1) { " (vue $n fois)" }) : $texte" }
        if ($EnTest -and $n -eq 1) { [Console]::Error.WriteLine("Erreur imprévue : $texte") }   # (les tests la voient)
        if (-not $SansMessage -and -not $script:Filet.Montre -and -not $script:Quitting) { $script:Filet.Montre = $true; Show-ProblemeImprevu }
    } catch {}
}

# Un clic, une minuterie, un événement protégés : $b.Add_Click((Protect-Action { ... }))
# (une fermeture : elle ne voit que ses deux variables ; $Action, lui, s'exécute comme avant, dans le script)
function Protect-Action([scriptblock]$Action) {
    $note = ${function:Write-ErreurImprevue}
    { param($s, $e) try { & $Action $s $e } catch { & $note $_ } }.GetNewClosure()
}

function Register-FiletErreurs {
    [System.Windows.Threading.Dispatcher]::CurrentDispatcher.Add_UnhandledException({
        param($s, $ev)
        try {
            $grave = $false
            for ($x = $ev.Exception; $x; $x = $x.InnerException) { if ($x -is [OutOfMemoryException] -or $x -is [AccessViolationException]) { $grave = $true } }
            if ($grave) {
                Write-ErreurImprevue $ev.Exception -SansMessage
                if (-not $script:Filet.Fatale) { $script:Filet.Fatale = Get-TexteErreur $ev.Exception; Write-Log 'Mindcare doit se fermer après cette erreur' }
                return
            }
            Write-ErreurImprevue $ev.Exception
            $ev.Handled = $true
        } catch {}
    })
}

# Écrit un fichier JSON sans risque de le perdre : à côté, puis remplacé d'un coup (l'ancien reste dans « .bak »)
function Save-JsonFile([string]$Path, $Object) {
    $tmp = "$Path.tmp"
    [IO.File]::WriteAllText($tmp, (ConvertTo-Json -InputObject $Object -Depth 8), $Utf8NoBom)
    if ([IO.File]::Exists($Path)) { [IO.File]::Replace($tmp, $Path, "$Path.bak") } else { [IO.File]::Move($tmp, $Path) }
}

# Lit un fichier JSON. Un fichier abîmé est mis de côté (rien n'est jamais effacé) et on repart de la sauvegarde.
function Read-JsonFile([string]$Path) {
    foreach ($p in $Path, "$Path.bak") {
        if (-not [IO.File]::Exists($p)) { continue }
        try {
            # (un fichier vide ne lève aucune erreur : sans ce contrôle, il passait pour un historique vide et écrasait le .bak)
            $texte = [IO.File]::ReadAllText($p, [Text.Encoding]::UTF8)
            if ([string]::IsNullOrWhiteSpace($texte) -or $texte.Trim() -eq 'null') { throw 'fichier vide' }
            return ($texte | ConvertFrom-Json)
        }
        catch {
            Write-Log "Fichier illisible ($p) : $_"
            try { Copy-Item -LiteralPath $p -Destination "$p.abime-$(Get-Date -Format yyyyMMdd-HHmmss)" -Force } catch {}
        }
    }
    $null
}

# ---------------------------------------------------------------------------
# Réglages (reglages.json)
# ---------------------------------------------------------------------------
$script:Settings = @{}
$j = Read-JsonFile $SettingsFile
if ($j) { foreach ($p in $j.PSObject.Properties) { $script:Settings[$p.Name] = $p.Value } }
function Get-Setting([string]$Nom, $Defaut) { if ($script:Settings.ContainsKey($Nom)) { $script:Settings[$Nom] } else { $Defaut } }
function Set-Setting([string]$Nom, $Valeur) {
    $script:Settings[$Nom] = $Valeur
    try { Save-JsonFile $SettingsFile $script:Settings } catch { Write-Log "Réglages non enregistrés : $_" }
}
