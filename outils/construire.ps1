# Construit le dossier de Mindcare prêt à installer, puis release\Mindcare.zip (signé ensuite par publier.ps1).
# (Mindcare doit être fermée : son .exe est remplacé.)
#
#   Mindcare\
#     Mindcare.exe                    <- le programme (icône de Mindcare)
#     LISEZMOI.txt
#     fichiers\Mindcare.ps1, Mindcare.ico
#     fichiers\cle-publique.xml        <- la clé publique de la suite (la même que Mindset)
#     fichiers\modules\                <- le code découpé par partie

$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$app    = Join-Path $racine 'Mindcare'
$fich   = Join-Path $app 'fichiers'
$icones = Join-Path $PSScriptRoot 'icones'
$csc    = Join-Path $env:windir 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'

& (Join-Path $PSScriptRoot 'icone.ps1') -OutDir $icones | Out-Null
Copy-Item (Join-Path $icones 'Mindcare.ico') (Join-Path $fich 'Mindcare.ico') -Force

$exe = Join-Path $app 'Mindcare.exe'
$sma = [psobject].Assembly.Location   # le moteur PowerShell de Windows, que l'exe fait tourner
& $csc /nologo /target:winexe /optimize+ /platform:anycpu "/win32icon:$(Join-Path $icones 'Mindcare.ico')" "/win32manifest:$(Join-Path $PSScriptRoot 'lanceur.manifest')" /reference:System.Windows.Forms.dll "/reference:$sma" "/out:$exe" (Join-Path $PSScriptRoot 'lanceur.cs')
if ($LASTEXITCODE) { throw "Échec de compilation de $exe" }

# Le zip (release\ n'est pas suivi par git : publier.ps1 le signe et le joint à la Release GitHub, où Mindset le prend)
$release = Join-Path $racine 'release'
New-Item -ItemType Directory -Force -Path $release | Out-Null
$zip = Join-Path $release 'Mindcare.zip'
Compress-Archive -Path $app -DestinationPath $zip -Force
"OK : $zip"
