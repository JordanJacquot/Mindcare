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
    if ($EnTest -and $env:MINDCARE_TEST_BAS) { $Window.Add_ContentRendered({ $ui.Defilement.ScrollToEnd() }) }   # (tests : capture du bas)
    if ($EnTest -and $env:MINDCARE_TEST_GALERIE) {
        $g = New-Object System.Windows.Controls.WrapPanel
        foreach ($f in Get-ChildItem -LiteralPath $ImagesDir -Filter '*.svg') { $v = ConvertFrom-Svg $f.BaseName; $v.Width = 150; $v.Height = 100; $v.Margin = '0,0,12,12'; [void]$g.Children.Add($v) }
        [void]$ui.Contenu.Children.Add($g)
        return
    }
    if ($EnTest -and $script:Donnees) {
        if ($env:MINDCARE_TEST_REPRENDRE -and $script:Historique.Count) { Open-Diagnostic ([string]$script:Historique[0].Id); return }
        if ($env:MINDCARE_TEST_FAMILLE) { Show-Famille $env:MINDCARE_TEST_FAMILLE; return }
        if ($env:MINDCARE_TEST_RECHERCHE) { Show-Accueil; $ui.Recherche.Text = $env:MINDCARE_TEST_RECHERCHE; return }
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
    $b = New-Button $Texte $Style; $b.Margin = '0,0,10,8'; $b.Add_Click($Clic); [void]$Rangee.Children.Add($b)
}
# Un grand bouton à choisir (une réponse, une panne, une famille), le contenu à gauche ; Tag dit lequel
function Add-ChoixBouton($Parent, $Contenu, $Tag, [scriptblock]$Clic) {
    $b = New-Button '' 'BtnChoix'; $b.Content = $Contenu; $b.Tag = $Tag; $b.Add_Click($Clic)
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
    $script:PanneEnCours = $null; $script:ChoixFaits = @(); $script:ReparateurOuvert = $false
    Clear-Contenu
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
    $zone = New-Object System.Windows.Controls.Grid
    $indice = New-Text 'Tu peux aussi écrire ton problème avec tes mots : « bip », « le son grésille », « écran bleu »...' 14 '#7E78A0'
    $indice.IsHitTestVisible = $false; $indice.VerticalAlignment = 'Center'
    $saisie = New-Object System.Windows.Controls.TextBox
    $saisie.Background = [System.Windows.Media.Brushes]::Transparent; $saisie.BorderThickness = 0; $saisie.FontSize = 15
    $saisie.Foreground = Get-Brush '#FFFFFF'; $saisie.CaretBrush = Get-Brush '#00E5FF'; $saisie.VerticalContentAlignment = 'Center'
    $saisie.Tag = $indice
    $saisie.Add_TextChanged({ param($s) $s.Tag.Visibility = $(if ($s.Text) { 'Collapsed' } else { 'Visible' }); Update-Accueil })
    [void]$zone.Children.Add($indice); [void]$zone.Children.Add($saisie)
    [void]$dp.Children.Add($zone)
    $champ.Child = $dp
    [void]$ui.Contenu.Children.Add($champ)
    $ui.Recherche = $saisie

    # Une enquête pas finie : la reprendre est proposé tout de suite
    $dernier = if ($script:Historique.Count) { $script:Historique[0] } else { $null }
    $info = if ($dernier -and -not $dernier.Fini) { Get-DiagnosticInfo $dernier } else { $null }
    if ($info -and $info.Panne) {
        $bandeau = New-Object System.Windows.Controls.Border
        $bandeau.CornerRadius = 14; $bandeau.Padding = '16,8'; $bandeau.Margin = '0,0,0,14'
        $bandeau.Background = Get-Brush '#1400E5FF'; $bandeau.BorderBrush = Get-Brush '#4000E5FF'; $bandeau.BorderThickness = 1
        $bd = New-Object System.Windows.Controls.DockPanel
        $script:AReprendre = [string]$dernier.Id
        $b = New-Button 'La reprendre' 'BtnPrimary'; $b.Add_Click({ Open-Diagnostic $script:AReprendre })
        [System.Windows.Controls.DockPanel]::SetDock($b, 'Right'); [void]$bd.Children.Add($b)
        $t = New-Text "Ton enquête « $($info.Panne.Titre) » n'est pas finie (commencée le $(Format-Date ([string]$dernier.Debut)))." 14 '#C9C3DD'
        $t.VerticalAlignment = 'Center'; [void]$bd.Children.Add($t)
        $bandeau.Child = $bd
        [void]$ui.Contenu.Children.Add($bandeau)
    }

    $ui.ZoneAccueil = New-Object System.Windows.Controls.StackPanel
    [void]$ui.Contenu.Children.Add($ui.ZoneAccueil)
    Update-Accueil
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
        $b.Add_Click({ $script:FamilleChoisie = ''; Update-Accueil; $ui.Defilement.ScrollToTop() })
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
        if ($n) { $b.Add_Click({ param($s) Show-Famille ([string]$s.Tag) }) } else { $b.IsEnabled = $false; $b.Opacity = 0.45 }
        $b
    }
    Add-EnGrille $z @($tuiles) 4

    # Les derniers diagnostics : un clic pour revoir la réponse, ou reprendre une enquête
    $gardes = @(foreach ($d in ($script:Historique | Select-Object -First 5)) { $i = Get-DiagnosticInfo $d; if ($i.Panne) { [pscustomobject]@{ D = $d; I = $i } } })
    if (-not $gardes) { return }
    Add-Titre $z 'Tes derniers diagnostics' 'H2' '4,10,0,4'
    foreach ($g in $gardes) {
        $sp = New-Object System.Windows.Controls.DockPanel
        $quand = New-Text "le $(Format-Date ([string]$g.D.Debut))" 12 '#7E78A0'; $quand.VerticalAlignment = 'Center'; $quand.Margin = '14,0,0,0'
        [System.Windows.Controls.DockPanel]::SetDock($quand, 'Right'); [void]$sp.Children.Add($quand)
        [void]$sp.Children.Add((New-Text "$($g.I.Panne.Titre) : $($g.I.Phrase)" 14 $(if ($g.D.Fini) { '#EEEBF7' } else { '#00E5FF' })))
        $b = Add-ChoixBouton $z $sp ([string]$g.D.Id) { param($s) Open-Diagnostic ([string]$s.Tag) }
        $b.Padding = '16,9'; $b.Margin = '0,6,0,0'; $b.HorizontalContentAlignment = 'Stretch'
    }
}

