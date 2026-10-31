# Mindcare : CE QUE FAIT L'APPLI. L'accueil (le catalogue : les familles de pannes, et la recherche par mots), une famille,
# l'enquête (une étape à la fois, les suspects à gauche) et la réponse. Le moteur est dans enquete.ps1, ce que Windows a
# noté dans journaux.ps1, les diagnostics gardés et la carte de Mindset dans historique.ps1.
# Chargé par Mindcare.ps1 après historique.ps1 ; Initialize-Contenu est appelé par evenements.ps1.

$script:Donnees      = $null   # le contenu de enquetes.json
$script:PanneEnCours = $null   # la panne de l'enquête en cours
$script:ChoixFaits   = @()     # les réponses données jusqu'ici (voir enquete.ps1)
$script:DiagId       = $null   # l'enquête en cours, telle qu'elle est gardée (historique.ps1)

function Initialize-Contenu {
    try { $script:Donnees = Read-Enquetes } catch { Write-Log "Enquêtes illisibles ($EnquetesFichier) : $_" }
    Read-Historique
    Start-LectureJournaux   # (ce que Windows a noté, lu en arrière-plan pour l'enquête)
    # (l'assistant n'est pas lancé ici, il prendrait la carte graphique pour rien : seulement quand on écrit dans la recherche)
    if ($EnTest -and $env:MINDCARE_TEST_ERREUR) {   # (tests : une minuterie qui se trompe à chaque tour, pour le filet)
        $t = New-Object System.Windows.Threading.DispatcherTimer; $t.Interval = [TimeSpan]::FromMilliseconds(50)
        $t.Add_Tick((Protect-Action { $script:MinuterieAbsente.Stop() })); $t.Start()
    }
    if ($EnTest -and $env:MINDCARE_TEST_BAS) { $Window.Add_ContentRendered((Protect-Action { $ui.Defilement.ScrollToEnd() })) }   # (tests : capture du bas)
    if ($EnTest -and $env:MINDCARE_TEST_GALERIE) {
        $g = New-Object System.Windows.Controls.WrapPanel
        foreach ($f in Get-ChildItem -LiteralPath $ImagesDir -Filter '*.svg') { $v = ConvertFrom-Svg $f.BaseName; $v.Width = 150; $v.Height = 100; $v.Margin = '0,0,12,12'; [void]$g.Children.Add($v) }
        [void]$ui.Contenu.Children.Add($g)
        return
    }
    if ($EnTest -and $script:Donnees) {
        if ($env:MINDCARE_TEST_REPRENDRE -and $script:Historique.Count) { Open-Diagnostic ([string]$script:Historique[0].Id); return }
        if ($env:MINDCARE_TEST_FAMILLE) { Show-Famille $env:MINDCARE_TEST_FAMILLE; return }
        # La dictée, pour les captures : 'installer' (le bandeau), 'ecoute' (le micro allumé), 'texte' (la phrase écrite)
        if ($env:MINDCARE_TEST_DICTEE) {
            Show-Accueil
            switch ($env:MINDCARE_TEST_DICTEE) { 'installer' { Invoke-BoutonMicro } 'ecoute' { Show-DicteeEcoute; Update-DicteeNiveau 0.3 } 'texte' { Set-DicteeTexte 'mon pc s''éteint quand je joue' } }
            return
        }
        if ($env:MINDCARE_TEST_RECHERCHE) { Show-Accueil; $ui.Recherche.Text = $env:MINDCARE_TEST_RECHERCHE; if ($env:MINDCARE_TEST_IA -or $env:MINDCARE_IA_MODELE) { Request-Ia; if ($env:MINDCARE_TEST_IA_DOUBLE) { Request-Ia } }; return }   # (DOUBLE : Entrée appuyée deux fois)
        # Une enquête toute faite : « redemarre » ou « redemarre:1,0,-1 » (outils\tester.ps1)
        if ($env:MINDCARE_TEST_PARCOURS) {
            $id, $suite = $env:MINDCARE_TEST_PARCOURS -split ':', 2
            $p = Get-Panne $script:Donnees $id
            if ($p) {
                $script:PanneEnCours = $p; $script:DiagId = New-DiagId
                $script:ChoixFaits = @(if ($suite) { $suite -split ',' | ForEach-Object { [int]$_ } })
                if ($env:MINDCARE_TEST_REPARATEUR) { $script:ReparateurOuvert = $true }
                Show-Etape
                if ($env:MINDCARE_TEST_RESUME) { Save-Resume (Get-ResumeEnCours) | Out-Null }
                return
            }
        }
    }
    Show-Accueil
}

# ---------------------------------------------------------------------------
# Petits outils d'affichage
# ---------------------------------------------------------------------------
# Vide la fenêtre et revient en haut
function Clear-Contenu { $ui.Contenu.Children.Clear(); $ui.Defilement.ScrollToTop() }

function Add-Texte($Parent, [string]$Texte, [double]$Taille = 14, [string]$Couleur = '#EEEBF7', [string]$Graisse = 'Normal', [string]$Marge = '0,0,0,0') {
    $t = New-Text $Texte $Taille $Couleur $Graisse; $t.Margin = $Marge
    [void]$Parent.Children.Add($t)
}
function Add-Titre($Parent, [string]$Texte, [string]$Style = 'H1', [string]$Marge = '0,0,0,0') {
    $t = New-Object System.Windows.Controls.TextBlock
    $t.Text = $Texte; $t.Style = $Window.FindResource($Style); $t.TextWrapping = 'Wrap'; $t.Margin = $Marge
    [void]$Parent.Children.Add($t)
}
# Une rangée de boutons, et un bouton dedans
function New-Rangee($Parent) { $r = New-Object System.Windows.Controls.WrapPanel; $r.Margin = '0,18,0,0'; [void]$Parent.Children.Add($r); $r }
function Add-Bouton($Rangee, [string]$Texte, [scriptblock]$Clic, [string]$Style = 'BtnSecondary') {
    $b = New-Button $Texte $Style; $b.Margin = '0,0,10,8'; $b.Add_Click((Protect-Action $Clic)); [void]$Rangee.Children.Add($b)
}
# Un grand bouton à choisir (une réponse, une panne, une famille), le contenu à gauche ; Tag dit lequel
function Add-ChoixBouton($Parent, $Contenu, $Tag, [scriptblock]$Clic) {
    $b = New-Button '' 'BtnChoix'; $b.Content = $Contenu; $b.Tag = $Tag; $b.Add_Click((Protect-Action $Clic))
    [void]$Parent.Children.Add($b)
    $b
}
# Une liste, numérotée (les gestes) ou à puces
function Add-Liste($Parent, $Lignes, [string]$Puce = '', [string]$CouleurPuce = '#00E5FF', [string]$CouleurTexte = '#EEEBF7') {
    $n = 0
    foreach ($l in $Lignes) {
        $n++
        $d = New-Object System.Windows.Controls.DockPanel; $d.Margin = '0,8,0,0'
        $num = New-Text $(if ($Puce) { $Puce } else { "$n." }) 14 $CouleurPuce 'SemiBold'; $num.Width = 26
        [System.Windows.Controls.DockPanel]::SetDock($num, 'Left')
        [void]$d.Children.Add($num)
        [void]$d.Children.Add((New-Text ([string]$l) 14 $CouleurTexte))
        [void]$Parent.Children.Add($d)
    }
}
# Une carte en verre dans une colonne d'une grille (New-Card la pose dans la fenêtre) : renvoie son contenu
function New-CarteDans($Grille, [int]$Colonne) {
    $c = New-Object System.Windows.Controls.Border
    $c.Style = $Window.FindResource('Card'); $c.VerticalAlignment = 'Top'
    if ($Colonne -gt 0) { $c.Margin = '0,0,0,18' }
    $sp = New-Object System.Windows.Controls.StackPanel
    $c.Child = $sp
    [System.Windows.Controls.Grid]::SetColumn($c, $Colonne)
    [void]$Grille.Children.Add($c)
    $sp
}
# Une grille de colonnes égales (avec un espace entre elles)
function New-Grille([int]$Colonnes, [double]$Espace = 14) {
    $g = New-Object System.Windows.Controls.Grid
    for ($k = 0; $k -lt $Colonnes; $k++) {
        if ($k) { $e = New-Object System.Windows.Controls.ColumnDefinition; $e.Width = New-Object System.Windows.GridLength $Espace; [void]$g.ColumnDefinitions.Add($e) }
        [void]$g.ColumnDefinitions.Add((New-Object System.Windows.Controls.ColumnDefinition))
    }
    $g
}
# Range des éléments dans une grille de colonnes égales, ligne par ligne
function Add-EnGrille($Parent, $Elements, [int]$Colonnes) {
    $g = New-Grille $Colonnes
    $lignes = [math]::Ceiling(@($Elements).Count / $Colonnes)
    for ($k = 0; $k -lt $lignes; $k++) { [void]$g.RowDefinitions.Add((New-Object System.Windows.Controls.RowDefinition)) }
    $i = 0
    foreach ($el in $Elements) {
        [System.Windows.Controls.Grid]::SetColumn($el, ($i % $Colonnes) * 2); [System.Windows.Controls.Grid]::SetRow($el, [math]::Floor($i / $Colonnes))
        $el.Margin = '0,0,0,12'; $el.VerticalAlignment = 'Stretch'
        [void]$g.Children.Add($el); $i++
    }
    [void]$Parent.Children.Add($g)
}
# Un point de couleur et un texte (les suspects, ce que Windows a noté)
function New-Point([string]$Couleur, [double]$Taille = 9) {
    $p = New-Object System.Windows.Shapes.Ellipse; $p.Width = $Taille; $p.Height = $Taille; $p.Margin = '0,6,10,0'; $p.VerticalAlignment = 'Top'
    $p.Fill = Get-Brush $Couleur
    [System.Windows.Controls.DockPanel]::SetDock($p, 'Left')
    $p
}
function Add-Ligne($Parent, [string]$CouleurPoint, $Texte, [string]$Marge = '0,8,0,0') {
    $d = New-Object System.Windows.Controls.DockPanel; $d.Margin = $Marge
    [void]$d.Children.Add((New-Point $CouleurPoint))
    if ($Texte -is [string]) { $Texte = New-Text $Texte 14 '#EEEBF7' }
    [void]$d.Children.Add($Texte)
    [void]$Parent.Children.Add($d)
}

