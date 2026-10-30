# Mindcare : vivre avec Mindset. Chargé par Mindcare.ps1 après interface.ps1 ; tout démarre dans Start-Surveillances.
#
#   Affichée dans Mindset   Mindset accroche la vraie fenêtre dans son onglet (fenêtre enfant, sans bordure). Lancée
#                           cachée, elle n'a jamais été montrée : elle se montre alors sans s'activer, là où Mindset l'a mise.
#   quitter.demande         « Tout quitter » dans Mindset : ce fichier apparaît dans le dossier de données ; l'appli quitte
#                           tout de suite, comme par « Quitter ». Mindset l'arrête de force au bout de 6 secondes sinon.
#   etat.json               la carte vivante dans l'accueil de Mindset (format commun : Titre, Scores, Lignes), écrite
#                           seulement si elle change. Mindset ne fait que l'afficher.
#   suite.json              le thème de la suite change dans Mindset : l'appli se relance avec le nouveau.
#   Signal « Afficher »     un nouveau lancement demande de montrer la fenêtre : dans Mindset si elle est gérée.

$QuitterFichier = Join-Path $DataDir 'quitter.demande'
$EtatFichier    = Join-Path $DataDir 'etat.json'
try { if ([IO.File]::Exists($QuitterFichier)) { [IO.File]::Delete($QuitterFichier) } } catch {}   # (une vieille demande refermerait l'appli tout de suite)
$script:DansMindset = $false

# ---------------------------------------------------------------------------
# Affichée dans Mindset
# ---------------------------------------------------------------------------
function Update-DansMindset {
    $h = (New-Object System.Windows.Interop.WindowInteropHelper $Window).Handle
    if ($h -eq [IntPtr]::Zero) { return }
    $dans = [MindcareNatif]::EstEnfant($h)
    if ($dans -eq $script:DansMindset) { return }
    if (-not $script:TailleMin) { $script:TailleMin = @{ L = $Window.MinWidth; H = $Window.MinHeight } }
    $script:DansMindset = $dans
    if ($dans) {
        $Window.MinWidth = 0; $Window.MinHeight = 0   # Mindset donne la place qu'il a
        if (-not $Window.IsVisible) {
            # Lancée cachée : pour WPF la fenêtre n'a jamais été montrée, Mindset n'afficherait qu'un fond noir
            $place = [MindcareNatif]::PlaceDansParent($h)
            $Window.ShowActivated = $false; $Window.WindowStartupLocation = 'Manual'
            $Window.Show()
            [MindcareNatif]::Placer($h, $place)
        }
    } else { $Window.MinWidth = $script:TailleMin.L; $Window.MinHeight = $script:TailleMin.H }
    Write-Log $(if ($dans) { 'Affichée dans Mindset' } else { 'Plus affichée dans Mindset' })
    Update-EchelleMindset
}

# Dans Mindset, plus petite que la taille minimale : tout le contenu est réduit à l'échelle (rien n'est coupé ni tassé)
function Update-EchelleMindset {
    $e = 1.0
    if ($script:DansMindset -and $Window.ActualWidth -gt 0 -and $Window.ActualHeight -gt 0 -and $script:TailleMin) {
        $e = [math]::Min(1.0, [math]::Min($Window.ActualWidth / $script:TailleMin.L, $Window.ActualHeight / $script:TailleMin.H))
    }
    $Window.Content.LayoutTransform = if ($e -lt 1.0) { New-Object System.Windows.Media.ScaleTransform $e, $e } else { $null }
}

# ---------------------------------------------------------------------------
# Montrer la fenêtre (signal d'un nouveau lancement)
# ---------------------------------------------------------------------------
function Show-MainWindow {
    if ($EnTest -or $script:DansMindset) { return }
    if ($GereParMindset -and (Open-DansMindset)) { return }   # gérée : elle ne s'ouvre que dans Mindset
    try {
        if (-not $Window.IsVisible) { $Window.ShowActivated = $true; $Window.Show() }
        if ($Window.WindowState -eq 'Minimized') { $Window.WindowState = 'Normal' }
        $Window.Topmost = $true; $Window.Topmost = $false
        [void]$Window.Activate()
    } catch { Write-Log "Affichage : $_" }
}

