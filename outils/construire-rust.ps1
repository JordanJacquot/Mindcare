# Mindcare en Rust (étape 4.5) : construit les deux programmes en Rust et assemble le dossier de Mindcare prêt à
# zipper. Rien ici n'est livré aux utilisateurs pour l'instant : Mindcare publiée reste celle en PowerShell
# (construire.ps1). Lancé aussi par la CI de GitHub (.github\workflows\tests.yml).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File .\outils\construire-rust.ps1 -Sortie <dossier>
#
#   <dossier>\Mindcare\
#     Mindset.exe       <- le programme (rust\mindcare-appli) : la fenêtre de Mindcare dans Mindset. Même nom pour
#                          toute la suite, description « Mindset » : une seule ligne « Mindset » dans le Gestionnaire
#                          des tâches (contrat-appli, partie 4)
#     Mindcare.exe      <- le relais (rust\mindcare-relais), pour ce qui appelle encore Mindcare.exe
#     version.json      <- { "appli": "mindcare", "version": <version du Cargo.toml de mindcare-appli>,
#                            "mindset_min": "2.15.0" } (contrat-appli, partie 10 : une seule source pour la version)
#     LISEZMOI.txt      <- le même que celui de Mindcare (Mindcare\LISEZMOI.txt)
#   (enquetes.json et les images viennent à l'étape 5)
#
# Construction en release seulement, avec les versions du Cargo.lock (--locked) : un exe de mise au point n'est
# jamais livré (règle 5 de docs\regles-tauri.md, dépôt Mindset). Le dossier <dossier>\Mindcare est vidé d'abord.
# -SansConstruire : seulement l'assemblage, avec les exe déjà construits dans rust\target\release.
param(
    [Parameter(Mandatory = $true)][string]$Sortie,
    [switch]$SansConstruire
)

$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$rust = Join-Path $racine 'rust'
# La première version de Mindset qui accueille une appli en Rust (version.json, classe « Tauri Window »)
$MindsetMin = '2.15.0'

# La version de Mindcare : la ligne « version = "x.y.z" » de la section [package] du Cargo.toml de mindcare-appli
$section = ''
$version = $null
foreach ($ligne in [IO.File]::ReadAllLines((Join-Path $rust 'mindcare-appli\Cargo.toml'), [Text.Encoding]::UTF8)) {
    $l = $ligne.Trim()
    if ($l.StartsWith('[')) { $section = $l; continue }
    if ($section -eq '[package]' -and $l -match '^version\s*=\s*"([^"]*)"') { $version = $Matches[1]; break }
}
if ($version -notmatch '^\d+(\.\d+){1,3}$') { throw "Version de mindcare-appli introuvable ou mal écrite : « $version »." }

if (-not $SansConstruire) {
    # cargo écrit ses messages normaux sur la sortie d'erreur : seul son code de sortie compte
    $avant = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    Push-Location $rust
    try {
        & cargo build --release --locked -p mindcare-appli -p mindcare-relais 2>&1 | ForEach-Object { "$_" }
        $code = $LASTEXITCODE
    } finally {
        Pop-Location
        $ErrorActionPreference = $avant
    }
    if ($code) { throw "La construction a échoué (cargo build, code $code)." }
}

$construits = Join-Path $rust 'target\release'
$dossier = Join-Path ([IO.Path]::GetFullPath($Sortie)) 'Mindcare'
if (Test-Path -LiteralPath $dossier) { Remove-Item -LiteralPath $dossier -Recurse -Force }
New-Item -ItemType Directory -Force -Path $dossier | Out-Null
foreach ($exe in 'Mindset.exe', 'Mindcare.exe') {
    $chemin = Join-Path $construits $exe
    if (-not (Test-Path -LiteralPath $chemin -PathType Leaf)) { throw "$exe introuvable dans $construits : la construction ne l'a pas fait." }
    Copy-Item -LiteralPath $chemin -Destination (Join-Path $dossier $exe)
}

# version.json en UTF-8 sans marque, comme le lit Mindset (Read-VersionJson) et suite-commun
$json = [ordered]@{ appli = 'mindcare'; version = $version; mindset_min = $MindsetMin } | ConvertTo-Json -Compress
[IO.File]::WriteAllText((Join-Path $dossier 'version.json'), $json, (New-Object Text.UTF8Encoding($false)))
Copy-Item -LiteralPath (Join-Path $racine 'Mindcare\LISEZMOI.txt') -Destination (Join-Path $dossier 'LISEZMOI.txt')

"OK : $dossier (Mindcare $version)"
Get-ChildItem -LiteralPath $dossier | ForEach-Object { "  $($_.Name) ($($_.Length) octets)" }
