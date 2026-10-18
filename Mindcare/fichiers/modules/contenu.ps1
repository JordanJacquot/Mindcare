# Mindcare : CE QUE FAIT L'APPLI. C'est ici (et dans de nouveaux modules à côté) que vont ses vraies fonctions.
# Le reste du squelette (Mindset, thème, fermeture, carte vivante, journal) est déjà prêt et n'a pas à changer.
# Chargé par Mindcare.ps1 après systeme.ps1 ; Initialize-Contenu est appelé par evenements.ps1.
#
# Un nouveau module : l'ajouter dans la liste $modules de Mindcare.ps1 (avant 'evenements').

# Au démarrage : remplir la fenêtre ($ui.Contenu) avec des cartes (New-Card, New-Text, New-Button : interface.ps1)
function Initialize-Contenu {
    $c = New-Card
    $t = New-Object System.Windows.Controls.TextBlock
    $t.Text = 'Bienvenue dans Mindcare'; $t.Style = $Window.FindResource('H1')
    [void]$c.Children.Add($t)
    $d = New-Text $AppDescription 14 '#A9A3C9'; $d.Margin = '0,6,0,16'
    [void]$c.Children.Add($d)
    $b = New-Button 'Essayer'
    $b.HorizontalAlignment = 'Left'
    $b.Add_Click({ Write-Log 'Bouton Essayer'; $ui.Phrase.Text = 'Ça marche !' })
    [void]$c.Children.Add($b)
}

# La carte vivante dans l'accueil de Mindset (voir systeme.ps1, Write-EtatMindset) : ce qu'il y a d'utile à voir
# sans ouvrir l'appli. Rien d'important à dire : une seule ligne « ok ».
function Get-EtatCarte {
    @{
        Titre  = 'Dernier diagnostic'
        Scores = @()                                                                # par exemple : @(@{ Nom = 'Santé'; Valeur = 82 })
        Lignes = @(@{ Texte = "Aucun diagnostic pour l'instant."; Etat = 'info' })   # ok, info, attention, urgent
    }
}

# À la fermeture : arrêter ce qui tourne (minuteries, travaux de fond), sinon rien ne doit rester ouvert
function Stop-Contenu { }
