# Publie une nouvelle version de Mindcare sur GitHub, avec sa signature. Mindset la propose ensuite (il installe et met
# à jour Mindcare à partir du zip signé de la Release).
#
#   .\outils\publier.ps1 -Version 0.1.0 -Notes "Ce qui change, en mots simples"
#   .\outils\publier.ps1 -Version 0.16.0 -Notes "..." -Preversion        version d'essai (préversion GitHub) : seuls
#                                                                        ceux qui ont coché « Recevoir les versions
#                                                                        d'essai » de Mindcare dans Mindset la reçoivent
#
# Mindcare en Rust (rust\mindcare-appli) :
#   .\outils\publier.ps1 -Rust -Version 0.92.0 -Notes "..."               publie Mindcare en Rust pour tout le monde
#   .\outils\publier.ps1 -Rust -Preversion -Version 0.92.0 -Notes "..."   en version d'essai (préversion GitHub)
#   .\outils\publier.ps1 -Rust -Essai                                     essai à blanc : rien n'est modifié, envoyé ni
#                                                                        publié (voir plus bas)
#
# Avant de publier : syntaxe de tous les fichiers, fenêtre (interface.xaml), tirets cadratins, clés publiques (les mêmes que
# Mindset). Version PowerShell : construction, puis le test automatique (outils\tester.ps1). -SansTest le saute
# (urgence seulement).
# Le zip est signé avec la clé de la suite (Mindset\outils\signer.ps1) : sans signature, rien n'est publié.
# Les notes de version ne citent jamais un thème perso (Mario, Pokémon, Dofus...).
#
# -Rust : le zip n'est JAMAIS construit sur ce PC. Le numéro de version est écrit dans rust\mindcare-appli\Cargo.toml (et
# Cargo.lock), commité et envoyé ; GitHub construit alors Mindcare en Rust à partir de ce commit (release.yml, genre
# rust : cargo build --release --locked, cargo deny, zip vérifié, attestation), après ses tests (tests.yml, contrat de
# la suite compris). outils\zip-rust-github.ps1 récupère ce zip et vérifie son attestation (ce dépôt, release.yml, ce
# commit) ; outils\verifier-zip-rust.ps1 revérifie son contenu (fichiers exacts, version.json, fiches de version, pas de
# construction d'essai, aucun runtime C à part). Ensuite seulement : signature, tag, préversion avec catalogue.zip
# (inchangé, pour Mindcare Mobile, qui ne prend jamais une préversion). Mindcare.ps1 n'est pas touché : la version
# PowerShell n'est plus publiée. Sans -Preversion, la version va à tout le monde (décision de Jordan du 07/11 : à 4
# utilisateurs, pas de version d'essai ; Mindcare en Rust est pour tout le monde depuis la 0.90.0, le 04/11).
#
# -Essai (avec -Rust) : les mêmes vérifications, sans rien écrire dans le dépôt ni rien envoyer sur GitHub. Le zip est
# celui que GitHub a déjà construit pour le commit en cours (seulement lu), ou -ZipLocal <Mindcare.zip> (un zip fait sur
# ce PC par outils\zip-rust.ps1, sans attestation : seulement pour un essai). Il est signé avec une clé jetable.

param(
    [string]$Version = '',
    [string]$Notes = '',
    [switch]$Preversion,
    [switch]$Rust,
    [switch]$SansTest,
    [switch]$Essai,
    [string]$ZipLocal = '',
    # le dépôt de Mindset (clés publiques, signer.ps1, verifier-imports.ps1) : par défaut à côté de celui-ci
    [string]$Mindset = ''
)
$ErrorActionPreference = 'Stop'
$racine  = Split-Path $PSScriptRoot -Parent
$app     = Join-Path $racine 'Mindcare'
$ps1     = Join-Path $app 'fichiers\Mindcare.ps1'
$zip     = Join-Path $racine 'release\Mindcare.zip'
$mindset = if ($Mindset) { $Mindset } else { Join-Path (Split-Path $racine -Parent) 'Mindset' }   # le dépôt de Mindset, à côté de celui-ci
$utf8    = New-Object Text.UTF8Encoding($true)
$cargoAppli = Join-Path $racine 'rust\mindcare-appli\Cargo.toml'
$cargoLock  = Join-Path $racine 'rust\Cargo.lock'

if ($Essai -and -not $Rust) { throw 'L''essai à blanc (-Essai) n''existe qu''avec -Rust.' }
if ($ZipLocal -and -not $Essai) { throw '-ZipLocal ne sert qu''à un essai (-Rust -Essai) : une vraie publication prend toujours le zip construit par GitHub.' }
if ($Rust -and -not (Test-Path -LiteralPath $cargoAppli)) { throw "Mindcare en Rust introuvable : $cargoAppli" }

