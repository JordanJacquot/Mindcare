# Mindcare : le thème de couleurs de la suite (choisi dans Mindset), le même que Mindset, Nevermind et Evermind.
# Toute l'appli est dessinée avec la palette Néon (cyan #00E5FF, violet #B04BFF, magenta #FF2EB5, fonds #0C0920...) ;
# un thème « traduit » chaque couleur au chargement : les couleurs de la marque prennent celles du thème, les gris et
# les fonds prennent sa teinte. Les couleurs d'état (vert, orange, rouge) ne changent jamais.
# Thème perso de la suite (« pack:<id> ») : tout le thème, comme dans Evermind (couleurs, police, image de fond, logo,
# mascotte, dessin du bas, phrase d'accueil), lu dans son réglage pour Mindcare (<données>\packs\<id>, installé par
# Mindset depuis le pack-mindcare.json du zip), sinon dans celui de Mindset (<données de la suite>\Mindset\packs\<id>).
# Chargé par Mindcare.ps1 après donnees.ps1, avant interface.ps1 ; les visuels sont posés après la fenêtre
# (Initialize-ThemeVisuels, appelé par evenements.ps1).
#
# pack.json : Name, Desc ; Base (un thème de base) ou Colors { P, S, T, NH, NS, Map } ; Hello (« {0} » = Bonjour ou
#   Bonsoir) ; Logo (LogoFrames, LogoDelay : planche animée ; LogoGlow : lueur, il lévite) ; Mascotte { File, Frames,
#   Delay, Flip, Vignette (image opaque : vignette arrondie) }, en bas à gauche avec sa bulle ; Loader { File, Frames, Delay } :
#   l'image de chargement, qui tourne pendant « Je lis ta phrase... » ; Footer { File, Frames, Delay, Height } ; Background (BackgroundOpacity, BackgroundAlign) ;
#   Font, FontScope (« titres » ou « tout »), FontPixel (false : police lisse). Les planches : images côte à côte.

$AppThemes = [ordered]@{
    neon       = @{ P = '#00E5FF'; S = '#B04BFF'; T = '#FF2EB5'; Map = @{}; NH = $null; NS = 1.0; Font = $null; Decor = $null }
    crepuscule = @{ P = '#FF8A3D'; S = '#FF3D7F'; T = '#FFC83D'; NH = 345; NS = 0.75; Font = $null
                    Map = @{ '0C0920' = '1A0B12'; '06050F' = '0B0508'; '0B0820' = '170910'; '08060F' = '160806' } }
    terminal   = @{ P = '#33FF77'; S = '#B6FF3B'; T = '#00E0A0'; NH = 140; NS = 0.55; Font = 'Cascadia Code, Consolas'; Decor = 'terminal'
                    Map = @{ '0C0920' = '030B06'; '06050F' = '000302'; '0B0820' = '020A05'; '08060F' = '001A08' } }
    arcade     = @{ P = '#FBD000'; S = '#E52521'; T = '#1E88E5'; NH = 228; NS = 0.9; Font = $null
                    Map = @{ '0C0920' = '0B1640'; '06050F' = '050A20'; '0B0820' = '0A1338'; '08060F' = '1A1000' } }
    rubis      = @{ P = '#FF3B3B'; S = '#FFDE00'; T = '#3B6FFF'; NH = 0; NS = 0.15; Font = $null
                    Map = @{ '0C0920' = '17171D'; '06050F' = '0A0A0D'; '0B0820' = '141419'; '08060F' = '1A0606' } }
}

# Un nom de fichier du pack : simple, sans dossier (rien ne peut sortir du dossier du pack)
function Test-NomFichierPack($Nom) { $Nom -is [string] -and $Nom -match '^[\w .()-]{1,80}$' -and $Nom -notmatch '\.\.' }

