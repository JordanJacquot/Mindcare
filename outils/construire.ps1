# Construit le dossier de Mindcare prêt à installer, puis release\Mindcare.zip (signé ensuite par publier.ps1) et
# release\catalogue.zip (pour Mindcare Mobile). Lancé aussi par la CI de GitHub (.github\workflows\release.yml).
# (Mindcare doit être fermée : son .exe est remplacé.)
#
#   Mindcare\
#     Mindset.exe                     <- le programme (icône de Mindcare ; même nom pour toute la suite : une seule
#                                        ligne « Mindset » dans le Gestionnaire des tâches)
#     Mindcare.exe                    <- le relais vers Mindset.exe, pour ce qui appelle encore Mindcare.exe
#     LISEZMOI.txt
#     fichiers\Mindcare.ps1, Mindcare.ico
#     fichiers\cles-publiques\*.xml    <- les clés publiques de la suite (les mêmes que Mindset)
#     fichiers\modules\                <- le code découpé par partie

$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$app    = Join-Path $racine 'Mindcare'
$fich   = Join-Path $app 'fichiers'
$icones = Join-Path $PSScriptRoot 'icones'
$csc    = Join-Path $env:windir 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'

& (Join-Path $PSScriptRoot 'icone.ps1') -OutDir $icones | Out-Null
Copy-Item (Join-Path $icones 'Mindcare.ico') (Join-Path $fich 'Mindcare.ico') -Force

$exe = Join-Path $app 'Mindset.exe'
$sma = [psobject].Assembly.Location   # le moteur PowerShell de Windows, que l'exe fait tourner
& $csc /nologo /target:winexe /optimize+ /platform:anycpu "/win32icon:$(Join-Path $icones 'Mindcare.ico')" "/win32manifest:$(Join-Path $PSScriptRoot 'lanceur.manifest')" /reference:System.Windows.Forms.dll "/reference:$sma" "/out:$exe" (Join-Path $PSScriptRoot 'lanceur.cs')
if ($LASTEXITCODE) { throw "Échec de compilation de $exe" }
$relais = Join-Path $app 'Mindcare.exe'
& $csc /nologo /target:winexe /optimize+ /platform:anycpu "/win32icon:$(Join-Path $icones 'Mindcare.ico')" "/win32manifest:$(Join-Path $PSScriptRoot 'lanceur.manifest')" /reference:System.Windows.Forms.dll "/out:$relais" (Join-Path $PSScriptRoot 'relais.cs')
if ($LASTEXITCODE) { throw "Échec de compilation de $relais" }

# Le zip (release\ n'est pas suivi par git : publier.ps1 le signe et le joint à la Release GitHub, où Mindset le prend)
$release = Join-Path $racine 'release'
New-Item -ItemType Directory -Force -Path $release | Out-Null
$zip = Join-Path $release 'Mindcare.zip'
Compress-Archive -Path $app -DestinationPath $zip -Force
"OK : $zip"

# Le catalogue pour Mindcare Mobile (joint à la Release par publier.ps1 ; le téléphone le télécharge) : enquetes.json et
# images\*.svg seulement. Mindcare sur PC ne s'en sert pas : elle a déjà ces fichiers dans son dossier.
$catalogue = Join-Path $release 'catalogue.zip'
Remove-Item -LiteralPath $catalogue -ErrorAction SilentlyContinue
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
$archive = [IO.Compression.ZipFile]::Open($catalogue, 'Create')
try {
    [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, (Join-Path $fich 'enquetes.json'), 'enquetes.json')
    foreach ($img in Get-ChildItem -LiteralPath (Join-Path $fich 'images') -Filter '*.svg') {
        [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $img.FullName, "images/$($img.Name)")   # (des « / » : Android les attend)
    }
} finally { $archive.Dispose() }
"OK : $catalogue"