# La version du Cargo.toml de mindcare-appli (ligne « version = "x.y.z" » de sa section [package])
function Get-VersionCargo {
    $section = ''
    foreach ($ligne in [IO.File]::ReadAllLines($cargoAppli, [Text.Encoding]::UTF8)) {
        $l = $ligne.Trim()
        if ($l.StartsWith('[')) { $section = $l; continue }
        if ($section -eq '[package]' -and $l -match '^version\s*=\s*"([^"]*)"') { return $Matches[1] }
    }
    $null
}
# La version de Mindcare.ps1 (la version PowerShell)
$versionPs = if ([IO.File]::ReadAllText($ps1, [Text.Encoding]::UTF8) -match "(?m)^\`$AppVersion = '([\d\.]+)'") { $Matches[1] } else { '' }

if ($Essai -and -not $Version) { $Version = Get-VersionCargo }
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Numéro de version invalide : $Version (exemple : 0.1.0)" }
# Mindcare en Rust après la version PowerShell : Mindset ne revient jamais à une version plus ancienne que celle installée
if ($Rust -and -not $Essai -and $versionPs -and [version]$Version -le [version]$versionPs) {
    throw "Mindcare en Rust doit avoir un numéro plus grand que la version PowerShell ($versionPs) : par exemple 0.90.0."
}

# Un numéro déjà publié ne doit jamais être réutilisé (GitHub garde l'ancien zip en cache)
if (-not $Essai) {
    Push-Location $racine
    try {
        $ErrorActionPreference = 'Continue'
        git rev-parse -q --verify "refs/tags/v$Version" 2>$null | Out-Null; $tagExiste = ($LASTEXITCODE -eq 0)
        gh release view "v$Version" 2>$null | Out-Null; $releaseExiste = ($LASTEXITCODE -eq 0)
    } finally { Pop-Location; $ErrorActionPreference = 'Stop' }
    if ($tagExiste -or $releaseExiste) { throw "La version $Version existe déjà. Choisis un numéro plus grand." }
}

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

# La préversion porte un titre qui le dit (Mindset la montre comme une version d'essai)
$titre = "Mindcare $Version$(if ($Preversion) { ' (version d''essai)' })"