# Un thème perso : son réglage pour Mindcare, sinon celui de Mindset (mêmes images). $null s'il n'existe pas.
function Get-ThemePack([string]$Id) {
    if ($Id -notmatch '^[a-z0-9-]{1,40}$') { return $null }
    $mindset = Join-Path (Split-Path $DataDir -Parent) 'Mindset'
    foreach ($dir in @((Join-Path $DataDir "packs\$Id"), (Join-Path $mindset "packs\$Id"))) {
        $f = Join-Path $dir 'pack.json'
        if (-not [IO.File]::Exists($f)) { continue }
        try { $j = [IO.File]::ReadAllText($f, [Text.Encoding]::UTF8) | ConvertFrom-Json } catch { Write-Log "Thème $Id illisible ($f) : $_"; continue }
        $fichier = { param($n) if ((Test-NomFichierPack $n) -and [IO.File]::Exists((Join-Path $dir ([string]$n)))) { Join-Path $dir ([string]$n) } else { $null } }
        $p = @{ Id = $Id; Dir = $dir; Name = [string]$j.Name; Hello = [string]$j.Hello; Base = 'neon'; Font = $null; FontScope = 'titres' }
        $p.Logo = & $fichier $j.Logo
        $p.LogoFrames = [math]::Max(1, [int]$j.LogoFrames); $p.LogoDelay = [math]::Max(30, [int]$j.LogoDelay); $p.LogoGlow = [string]$j.LogoGlow
        $p.Mascotte = $null
        if ($j.Mascotte -and (& $fichier $j.Mascotte.File)) { $p.Mascotte = @{ File = (& $fichier $j.Mascotte.File); Frames = [math]::Max(1, [int]$j.Mascotte.Frames); Delay = [math]::Max(40, [int]$j.Mascotte.Delay); Flip = [bool]$j.Mascotte.Flip; Vignette = [bool]$j.Mascotte.Vignette } }
        $p.Loader = $null
        if ($j.Loader -and (& $fichier $j.Loader.File)) { $p.Loader = @{ File = (& $fichier $j.Loader.File); Frames = [math]::Max(1, [int]$j.Loader.Frames); Delay = [math]::Max(30, [int]$j.Loader.Delay) } }
        $p.Footer = $null
        if ($j.Footer -and (& $fichier $j.Footer.File)) { $p.Footer = @{ File = (& $fichier $j.Footer.File); Frames = [math]::Max(1, [int]$j.Footer.Frames); Delay = [math]::Max(40, [int]$j.Footer.Delay); Height = [math]::Min(48, [math]::Max(16, [int]$j.Footer.Height)) } }
        $p.Background = & $fichier $j.Background
        $p.BackgroundOpacity = if ($j.BackgroundOpacity) { [math]::Min(1.0, [math]::Max(0.05, [double]$j.BackgroundOpacity)) } else { 0.35 }
        $al = ([string]$j.BackgroundAlign).ToLowerInvariant()
        $p.BackgroundAlignX = if ($al -match 'droite') { 'Right' } elseif ($al -match 'gauche') { 'Left' } else { 'Center' }
        $p.BackgroundAlignY = if ($al -match 'bas') { 'Bottom' } elseif ($al -match 'haut') { 'Top' } else { 'Center' }
        if (& $fichier $j.Font) { $p.Font = & $fichier $j.Font; if ([string]$j.FontScope -eq 'tout') { $p.FontScope = 'tout' } }
        $p.FontPixel = if ($null -ne $j.FontPixel) { [bool]$j.FontPixel } else { $true }
        # Ses couleurs : les siennes (un thème « pack-<id> »), sinon un thème de base
        if ($j.Colors -and $j.Colors.P -and $j.Colors.S -and $j.Colors.T) {
            $map = @{}
            if ($j.Colors.Map) { foreach ($pr in $j.Colors.Map.PSObject.Properties) { $map[$pr.Name.TrimStart('#').ToUpper()] = ([string]$pr.Value).TrimStart('#').ToUpper() } }
            $AppThemes["pack-$Id"] = @{ P = [string]$j.Colors.P; S = [string]$j.Colors.S; T = [string]$j.Colors.T; Map = $map; Font = $null; Decor = $null
                NH = $(if ($null -ne $j.Colors.NH) { [double]$j.Colors.NH } else { $null }); NS = $(if ($j.Colors.NS) { [double]$j.Colors.NS } else { 1.0 }) }
            $p.Base = "pack-$Id"
        } elseif ([string]$j.Base -and $AppThemes.Contains([string]$j.Base)) { $p.Base = [string]$j.Base }
        return $p
    }
    $null
}

