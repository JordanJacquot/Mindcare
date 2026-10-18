# Mindcare : le thème de couleurs de la suite (choisi dans Mindset), le même que Mindset, Nevermind et Evermind.
# Toute l'appli est dessinée avec la palette Néon (cyan #00E5FF, violet #B04BFF, magenta #FF2EB5, fonds #0C0920...) ;
# un thème « traduit » chaque couleur au chargement : les couleurs de la marque prennent celles du thème, les gris et
# les fonds prennent sa teinte. Les couleurs d'état (vert, orange, rouge) ne changent jamais.
# Thème perso de la suite (« pack:<id> ») : ses couleurs, lues dans son réglage pour Mindcare (<données>\packs\<id>),
# sinon dans celui de Mindset (<données de la suite>\Mindset\packs\<id>). Les images du pack ne sont pas utilisées.
# Chargé par Mindcare.ps1 après donnees.ps1, avant interface.ps1.

$AppThemes = [ordered]@{
    neon       = @{ P = '#00E5FF'; S = '#B04BFF'; T = '#FF2EB5'; Map = @{}; NH = $null; NS = 1.0; Font = $null }
    crepuscule = @{ P = '#FF8A3D'; S = '#FF3D7F'; T = '#FFC83D'; NH = 345; NS = 0.75; Font = $null
                    Map = @{ '0C0920' = '1A0B12'; '06050F' = '0B0508'; '0B0820' = '170910'; '08060F' = '160806' } }
    terminal   = @{ P = '#33FF77'; S = '#B6FF3B'; T = '#00E0A0'; NH = 140; NS = 0.55; Font = 'Cascadia Code, Consolas'
                    Map = @{ '0C0920' = '030B06'; '06050F' = '000302'; '0B0820' = '020A05'; '08060F' = '001A08' } }
    arcade     = @{ P = '#FBD000'; S = '#E52521'; T = '#1E88E5'; NH = 228; NS = 0.9; Font = $null
                    Map = @{ '0C0920' = '0B1640'; '06050F' = '050A20'; '0B0820' = '0A1338'; '08060F' = '1A1000' } }
    rubis      = @{ P = '#FF3B3B'; S = '#FFDE00'; T = '#3B6FFF'; NH = 0; NS = 0.15; Font = $null
                    Map = @{ '0C0920' = '17171D'; '06050F' = '0A0A0D'; '0B0820' = '141419'; '08060F' = '1A0606' } }
}

# Les couleurs d'un thème perso : { Base } ou { Colors = { P, S, T, NH, NS, Map } } dans son pack.json
function Get-ThemePack([string]$Id) {
    if ($Id -notmatch '^[a-z0-9-]{1,40}$') { return $null }
    $mindset = Join-Path (Split-Path $DataDir -Parent) 'Mindset'
    foreach ($f in @((Join-Path $DataDir "packs\$Id\pack.json"), (Join-Path $mindset "packs\$Id\pack.json"))) {
        if (-not [IO.File]::Exists($f)) { continue }
        try { $j = [IO.File]::ReadAllText($f, [Text.Encoding]::UTF8) | ConvertFrom-Json } catch { Write-Log "Thème $Id illisible ($f) : $_"; continue }
        if ($j.Colors -and $j.Colors.P -and $j.Colors.S -and $j.Colors.T) {
            $map = @{}
            if ($j.Colors.Map) { foreach ($pr in $j.Colors.Map.PSObject.Properties) { $map[$pr.Name.TrimStart('#').ToUpper()] = ([string]$pr.Value).TrimStart('#').ToUpper() } }
            $AppThemes["pack-$Id"] = @{ P = [string]$j.Colors.P; S = [string]$j.Colors.S; T = [string]$j.Colors.T; Map = $map; Font = $null
                NH = $(if ($null -ne $j.Colors.NH) { [double]$j.Colors.NH } else { $null }); NS = $(if ($j.Colors.NS) { [double]$j.Colors.NS } else { 1.0 }) }
            return "pack-$Id"
        }
        if ([string]$j.Base -and $AppThemes.Contains([string]$j.Base)) { return [string]$j.Base }
    }
    $null
}

# Le thème : celui de la suite pour une copie gérée, sinon celui des réglages (copie de travail), sinon Néon
$ThemeSetting = if ($GereParMindset -and $script:Suite.Theme) { $script:Suite.Theme } else { [string](Get-Setting 'Theme' 'neon') }
if ($EnTest -and $env:MINDCARE_THEME) { $ThemeSetting = $env:MINDCARE_THEME }
$ThemeId = $ThemeSetting
if ($ThemeSetting -like 'pack:*') { $ThemeId = Get-ThemePack $ThemeSetting.Substring(5); if (-not $ThemeId) { Write-Log "Thème $ThemeSetting introuvable : Néon" } }
if (-not $ThemeId -or -not $AppThemes.Contains($ThemeId)) { $ThemeId = 'neon' }
$Theme = $AppThemes[$ThemeId]

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
    if ($ThemeId -eq 'neon') { return $Text }
    $Text = [regex]::Replace($Text, '(?<![&\w])#([0-9A-Fa-f]{8}|[0-9A-Fa-f]{6})(?![0-9A-Fa-f])', { param($m) ConvertTo-ThemeHex $m.Value })
    if ($Theme.Font) { $Text = $Text.Replace('Segoe UI Variable Display, Segoe UI', $Theme.Font).Replace('Segoe UI Variable Text, Segoe UI', $Theme.Font) }
    $Text
}
