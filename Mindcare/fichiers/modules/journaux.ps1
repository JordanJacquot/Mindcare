# Mindcare : ce que Windows a noté sur ce PC (son journal « Système », ces 30 derniers jours), pour départager les
# pistes quand Mindcare est ouvert sur le PC en panne (réponse « cepc »). Tout se lit sans droits administrateur et
# sans rien modifier. La lecture se fait en arrière-plan dès le démarrage (une à deux secondes).
# Chargé par Mindcare.ps1 avant historique.ps1 ; outils\tester.ps1 s'en sert aussi.
#
#   Un indice : { Vu, Texte (ce qui a été vu), TexteOk (rien de vu : la bonne nouvelle, ou vide), Affiche }
#   Ses points vont aux causes selon la panne (questions.json, « Journaux »).

$JournauxCode = {
    # Les codes d'écran bleu les plus courants, et ce qu'ils accusent le plus souvent
    $script:CodesEcranBleu = @{
        0x1A  = @('MEMORY_MANAGEMENT', 'memoire');           0x50  = @('PAGE_FAULT_IN_NONPAGED_AREA', 'memoire')
        0x4E  = @('PFN_LIST_CORRUPT', 'memoire');            0x12B = @('FAULTY_HARDWARE_CORRUPTED_PAGE', 'memoire')
        0xA   = @('IRQL_NOT_LESS_OR_EQUAL', 'pilote');       0xD1  = @('DRIVER_IRQL_NOT_LESS_OR_EQUAL', 'pilote')
        0x116 = @('VIDEO_TDR_FAILURE', 'pilote');            0x3B  = @('SYSTEM_SERVICE_EXCEPTION', 'pilote')
        0x7E  = @('SYSTEM_THREAD_EXCEPTION_NOT_HANDLED', 'pilote'); 0x133 = @('DPC_WATCHDOG_VIOLATION', 'pilote')
        0x9F  = @('DRIVER_POWER_STATE_FAILURE', 'pilote')
        0x7A  = @('KERNEL_DATA_INPAGE_ERROR', 'disque');     0x24  = @('NTFS_FILE_SYSTEM', 'disque')
        0xEF  = @('CRITICAL_PROCESS_DIED', 'disque');        0x7B  = @('INACCESSIBLE_BOOT_DEVICE', 'disque')
        0xF4  = @('CRITICAL_OBJECT_TERMINATION', 'disque')
        0x124 = @('WHEA_UNCORRECTABLE_ERROR', 'materiel');   0x101 = @('CLOCK_WATCHDOG_TIMEOUT', 'materiel')
    }

    # Des événements du journal Système (aucun : une liste vide)
    function Get-EvenementsSysteme([hashtable]$Filtre, [int]$Max = 200) {
        try { @(Get-WinEvent -FilterHashtable ($Filtre + @{ LogName = 'System' }) -MaxEvents $Max -ErrorAction Stop) } catch { @() }
    }
    function Get-Quantite([int]$N, [string]$Un, [string]$Plusieurs) { if ($N -le 1) { "$N $Un" } else { "$N $Plusieurs" } }

    function Get-IndicesWindows([int]$Jours = 30) {
        $fr = [Globalization.CultureInfo]::GetCultureInfo('fr-FR')
        $depuis = (Get-Date).AddDays(-$Jours)
        $quand = { param($D) $t = $D.ToString('d MMMM', $fr); if ($t -match '^1 ') { '1er' + $t.Substring(1) } else { $t } }
        $i = @{}
        $ajoute = { param($Id, $Vu, $Texte, $Ok, $Affiche = $true) $i[$Id] = [pscustomobject]@{ Vu = [bool]$Vu; Texte = [string]$Texte; TexteOk = [string]$Ok; Affiche = [bool]$Affiche } }
        $dernier = { param($Ev) if ($Ev.Count -gt 1) { " (le dernier le $(& $quand $Ev[0].TimeCreated))" } else { " (le $(& $quand $Ev[0].TimeCreated))" } }

        # Redémarrages et coupures brutales (Windows ne s'est pas arrêté normalement)
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-Kernel-Power'; Id = 41; StartTime = $depuis }
        & $ajoute 'coupures' $ev.Count "$(Get-Quantite $ev.Count 'redémarrage brutal' 'redémarrages brutaux') ces $Jours jours$(if ($ev.Count) { & $dernier $ev })" "Aucun redémarrage brutal ces $Jours jours"

        # Écrans bleus, et ce que leur code accuse
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-WER-SystemErrorReporting'; Id = 1001; StartTime = $depuis }
        $noms = @(); $familles = @{}
        foreach ($e in $ev) {
            $m = [regex]::Match("$($e.Properties[0].Value) $($e.Message)", '0x([0-9a-fA-F]{8})')
            if (-not $m.Success) { continue }
            $code = [Convert]::ToInt32($m.Groups[1].Value, 16)
            $c = $script:CodesEcranBleu[$code]
            if ($c) { $noms += $c[0]; $familles[$c[1]] = $true } else { $noms += ('0x{0:X}' -f $code) }
        }
        $noms = @($noms | Select-Object -Unique)
        & $ajoute 'ecransbleus' $ev.Count "$(Get-Quantite $ev.Count 'écran bleu' 'écrans bleus') ces $Jours jours$(if ($noms) { " ($($noms -join ', '))" })" "Aucun écran bleu ces $Jours jours"
        foreach ($f in 'memoire', 'pilote', 'disque', 'materiel') { & $ajoute "bsod-$f" $familles[$f] '' '' $false }

        # Erreurs matérielles graves, notées par le processeur ou la carte mère
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-WHEA-Logger'; Level = 1, 2; StartTime = $depuis }
        & $ajoute 'materiel' $ev.Count "$(Get-Quantite $ev.Count 'erreur matérielle grave' 'erreurs matérielles graves') ces $Jours jours" 'Aucune erreur matérielle grave'

        # Le dernier test de la mémoire de Windows (sur 6 mois) : 1101 sans erreur, 1102 avec des erreurs
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-MemoryDiagnostics-Results'; Id = 1101, 1102; StartTime = (Get-Date).AddDays(-180) } 1
        $erreurs = $ev.Count -and $ev[0].Id -eq 1102
        & $ajoute 'testmemoire-erreurs' $erreurs $(if ($ev.Count) { "Le test de mémoire du $(& $quand $ev[0].TimeCreated) a trouvé des erreurs" }) ''
        & $ajoute 'testmemoire-ok' ($ev.Count -and -not $erreurs) $(if ($ev.Count) { "Le test de mémoire du $(& $quand $ev[0].TimeCreated) n'a trouvé aucune erreur" }) ''

        # Le disque : ses erreurs, et ce que Windows dit de sa santé
        $ev = @(Get-EvenementsSysteme @{ ProviderName = 'disk'; Id = 7, 51, 153; StartTime = $depuis }) + @(Get-EvenementsSysteme @{ ProviderName = 'Ntfs'; Id = 55; StartTime = $depuis })
        $malades = @(try { Get-PhysicalDisk -ErrorAction Stop | Where-Object { $_.HealthStatus -and "$($_.HealthStatus)" -ne 'Healthy' } } catch { })
        $texte = if ($malades) { "Windows dit qu'un disque n'est pas en bonne santé ($($malades[0].FriendlyName))" } else { "Le disque a signalé $(Get-Quantite $ev.Count 'erreur' 'erreurs') ces $Jours jours" }
        & $ajoute 'disque' ($malades.Count -or $ev.Count) $texte 'Disques en bonne santé'

        # Le pilote de la carte graphique qui plante (puis repart)
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Display'; Id = 4101; StartTime = $depuis }
        & $ajoute 'pilotegraphique' $ev.Count "Le pilote de la carte graphique a planté $(Get-Quantite $ev.Count 'fois' 'fois') ces $Jours jours" 'Aucun plantage du pilote graphique'

        # Redémarrages lancés par les mises à jour de Windows
        $ev = @(Get-EvenementsSysteme @{ ProviderName = 'User32'; Id = 1074; StartTime = $depuis } | Where-Object { "$($_.Properties[0].Value)" -match 'TrustedInstaller|MoUsoCoreWorker|UsoClient|wuauclt|musnotification' })
        & $ajoute 'majredemarrage' $ev.Count "$(Get-Quantite $ev.Count 'redémarrage' 'redémarrages') pour les mises à jour de Windows$(if ($ev.Count) { & $dernier $ev })" "Aucun redémarrage pour les mises à jour ces $Jours jours"

        # Le processeur freiné (le plus souvent par la chaleur)
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-Kernel-Processor-Power'; Id = 37; StartTime = $depuis }
        & $ajoute 'freinage' $ev.Count "Le processeur a été freiné $(Get-Quantite $ev.Count 'fois' 'fois') ces $Jours jours (souvent la chaleur)" "Processeur jamais freiné ces $Jours jours"

        # La place libre sur le disque de Windows
        try {
            $c = Get-PSDrive -Name ($env:SystemDrive.TrimEnd(':')) -ErrorAction Stop
            $libre = [math]::Round($c.Free / 1GB); $part = [math]::Round(100.0 * $c.Free / ($c.Used + $c.Free))
            & $ajoute 'placelibre' ($part -lt 10) "Plus que $libre Go libres sur le disque $($env:SystemDrive) ($part %)" "$libre Go libres sur le disque $($env:SystemDrive)"
        } catch { & $ajoute 'placelibre' $false '' '' $false }

        # Le dernier vrai redémarrage (« Arrêter » ne compte pas)
        try {
            $jours = [int]((Get-Date) - (Get-CimInstance Win32_OperatingSystem -ErrorAction Stop).LastBootUpTime).TotalDays
            & $ajoute 'allume' ($jours -ge 7) "Pas vraiment redémarré depuis $jours jours" $(if ($jours -lt 1) { "Redémarré aujourd'hui" } else { "Redémarré il y a $(Get-Quantite $jours 'jour' 'jours')" })
        } catch { & $ajoute 'allume' $false '' '' $false }
        $i
    }
}
. $JournauxCode

