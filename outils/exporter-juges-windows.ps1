# Exporte les juges de la lecture de Windows en Rust (rust\mindcare-windows\tests\juges\) : des données brutes de
# Windows (journal Système, disques, place, batterie, démarrage, barrettes de mémoire) et les indices que le VRAI
# Get-IndicesWindows de Mindcare (Mindcare\fichiers\modules\journaux.ps1) en tire. Les tests Rust (cargo test, dans
# rust\) donnent exactement les mêmes données à un faux Windows et doivent trouver exactement les mêmes indices.
#
# Rien n'est lu sur le PC : journaux.ps1 est chargé tel quel, mais Get-WinEvent, Get-Disk, Get-Partition,
# Get-PhysicalDisk, Get-PSDrive, Get-CimInstance et Get-Date sont remplacés par de fausses fonctions qui rendent des
# données préparées, et l'appel à powercfg.exe (le rapport de batterie) par une fausse commande qui écrit un rapport
# préparé dans un dossier temporaire à elle (effacé à la fin). Aucune fenêtre, aucun réseau, rien d'écrit ailleurs que
# dans ce dossier temporaire et dans les juges.
#
#   powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File outils\exporter-juges-windows.ps1
#
# Les cas au hasard sont écrits dans les juges avec leur graine : le Rust ne refait jamais le hasard de .NET.
# À relancer quand journaux.ps1 change.
param(
    [int]$Hasard = 600,         # cas tirés au hasard
    [int]$Graine = 20261031
)
$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$fichier = Join-Path $racine 'Mindcare\fichiers\modules\journaux.ps1'
$sortie = Join-Path $racine 'rust\mindcare-windows\tests\juges'
New-Item -ItemType Directory -Force -Path $sortie | Out-Null
$temp = Join-Path ([IO.Path]::GetTempPath()) "mindcare-juges-windows-$PID"
New-Item -ItemType Directory -Force -Path $temp | Out-Null
$ancienTemp = $env:TEMP; $ancienLecteur = $env:SystemDrive
$env:TEMP = $temp   # (journaux.ps1 y écrit le rapport de batterie : celui de la fausse commande)
# (même en cas d'échec : le dossier temporaire est effacé, les variables remises)
trap { $env:TEMP = $ancienTemp; $env:SystemDrive = $ancienLecteur; Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue; break }

# ---------------------------------------------------------------------------
# Le faux Windows : les données du cas en cours ($script:Cas)
# ---------------------------------------------------------------------------
$script:Cas = $null
function Write-Log { }
$EnTest = $false

# Les noms de Storage.types.ps1xml (le module Storage de Windows) : BusType et HealthStatus
function Get-NomBus($Code) {
    switch ($Code) {
        0 { 'Unknown' } 1 { 'SCSI' } 2 { 'ATAPI' } 3 { 'ATA' } 4 { '1394' } 5 { 'SSA' } 6 { 'Fibre Channel' } 7 { 'USB' }
        8 { 'RAID' } 9 { 'iSCSI' } 10 { 'SAS' } 11 { 'SATA' } 12 { 'SD' } 13 { 'MMC' } 14 { 'Virtual' }
        15 { 'File Backed Virtual' } 16 { 'Spaces' } 17 { 'NVMe' } 18 { 'SCM' } 19 { 'UFS' } Default { 'Unknown' }
    }
}
function Get-NomSante($Code) { switch ($Code) { 0 { 'Healthy' } 1 { 'Warning' } 2 { 'Unhealthy' } 5 { 'Unknown' } Default { 'Unknown' } } }

# Un faux événement : un vrai objet .NET, comme l'EventLogRecord de Get-WinEvent (un [pscustomobject] n'a pas le .Count
# implicite de PowerShell 5.1, et un seul événement compterait pour 0 dans journaux.ps1)
class FauxEvenement { [int]$Id; [string]$ProviderName; [object]$Level; [datetime]$TimeCreated; [string]$Message; [object[]]$Properties }

function Get-Date { $script:Maintenant }

function Get-WinEvent {
    [CmdletBinding()] param([hashtable]$FilterHashtable, [string]$LogName, [int]$MaxEvents = [int]::MaxValue, [switch]$Oldest)
    $c = $script:Cas
    if ($LogName) {
        # La sonde (-Oldest -MaxEvents 1)
        if ($c.Sonde.Contains('Erreur')) { throw $c.Sonde.Erreur }
        if (-not $c.Sonde.Contains('Debut')) { return }
        return [pscustomobject]@{ TimeCreated = [datetime]$c.Sonde.Debut }
    }
    $f = $FilterHashtable
    if (@($c.Echecs) -contains $f.ProviderName) { throw "La requête au journal a échoué ($($f.ProviderName))" }
    $ids = @($f.Id | Where-Object { $null -ne $_ }); $niveaux = @($f.Level | Where-Object { $null -ne $_ })
    $l = @($c.Evenements | Where-Object {
        $_.Fournisseur -eq $f.ProviderName -and (-not $ids.Count -or $ids -contains $_.Id) -and
        (-not $niveaux.Count -or $niveaux -contains $_.Niveau) -and [datetime]$_.Date -ge $f.StartTime
    } | Sort-Object { [datetime]$_.Date } -Descending | Select-Object -First $MaxEvents)
    # (comme le vrai : aucun événement est une erreur)
    if (-not $l.Count) { throw 'Aucun événement ne correspond aux critères de sélection spécifiés.' }
    foreach ($e in $l) {
        [FauxEvenement]@{
            Id = $e.Id; ProviderName = $e.Fournisseur; Level = $e.Niveau; TimeCreated = [datetime]$e.Date; Message = $e.Message
            Properties = @(foreach ($p in $e.Proprietes) { [pscustomobject]@{ Value = $p } })
        }
    }
}