# ---------------------------------------------------------------------------
# 1. L'accueil : « Qu'est-ce qui ne va pas ? », la recherche, les familles
# ---------------------------------------------------------------------------
function Show-Accueil([string]$Famille = '') {
    $script:FamilleChoisie = $Famille
    $script:PanneEnCours = $null; $script:ChoixFaits = @(); $script:ReparateurOuvert = $false; $script:IndicesGardes = $null
    $script:FamillesAnimees = $false
    Stop-DicteeSansSuite; $script:DicteeMessage = $null
    Clear-Contenu
    Set-BulleMascotte $(if ($Famille) { 'Choisis ce qui ressemble le plus à ce qui se passe.' } else { 'Un souci avec l''ordinateur ? Dis-moi ce qui se passe.' })
    if (-not $script:Donnees) {
        $c = New-Card
        Add-Titre $c "Mindcare n'a pas pu lire ses enquêtes"
        Add-Texte $c 'Son fichier est absent ou abîmé. Réinstalle Mindcare depuis Mindset.' 14 '#A9A3C9' 'Normal' '0,8,0,0'
        return
    }
    # En haut, la recherche par mots (pour qui préfère écrire) : les résultats remplacent les questions pendant qu'on tape
    $champ = New-Object System.Windows.Controls.Border
    $champ.CornerRadius = 14; $champ.Padding = '14,7'; $champ.Margin = '0,0,0,18'
    $champ.Background = Get-Brush '#12FFFFFF'; $champ.BorderBrush = Get-Brush '#7300E5FF'; $champ.BorderThickness = 1
    $dp = New-Object System.Windows.Controls.DockPanel
    $loupe = New-Object System.Windows.Shapes.Path
    $loupe.Data = [System.Windows.Media.Geometry]::Parse('M 8,8 m -6,0 a 6,6 0 1 0 12,0 a 6,6 0 1 0 -12,0 M 12.5,12.5 L 17,17')
    $loupe.Stroke = Get-Brush '#00E5FF'; $loupe.StrokeThickness = 2; $loupe.StrokeEndLineCap = 'Round'
    $loupe.Width = 18; $loupe.Height = 18; $loupe.Margin = '0,0,10,0'; $loupe.VerticalAlignment = 'Center'
    [System.Windows.Controls.DockPanel]::SetDock($loupe, 'Left'); [void]$dp.Children.Add($loupe)
    $ui.BoutonMicro = New-BoutonMicro   # (parler au lieu d'écrire : dictee.ps1)
    [System.Windows.Controls.DockPanel]::SetDock($ui.BoutonMicro, 'Right'); [void]$dp.Children.Add($ui.BoutonMicro)
    $zone = New-Object System.Windows.Controls.Grid
    $indice = New-Text $(if (Test-IaInstallee) { 'Raconte ton problème avec tes mots, puis appuie sur Entrée : « mon pc s''éteint quand je joue »...' } else { 'Tu peux aussi écrire ton problème avec tes mots : « bip », « le son grésille », « écran bleu »...' }) 14 '#7E78A0'
    $indice.IsHitTestVisible = $false; $indice.VerticalAlignment = 'Center'
    $saisie = New-Object System.Windows.Controls.TextBox
    $saisie.Background = [System.Windows.Media.Brushes]::Transparent; $saisie.BorderThickness = 0; $saisie.FontSize = 15
    $saisie.Foreground = Get-Brush '#FFFFFF'; $saisie.CaretBrush = Get-Brush '#00E5FF'; $saisie.VerticalContentAlignment = 'Center'
    $saisie.Tag = $indice
    $saisie.Add_TextChanged((Protect-Action { param($s) $s.Tag.Visibility = $(if ($s.Text) { 'Collapsed' } else { 'Visible' }); if ($s.Text) { Start-Ia }; Clear-ZoneIa; Update-Accueil }))   # (l'assistant se prépare dès qu'on écrit)
    $saisie.Add_KeyDown((Protect-Action { param($s, $ev) if ($ev.Key -eq 'Return') { if (-not $script:DicteeDl -or $script:DicteeDl.Fini) { $script:DicteeMessage = $null; Update-ZoneDictee }; Request-Ia } }))
    [void]$zone.Children.Add($indice); [void]$zone.Children.Add($saisie)
    [void]$dp.Children.Add($zone)
    $champ.Child = $dp
    [void]$ui.Contenu.Children.Add($champ)
    $ui.Recherche = $saisie; $ui.ChampRecherche = $champ

    # La dictée : ce qu'elle fait (j'écoute, j'écris...), ou le bandeau pour l'installer
    $ui.ZoneDictee = New-Object System.Windows.Controls.StackPanel
    [void]$ui.Contenu.Children.Add($ui.ZoneDictee)
    Update-ZoneDictee

    # Une enquête pas finie : la reprendre est proposé tout de suite
    $dernier = if ($script:Historique.Count) { $script:Historique[0] } else { $null }
    $info = if ($dernier -and -not $dernier.Fini) { Get-DiagnosticInfo $dernier } else { $null }
    if ($info -and $info.Panne) {
        $bandeau = New-Object System.Windows.Controls.Border
        $bandeau.CornerRadius = 14; $bandeau.Padding = '16,8'; $bandeau.Margin = '0,0,0,14'
        $bandeau.Background = Get-Brush '#1400E5FF'; $bandeau.BorderBrush = Get-Brush '#4000E5FF'; $bandeau.BorderThickness = 1
        $bd = New-Object System.Windows.Controls.DockPanel
        $script:AReprendre = [string]$dernier.Id
        $b = New-Button 'La reprendre' 'BtnPrimary'; $b.Add_Click((Protect-Action { Open-Diagnostic $script:AReprendre }))
        [System.Windows.Controls.DockPanel]::SetDock($b, 'Right'); [void]$bd.Children.Add($b)
        $t = New-Text "Ton enquête « $($info.Panne.Titre) » n'est pas finie (commencée le $(Format-Date ([string]$dernier.Debut)))." 14 '#C9C3DD'
        $t.VerticalAlignment = 'Center'; [void]$bd.Children.Add($t)
        $bandeau.Child = $bd
        [void]$ui.Contenu.Children.Add($bandeau)
        if (-not $Famille) { Set-BulleMascotte "Ton enquête « $($info.Panne.Titre) » t'attend. On la reprend ?" }
    }

    # L'assistant : sa proposition (après Entrée), ou le bandeau pour l'installer
    $ui.ZoneIa = New-Object System.Windows.Controls.StackPanel
    [void]$ui.Contenu.Children.Add($ui.ZoneIa)
    Update-BandeauIa

    $ui.ZoneAccueil = New-Object System.Windows.Controls.StackPanel
    [void]$ui.Contenu.Children.Add($ui.ZoneAccueil)
    Update-Accueil
}

# ---------------------------------------------------------------------------
# La dictée (dictee.ps1) : le bouton micro de la barre, et ses messages juste en dessous
# ---------------------------------------------------------------------------
function New-BoutonMicro {
    $b = New-Object System.Windows.Controls.Button
    $b.Template = [System.Windows.Markup.XamlReader]::Parse('<ControlTemplate xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="Button"><Border Background="{TemplateBinding Background}" CornerRadius="17"><ContentPresenter HorizontalAlignment="Center" VerticalAlignment="Center"/></Border></ControlTemplate>')
    $b.Width = 34; $b.Height = 34; $b.Margin = '10,0,0,0'; $b.Cursor = 'Hand'; $b.Focusable = $true
    $b.Background = [System.Windows.Media.Brushes]::Transparent
    $b.ToolTip = 'Parler au lieu d''écrire'
    $p = New-Object System.Windows.Shapes.Path
    $p.Data = [System.Windows.Media.Geometry]::Parse('M 9,1 A 3,3 0 0 1 12,4 L 12,9 A 3,3 0 0 1 6,9 L 6,4 A 3,3 0 0 1 9,1 Z M 3,9 A 6,6 0 0 0 15,9 M 9,15 L 9,18 M 6,18 L 12,18')
    $p.Stroke = Get-Brush '#00E5FF'; $p.StrokeThickness = 2; $p.StrokeStartLineCap = 'Round'; $p.StrokeEndLineCap = 'Round'
    $p.Width = 16; $p.Height = 19; $p.Stretch = 'Uniform'
    $b.Content = $p
    $b.Add_MouseEnter((Protect-Action { param($s) if (-not (Test-DicteeEcoute)) { $s.Background = Get-Brush '#2600E5FF' } }))
    $b.Add_MouseLeave((Protect-Action { param($s) if (-not (Test-DicteeEcoute)) { $s.Background = [System.Windows.Media.Brushes]::Transparent } }))
    $b.Add_Click((Protect-Action { Invoke-BoutonMicro }))
    $b
}

function Invoke-BoutonMicro {
    $r = Start-Dictee
    switch ($r) {
        'ecoute'    { Show-DicteeEcoute }
        'fin'       { }   # (deuxième appui : Stop-Dictee a déjà montré la suite)
        'occupee'   { }   # (Whisper écrit encore la phrase d'avant)
        'installer' { $script:DicteeProposee = $true; Write-Log 'Dictée : installation proposée'; Update-ZoneDictee }
        default     { Show-DicteeFin $r }
    }
}

# Le bouton micro allumé pendant l'écoute ; sa lumière suit la voix
function Set-BoutonMicroAllume([bool]$Allume) {
    if (-not $ui.BoutonMicro) { return }
    $ui.BoutonMicro.Background = if ($Allume) { Get-Brush '#6600E5FF' } else { [System.Windows.Media.Brushes]::Transparent }
    $ui.BoutonMicro.ToolTip = if ($Allume) { 'Arrêter d''écouter' } else { 'Parler au lieu d''écrire' }
}
function Update-DicteeNiveau([double]$Niveau) {
    if (-not $ui.BoutonMicro) { return }
    $a = [int](255 * [math]::Min(1.0, 0.4 + 1.5 * $Niveau))
    $ui.BoutonMicro.Background = Get-Brush ('#{0:X2}00E5FF' -f $a)
}

