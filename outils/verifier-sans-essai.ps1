# L'exe livré ne connaît pas la construction d'essai (option cargo « essai » de rust\mindcare-appli) : ni le dossier
# de données d'essai (MINDCARE_DONNEES), ni les autres variables de l'essai, ni la capture de la fenêtre, ni le faux
# presse-papiers d'essai, ni le faux serveur de téléchargement et le faux micro de l'essai (étape 6.4). Le test cherche
# ces noms dans l'exe, en texte simple et en UTF-16 (sans rien lancer).
#
#   & '.\outils\verifier-sans-essai.ps1' -Exe <Mindset.exe>
#
# Code 0 : aucun nom trouvé ; 1 : un nom trouvé (ou exe introuvable). Contre-épreuve : sur l'exe construit avec
# « --features essai », il doit rendre 1.
param([Parameter(Mandatory = $true)][string]$Exe)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) { Write-Host "Introuvable : $Exe"; exit 1 }
$octets = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $Exe).Path)
# (chaque octet tel quel : les noms cherchés sont en ASCII)
$simple = [Text.Encoding]::GetEncoding(28591).GetString($octets)
$noms = 'MINDCARE_DONNEES', 'MINDCARE_ESSAI_LOCAL', 'MINDCARE_ESSAI_NOM', 'MINDCARE_ESSAI_DEPOT', 'capture.demande', 'actions.demande', 'presse-papiers.txt', 'essai-sans-depot', "Micro d'essai"
$trouves = @()
foreach ($n in $noms) {
    $large = ($n.ToCharArray() | ForEach-Object { "$_`0" }) -join ''
    if ($simple.Contains($n) -or $simple.Contains($large)) { $trouves += $n }
}
if ($trouves.Count) { Write-Host "$Exe contient la construction d'essai : $($trouves -join ', ')"; exit 1 }
Write-Host "$Exe : aucune trace de la construction d'essai ($($noms -join ', '))."
exit 0