function Get-Disk {
    [CmdletBinding()] param()
    if ($script:Cas.Erreurs.Contains('Disques')) { throw $script:Cas.Erreurs.Disques }
    # (MSFT_Disk : un bus inconnu ou absent s'écrit « Unknown »)
    foreach ($d in $script:Cas.Disques) { [pscustomobject]@{ Number = [uint32]$d.Numero; BusType = (Get-NomBus $d.Bus) } }
}
function Get-Partition {
    [CmdletBinding()] param()
    if ($script:Cas.Erreurs.Contains('Partitions')) { throw $script:Cas.Erreurs.Partitions }
    foreach ($p in $script:Cas.Partitions) { [pscustomobject]@{ DiskNumber = [uint32]$p.Disque; DriveLetter = [char][int]$p.Lettre } }
}
function Get-PhysicalDisk {
    [CmdletBinding()] param()
    if ($script:Cas.Erreurs.Contains('DisquesPhysiques')) { throw $script:Cas.Erreurs.DisquesPhysiques }
    # (MSFT_PhysicalDisk : un bus absent reste absent ; une santé absente s'écrit « Unknown »)
    foreach ($d in $script:Cas.DisquesPhysiques) {
        [pscustomobject]@{ FriendlyName = $d.Nom; BusType = $(if ($null -ne $d.Bus) { Get-NomBus $d.Bus }); HealthStatus = (Get-NomSante $d.Sante) }
    }
}
function Get-PSDrive {
    [CmdletBinding()] param([string]$Name)
    if ($script:Cas.Erreurs.Contains('Place')) { throw $script:Cas.Erreurs.Place }
    $p = $script:Cas.Place
    [pscustomobject]@{ Free = [uint64]$p.Libre; Used = [uint64]$p.Taille - [uint64]$p.Libre }
}
function Get-CimInstance {
    [CmdletBinding()] param([Parameter(Position = 0)][string]$ClassName)
    switch ($ClassName) {
        'Win32_OperatingSystem' {
            if ($script:Cas.Erreurs.Contains('Demarrage')) { throw $script:Cas.Erreurs.Demarrage }
            [pscustomobject]@{ LastBootUpTime = [datetime]$script:Cas.Demarrage }
        }
        'Win32_PhysicalMemory' {
            if ($script:Cas.Erreurs.Contains('Barrettes')) { throw $script:Cas.Erreurs.Barrettes }
            foreach ($b in $script:Cas.Barrettes) {
                [pscustomobject]@{ Capacity = $(if ($null -ne $b.Capacite) { [uint64]$b.Capacite }); Manufacturer = $b.Fabricant; PartNumber = $b.Reference; DeviceLocator = $b.Emplacement }
            }
        }
        Default { throw "Get-CimInstance $ClassName : pas dans le faux Windows" }
    }
}
# Le rapport de batterie : rien d'écrit si powercfg échoue ; sinon un rapport avec ou sans batterie
function Invoke-FauxPowercfg([string]$Fichier) {
    if ($script:Cas.Erreurs.Contains('Batterie')) { return }
    $b = $script:Cas.Batterie
    $batteries = if ($null -eq $b) { '<Batteries />' } else {
        "<Batteries><Battery><Id>Batterie</Id><DesignCapacity>$($b.Origine)</DesignCapacity><FullChargeCapacity>$($b.Pleine)</FullChargeCapacity></Battery></Batteries>"
    }
    [IO.File]::WriteAllText($Fichier, "<?xml version=`"1.0`" encoding=`"utf-8`"?><BatteryReport xmlns=`"http://schemas.microsoft.com/battery/2012`">$batteries</BatteryReport>")
}

# Le vrai journaux.ps1, avec l'appel à powercfg.exe remplacé par la fausse commande
$texte = [IO.File]::ReadAllText($fichier, [Text.Encoding]::UTF8)
$appel = '& "$env:SystemRoot\System32\powercfg.exe" /batteryreport /xml /output $f 2>&1'
if (([regex]::Matches($texte, [regex]::Escape($appel))).Count -ne 1) { throw "journaux.ps1 : l'appel à powercfg a changé, l'exporteur est à revoir" }
$texte = $texte.Replace($appel, 'Invoke-FauxPowercfg $f')
. ([scriptblock]::Create($texte))

# ---------------------------------------------------------------------------
# Les cas
# ---------------------------------------------------------------------------
$fmt = 'yyyy-MM-ddTHH:mm:ss.fffffff'
$script:Ordre = 0
function New-Cas([string]$Nom, [string]$Maintenant = '2026-10-31T14:30:00') {
    $m = [datetime]$Maintenant
    [ordered]@{
        Nom = $Nom
        Maintenant = $m.ToString($fmt)
        Sonde = [ordered]@{ Debut = $m.AddDays(-90).ToString($fmt) }
        Evenements = New-Object 'Collections.Generic.List[object]'
        Echecs = @()
        Disques = @([ordered]@{ Numero = 0; Bus = 17 })
        Partitions = @([ordered]@{ Disque = 0; Lettre = 67 })
        DisquesPhysiques = @([ordered]@{ Nom = 'Samsung SSD 980 1TB'; Bus = 17; Sante = 0 })
        Lecteur = 'C:'
        Place = [ordered]@{ Libre = [uint64]200GB; Taille = [uint64]500GB }
        Batterie = $null
        Demarrage = $m.AddDays(-2).AddHours(-3).ToString($fmt)
        Barrettes = @()
        Erreurs = [ordered]@{}
    }
}
# Un événement, $Jours avant « maintenant » (chaque événement a sa propre date : quelques millisecondes d'écart)
function Add-Ev($Cas, [string]$Fournisseur, [int]$Id, [double]$Jours, [string[]]$Proprietes = @(), [string]$Message = '', [int]$Niveau = 2) {
    $script:Ordre++
    $d = ([datetime]$Cas.Maintenant).AddDays(-$Jours).AddMilliseconds(-($script:Ordre % 1000))
    $Cas.Evenements.Add([ordered]@{ Id = $Id; Fournisseur = $Fournisseur; Niveau = $Niveau; Date = $d.ToString($fmt); Message = $Message; Proprietes = @($Proprietes) })
}

# Joue un cas avec le vrai Get-IndicesWindows ; rend { Nom, Donnees, Indices }
function Invoke-Cas($Cas) {
    $script:Cas = $Cas
    $script:Maintenant = [datetime]$Cas.Maintenant
    $env:SystemDrive = $Cas.Lecteur
    $i = Get-IndicesWindows
    $script:Barrettes = $null
    $indices = [ordered]@{}
    foreach ($k in @($i.Keys | Sort-Object)) {
        $v = $i[$k]
        $indices[$k] = [ordered]@{ Vu = $v.Vu; Texte = $v.Texte; TexteOk = $v.TexteOk; Affiche = $v.Affiche; Partiel = $v.Partiel }
    }
    $donnees = [ordered]@{}
    foreach ($k in @($Cas.Keys)) {
        if ($k -eq 'Evenements') { $donnees[$k] = $Cas[$k].ToArray() }
        elseif ($k -ne 'Nom' -and $k -ne 'Barrettes') { $donnees[$k] = $Cas[$k] }
    }
    [ordered]@{ Nom = $Cas.Nom; Donnees = $donnees; Indices = $indices }
}

# (foreach, pas @($Elements) : PowerShell 5.1 échoue sur @() d'une List[object] de [ordered] ici, « Les types des arguments ne correspondent pas »)
function ConvertTo-JsonListe($Elements) { '[' + ($(foreach ($x in $Elements) { ConvertTo-Json -InputObject $x -Depth 12 -Compress }) -join ',') + ']' }
function Write-Juge([string]$Nom, $Champs) {
    $morceaux = foreach ($k in $Champs.Keys) { (ConvertTo-Json -InputObject $k -Compress) + ':' + $Champs[$k] }
    [IO.File]::WriteAllText((Join-Path $sortie $Nom), ('{' + ($morceaux -join ',') + '}'), (New-Object Text.UTF8Encoding($false)))
    Write-Host "  $Nom"
}