function Set-DicteeMessage([string]$Texte, [string]$Bulle) {
    $script:DicteeMessage = $Texte
    Update-ZoneDictee
    if ($Bulle) { Set-BulleMascotte $Bulle }
}
function Show-DicteeEcoute {
    Set-BoutonMicroAllume $true
    Set-DicteeMessage 'Je t''écoute. Dis ce qui ne va pas : j''écris ta phrase dès que tu te tais.' 'Je t''écoute...'
}
function Show-DicteeEcrit {
    Set-BoutonMicroAllume $false
    Set-DicteeMessage 'J''écris ce que tu as dit...' 'J''écris ce que tu as dit...'
}
function Show-DicteeFin([string]$Texte) {
    Set-BoutonMicroAllume $false
    Set-DicteeMessage $Texte 'Je n''ai pas pu écrire ta phrase cette fois.'
}
# La phrase dite va dans la barre : la recherche se fait comme si elle avait été tapée, et Entrée la donne à l'assistant
function Set-DicteeTexte([string]$Texte) {
    Set-BoutonMicroAllume $false
    if (-not $ui.Recherche -or -not $ui.Contenu.Children.Contains($ui.ChampRecherche)) { $script:DicteeMessage = $null; return }   # (on a quitté l'accueil entre-temps)
    $ui.Recherche.Text = $Texte
    $ui.Recherche.CaretIndex = $Texte.Length
    [void]$ui.Recherche.Focus()
    Set-DicteeMessage 'Voilà ce que j''ai compris. Corrige si besoin, puis appuie sur Entrée.' 'C''est bien ça ? Appuie sur Entrée.'
}

function Update-ZoneDictee {
    $z = $ui.ZoneDictee
    if (-not $z) { return }
    $z.Children.Clear()
    $enCours = $script:DicteeDl -and -not $script:DicteeDl.Fini
    $erreur = $script:DicteeDl -and $script:DicteeDl.Fini -and $script:DicteeDl.Erreur
    $proposer = $script:DicteeProposee -and -not (Test-DicteeInstallee)
    if (-not $enCours -and -not $erreur -and -not $proposer -and -not $script:DicteeMessage) { return }
    $bandeau = New-BandeauIa
    $dp = New-Object System.Windows.Controls.DockPanel
    if (($proposer -or $erreur) -and -not $enCours) {
        $b = New-Button $(if ($erreur) { 'Réessayer' } elseif (Test-Path -LiteralPath $DicteeExe) { 'Réparer' } else { 'Installer' }) 'BtnPrimary'
        $b.Add_Click((Protect-Action { Start-DicteeInstall; Start-SuiviDictee; Update-ZoneDictee }))
        [System.Windows.Controls.DockPanel]::SetDock($b, 'Right'); [void]$dp.Children.Add($b)
    }
    $texte = if ($enCours) {
        $pc = if ($script:DicteeDl.Total) { [math]::Floor(100 * $script:DicteeDl.Recu / $script:DicteeDl.Total) } else { 0 }
        "Installation de la dictée : $pc % ($($script:DicteeDl.Etape)). Tu peux continuer à te servir de Mindcare."
    } elseif ($erreur) {
        "La dictée n'a pas pu s'installer : $($script:DicteeDl.Erreur)."
    } elseif ($proposer) {
        "Pour parler au lieu d'écrire, installe la dictée : environ $(Get-DicteeTaille) Mo, une seule fois. Elle marche ensuite sans internet, et ta voix n'est jamais gardée."
    } else { $script:DicteeMessage }
    $t = New-Text $texte 14 '#C9C3DD'; $t.VerticalAlignment = 'Center'; $t.Margin = '0,0,12,0'; $t.TextWrapping = 'Wrap'
    [void]$dp.Children.Add($t)
    $bandeau.Child = $dp
    [void]$z.Children.Add($bandeau)
}

# Suit l'installation de la dictée
function Start-SuiviDictee {
    if ($script:DicteeSuiviDl) { return }
    $script:DicteeSuiviDl = New-Object System.Windows.Threading.DispatcherTimer
    $script:DicteeSuiviDl.Interval = [TimeSpan]::FromMilliseconds(500)
    $script:DicteeSuiviDl.Add_Tick((Protect-Action {
        param($s)
        if ($script:DicteeDl -and $script:DicteeDl.Fini) {
            $s.Stop(); $script:DicteeSuiviDl = $null
            if ($script:DicteeDl.Erreur) { Write-Log "Dictée non installée : $($script:DicteeDl.Erreur)" }
            else { Write-Log 'Dictée installée'; $script:DicteeProposee = $false; $script:DicteeMessage = 'La dictée est prête : appuie sur le micro, à droite de la barre, et dis ce qui ne va pas.' }
        }
        if ($ui.ZoneDictee -and $ui.ZoneDictee.IsLoaded) { Update-ZoneDictee }
    }))
    $script:DicteeSuiviDl.Start()
}

# ---------------------------------------------------------------------------
# L'assistant (ia.ps1) : le bandeau d'installation, et la carte « J'ai compris, c'est bien ça ? »
# ---------------------------------------------------------------------------
function New-BandeauIa {
    $bandeau = New-Object System.Windows.Controls.Border
    $bandeau.CornerRadius = 14; $bandeau.Padding = '16,10'; $bandeau.Margin = '0,0,0,14'
    $bandeau.Background = Get-Brush '#14B04BFF'; $bandeau.BorderBrush = Get-Brush '#40B04BFF'; $bandeau.BorderThickness = 1
    $bandeau
}

function Update-BandeauIa {
    $z = $ui.ZoneIa
    if (-not $z -or $script:IaCarte) { return }
    $z.Children.Clear()
    $enCours = $script:IaDl -and -not $script:IaDl.Fini
    if ((Test-IaAJour) -and -not $enCours) { return }
    $maj = Test-IaInstallee   # (un ancien assistant marche : il s'agit de le mettre à jour)
    if ($maj -and -not $script:IaMajNotee) { $script:IaMajNotee = $true; Write-Log 'Assistant : une nouvelle version est proposée' }
    $bandeau = New-BandeauIa
    $dp = New-Object System.Windows.Controls.DockPanel
    if (-not $enCours) {
        $b = New-Button $(if ($script:IaDl -and $script:IaDl.Erreur) { 'Réessayer' } elseif ($maj) { 'Mettre à jour' } elseif (Test-IaIncomplete) { 'Réparer' } else { 'Installer' }) 'BtnPrimary'
        $b.Add_Click((Protect-Action { Start-IaInstall; Start-SuiviIa; Update-BandeauIa }))
        [System.Windows.Controls.DockPanel]::SetDock($b, 'Right'); [void]$dp.Children.Add($b)
    }
    $texte = if ($enCours) {
        $pc = if ($script:IaDl.Total) { [math]::Floor(100 * $script:IaDl.Recu / $script:IaDl.Total) } else { 0 }
        "Installation de l'assistant : $pc % ($($script:IaDl.Etape)). Tu peux continuer à te servir de Mindcare."
    } elseif ($script:IaDl -and $script:IaDl.Erreur) {
        "L'assistant n'a pas pu s'installer : $($script:IaDl.Erreur)."
    } elseif (Test-IaIncomplete) {
        "L'installation de l'assistant n'est pas allée jusqu'au bout (Mindcare a peut-être été fermée pendant) : répare-la, environ $([math]::Ceiling(((Get-IaManquants | ForEach-Object { $_.Taille }) | Measure-Object -Sum).Sum / 1MB)) Mo, une seule fois."
    } elseif ($maj) {
        "Une nouvelle version de l'assistant comprend mieux tes phrases : mets-la à jour (environ 640 Mo, une seule fois). L'assistant actuel marche en attendant."
    } else {
        "Mindcare peut comprendre tes phrases (« mon pc s'éteint quand je joue ») : installe son assistant, environ 700 Mo, une seule fois. Il marche ensuite sans internet."
    }
    $t = New-Text $texte 14 '#C9C3DD'; $t.VerticalAlignment = 'Center'; $t.Margin = '0,0,12,0'
    [void]$dp.Children.Add($t)
    $bandeau.Child = $dp
    [void]$z.Children.Add($bandeau)
}

# Suit l'installation, puis lance l'assistant
function Start-SuiviIa {
    if ($script:IaSuivi) { return }
    $script:IaSuivi = New-Object System.Windows.Threading.DispatcherTimer
    $script:IaSuivi.Interval = [TimeSpan]::FromMilliseconds(500)
    $script:IaSuivi.Add_Tick((Protect-Action {
        param($s)
        if ($script:IaDl -and $script:IaDl.Fini) {
            $s.Stop(); $script:IaSuivi = $null
            if ($script:IaDl.Erreur) { Write-Log "Assistant non installé : $($script:IaDl.Erreur)" }
            else { Write-Log 'Assistant installé'; Stop-IaServeur; Remove-IaAnciens }   # (lancé à la prochaine question)
        }
        if ($ui.ZoneIa -and $ui.ZoneIa.IsLoaded) { Update-BandeauIa }
    }))
    $script:IaSuivi.Start()
}

# L'assistant n'a pas pu lire la phrase (trop long, arrêté, réponse illisible) : on le dit, au lieu d'effacer l'attente
# sans un mot (la vraie cause est dans le journal). La recherche par mots, juste en dessous, marche toujours.
function Show-EchecIa {
    if (-not $ui.ZoneIa) { return }
    $b = New-BandeauIa
    $t = New-Text "L'assistant n'a pas réussi à lire ta phrase cette fois. Regarde les pannes ci-dessous, ou appuie encore sur Entrée pour réessayer." 14 '#C9C3DD'
    $t.TextWrapping = 'Wrap'; $b.Child = $t
    $ui.ZoneIa.Children.Insert(0, $b)
    Set-BulleMascotte 'Je n''ai pas compris cette fois. Regarde les pannes en dessous.'
}

function Clear-ZoneIa {
    $script:IaCarte = $null
    if ($script:IaAttente) { $script:IaAttente.Stop(); $script:IaAttente = $null }
    Stop-ChargementTheme
    Update-BandeauIa
}