if ($Rust) {
    # 1. Tout est commité : la version publiée est celle de git (GitHub construit le commit, pas ce dossier). Une
    # publication arrêtée en route laisse seulement le numéro dans Cargo.toml et Cargo.lock : remis comme dans git.
    if (-not $Essai) {
        $avant = [Console]::OutputEncoding
        try { [Console]::OutputEncoding = New-Object Text.UTF8Encoding($false); $sale = @(& git -C $racine -c core.quotepath=false status --porcelain 2>$null | Where-Object { $_ }) }
        finally { [Console]::OutputEncoding = $avant }
        $siens = @('rust/mindcare-appli/Cargo.toml', 'rust/Cargo.lock')
        if ($sale.Count -and -not @($sale | Where-Object { $_ -notmatch '^(M |MM| M) ' -or $siens -notcontains $_.Substring(3).Trim('"') }).Count) {
            Invoke-Native 'git' @('-C', $racine, 'reset', '-q', '--', $siens[0], $siens[1]) | Out-Null
            Invoke-Native 'git' @('-C', $racine, 'checkout', '--', $siens[0], $siens[1]) | Out-Null
            Write-Host 'Le numéro de version laissé par une publication arrêtée en route est remis comme dans git.'
            $sale = @()
        }
        if ($sale.Count) { throw "Publication annulée : des fichiers ne sont pas commités ($(($sale | Select-Object -First 5) -join ' ; ')). Commite-les d'abord : la version publiée est celle de git." }

        # 2. Le numéro de version : rust\mindcare-appli\Cargo.toml (la seule source : version.json, Mindset.exe et
        # Mindcare.exe le prennent là) et sa ligne dans Cargo.lock (sinon --locked refuse de construire). Commité tout
        # de suite et envoyé : GitHub construit ce commit. Une tentative arrêtée plus loin laisse ce commit ; la suivante
        # avec le même numéro n'a rien de nouveau à commiter et reprend la même construction.
        & (Join-Path $PSScriptRoot 'version-rust.ps1') -Version $Version
        if ((Get-VersionCargo) -cne $Version) { throw 'Publication annulée : le numéro de version n''a pas pu être écrit dans Cargo.toml.' }
        & git -C $racine diff --quiet -- $cargoAppli $cargoLock
        if ($LASTEXITCODE) {
            Invoke-Native git @('-C', $racine, 'add', '--', $cargoAppli, $cargoLock)
            Invoke-Native git @('-C', $racine, 'commit', '-m', "Mindcare $Version (Rust$(if ($Preversion) { ', version d''essai' }))")
        }
        Invoke-Native git @('-C', $racine, 'push', 'origin', 'HEAD')
    } elseif ((Get-VersionCargo) -cne $Version) {
        throw "Essai : -Version $Version, mais le Cargo.toml de mindcare-appli dit $(Get-VersionCargo) (l'essai ne modifie rien : laisse -Version vide)."
    }

    # 3. Le zip construit et attesté par GitHub pour ce commit (ou, en essai seulement, un zip fait sur ce PC)
    if ($ZipLocal) {
        Write-Host "Essai : zip fait sur ce PC ($ZipLocal), sans attestation. Une vraie publication prend celui de GitHub." -ForegroundColor Yellow
        $source = (Resolve-Path -LiteralPath $ZipLocal).Path
    } else {
        $source = & (Join-Path $PSScriptRoot 'zip-rust-github.ps1') -Essai:$Essai -SansTest:$SansTest
        $source = @($source | Where-Object { $_ }) | Select-Object -Last 1
        if (-not $source) {
            if ($Essai) { Write-Host 'Essai arrêté : aucun zip construit par GitHub pour ce commit (ou passe -ZipLocal).' -ForegroundColor Yellow; return }
            throw 'Publication annulée : pas de zip construit par GitHub.'
        }
    }
    New-Item -ItemType Directory -Force -Path (Split-Path $zip -Parent) | Out-Null
    Remove-Item -LiteralPath $zip, "$zip.sig" -ErrorAction SilentlyContinue   # (jamais l'ancien zip ni l'ancienne signature)
    Copy-Item -LiteralPath $source -Destination $zip

    # 4. Son contenu revérifié ici (fichiers exacts, version.json, fiches de version, pas de construction d'essai, aucun
    # runtime C à part)
    & (Join-Path $PSScriptRoot 'verifier-zip-rust.ps1') -Zip $zip -Version $Version -VerifierImports (Join-Path $mindset 'outils\verifier-imports.ps1')
    if ($LASTEXITCODE) { throw 'Publication annulée : le zip de Mindcare en Rust n''est pas conforme.' }

    # 5. Le catalogue pour Mindcare Mobile (inchangé : construit depuis Mindcare\fichiers comme pour la version PowerShell)
    & (Join-Path $PSScriptRoot 'construire.ps1') -CatalogueSeul
    $catalogue = Join-Path $racine 'release\catalogue.zip'
    if (-not (Test-Path -LiteralPath $catalogue)) { throw 'Publication annulée : release\catalogue.zip n''a pas été construit.' }
    $shaCatalogue = (Get-FileHash -LiteralPath $catalogue -Algorithm SHA256).Hash.ToLowerInvariant()

    # 6. La signature (en essai : une clé jetable, jamais la vraie)
    if ($Essai) {
        $cleEssai = New-Object Security.Cryptography.RSACryptoServiceProvider(3072); $cleEssai.PersistKeyInCsp = $false
        try { [IO.File]::WriteAllBytes("$zip.sig", $cleEssai.SignData([IO.File]::ReadAllBytes($zip), [Security.Cryptography.CryptoConfig]::MapNameToOID('SHA256'))) }
        finally { $cleEssai.Dispose() }
    } else { & (Join-Path $mindset 'outils\signer.ps1') -Fichier $zip }
    if (-not (Test-Path -LiteralPath "$zip.sig")) { throw 'Publication annulée : le zip n''a pas pu être signé.' }

    $notesFinales = $(if ($Notes) { $Notes } else { "Mindcare $Version" }) + "`n`nSHA-256 du catalogue : $shaCatalogue"
    if ($Essai) {
        Write-Host "Essai fini : $zip (Mindcare $Version en Rust, signé avec une clé jetable) et $catalogue. Rien n'est publié." -ForegroundColor Green
        Write-Host "Une vraie publication ferait : tag v$Version, envoi, puis gh release create v$Version Mindcare.zip Mindcare.zip.sig catalogue.zip --title `"$titre`"$(if ($Preversion) { ' --prerelease' })."
        return
    }

    # 7. Tag et préversion (le numéro est déjà commité et envoyé, à l'étape 2)
    Push-Location $racine
    try {
        Invoke-Native git @('tag', "v$Version")
        Invoke-Native git @('push', 'origin', "v$Version")
        Invoke-Native gh (@('release', 'create', "v$Version", $zip, "$zip.sig", $catalogue, '--title', $titre, '--notes', $notesFinales) + $(if ($Preversion) { @('--prerelease') } else { @('--latest') }))
    } finally { Pop-Location }
    if ($Preversion) { "Mindcare $Version (Rust, version d'essai) publiée." } else { "Mindcare $Version (Rust) publiée pour tout le monde." }
    return
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
    $ghArgs = @('release', 'create', "v$Version", $zip, "$zip.sig", $catalogue, '--title', $titre, '--notes', $Notes)
    if ($Preversion) { $ghArgs += '--prerelease' }
    Invoke-Native gh $ghArgs
} finally { Pop-Location }
"Mindcare $Version publiée."