# Un bouton de panne : son nom et sa phrase (et sa famille, dans les résultats d'une recherche)
function New-PanneBouton($Panne, [bool]$AvecFamille = $false) {
    $sp = New-Object System.Windows.Controls.StackPanel
    if ($AvecFamille) { $f = Get-Famille $script:Donnees ([string]$Panne.Famille); Add-Texte $sp ([string]$f.Titre).ToUpper() 11 '#00E5FF' 'Bold' '0,0,0,3' }
    [void]$sp.Children.Add((New-Text ([string]$Panne.Titre) 16 '#FFFFFF' 'SemiBold'))
    $ph = New-Text ([string]$Panne.Phrase) 13 '#A9A3C9'; $ph.Margin = '0,3,0,0'
    [void]$sp.Children.Add($ph)
    $b = New-Button '' 'BtnChoix'; $b.Content = $sp; $b.Tag = [string]$Panne.Id; $b.Margin = '0'
    $b.Add_Click({ param($s) Start-Diagnostic ([string]$s.Tag) })
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

function Start-Diagnostic([string]$Id) {
    $script:PanneEnCours = Get-Panne $script:Donnees $Id
    if (-not $script:PanneEnCours) { Show-Accueil; return }
    $script:ChoixFaits = @(); $script:DiagId = New-DiagId; $script:ReparateurOuvert = $false
    Write-Log "Enquête : $Id"
    Show-Etape
}

# Une enquête gardée : là où on en était (son étape, ou sa réponse)
function Open-Diagnostic([string]$Id) {
    $d = @($script:Historique | Where-Object { $_.Id -eq $Id }) | Select-Object -First 1
    $p = if ($d) { Get-Panne $script:Donnees ([string]$d.Panne) } else { $null }
    if (-not $p) { Write-Log "Enquête $Id introuvable"; Show-Accueil; return }
    $script:PanneEnCours = $p; $script:ChoixFaits = @(foreach ($n in $d.Choix) { [int]$n }); $script:DiagId = $Id; $script:ReparateurOuvert = $false
    Write-Log "Reprise de l'enquête $Id ($($p.Id))"
    Show-Etape
}

# Rejoue l'enquête ; ouverte sur le PC en panne (réponse « cepc »), avec ce que Windows a noté
function Get-EnqueteEnCours {
    $e = Get-Enquete $script:Donnees $script:PanneEnCours $script:ChoixFaits
    if ($e.Notes -contains 'cepc') { $e = Get-Enquete $script:Donnees $script:PanneEnCours $script:ChoixFaits (Get-Indices) }
    $e
}

function Show-Etape {
    $e = Get-EnqueteEnCours
    $script:ChoixFaits = $e.Choix
    if ($e.Etape) {
        Save-Diagnostic $script:DiagId $script:PanneEnCours.Id $script:ChoixFaits $false $null ''
        Show-EtapeEnquete $e
    } else { Show-Verdict $e }
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
            $d.Add_MouseLeftButtonUp({ param($s) $script:ChoixFaits = @($script:ChoixFaits | Select-Object -First ([int]$s.Tag)); Show-Etape })
            [void]$Parent.Children.Add($d)
            $i++
        }
    }
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

