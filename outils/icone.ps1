# Génère l'icône de Mindcare : la lettre M glitchée (échos cyan et magenta, tranches décalées) sur fond sombre,
# comme le N de Nevermind et le E d'Evermind.
#   Mindcare.ico   icône de l'application
#   apercu.png     rendu à plusieurs tailles, sur fond clair et sombre
param([string]$OutDir = (Join-Path $PSScriptRoot 'icones'))

Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function New-RoundedRect([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $p = New-Object Drawing.Drawing2D.GraphicsPath
    $d = 2 * $r
    $p.AddArc($x, $y, $d, $d, 180, 90); $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90); $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    $p
}
function Col([string]$Hex, [int]$Alpha = 255) { $c = [Drawing.ColorTranslator]::FromHtml($Hex); [Drawing.Color]::FromArgb($Alpha, $c.R, $c.G, $c.B) }

# La lettre, en très gras, centrée
function New-LettrePath([float]$s) {
    $p = New-Object Drawing.Drawing2D.GraphicsPath
    $fam = New-Object Drawing.FontFamily 'Segoe UI Black'
    $p.AddString('M', $fam, [int][Drawing.FontStyle]::Regular, [float]($s * 0.56), [Drawing.PointF]::new(0, 0), [Drawing.StringFormat]::GenericTypographic)
    $b = $p.GetBounds()
    $m = New-Object Drawing.Drawing2D.Matrix
    $m.Translate([float](($s - $b.Width) / 2 - $b.X), [float](($s - $b.Height) / 2 - $b.Y))
    $p.Transform($m)
    $p
}

function Fill-Shifted($g, $path, $brush, [float]$dx) {
    $m = New-Object Drawing.Drawing2D.Matrix; $m.Translate($dx, 0)
    $c = $path.Clone(); $c.Transform($m)
    $g.FillPath($brush, $c)
}

function New-IconBitmap([int]$s) {
    $bmp = New-Object Drawing.Bitmap $s, $s, ([Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'; $g.PixelOffsetMode = 'HighQuality'
    $g.Clear([Drawing.Color]::Transparent)
    $m = [math]::Max(0.5, $s * 0.03)
    $bg = New-RoundedRect $m $m ($s - 2 * $m) ($s - 2 * $m) ($s * 0.22)
    $bgBrush = New-Object Drawing.Drawing2D.LinearGradientBrush ([Drawing.PointF]::new(0, 0)), ([Drawing.PointF]::new(0, $s)), (Col '#1D1C2B'), (Col '#07070A')
    $g.FillPath($bgBrush, $bg)
    if ($s -ge 32) { $g.DrawPath((New-Object Drawing.Pen (Col '#FFFFFF' 50), ([float]($s / 128))), $bg) }
    $g.SetClip($bg)
    if ($s -ge 64) { for ($y = 0.08; $y -lt 0.95; $y += 0.03) { $g.FillRectangle((New-Object Drawing.SolidBrush (Col '#FFFFFF' 12)), 0, [float]($s * $y), $s, [float][math]::Max(1, $s * 0.006)) } }
    $e = New-LettrePath $s
    $dx = [float][math]::Max(1, $s * 0.03)
    Fill-Shifted $g $e (New-Object Drawing.SolidBrush (Col '#00E5FF' 225)) (-$dx)
    Fill-Shifted $g $e (New-Object Drawing.SolidBrush (Col '#FF2EB5' 225)) $dx
    $g.FillPath([Drawing.Brushes]::White, $e)
    if ($s -ge 32) {
        foreach ($band in @(@(0.36, 0.05, 0.05), @(0.62, 0.035, -0.04))) {
            $r = [Drawing.RectangleF]::new(0, [float]($s * $band[0]), $s, [float][math]::Max(1, $s * $band[1]))
            $g.SetClip($r, 'Intersect')
            $g.FillRectangle($bgBrush, $r)
            Fill-Shifted $g $e (New-Object Drawing.SolidBrush (Col '#00E5FF')) ([float]($s * $band[2]))
            Fill-Shifted $g $e ([Drawing.Brushes]::White) ([float]($s * $band[2] * 0.6))
            $g.ResetClip(); $g.SetClip($bg)
        }
    }
    $g.ResetClip(); $g.Dispose()
    $bmp
}

function Save-Ico([string]$Path) {
    $sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
    $pngs = foreach ($s in $sizes) { $b = New-IconBitmap $s; $ms = New-Object IO.MemoryStream; $b.Save($ms, [Drawing.Imaging.ImageFormat]::Png); $b.Dispose(); , $ms.ToArray() }
    $out = New-Object IO.MemoryStream
    $bw = New-Object IO.BinaryWriter $out
    $bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $s = $sizes[$i]
        $bw.Write([byte]$(if ($s -ge 256) { 0 } else { $s })); $bw.Write([byte]$(if ($s -ge 256) { 0 } else { $s }))
        $bw.Write([byte]0); $bw.Write([byte]0); $bw.Write([uint16]1); $bw.Write([uint16]32)
        $bw.Write([uint32]$pngs[$i].Length); $bw.Write([uint32]$offset)
        $offset += $pngs[$i].Length
    }
    foreach ($p in $pngs) { $bw.Write($p) }
    $bw.Flush()
    [IO.File]::WriteAllBytes($Path, $out.ToArray())
}

Save-Ico (Join-Path $OutDir 'Mindcare.ico')
$prev = New-Object Drawing.Bitmap 600, 160
$g = [Drawing.Graphics]::FromImage($prev)
$g.Clear((Col '#F3F3F3')); $g.FillRectangle((New-Object Drawing.SolidBrush (Col '#202020')), 300, 0, 300, 160)
foreach ($x0 in 10, 310) { $x = $x0; foreach ($s in 128, 64, 32, 16) { $g.DrawImage((New-IconBitmap $s), $x, 10, $s, $s); $x += $s + 10 } }
$g.Dispose()
$prev.Save((Join-Path $OutDir 'apercu.png'), [Drawing.Imaging.ImageFormat]::Png)
'icône créée'