# ---------------------------------------------------------------------------
# Les cas nommés
# ---------------------------------------------------------------------------
$nommes = New-Object 'Collections.Generic.List[object]'
function Add-Nomme($Cas) { $nommes.Add((Invoke-Cas $Cas)) }

Add-Nomme (New-Cas 'PC sans souci')
$c = New-Cas 'journal vide'; $c.Sonde = [ordered]@{}; Add-Nomme $c
$c = New-Cas 'journal illisible'; $c.Sonde = [ordered]@{ Erreur = 'Accès refusé' }; Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 3; Add-Nomme $c
$c = New-Cas 'journal illisible, disque malade'; $c.Sonde = [ordered]@{ Erreur = 'Le journal est abîmé' }; $c.DisquesPhysiques[0].Sante = 1; Add-Nomme $c
$c = New-Cas 'journal illisible, disque USB malade'; $c.Sonde = [ordered]@{ Erreur = 'x' }; $c.DisquesPhysiques += [ordered]@{ Nom = 'Clé'; Bus = 7; Sante = 2 }; Add-Nomme $c
$c = New-Cas 'journal court (10 jours)'; $c.Sonde.Debut = ([datetime]$c.Maintenant).AddDays(-10).ToString($fmt); Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 2; Add-Nomme $c
$c = New-Cas 'journal court commencé un 1er'; $c.Sonde.Debut = '2026-10-01T09:12:00.0000000'; Add-Nomme $c
$c = New-Cas 'journal de 100 jours (test de mémoire partiel)'; $c.Sonde.Debut = ([datetime]$c.Maintenant).AddDays(-100).ToString($fmt); Add-Nomme $c
$c = New-Cas 'journal de 200 jours'; $c.Sonde.Debut = ([datetime]$c.Maintenant).AddDays(-200).ToString($fmt); Add-Nomme $c
$c = New-Cas 'journal commencé pile 30 jours avant'; $c.Sonde.Debut = ([datetime]$c.Maintenant).AddDays(-30).ToString($fmt); Add-Nomme $c

$c = New-Cas 'un redémarrage brutal'; Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 4.2; Add-Nomme $c
$c = New-Cas 'trois redémarrages brutaux'; foreach ($j in 1, 9, 20) { Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 $j }; Add-Nomme $c
$c = New-Cas 'redémarrages brutaux, un trop vieux et un autre numéro'; Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 31; Add-Ev $c 'Microsoft-Windows-Kernel-Power' 42 2; Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 29.9; Add-Nomme $c
$c = New-Cas '230 redémarrages brutaux (200 au plus)'; for ($k = 0; $k -lt 230; $k++) { Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 ($k / 10) }; Add-Nomme $c
$c = New-Cas 'redémarrage brutal un 1er'; Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 30; $c.Evenements[0].Date = '2026-10-01T20:00:00.0000000'; Add-Nomme $c

$c = New-Cas 'écrans bleus de toutes les familles'
Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 1 @('0x0000001a (0x0000000000041792, 0xffffd28000a13000, 0x0000000000000000, 0x0000000000000000)', 'C:\WINDOWS\MEMORY.DMP', '1b2c')
Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 2 @('0x00000116 (0xffff, 0x0)')
Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 3 @('0x0000007a (0x1)')
Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 4 @('0x00000124 (0x0)')
Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 5 @('0x0000001a (0x2)')
Add-Nomme $c
$c = New-Cas 'écran bleu au code inconnu, et code au-delà de 0x7FFFFFFF'
Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 1 @('0x000000c2 (0x7)'); Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 2 @('0xc000021a (0x0)'); Add-Nomme $c
$c = New-Cas 'écran bleu : le code dans le message seulement'
Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 1 @() "L'ordinateur a redémarré après une vérification d'erreur. La vérification d'erreur était : 0x0000009f (0x0000000000000003)."
Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 2 @('', 'x') 'Code 0x00000133 noté'
Add-Nomme $c
$c = New-Cas 'écran bleu sans code lisible'; Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 1 @('0x9F'); Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 6 @('0X0000009F'); Add-Nomme $c
$c = New-Cas 'écrans bleus, le même code plusieurs fois'; foreach ($j in 1, 2, 3) { Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 $j @('0x000000d1 (0x0)') }; Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 4 @('0x0000000a (0x0)'); Add-Nomme $c
$c = New-Cas 'écran bleu dans un journal court'; $c.Sonde.Debut = ([datetime]$c.Maintenant).AddDays(-5).ToString($fmt); Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 1 @('0x00000050 (0x0)'); Add-Nomme $c

$c = New-Cas 'erreurs matérielles : niveaux 1, 2 et 3'; Add-Ev $c 'Microsoft-Windows-WHEA-Logger' 18 1 -Niveau 1; Add-Ev $c 'Microsoft-Windows-WHEA-Logger' 19 2 -Niveau 2; Add-Ev $c 'Microsoft-Windows-WHEA-Logger' 47 3 -Niveau 3; Add-Nomme $c
$c = New-Cas 'erreur matérielle d''avertissement seulement'; Add-Ev $c 'Microsoft-Windows-WHEA-Logger' 17 1 -Niveau 3; Add-Nomme $c

$c = New-Cas 'test de mémoire sans erreur'; Add-Ev $c 'Microsoft-Windows-MemoryDiagnostics-Results' 1101 40; Add-Nomme $c
$c = New-Cas 'test de mémoire avec des erreurs'; Add-Ev $c 'Microsoft-Windows-MemoryDiagnostics-Results' 1102 12; Add-Nomme $c
$c = New-Cas 'deux tests de mémoire : le plus récent compte'; Add-Ev $c 'Microsoft-Windows-MemoryDiagnostics-Results' 1102 60; Add-Ev $c 'Microsoft-Windows-MemoryDiagnostics-Results' 1101 10; Add-Nomme $c
$c = New-Cas 'test de mémoire trop vieux (200 jours)'; $c.Sonde.Debut = ([datetime]$c.Maintenant).AddDays(-400).ToString($fmt); Add-Ev $c 'Microsoft-Windows-MemoryDiagnostics-Results' 1102 200; Add-Nomme $c
$c = New-Cas 'test de mémoire un 1er'; Add-Ev $c 'Microsoft-Windows-MemoryDiagnostics-Results' 1101 0; $c.Evenements[0].Date = '2026-09-01T10:00:00.0000000'; Add-Nomme $c

