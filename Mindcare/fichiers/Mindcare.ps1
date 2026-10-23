#Requires -Version 5.1
<#
    Mindcare 0.12.0
    Trouve pourquoi ton PC est en panne, en répondant à quelques questions simples.

    Une appli de la suite Mindset : Mindset l'installe, la met à jour et l'affiche dans son onglet.
    Ses données sont dans le dossier de données que Mindset lui donne (voir modules\donnees.ps1).

    Mindcare.ps1 -Demarrage  lancée cachée : par Mindset (qui l'accroche ensuite dans son onglet) ou avec Windows.
#>
param([switch]$Demarrage)   # (pas d'autre paramètre au nom d'une variable du script : il la cacherait aux fonctions appelées)

$AppVersion = '0.12.0'
$AppNom     = 'Mindcare'
$AppId      = 'mindcare'
$AppDescription = @'
Trouve pourquoi ton PC est en panne, en répondant à quelques questions simples.
'@
$MainScript = $PSCommandPath
$AppDir     = $PSScriptRoot
$ModulesDir = Join-Path $AppDir 'modules'
$AppExe     = Join-Path (Split-Path $AppDir -Parent) 'Mindset.exe'    # le programme qui exécute ce script (Mindcare.exe n'est qu'un relais vers lui)
if (-not (Test-Path -LiteralPath $AppExe)) { $AppExe = Join-Path (Split-Path $AppDir -Parent) 'Mindcare.exe' }
$EnTest     = [bool]$env:MINDCARE_TEST      # test automatique (outils\tester.ps1) : rien n'est modifié sur le PC
if ($EnTest -and $env:MINDCARE_TEST_DEMARRAGE) { $Demarrage = $true }

Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase, System.Drawing, System.Windows.Forms

# ---------------------------------------------------------------------------
# Mindset d'abord : une copie qu'il ne gère pas (ni un dossier de travail) l'explique puis s'arrête ; lancée à la main,
# une copie gérée s'ouvre dans Mindset (qui la lance cachée puis l'accroche dans son onglet).
# ---------------------------------------------------------------------------
. (Join-Path $ModulesDir 'mindset.ps1')
if (Test-BlocageMindset) { Show-BlocageMindset; exit }
if ($GereParMindset -and -not $Demarrage -and -not $EnTest -and (Open-DansMindset)) { exit }

# ---------------------------------------------------------------------------
# Une seule à la fois
# ---------------------------------------------------------------------------
if (-not $EnTest) {
    $cree = $false
    $script:InstanceMutex = New-Object System.Threading.Mutex($true, 'Local\Mindcare-Instance', [ref]$cree)
    if (-not $cree) {
        $libre = $false
        if ($Demarrage) {
            # Mindset la relance juste après une fermeture : l'ancienne libère la place dans quelques secondes
            try { $libre = $script:InstanceMutex.WaitOne(5000) } catch [System.Threading.AbandonedMutexException] { $libre = $true }
        } else {
            # Un clic alors qu'elle tourne déjà : sa fenêtre est ramenée
            try { $s = [System.Threading.EventWaitHandle]::OpenExisting('Local\Mindcare-Afficher'); [void]$s.Set(); $s.Dispose() } catch {}
        }
        if (-not $libre) { exit }
    }
    $script:ShowSignal = New-Object System.Threading.EventWaitHandle($false, [System.Threading.EventResetMode]::AutoReset, 'Local\Mindcare-Afficher')
}

# ---------------------------------------------------------------------------
# Chargement des modules (dossier « modules » à côté de ce fichier)
# ---------------------------------------------------------------------------
$modules = 'donnees', 'themes', 'interface', 'systeme', 'enquete', 'images', 'journaux', 'historique', 'ia', 'contenu', 'evenements'
$manquants = @(@('natif.cs', 'interface.xaml') + @($modules | ForEach-Object { "$_.ps1" }) | Where-Object { -not (Test-Path -LiteralPath (Join-Path $ModulesDir $_)) })
if ($manquants) {
    [System.Windows.MessageBox]::Show("Des fichiers de $AppNom sont manquants :`n`n$($manquants -join ', ')`n`nRéinstalle $AppNom depuis Mindset.", $AppNom, 'OK', 'Error') | Out-Null
    exit
}
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $ModulesDir 'natif.cs'), [Text.Encoding]::UTF8))
try { [MindcareNatif]::SetAppId("JordanJacquot.$AppNom") } catch {}   # son icône dans la barre des tâches, pas celle de PowerShell
foreach ($m in $modules) { . (Join-Path $ModulesDir "$m.ps1") }

# ---------------------------------------------------------------------------
# Lancement
# ---------------------------------------------------------------------------
Write-Log "Démarrage $AppNom $AppVersion (PowerShell $($PSVersionTable.PSVersion))$(if ($Demarrage) { ', lancée cachée' })"
if ($env:MINDCARE_CAPTURE) { Start-Capture $env:MINDCARE_CAPTURE }
if ($Demarrage) {
    # Cachée : la fenêtre existe quand même (Mindset la cherche par son titre pour l'accrocher dans son onglet).
    # Elle ne se montre qu'une fois accrochée (systeme.ps1, Update-DansMindset).
    [void](New-Object System.Windows.Interop.WindowInteropHelper $Window).EnsureHandle()
} else { $Window.Show() }
$script:Debut = Get-Date
[System.Windows.Threading.Dispatcher]::Run()

# Une erreur imprévue a dû fermer Mindcare (modules\donnees.ps1, Register-FiletErreurs) : elle est au journal, et la
# personne le sait au lieu de voir l'appli disparaître
if ($script:Filet.Fatale -and -not $script:Quitting) {
    try { Stop-Contenu } catch {}
    if (-not $EnTest) { [System.Windows.MessageBox]::Show("$AppNom a dû se fermer à cause d'un problème imprévu.`n`nRelance-la. Si ça se répète, utilise « Signaler un problème ».", $AppNom, 'OK', 'Warning') | Out-Null }
}

# Relance demandée (nouveau thème de la suite) : la place est libérée, puis une nouvelle copie démarre
if ($script:Relancer -and -not $EnTest) {
    try { $script:InstanceMutex.ReleaseMutex(); $script:InstanceMutex.Dispose() } catch {}
    try { $script:ShowSignal.Dispose() } catch {}
    $opt = @(if ($script:RelancerCachee) { '-Demarrage' })
    if (Test-Path -LiteralPath $AppExe) { if ($opt) { Start-Process -FilePath $AppExe -ArgumentList $opt } else { Start-Process -FilePath $AppExe } }
    else { Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -ArgumentList (@('-NoProfile', '-ExecutionPolicy', 'Bypass', '-WindowStyle', 'Hidden', '-File', "`"$MainScript`"") + $opt) }
}
# (le programme Mindset.exe termine ensuite le processus : aucun fil de fond ne le garde ouvert sans fenêtre)
