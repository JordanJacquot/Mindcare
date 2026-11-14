# Mindcare en Rust : vérifie le zip à publier (Mindcare.zip), sans rien lancer. Utilisé par la construction GitHub
# (.github\workflows\release.yml, job « Mindcare en Rust »), par outils\zip-rust.ps1 et par outils\publier.ps1 -Rust,
# juste avant la signature.
#
#   & '.\outils\verifier-zip-rust.ps1' -Zip <Mindcare.zip> [-Version 0.90.0] [-VerifierImports <verifier-imports.ps1>]
#
# Le zip doit être celui que Mindset attend pour une appli en Rust (docs\contrat-appli.md et docs\regles-tauri.md du
# dépôt Mindset) :
#   - exactement un dossier « Mindcare » et dedans exactement Mindset.exe, Mindcare.exe, MindcareAide.exe (l'Aide de
#     Mindcare : sans elle, l'installateur de Mindset ne crée pas sa tâche), version.json, LISEZMOI.txt et
#     enquetes.json (rien d'autre : ni script, ni DLL, ni fichier en plus, ni version.json à la racine du zip) ;
#   - version.json en UTF-8 sans marque : appli « mindcare », version celle du Cargo.toml de mindcare-appli (ou
#     -Version, qui doit alors être la même), mindset_min 2.15.0 ;
#   - enquetes.json identique à Mindcare\fichiers\enquetes.json ;
#   - fiches de version : Mindset.exe décrit « Mindset » (une seule ligne « Mindset » dans le Gestionnaire des tâches),
#     Mindcare.exe et MindcareAide.exe décrivent « Mindcare », produits « Mindset » et « Mindcare », version celle de
#     version.json ;
#   - les exe livrés ne connaissent pas la construction d'essai (outils\verifier-sans-essai.ps1) ;
#   - avec -VerifierImports (outils\verifier-imports.ps1 du dépôt Mindset) : aucun runtime C à installer à part.
# Code de sortie 0 si tout va bien, 1 sinon (avec la liste des problèmes).
param(
    [Parameter(Mandatory = $true)][string]$Zip,
    [string]$Version = '',
    [string]$VerifierImports = ''
)
$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$MindsetMin = '2.15.0'
$attendus = @('Mindcare\Mindset.exe', 'Mindcare\Mindcare.exe', 'Mindcare\MindcareAide.exe', 'Mindcare\version.json', 'Mindcare\LISEZMOI.txt', 'Mindcare\enquetes.json')
$problemes = New-Object Collections.Generic.List[string]

if (-not (Test-Path -LiteralPath $Zip -PathType Leaf)) { Write-Host "Zip introuvable : $Zip"; exit 1 }
$Zip = (Resolve-Path -LiteralPath $Zip).Path

