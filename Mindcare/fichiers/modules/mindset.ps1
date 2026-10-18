# Mindcare : Mindset, le portail de la suite, qui l'installe, la met à jour, la lance et l'affiche dans son onglet.
# Chargé par Mindcare.ps1 tout au début, avant même de vérifier qu'aucune autre copie ne tourne : une copie qui n'est
# pas celle de Mindset s'arrête tout de suite (Test-BlocageMindset), sans rien démarrer.
#
# %LOCALAPPDATA%\Mindset\suite.json, écrit par Mindset :
#   { "theme": "neon" | "pack:<id>", "applis": { "mindcare": "<dossier de Mindcare.exe>", ... }, "micro": "...", "casque": "..." }
# Mindcare est « gérée par Mindset » si applis.mindcare est le dossier de cette copie.
# Sécurité : n'importe quel programme de l'utilisateur peut modifier ce fichier. On n'y accepte qu'un nom de thème connu,
# et le dossier ne sert qu'à comparer avec le nôtre : rien n'y est jamais lancé ni ouvert.

$SuiteFile   = if ($EnTest) { $env:MINDCARE_SUITE } else { Join-Path $env:LOCALAPPDATA 'Mindset\suite.json' }   # (tests : un faux fichier, ou aucun)
$ThemesSuite = @('neon', 'crepuscule', 'terminal', 'arcade', 'rubis')
$MindsetPage = 'https://github.com/JordanJacquot/Mindset/releases/latest'

