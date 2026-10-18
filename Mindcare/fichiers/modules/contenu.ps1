# Mindcare : CE QUE FAIT L'APPLI. Les trois écrans du diagnostic : le choix de la panne, les questions une à une (avec
# « Je ne sais pas » et « Retour »), puis la conclusion. Les pannes et le calcul sont dans diagnostic.ps1.
# Chargé par Mindcare.ps1 après diagnostic.ps1 ; Initialize-Contenu est appelé par evenements.ps1.

$script:Pannes       = $null   # le contenu de questions.json
$script:PanneEnCours = $null   # la panne choisie
$script:ChoixFaits   = @()     # les choix faits jusqu'ici (voir diagnostic.ps1)

function Initialize-Contenu {
    try { $script:Pannes = Read-Pannes } catch { Write-Log "Questions illisibles ($QuestionsFichier) : $_" }
    # Tests : un parcours tout fait, « extinction » ou « extinction:0,1,-1 » (outils\tester.ps1)
    if ($EnTest -and $env:MINDCARE_TEST_PARCOURS -and $script:Pannes) {
        $id, $suite = $env:MINDCARE_TEST_PARCOURS -split ':', 2
        $p = Get-Panne $script:Pannes $id
        if ($p) {
            $script:PanneEnCours = $p
            $script:ChoixFaits = @(if ($suite) { $suite -split ',' | ForEach-Object { [int]$_ } })
            Show-Etape
            if ($env:MINDCARE_TEST_BAS) { $Window.Add_ContentRendered({ $ui.Defilement.ScrollToEnd() }) }   # (capture du bas)
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
}

function Start-Diagnostic([string]$Id) {
    $script:PanneEnCours = Get-Panne $script:Pannes $Id
    $script:ChoixFaits = @()
    Write-Log "Diagnostic : $Id"
    Show-Etape
}

# La question où on en est, ou la conclusion si le parcours est fini
function Show-Etape {
    $etat = Get-Parcours $script:PanneEnCours $script:ChoixFaits
    $script:ChoixFaits = $etat.Choix
    if ($etat.Question) { Show-Question $etat } else { Show-Conclusion $etat }
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
    $c = New-Card
    Add-Texte $c "$($script:PanneEnCours.Titre) : question $($script:ChoixFaits.Count + 1)" 13 '#00E5FF' 'SemiBold' '0,0,0,8'
    Add-Titre $c ([string]$q.Texte) 'H2'
    if ($q.Aide) { Add-Texte $c ([string]$q.Aide) 14 '#A9A3C9' 'Normal' '0,6,0,0' }
    $i = 0
    foreach ($r in $q.Reponses) {
        Add-ChoixBouton $c (New-Text ([string]$r.Texte) 15 '#F2EFFA') $i { param($s) Add-Choix ([int]$s.Tag) }
        $i++
    }
    $rg = New-Rangee $c
    Add-Bouton $rg 'Je ne sais pas' { Add-Choix -1 }
    Add-Bouton $rg 'Retour' { Undo-Choix }
}

# ---------------------------------------------------------------------------
# 3. La conclusion
# ---------------------------------------------------------------------------
function Show-Conclusion($Etat) {
    $causes = @(Get-Conclusion $script:PanneEnCours $Etat)
    $liste = if ($causes) { ($causes | ForEach-Object { "$($_.Cause.Id) ($($_.Points))" }) -join ', ' } else { 'rien de clair' }
    Write-Log "Conclusion $($script:PanneEnCours.Id) : $liste (choix $($script:ChoixFaits -join ','))"
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
    $rg = New-Rangee $rp
    Add-Bouton $rg 'Nouveau diagnostic' { Show-Accueil } 'BtnPrimary'
    Add-Bouton $rg 'Changer ma dernière réponse' { Undo-Choix }
}

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

# La carte vivante dans l'accueil de Mindset (voir systeme.ps1, Write-EtatMindset) : ce qu'il y a d'utile à voir
# sans ouvrir l'appli. Le dernier diagnostic arrivera avec les diagnostics gardés (étape 5 du plan).
function Get-EtatCarte {
    @{
        Titre  = 'Dernier diagnostic'
        Scores = @()                                                                # par exemple : @(@{ Nom = 'Santé'; Valeur = 82 })
        Lignes = @(@{ Texte = "Aucun diagnostic pour l'instant."; Etat = 'info' })   # ok, info, attention, urgent
    }
}

# À la fermeture : arrêter ce qui tourne (minuteries, travaux de fond), sinon rien ne doit rester ouvert
function Stop-Contenu { }