# Les gestes d'une manipulation, chacun avec sa case à cocher (un clic sur la ligne coche ou décoche)
function Add-Gestes($Parent, $Gestes) {
    $n = 0
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
        $d.Add_MouseLeftButtonUp({
            param($s)
            $g = $s.Tag; $g.Fait = -not $g.Fait
            $g.Case.Background = if ($g.Fait) { Get-Brush '#00E5FF' } else { [System.Windows.Media.Brushes]::Transparent }
            $g.Coche.Visibility = if ($g.Fait) { 'Visible' } else { 'Hidden' }
            $g.Texte.Foreground = Get-Brush $(if ($g.Fait) { '#6E6890' } else { '#C9C3DD' })
        })
        [void]$Parent.Children.Add($d)
    }
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
        Add-Gestes $droite $et.Gestes
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
    Add-Bouton $rg 'Retour' { Undo-Choix }
    Add-Bouton $rg 'Je ne sais pas' { Add-Choix -1 }
    if ($test) { Add-Bouton $rg 'Je le ferai plus tard (Mindcare s''en souviendra)' { Write-Log 'Test remis à plus tard'; Show-Accueil } }
}

# ---------------------------------------------------------------------------
# 4. La réponse : trouvée (ou tout le reste écarté), ou ce qui reste pour le réparateur
# ---------------------------------------------------------------------------
function Show-Verdict($E) {
    $v = $E.Verdict
    $titre = Get-VerdictTitre $script:Donnees $script:PanneEnCours $v
    $suspect = Get-Suspect $script:Donnees $v.Suspect
    Write-Log "Réponse $($script:PanneEnCours.Id) : $($v.Type) $($v.Suspect) (réponses $($script:ChoixFaits -join ','))"
    Save-Diagnostic $script:DiagId $script:PanneEnCours.Id $script:ChoixFaits $true $v.Suspect $titre
    $script:ResumeDe = @{ Panne = $script:PanneEnCours; Enquete = $E; Titre = $titre }
    Clear-Contenu

    $haut = New-Object System.Windows.Controls.DockPanel; $haut.Margin = '0,0,0,14'
    $boutons = New-Object System.Windows.Controls.StackPanel; $boutons.Orientation = 'Horizontal'; $boutons.VerticalAlignment = 'Center'
    [System.Windows.Controls.DockPanel]::SetDock($boutons, 'Right'); [void]$haut.Children.Add($boutons)
    Add-Bouton $boutons 'Retour' { Undo-Choix }
    Add-Bouton $boutons 'Pour le réparateur' { $script:ReparateurOuvert = -not $script:ReparateurOuvert; Show-Verdict (Get-EnqueteEnCours) }
    Add-Bouton $boutons 'Nouveau diagnostic' { Show-Accueil } 'BtnPrimary'
    $pn = New-Text ([string]$script:PanneEnCours.Titre) 14 '#A9A3C9'; $pn.VerticalAlignment = 'Center'
    [void]$haut.Children.Add($pn)
    [void]$ui.Contenu.Children.Add($haut)

    # La réponse, en grand
    $urgent = $suspect -and $suspect.Urgent
    $couleur, $signe, $label = switch ($v.Type) {
        'trouve'      { if ($urgent) { '#FF5A6E', '!', 'URGENT' } else { '#3DDC97', ([string][char]0x2713), 'TROUVÉ' } }
        'elimination' { '#3DDC97', ([string][char]0x2713), 'TOUT LE RESTE EST ÉCARTÉ' }
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
function Stop-Contenu { Stop-LectureJournaux; Stop-ThemeVisuels }