# Entrée dans la barre : l'assistant lit la phrase (en arrière-plan), puis sa proposition s'affiche
function Request-Ia {
    if ($script:IaAttente) { return }   # (l'assistant lit déjà une phrase : Entrée appuyée une deuxième fois, ignorée)
    $texte = ([string]$ui.Recherche.Text).Trim()
    if (-not $texte -or -not $ui.ZoneIa) { return }
    $tache = Start-IaQuestion $texte @($script:Donnees.Pannes | ForEach-Object { [string]$_.Id })
    if (-not $tache) {   # (pas d'assistant : la recherche par mots reste ; installé mais il ne démarre pas : on le dit)
        if (Test-IaInstallee) { Write-Log 'Assistant : pas de réponse : il ne démarre pas'; Show-EchecIa }
        return
    }
    $script:IaCarte = @{ Texte = $texte }
    $z = $ui.ZoneIa; $z.Children.Clear()
    $attente = New-BandeauIa
    $lit = New-Text 'Je lis ta phrase...' 14 '#C9C3DD'
    $charge = New-ChargementTheme 40
    if ($charge) {
        $lit.VerticalAlignment = 'Center'; $charge.Margin = '0,0,12,0'
        $sp = New-Object System.Windows.Controls.StackPanel; $sp.Orientation = 'Horizontal'
        [void]$sp.Children.Add($charge); [void]$sp.Children.Add($lit); $attente.Child = $sp
    } else { $attente.Child = $lit }
    Set-BulleMascotte 'Je regarde ce qui ressemble à ton problème...'
    [void]$z.Children.Add($attente)
    $script:IaTache = $tache
    $script:IaAttente = New-Object System.Windows.Threading.DispatcherTimer
    $script:IaAttente.Interval = [TimeSpan]::FromMilliseconds(120)
    $script:IaAttente.Add_Tick((Protect-Action {
        param($s)
        $r = Get-IaReponse $script:IaTache
        if ($null -eq $r) { return }
        $s.Stop(); if ($script:IaAttente -eq $s) { $script:IaAttente = $null }; Stop-ChargementTheme
        if ($r.Erreur -or -not (Get-Panne $script:Donnees $r.Panne)) {
            Write-Log "Assistant : $(if ($r.Erreur) { $r.Erreur } else { "panne inconnue ($($r.Panne))" })"
            $script:IaCarte = $null; Update-BandeauIa; Show-EchecIa; return
        }
        # (chaque idée garde seulement les réponses que la phrase dit vraiment : le garde-fou)
        # (ses autres idées : les premières de la recherche par mots, plus justes que les siennes quand il hésite)
        $sur = $r.Confiance -ge $IaSeuil
        $mots = if ($sur) { @() } else { @(Find-Pannes $script:Donnees $script:IaCarte.Texte | Select-Object -First 3 | ForEach-Object { [pscustomobject]@{ Panne = [string]$_.Id; Reponses = @{} } }) }
        $idees = @(@([pscustomobject]@{ Panne = $r.Panne; Reponses = $r.Reponses }) + $mots + $(if ($sur) { @() } else { @($r.Autres) }) | Where-Object { Get-Panne $script:Donnees $_.Panne } |
                   Group-Object Panne | ForEach-Object { $_.Group[0] } | Select-Object -First 3)
        foreach ($i in $idees) {
            $propose = @($i.Reponses.Keys) -join ','
            $i.Reponses = Select-PreRempli $script:Donnees (Get-Panne $script:Donnees $i.Panne) $i.Reponses $script:IaCarte.Texte
            Write-Log "Assistant : ($($script:IaCarte.Texte.Length) caractères) -> $($i.Panne) $(@($i.Reponses.Keys) -join ',') (proposé : $propose)$(if ($i -eq $idees[0]) { " (sûr à $([math]::Round(100 * $r.Confiance)) %)" })"
        }
        # Sûr : l'enquête commence tout de suite. Il hésite : la question « Pour être sûr », sur ce qu'on voit
        if ($sur -or $idees.Count -lt 2) { Start-Diagnostic $idees[0].Panne $idees[0].Reponses }
        else { Show-PourEtreSur $idees }
    }))
    $script:IaAttente.Start()
}

# L'assistant hésite : une étape « Pour être sûr » avant l'enquête, comme ses autres questions. Les réponses décrivent ce
# qu'on voit (le titre et la phrase de chaque panne) ; « Aucune de ces phrases » ramène aux questions guidées de l'accueil.
function Show-PourEtreSur($Idees) {
    $script:IaIdees = @($Idees)
    $phrase = $script:IaCarte.Texte
    Clear-Contenu
    $g = New-Object System.Windows.Controls.Grid
    foreach ($l in 280, 22, 0) {
        $cd = New-Object System.Windows.Controls.ColumnDefinition
        $cd.Width = if ($l) { New-Object System.Windows.GridLength $l } else { New-Object System.Windows.GridLength 1, ([System.Windows.GridUnitType]::Star) }
        [void]$g.ColumnDefinitions.Add($cd)
    }
    [void]$ui.Contenu.Children.Add($g)
    $gauche = New-CarteDans $g 0
    Add-Titre $gauche 'Ce que tu as écrit' 'H2'
    Add-Texte $gauche "« $phrase »" 14 '#C9C3DD' 'Normal' '0,8,0,0'
    Add-Texte $gauche 'Plusieurs pannes y ressemblent : une question pour choisir la bonne enquête.' 13 '#7E78A0' 'Normal' '0,12,0,0'
    $droite = New-CarteDans $g 2
    Set-BulleMascotte 'Je veux être sûr de bien comprendre.'
    Add-Texte $droite 'POUR ÊTRE SÛR' 13 '#00E5FF' 'Bold' '0,0,0,8'
    Add-Titre $droite 'Laquelle de ces phrases ressemble le plus à ce qui se passe ?' 'H1'
    $reponses = New-Object System.Windows.Controls.StackPanel; $reponses.Margin = '0,6,0,0'
    [void]$droite.Children.Add($reponses)
    $i = 0
    foreach ($idee in $script:IaIdees) {
        $p = Get-Panne $script:Donnees $idee.Panne
        $sp = New-Object System.Windows.Controls.StackPanel
        [void]$sp.Children.Add((New-Text ([string]$p.Titre) 15 '#F2EFFA' 'SemiBold'))
        $ph = New-Text ([string]$p.Phrase) 13 '#A9A3C9'; $ph.Margin = '0,3,0,0'; [void]$sp.Children.Add($ph)
        [void](Add-ChoixBouton $reponses $sp $i { param($s) Select-PourEtreSur ([int]$s.Tag) })
        $i++
    }
    [void](Add-ChoixBouton $reponses (New-Text 'Aucune de ces phrases' 15 '#F2EFFA') -1 { param($s) Select-PourEtreSur -1 })
    $rg = New-Rangee $droite
    Add-Bouton $rg 'Accueil' { $script:IaCarte = $null; Show-Accueil }
    Write-Log "Pour être sûr : $(@($script:IaIdees | ForEach-Object { $_.Panne }) -join ', ')"
    if ($EnTest -and $env:MINDCARE_TEST_IA_CHOISIR) { Select-PourEtreSur ([int]$env:MINDCARE_TEST_IA_CHOISIR) }
}

function Select-PourEtreSur([int]$N) {
    if ($N -lt 0 -or $N -ge @($script:IaIdees).Count) { Write-Log 'Pour être sûr : aucune de ces phrases'; $script:IaCarte = $null; Show-Accueil; return }
    $idee = @($script:IaIdees)[$N]
    Write-Log "Pour être sûr : $($idee.Panne)"
    Start-Diagnostic $idee.Panne $idee.Reponses
}

# Sous la recherche : les familles, ou les pannes trouvées par les mots tapés
function Update-Accueil {
    $z = $ui.ZoneAccueil
    if (-not $z) { return }
    $z.Children.Clear()
    $texte = [string]$ui.Recherche.Text
    if ($texte.Trim()) {
        $trouvees = @(Find-Pannes $script:Donnees $texte)
        if (-not $trouvees) {
            Add-Texte $z "Rien ne correspond à « $($texte.Trim()) ». Essaie d'autres mots, ou efface pour répondre aux questions." 15 '#A9A3C9' 'Normal' '4,4,0,0'
            return
        }
        Add-Texte $z $(if ($trouvees.Count -eq 1) { 'Une panne correspond :' } else { "$($trouvees.Count) pannes correspondent :" }) 14 '#A9A3C9' 'Normal' '4,0,0,4'
        Add-EnGrille $z @($trouvees | Select-Object -First 8 | ForEach-Object { New-PanneBouton $_ $true }) 2
        return
    }
    # Question 2 : la famille choisie, et ses pannes comme réponses
    $fc = if ($script:FamilleChoisie) { Get-Famille $script:Donnees $script:FamilleChoisie } else { $null }
    if ($fc) {
        $choisi = New-Object System.Windows.Controls.DockPanel; $choisi.Margin = '0,0,0,16'
        $b = New-Button 'Changer' 'BtnSecondary'; $b.VerticalAlignment = 'Center'
        $b.Add_Click((Protect-Action { $script:FamilleChoisie = ''; Update-Accueil; $ui.Defilement.ScrollToTop() }))
        [System.Windows.Controls.DockPanel]::SetDock($b, 'Right'); [void]$choisi.Children.Add($b)
        $ic = ConvertFrom-Svg ([string]$fc.Icone)
        if ($ic) { $ic.Width = 26; $ic.Height = 26; $ic.Margin = '0,0,10,0'; [System.Windows.Controls.DockPanel]::SetDock($ic, 'Left'); [void]$choisi.Children.Add($ic) }
        $t = New-Text "1. Ça concerne : $($fc.Titre)" 15 '#A9A3C9' 'SemiBold'; $t.VerticalAlignment = 'Center'; [void]$choisi.Children.Add($t)
        [void]$z.Children.Add($choisi)
        Add-Titre $z "2. Qu'est-ce qui se passe ?" 'H1' '0,0,0,12'
        foreach ($p in @($script:Donnees.Pannes | Where-Object { $_.Famille -eq $fc.Id })) {
            $b = New-PanneBouton $p; $b.Margin = '0,0,0,10'; $b.HorizontalContentAlignment = 'Stretch'
            [void]$z.Children.Add($b)
        }
        Add-Texte $z 'Rien ne ressemble ? Écris ton problème tout en haut, ou change de famille. Ton cas pourra être ajouté.' 14 '#A9A3C9' 'Normal' '4,6,0,0'
        return
    }

    # Question 1 : ça concerne quoi ?
    Add-Titre $z '1. Ça concerne quoi ?' 'H1' '0,0,0,12'
    $tuiles = foreach ($f in $script:Donnees.Familles) {
        $n = @($script:Donnees.Pannes | Where-Object { $_.Famille -eq $f.Id }).Count
        $sp = New-Object System.Windows.Controls.StackPanel
        $ic = ConvertFrom-Svg ([string]$f.Icone)
        if ($ic) { $ic.Width = 34; $ic.Height = 34; $ic.HorizontalAlignment = 'Left'; $ic.Margin = '0,0,0,8'; [void]$sp.Children.Add($ic) }
        [void]$sp.Children.Add((New-Text ([string]$f.Titre) 15 '#FFFFFF' 'SemiBold'))
        $ph = New-Text $(if ($n) { [string]$f.Phrase } else { 'Arrive bientôt' }) 12 '#A9A3C9'; $ph.Margin = '0,3,0,0'
        [void]$sp.Children.Add($ph)
        $b = New-Button '' 'BtnChoix'; $b.Content = $sp; $b.Tag = [string]$f.Id; $b.Padding = '16,14'
        $b.HorizontalContentAlignment = 'Left'; $b.VerticalContentAlignment = 'Top'
        if ($n) { $b.Add_Click((Protect-Action { param($s) Show-Famille ([string]$s.Tag) })) } else { $b.IsEnabled = $false; $b.Opacity = 0.45 }
        $b
    }
    Add-EnGrille $z @($tuiles) 4
    if (-not $script:FamillesAnimees -and -not $EnTest) {
        $script:FamillesAnimees = $true
        $i = 0
        foreach ($b in @($tuiles)) {
            # (invisible jusqu'à son tour, puis fondu jusqu'à son opacité normale ; ensuite l'animation s'efface)
            $t0 = 60 + 35 * $i++
            $a = New-Object System.Windows.Media.Animation.DoubleAnimationUsingKeyFrames; $a.FillBehavior = 'Stop'
            foreach ($k in @(@(0, 0), @($t0, 0), @(($t0 + 380), $b.Opacity))) { [void]$a.KeyFrames.Add((New-Object System.Windows.Media.Animation.LinearDoubleKeyFrame ([double]$k[1]), ([System.Windows.Media.Animation.KeyTime]::FromTimeSpan([TimeSpan]::FromMilliseconds($k[0]))))) }
            $b.BeginAnimation([System.Windows.UIElement]::OpacityProperty, $a)
        }
    }
    # (pas de liste des enquêtes passées : rejouer les mêmes réponses redonne la même réponse ; seule l'enquête en
    # cours se reprend, par le bandeau du haut)
}