# Le thème : celui de la suite pour une copie gérée, sinon celui des réglages (copie de travail), sinon Néon
$ThemeSetting = if ($GereParMindset -and $script:Suite.Theme) { $script:Suite.Theme } else { [string](Get-Setting 'Theme' 'neon') }
if ($EnTest -and $env:MINDCARE_THEME) { $ThemeSetting = $env:MINDCARE_THEME }
$ThemePack = $null
$ThemeId = $ThemeSetting
if ($ThemeSetting -like 'pack:*') {
    $ThemePack = Get-ThemePack $ThemeSetting.Substring(5)
    $ThemeId = if ($ThemePack) { $ThemePack.Base } else { Write-Log "Thème $ThemeSetting introuvable : Néon"; 'neon' }
}
if (-not $ThemeId -or -not $AppThemes.Contains($ThemeId)) { $ThemeId = 'neon' }
$Theme = $AppThemes[$ThemeId]

# La police du pack, utilisable comme un nom de police (« file:///dossier/#Nom »)
$PackFont = $null
if ($ThemePack -and $ThemePack.Font) {
    try {
        $gt = New-Object System.Windows.Media.GlyphTypeface (New-Object Uri $ThemePack.Font)
        $PackFont = 'file:///' + ($ThemePack.Dir -replace '\\', '/') + '/#' + @($gt.FamilyNames.Values)[0]
    } catch { Write-Log "Police du thème illisible : $_" }
}

# Attention : PowerShell ne distingue pas $R de $r, d'où des noms différents pour les valeurs 0 à 1
function ConvertTo-Hsl([int]$Red, [int]$Green, [int]$Blue) {
    $r = $Red / 255.0; $g = $Green / 255.0; $b = $Blue / 255.0
    $max = [math]::Max($r, [math]::Max($g, $b)); $min = [math]::Min($r, [math]::Min($g, $b))
    $l = ($max + $min) / 2
    if ($max -eq $min) { return @(0.0, 0.0, $l) }
    $d = $max - $min
    $s = if ($l -gt 0.5) { $d / (2 - $max - $min) } else { $d / ($max + $min) }
    $h = if ($max -eq $r) { (($g - $b) / $d) + $(if ($g -lt $b) { 6 } else { 0 }) } elseif ($max -eq $g) { (($b - $r) / $d) + 2 } else { (($r - $g) / $d) + 4 }
    @(($h * 60.0), $s, $l)
}

function ConvertFrom-Hsl([double]$H, [double]$S, [double]$L) {
    $H = (($H % 360) + 360) % 360
    $c = (1 - [math]::Abs(2 * $L - 1)) * $S
    $x = $c * (1 - [math]::Abs((($H / 60.0) % 2) - 1))
    $m = $L - $c / 2
    $rgb = switch ([int][math]::Floor($H / 60.0)) { 0 { @($c, $x, 0.0) } 1 { @($x, $c, 0.0) } 2 { @(0.0, $c, $x) } 3 { @(0.0, $x, $c) } 4 { @($x, 0.0, $c) } default { @($c, 0.0, $x) } }
    '{0:X2}{1:X2}{2:X2}' -f [int][math]::Round(255 * ($rgb[0] + $m)), [int][math]::Round(255 * ($rgb[1] + $m)), [int][math]::Round(255 * ($rgb[2] + $m))
}

