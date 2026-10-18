# Mindcare : les images des questions (dossier images\, une image SVG par fichier : le même format servira au téléphone,
# Android sait les importer). Seule une petite partie de SVG est comprise, assez pour des dessins simples : rect,
# circle, ellipse, line, polyline, polygon, path, text, et g pour grouper (ses réglages passent à ce qu'il contient).
# Les couleurs sont celles du thème Néon, traduites dans le thème de la suite (Get-Brush, themes.ps1).
# Chargé par Mindcare.ps1 avant contenu.ps1 ; outils\tester.ps1 s'en sert aussi tel quel.

$ImagesDir = Join-Path $AppDir 'images'
$script:Invariant = [Globalization.CultureInfo]::InvariantCulture

function Get-Nombre([string]$Valeur, [double]$Defaut = 0) { if ($Valeur) { [double]::Parse($Valeur, $script:Invariant) } else { $Defaut } }

# Un pinceau d'une couleur Néon, avec son opacité ; « none » : rien
function Get-SvgBrush([string]$Couleur, [string]$Opacite) {
    if (-not $Couleur -or $Couleur -eq 'none') { return $null }
    $b = (Get-Brush $Couleur).Clone()
    if ($Opacite) { $b.Opacity = Get-Nombre $Opacite 1 }
    $b
}

# Une image SVG en dessin WPF, qui s'adapte à la place qu'on lui donne ($null si le fichier n'existe pas)
function ConvertFrom-Svg([string]$Nom) {
    $f = Join-Path $ImagesDir "$Nom.svg"
    if (-not [IO.File]::Exists($f)) { return $null }
    $svg = ([xml][IO.File]::ReadAllText($f, [Text.Encoding]::UTF8)).DocumentElement
    $vb = @(([string]$svg.GetAttribute('viewBox')).Trim() -split '[\s,]+' | ForEach-Object { Get-Nombre $_ })
    $toile = New-Object System.Windows.Controls.Canvas
    $toile.Width = $vb[2]; $toile.Height = $vb[3]; $toile.ClipToBounds = $true
    Add-SvgEnfants $toile $svg @{}
    $v = New-Object System.Windows.Controls.Viewbox
    $v.Stretch = 'Uniform'; $v.Child = $toile
    $v
}