# Un bouton de panne : son nom et sa phrase (et sa famille, dans les résultats d'une recherche)
function New-PanneBouton($Panne, [bool]$AvecFamille = $false) {
    $sp = New-Object System.Windows.Controls.StackPanel
    if ($AvecFamille) { $f = Get-Famille $script:Donnees ([string]$Panne.Famille); Add-Texte $sp ([string]$f.Titre).ToUpper() 11 '#00E5FF' 'Bold' '0,0,0,3' }
    [void]$sp.Children.Add((New-Text ([string]$Panne.Titre) 16 '#FFFFFF' 'SemiBold'))
    $ph = New-Text ([string]$Panne.Phrase) 13 '#A9A3C9'; $ph.Margin = '0,3,0,0'
    [void]$sp.Children.Add($ph)
    $b = New-Button '' 'BtnChoix'; $b.Content = $sp; $b.Tag = [string]$Panne.Id; $b.Margin = '0'
    $b.Add_Click((Protect-Action { param($s) Start-Diagnostic ([string]$s.Tag) }))
    $b
}

# ---------------------------------------------------------------------------
# 2. Une famille : ses pannes (la question 2 de l'accueil)
# ---------------------------------------------------------------------------
function Show-Famille([string]$Id) {
    if (-not (Get-Famille $script:Donnees $Id)) { Show-Accueil; return }
    Show-Accueil $Id
}

# ---------------------------------------------------------------------------
# 3. L'enquête
# ---------------------------------------------------------------------------
function New-DiagId { "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$((Get-Random -Maximum 65536).ToString('x4'))" }

function Start-Diagnostic([string]$Id, [hashtable]$PreRempli = $null) {
    $script:PanneEnCours = Get-Panne $script:Donnees $Id
    if (-not $script:PanneEnCours) { Show-Accueil; return }
    $script:ChoixFaits = @(); $script:DiagId = New-DiagId; $script:ReparateurOuvert = $false; $script:IndicesGardes = $null; $script:Reprise = $null
    $script:PreRempli = if ($PreRempli) { $PreRempli.Clone() } else { $null }   # (les réponses déjà données par la phrase, assistant)
    if ($script:PreRempli) { $script:PreRempli.Remove('danger') }   # (la question de sécurité est toujours posée)
    $script:IaCarte = $null
    Write-Log "Enquête : $Id$(if ($PreRempli -and $PreRempli.Count) { " (déjà su : $(@($PreRempli.Keys) -join ', '))" })"
    Show-Etape
}

# Une enquête gardée : là où on en était (son étape, ou sa réponse)
function Open-Diagnostic([string]$Id) {
    $d = @($script:Historique | Where-Object { $_.Id -eq $Id }) | Select-Object -First 1
    $p = if ($d) { Get-Panne $script:Donnees ([string]$d.Panne) } else { $null }
    if (-not $p) { Write-Log "Enquête $Id introuvable"; Show-Accueil; return }
    $script:PanneEnCours = $p; $script:DiagId = $Id; $script:ReparateurOuvert = $false
    # (finie : revue avec ce que Windows avait noté ce jour-là, pas celui d'aujourd'hui : sa réponse ne change pas toute seule)
    $script:IndicesGardes = if ($d.Fini -and $d.PSObject.Properties['Indices'] -and $d.Indices) { ConvertTo-Indices $d.Indices } else { $null }
    $script:Reprise = $null
    if ($d.PSObject.Properties['Reponses'] -and @($d.Reponses).Count) {
        # Chaque réponse ne va qu'à la question qui porte son Id (Show-Etape) : une question qui n'est plus là est sautée,
        # une nouvelle est posée, et les suivantes reprennent leur réponse
        $script:ChoixFaits = @(); $script:PreRempli = @{}
        foreach ($r in $d.Reponses) { $script:PreRempli[[string]$r.Etape] = [int]$r.Reponse }
        # (le catalogue a pu changer depuis : ce qui n'a pas pu être rejoué n'est jamais effacé, voir Show-Etape)
        $script:Reprise = @{ Id = $Id; Reponses = @($d.Reponses) }
    } else {
        $script:ChoixFaits = @(foreach ($n in $d.Choix) { [int]$n }); $script:PreRempli = $null   # (ancien format : les numéros seulement)
    }
    Write-Log "Reprise de l'enquête $Id ($($p.Id))"
    Show-Etape
}

# Rejoue l'enquête ; ouverte sur le PC en panne (réponse « cepc »), avec ce que Windows a noté
function Get-EnqueteEnCours {
    $e = Get-Enquete $script:Donnees $script:PanneEnCours $script:ChoixFaits
    if ($e.Notes -contains 'cepc') { $e = Get-Enquete $script:Donnees $script:PanneEnCours $script:ChoixFaits $(if ($script:IndicesGardes) { $script:IndicesGardes } else { Get-Indices }) }
    $e
}

# Sur le PC en panne, l'enquête attend ce que Windows a noté (lu en arrière-plan, journaux.ps1) avant d'avancer : la
# fenêtre ne se fige pas, et les suspects ne changent pas en cours de route quand la lecture finit. Vrai si elle attend :
# un écran « Mindcare regarde... », puis Show-Etape de nouveau une fois la lecture finie (ou abandonnée au bout de
# $AttenteJournauxMax secondes). Si la personne est partie ailleurs entre-temps, rien n'est réaffiché.
$AttenteJournauxMax = 30
function Wait-Journaux($E) {
    if ($E.Notes -notcontains 'cepc' -or $script:IndicesGardes -or (Test-JournauxLus)) { return $false }
    Clear-Contenu
    $carte = New-Card
    Add-Titre $carte 'Mindcare regarde ce que Windows a noté...' 'H1'
    Add-Texte $carte 'Quelques secondes : ce que Windows a noté sur ce PC permet d''écarter des pistes sans rien te demander.' 14 '#A9A3C9' 'Normal' '0,8,0,0'
    $rg = New-Rangee $carte
    Add-Bouton $rg 'Étape précédente' { Undo-Choix }
    Add-Bouton $rg 'Accueil' { Show-Accueil }
    Set-BulleMascotte 'Je regarde ce que Windows a noté, ça ne prend qu''un instant.'
    if ($script:AttenteJournaux) { $script:AttenteJournaux.Ecran = $carte; return $true }
    $t = New-Object System.Windows.Threading.DispatcherTimer; $t.Interval = [TimeSpan]::FromMilliseconds(200)
    $script:AttenteJournaux = @{ Ecran = $carte; Debut = Get-Date }
    $t.Add_Tick((Protect-Action {
        param($s)
        $a = $script:AttenteJournaux
        if (-not (Test-JournauxLus) -and ((Get-Date) - $a.Debut).TotalSeconds -lt $AttenteJournauxMax) { return }
        $s.Stop(); $script:AttenteJournaux = $null
        if (-not (Test-JournauxLus)) { Stop-AttenteJournaux }
        Write-Log "Journaux de Windows : attendus $([math]::Round(((Get-Date) - $a.Debut).TotalSeconds, 1)) s"
        # (toujours sur l'écran d'attente : l'enquête reprend)
        if ($a.Ecran.Parent -and $ui.Contenu.Children.Contains($a.Ecran.Parent)) { Show-Etape }
    }))
    $t.Start()
    $true
}

function Show-Etape {
    $e = Get-EnqueteEnCours
    if (Wait-Journaux $e) { return }
    # (une question à laquelle la phrase a déjà répondu : sa réponse est donnée toute seule, une seule fois)
    while ($e.Etape -and $script:PreRempli -and $script:PreRempli.ContainsKey([string]$e.Etape.Id)) {
        $n = [int]$script:PreRempli[[string]$e.Etape.Id]
        $script:PreRempli.Remove([string]$e.Etape.Id)
        if ($n -lt -1 -or $n -ge @($e.Etape.Reponses).Count) { break }   # (-1 : « Je ne sais pas », gardé dans une enquête reprise)
        $script:ChoixFaits = @($e.Choix) + $n
        $e = Get-EnqueteEnCours
        if (Wait-Journaux $e) { return }
    }
    $script:ChoixFaits = $e.Choix
    if ($e.Etape) {
        # (une enquête déjà finie le reste dans l'historique tant qu'une nouvelle réponse ne la remplace pas ; une enquête
        # reprise sans aucune réponse rejouée reste telle quelle, Save-Diagnostic -Reprise)
        $ancien = @($script:Historique | Where-Object { $_.Id -eq $script:DiagId }) | Select-Object -First 1
        if (-not ($ancien -and $ancien.Fini)) { Save-Diagnostic $script:DiagId $script:PanneEnCours.Id $script:ChoixFaits $false $null '' (@(Get-Paires $e) + @(Get-PairesEnAttente $e)) -Reprise:(Test-Reprise) }
        Show-EtapeEnquete $e
    } else { Show-Verdict $e }
}