# ---------------------------------------------------------------------------
# Carte vivante dans Mindset (etat.json, format commun)
#   { Version, Maj, Titre, Scores : [ { Nom, Valeur 0 à 100 } ] (2 au plus), Lignes : [ { Texte, Etat } ] (3 au plus) }
#   Etat : ok (vert), info (blanc), attention (orange), urgent (rouge). Textes courts : 80 caractères au plus.
# Ce que la carte montre est décidé par Get-EtatCarte (historique.ps1).
# ---------------------------------------------------------------------------
function Write-EtatMindset {
    if (-not $GereParMindset -and -not $EnTest) { return }
    try {
        $c = Get-EtatCarte
        $etat = [ordered]@{ Version = 1; Titre = [string]$c.Titre; Scores = @($c.Scores | Select-Object -First 2); Lignes = @($c.Lignes | Select-Object -First 3) }
        $cle = ConvertTo-Json -InputObject $etat -Depth 5 -Compress
        if ($cle -eq $script:EtatEcrit) { return }   # (rien de neuf : pas d'écriture)
        $etat.Maj = Get-Date -Format s
        Save-JsonFile $EtatFichier $etat
        $script:EtatEcrit = $cle
    } catch { Write-Log "etat.json pour Mindset : $($_.Exception.Message)" }
}

# ---------------------------------------------------------------------------
# Thème de la suite changé dans Mindset : relance avec le nouveau (cachée si elle l'était)
# ---------------------------------------------------------------------------
function Update-SuiteSurveillance {
    if (-not $GereParMindset -or -not $SuiteFile) { return }
    $fi = New-Object IO.FileInfo $SuiteFile
    $date = if ($fi.Exists) { $fi.LastWriteTimeUtc } else { [datetime]::MinValue }
    # (suite.json pris au lancement : son thème n'a pas pu être lu, il l'est au premier tour où il est libre)
    if ($null -eq $script:SuiteDate -and -not $script:Suite.Pris) { $script:SuiteDate = $date; return }
    if ($date -eq $script:SuiteDate) { return }
    $s = Read-Suite 1   # (un seul essai : la fenêtre ne se fige pas, le tour suivant réessaie)
    if ($s.Existe -and -not $s.Lisible) { return }   # en train d'être remplacé, ou pris : relu au prochain tour
    $script:SuiteDate = $date
    if ($s.Theme -and $s.Theme -ne $ThemeSetting) {
        Write-Log "Thème de la suite changé : $($s.Theme), relance"
        $script:RelancerCachee = -not $Window.IsVisible
        Restart-App
    }
}

# ---------------------------------------------------------------------------
# Les surveillances, une seule minuterie (toutes les 500 ms)
# ---------------------------------------------------------------------------
function Start-Surveillances {
    $script:Tours = 0
    $script:Surveillance = New-Object System.Windows.Threading.DispatcherTimer
    $script:Surveillance.Interval = [TimeSpan]::FromMilliseconds(500)
    $script:Surveillance.Add_Tick((Protect-Action {
        $script:Tours++
        try { Update-DansMindset } catch { Write-Log "Affichage dans Mindset : $_" }
        if ($script:ShowSignal -and $script:ShowSignal.WaitOne(0)) { Show-MainWindow }
        if ([IO.File]::Exists($QuitterFichier)) {
            try { [IO.File]::Delete($QuitterFichier) } catch {}
            Write-Log 'Fermeture demandée par Mindset'
            Exit-App
            return
        }
        if ($script:Tours % 5 -eq 1) { try { Update-SuiteSurveillance } catch { Write-Log "Suite Mindset : $_" } }
        if ($script:Tours % 120 -eq 0) { Write-EtatMindset }   # toutes les minutes (et dès le démarrage, plus bas)
    }))
    $script:Surveillance.Start()
    Write-EtatMindset
    $Window.Add_SizeChanged((Protect-Action { if ($script:DansMindset) { Update-EchelleMindset } }))
}

# ---------------------------------------------------------------------------
# Quitter, relancer
# ---------------------------------------------------------------------------
function Restart-App { $script:Relancer = $true; Exit-App }

function Exit-App {
    if ($script:Quitting) { return }
    $script:Quitting = $true
    Write-Log 'Fermeture'
    try { if ($script:Surveillance) { $script:Surveillance.Stop() } } catch {}
    try { Stop-Contenu } catch { Write-Log "Arrêt du contenu : $_" }   # (contenu.ps1 : ses minuteries, ses travaux de fond)
    try { $Window.Close() } catch {}
    [System.Windows.Threading.Dispatcher]::CurrentDispatcher.InvokeShutdown()
}