$c = New-Cas 'erreurs du disque du PC'; Add-Ev $c 'disk' 7 1 @('\Device\Harddisk0\DR0') 'Le périphérique \Device\Harddisk0\DR0 a un bloc défectueux.'; Add-Ev $c 'disk' 51 2 @('\Device\Harddisk0\DR0', '') ''; Add-Nomme $c
$c = New-Cas 'erreurs d''une clé USB (ne comptent pas)'
$c.Disques = @([ordered]@{ Numero = 0; Bus = 17 }, [ordered]@{ Numero = 1; Bus = 7 }); $c.Partitions = @([ordered]@{ Disque = 0; Lettre = 67 }, [ordered]@{ Disque = 1; Lettre = 69 })
Add-Ev $c 'disk' 7 1 @('\Device\Harddisk1\DR4'); Add-Ev $c 'disk' 153 2 @() 'The IO operation at logical block address 0x1 for Disk 1 (PDO name: \Device\0000003a) was retried.'
Add-Ev $c 'Ntfs' 55 3 @('E:', '\Device\HarddiskVolume9') ''; Add-Ev $c 'Ntfs' 55 4 @() 'Le système de fichiers du volume e: est endommagé.'
Add-Nomme $c
$c = New-Cas 'erreurs mêlées : USB et disque du PC'
$c.Disques = @([ordered]@{ Numero = 0; Bus = 17 }, [ordered]@{ Numero = 1; Bus = 7 }, [ordered]@{ Numero = 2; Bus = 11 }); $c.Partitions = @([ordered]@{ Disque = 0; Lettre = 67 }, [ordered]@{ Disque = 1; Lettre = 69 }, [ordered]@{ Disque = 2; Lettre = 100 }, [ordered]@{ Disque = 2; Lettre = 0 })
Add-Ev $c 'disk' 7 1 @('\Device\Harddisk1\DR4'); Add-Ev $c 'disk' 7 2 @('\Device\Harddisk2\DR2'); Add-Ev $c 'disk' 51 3 @() 'Une erreur sur le disque 2 pendant une pagination.'
Add-Ev $c 'Ntfs' 55 4 @('D:') ''; Add-Ev $c 'Ntfs' 55 5 @('\Device\HarddiskVolume2') 'Volume sans lettre'; Add-Ev $c 'NTFS' 55 6 @('C:') ''
Add-Ev $c 'disk' 11 7 @('\Device\Harddisk0\DR0'); Add-Ev $c 'disk' 7 40 @('\Device\Harddisk0\DR0')
Add-Nomme $c
$c = New-Cas 'erreurs de disque, Get-Disk illisible (toutes comptent)'; $c.Erreurs.Disques = 'Accès refusé'; Add-Ev $c 'disk' 7 1 @('\Device\Harddisk5\DR5'); Add-Ev $c 'Ntfs' 55 1 @('Z:'); Add-Nomme $c
$c = New-Cas 'erreurs de disque, Get-Partition illisible'
$c.Disques = @([ordered]@{ Numero = 0; Bus = 17 }, [ordered]@{ Numero = 1; Bus = 7 }); $c.Erreurs.Partitions = 'Accès refusé'
Add-Ev $c 'disk' 7 1 @('\Device\Harddisk1\DR4'); Add-Ev $c 'Ntfs' 55 1 @('Z:'); Add-Nomme $c
$c = New-Cas 'erreurs de disque, aucun disque interne'; $c.Disques = @([ordered]@{ Numero = 3; Bus = 7 }); Add-Ev $c 'disk' 7 1 @('\Device\Harddisk3\DR3'); Add-Ev $c 'disk' 7 2 @() 'sans numéro'; Add-Ev $c 'Ntfs' 55 1 @('C:'); Add-Ev $c 'Ntfs' 55 2 @() 'pas de lettre'; Add-Nomme $c
# (un numéro de disque trop grand pour [int], « Harddisk99999999999\ », fait échouer toute la lecture en PowerShell :
# pas dans les juges, voir le commentaire de erreurs_disque dans le Rust)
$c = New-Cas 'numéros de disque en majuscules et à zéros'; Add-Ev $c 'disk' 7 1 @('\Device\Harddisk007\DR0'); Add-Ev $c 'disk' 7 2 @('HARDDISK0\x'); Add-Ev $c 'disk' 7 3 @('\Device\Harddisk00\DR0'); Add-Nomme $c
$c = New-Cas 'disque malade (Warning)'; $c.DisquesPhysiques = @([ordered]@{ Nom = 'WDC WD10EZEX'; Bus = 11; Sante = 1 }); Add-Nomme $c
$c = New-Cas 'disque à la santé inconnue (compte comme malade)'; $c.DisquesPhysiques = @([ordered]@{ Nom = 'Disque X'; Bus = 17; Sante = $null }, [ordered]@{ Nom = 'Disque Y'; Bus = 17; Sante = 5 }); Add-Nomme $c
$c = New-Cas 'disque USB malade (ne compte pas), bus absent sain'; $c.DisquesPhysiques = @([ordered]@{ Nom = 'Clé'; Bus = 7; Sante = 2 }, [ordered]@{ Nom = 'Sans bus'; Bus = $null; Sante = 0 }); Add-Nomme $c
$c = New-Cas 'disque malade et erreurs'; $c.DisquesPhysiques = @([ordered]@{ Nom = 'A'; Bus = 17; Sante = 0 }, [ordered]@{ Nom = 'B'; Bus = $null; Sante = 2 }); Add-Ev $c 'disk' 7 1 @('\Device\Harddisk0\DR0'); Add-Nomme $c

$c = New-Cas 'pilote graphique qui plante'; foreach ($j in 1, 2) { Add-Ev $c 'Display' 4101 $j @('amdkmdag', '') 'Le pilote d''affichage amdkmdag a cessé de répondre et a récupéré.' }; Add-Nomme $c
$c = New-Cas 'redémarrages des mises à jour'
Add-Ev $c 'User32' 1074 1 @('C:\Windows\servicing\TrustedInstaller.exe (PC-JORDAN)', 'PC-JORDAN', 'Mise à jour')
Add-Ev $c 'User32' 1074 3 @('C:\WINDOWS\system32\MoUsoCoreWorker.exe (PC)'); Add-Ev $c 'User32' 1074 5 @('C:\Windows\explorer.exe (PC)')
Add-Ev $c 'User32' 1074 6 @('c:\windows\system32\usoclient.exe'); Add-Ev $c 'User32' 1074 7 @() 'TrustedInstaller dans le message seulement'
Add-Nomme $c
$c = New-Cas 'un seul redémarrage de mise à jour'; Add-Ev $c 'User32' 1074 2 @('C:\Windows\System32\wuauclt.exe'); Add-Ev $c 'User32' 1074 3 @('C:\Windows\System32\shutdown.exe'); Add-Nomme $c
$c = New-Cas 'processeur ralenti'; foreach ($j in 0.5, 1, 1.5) { Add-Ev $c 'Microsoft-Windows-Kernel-Processor-Power' 37 $j @('0', '1200', '3600') }; Add-Nomme $c
$c = New-Cas 'événements d''autres fournisseurs (ne comptent pas)'; Add-Ev $c 'Service Control Manager' 41 1; Add-Ev $c 'Microsoft-Windows-Kernel-General' 1001 1; Add-Ev $c 'disk2' 7 1; Add-Nomme $c