# Une enquête reprise de l'historique (Open-Diagnostic), et pas une nouvelle
function Test-Reprise { [bool]($script:Reprise -and $script:Reprise.Id -eq $script:DiagId) }

# Les réponses d'une enquête reprise pas encore rejouées (leur question n'est pas encore revenue) et toujours valables
# (la question existe encore dans le catalogue, et cette réponse aussi) : gardées avec elle, pour être rejouées quand
# leur question reviendra. Celles d'une question disparue sont laissées.
function Get-PairesEnAttente($E) {
    if (-not (Test-Reprise) -or -not $script:PreRempli -or -not $script:PreRempli.Count) { return @() }
    $nb = @{}; foreach ($x in @(Get-Etapes $script:Donnees $script:PanneEnCours)) { if ($x.Type -ne 'auto') { $nb[[string]$x.Id] = @($x.Reponses).Count } }
    $faites = @($E.Faits | ForEach-Object { [string]$_.Etape.Id })
    @(foreach ($r in $script:Reprise.Reponses) {
        $id = [string]$r.Etape; $n = [int]$r.Reponse
        if ($script:PreRempli.ContainsKey($id) -and $nb.ContainsKey($id) -and $faites -notcontains $id -and $n -ge -1 -and $n -lt $nb[$id]) { [ordered]@{ Etape = $id; Reponse = $n } }
    })
}
function Add-Choix([int]$N) { $script:ChoixFaits = @($script:ChoixFaits) + $N; Show-Etape }
function Undo-Choix {
    if (-not $script:ChoixFaits.Count) { Show-Famille ([string]$script:PanneEnCours.Famille); return }
    $script:ChoixFaits = @($script:ChoixFaits | Select-Object -First ($script:ChoixFaits.Count - 1))
    Show-Etape
}

# À gauche de l'enquête : les suspects (écartés au fur et à mesure), ce que Windows a noté, les réponses déjà données
function Add-PanneauEnquete($Parent, $E) {
    Add-Texte $Parent ([string]$script:PanneEnCours.Titre).ToUpper() 12 '#7E78A0' 'Bold' '0,0,0,12'
    Add-Texte $Parent 'Les suspects' 14 '#FFFFFF' 'SemiBold'
    Add-AnneauSuspects $Parent $E
    foreach ($id in $script:PanneEnCours.Suspects) {
        $s = Get-Suspect $script:Donnees $id
        if ($s.Urgent) { continue }   # (le danger se vérifie, ce n'est pas un suspect à montrer)
        $reste = $E.Restants -contains $id
        $t = New-Text ([string]$s.Titre) 14 $(if ($reste) { '#FFFFFF' } else { '#6E6890' })
        if (-not $reste) { $t.TextDecorations = [System.Windows.TextDecorations]::Strikethrough }
        Add-Ligne $Parent $(if ($reste) { '#00E5FF' } else { '#3A3350' }) $t '0,7,0,0'
    }
    if ($E.Auto) {
        Add-Texte $Parent 'Ce que Mindcare a vérifié' 14 '#FFFFFF' 'SemiBold' '0,18,0,0'
        foreach ($a in $E.Auto) { Add-Ligne $Parent $(if ($a.Vu) { '#FFB547' } else { '#3DDC97' }) (New-Text $a.Texte 13 '#C9C3DD') '0,6,0,0' }
    }
    if ($E.Faits) {
        Add-Texte $Parent 'Tes réponses' 14 '#FFFFFF' 'SemiBold' '0,18,0,2'
        $i = 0
        foreach ($f in $E.Faits) {
            $d = New-Object System.Windows.Controls.StackPanel; $d.Margin = '0,6,0,0'; $d.Background = [System.Windows.Media.Brushes]::Transparent
            [void]$d.Children.Add((New-Text $f.Court 12 '#A9A3C9'))
            $r = New-Text $f.Reponse 13 '#EEEBF7'; $r.MaxHeight = 36; $r.TextTrimming = 'CharacterEllipsis'; [void]$d.Children.Add($r)
            $d.Cursor = 'Hand'; $d.Tag = $i; $d.ToolTip = 'Changer cette réponse'
            $d.Add_MouseLeftButtonUp((Protect-Action { param($s) $script:ChoixFaits = @($script:ChoixFaits | Select-Object -First ([int]$s.Tag)); Show-Etape }))
            [void]$Parent.Children.Add($d)
            $i++
        }
    }
}

# L'anneau des suspects : combien sont déjà écartés (il se remplit à chaque réponse)
function Add-AnneauSuspects($Parent, $E) {
    $tous = @($script:PanneEnCours.Suspects | Where-Object { -not (Get-Suspect $script:Donnees $_).Urgent })
    if (-not $tous.Count) { return }
    $ecartes = @($tous | Where-Object { $E.Restants -notcontains $_ }).Count
    $reste = $tous.Count - $ecartes
    $g = New-Object System.Windows.Controls.Grid; $g.Margin = '0,10,0,4'
    foreach ($w in 'Auto', '*') { $cd = New-Object System.Windows.Controls.ColumnDefinition; $cd.Width = $(if ($w -eq 'Auto') { [System.Windows.GridLength]::Auto } else { New-Object System.Windows.GridLength 1, ([System.Windows.GridUnitType]::Star) }); [void]$g.ColumnDefinitions.Add($cd) }
    $rond = New-Object System.Windows.Controls.Grid; $rond.Width = 58; $rond.Height = 58
    $fond = New-Object System.Windows.Shapes.Ellipse; $fond.Stroke = Get-Brush '#16FFFFFF'; $fond.StrokeThickness = 6; [void]$rond.Children.Add($fond)
    $arc = New-Object System.Windows.Shapes.Ellipse; $arc.Stroke = Get-Brush '#00E5FF'; $arc.StrokeThickness = 6; $arc.StrokeDashCap = 'Round'
    $arc.RenderTransformOrigin = [System.Windows.Point]::new(0.5, 0.5); $arc.RenderTransform = New-Object System.Windows.Media.RotateTransform -90
    $L = [math]::PI * 52 / 6   # (longueur du cercle, en épaisseurs de trait)
    $arc.StrokeDashArray = New-Object System.Windows.Media.DoubleCollection (,[double[]]@($L, $L))
    $cible = $L * (1 - $ecartes / $tous.Count)
    $avant = if ($script:AnneauSuspects -and $script:AnneauSuspects.Panne -eq $script:PanneEnCours.Id) { $script:AnneauSuspects.Valeur } else { $L }
    $script:AnneauSuspects = @{ Panne = $script:PanneEnCours.Id; Valeur = $cible }
    if ($EnTest -or $avant -eq $cible) { $arc.StrokeDashOffset = $cible }
    else {
        $arc.StrokeDashOffset = $avant
        $a = New-Object System.Windows.Media.Animation.DoubleAnimation $avant, $cible, ([System.Windows.Duration]::new([TimeSpan]::FromMilliseconds(900)))
        $a.EasingFunction = New-Object System.Windows.Media.Animation.CubicEase; $a.BeginTime = [TimeSpan]::FromMilliseconds(150)
        $arc.BeginAnimation([System.Windows.Shapes.Shape]::StrokeDashOffsetProperty, $a)
    }
    [void]$rond.Children.Add($arc)
    $n = New-Text "$ecartes/$($tous.Count)" 15 '#FFFFFF' 'Bold'; $n.HorizontalAlignment = 'Center'; $n.VerticalAlignment = 'Center'; [void]$rond.Children.Add($n)
    [void]$g.Children.Add($rond)
    $tx = New-Object System.Windows.Controls.StackPanel; $tx.VerticalAlignment = 'Center'; $tx.Margin = '12,0,0,0'
    [void]$tx.Children.Add((New-Text $(if ($ecartes -eq 1) { '1 écarté' } else { "$ecartes écartés" }) 14 '#FFFFFF' 'SemiBold'))
    [void]$tx.Children.Add((New-Text $(if ($reste -eq 1) { 'Encore 1 à vérifier' } elseif ($reste) { "Encore $reste à vérifier" } else { 'Tous vérifiés' }) 13 '#A9A3C9'))
    [System.Windows.Controls.Grid]::SetColumn($tx, 1); [void]$g.Children.Add($tx)
    [void]$Parent.Children.Add($g)
}

# « Étape 2 sur 5 » et sa barre (le total peut baisser : des étapes deviennent inutiles en route)
function Add-Avancement($Parent, [int]$N, [int]$Total) {
    if ($Total -lt $N) { $Total = $N }
    Add-Texte $Parent "ÉTAPE $N SUR $Total" 12 '#A9A3C9' 'Bold'
    $barre = New-Object System.Windows.Controls.Grid; $barre.Height = 6; $barre.Margin = '0,6,0,16'
    $fond = New-Object System.Windows.Controls.Border; $fond.CornerRadius = 3; $fond.Background = Get-Brush '#22FFFFFF'
    [System.Windows.Controls.Grid]::SetColumnSpan($fond, 2); [void]$barre.Children.Add($fond)
    foreach ($l in $N, ($Total - $N)) {
        $cd = New-Object System.Windows.Controls.ColumnDefinition; $cd.Width = New-Object System.Windows.GridLength ([double]$l), ([System.Windows.GridUnitType]::Star)
        [void]$barre.ColumnDefinitions.Add($cd)
    }
    $plein = New-Object System.Windows.Controls.Border; $plein.CornerRadius = 3; $plein.Background = Get-Brush '#00E5FF'
    [void]$barre.Children.Add($plein)
    [void]$Parent.Children.Add($barre)
}

