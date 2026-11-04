# Mindcare en Rust : écrit le numéro de version dans rust\mindcare-appli\Cargo.toml (la seule source : version.json,
# Mindset.exe et Mindcare.exe le prennent là) et dans sa ligne de rust\Cargo.lock (sinon « cargo build --locked »
# refuse de construire). Rien d'autre n'est touché. Utilisé par outils\publier.ps1 -Rust.
#
#   & '.\outils\version-rust.ps1' -Version 0.90.0
param([Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Numéro de version invalide : $Version (exemple : 0.90.0)" }
$racine = Split-Path $PSScriptRoot -Parent
$cargoAppli = Join-Path $racine 'rust\mindcare-appli\Cargo.toml'
$cargoLock = Join-Path $racine 'rust\Cargo.lock'
$sansBom = New-Object Text.UTF8Encoding($false)

# Cargo.toml : la ligne « version = "..." » de la section [package] (les autres « version = » sont celles des
# bibliothèques), fins de ligne gardées
$texte = [IO.File]::ReadAllText($cargoAppli, [Text.Encoding]::UTF8)
$lignes = [regex]::Split($texte, '(?<=\n)')
$section = ''; $fait = $false
for ($i = 0; $i -lt $lignes.Count; $i++) {
    $l = $lignes[$i].Trim()
    if ($l.StartsWith('[')) { $section = $l; continue }
    if ($section -eq '[package]' -and $l -match '^version\s*=\s*"[^"]*"\s*$') { $lignes[$i] = [regex]::Replace($lignes[$i], '"[^"]*"', "`"$Version`"", 1); $fait = $true; break }
}
if (-not $fait) { throw "Ligne « version » de [package] introuvable dans $cargoAppli" }
[IO.File]::WriteAllText($cargoAppli, ($lignes -join ''), $sansBom)

# Cargo.lock : l'entrée de mindcare-appli seulement
$lock = [IO.File]::ReadAllText($cargoLock, [Text.Encoding]::UTF8)
$motif = '(?m)^(name = "mindcare-appli"\r?\nversion = ")[^"]*(")'
if (-not [regex]::IsMatch($lock, $motif)) { throw "mindcare-appli introuvable dans $cargoLock" }
[IO.File]::WriteAllText($cargoLock, [regex]::Replace($lock, $motif, "`${1}$Version`${2}"), $sansBom)
"Mindcare en Rust : version $Version écrite dans rust\mindcare-appli\Cargo.toml et rust\Cargo.lock."
