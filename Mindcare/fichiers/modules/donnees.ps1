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
$env:MINDCARE_JOURNAL = $LogFile   # Mindcare.exe (outils\lanceur.cs) y écrit les erreurs qu'il voit passer

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
        try { return ([IO.File]::ReadAllText($p, [Text.Encoding]::UTF8) | ConvertFrom-Json) }
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
