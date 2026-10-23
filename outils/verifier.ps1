# Mindcare : vérifications sans lancer Mindcare (les mêmes que outils\publier.ps1 avant une publication).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File .\outils\verifier.ps1
#
#   - syntaxe de tous les .ps1 du dépôt ;
#   - chaque .ps1 est en UTF-8 avec BOM (sans BOM, Windows PowerShell lit mal les accents) ;
#   - la fenêtre (Mindcare\fichiers\modules\interface.xaml) se lit ;
#   - aucun tiret long (U+2013, U+2014) dans les textes : .ps1, .cs, .xaml, .txt, .md, .json, .yml ;
#   - le fichier LICENSE est présent.
# Code de sortie 1 s'il y a un problème (utilisé par publier.ps1 et par la CI de GitHub, .github\workflows\tests.yml).

$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$app = Join-Path $racine 'Mindcare'
$exclus = '\\(\.git|_test|release|outils\\icones)\\'
$problemes = New-Object Collections.Generic.List[string]
function Get-Relatif([string]$Chemin) { $Chemin.Substring($racine.Length).TrimStart('\') }

# Syntaxe et encodage des .ps1
$scripts = @(Get-ChildItem -LiteralPath $racine -Recurse -File -Filter '*.ps1' | Where-Object { $_.FullName -notmatch $exclus })
foreach ($f in $scripts) {
    $err = $null
    [void][System.Management.Automation.Language.Parser]::ParseFile($f.FullName, [ref]$null, [ref]$err)
    foreach ($e in $err) { $problemes.Add("Syntaxe : $(Get-Relatif $f.FullName) ligne $($e.Extent.StartLineNumber) : $($e.Message)") }
    $o = [IO.File]::ReadAllBytes($f.FullName)
    if ($o.Length -lt 3 -or $o[0] -ne 0xEF -or $o[1] -ne 0xBB -or $o[2] -ne 0xBF) { $problemes.Add("Encodage : $(Get-Relatif $f.FullName) doit être en UTF-8 avec BOM (sinon les accents cassent)") }
}

# La fenêtre
try { [void][xml][IO.File]::ReadAllText((Join-Path $app 'fichiers\modules\interface.xaml'), [Text.Encoding]::UTF8) }
catch { $problemes.Add("interface.xaml : $($_.Exception.Message)") }

# Tirets longs (lus en UTF-8 : un fichier sans BOM serait sinon mal lu)
$textes = @(Get-ChildItem -LiteralPath $racine -Recurse -File -Force -Include '*.ps1', '*.cs', '*.xaml', '*.txt', '*.md', '*.json', '*.yml' |
        Where-Object { $_.FullName -notmatch $exclus })
foreach ($f in $textes) {
    $n = 0
    foreach ($l in [IO.File]::ReadAllLines($f.FullName, [Text.Encoding]::UTF8)) {
        $n++
        if ($l -match '[\u2013\u2014]') { $problemes.Add("Tiret long : $(Get-Relatif $f.FullName) ligne $n") }
    }
}

# Licence
if (-not (Test-Path -LiteralPath (Join-Path $racine 'LICENSE'))) { $problemes.Add('LICENSE absent à la racine du dépôt') }

Write-Host "Vérifié : $($scripts.Count) scripts .ps1, $($textes.Count) fichiers texte, interface.xaml, LICENSE."
if ($problemes.Count) {
    $problemes | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
    Write-Host "$($problemes.Count) problème(s) trouvé(s)." -ForegroundColor Red
    exit 1
}
Write-Host 'Aucun problème.' -ForegroundColor Green
exit 0
