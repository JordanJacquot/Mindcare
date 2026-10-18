# Mindcare : la fenêtre (interface.xaml), traduite dans le thème de la suite, et ses éléments nommés dans $ui.
# Chargé par Mindcare.ps1 après themes.ps1.

$xamlTexte = Convert-ThemeXaml ([IO.File]::ReadAllText((Join-Path $ModulesDir 'interface.xaml'), [Text.Encoding]::UTF8))
try { $Window = [Windows.Markup.XamlReader]::Parse($xamlTexte) }
catch {
    # Sans fenêtre, l'appli ne peut rien faire : elle le dit, le note, et s'arrête (au lieu de tourner sans fenêtre)
    $m = if ($_.Exception.InnerException) { $_.Exception.InnerException.Message } else { $_.Exception.Message }
    Write-Log "Fenêtre illisible (interface.xaml) : $m"
    [Console]::Error.WriteLine("interface.xaml : $m")
    if (-not $EnTest) { [System.Windows.MessageBox]::Show("$AppNom n'a pas pu afficher sa fenêtre.`n`n$m", $AppNom, 'OK', 'Error') | Out-Null }
    exit
}

# Chaque élément qui a un x:Name : $ui.Contenu, $ui.Phrase...
$ui = @{}
foreach ($m in [regex]::Matches($xamlTexte, 'x:Name="([A-Za-z0-9_]+)"')) {
    $n = $m.Groups[1].Value
    $o = $Window.FindName($n)
    if ($o) { $ui[$n] = $o }
}

# Barre de titre sombre (de la couleur du fond sur Windows 11) et icône de l'appli
$Window.Add_SourceInitialized({
    try { [MindcareNatif]::SetDarkTitleBar((New-Object System.Windows.Interop.WindowInteropHelper $Window).Handle, 0x200C0C) } catch {}
})
$ico = Join-Path $AppDir 'Mindcare.ico'
if (Test-Path -LiteralPath $ico) { try { $Window.Icon = [System.Windows.Media.Imaging.BitmapFrame]::Create((New-Object Uri $ico)) } catch {} }

# Petits outils pour construire l'interface en code (contenu.ps1)
function New-Text([string]$Texte, [double]$Taille = 14, [string]$Couleur = '#EEEBF7', [string]$Graisse = 'Normal') {
    $t = New-Object System.Windows.Controls.TextBlock
    $t.Text = $Texte; $t.FontSize = $Taille; $t.Foreground = Get-Brush $Couleur; $t.FontWeight = $Graisse; $t.TextWrapping = 'Wrap'
    $t
}
function New-Button([string]$Texte, [string]$Style = 'BtnPrimary') {
    $b = New-Object System.Windows.Controls.Button
    $b.Content = $Texte; $b.Style = $Window.FindResource($Style)
    $b
}
function New-Card {
    $c = New-Object System.Windows.Controls.Border
    $c.Style = $Window.FindResource('Card')
    $sp = New-Object System.Windows.Controls.StackPanel
    $c.Child = $sp
    [void]$ui.Contenu.Children.Add($c)
    $sp
}

# Capture d'écran (tests automatiques) : la fenêtre est dessinée hors de l'écran, enregistrée en image, puis l'appli quitte
function Start-Capture([string]$Fichier) {
    $Window.WindowStartupLocation = 'Manual'; $Window.Left = -10000; $Window.Top = 0; $Window.ShowActivated = $false; $Window.ShowInTaskbar = $false
    $Window.Add_ContentRendered({
        $t = New-Object System.Windows.Threading.DispatcherTimer
        $t.Interval = [TimeSpan]::FromMilliseconds(700)
        $t.Add_Tick({ param($s) $s.Stop(); Save-Capture $Window.Content $env:MINDCARE_CAPTURE; if (-not $env:MINDCARE_TEST_RESTER) { Exit-App } })
        $t.Start()
    })
}