# Couleur Néon (« #RRGGBB » ou « #AARRGGBB ») traduite dans le thème
$script:ThemeCache = @{}
function ConvertTo-ThemeHex([string]$Hex, [string]$Id = $ThemeId) {
    if ($Id -eq 'neon' -or -not $Hex -or $Hex[0] -ne '#' -or ($Hex.Length -ne 7 -and $Hex.Length -ne 9)) { return $Hex }
    $key = "$Id|$Hex"
    if ($script:ThemeCache[$key]) { return $script:ThemeCache[$key] }
    $th = $AppThemes[$Id]
    $alpha = if ($Hex.Length -eq 9) { $Hex.Substring(1, 2) } else { '' }
    $rgb = $Hex.Substring($Hex.Length - 6).ToUpper()
    $out = $rgb
    if ($th.Map.ContainsKey($rgb)) { $out = $th.Map[$rgb] }
    elseif ($rgb -eq '00E5FF') { $out = $th.P.Substring(1) } elseif ($rgb -eq 'B04BFF') { $out = $th.S.Substring(1) } elseif ($rgb -eq 'FF2EB5') { $out = $th.T.Substring(1) }
    else {
        $hsl = ConvertTo-Hsl ([Convert]::ToInt32($rgb.Substring(0, 2), 16)) ([Convert]::ToInt32($rgb.Substring(2, 2), 16)) ([Convert]::ToInt32($rgb.Substring(4, 2), 16))
        $h = $hsl[0]; $s = $hsl[1]; $l = $hsl[2]
        $brand = $null
        if ($s -gt 0.5 -and $l -gt 0.2 -and $l -lt 0.92) {
            if ($h -ge 170 -and $h -lt 200) { $brand = $th.P } elseif ($h -ge 228 -and $h -lt 292) { $brand = $th.S } elseif ($h -ge 292 -and $h -lt 336) { $brand = $th.T }
        }
        if ($brand) {
            # Même clarté que la couleur d'origine, teinte et saturation du thème
            $bh = ConvertTo-Hsl ([Convert]::ToInt32($brand.Substring(1, 2), 16)) ([Convert]::ToInt32($brand.Substring(3, 2), 16)) ([Convert]::ToInt32($brand.Substring(5, 2), 16))
            $dl = $l - 0.5
            $out = if ([math]::Abs($dl) -lt 0.06) { $brand.Substring(1).ToUpper() } else { ConvertFrom-Hsl $bh[0] $bh[1] ([math]::Min(0.95, [math]::Max(0.05, $bh[2] + $dl))) }
        } elseif (($s -lt 0.35 -or $l -lt 0.16) -and $l -gt 0.015 -and $l -lt 0.97 -and $null -ne $th.NH) {
            $out = ConvertFrom-Hsl $th.NH ([math]::Min(1.0, $s * $th.NS)) $l   # gris et fonds : la teinte du thème, même clarté
        }
    }
    $res = "#$alpha$out"
    $script:ThemeCache[$key] = $res
    $res
}

# Une couleur de l'appli (« #RRGGBB »), dans le thème : pinceau prêt à l'emploi
function Get-Brush([string]$Hex) { [System.Windows.Media.BrushConverter]::new().ConvertFromString((ConvertTo-ThemeHex $Hex)) }

# Texte de la fenêtre (interface.xaml) traduit avant d'être chargé : couleurs, et police du thème
function Convert-ThemeXaml([string]$Text) {
    if ($ThemeId -ne 'neon') { $Text = [regex]::Replace($Text, '(?<![&\w])#([0-9A-Fa-f]{8}|[0-9A-Fa-f]{6})(?![0-9A-Fa-f])', { param($m) ConvertTo-ThemeHex $m.Value }) }
    if ($Theme.Font) { $Text = $Text.Replace('Segoe UI Variable Display, Segoe UI', $Theme.Font).Replace('Segoe UI Variable Text, Segoe UI', $Theme.Font) }
    if ($PackFont) { $Text = Add-PackFontXaml $Text }
    if ($ThemePack -and $ThemePack.Background) {
        # Fond en image : les cartes deviennent plus sombres et moins transparentes, le texte reste bien lisible
        $Text = [regex]::Replace($Text, '(<LinearGradientBrush x:Key="CardBg"[^>]*>\s*)<GradientStop Color="#[0-9A-Fa-f]{8}" Offset="0"/>(\s*)<GradientStop Color="#[0-9A-Fa-f]{8}" Offset="1"/>', '$1<GradientStop Color="#E0120F26" Offset="0"/>$2<GradientStop Color="#D80A0818" Offset="1"/>')
    }
    $Text
}