# ---------------------------------------------------------------------------
# La lecture en arrière-plan (lancée au démarrage) et son résultat
# ---------------------------------------------------------------------------
$script:Journaux = [hashtable]::Synchronized(@{})

function Start-LectureJournaux {
    if ($EnTest) { return }   # (les tests ne lisent jamais les journaux du vrai PC : voir Get-Indices)
    try {
        $ps = [powershell]::Create()
        [void]$ps.AddScript($JournauxCode.ToString() + "`n" + '$e = $args[0]; try { $e.Indices = Get-IndicesWindows } catch { $e.Erreur = "$_" }')
        [void]$ps.AddArgument($script:Journaux)
        $script:JournauxTache = @{ PS = $ps; Res = $ps.BeginInvoke() }
    } catch { Write-Log "Journaux de Windows : lecture impossible ($_)" }
}

# Les indices (attend la fin de la lecture, quelques secondes au plus). Tests : ceux de MINDCARE_TEST_JOURNAUX.
function Get-Indices {
    if ($EnTest) {
        $h = @{}
        if ($env:MINDCARE_TEST_JOURNAUX) {
            try { foreach ($pr in ($env:MINDCARE_TEST_JOURNAUX | ConvertFrom-Json).PSObject.Properties) { $h[$pr.Name] = [pscustomobject]@{ Vu = [bool]$pr.Value.Vu; Texte = [string]$pr.Value.Texte; TexteOk = [string]$pr.Value.TexteOk; Affiche = ($pr.Value.Affiche -ne $false) } } }
            catch { Write-Log "MINDCARE_TEST_JOURNAUX illisible : $_" }
        }
        return $h
    }
    if (-not $script:Journaux.Indices -and $script:JournauxTache) { [void]$script:JournauxTache.Res.AsyncWaitHandle.WaitOne(8000) }
    if (-not $script:Journaux.Indices) { Write-Log "Journaux de Windows non lus : $($script:Journaux.Erreur)"; return @{} }
    if (-not $script:JournauxNotes) {
        $script:JournauxNotes = $true
        Write-Log "Journaux de Windows : $(@($script:Journaux.Indices.GetEnumerator() | Where-Object { $_.Value.Vu } | ForEach-Object { $_.Key }) -join ', ')"
    }
    $script:Journaux.Indices
}