$c = New-Cas 'disque presque plein'; $c.Place = [ordered]@{ Libre = [uint64]12GB; Taille = [uint64]256GB }; Add-Nomme $c
$c = New-Cas 'place : 9,5 % arrondi au pair'; $c.Place = [ordered]@{ Libre = [uint64]19GB; Taille = [uint64]200GB }; Add-Nomme $c
$c = New-Cas 'place : 10,5 % arrondi au pair'; $c.Place = [ordered]@{ Libre = [uint64]21GB; Taille = [uint64]200GB }; Add-Nomme $c
$c = New-Cas 'place : 2,5 Go arrondis au pair'; $c.Place = [ordered]@{ Libre = [uint64](2.5 * 1GB); Taille = [uint64]100GB }; Add-Nomme $c
$c = New-Cas 'place : disque D'; $c.Lecteur = 'D:'; $c.Place = [ordered]@{ Libre = [uint64]0; Taille = [uint64]1TB }; Add-Nomme $c

$c = New-Cas 'batterie usée'; $c.Batterie = [ordered]@{ Origine = '50000'; Pleine = '27000' }; Add-Nomme $c
$c = New-Cas 'batterie en forme'; $c.Batterie = [ordered]@{ Origine = '50000'; Pleine = '46000' }; Add-Nomme $c
$c = New-Cas 'batterie à 59,5 % arrondi au pair'; $c.Batterie = [ordered]@{ Origine = '40000'; Pleine = '23800' }; Add-Nomme $c
$c = New-Cas 'batterie au-dessus de son origine'; $c.Batterie = [ordered]@{ Origine = ' 41000 '; Pleine = '45100.0' }; Add-Nomme $c
$c = New-Cas 'batterie vide'; $c.Batterie = [ordered]@{ Origine = '41000'; Pleine = '' }; Add-Nomme $c

$c = New-Cas 'redémarré il y a quelques heures'; $c.Demarrage = ([datetime]$c.Maintenant).AddHours(-5).ToString($fmt); Add-Nomme $c
$c = New-Cas 'redémarré il y a 1 jour'; $c.Demarrage = ([datetime]$c.Maintenant).AddDays(-1).ToString($fmt); Add-Nomme $c
$c = New-Cas 'redémarré il y a 0,5 jour (arrondi au pair : 0)'; $c.Demarrage = ([datetime]$c.Maintenant).AddDays(-0.5).ToString($fmt); Add-Nomme $c
$c = New-Cas 'redémarré il y a 6,5 jours (arrondi au pair : 6)'; $c.Demarrage = ([datetime]$c.Maintenant).AddDays(-6.5).ToString($fmt); Add-Nomme $c
$c = New-Cas 'redémarré il y a 7,5 jours (arrondi au pair : 8)'; $c.Demarrage = ([datetime]$c.Maintenant).AddDays(-7.5).ToString($fmt); Add-Nomme $c
$c = New-Cas 'pas redémarré depuis 7 jours'; $c.Demarrage = ([datetime]$c.Maintenant).AddDays(-7).ToString($fmt); Add-Nomme $c
$c = New-Cas 'démarrage dans le futur (horloge changée)'; $c.Demarrage = ([datetime]$c.Maintenant).AddDays(2).ToString($fmt); Add-Nomme $c

$c = New-Cas 'PC en mauvais état, tout à la fois' '2026-03-01T08:00:00'
$c.Sonde.Debut = '2026-02-14T00:00:00.0000000'
Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 1; Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 3
Add-Ev $c 'Microsoft-Windows-WER-SystemErrorReporting' 1001 2 @('0x00000154 (0x0)'); Add-Ev $c 'Microsoft-Windows-WHEA-Logger' 1 2 -Niveau 2
Add-Ev $c 'Microsoft-Windows-MemoryDiagnostics-Results' 1102 4; Add-Ev $c 'disk' 7 5 @('\Device\Harddisk0\DR0'); Add-Ev $c 'Display' 4101 6
Add-Ev $c 'User32' 1074 7 @('C:\Windows\System32\musNotification.exe'); Add-Ev $c 'Microsoft-Windows-Kernel-Processor-Power' 37 8
$c.Place = [ordered]@{ Libre = [uint64]3GB; Taille = [uint64]120GB }; $c.Batterie = [ordered]@{ Origine = '45000'; Pleine = '20000' }
$c.Demarrage = '2026-02-01T08:00:00.0000000'
Add-Nomme $c
# (le 1er janvier, le 29 février... : les mois en français)
foreach ($date in '2028-02-29T10:00:00', '2027-01-01T00:30:00', '2026-08-15T12:00:00', '2026-12-31T23:59:59', '2026-05-01T07:00:00') {
    $c = New-Cas "dates : maintenant $date" $date; $c.Sonde.Debut = ([datetime]$c.Maintenant).AddDays(-12).ToString($fmt)
    Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 0.2; Add-Ev $c 'Microsoft-Windows-Kernel-Power' 41 11; Add-Ev $c 'Microsoft-Windows-MemoryDiagnostics-Results' 1101 3
    Add-Nomme $c
}

