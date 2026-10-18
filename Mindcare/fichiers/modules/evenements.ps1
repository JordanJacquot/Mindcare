# Mindcare : tout est branché ici, une fois les modules chargés. Chargé en dernier par Mindcare.ps1.

Initialize-ThemeVisuels   # (le décor et les images du thème : themes.ps1)
Initialize-Contenu
Start-Surveillances

# La croix (ou « Fermer » dans la barre de Mindset) : l'appli quitte proprement.
# (Une appli qui doit continuer en arrière-plan, comme les rappels d'Evermind, se cacherait ici au lieu de quitter.)
$Window.Add_Closing({ if (-not $script:Quitting) { [void]$Window.Dispatcher.BeginInvoke([action]{ Exit-App }) } })