# ---------------------------------------------------------------------------
# La police d'un thème perso : sur les titres (H1, H2), ou partout (« tout »). Une police pixel, bien plus large, passe
# en tailles « pixel exactes » (8, 12, 16 ou 24) : nette, et rien ne déborde.
# ---------------------------------------------------------------------------
function Get-PackFontSize([double]$Size) { if ($Size -lt 15) { 8 } elseif ($Size -lt 21) { 12 } elseif ($Size -lt 30) { 16 } else { 24 } }
function Convert-PackFontSizes([string]$Text) {
    $Text = [regex]::Replace($Text, '<[A-Za-z][^<>]*?FontSize="[0-9.]+"[^<>]*>', {
        param($m)
        [regex]::Replace($m.Value, 'FontSize="([0-9.]+)"', { param($n) 'FontSize="' + (Get-PackFontSize ([double]::Parse($n.Groups[1].Value, [Globalization.CultureInfo]::InvariantCulture))) + '"' })
    })
    $Text = [regex]::Replace($Text, '<Setter Property="FontSize" Value="([0-9.]+)"/>', { param($m) '<Setter Property="FontSize" Value="' + (Get-PackFontSize ([double]::Parse($m.Groups[1].Value, [Globalization.CultureInfo]::InvariantCulture))) + '"/>' })
    [regex]::Replace($Text, '(<Window [^>]*?)FontSize="[0-9.]+"', '$1FontSize="8"', 1)
}
function Add-PackFontXaml([string]$Text) {
    $spec = [Security.SecurityElement]::Escape("$PackFont, Segoe UI Variable Display, Segoe UI")
    $net = if ($ThemePack.FontPixel) { '<Setter Property="TextOptions.TextRenderingMode" Value="Aliased"/><Setter Property="TextOptions.TextFormattingMode" Value="Display"/>' } else { '' }
    $Text = $Text.Replace('Segoe UI Variable Display, Segoe UI', $spec)
    $Text = $Text.Replace('<Style x:Key="H1" TargetType="TextBlock">', '<Style x:Key="H1" TargetType="TextBlock">' + $net)
    $Text = $Text.Replace('<Style x:Key="H2" TargetType="TextBlock">', '<Style x:Key="H2" TargetType="TextBlock">' + $net)
    if ($ThemePack.FontScope -eq 'tout') {
        $Text = $Text.Replace('Segoe UI Variable Text, Segoe UI', $spec)
        if ($ThemePack.FontPixel) {
            $Text = [regex]::Replace($Text, '(<Window [^>]*?)FontFamily=', '$1TextOptions.TextRenderingMode="Aliased" TextOptions.TextFormattingMode="Display" FontFamily=', 1)
            $Text = Convert-PackFontSizes $Text
        }
    }
    $Text
}

# Police pixel partout : les textes créés par le code (New-Text...) passent aussi en tailles pixel, vérifié peu après
# chaque changement d'affichage (au plus 3 fois par seconde)
function Update-TaillesPixel($Racine) {
    $pile = New-Object System.Collections.Stack
    $pile.Push($Racine)
    while ($pile.Count) {
        $el = $pile.Pop()
        if ($el -is [System.Windows.Controls.TextBlock] -or $el -is [System.Windows.Controls.Control]) {
            if ($el.FontSize -notin 8, 12, 16, 24) { $el.FontSize = Get-PackFontSize $el.FontSize }
            if ($PackFont -and $el -is [System.Windows.Controls.TextBlock]) { $el.FontFamily = New-Object System.Windows.Media.FontFamily $PackFont }
        }
        if ($el -is [System.Windows.Media.Visual]) {
            $n = [System.Windows.Media.VisualTreeHelper]::GetChildrenCount($el)
            for ($k = 0; $k -lt $n; $k++) { $pile.Push([System.Windows.Media.VisualTreeHelper]::GetChild($el, $k)) }
        }
    }
}

# ---------------------------------------------------------------------------
# Les visuels du thème, une fois la fenêtre créée : décor (lignes du Terminal), et pour un thème perso l'image de fond,
# le logo, la mascotte, le dessin du bas, la phrase d'accueil. Rien ne s'anime quand la fenêtre est cachée.
# ---------------------------------------------------------------------------
# Planche PNG (images côte à côte) découpée en images prêtes à afficher
function Get-SheetFrames([string]$File, [int]$Count) {
    $bi = New-Object System.Windows.Media.Imaging.BitmapImage
    $bi.BeginInit(); $bi.UriSource = New-Object Uri $File; $bi.CacheOption = 'OnLoad'; $bi.EndInit(); $bi.Freeze()
    $fw = [int]($bi.PixelWidth / [math]::Max(1, $Count))
    @(for ($k = 0; $k -lt $Count; $k++) { $c = New-Object System.Windows.Media.Imaging.CroppedBitmap $bi, ([System.Windows.Int32Rect]::new($k * $fw, 0, $fw, $bi.PixelHeight)); $c.Freeze(); $c })
}