# Chemin complet, sans « \ » à la fin (pour comparer deux dossiers), ou $null s'il n'est pas valable
function Get-CheminNormal([string]$Chemin) {
    if ($Chemin -notmatch '^[A-Za-z]:\\') { return $null }
    try { [IO.Path]::GetFullPath($Chemin).TrimEnd('\') } catch { $null }
}

# Ce qu'on garde de suite.json : le thème (connu, ou « pack:<id> » au nom valable) et le dossier de Mindcare
function Read-Suite {
    $s = @{ Existe = $false; Lisible = $false; Theme = $null; Dossier = $null }
    if (-not $SuiteFile -or -not [IO.File]::Exists($SuiteFile)) { return $s }
    $s.Existe = $true
    try {
        $j = [IO.File]::ReadAllText($SuiteFile, [Text.Encoding]::UTF8) | ConvertFrom-Json
        if ($j.theme -is [string] -and $j.theme -in $ThemesSuite) { $s.Theme = $j.theme.ToLowerInvariant() }
        elseif ($j.theme -is [string] -and $j.theme -cmatch '^pack:[a-z0-9-]{1,40}$') { $s.Theme = $j.theme }
        if ($j.applis -and $j.applis.$AppId -is [string]) { $s.Dossier = Get-CheminNormal $j.applis.$AppId }
        $s.Lisible = $true
    } catch {}   # (illisible, ou en train d'être remplacé par Mindset)
    $s
}

$AppRacine = Get-CheminNormal (Split-Path $AppDir -Parent)   # le dossier de cette copie (celui de Mindcare.exe)
$script:Suite = Read-Suite
$GereParMindset = [bool]($AppRacine -and $script:Suite.Dossier -and $script:Suite.Dossier -ieq $AppRacine)

# Le programme de Mindset, donné par son installation (HKLM, modifiable seulement par un administrateur), jamais par suite.json
function Get-MindsetExe {
    try {
        $d = Get-CheminNormal ([string](Get-ItemProperty -LiteralPath 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Mindset' -Name InstallLocation -ErrorAction Stop).InstallLocation).Trim().Trim('"')
        if ($d -and (Test-Path -LiteralPath (Join-Path $d 'Mindset.exe') -PathType Leaf)) { return (Join-Path $d 'Mindset.exe') }
    } catch {}
    $null
}

# Demande à Mindset d'ouvrir une page (« appli:mindcare », « parametres:apparence », « parametres:son ») : la page est
# écrite dans afficher-page.txt, puis Mindset est lancé (s'il tourne déjà, il se ramène et lit le fichier).
# $false si Mindset est introuvable.
function Open-PageMindset([string]$Page) {
    $exe = Get-MindsetExe
    if (-not $exe -or $EnTest) { return $false }
    try {
        $dir = Join-Path $env:LOCALAPPDATA 'Mindset'
        [void][IO.Directory]::CreateDirectory($dir)
        [IO.File]::WriteAllText((Join-Path $dir 'afficher-page.txt'), $Page, (New-Object Text.UTF8Encoding($false)))
        Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe -Parent) -ErrorAction Stop
        $true
    } catch { $false }
}
function Open-DansMindset { Open-PageMindset "appli:$AppId" }

# Dossier de travail (code source) : un dossier .git dans son dossier ou juste au-dessus. Jamais bloqué.
# Pendant les tests : seulement si MINDCARE_TEST_TRAVAIL est défini.
function Test-DossierTravail {
    if ($EnTest) { return [bool]$env:MINDCARE_TEST_TRAVAIL }
    if (-not $AppRacine) { return $false }
    foreach ($d in $AppRacine, (Split-Path $AppRacine -Parent)) { if ($d -and (Test-Path -LiteralPath (Join-Path $d '.git'))) { return $true } }
    $false
}
$DossierTravail = (-not $GereParMindset) -and (Test-DossierTravail)

# Bloquée si elle n'est pas gérée par Mindset, sauf un dossier de travail. Pendant les tests : seulement si
# MINDCARE_TEST_BLOCAGE est défini.
function Test-BlocageMindset {
    if ($GereParMindset) { return $false }
    if ($EnTest) { return [bool]$env:MINDCARE_TEST_BLOCAGE }
    -not $DossierTravail
}

# La fenêtre qui explique comment faire : « Ouvrir Mindset » (s'il est installé) ou « Télécharger Mindset »
# (tests : hors de l'écran, enregistrée en image dans MINDCARE_CAPTURE, puis fermée)
function Show-BlocageMindset {
    $exe = Get-MindsetExe
    $xaml = @"
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="$AppNom" Width="520" SizeToContent="Height" ResizeMode="NoResize" WindowStartupLocation="CenterScreen"
        WindowStyle="None" AllowsTransparency="True" Background="Transparent" FontFamily="Segoe UI Variable Text, Segoe UI">
  <Window.Resources>
    <Style x:Key="Btn" TargetType="Button">
      <Setter Property="Foreground" Value="#EEEBF7"/><Setter Property="FontSize" Value="14"/><Setter Property="FontWeight" Value="SemiBold"/>
      <Setter Property="Cursor" Value="Hand"/><Setter Property="Margin" Value="10,0,0,0"/>
      <Setter Property="Template"><Setter.Value>
        <ControlTemplate TargetType="Button">
          <Border x:Name="Fond" CornerRadius="18" Padding="20,9" Background="#1CFFFFFF" BorderBrush="#33FFFFFF" BorderThickness="1"><ContentPresenter HorizontalAlignment="Center"/></Border>
          <ControlTemplate.Triggers><Trigger Property="IsMouseOver" Value="True"><Setter TargetName="Fond" Property="Background" Value="#2CFFFFFF"/></Trigger></ControlTemplate.Triggers>
        </ControlTemplate>
      </Setter.Value></Setter>
    </Style>
    <Style x:Key="BtnPrincipal" TargetType="Button" BasedOn="{StaticResource Btn}">
      <Setter Property="Foreground" Value="#0C0920"/>
      <Setter Property="Template"><Setter.Value>
        <ControlTemplate TargetType="Button">
          <Border x:Name="Fond" CornerRadius="18" Padding="20,9">
            <Border.Background><LinearGradientBrush StartPoint="0,0" EndPoint="1,0"><GradientStop Color="#00E5FF" Offset="0"/><GradientStop Color="#B04BFF" Offset="1"/></LinearGradientBrush></Border.Background>
            <ContentPresenter HorizontalAlignment="Center"/>
          </Border>
          <ControlTemplate.Triggers><Trigger Property="IsMouseOver" Value="True"><Setter TargetName="Fond" Property="Opacity" Value="0.88"/></Trigger></ControlTemplate.Triggers>
        </ControlTemplate>
      </Setter.Value></Setter>
    </Style>
  </Window.Resources>
  <Grid>
  <Border x:Name="Cadre" CornerRadius="20" Margin="16" Padding="30,26" BorderThickness="1" BorderBrush="#66FFFFFF">
    <Border.Background><LinearGradientBrush StartPoint="0,0" EndPoint="1,1"><GradientStop Color="#0C0920" Offset="0"/><GradientStop Color="#06050F" Offset="1"/></LinearGradientBrush></Border.Background>
    <Border.Effect><DropShadowEffect Color="#FF8A00" BlurRadius="24" ShadowDepth="0" Opacity="0.5"/></Border.Effect>
    <StackPanel>
      <TextBlock Text="$AppNom" FontSize="15" Foreground="#FF8A00" FontWeight="SemiBold"/>
      <TextBlock Text="$AppNom s'utilise avec Mindset" FontSize="21" FontWeight="Bold" Foreground="White" TextWrapping="Wrap" Margin="0,14,0,0"/>
      <TextBlock Text="Installe Mindset, le portail de la suite : il installe $AppNom, le garde à jour et l'affiche dans sa fenêtre." FontSize="14.5" Foreground="#C9C4DD" TextWrapping="Wrap" LineHeight="21" Margin="0,10,0,0"/>
      <StackPanel Orientation="Horizontal" HorizontalAlignment="Right" Margin="0,24,0,0">
        <Button x:Name="BtnFermer" Style="{StaticResource Btn}" Content="Fermer"/>
        <Button x:Name="BtnMindset" Content="$(if ($exe) { 'Ouvrir Mindset' } else { 'Télécharger Mindset' })" Style="{StaticResource BtnPrincipal}" IsDefault="True"/>
      </StackPanel>
    </StackPanel>
  </Border>
  </Grid>
</Window>
"@
    $w = [Windows.Markup.XamlReader]::Parse($xaml)
    $w.FindName('Cadre').Add_MouseLeftButtonDown({ try { $w.DragMove() } catch {} })
    $w.FindName('BtnFermer').Add_Click({ $w.Close() })
    $w.FindName('BtnMindset').Add_Click({
        if (-not $EnTest) { try { if ($exe) { Start-Process -FilePath $exe } else { Start-Process $MindsetPage } } catch {} }
        $w.Close()
    })
    $w.Add_KeyDown({ param($s, $e) if ($e.Key -eq 'Escape') { $w.Close() } })
    if ($EnTest) {
        $w.WindowStartupLocation = 'Manual'; $w.Left = -10000; $w.Top = 0; $w.ShowActivated = $false; $w.ShowInTaskbar = $false
        $w.Add_ContentRendered({ Save-Capture $w.Content $env:MINDCARE_CAPTURE; $w.Close() })
    }
    [void]$w.ShowDialog()
}

# Image d'un élément de la fenêtre (tests automatiques)
function Save-Capture($Element, [string]$Fichier) {
    if (-not $Fichier) { return }
    try {
        $bmp = New-Object System.Windows.Media.Imaging.RenderTargetBitmap([int]$Element.ActualWidth, [int]$Element.ActualHeight, 96, 96, [System.Windows.Media.PixelFormats]::Pbgra32)
        $bmp.Render($Element)
        $enc = New-Object System.Windows.Media.Imaging.PngBitmapEncoder
        $enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($bmp))
        $fs = [IO.File]::Create($Fichier); try { $enc.Save($fs) } finally { $fs.Dispose() }
    } catch { [Console]::Error.WriteLine("Capture : $_") }
}