# ---------------------------------------------------------------------------
# Les cas au hasard
# ---------------------------------------------------------------------------
$r = New-Object Random($Graine)
function Get-Un($Liste) { $Liste[$r.Next($Liste.Count)] }
$fournisseurs = @(
    @{ N = 'Microsoft-Windows-Kernel-Power'; Ids = @(41, 41, 41, 42, 109) }
    @{ N = 'Microsoft-Windows-WER-SystemErrorReporting'; Ids = @(1001, 1001, 1001, 1000) }
    @{ N = 'Microsoft-Windows-WHEA-Logger'; Ids = @(1, 17, 18, 19, 47) }
    @{ N = 'Microsoft-Windows-MemoryDiagnostics-Results'; Ids = @(1101, 1102, 1201) }
    @{ N = 'disk'; Ids = @(7, 51, 153, 11, 157) }
    @{ N = 'Ntfs'; Ids = @(55, 55, 98, 137) }
    @{ N = 'NTFS'; Ids = @(55) }
    @{ N = 'Display'; Ids = @(4101, 4101, 4100) }
    @{ N = 'User32'; Ids = @(1074, 1074, 1076) }
    @{ N = 'Microsoft-Windows-Kernel-Processor-Power'; Ids = @(37, 37, 55) }
    @{ N = 'Service Control Manager'; Ids = @(7036, 41) }
    @{ N = 'Microsoft-Windows-Kernel-General'; Ids = @(1, 12, 1001) }
)
$codes = @(0x139, 0x1E, 0x154, 0x119, 0x1A, 0x50, 0x4E, 0x12B, 0xA, 0xD1, 0x116, 0x3B, 0x7E, 0x133, 0x9F, 0x7A, 0x24, 0xEF, 0x7B, 0xF4, 0x124, 0x101, 0xC2, 0x19, 3221226010, 2147483651, 0x1000007E)   # (0xC000021A et 0x80000003 en décimal : en hexadécimal, PowerShell en ferait des nombres négatifs)
$programmes = @('C:\Windows\servicing\TrustedInstaller.exe (PC)', 'C:\WINDOWS\system32\MoUsoCoreWorker.exe (PC)', 'C:\Windows\System32\UsoClient.exe', 'C:\Windows\System32\wuauclt.exe', 'C:\Windows\System32\MusNotification.exe', 'C:\Windows\explorer.exe (PC)', 'C:\Windows\System32\shutdown.exe', 'C:\Windows\System32\winlogon.exe (PC)', '')
$erreursLues = @('Accès refusé', 'Le service n''a pas répondu', 'RPC indisponible', 'Échec de la requête')
function Get-Proprietes([string]$Fournisseur) {
    switch -Exact -CaseSensitive ($Fournisseur) {
        'Microsoft-Windows-WER-SystemErrorReporting' {
            $code = [uint32](Get-Un $codes)
            switch ($r.Next(5)) {
                0 { }
                1 { '', ('0x{0:x8}' -f $code) }
                2 { '0x{0:X8} (0x0000000000000001, 0x0)' -f $code }
                default { ('0x{0:x8} (0x{1:x16}, 0x0, 0x0, 0x0)' -f $code, $r.Next()), 'C:\WINDOWS\MEMORY.DMP', ('{0:x8}' -f $r.Next()) }
            }
        }
        'disk' { $n = $r.Next(6); switch ($r.Next(4)) { 0 { } 1 { "\Device\Harddisk$n\DR$($r.Next(9))" } 2 { "\Device\Harddisk$n\DR$n", '' } default { '\Device\00000042' } } }
        'Ntfs' { switch ($r.Next(4)) { 0 { } 1 { "$([char](65 + $r.Next(26))):", '\Device\HarddiskVolume3' } 2 { "$([char](97 + $r.Next(26))):" } default { '\Device\HarddiskVolume7' } } }
        'NTFS' { "$([char](65 + $r.Next(6))):" }
        'User32' { (Get-Un $programmes), 'PC', 'Raison' }
        default { for ($k = 0; $k -lt $r.Next(3); $k++) { "$($r.Next(10000))" } }
    }
}
function Get-Message([string]$Fournisseur) {
    if ($r.Next(3) -eq 0) { return '' }
    switch -Exact -CaseSensitive ($Fournisseur) {
        'Microsoft-Windows-WER-SystemErrorReporting' { "L'ordinateur a redémarré après une vérification d'erreur. La vérification d'erreur était : $('0x{0:x8}' -f [uint32](Get-Un $codes)) (0x0)." }
        'disk' { Get-Un @("Le périphérique \Device\Harddisk$($r.Next(6))\DR1 a un bloc défectueux.", "Une erreur a été détectée sur le disque $($r.Next(6)) pendant une opération de pagination.", "The IO operation at logical block address 0x2 for Disk $($r.Next(6)) was retried.", 'Erreur sans disque nommé', "disk $($r.Next(6))x") }
        'Ntfs' { Get-Un @("Le système de fichiers du volume $([char](65 + $r.Next(26))): est endommagé.", 'Une corruption a été trouvée sur un volume.', 'Volume \\?\Volume{1234}: réparé') }
        default { "Message $($r.Next(1000))" }
    }
}
$tires = New-Object 'Collections.Generic.List[object]'
for ($k = 0; $k -lt $Hasard; $k++) {
    $m = ([datetime]'2026-10-31T00:00:00').AddDays($r.Next(-400, 400)).AddSeconds($r.Next(86400))
    $c = New-Cas "hasard $k" $m.ToString($fmt)
    $etendue = 250.0
    switch ($r.Next(25)) {
        0 { $c.Sonde = [ordered]@{ Erreur = Get-Un $erreursLues } }
        1 { $c.Sonde = [ordered]@{} }
        default { $etendue = 1 + $r.NextDouble() * 400; $c.Sonde = [ordered]@{ Debut = $m.AddDays(-$etendue).AddSeconds($r.Next(60)).ToString($fmt) } }
    }
    $n = if ($r.Next(20) -eq 0) { 150 + $r.Next(90) } else { $r.Next(26) }
    $beaucoup = Get-Un $fournisseurs
    for ($e = 0; $e -lt $n; $e++) {
        $f = if ($n -gt 100 -and $r.Next(4) -gt 0) { $beaucoup } else { Get-Un $fournisseurs }
        Add-Ev $c $f.N (Get-Un $f.Ids) ($r.NextDouble() * [math]::Min($etendue, 250)) @(Get-Proprietes $f.N) (Get-Message $f.N) (1 + $r.Next(4))
    }
    $nd = 1 + $r.Next(4)
    $c.Disques = @(for ($d = 0; $d -lt $nd; $d++) { [ordered]@{ Numero = $d; Bus = (Get-Un @(17, 17, 11, 11, 7, 3, $null, 99)) } })
    $c.Partitions = @(for ($d = 0; $d -lt $nd; $d++) { for ($p = 0; $p -le $r.Next(3); $p++) { [ordered]@{ Disque = $d; Lettre = (Get-Un @(0, 0, (65 + $r.Next(26)), (97 + $r.Next(26)), 67, 68, 49)) } } })
    if ($r.Next(14) -eq 0) { $c.Erreurs.Disques = Get-Un $erreursLues } elseif ($r.Next(20) -eq 0) { $c.Erreurs.Partitions = Get-Un $erreursLues }
    $c.DisquesPhysiques = @(for ($d = 0; $d -lt 1 + $r.Next(3); $d++) { [ordered]@{ Nom = "Disque $d $(Get-Un @('Samsung', 'WDC', 'Crucial', 'Kingston'))"; Bus = (Get-Un @(17, 11, 7, $null, 3)); Sante = (Get-Un @(0, 0, 0, 0, 0, 1, 2, 5, $null, 9)) } })
    if ($r.Next(10) -eq 0) { $c.Lecteur = 'D:' }
    $taille = [uint64](32GB + [uint64]($r.NextDouble() * 4TB))
    $libre = switch ($r.Next(6)) { 0 { [uint64]($taille * 0.095) } 1 { [uint64]($r.Next(40) * 1GB + 0.5GB) } default { [uint64]($r.NextDouble() * $taille) } }
    $c.Place = [ordered]@{ Libre = $libre; Taille = $taille }
    if ($r.Next(2) -eq 0) {
        $origine = 20000 + $r.Next(70000)
        $pleine = switch ($r.Next(4)) { 0 { [math]::Round($origine * 0.595) } 1 { [math]::Round($origine * $r.NextDouble() * 1.1, 1) } default { $r.Next([int]($origine * 1.1)) } }
        $c.Batterie = [ordered]@{ Origine = $(if ($r.Next(5) -eq 0) { " $origine " } else { "$origine" }); Pleine = ([string]$pleine).Replace(',', '.') }
    }
    $jours = switch ($r.Next(5)) { 0 { $r.Next(40) + 0.5 } 1 { $r.Next(40) } default { $r.NextDouble() * 40 } }
    $c.Demarrage = $m.AddDays(-$jours).ToString($fmt)
    $tires.Add((Invoke-Cas $c))
}