# Les gestes d'une manipulation, chacun avec sa case à cocher (un clic sur la ligne coche ou décoche), et la commande à taper
function Add-Gestes($Parent, $Gestes, $Commande = $null) {
    $n = 0; $posee = $false
    foreach ($l in $Gestes) {
        $n++
        $d = New-Object System.Windows.Controls.DockPanel; $d.Margin = '0,8,0,0'; $d.Cursor = 'Hand'; $d.Background = [System.Windows.Media.Brushes]::Transparent
        $case = New-Object System.Windows.Controls.Border
        $case.Width = 20; $case.Height = 20; $case.CornerRadius = 6; $case.BorderThickness = 2; $case.Margin = '0,0,12,0'; $case.VerticalAlignment = 'Top'
        $case.BorderBrush = Get-Brush '#00E5FF'; $case.Background = [System.Windows.Media.Brushes]::Transparent
        $coche = New-Object System.Windows.Shapes.Path
        $coche.Data = [System.Windows.Media.Geometry]::Parse('M 3,8 L 7,12 L 13,4'); $coche.Stroke = Get-Brush '#0B0A14'; $coche.StrokeThickness = 2.4
        $coche.StrokeEndLineCap = 'Round'; $coche.StrokeStartLineCap = 'Round'; $coche.Visibility = 'Hidden'; $coche.Margin = '0.5,0.5,0,0'
        $case.Child = $coche
        [System.Windows.Controls.DockPanel]::SetDock($case, 'Left'); [void]$d.Children.Add($case)
        $t = New-Text "$n. $l" 14 '#C9C3DD'; [void]$d.Children.Add($t)
        $d.Tag = @{ Case = $case; Coche = $coche; Texte = $t; Fait = $false }
        $d.Add_MouseLeftButtonUp((Protect-Action {
            param($s)
            $g = $s.Tag; $g.Fait = -not $g.Fait
            $g.Case.Background = if ($g.Fait) { Get-Brush '#00E5FF' } else { [System.Windows.Media.Brushes]::Transparent }
            $g.Coche.Visibility = if ($g.Fait) { 'Visible' } else { 'Hidden' }
            $g.Texte.Foreground = Get-Brush $(if ($g.Fait) { '#6E6890' } else { '#C9C3DD' })
        }))
        [void]$Parent.Children.Add($d)
        # (la commande juste sous le geste qui dit de la copier)
        if ($Commande -and -not $posee -and [string]$l -match 'Copier') { Add-Commandes $Parent $Commande; $posee = $true }
    }
    if ($Commande -and -not $posee) { Add-Commandes $Parent $Commande }
}

# Les commandes à taper dans le Terminal (Mindcare ne les lance jamais) : chacune avec son bouton « Copier »
function Add-Commandes($Parent, $Commande) {
    $bloc = New-Object System.Windows.Controls.Border
    $bloc.CornerRadius = 12; $bloc.Padding = '14,10'; $bloc.Margin = '0,12,0,0'
    $bloc.Background = Get-Brush '#33000000'; $bloc.BorderBrush = Get-Brush '#4000E5FF'; $bloc.BorderThickness = 1
    $sp = New-Object System.Windows.Controls.StackPanel
    $titre = if ($Commande.Admin) { 'À TAPER DANS LE TERMINAL (ADMINISTRATEUR)' } else { 'À TAPER DANS LE TERMINAL' }
    Add-Texte $sp $titre 11 '#00E5FF' 'Bold' '0,0,0,4'
    foreach ($l in @($Commande.Lignes)) {
        $d = New-Object System.Windows.Controls.DockPanel; $d.Margin = '0,6,0,0'
        $b = New-Button 'Copier' 'BtnSecondary'; $b.Tag = [string]$l; $b.Margin = '12,0,0,0'
        $b.Add_Click((Protect-Action {
            param($s)
            try { [System.Windows.Clipboard]::SetText([string]$s.Tag); $s.Content = 'Copiée' } catch { Write-Log "Commande non copiée : $_"; $s.Content = 'Copie impossible' }
        }))
        [System.Windows.Controls.DockPanel]::SetDock($b, 'Right'); [void]$d.Children.Add($b)
        $t = New-Object System.Windows.Controls.TextBox
        $t.Text = [string]$l; $t.IsReadOnly = $true; $t.BorderThickness = 0; $t.Background = [System.Windows.Media.Brushes]::Transparent
        $t.Foreground = Get-Brush '#FFFFFF'; $t.FontFamily = New-Object System.Windows.Media.FontFamily 'Cascadia Mono, Consolas'; $t.FontSize = 15
        $t.VerticalAlignment = 'Center'; $t.TextWrapping = 'Wrap'
        [void]$d.Children.Add($t)
        [void]$sp.Children.Add($d)
    }
    $bloc.Child = $sp
    [void]$Parent.Children.Add($bloc)
}

function Show-EtapeEnquete($E) {
    $et = $E.Etape
    Clear-Contenu
    $g = New-Object System.Windows.Controls.Grid
    foreach ($l in 280, 22, 0) {
        $cd = New-Object System.Windows.Controls.ColumnDefinition
        $cd.Width = if ($l) { New-Object System.Windows.GridLength $l } else { New-Object System.Windows.GridLength 1, ([System.Windows.GridUnitType]::Star) }
        [void]$g.ColumnDefinitions.Add($cd)
    }
    [void]$ui.Contenu.Children.Add($g)
    Add-PanneauEnquete (New-CarteDans $g 0) $E

    $droite = New-CarteDans $g 2
    $test = $et.Type -eq 'test'
    $suspects = @($E.Restants | Where-Object { -not (Get-Suspect $script:Donnees $_).Urgent }).Count
    Set-BulleMascotte $(if ($test) { 'Prends ton temps, je t''attends ici.' } elseif ($suspects -gt 1) { "On avance ! Il reste $suspects suspects." } else { 'Presque fini ! Une dernière vérification.' })
    Add-Avancement $droite (@($E.Faits).Count + 1) (@($E.Faits).Count + 1 + [int]$E.AVenir)
    $label = if ($test) { "UNE MANIPULATION À FAIRE$(if ($et.Duree) { " ($($et.Duree))" })" } elseif ($suspects -gt 1) { "Il reste $suspects suspects" } else { 'Une dernière vérification' }
    Add-Texte $droite $label 13 $(if ($test) { '#FF2EB5' } else { '#00E5FF' }) 'Bold' '0,0,0,8'
    $haut = New-Object System.Windows.Controls.DockPanel
    [void]$droite.Children.Add($haut)
    if ($et.Image) {
        $image = $null
        try { $image = ConvertFrom-Svg ([string]$et.Image) } catch { Write-Log "Image $($et.Image) illisible : $_" }
        if ($image) {
            $image.Width = 210; $image.Height = 140; $image.Margin = '20,0,0,0'; $image.VerticalAlignment = 'Top'
            [System.Windows.Controls.DockPanel]::SetDock($image, 'Right'); [void]$haut.Children.Add($image)
        }
    }
    $texte = New-Object System.Windows.Controls.StackPanel
    [void]$haut.Children.Add($texte)
    Add-Titre $texte ([string]$et.Texte) 'H1'
    if ($et.Aide) { Add-Texte $texte ([string]$et.Aide) 14 '#A9A3C9' 'Normal' '0,8,0,0' }
    if ($test) {
        Add-Texte $droite 'Coche chaque geste une fois fait :' 14 '#A9A3C9' 'Normal' '0,12,0,0'
        Add-Gestes $droite $et.Gestes $et.Commande
        # (la consigne de sécurité seulement pour les gestes sur le matériel, pas pour une manipulation dans Windows)
        if ((@($et.Gestes) -join ' ') -match 'débranch|câble|barrette|boîtier|capot|ventilat|multiprise|ouvr(e|ir) le PC|tournevis|batterie') {
            Add-Ligne $droite '#FFB547' (New-Text ([string]@($script:Donnees.Securite)[0]) 13 '#FFB547') '0,12,0,0'
        }
        Add-Texte $droite 'Ensuite, dis à Mindcare ce que ça a donné :' 14 '#A9A3C9' 'Normal' '0,16,0,0'
    }
    $reponses = New-Object System.Windows.Controls.StackPanel; $reponses.Margin = '0,6,0,0'
    [void]$droite.Children.Add($reponses)
    $i = 0
    foreach ($r in $et.Reponses) { [void](Add-ChoixBouton $reponses (New-Text ([string]$r.Texte) 15 '#F2EFFA') $i { param($s) Add-Choix ([int]$s.Tag) }); $i++ }
    $rg = New-Rangee $droite
    Add-Bouton $rg 'Étape précédente' { Undo-Choix }
    Add-Bouton $rg 'Je ne sais pas' { Add-Choix -1 }
    if ($test) { Add-Bouton $rg 'Je le ferai plus tard (Mindcare s''en souviendra)' { Write-Log 'Test remis à plus tard'; Show-Accueil } }
    else { Add-Bouton $rg 'Accueil' { Show-Accueil } }
}

