# La recherche par mots actuelle de Mindcare (Find-Pannes), sur une liste de phrases : une ligne par phrase, les 3
# premières pannes séparées par des tabulations. Sert à comparer l'IA à ce qui existe (mesurer.py).
param([string]$Catalogue, [string]$Phrases)
$AppDir = Split-Path $Catalogue -Parent
. (Join-Path $AppDir 'modules\enquete.ps1')
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$d = Read-Enquetes $Catalogue
foreach ($q in [IO.File]::ReadAllLines($Phrases)) { (@(Find-Pannes $d $q | Select-Object -First 3 | ForEach-Object Id) -join "`t") }
