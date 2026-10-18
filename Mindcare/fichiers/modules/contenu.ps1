# Mindcare : CE QUE FAIT L'APPLI. Les trois écrans du diagnostic : le choix de la panne (et les derniers diagnostics),
# les questions une à une (avec « Je ne sais pas » et « Retour »), puis la conclusion. Les pannes et le calcul sont dans
# diagnostic.ps1, les diagnostics gardés et la carte de Mindset dans historique.ps1.
# Chargé par Mindcare.ps1 après historique.ps1 ; Initialize-Contenu est appelé par evenements.ps1.

$script:Pannes       = $null   # le contenu de questions.json
$script:PanneEnCours = $null   # la panne choisie
$script:ChoixFaits   = @()     # les choix faits jusqu'ici (voir diagnostic.ps1)
$script:DiagId       = $null   # le diagnostic en cours, tel qu'il est gardé (historique.ps1)

function Initialize-Contenu {
    try { $script:Pannes = Read-Pannes } catch { Write-Log "Questions illisibles ($QuestionsFichier) : $_" }
    Read-Historique
    if ($EnTest -and $env:MINDCARE_TEST_BAS) { $Window.Add_ContentRendered({ $ui.Defilement.ScrollToEnd() }) }   # (tests : capture du bas)
    # Tests : reprendre le dernier diagnostic gardé
    if ($EnTest -and $env:MINDCARE_TEST_REPRENDRE -and $script:Historique.Count -and $script:Pannes) { Open-Diagnostic ([string]$script:Historique[0].Id); return }
    # Tests : toutes les images sur une planche, pour les relire d'un coup d'œil
    if ($EnTest -and $env:MINDCARE_TEST_GALERIE) {
        $g = New-Object System.Windows.Controls.WrapPanel
        foreach ($f in Get-ChildItem -LiteralPath $ImagesDir -Filter '*.svg') {
            $v = ConvertFrom-Svg $f.BaseName; $v.Width = 180; $v.Height = 120; $v.Margin = '0,0,12,12'
            [void]$g.Children.Add($v)
        }
        [void]$ui.Contenu.Children.Add($g)
        return
    }
    # Tests : un parcours tout fait, « extinction » ou « extinction:0,1,-1 » (outils\tester.ps1)
    if ($EnTest -and $env:MINDCARE_TEST_PARCOURS -and $script:Pannes) {
        $id, $suite = $env:MINDCARE_TEST_PARCOURS -split ':', 2
        $p = Get-Panne $script:Pannes $id
        if ($p) {
            $script:PanneEnCours = $p; $script:DiagId = New-DiagId
            $script:ChoixFaits = @(if ($suite) { $suite -split ',' | ForEach-Object { [int]$_ } })
            Show-Etape
            if ($env:MINDCARE_TEST_RESUME) { Save-Resume (Get-ResumeEnCours) | Out-Null }
            return
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
# Une réponse (ou une panne) à choisir : un grand bouton, le texte à gauche ; Tag dit lequel
function Add-ChoixBouton($Parent, $Contenu, $Tag, [scriptblock]$Clic) {
    $b = New-Button '' 'BtnChoix'; $b.Content = $Contenu; $b.Tag = $Tag; $b.Add_Click($Clic)
    [void]$Parent.Children.Add($b)
}
# Une liste, numérotée (les gestes) ou à puces (la sécurité)
function Add-Liste($Parent, $Lignes, [string]$Puce = '', [string]$CouleurPuce = '#00E5FF') {
    $n = 0
    foreach ($l in $Lignes) {
        $n++
        $d = New-Object System.Windows.Controls.DockPanel; $d.Margin = '0,8,0,0'
        $num = New-Text $(if ($Puce) { $Puce } else { "$n." }) 14 $CouleurPuce 'SemiBold'; $num.Width = 26
        [System.Windows.Controls.DockPanel]::SetDock($num, 'Left')
        [void]$d.Children.Add($num)
        [void]$d.Children.Add((New-Text ([string]$l) 14 '#EEEBF7'))
        [void]$Parent.Children.Add($d)
    }
}

# ---------------------------------------------------------------------------
# 1. Le choix de la panne
# ---------------------------------------------------------------------------
function Show-Accueil {
    $script:PanneEnCours = $null; $script:ChoixFaits = @()
    Clear-Contenu
    # Le dernier diagnostic n'est pas fini : le reprendre est proposé tout en haut
    $dernier = if ($script:Historique.Count -and $script:Pannes) { $script:Historique[0] } else { $null }
    $info = if ($dernier -and -not $dernier.Fini) { Get-DiagnosticInfo $dernier } else { $null }
    if ($info -and $info.Panne) {
        $r = New-Card
        Add-Titre $r "Un diagnostic n'est pas fini" 'H2'
        Add-Texte $r "« $($info.Panne.Titre) », commencé le $(Format-Date ([string]$dernier.Debut)), après $(@($dernier.Choix).Count) réponse(s)." 14 '#A9A3C9' 'Normal' '0,6,0,0'
        $rg = New-Rangee $r
        $script:AReprendre = [string]$dernier.Id
        Add-Bouton $rg 'Le reprendre' { Open-Diagnostic $script:AReprendre } 'BtnPrimary'
    }
    $c = New-Card
    if (-not $script:Pannes) {
        Add-Titre $c "Mindcare n'a pas pu lire ses questions"
        Add-Texte $c 'Son fichier de questions est absent ou abîmé. Réinstalle Mindcare depuis Mindset.' 14 '#A9A3C9' 'Normal' '0,8,0,0'
        return
    }
    Add-Titre $c 'Quelle est la panne ?'
    Add-Texte $c 'Choisis ce qui ressemble le plus à ce qui arrive. Mindcare te pose ensuite quelques questions simples, une à une.' 14 '#A9A3C9' 'Normal' '0,6,0,8'
    foreach ($p in $script:Pannes.Pannes) {
        $sp = New-Object System.Windows.Controls.StackPanel
        [void]$sp.Children.Add((New-Text ([string]$p.Titre) 16 '#FFFFFF' 'SemiBold'))
        $ph = New-Text ([string]$p.Phrase) 13 '#A9A3C9'; $ph.Margin = '0,2,0,0'
        [void]$sp.Children.Add($ph)
        Add-ChoixBouton $c $sp ([string]$p.Id) { param($s) Start-Diagnostic ([string]$s.Tag) }
    }

    # Les derniers diagnostics : un clic pour reprendre celui qui n'est pas fini, ou revoir une conclusion
    $gardes = @(foreach ($d in $script:Historique) { $i = Get-DiagnosticInfo $d; if ($i.Panne) { [pscustomobject]@{ D = $d; I = $i } } })
    if (-not $gardes) { return }
    $h = New-Card
    Add-Titre $h 'Tes derniers diagnostics' 'H2'
    Add-Texte $h 'Clique sur un diagnostic pour revoir sa conclusion, ou pour le reprendre là où tu en étais.' 14 '#A9A3C9' 'Normal' '0,6,0,4'
    foreach ($g in $gardes) {
        $sp = New-Object System.Windows.Controls.StackPanel
        [void]$sp.Children.Add((New-Text ([string]$g.I.Panne.Titre) 15 '#FFFFFF' 'SemiBold'))
        $quoi = if ($g.D.Fini) { $g.I.Phrase } else { 'pas fini, clique pour le reprendre' }
        $ph = New-Text "Le $(Format-Date ([string]$g.D.Debut)) : $quoi" 13 $(if ($g.D.Fini) { '#A9A3C9' } else { '#00E5FF' }); $ph.Margin = '0,2,0,0'
        [void]$sp.Children.Add($ph)
        Add-ChoixBouton $h $sp ([string]$g.D.Id) { param($s) Open-Diagnostic ([string]$s.Tag) }
    }
}

function New-DiagId { "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$((Get-Random -Maximum 65536).ToString('x4'))" }

function Start-Diagnostic([string]$Id) {
    $script:PanneEnCours = Get-Panne $script:Pannes $Id
    $script:ChoixFaits = @()
    $script:DiagId = New-DiagId
    Write-Log "Diagnostic : $Id"
    Show-Etape
}

# Un diagnostic gardé : là où on en était (sa question, ou sa conclusion)
function Open-Diagnostic([string]$Id) {
    $d = @($script:Historique | Where-Object { $_.Id -eq $Id }) | Select-Object -First 1
    $p = if ($d) { Get-Panne $script:Pannes ([string]$d.Panne) } else { $null }
    if (-not $p) { Write-Log "Diagnostic $Id introuvable"; Show-Accueil; return }
    $script:PanneEnCours = $p
    $script:ChoixFaits = @(foreach ($n in $d.Choix) { [int]$n })
    $script:DiagId = $Id
    Write-Log "Reprise du diagnostic $Id ($($p.Id))"
    Show-Etape
}

# La question où on en est, ou la conclusion si le parcours est fini
function Show-Etape {
    $etat = Get-Parcours $script:PanneEnCours $script:ChoixFaits
    $script:ChoixFaits = $etat.Choix
    if ($etat.Question) {
        Save-Diagnostic $script:DiagId $script:PanneEnCours.Id $script:ChoixFaits $false @()
        Show-Question $etat
    } else { Show-Conclusion $etat }
}
function Add-Choix([int]$N) { $script:ChoixFaits = @($script:ChoixFaits) + $N; Show-Etape }
# Retour : la question d'avant (ou le choix de la panne, depuis la première)
function Undo-Choix {
    if (-not $script:ChoixFaits.Count) { Show-Accueil; return }
    $script:ChoixFaits = @($script:ChoixFaits | Select-Object -First ($script:ChoixFaits.Count - 1))
    Show-Etape
}

# ---------------------------------------------------------------------------
# 2. Une question
# ---------------------------------------------------------------------------
function Show-Question($Etat) {
    $q = $Etat.Question
    Clear-Contenu
    # Deux colonnes : les étapes à gauche, la question à droite
    $g = New-Object System.Windows.Controls.Grid
    foreach ($l in 270, 22, 0) {
        $cd = New-Object System.Windows.Controls.ColumnDefinition
        $cd.Width = if ($l) { New-Object System.Windows.GridLength $l } else { New-Object System.Windows.GridLength 1, ([System.Windows.GridUnitType]::Star) }
        [void]$g.ColumnDefinitions.Add($cd)
    }
    [void]$ui.Contenu.Children.Add($g)

    # À gauche : les étapes faites (un clic pour changer sa réponse), celle en cours, celles qui viennent sans doute
    $gauche = New-CarteDans $g 0
    Add-Texte $gauche ([string]$script:PanneEnCours.Titre).ToUpper() 12 '#6E6890' 'SemiBold' '0,0,0,16'
    $n = 0
    foreach ($e in $Etat.Resume) { Add-Etape $gauche ($n + 1) 'fait' $e.Court $e.Reponse $n; $n++ }
    Add-Etape $gauche ($n + 1) 'encours' ([string]$q.Court) '' -1
    $prochaines = @(Get-Prochaines $script:PanneEnCours $q)
    $k = $n + 2
    foreach ($p in $prochaines) { Add-Etape $gauche $k 'avenir' ([string]$p.Court) '' -1; $k++ }

    # À droite : la question, son dessin, les réponses
    $droite = New-CarteDans $g 2
    $haut = New-Object System.Windows.Controls.DockPanel
    [void]$droite.Children.Add($haut)
    $image = $null
    if ($q.Image) {
        try { $image = ConvertFrom-Svg ([string]$q.Image) } catch { Write-Log "Image $($q.Image) illisible : $_" }
        if (-not $image) { Write-Log "Image $($q.Image) introuvable" }
    }
    if ($image) {
        $image.Width = 210; $image.Height = 140; $image.Margin = '20,0,0,0'; $image.VerticalAlignment = 'Top'
        [System.Windows.Controls.DockPanel]::SetDock($image, 'Right')
        [void]$haut.Children.Add($image)
    }
    $texte = New-Object System.Windows.Controls.StackPanel
    [void]$haut.Children.Add($texte)
    $reste = if ($prochaines.Count) { "encore $($prochaines.Count) environ" } else { 'la dernière' }
    Add-Texte $texte "Question $($n + 1), $reste" 13 '#00E5FF' 'SemiBold' '0,0,0,8'
    Add-Titre $texte ([string]$q.Texte) 'H1'
    if ($q.Aide) { Add-Texte $texte ([string]$q.Aide) 14 '#A9A3C9' 'Normal' '0,8,0,0' }
    $reponses = New-Object System.Windows.Controls.StackPanel; $reponses.Margin = '0,10,0,0'
    [void]$droite.Children.Add($reponses)
    $i = 0
    foreach ($r in $q.Reponses) {
        Add-ChoixBouton $reponses (New-Text ([string]$r.Texte) 15 '#F2EFFA') $i { param($s) Add-Choix ([int]$s.Tag) }
        $i++
    }
    $rg = New-Rangee $droite
    Add-Bouton $rg 'Retour' { Undo-Choix }
    Add-Bouton $rg 'Je ne sais pas' { Add-Choix -1 }
}

# Une carte en verre dans une colonne d'une grille (New-Card la pose dans la fenêtre) : renvoie son contenu
function New-CarteDans($Grille, [int]$Colonne) {
    $c = New-Object System.Windows.Controls.Border
    $c.Style = $Window.FindResource('Card'); $c.VerticalAlignment = 'Top'
    $sp = New-Object System.Windows.Controls.StackPanel
    $c.Child = $sp
    [System.Windows.Controls.Grid]::SetColumn($c, $Colonne)
    [void]$Grille.Children.Add($c)
    $sp
}

# Une étape dans la liste de gauche : faite (coche verte, un clic pour revenir changer sa réponse), en cours, à venir
function Add-Etape($Parent, [int]$Numero, [string]$Statut, [string]$Titre, [string]$Detail, [int]$Index) {
    $d = New-Object System.Windows.Controls.DockPanel
    $d.Margin = '0,0,0,12'; $d.Background = [System.Windows.Media.Brushes]::Transparent
    $rond = New-Object System.Windows.Controls.Border
    $rond.Width = 24; $rond.Height = 24; $rond.CornerRadius = 12; $rond.Margin = '0,0,12,0'; $rond.VerticalAlignment = 'Top'
    $chiffre = New-Text $(if ($Statut -eq 'fait') { [string][char]0x2713 } else { "$Numero" }) 12 '#6E6890' 'Bold'
    $chiffre.HorizontalAlignment = 'Center'; $chiffre.VerticalAlignment = 'Center'
    if ($Statut -eq 'fait') { $rond.Background = Get-Brush '#2E3DDC97'; $chiffre.Foreground = Get-Brush '#3DDC97' }
    elseif ($Statut -eq 'encours') { $rond.Background = Get-Brush '#00E5FF'; $chiffre.Foreground = Get-Brush '#08060F' }
    else { $rond.BorderBrush = Get-Brush '#3A3350'; $rond.BorderThickness = 1 }
    $rond.Child = $chiffre
    [System.Windows.Controls.DockPanel]::SetDock($rond, 'Left')
    [void]$d.Children.Add($rond)
    $t = New-Object System.Windows.Controls.StackPanel; $t.VerticalAlignment = 'Center'
    $couleur = if ($Statut -eq 'fait') { '#EEEBF7' } elseif ($Statut -eq 'encours') { '#FFFFFF' } else { '#6E6890' }
    [void]$t.Children.Add((New-Text $Titre 13 $couleur $(if ($Statut -eq 'encours') { 'SemiBold' } else { 'Normal' })))
    if ($Detail) { $dt = New-Text $Detail 12 '#A9A3C9'; $dt.MaxHeight = 34; $dt.TextTrimming = 'CharacterEllipsis'; [void]$t.Children.Add($dt) }
    [void]$d.Children.Add($t)
    if ($Statut -eq 'fait') {
        $d.Cursor = 'Hand'; $d.Tag = $Index; $d.ToolTip = 'Changer cette réponse'
        $d.Add_MouseLeftButtonUp({ param($s) $script:ChoixFaits = @($script:ChoixFaits | Select-Object -First ([int]$s.Tag)); Show-Etape })
    }
    [void]$Parent.Children.Add($d)
}

# ---------------------------------------------------------------------------
# 3. La conclusion
# ---------------------------------------------------------------------------
function Show-Conclusion($Etat) {
    $causes = @(Get-Conclusion $script:PanneEnCours $Etat)
    $liste = if ($causes) { ($causes | ForEach-Object { "$($_.Cause.Id) ($($_.Points))" }) -join ', ' } else { 'rien de clair' }
    Write-Log "Conclusion $($script:PanneEnCours.Id) : $liste (choix $($script:ChoixFaits -join ','))"
    Save-Diagnostic $script:DiagId $script:PanneEnCours.Id $script:ChoixFaits $true @($causes | ForEach-Object { $_.Cause.Id })
    Clear-Contenu
    $c = New-Card
    Add-Texte $c "$($script:PanneEnCours.Titre) : la conclusion" 13 '#00E5FF' 'SemiBold' '0,0,0,8'
    if (-not $causes) {
        $rien = $script:Pannes.RienTrouve
        Add-Titre $c ([string]$rien.Titre)
        Add-Texte $c ([string]$rien.Pourquoi) 14 '#A9A3C9' 'Normal' '0,8,0,0'
        Add-Liste $c $rien.Gestes
    } else {
        $urgent = [bool]$causes[0].Cause.Urgent
        Add-Texte $c $(if ($urgent) { 'Urgent' } else { 'Le plus probable' }) 13 $(if ($urgent) { '#FF5A6E' } else { '#A9A3C9' }) 'SemiBold'
        Add-Cause $c $causes[0].Cause 'H1'
    }
    if ($script:PanneEnCours.Conseil) {
        $bon = New-Card
        Add-Texte $bon 'Bon à savoir' 18 '#FFFFFF' 'SemiBold'
        Add-Texte $bon ([string]$script:PanneEnCours.Conseil) 14 '#EEEBF7' 'Normal' '0,8,0,0'
    }
    foreach ($autre in ($causes | Select-Object -Skip 1)) {
        $c2 = New-Card
        Add-Texte $c2 'Autre piste' 13 '#A9A3C9' 'SemiBold'
        Add-Cause $c2 $autre.Cause 'H2'
    }

    $sec = New-Card
    Add-Texte $sec 'Pour ta sécurité' 18 '#FFB547' 'SemiBold'
    Add-Liste $sec $script:Pannes.Securite '!' '#FFB547'

    $rp = New-Card
    Add-Titre $rp 'Tes réponses' 'H2'
    foreach ($e in $Etat.Resume) {
        Add-Texte $rp $e.Question 13 '#A9A3C9' 'Normal' '0,10,0,0'
        Add-Texte $rp $e.Reponse 14 '#EEEBF7' 'SemiBold'
    }

    # Pour le réparateur : le résumé (la panne, la conclusion, les réponses, le modèle du PC) en fichier ou à coller
    $script:ResumeDe = @{ Panne = $script:PanneEnCours; Etat = $Etat; Causes = $causes }   # (le texte est fait au clic)
    $rep = New-Card
    Add-Titre $rep 'Pour le réparateur' 'H2'
    Add-Texte $rep 'Garde un résumé de ce diagnostic (la panne, la conclusion, tes réponses et le modèle du PC) pour le montrer à un réparateur, ou pour l''envoyer à quelqu''un qui peut t''aider.' 14 '#A9A3C9' 'Normal' '0,6,0,0'
    $rg = New-Rangee $rep
    Add-Bouton $rg 'Enregistrer le résumé sur le Bureau' {
        try { $f = Save-Resume (Get-ResumeEnCours); $ui.ResumeInfo.Text = "C'est fait : le fichier « $([IO.Path]::GetFileName($f)) » est sur le Bureau." }
        catch { Write-Log "Résumé non enregistré : $_"; $ui.ResumeInfo.Text = "Le résumé n'a pas pu être enregistré sur le Bureau. Essaie plutôt « Copier le résumé »." }
    }
    Add-Bouton $rg 'Copier le résumé' {
        try { [System.Windows.Clipboard]::SetText((Get-ResumeEnCours)); $ui.ResumeInfo.Text = 'Le résumé est copié : tu peux le coller dans un message ou un e-mail.' }
        catch { Write-Log "Résumé non copié : $_"; $ui.ResumeInfo.Text = "Le résumé n'a pas pu être copié. Essaie plutôt de l'enregistrer sur le Bureau." }
    }
    $ui.ResumeInfo = New-Text '' 14 '#3DDC97'; $ui.ResumeInfo.Margin = '0,4,0,0'
    [void]$rep.Children.Add($ui.ResumeInfo)

    $rg = New-Rangee $rep
    Add-Bouton $rg 'Nouveau diagnostic' { Show-Accueil } 'BtnPrimary'
    Add-Bouton $rg 'Changer ma dernière réponse' { Undo-Choix }
}

function Get-ResumeEnCours { Get-ResumeTexte $script:ResumeDe.Panne $script:ResumeDe.Etat $script:ResumeDe.Causes }

# Une cause : son titre, pourquoi, ce qu'on peut faire soi-même, quand aller chez un réparateur, et parfois un bouton
# vers le diagnostic d'une autre panne (VoirPanne)
function Add-Cause($Parent, $Cause, [string]$Style) {
    Add-Titre $Parent ([string]$Cause.Titre) $Style '0,2,0,0'
    Add-Texte $Parent ([string]$Cause.Pourquoi) 14 '#A9A3C9' 'Normal' '0,8,0,0'
    Add-Texte $Parent 'Ce que tu peux faire' 15 '#FFFFFF' 'SemiBold' '0,18,0,0'
    Add-Liste $Parent $Cause.Gestes
    Add-Texte $Parent 'Quand aller chez un réparateur' 15 '#FFFFFF' 'SemiBold' '0,18,0,0'
    Add-Texte $Parent ([string]$Cause.Pro) 14 '#EEEBF7' 'Normal' '0,8,0,0'
    $autre = if ($Cause.VoirPanne) { Get-Panne $script:Pannes ([string]$Cause.VoirPanne) } else { $null }
    if ($autre) {
        $b = New-Button "Faire le diagnostic « $($autre.Titre) »" 'BtnPrimary'
        $b.HorizontalAlignment = 'Left'; $b.Margin = '0,16,0,0'; $b.Tag = [string]$autre.Id
        $b.Add_Click({ param($s) Start-Diagnostic ([string]$s.Tag) })
        [void]$Parent.Children.Add($b)
    }
}

# À la fermeture : arrêter ce qui tourne (minuteries, travaux de fond), sinon rien ne doit rester ouvert
function Stop-Contenu { }