function Start-Defilement($Img, $Frames, [int]$Delay) {
    $t = New-Object System.Windows.Threading.DispatcherTimer
    $t.Interval = [TimeSpan]::FromMilliseconds($Delay)
    $t.Tag = @{ Img = $Img; Frames = $Frames; Frame = 0 }
    $t.Add_Tick({ param($s) if (-not $Window.IsVisible -or $Window.WindowState -eq 'Minimized') { return }; $x = $s.Tag; $x.Frame = ($x.Frame + 1) % $x.Frames.Count; $x.Img.Source = $x.Frames[$x.Frame] })
    if ($Frames.Count -gt 1) { $t.Start() }
    $script:ThemeMinuteries += $t
}

# Une image fixe qui lévite doucement (avec une lueur si une couleur est donnée)
function Start-Levitation($Img, [double]$Haut, [string]$Lueur) {
    $mv = New-Object System.Windows.Media.TranslateTransform
    $Img.RenderTransform = $mv
    $anim = New-Object System.Windows.Media.Animation.DoubleAnimation 0, (-$Haut), ([System.Windows.Duration]::new([TimeSpan]::FromMilliseconds(900)))
    $anim.AutoReverse = $true; $anim.RepeatBehavior = [System.Windows.Media.Animation.RepeatBehavior]::Forever
    $douce = New-Object System.Windows.Media.Animation.SineEase; $douce.EasingMode = 'EaseInOut'; $anim.EasingFunction = $douce
    [System.Windows.Media.Animation.Timeline]::SetDesiredFrameRate($anim, 30)
    $mv.BeginAnimation([System.Windows.Media.TranslateTransform]::YProperty, $anim)
    if ($Lueur) {
        $fx = New-Object System.Windows.Media.Effects.DropShadowEffect
        $fx.Color = [System.Windows.Media.ColorConverter]::ConvertFromString($Lueur); $fx.ShadowDepth = 0; $fx.Opacity = 0.9; $fx.BlurRadius = 10
        $Img.Effect = $fx
    }
}

function New-ImagePack($Frames, [double]$Hauteur) {
    $img = New-Object System.Windows.Controls.Image
    $img.Source = $Frames[0]; $img.Height = $Hauteur; $img.Stretch = 'Uniform'
    [System.Windows.Media.RenderOptions]::SetBitmapScalingMode($img, $(if ($Frames[0].PixelHeight -le $Hauteur) { 'NearestNeighbor' } else { 'HighQuality' }))
    $img
}