# La version de Mindcare : la ligne « version = "x.y.z" » de la section [package] du Cargo.toml de mindcare-appli
$section = ''; $cargo = $null
foreach ($ligne in [IO.File]::ReadAllLines((Join-Path $racine 'rust\mindcare-appli\Cargo.toml'), [Text.Encoding]::UTF8)) {
    $l = $ligne.Trim()
    if ($l.StartsWith('[')) { $section = $l; continue }
    if ($section -eq '[package]' -and $l -match '^version\s*=\s*"([^"]*)"') { $cargo = $Matches[1]; break }
}
if (-not $Version) { $Version = $cargo }
elseif ($Version -cne $cargo) { $problemes.Add("La version demandée ($Version) n'est pas celle du Cargo.toml de mindcare-appli ($cargo).") }

Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
$tmp = Join-Path ([IO.Path]::GetTempPath()) "mindcare-zip-rust-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 8))"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
    # Les entrées du zip (avec « \ » ou « / », comme Mindset les lit)
    $archive = [IO.Compression.ZipFile]::OpenRead($Zip)
    try {
        $noms = @($archive.Entries | ForEach-Object { $_.FullName.Replace('/', '\') })
        $fichiers = @($noms | Where-Object { -not $_.EndsWith('\') } | Sort-Object)
        $enTrop = @($fichiers | Where-Object { $attendus -cnotcontains $_ })
        $absents = @($attendus | Where-Object { $fichiers -cnotcontains $_ })
        if ($enTrop.Count) { $problemes.Add("Fichiers en trop dans le zip : $($enTrop -join ', ')") }
        if ($absents.Count) { $problemes.Add("Fichiers absents du zip : $($absents -join ', ')") }
        if (@($noms | Where-Object { -not $_.StartsWith('Mindcare\', [StringComparison]::Ordinal) -and $_ -cne 'Mindcare\' }).Count) { $problemes.Add('Le zip a autre chose que le dossier Mindcare.') }
        foreach ($e in $archive.Entries) {
            $n = $e.FullName.Replace('/', '\')
            if ($attendus -cnotcontains $n) { continue }
            [IO.Compression.ZipFileExtensions]::ExtractToFile($e, (Join-Path $tmp (Split-Path $n -Leaf)), $true)
        }
    } finally { $archive.Dispose() }

    # version.json
    $vj = Join-Path $tmp 'version.json'
    if (Test-Path -LiteralPath $vj) {
        $o = [IO.File]::ReadAllBytes($vj)
        if ($o.Length -ge 3 -and $o[0] -eq 0xEF -and $o[1] -eq 0xBB -and $o[2] -eq 0xBF) { $problemes.Add('version.json a une marque UTF-8 (BOM).') }
        $j = $null
        try { $j = [Text.Encoding]::UTF8.GetString($o).TrimStart([char]0xFEFF) | ConvertFrom-Json } catch { $problemes.Add("version.json illisible : $($_.Exception.Message)") }
        if ($j) {
            Write-Host "version.json : appli $($j.appli), version $($j.version), mindset_min $($j.mindset_min)"
            if ($j.appli -cne 'mindcare') { $problemes.Add("version.json : appli « $($j.appli) » au lieu de « mindcare »") }
            if ($j.version -cne $Version) { $problemes.Add("version.json : version $($j.version) au lieu de $Version") }
            if ($j.mindset_min -cne $MindsetMin) { $problemes.Add("version.json : mindset_min $($j.mindset_min) au lieu de $MindsetMin") }
        }
    }

    # Le catalogue des enquêtes : le même que celui du dépôt, octet pour octet. Le zip de GitHub a les fins de ligne de
    # git, un zip fait sur ce PC celles du dossier (git pour Windows peut y écrire des CRLF) : l'un ou l'autre convient.
    # Avant : seulement le dossier, et 0.91.0 refusé à tort parce que git y avait remis le fichier en CRLF.
    $cat = Join-Path $tmp 'enquetes.json'
    if (Test-Path -LiteralPath $cat) {
        $dossier = (Get-FileHash -LiteralPath (Join-Path $racine 'Mindcare\fichiers\enquetes.json')).Hash -eq (Get-FileHash -LiteralPath $cat).Hash
        $avantGit = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        try {
            $dansGit = ((& git -C $racine rev-parse 'HEAD:Mindcare/fichiers/enquetes.json' 2>$null) -join '').Trim()
            $duZip = ((& git -C $racine hash-object --no-filters -- $cat 2>$null) -join '').Trim()
        } finally { $ErrorActionPreference = $avantGit }
        $commeGit = $dansGit -match '^[0-9a-f]{40}$' -and $dansGit -eq $duZip
        if (-not ($dossier -or $commeGit)) { $problemes.Add('enquetes.json du zip différent de Mindcare\fichiers\enquetes.json (ni celui du dossier, ni celui de git)') }
    }

    # Les fiches de version des deux programmes
    $v4 = @($Version.Split('.')); while ($v4.Count -lt 4) { $v4 += '0' }; $v4 = $v4 -join '.'
    foreach ($e in @(@('Mindset.exe', 'Mindset'), @('Mindcare.exe', 'Mindcare'), @('MindcareAide.exe', 'Mindcare'))) {
        $f = Join-Path $tmp $e[0]
        if (-not (Test-Path -LiteralPath $f)) { continue }
        $i = (Get-Item -LiteralPath $f).VersionInfo
        Write-Host "$($e[0]) : description « $($i.FileDescription) », produit « $($i.ProductName) », version $($i.ProductVersion), SHA-256 $((Get-FileHash -LiteralPath $f -Algorithm SHA256).Hash)"
        if ($i.FileDescription -cne $e[1]) { $problemes.Add("$($e[0]) : description « $($i.FileDescription) » au lieu de « $($e[1]) »") }
        if ($i.ProductName -cne $e[1]) { $problemes.Add("$($e[0]) : produit « $($i.ProductName) » au lieu de « $($e[1]) »") }
        $p = @(([string]$i.ProductVersion).Split('.')); while ($p.Count -lt 4) { $p += '0' }
        if (($p -join '.') -ne $v4) { $problemes.Add("$($e[0]) : version $($i.ProductVersion) au lieu de $Version") }
        # La construction d'essai (option cargo « essai ») n'est jamais livrée
        & (Join-Path $PSScriptRoot 'verifier-sans-essai.ps1') -Exe $f | Out-Host
        if ($LASTEXITCODE) { $problemes.Add("$($e[0]) contient la construction d'essai") }
    }

    # Aucun runtime C à installer à part (contrôle du dépôt Mindset, lu dans la table des imports, sans rien lancer)
    if ($VerifierImports) {
        $exes = @('Mindset.exe', 'Mindcare.exe', 'MindcareAide.exe' | ForEach-Object { Join-Path $tmp $_ } | Where-Object { Test-Path -LiteralPath $_ })
        if ($exes.Count) {
            & $VerifierImports -Fichiers $exes -Liste | Out-Host
            if ($LASTEXITCODE) { $problemes.Add('un programme du zip a besoin d''un runtime C à installer à part') }
        }
    }
} finally {
    Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

if ($problemes.Count) {
    $problemes | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
    Write-Host "Zip refusé : $Zip ($($problemes.Count) problème(s))." -ForegroundColor Red
    exit 1
}
Write-Host "Zip conforme : $Zip (Mindcare $Version, SHA-256 $((Get-FileHash -LiteralPath $Zip -Algorithm SHA256).Hash))." -ForegroundColor Green
exit 0
