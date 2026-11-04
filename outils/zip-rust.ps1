# Mindcare en Rust : fait Mindcare.zip à partir du dossier assemblé par outils\construire-rust.ps1, au même format que
# les autres zips de la suite (Compress-Archive du dossier « Mindcare » : le dossier « dossier_zip » de l'entrée
# apps.json, avec Mindcare.exe dedans), puis le vérifie (outils\verifier-zip-rust.ps1). Utilisé par la construction
# GitHub (.github\workflows\release.yml) ; sur le PC, seulement pour un essai : le zip publié vient toujours de GitHub.
#
#   & '.\outils\zip-rust.ps1' -Dossier <sortie>\Mindcare -Zip <dossier>\Mindcare.zip [-VerifierImports <...>]
#
# Le zip déjà là est remplacé. Code de sortie 1 (et le zip effacé) si la vérification échoue.
param(
    [Parameter(Mandatory = $true)][string]$Dossier,
    [Parameter(Mandatory = $true)][string]$Zip,
    [string]$VerifierImports = ''
)
$ErrorActionPreference = 'Stop'
$Dossier = [IO.Path]::GetFullPath($Dossier).TrimEnd('\')
$Zip = [IO.Path]::GetFullPath($Zip)
if ((Split-Path $Dossier -Leaf) -cne 'Mindcare') { throw "Le dossier à zipper doit s'appeler Mindcare : $Dossier" }
if (-not (Test-Path -LiteralPath (Join-Path $Dossier 'Mindcare.exe') -PathType Leaf)) { throw "Mindcare.exe introuvable dans $Dossier" }
New-Item -ItemType Directory -Force -Path (Split-Path $Zip -Parent) | Out-Null
Remove-Item -LiteralPath $Zip -Force -ErrorAction SilentlyContinue
Compress-Archive -LiteralPath $Dossier -DestinationPath $Zip -Force
$p = @{ Zip = $Zip }
if ($VerifierImports) { $p.VerifierImports = $VerifierImports }
& (Join-Path $PSScriptRoot 'verifier-zip-rust.ps1') @p
if ($LASTEXITCODE) { Remove-Item -LiteralPath $Zip -Force -ErrorAction SilentlyContinue; exit 1 }
exit 0