function Initialize-ThemeVisuels {
    $script:ThemeMinuteries = @()
    $d = $ui.DecorTheme
    # Lignes de balayage d'un vieil écran (Terminal)
    if ($Theme.Decor -eq 'terminal' -and $d) {
        $lignes = New-Object System.Windows.Media.DrawingBrush
        $lignes.TileMode = 'Tile'; $lignes.Viewport = [System.Windows.Rect]::new(0, 0, 4, 4); $lignes.ViewportUnits = 'Absolute'
        $lignes.Drawing = New-Object System.Windows.Media.GeometryDrawing ([System.Windows.Media.BrushConverter]::new().ConvertFromString('#2233FF77')), $null, ([System.Windows.Media.RectangleGeometry]::new([System.Windows.Rect]::new(0, 0, 4, 1)))
        $r = New-Object System.Windows.Shapes.Rectangle; $r.Fill = $lignes; $r.Opacity = 0.5
        [void]$d.Children.Add($r)
    }
    if (-not $ThemePack) { return }
    try {
        # L'image de fond, sous un voile sombre qui garde le texte lisible
        if ($ThemePack.Background -and $d) {
            $bi = New-Object System.Windows.Media.Imaging.BitmapImage
            $bi.BeginInit(); $bi.UriSource = New-Object Uri $ThemePack.Background; $bi.DecodePixelWidth = 1920; $bi.CacheOption = 'OnLoad'; $bi.EndInit(); $bi.Freeze()
            $pinceau = New-Object System.Windows.Media.ImageBrush $bi
            $pinceau.Stretch = 'UniformToFill'; $pinceau.AlignmentX = $ThemePack.BackgroundAlignX; $pinceau.AlignmentY = $ThemePack.BackgroundAlignY
            $img = New-Object System.Windows.Shapes.Rectangle; $img.Fill = $pinceau; $img.Opacity = $ThemePack.BackgroundOpacity
            [void]$d.Children.Add($img)
            $voile = New-Object System.Windows.Shapes.Rectangle
            $voile.Fill = New-Object System.Windows.Media.LinearGradientBrush ([System.Windows.Media.ColorConverter]::ConvertFromString('#A6000000')), ([System.Windows.Media.ColorConverter]::ConvertFromString('#D0000000')), 90
            [void]$d.Children.Add($voile)
        }
        # Le logo du thème à la place du pouls : animé (planche), ou qui lévite
        if ($ThemePack.Logo -and $ui.LogoPack) {
            $frames = Get-SheetFrames $ThemePack.Logo $ThemePack.LogoFrames
            $ui.LogoPack.Source = $frames[0]; $ui.LogoPack.Visibility = 'Visible'; $ui.LogoAnneau.Visibility = 'Collapsed'
            if ($frames.Count -gt 1) { Start-Defilement $ui.LogoPack $frames $ThemePack.LogoDelay } else { Start-Levitation $ui.LogoPack 3 $ThemePack.LogoGlow }
        }
        # La mascotte, en bas à gauche, avec sa bulle (Set-BulleMascotte : ce qu'elle dit suit l'écran, contenu.ps1)
        if ($ThemePack.Mascotte -and $ui.Mascotte) {
            $M = $ThemePack.Mascotte
            $frames = Get-SheetFrames $M.File $M.Frames
            $h = if ($M.Vignette) { 118 } else { 132 }
            $img = New-ImagePack $frames $h
            if ($ThemePack.FontPixel) { [System.Windows.Media.RenderOptions]::SetBitmapScalingMode($img, 'NearestNeighbor') }   # (pixel art : jamais lissé)
            if ($M.Flip) { $img.RenderTransformOrigin = [System.Windows.Point]::new(0.5, 0.5); $img.RenderTransform = New-Object System.Windows.Media.ScaleTransform -1, 1 }
            if ($M.Vignette) {
                # Image opaque (une scène) : vignette arrondie avec un fin bord aux couleurs du thème
                $img.Stretch = 'UniformToFill'; $img.Width = $h
                $img.Clip = New-Object System.Windows.Media.RectangleGeometry ([System.Windows.Rect]::new(0, 0, $h, $h)), 15, 15
                $v = New-Object System.Windows.Controls.Border
                $v.CornerRadius = 16; $v.BorderThickness = 1; $v.BorderBrush = Get-Brush '#8000E5FF'; $v.Child = $img
                $ui.Mascotte.Child = $v
            } else {
                $fx = New-Object System.Windows.Media.Effects.DropShadowEffect; $fx.ShadowDepth = 5; $fx.BlurRadius = 14; $fx.Opacity = 0.6; $fx.Direction = 270
                $img.Effect = $fx
                $ui.Mascotte.Child = $img
            }
            $ui.ColMascotte.Width = New-Object System.Windows.GridLength 230
            $ui.MascotteZone.Visibility = 'Visible'
            if (-not $ui.MascotteTexte.Text) { $ui.MascotteBulle.Visibility = 'Collapsed' }
            if ($frames.Count -gt 1) { Start-Defilement $img $frames $M.Delay } elseif (-not $M.Vignette) { Start-Levitation $img 5 '' }
        }
        # L'image de chargement du thème, prête pour « Je lis ta phrase... » (New-ChargementTheme)
        if ($ThemePack.Loader) { $script:ChargementImages = Get-SheetFrames $ThemePack.Loader.File $ThemePack.Loader.Frames }
        # Le dessin animé du bas
        if ($ThemePack.Footer -and $ui.DessinBas) {
            $F = $ThemePack.Footer
            $frames = Get-SheetFrames $F.File $F.Frames
            $ui.DessinBas.Child = New-ImagePack $frames $F.Height; $ui.DessinBas.Visibility = 'Visible'
            Start-Defilement $ui.DessinBas.Child $frames $F.Delay
        }
        # La phrase d'accueil du thème (« {0} » = Bonjour ou Bonsoir)
        if ($ThemePack.Hello -and $ui.Phrase) {
            $h = (Get-Date).Hour
            try { $ui.Phrase.Text = $ThemePack.Hello -f $(if ($h -ge 18 -or $h -lt 5) { 'Bonsoir' } else { 'Bonjour' }) } catch { $ui.Phrase.Text = $ThemePack.Hello }
        }
        # Police pixel partout : les textes créés par le code aussi
        if ($PackFont -and $ThemePack.FontScope -eq 'tout' -and $ThemePack.FontPixel) {
            $script:PixelAFaire = $true
            $Window.Add_LayoutUpdated({ $script:PixelAFaire = $true })
            $t = New-Object System.Windows.Threading.DispatcherTimer
            $t.Interval = [TimeSpan]::FromMilliseconds(350)
            $t.Add_Tick({ if ($script:PixelAFaire -and $Window.IsVisible) { $script:PixelAFaire = $false; Update-TaillesPixel $Window } })
            $t.Start(); $script:ThemeMinuteries += $t
        }
        Write-Log "Thème perso : $($ThemePack.Id) ($($ThemePack.Dir))"
    } catch { Write-Log "Thème perso $($ThemePack.Id) : visuels non posés ($_)" }
}