# ---------------------------------------------------------------------------
# 4. La réponse : trouvée (ou tout le reste écarté), ou ce qui reste pour le réparateur
# ---------------------------------------------------------------------------
# (Reaffiche : la même réponse, montrée de nouveau avec ou sans « Pour le réparateur » : rien n'est noté ni enregistré)
function Show-Verdict($E, [switch]$Reaffiche) {
    $v = $E.Verdict
    $titre = Get-VerdictTitre $script:Donnees $script:PanneEnCours $v
    $suspect = Get-Suspect $script:Donnees $v.Suspect
    if (-not $Reaffiche) { Write-Log "Réponse $($script:PanneEnCours.Id) : $($v.Type)$(if ($v.Suspect) { " $($v.Suspect)" }) (réponses $($script:ChoixFaits -join ','))" }
    # Une pièce seulement restée par élimination n'a pas été testée : jamais présentée comme sûre (on ne fait pas acheter sur une supposition)
    $piece = $suspect -and $suspect.Piece
    $supposee = $piece -and $v.Type -eq 'elimination'
    if ($supposee) { $titre = "À vérifier : $(Get-Minuscule ([string]$suspect.Titre))" }
    Set-BulleMascotte $(if ($supposee) { 'Tout le reste est écarté, mais cette pièce n''a pas été testée : fais-la vérifier avant d''acheter.' } else { switch ($v.Type) { 'trouve' { 'Trouvé ! Regarde la réponse.' } 'elimination' { 'Tout le reste est écarté : voilà la piste.' } default { 'Je n''ai pas tout trouvé : un réparateur saura voir la suite.' } } })
    if (-not $Reaffiche) {
        $indices = if ($E.Notes -contains 'cepc') { if ($script:IndicesGardes) { $script:IndicesGardes } else { Get-Indices } } else { $null }
        Save-Diagnostic $script:DiagId $script:PanneEnCours.Id $script:ChoixFaits $true $v.Suspect $titre (Get-Paires $E) $indices -Reprise:(Test-Reprise)
    }
    $script:ResumeDe = @{ Panne = $script:PanneEnCours; Enquete = $E; Titre = $titre }
    Clear-Contenu

    $haut = New-Object System.Windows.Controls.DockPanel; $haut.Margin = '0,0,0,14'
    $boutons = New-Object System.Windows.Controls.StackPanel; $boutons.Orientation = 'Horizontal'; $boutons.VerticalAlignment = 'Center'
    [System.Windows.Controls.DockPanel]::SetDock($boutons, 'Right'); [void]$haut.Children.Add($boutons)
    Add-Bouton $boutons 'Changer ma dernière réponse' { Undo-Choix }
    Add-Bouton $boutons 'Pour le réparateur' { $script:ReparateurOuvert = -not $script:ReparateurOuvert; Show-Verdict (Get-EnqueteEnCours) -Reaffiche }
    Add-Bouton $boutons 'Retour à l''accueil' { Show-Accueil } 'BtnPrimary'
    $pn = New-Text ([string]$script:PanneEnCours.Titre) 14 '#A9A3C9'; $pn.VerticalAlignment = 'Center'
    [void]$haut.Children.Add($pn)
    [void]$ui.Contenu.Children.Add($haut)

    # La réponse, en grand
    $urgent = $suspect -and $suspect.Urgent
    $couleur, $signe, $label = switch ($v.Type) {
        'trouve'      { if ($urgent) { '#FF5A6E', '!', 'URGENT' } else { '#3DDC97', ([string][char]0x2713), 'TROUVÉ' } }
        'elimination' { if ($supposee) { '#FFB547', '?', 'LA PISTE LA PLUS PROBABLE' } else { '#3DDC97', ([string][char]0x2713), 'TOUT LE RESTE EST ÉCARTÉ' } }
        'reparateur'  { '#FFB547', '?', 'À FAIRE VÉRIFIER PAR UN RÉPARATEUR' }
        default       { '#FFB547', '?', 'AUCUNE PISTE NE CORRESPOND' }
    }
    $carte = New-Object System.Windows.Controls.Border
    $carte.Style = $Window.FindResource('Card'); $carte.BorderBrush = Get-Brush $couleur; $carte.BorderThickness = 2
    $cd = New-Object System.Windows.Controls.DockPanel
    $rond = New-Object System.Windows.Controls.Border
    $rond.Width = 60; $rond.Height = 60; $rond.CornerRadius = 30; $rond.Margin = '0,0,20,0'; $rond.BorderThickness = 3
    $rond.BorderBrush = Get-Brush $couleur; $rond.Background = Get-Brush ('#26' + $couleur.Substring(1))
    $s = New-Text $signe 28 $couleur 'Bold'; $s.HorizontalAlignment = 'Center'; $s.VerticalAlignment = 'Center'; $rond.Child = $s
    [System.Windows.Controls.DockPanel]::SetDock($rond, 'Left'); [void]$cd.Children.Add($rond)
    $tx = New-Object System.Windows.Controls.StackPanel; $tx.VerticalAlignment = 'Center'
    Add-Texte $tx $label 13 $couleur 'Bold'
    Add-Titre $tx $titre 'H1' '0,4,0,0'
    [void]$cd.Children.Add($tx)
    $carte.Child = $cd
    [void]$ui.Contenu.Children.Add($carte)

    if ($piece) {
        $av = New-Object System.Windows.Controls.Border
        $av.Style = $Window.FindResource('Card'); $av.Margin = '0,14,0,16'; $av.BorderBrush = Get-Brush '#FFB547'; $av.BorderThickness = 2
        $avs = New-Object System.Windows.Controls.StackPanel
        Add-Texte $avs 'AVANT D''ACHETER UNE PIÈCE' 13 '#FFB547' 'Bold'
        Add-Texte $avs $(if ($supposee) { 'Mindcare n''a pas pu tester cette pièce : elle reste parce que tout le reste est écarté. Avant d''en acheter une neuve, fais-la vérifier par un réparateur, ou essaie avec celle d''un proche.' }
                         else { 'Si le PC a moins de 2 ans, il est encore sous garantie : contacte d''abord le magasin ou le fabricant, la réparation peut être gratuite. Dans le doute, un réparateur peut confirmer avant que tu achètes.' }) 14 '#C9C3DD' 'Normal' '0,6,0,0'
        $av.Child = $avs
        [void]$ui.Contenu.Children.Add($av)
        if (-not $Reaffiche) { Write-Log "Avant d'acheter : pièce $(if ($supposee) { 'supposée (pas testée)' } else { 'confirmée' })" }
    }

    if ($script:ReparateurOuvert) { Add-Reparateur }

    $g = New-Object System.Windows.Controls.Grid
    foreach ($l in 1, 0, 1) {
        $c = New-Object System.Windows.Controls.ColumnDefinition
        $c.Width = if ($l) { New-Object System.Windows.GridLength 1, ([System.Windows.GridUnitType]::Star) } else { New-Object System.Windows.GridLength 18 }
        [void]$g.ColumnDefinitions.Add($c)
    }
    [void]$ui.Contenu.Children.Add($g)

    # Comment on le sait : les preuves, ce que Windows a noté, ce qui est écarté
    $gauche = New-CarteDans $g 0
    Add-Titre $gauche 'Comment on le sait' 'H2'
    foreach ($p in $E.Preuves) { Add-Ligne $gauche '#3DDC97' $p }
    foreach ($a in @($E.Auto | Where-Object { $E.Preuves -notcontains $_.Texte })) { Add-Ligne $gauche $(if ($a.Vu) { '#FFB547' } else { '#3DDC97' }) (New-Text $a.Texte 14 '#C9C3DD') }
    if (-not $E.Preuves -and -not $E.Auto) { Add-Texte $gauche 'Tes réponses ne permettaient pas d''écarter de piste.' 14 '#A9A3C9' 'Normal' '0,8,0,0' }
    $ecartes = @($E.Ecartes | Where-Object { -not (Get-Suspect $script:Donnees $_.Id).Urgent } | ForEach-Object { Get-Minuscule ([string](Get-Suspect $script:Donnees $_.Id).Titre) } | Select-Object -Unique)
    if ($ecartes) { Add-Texte $gauche "Écartés : $($ecartes -join ', ')." 14 '#7E78A0' 'Normal' '0,12,0,0' }

    # Ce qu'il faut faire
    $droite = New-CarteDans $g 2
    Add-Titre $droite 'Ce qu''il faut faire' 'H2'
    if ($suspect) {
        Add-Texte $droite ([string]$suspect.Pourquoi) 14 '#A9A3C9' 'Normal' '0,8,0,0'
        Add-Liste $droite $suspect.AFaire
        if ($v.Suspect -eq 'memoire' -and $E.Notes -contains 'cepc') {
            $barrettes = @(Get-Barrettes)
            if ($barrettes) { Add-Texte $droite "Mindcare a lu les barrettes de ce PC : $($barrettes -join ' ; ')." 13 '#C9C3DD' 'Normal' '26,10,0,0' }
        }
        Add-Texte $droite 'Quand aller chez un réparateur' 15 '#FFFFFF' 'SemiBold' '0,16,0,0'
        Add-Texte $droite ([string]$suspect.Pro) 14 '#C9C3DD' 'Normal' '0,6,0,0'
    } elseif ($v.Type -eq 'reparateur') {
        Add-Texte $droite 'Ces pistes se départagent avec un outil de réparateur. Montre-lui le rapport (bouton « Pour le réparateur ») : il saura par où commencer.' 14 '#A9A3C9' 'Normal' '0,8,0,0'
        foreach ($id in $v.Restants) { $r = Get-Suspect $script:Donnees $id; Add-Texte $droite ([string]$r.Titre) 15 '#FFFFFF' 'SemiBold' '0,14,0,0'; Add-Texte $droite ([string]$r.Pourquoi) 14 '#C9C3DD' 'Normal' '0,4,0,0' }
    } else {
        Add-Texte $droite 'Aucun suspect ne tient après tes réponses. La panne est peut-être ailleurs : essaie une autre panne qui ressemble, ou montre le rapport à un réparateur.' 14 '#A9A3C9' 'Normal' '0,8,0,0'
    }
    Add-Ligne $ui.Contenu '#FFB547' (New-Text (@($script:Donnees.Securite) -join ' ') 13 '#FFB547') '4,0,0,8'
}

# Pour le réparateur : le rapport (la panne, la réponse, les preuves, les réponses, le PC) en fichier ou à coller
function Add-Reparateur {
    $rep = New-Card
    Add-Titre $rep 'Pour le réparateur' 'H2'
    Add-Texte $rep 'Le rapport de cette enquête (la panne, la réponse, les preuves, tes réponses et le modèle du PC), à montrer à un réparateur ou à envoyer à quelqu''un qui peut t''aider.' 14 '#A9A3C9' 'Normal' '0,6,0,0'
    $rg = New-Rangee $rep
    Add-Bouton $rg 'Enregistrer le rapport sur le Bureau' {
        try { $f = Save-Resume (Get-ResumeEnCours); $ui.ResumeInfo.Visibility = 'Visible'; $ui.ResumeInfo.Text = "C'est fait : le fichier « $([IO.Path]::GetFileName($f)) » est sur le Bureau." }
        catch { Write-Log "Rapport non enregistré : $_"; $ui.ResumeInfo.Visibility = 'Visible'; $ui.ResumeInfo.Text = "Le rapport n'a pas pu être enregistré sur le Bureau. Essaie plutôt « Copier le rapport »." }
    }
    Add-Bouton $rg 'Copier le rapport' {
        try { [System.Windows.Clipboard]::SetText((Get-ResumeEnCours)); $ui.ResumeInfo.Visibility = 'Visible'; $ui.ResumeInfo.Text = 'Le rapport est copié : tu peux le coller dans un message ou un e-mail.' }
        catch { Write-Log "Rapport non copié : $_"; $ui.ResumeInfo.Visibility = 'Visible'; $ui.ResumeInfo.Text = "Le rapport n'a pas pu être copié. Essaie plutôt de l'enregistrer sur le Bureau." }
    }
    $ui.ResumeInfo = New-Text '' 14 '#3DDC97'; $ui.ResumeInfo.Visibility = 'Collapsed'
    [void]$rep.Children.Add($ui.ResumeInfo)
}

function Get-ResumeEnCours { Get-ResumeTexte $script:ResumeDe.Panne $script:ResumeDe.Enquete $script:ResumeDe.Titre }

# À la fermeture : arrêter ce qui tourne (minuteries, travaux de fond), sinon rien ne doit rester ouvert
function Stop-Contenu { Stop-LectureJournaux; Stop-ThemeVisuels; Stop-Ia; Stop-Dictees }