function Stop-LectureJournaux { try { if ($script:JournauxTache) { $script:JournauxTache.PS.Stop(); $script:JournauxTache.PS.Dispose() } } catch {} }

# Les barrettes de mémoire de ce PC (taille, marque, référence, emplacement), pour la réponse « mémoire » et le rapport :
# lu une fois, sans droits administrateur. Tests : MINDCARE_TEST_BARRETTES (rien du vrai PC).
function Get-Barrettes {
    if ($EnTest) { return @($env:MINDCARE_TEST_BARRETTES -split '\|' | Where-Object { $_ }) }
    if ($null -ne $script:Barrettes) { return $script:Barrettes }
    $script:Barrettes = @()
    try {
        $script:Barrettes = @(Get-CimInstance Win32_PhysicalMemory -ErrorAction Stop | ForEach-Object {
            $go = [math]::Round($_.Capacity / 1GB)
            $ref = ("$(([string]$_.Manufacturer).Trim()) $(([string]$_.PartNumber).Trim())").Trim()
            "$go Go$(if ($ref) { " $ref" })$(if ($_.DeviceLocator) { " ($(([string]$_.DeviceLocator).Trim()))" })"
        })
    } catch { Write-Log "Barrettes de mémoire illisibles : $_" }
    $script:Barrettes
}