# ---------------------------------------------------------------------------
# Les divergences voulues (constats 20 et 21)
# ---------------------------------------------------------------------------
$div20 = New-Object 'Collections.Generic.List[object]'
# Un cas où une mesure rate : les indices de PowerShell, ceux qu'il ne faut PAS donner (Absents), et ce qui reste
function Add-Divergence($Cas, [string[]]$Absents) {
    $j = Invoke-Cas $Cas
    foreach ($a in $Absents) { if (-not $j.Indices.Contains($a)) { throw "« $($Cas.Nom) » : PowerShell ne donne pas l'indice $a" } }
    $voulus = [ordered]@{}
    foreach ($k in @($j.Indices.Keys)) { if ($Absents -notcontains $k) { $voulus[$k] = $j.Indices[$k] } }
    $div20.Add([ordered]@{ Constat = 20; Nom = $j.Nom; Donnees = $j.Donnees; IndicesPowerShell = $j.Indices; Absents = @($Absents); IndicesVoulus = $voulus })
}
$bsod = @('ecransbleus', 'bsod-memoire', 'bsod-pilote', 'bsod-disque', 'bsod-materiel')
$c = New-Cas 'démarrage illisible : « allume » absent'; $c.Erreurs.Demarrage = 'WMI ne répond pas'; Add-Divergence $c @('allume')
$c = New-Cas 'place illisible : « placelibre » absent'; $c.Erreurs.Place = 'Lecteur introuvable'; Add-Divergence $c @('placelibre')
$c = New-Cas 'rapport de batterie illisible : « usurebatterie » absent'; $c.Erreurs.Batterie = 'powercfg a échoué'; Add-Divergence $c @('usurebatterie')
$c = New-Cas 'capacité d''origine de la batterie à 0 : « usurebatterie » absent'; $c.Batterie = [ordered]@{ Origine = '0'; Pleine = '30000' }; Add-Divergence $c @('usurebatterie')
$c = New-Cas 'capacité d''origine de la batterie vide : « usurebatterie » absent'; $c.Batterie = [ordered]@{ Origine = ''; Pleine = '30000' }; Add-Divergence $c @('usurebatterie')
$c = New-Cas 'capacité de la batterie illisible : « usurebatterie » absent'; $c.Batterie = [ordered]@{ Origine = '50000'; Pleine = 'inconnue' }; Add-Divergence $c @('usurebatterie')
$c = New-Cas 'requête des coupures ratée : « coupures » absent'; $c.Echecs = @('Microsoft-Windows-Kernel-Power'); Add-Divergence $c @('coupures')
$c = New-Cas 'requête des écrans bleus ratée : écrans bleus absents'; $c.Echecs = @('Microsoft-Windows-WER-SystemErrorReporting'); Add-Divergence $c $bsod
$c = New-Cas 'requête des erreurs matérielles ratée : « materiel » absent'; $c.Echecs = @('Microsoft-Windows-WHEA-Logger'); Add-Divergence $c @('materiel')
$c = New-Cas 'requête du test de mémoire ratée : tests de mémoire absents'; $c.Echecs = @('Microsoft-Windows-MemoryDiagnostics-Results'); Add-Divergence $c @('testmemoire-erreurs', 'testmemoire-ok')
$c = New-Cas 'requête des erreurs de disque ratée : « disque » absent'; $c.Echecs = @('disk'); Add-Divergence $c @('disque')
$c = New-Cas 'requête des erreurs NTFS ratée : « disque » absent'; $c.Echecs = @('Ntfs'); Add-Divergence $c @('disque')
$c = New-Cas 'santé des disques illisible : « disque » absent'; $c.Erreurs.DisquesPhysiques = 'Accès refusé'; Add-Divergence $c @('disque')
$c = New-Cas 'santé des disques illisible, mais des erreurs vues : « disque » reste vu'; $c.Erreurs.DisquesPhysiques = 'Accès refusé'; Add-Ev $c 'disk' 7 1 @('\Device\Harddisk0\DR0'); Add-Ev $c 'disk' 7 2 @('\Device\Harddisk0\DR0'); Add-Divergence $c @()
$c = New-Cas 'requête NTFS ratée, mais un disque malade : « disque » reste vu'; $c.Echecs = @('Ntfs'); $c.DisquesPhysiques[0].Sante = 2; Add-Divergence $c @()
$c = New-Cas 'requête des erreurs de disque ratée, mais une erreur NTFS vue : « disque » reste vu'; $c.Echecs = @('disk'); Add-Ev $c 'Ntfs' 55 3 @('C:'); Add-Divergence $c @()
$c = New-Cas 'requête du pilote graphique ratée : « pilotegraphique » absent'; $c.Echecs = @('Display'); Add-Divergence $c @('pilotegraphique')
$c = New-Cas 'requête des redémarrages de mise à jour ratée : « majredemarrage » absent'; $c.Echecs = @('User32'); Add-Divergence $c @('majredemarrage')
$c = New-Cas 'requête du processeur ralenti ratée : « freinage » absent'; $c.Echecs = @('Microsoft-Windows-Kernel-Processor-Power'); Add-Divergence $c @('freinage')
$c = New-Cas 'requête ratée dans un journal court : l''indice partiel est absent aussi'; $c.Sonde.Debut = ([datetime]$c.Maintenant).AddDays(-8).ToString($fmt); $c.Echecs = @('Display'); Add-Divergence $c @('pilotegraphique')
$c = New-Cas 'tout rate sauf la sonde : seul « journal » reste'
$c.Sonde.Debut = ([datetime]$c.Maintenant).AddDays(-8).ToString($fmt)
$c.Echecs = @('Microsoft-Windows-Kernel-Power', 'Microsoft-Windows-WER-SystemErrorReporting', 'Microsoft-Windows-WHEA-Logger', 'Microsoft-Windows-MemoryDiagnostics-Results', 'disk', 'Ntfs', 'Display', 'User32', 'Microsoft-Windows-Kernel-Processor-Power')
$c.Erreurs.DisquesPhysiques = 'x'; $c.Erreurs.Place = 'x'; $c.Erreurs.Batterie = 'x'; $c.Erreurs.Demarrage = 'x'
Add-Divergence $c (@('coupures') + $bsod + @('materiel', 'testmemoire-erreurs', 'testmemoire-ok', 'disque', 'pilotegraphique', 'majredemarrage', 'freinage', 'placelibre', 'usurebatterie', 'allume'))
$c = New-Cas 'journal illisible et santé des disques illisible : déjà comme voulu'; $c.Sonde = [ordered]@{ Erreur = 'Accès refusé' }; $c.Erreurs.DisquesPhysiques = 'x'; Add-Divergence $c @()
$c = New-Cas 'aucune batterie dans le rapport : pas une mesure ratée, comme PowerShell'; Add-Divergence $c @()

