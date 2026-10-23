# Publie une nouvelle version de Mindcare sur GitHub, avec sa signature. Mindset la propose ensuite (il installe et met
# à jour Mindcare à partir du zip signé de la Release).
#
#   .\outils\publier.ps1 -Version 0.1.0 -Notes "Ce qui change, en mots simples"
#
# Avant de publier : syntaxe de tous les fichiers, fenêtre (interface.xaml), tirets cadratins, clés publiques (les mêmes que
# Mindset), construction, puis le test automatique (outils\tester.ps1). -SansTest le saute (urgence seulement).
# Le zip est signé avec la clé de la suite (Mindset\outils\signer.ps1) : sans signature, rien n'est publié.
# Les notes de version ne citent jamais un thème perso (Mario, Pokémon, Dofus...).

param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Notes = '',
    [switch]$SansTest
)
$ErrorActionPreference = 'Stop'
$racine  = Split-Path $PSScriptRoot -Parent
$app     = Join-Path $racine 'Mindcare'
$ps1     = Join-Path $app 'fichiers\Mindcare.ps1'
$zip     = Join-Path $racine 'release\Mindcare.zip'
$mindset = Join-Path (Split-Path $racine -Parent) 'Mindset'   # le dépôt de Mindset, à côté de celui-ci
$utf8    = New-Object Text.UTF8Encoding($true)

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Numéro de version invalide : $Version (exemple : 0.1.0)" }

# Un numéro déjà publié ne doit jamais être réutilisé (GitHub garde l'ancien zip en cache)
Push-Location $racine
try {
    $ErrorActionPreference = 'Continue'
    git rev-parse -q --verify "refs/tags/v$Version" 2>$null | Out-Null; $tagExiste = ($LASTEXITCODE -eq 0)
    gh release view "v$Version" 2>$null | Out-Null; $releaseExiste = ($LASTEXITCODE -eq 0)
} finally { Pop-Location; $ErrorActionPreference = 'Stop' }
if ($tagExiste -or $releaseExiste) { throw "La version $Version existe déjà. Choisis un numéro plus grand." }

# Vérifications
$problemes = @()
# Syntaxe, encodage, fenêtre, tirets longs, licence : outils\verifier.ps1 (le même que la CI de GitHub)
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verifier.ps1')
if ($LASTEXITCODE) { $problemes += 'Vérifications : voir les problèmes listés par outils\verifier.ps1 ci-dessus' }
# Clés publiques : exactement la liste de Mindset (mêmes fichiers, même contenu)
$cles = Join-Path $app 'fichiers\cles-publiques'
$clesMindset = Join-Path $mindset 'Mindset\fichiers\cles-publiques'
function Get-EmpreinteCles([string]$Dossier) { @(Get-ChildItem -LiteralPath $Dossier -File -Force -ErrorAction SilentlyContinue | Sort-Object Name | ForEach-Object { "$($_.Name)=$((Get-FileHash -LiteralPath $_.FullName).Hash)" }) -join ';' }
$empreinte = Get-EmpreinteCles $cles
if (-not $empreinte -or $empreinte -ne (Get-EmpreinteCles $clesMindset)) {
    $problemes += "Clés publiques : Mindcare\fichiers\cles-publiques absentes ou différentes de celles de Mindset ($clesMindset)"
}
if ($problemes) { $problemes | ForEach-Object { Write-Host $_ -ForegroundColor Red }; throw 'Publication annulée : corrige les points ci-dessus.' }

# Un programme (git, gh) : seul son code de sortie compte. Ses avertissements (« LF will be replaced by CRLF »...) ne
# sont pas des échecs : sans « Continue », PowerShell les prenait pour des erreurs et arrêtait la publication.
function Invoke-Native([string]$Exe, [string[]]$Arguments) {
    $ErrorActionPreference = 'Continue'
    & $Exe @Arguments 2>&1 | ForEach-Object { "$_" }
    if ($LASTEXITCODE) { throw "Échec : $Exe $($Arguments -join ' ')" }
}

# 1. Numéro de version : dans l'appli (variable et en-tête) et dans le programme
$c = [IO.File]::ReadAllText($ps1, [Text.Encoding]::UTF8)
$c = [regex]::Replace($c, "(?m)^\`$AppVersion = '[^']*'", "`$AppVersion = '$Version'")
$c = [regex]::Replace($c, '(?m)^(    Mindcare) [\d\.]+(\r?)$', "`$1 $Version`$2")
[IO.File]::WriteAllText($ps1, $c, $utf8)
foreach ($cs in (Join-Path $PSScriptRoot 'lanceur.cs'), (Join-Path $PSScriptRoot 'relais.cs')) {   # (Mindset.exe et le relais Mindcare.exe)
    $l = [IO.File]::ReadAllText($cs, [Text.Encoding]::UTF8)
    $l = [regex]::Replace($l, 'Assembly(File)?Version\("[\d\.]+"\)', { param($m) "Assembly$($m.Groups[1].Value)Version(`"$Version.0`")" })
    [IO.File]::WriteAllText($cs, $l, $utf8)
}

# 2. Construction, test automatique (avec le vrai programme), puis signature
& (Join-Path $PSScriptRoot 'construire.ps1')
if (-not $SansTest) {
    & (Join-Path $PSScriptRoot 'tester.ps1')
    if ($LASTEXITCODE) { throw 'Publication annulée : le test automatique a échoué.' }
}
Remove-Item -LiteralPath "$zip.sig" -ErrorAction SilentlyContinue   # jamais l'ancienne signature d'un autre zip
& (Join-Path $mindset 'outils\signer.ps1') -Fichier $zip
if (-not (Test-Path -LiteralPath "$zip.sig")) { throw 'Publication annulée : le zip n''a pas pu être signé.' }

# 2 bis. Le catalogue pour Mindcare Mobile (le téléphone le télécharge tout seul), construit par construire.ps1, avec
# son empreinte dans les notes (« SHA-256 du catalogue : ... », lue par l'appli Android, maj/MajCatalogue.kt)
$catalogue = Join-Path $racine 'release\catalogue.zip'
if (-not (Test-Path -LiteralPath $catalogue)) { throw 'Publication annulée : release\catalogue.zip n''a pas été construit.' }
$shaCatalogue = (Get-FileHash -LiteralPath $catalogue -Algorithm SHA256).Hash.ToLowerInvariant()

# 3. Commit, tag, Release (le zip et sa signature sont seulement joints à la Release : release\ n'est pas suivi par git)
Push-Location $racine
try {
    Invoke-Native git @('add', '-A')
    if (git status --porcelain) { Invoke-Native git @('commit', '-m', "Mindcare $Version", '-m', $(if ($Notes) { $Notes } else { "Version $Version" })) }
    Invoke-Native git @('tag', "v$Version")
    Invoke-Native git @('push', 'origin', 'HEAD', '--tags')
    if (-not $Notes) { $Notes = "Mindcare $Version" }
    $Notes += "`n`nLe catalogue de pannes est aussi pour Mindcare Mobile (il se met à jour tout seul).`n`nSHA-256 du catalogue : $shaCatalogue"
    Invoke-Native gh @('release', 'create', "v$Version", $zip, "$zip.sig", $catalogue, '--title', "Mindcare $Version", '--notes', $Notes)
} finally { Pop-Location }
"Mindcare $Version publiée."