# Ce que dit la mascotte du thème (rien sans thème perso) : la bulle apparaît en grandissant depuis la mascotte
function Set-BulleMascotte([string]$Texte) {
    if (-not $ui.MascotteTexte -or -not ($ThemePack -and $ThemePack.Mascotte)) { return }
    if ($ui.MascotteTexte.Text -eq $Texte -and $ui.MascotteBulle.Visibility -eq 'Visible') { return }
    $ui.MascotteTexte.Text = $Texte
    $b = $ui.MascotteBulle
    $b.Visibility = if ($Texte) { 'Visible' } else { 'Collapsed' }
    if (-not $Texte -or $EnTest) { return }   # (tests : tout de suite, pour la capture)
    $b.RenderTransformOrigin = [System.Windows.Point]::new(0, 1)
    $sc = New-Object System.Windows.Media.ScaleTransform 0.6, 0.6; $b.RenderTransform = $sc
    $ease = New-Object System.Windows.Media.Animation.BackEase; $ease.Amplitude = 0.35; $ease.EasingMode = 'EaseOut'
    $dur = [System.Windows.Duration]::new([TimeSpan]::FromMilliseconds(500))
    foreach ($pr in @([System.Windows.Media.ScaleTransform]::ScaleXProperty, [System.Windows.Media.ScaleTransform]::ScaleYProperty)) {
        $a = New-Object System.Windows.Media.Animation.DoubleAnimation 0.6, 1, $dur; $a.EasingFunction = $ease; $sc.BeginAnimation($pr, $a)
    }
    $b.BeginAnimation([System.Windows.UIElement]::OpacityProperty, (New-Object System.Windows.Media.Animation.DoubleAnimation 0, 1, $dur))
}

# L'image de chargement du thème qui tourne ($null sans thème perso) ; Stop-ChargementTheme l'arrête
function New-ChargementTheme([double]$Taille = 40) {
    if (-not $script:ChargementImages) { return $null }
    Stop-ChargementTheme
    $img = New-Object System.Windows.Controls.Image
    $img.Width = $Taille; $img.Height = $Taille; $img.Stretch = 'Uniform'; $img.Source = $script:ChargementImages[0]
    [System.Windows.Media.RenderOptions]::SetBitmapScalingMode($img, 'HighQuality')
    $t = New-Object System.Windows.Threading.DispatcherTimer
    $t.Interval = [TimeSpan]::FromMilliseconds($ThemePack.Loader.Delay)
    $t.Tag = @{ Img = $img; Frame = 0 }
    $t.Add_Tick({ param($s) if (-not $Window.IsVisible) { return }; $x = $s.Tag; $x.Frame = ($x.Frame + 1) % $script:ChargementImages.Count; $x.Img.Source = $script:ChargementImages[$x.Frame] })
    $t.Start(); $script:ChargementTimer = $t
    $img
}
function Stop-ChargementTheme { if ($script:ChargementTimer) { $script:ChargementTimer.Stop(); $script:ChargementTimer = $null } }

function Stop-ThemeVisuels { foreach ($t in @($script:ThemeMinuteries)) { try { $t.Stop() } catch {} } }