# Constat 21 : la relecture au début d'une enquête (PowerShell ne relit jamais). « Lancee » : il y a combien de minutes
# la lecture a été lancée ; « Etat » : où elle en est quand l'enquête commence.
$div21 = @(
    [ordered]@{ Constat = 21; Nom = 'lue il y a 5 minutes : gardée'; Lancee = 5; Etat = 'lue'; PowerShell = $false; Relue = $false }
    [ordered]@{ Constat = 21; Nom = 'lue il y a 10 minutes pile : gardée'; Lancee = 10; Etat = 'lue'; PowerShell = $false; Relue = $false }
    [ordered]@{ Constat = 21; Nom = 'lue il y a 11 minutes : relue'; Lancee = 11; Etat = 'lue'; PowerShell = $false; Relue = $true }
    [ordered]@{ Constat = 21; Nom = 'lue il y a 3 heures : relue'; Lancee = 180; Etat = 'lue'; PowerShell = $false; Relue = $true }
    [ordered]@{ Constat = 21; Nom = 'ratée il y a 20 minutes : relue'; Lancee = 20; Etat = 'erreur'; PowerShell = $false; Relue = $true }
    [ordered]@{ Constat = 21; Nom = 'abandonnée (trop lente) il y a 20 minutes, finie depuis : relue'; Lancee = 20; Etat = 'abandon'; PowerShell = $false; Relue = $true }
    [ordered]@{ Constat = 21; Nom = 'encore en cours après 20 minutes : pas relancée'; Lancee = 20; Etat = 'en cours'; PowerShell = $false; Relue = $false }
    [ordered]@{ Constat = 21; Nom = 'jamais lancée : rien à relire'; Lancee = 0; Etat = 'jamais'; PowerShell = $false; Relue = $false }
    [ordered]@{ Constat = 21; Nom = 'horloge revenue en arrière de 5 minutes : relue'; Lancee = -5; Etat = 'lue'; PowerShell = $false; Relue = $true }
)

# ---------------------------------------------------------------------------
# Les barrettes de mémoire (Get-Barrettes)
# ---------------------------------------------------------------------------
# (pas $barrettes : ce serait $script:Barrettes, le cache de Get-Barrettes)
$casBarrettes = New-Object 'Collections.Generic.List[object]'
function Add-Barrettes([string]$Nom, $Liste, [string]$Erreur = '') {
    $c = New-Cas $Nom; $c.Barrettes = @($Liste); if ($Erreur) { $c.Erreurs.Barrettes = $Erreur }
    $script:Cas = $c; $script:Barrettes = $null
    $b = @(Get-Barrettes)
    $casBarrettes.Add([ordered]@{ Nom = $Nom; Barrettes = @($Liste); Erreur = $Erreur; Textes = $b })
}
function New-B($Capacite, $Fabricant, $Reference, $Emplacement) { [ordered]@{ Capacite = $Capacite; Fabricant = $Fabricant; Reference = $Reference; Emplacement = $Emplacement } }
Add-Barrettes 'deux barrettes' @((New-B ([uint64]16GB) 'Kingston' 'KF3200C16D4/16GX' 'DIMM1'), (New-B ([uint64]16GB) 'Kingston' 'KF3200C16D4/16GX' 'DIMM2'))
Add-Barrettes 'espaces autour' @((New-B ([uint64]8GB) '  Samsung ' ' M471A1K43DB1-CTD   ' ' ChannelA-DIMM0 '))
Add-Barrettes 'sans marque ni référence' @((New-B ([uint64]4GB) '' '' 'Slot 1'), (New-B ([uint64]4GB) $null $null $null))
Add-Barrettes 'marque seule, emplacement fait d''espaces' @((New-B ([uint64]32GB) 'Corsair' '' '   '))
Add-Barrettes 'référence seule, emplacement vide' @((New-B ([uint64]2GB) '' 'ABC' ''))
Add-Barrettes 'capacité inconnue' @((New-B $null 'Micron' '8ATF1G64HZ' 'DIMM A'))
Add-Barrettes 'capacités de 1,5 et 2,5 Go (arrondis au pair)' @((New-B ([uint64](1.5 * 1GB)) 'X' 'Y' 'Z'), (New-B ([uint64](2.5 * 1GB)) 'X' 'Y' 'Z'), (New-B ([uint64](512MB)) 'X' 'Y' 'Z'))
Add-Barrettes 'aucune barrette' @()
Add-Barrettes 'WMI illisible' @((New-B ([uint64]8GB) 'A' 'B' 'C')) 'Accès refusé'
for ($k = 0; $k -lt 60; $k++) {
    $l = @(for ($n = 0; $n -lt $r.Next(5); $n++) {
        New-B $(if ($r.Next(8) -eq 0) { $null } else { [uint64]((Get-Un @(0.5, 1, 2, 4, 8, 16, 32, 64, 1.5, 3)) * 1GB + (Get-Un @(0, 0, 0, 1, -1, 512MB))) }) (Get-Un @('Kingston', ' Samsung', 'Micron  ', '', $null, 'SK Hynix', '0x2C00')) (Get-Un @('KF3200C16D4/16GX', ' HMA82GS6CJR8N-XN ', '', $null, '8ATF1G64HZ-3G2J1')) (Get-Un @('DIMM1', 'ChannelB-DIMM0', '', $null, ' ', 'Controller0-ChannelA'))
    })
    Add-Barrettes "barrettes au hasard $k" $l
}

# ---------------------------------------------------------------------------
# L'écriture
# ---------------------------------------------------------------------------
$source = [ordered]@{ Fichier = 'Mindcare/fichiers/modules/journaux.ps1' }
Write-Host 'Juges de la lecture de Windows :'
Write-Juge 'windows-cas.json' ([ordered]@{ Source = (ConvertTo-Json $source -Compress); Cas = (ConvertTo-JsonListe $nommes) })
Write-Juge 'windows-hasard.json' ([ordered]@{ Source = (ConvertTo-Json $source -Compress); Graine = "$Graine"; Cas = (ConvertTo-JsonListe $tires) })
$explication = 'Divergences voulues avec la lecture de Windows de PowerShell (audit du 25/10). Constat 20 : une mesure ratée (une requête au journal qui échoue, la santé des disques, la place, la batterie ou le démarrage illisibles, une capacité de batterie inconnue) qui n''a rien vu ne donne aucun indice (« Absents ») : le moteur n''écarte rien. PowerShell rend « pas vu » et écarte des pistes à tort. Ce qu''une mesure a vu reste vu. Constat 21 : au début d''une enquête, une lecture finie lancée il y a plus de 10 minutes est relancée ; PowerShell ne relit jamais.'
Write-Juge 'windows-divergences.json' ([ordered]@{ Source = (ConvertTo-Json $source -Compress); Explication = (ConvertTo-Json $explication -Compress); Constat20 = (ConvertTo-JsonListe $div20); Constat21 = (ConvertTo-JsonListe $div21) })
Write-Juge 'barrettes.json' ([ordered]@{ Source = (ConvertTo-Json $source -Compress); Graine = "$Graine"; Cas = (ConvertTo-JsonListe $casBarrettes) })
Write-Host "$($nommes.Count) cas nommés, $($tires.Count) au hasard, $($div20.Count + $div21.Count) divergences, $($casBarrettes.Count) cas de barrettes"

$env:TEMP = $ancienTemp; $env:SystemDrive = $ancienLecteur
Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
