# Mindcare : ce que Windows a noté sur ce PC (son journal « Système », ces 30 derniers jours, et quelques mesures :
# place libre, batterie, dernier redémarrage), pour départager les pistes quand Mindcare est ouvert sur le PC en panne
# (réponse « cepc »). Tout se lit sans droits administrateur et sans rien modifier. La lecture se fait en arrière-plan
# dès le démarrage (quelques secondes, plus sur un PC lent) : l'enquête l'attend sans figer la fenêtre (contenu.ps1).
# Chargé par Mindcare.ps1 avant historique.ps1 ; outils\tester.ps1 s'en sert aussi.
#
#   Un indice : { Vu, Texte (ce qui a été vu), TexteOk (rien de vu : la bonne nouvelle, ou vide), Affiche }
#   Ses points vont aux causes selon la panne (questions.json, « Journaux »).

$JournauxCode = {
    # Les codes d'écran bleu les plus courants, et ce qu'ils accusent le plus souvent
    $script:CodesEcranBleu = @{
        0x139 = @('KERNEL_SECURITY_CHECK_FAILURE', 'pilote');  0x1E  = @('KMODE_EXCEPTION_NOT_HANDLED', 'pilote')
        0x154 = @('UNEXPECTED_STORE_EXCEPTION', 'disque');   0x119 = @('VIDEO_SCHEDULER_INTERNAL_ERROR', 'pilote')
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

    # Les erreurs de disque qui concernent les disques du PC lui-même : une clé USB ou un disque externe qui signale
    # des erreurs ne dit rien du disque du PC. Internes : les numéros des disques du PC ; Lettres : leurs lecteurs (C, D...).
    # Inconnus ($null), ou une erreur qui ne dit pas son disque : elle compte.
    function Select-ErreursDisque($Evenements, $Internes, $Lettres) {
        @(foreach ($e in $Evenements) {
            $t = "$(@($e.Properties | ForEach-Object { $_.Value }) -join ' ') $($e.Message)"
            if ($e.ProviderName -eq 'Ntfs') {
                $m = [regex]::Match($t, '\b([A-Za-z]):')
                if ($m.Success -and $null -ne $Lettres -and @($Lettres) -notcontains $m.Groups[1].Value.ToUpperInvariant()) { continue }
            } else {
                $m = [regex]::Match($t, '(?i)Harddisk(\d+)\\|\b(?:disk|disque)\s+(\d+)\b')
                if ($m.Success -and $null -ne $Internes -and @($Internes) -notcontains [int]"$($m.Groups[1].Value)$($m.Groups[2].Value)") { continue }
            }
            $e
        })
    }

    function Get-IndicesWindows([int]$Jours = 30) {
        $fr = [Globalization.CultureInfo]::GetCultureInfo('fr-FR')
        $depuis = (Get-Date).AddDays(-$Jours)
        $quand = { param($D) $t = $D.ToString('d MMMM', $fr); if ($t -match '^1 ') { '1er' + $t.Substring(1) } else { $t } }
        $i = @{}
        # (Partiel : le journal ne remonte pas assez loin ; s'il n'a rien vu, ça ne prouve rien : le moteur n'écarte aucune piste)
        $ajoute = { param($Id, $Vu, $Texte, $Ok, $Affiche = $true, $Partiel = $false) $i[$Id] = [pscustomobject]@{ Vu = [bool]$Vu; Texte = [string]$Texte; TexteOk = [string]$Ok; Affiche = [bool]$Affiche; Partiel = [bool]$Partiel } }
        $dernier = { param($Ev) if ($Ev.Count -gt 1) { " (le dernier le $(& $quand $Ev[0].TimeCreated))" } else { " (le $(& $quand $Ev[0].TimeCreated))" } }

        # La sonde : le journal Système se lit-il, et depuis quand ? Un journal illisible lu avec un filtre répond « aucun
        # événement », comme un PC sans souci : sans la sonde, ce serait pris pour une bonne nouvelle, et des pistes écartées.
        $illisible = ''; $debut = $null
        try { $debut = @(Get-WinEvent -LogName System -MaxEvents 1 -Oldest -ErrorAction Stop)[0].TimeCreated } catch { $illisible = "$_" }
        if (-not $illisible -and -not $debut) { $illisible = 'journal vide' }
        $court = [bool]($debut -and $debut -gt $depuis)
        $periode = if ($court) { "depuis le $(& $quand $debut)" } else { "ces $Jours jours" }
        if ($illisible) { & $ajoute 'journal' $false '' "Le journal de Windows n'a pas pu être lu ($illisible) : Mindcare ne s'en est pas servie" '' $false }
        elseif ($court) { & $ajoute 'journal' $false '' "Le journal de Windows ne remonte qu'au $(& $quand $debut) : ce qu'il n'a pas vu n'écarte aucune piste" '' $false }

        if (-not $illisible) {
        # Redémarrages et coupures brutales (Windows ne s'est pas arrêté normalement)
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-Kernel-Power'; Id = 41; StartTime = $depuis }
        & $ajoute 'coupures' $ev.Count "$(Get-Quantite $ev.Count 'redémarrage brutal' 'redémarrages brutaux') $periode$(if ($ev.Count) { & $dernier $ev })" "Aucun redémarrage brutal $periode" $true $court

        # Écrans bleus, et ce que leur code accuse
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-WER-SystemErrorReporting'; Id = 1001; StartTime = $depuis }
        $noms = @(); $familles = @{}; $parFamille = @{}
        foreach ($e in $ev) {
            $m = [regex]::Match("$($e.Properties[0].Value) $($e.Message)", '0x([0-9a-fA-F]{8})')
            if (-not $m.Success) { continue }
            $code = [Convert]::ToInt32($m.Groups[1].Value, 16)
            $c = $script:CodesEcranBleu[$code]
            if ($c) { $noms += $c[0]; $familles[$c[1]] = $true; $parFamille[$c[1]] = @($parFamille[$c[1]]) + $c[0] } else { $noms += ('0x{0:X}' -f $code) }
        }
        $noms = @($noms | Select-Object -Unique)
        & $ajoute 'ecransbleus' $ev.Count "$(Get-Quantite $ev.Count 'écran bleu' 'écrans bleus') $periode$(if ($noms) { " ($($noms -join ', '))" })" "Aucun écran bleu $periode" $true $court
        foreach ($f in 'memoire', 'pilote', 'disque', 'materiel') { & $ajoute "bsod-$f" $familles[$f] "Code d'écran bleu noté par Windows : $(@($parFamille[$f] | Where-Object { $_ } | Select-Object -Unique) -join ', ')" '' $false $court }

        # Erreurs matérielles graves, notées par le processeur ou la carte mère
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-WHEA-Logger'; Level = 1, 2; StartTime = $depuis }
        & $ajoute 'materiel' $ev.Count "$(Get-Quantite $ev.Count 'erreur matérielle grave' 'erreurs matérielles graves') $periode" "Aucune erreur matérielle grave $periode" $true $court

        # Le dernier test de la mémoire de Windows (sur 6 mois) : 1101 sans erreur, 1102 avec des erreurs
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-MemoryDiagnostics-Results'; Id = 1101, 1102; StartTime = (Get-Date).AddDays(-180) } 1
        $erreurs = $ev.Count -and $ev[0].Id -eq 1102
        $court6 = [bool]($debut -gt (Get-Date).AddDays(-180))
        & $ajoute 'testmemoire-erreurs' $erreurs $(if ($ev.Count) { "Le test de mémoire du $(& $quand $ev[0].TimeCreated) a trouvé des erreurs" }) '' $true $court6
        & $ajoute 'testmemoire-ok' ($ev.Count -and -not $erreurs) $(if ($ev.Count) { "Le test de mémoire du $(& $quand $ev[0].TimeCreated) n'a trouvé aucune erreur" }) '' $true $court6
        }

        # Le disque du PC : ses erreurs, et ce que Windows dit de sa santé (les clés USB et disques externes ne comptent pas)
        $internes = $null; $lettres = $null
        try {
            $internes = @(Get-Disk -ErrorAction Stop | Where-Object { "$($_.BusType)" -ne 'USB' } | ForEach-Object { [int]$_.Number })
            $lettres = @(Get-Partition -ErrorAction Stop | Where-Object { $internes -contains [int]$_.DiskNumber -and "$($_.DriveLetter)" -match '^[A-Za-z]$' } | ForEach-Object { "$($_.DriveLetter)".ToUpperInvariant() })
        } catch { }
        $ev = if ($illisible) { @() } else { @(Select-ErreursDisque (@(Get-EvenementsSysteme @{ ProviderName = 'disk'; Id = 7, 51, 153; StartTime = $depuis }) + @(Get-EvenementsSysteme @{ ProviderName = 'Ntfs'; Id = 55; StartTime = $depuis })) $internes $lettres) }
        $malades = @(try { Get-PhysicalDisk -ErrorAction Stop | Where-Object { "$($_.BusType)" -ne 'USB' -and $_.HealthStatus -and "$($_.HealthStatus)" -ne 'Healthy' } } catch { })
        $texte = if ($malades) { "Windows dit qu'un disque du PC n'est pas en bonne santé ($($malades[0].FriendlyName))" } else { "Le disque du PC a signalé $(Get-Quantite $ev.Count 'erreur' 'erreurs') $periode" }
        # (journal illisible : seul un disque que Windows dit malade compte ; « en bonne santé » ne se dit pas sans le journal)
        if (-not $illisible -or $malades) { & $ajoute 'disque' ($malades.Count -or $ev.Count) $texte 'Disques en bonne santé' $true $court }

        if (-not $illisible) {

        # Le pilote de la carte graphique qui plante (puis repart)
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Display'; Id = 4101; StartTime = $depuis }
        & $ajoute 'pilotegraphique' $ev.Count "Le pilote de la carte graphique a planté $(Get-Quantite $ev.Count 'fois' 'fois') $periode" "Aucun plantage du pilote graphique $periode" $true $court

        # Redémarrages lancés par les mises à jour de Windows
        $ev = @(Get-EvenementsSysteme @{ ProviderName = 'User32'; Id = 1074; StartTime = $depuis } | Where-Object { "$($_.Properties[0].Value)" -match 'TrustedInstaller|MoUsoCoreWorker|UsoClient|wuauclt|musnotification' })
        & $ajoute 'majredemarrage' $ev.Count "$(Get-Quantite $ev.Count 'redémarrage' 'redémarrages') pour les mises à jour de Windows$(if ($ev.Count) { & $dernier $ev })" "Aucun redémarrage pour les mises à jour $periode" $true $court

        # Le processeur ralenti par la carte mère (événement 37 : « vitesse limitée par le microprogramme » ; sur un
        # portable, c'est souvent la gestion de l'énergie, sinon la chaleur)
        $ev = Get-EvenementsSysteme @{ ProviderName = 'Microsoft-Windows-Kernel-Processor-Power'; Id = 37; StartTime = $depuis }
        & $ajoute 'freinage' $ev.Count "Le processeur a été ralenti $(Get-Quantite $ev.Count 'fois' 'fois') $periode (souvent pour économiser l'énergie, parfois à cause de la chaleur)" "Processeur jamais ralenti $periode" $true $court
        }

        # La place libre sur le disque de Windows
        try {
            $c = Get-PSDrive -Name ($env:SystemDrive.TrimEnd(':')) -ErrorAction Stop
            $libre = [math]::Round($c.Free / 1GB); $part = [math]::Round(100.0 * $c.Free / ($c.Used + $c.Free))
            & $ajoute 'placelibre' ($part -lt 10) "Plus que $libre Go libres sur le disque $($env:SystemDrive) ($part %)" "$libre Go libres sur le disque $($env:SystemDrive)"
        } catch { & $ajoute 'placelibre' $false '' '' $false }

        # L'usure de la batterie (portables) : sa capacité actuelle, comparée à celle d'origine (le rapport de batterie de
        # Windows, sans droits administrateur ; un fichier temporaire, effacé tout de suite)
        try {
            $f = Join-Path $env:TEMP "mindcare-batterie-$PID.xml"
            $null = & "$env:SystemRoot\System32\powercfg.exe" /batteryreport /xml /output $f 2>&1
            $b = $null
            if ([IO.File]::Exists($f)) { $b = @(([xml][IO.File]::ReadAllText($f)).BatteryReport.Batteries.Battery)[0]; Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue }
            $origine = if ($b) { [double]$b.DesignCapacity } else { 0 }
            if ($origine -gt 0) {
                $part = [math]::Round(100.0 * [double]$b.FullChargeCapacity / $origine)
                & $ajoute 'usurebatterie' ($part -lt 60) "La batterie ne garde plus que $part % de sa capacité d'origine" "La batterie garde $part % de sa capacité d'origine"
            } else { & $ajoute 'usurebatterie' $false '' '' $false }
        } catch { & $ajoute 'usurebatterie' $false '' '' $false }

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

# Lecture : la commande lancée en arrière-plan (les tests en donnent une fausse ; sans elle, en test, rien n'est lu)
function Start-LectureJournaux([string]$Lecture = 'Get-IndicesWindows') {
    if ($EnTest -and $Lecture -eq 'Get-IndicesWindows') { return }   # (les tests ne lisent jamais les journaux du vrai PC : voir Get-Indices)
    try {
        $ps = [powershell]::Create()
        [void]$ps.AddScript($JournauxCode.ToString() + "`n" + '$e = $args[0]; try { $e.Indices = & { ' + $Lecture + ' } } catch { $e.Erreur = "$_" }')
        [void]$ps.AddArgument($script:Journaux)
        $script:JournauxTache = @{ PS = $ps; Res = $ps.BeginInvoke() }
    } catch { Write-Log "Journaux de Windows : lecture impossible ($_)"; $script:Journaux.Erreur = "lecture impossible ($_)" }
}

# La lecture est-elle finie (ou abandonnée) ? Jamais d'attente ici : la fenêtre ne doit pas se figer.
# Tests sans fausse lecture : MINDCARE_TEST_JOURNAUX_DELAI (en millisecondes) fait attendre l'enquête comme sur un PC lent.
function Test-JournauxLus {
    if ($script:JournauxAbandon -or $script:Journaux.Indices -or $script:Journaux.Erreur) { return $true }
    if ($script:JournauxTache) { return $script:JournauxTache.Res.IsCompleted }
    if ($EnTest -and $env:MINDCARE_TEST_JOURNAUX_DELAI) {
        if (-not $script:JournauxDebutTest) { $script:JournauxDebutTest = Get-Date }
        return ((Get-Date) - $script:JournauxDebutTest).TotalMilliseconds -ge [int]$env:MINDCARE_TEST_JOURNAUX_DELAI
    }
    $true
}

# La lecture prend trop de temps : l'enquête continue sans elle, et ne s'en servira plus ensuite (sinon les suspects
# changeraient en cours de route quand elle finirait)
function Stop-AttenteJournaux { if (-not (Test-JournauxLus)) { $script:JournauxAbandon = $true } }

# Les indices, si la lecture est finie (sinon aucun : contenu.ps1 attend avant de s'en servir). Tests : ceux de
# MINDCARE_TEST_JOURNAUX, ou ceux de la fausse lecture donnée à Start-LectureJournaux.
function Get-Indices {
    if (-not (Test-JournauxLus)) { return @{} }
    if ($EnTest -and -not $script:JournauxTache) {
        $h = @{}
        if ($env:MINDCARE_TEST_JOURNAUX) {
            try { foreach ($pr in ($env:MINDCARE_TEST_JOURNAUX | ConvertFrom-Json).PSObject.Properties) { $h[$pr.Name] = [pscustomobject]@{ Vu = [bool]$pr.Value.Vu; Texte = [string]$pr.Value.Texte; TexteOk = [string]$pr.Value.TexteOk; Affiche = ($pr.Value.Affiche -ne $false); Partiel = [bool]$pr.Value.Partiel } } }
            catch { Write-Log "MINDCARE_TEST_JOURNAUX illisible : $_" }
        }
        return $h
    }
    if ($script:JournauxAbandon -or -not $script:Journaux.Indices) {
        if (-not $script:JournauxNonLus) {
            $script:JournauxNonLus = $true
            $cause = if ($script:JournauxAbandon) { 'trop longs à lire, enquête continuée sans eux' } elseif ($script:Journaux.Erreur) { $script:Journaux.Erreur } elseif (-not $script:JournauxTache) { 'lecture pas lancée' } else { 'lecture finie sans résultat' }
            Write-Log "Journaux de Windows non lus : $cause"
        }
        return @{}
    }
    if (-not $script:JournauxNotes) {
        $script:JournauxNotes = $true
        Write-Log "Journaux de Windows : $(@($script:Journaux.Indices.GetEnumerator() | Where-Object { $_.Value.Vu } | ForEach-Object { $_.Key }) -join ', ')"
        if ($script:Journaux.Indices['journal']) { Write-Log "Journaux de Windows : $($script:Journaux.Indices['journal'].TexteOk)" }
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