function Add-SvgEnfants($Toile, $Noeud, [hashtable]$Herite) {
    foreach ($n in $Noeud.ChildNodes) {
        if ($n.NodeType -ne 'Element') { continue }
        $h = $Herite.Clone()
        foreach ($a in 'fill', 'stroke', 'stroke-width', 'opacity', 'fill-opacity', 'stroke-opacity', 'stroke-linecap', 'stroke-linejoin', 'stroke-dasharray', 'font-size', 'font-weight', 'text-anchor') {
            if ($n.HasAttribute($a)) { $h[$a] = $n.GetAttribute($a) }
        }
        $at = { param($Cle) Get-Nombre $n.GetAttribute($Cle) }
        $forme = $null; $gauche = 0.0; $haut = 0.0
        switch ($n.LocalName) {
            'g'        { Add-SvgEnfants $Toile $n $h }
            'rect'     { $forme = New-Object System.Windows.Shapes.Rectangle
                         $forme.Width = & $at 'width'; $forme.Height = & $at 'height'; $forme.RadiusX = & $at 'rx'; $forme.RadiusY = & $at 'rx'
                         $gauche = & $at 'x'; $haut = & $at 'y' }
            'circle'   { $forme = New-Object System.Windows.Shapes.Ellipse
                         $r = & $at 'r'; $forme.Width = 2 * $r; $forme.Height = 2 * $r
                         $gauche = (& $at 'cx') - $r; $haut = (& $at 'cy') - $r }
            'ellipse'  { $forme = New-Object System.Windows.Shapes.Ellipse
                         $forme.Width = 2 * (& $at 'rx'); $forme.Height = 2 * (& $at 'ry')
                         $gauche = (& $at 'cx') - (& $at 'rx'); $haut = (& $at 'cy') - (& $at 'ry') }
            'line'     { $forme = New-Object System.Windows.Shapes.Line
                         $forme.X1 = & $at 'x1'; $forme.Y1 = & $at 'y1'; $forme.X2 = & $at 'x2'; $forme.Y2 = & $at 'y2' }
            'polyline' { $forme = New-Object System.Windows.Shapes.Polyline; $forme.Points = [System.Windows.Media.PointCollection]::Parse($n.GetAttribute('points')) }
            'polygon'  { $forme = New-Object System.Windows.Shapes.Polygon; $forme.Points = [System.Windows.Media.PointCollection]::Parse($n.GetAttribute('points')) }
            'path'     { $forme = New-Object System.Windows.Shapes.Path; $forme.Data = [System.Windows.Media.Geometry]::Parse($n.GetAttribute('d')) }
            'text'     { Add-SvgTexte $Toile $n $h }
        }
        if (-not $forme) { continue }
        # Comme en SVG : remplie en noir si rien n'est dit, sans contour
        $forme.Fill = Get-SvgBrush $(if ($h.ContainsKey('fill')) { $h['fill'] } else { '#000000' }) $h['fill-opacity']
        $forme.Stroke = Get-SvgBrush $h['stroke'] $h['stroke-opacity']
        $ep = Get-Nombre $h['stroke-width'] 1
        $forme.StrokeThickness = $ep
        if ($h['stroke-linecap'] -eq 'round') { $forme.StrokeStartLineCap = 'Round'; $forme.StrokeEndLineCap = 'Round' }
        if ($h['stroke-linejoin'] -eq 'round') { $forme.StrokeLineJoin = 'Round' }
        if ($h['stroke-dasharray']) {   # (WPF compte les tirets en épaisseurs de trait, SVG en points)
            $tirets = New-Object System.Windows.Media.DoubleCollection
            foreach ($t in ($h['stroke-dasharray'].Trim() -split '[\s,]+')) { $tirets.Add((Get-Nombre $t) / $ep) }
            $forme.StrokeDashArray = $tirets
        }
        if ($h['opacity']) { $forme.Opacity = Get-Nombre $h['opacity'] 1 }
        [System.Windows.Controls.Canvas]::SetLeft($forme, $gauche); [System.Windows.Controls.Canvas]::SetTop($forme, $haut)
        [void]$Toile.Children.Add($forme)
    }
}

# Un texte SVG : x et y donnent le début (ou le milieu, ou la fin) de sa ligne de base
function Add-SvgTexte($Toile, $Noeud, [hashtable]$Style) {
    $t = New-Object System.Windows.Controls.TextBlock
    $t.Text = $Noeud.InnerText
    $t.FontSize = Get-Nombre $Style['font-size'] 16
    if ($Style['font-weight'] -eq 'bold') { $t.FontWeight = 'Bold' }
    $t.Foreground = Get-SvgBrush $(if ($Style['fill']) { $Style['fill'] } else { '#000000' }) $Style['fill-opacity']
    if ($Style['opacity']) { $t.Opacity = Get-Nombre $Style['opacity'] 1 }
    $t.Measure((New-Object System.Windows.Size ([double]::PositiveInfinity), ([double]::PositiveInfinity)))
    $x = Get-Nombre $Noeud.GetAttribute('x')
    if ($Style['text-anchor'] -eq 'middle') { $x -= $t.DesiredSize.Width / 2 } elseif ($Style['text-anchor'] -eq 'end') { $x -= $t.DesiredSize.Width }
    [System.Windows.Controls.Canvas]::SetLeft($t, $x)
    [System.Windows.Controls.Canvas]::SetTop($t, (Get-Nombre $Noeud.GetAttribute('y')) - $t.FontSize * $t.FontFamily.Baseline)
    [void]$Toile.Children.Add($t)
}
