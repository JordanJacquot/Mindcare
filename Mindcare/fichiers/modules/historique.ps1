# Mindcare : les enquêtes gardées (diagnostics.json, dans le dossier de données) : les 10 dernières, finies ou non, pour
# en reprendre une, revoir sa réponse, ou en faire un rapport pour le réparateur. La dernière s'affiche sur la carte de
# Mindset. Chargé par Mindcare.ps1 avant contenu.ps1.
#
#   Une enquête gardée : { Id, Panne, Choix (voir enquete.ps1), Debut, Maj (dates), Fini, Suspect (trouvé), Titre (la réponse) }

$HistoriqueFichier = Join-Path $DataDir 'diagnostics.json'
$HistoriqueMax     = 10
$script:Historique = @()   # de la plus récente à la plus ancienne
$script:Fr         = [Globalization.CultureInfo]::GetCultureInfo('fr-FR')

function Read-Historique {
    $j = Read-JsonFile $HistoriqueFichier
    $script:Historique = @(if ($j) { foreach ($d in $j.Diagnostics) { if ($d.Id -and $d.Panne) { $d } } })
}

# Enregistre où en est une enquête, qui passe en tête de liste. Sans aucune réponse, elle n'est pas gardée.
# Revoir une enquête sans rien y changer ne la déplace pas.
function Save-Diagnostic([string]$Id, [string]$PanneId, [int[]]$LesChoix, [bool]$EstFini, $LeSuspect, [string]$LeTitre) {
    $ancien = @($script:Historique | Where-Object { $_.Id -eq $Id }) | Select-Object -First 1
    if ($ancien -and (@($ancien.Choix) -join ',') -eq (@($LesChoix) -join ',') -and [bool]$ancien.Fini -eq $EstFini -and [string]$ancien.Titre -eq $LeTitre) { return }
    $liste = @($script:Historique | Where-Object { $_.Id -ne $Id })
    if ($LesChoix.Count) {
        $maintenant = Get-Date -Format s
        $d = [pscustomobject][ordered]@{
            Id = $Id; Panne = $PanneId; Choix = @($LesChoix)
            Debut = $(if ($ancien) { [string]$ancien.Debut } else { $maintenant }); Maj = $maintenant
            Fini = $EstFini; Suspect = [string]$LeSuspect; Titre = $LeTitre
        }
        $liste = @($d) + $liste
    }
    $script:Historique = @($liste | Select-Object -First $HistoriqueMax)
    try { Save-JsonFile $HistoriqueFichier ([ordered]@{ Version = 2; Diagnostics = $script:Historique }) } catch { Write-Log "Enquêtes non enregistrées : $_" }
    Write-EtatMindset
}

function Format-Date([string]$Iso, [string]$Modele = "d MMMM 'à' HH'h'mm") {
    try {
        $t = [datetime]::Parse($Iso, [Globalization.CultureInfo]::InvariantCulture).ToString($Modele, $script:Fr)
        if ($t -match '^1 ') { $t = '1er' + $t.Substring(1) }   # (le 1er janvier, pas le 1 janvier)
        $t
    } catch { $Iso }
}

# Ce que dit une enquête gardée : sa panne (absente d'une ancienne version : elle n'est plus montrée), son suspect
# trouvé, et une phrase (la réponse, ou « pas finie »)
function Get-DiagnosticInfo($D) {
    $p = if ($script:Donnees) { Get-Panne $script:Donnees ([string]$D.Panne) } else { $null }
    $s = if ($script:Donnees -and $D.Suspect) { Get-Suspect $script:Donnees ([string]$D.Suspect) } else { $null }
    $phrase = if (-not $D.Fini) { 'pas finie' } elseif ($D.Titre) { [string]$D.Titre } else { 'à faire vérifier' }
    [pscustomobject]@{ Panne = $p; Suspect = $s; Phrase = $phrase }
}

# ---------------------------------------------------------------------------
# Le rapport pour le réparateur : un texte simple (la panne, la réponse, les preuves, les réponses, le PC)
# ---------------------------------------------------------------------------
function Get-InfosPC {
    if ($null -ne $script:InfosPC) { return $script:InfosPC }   # (lu une fois : la lecture prend un moment)
    $script:InfosPC = ''
    try {
        $cs = Get-CimInstance Win32_ComputerSystem -ErrorAction Stop
        $os = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
        $script:InfosPC = "$(([string]$cs.Manufacturer).Trim()) $(([string]$cs.Model).Trim()), $(([string]$os.Caption).Trim())"
    } catch { Write-Log "Modèle du PC illisible : $_" }
    $script:InfosPC
}

function Get-ResumeTexte($Panne, $Enquete, [string]$Titre) {
    $l = New-Object 'Collections.Generic.List[string]'
    $l.Add('Mindcare : rapport de l''enquête'); $l.Add('')
    $l.Add("Date : $(Format-Date (Get-Date -Format s) "d MMMM yyyy 'à' HH'h'mm")")
    $l.Add("Panne : $($Panne.Titre)")
    if ($Enquete.Notes -contains 'cepc') {
        $pc = Get-InfosPC
        if ($pc) { $l.Add("Ordinateur : $pc") }
        $b = @(Get-Barrettes)
        if ($b) { $l.Add("Mémoire : $($b -join ' ; ')") }
    }
    $l.Add(''); $l.Add("Réponse : $Titre")
    if ($Enquete.Preuves) { $l.Add(''); $l.Add('Comment on le sait :'); foreach ($p in $Enquete.Preuves) { $l.Add("- $p") } }
    $ecartes = @($Enquete.Ecartes | Where-Object { -not (Get-Suspect $script:Donnees $_.Id).Urgent } | ForEach-Object { "$((Get-Suspect $script:Donnees $_.Id).Titre)$(if ($_.Raison) { " ($($_.Raison))" })" })
    if ($ecartes) { $l.Add(''); $l.Add('Écartés :'); foreach ($e in $ecartes) { $l.Add("- $e") } }
    if ($Enquete.Verdict.Type -eq 'reparateur') { $l.Add(''); $l.Add("Il reste : $(@($Enquete.Verdict.Restants | ForEach-Object { (Get-Suspect $script:Donnees $_).Titre }) -join ', ')") }
    if ($Enquete.Auto) {
        $l.Add(''); $l.Add('Ce que Mindcare a vérifié sur ce PC (ce que Windows a noté, la place libre, la batterie...) :')
        foreach ($a in $Enquete.Auto) { $l.Add("- $($a.Texte)") }
    }
    $l.Add(''); $l.Add('Les réponses :')
    foreach ($f in $Enquete.Faits) { $l.Add("- $($f.Texte)"); $l.Add("  $($f.Reponse)") }
    $l.Add(''); $l.Add("Fait avec Mindcare $AppVersion.")
    $l -join "`r`n"
}

# Enregistre le rapport sur le Bureau (dans le dossier de données pendant les tests) et l'ouvre ; renvoie son chemin
function Save-Resume([string]$Texte) {
    $dossier = if ($EnTest) { $DataDir } else { [Environment]::GetFolderPath('Desktop') }
    $nom = "Mindcare, diagnostic du $((Get-Date).ToString('d-MM-yyyy HH''h''mm'))"
    $f = Join-Path $dossier "$nom.txt"
    for ($n = 2; [IO.File]::Exists($f); $n++) { $f = Join-Path $dossier "$nom ($n).txt" }   # (deux rapports dans la même minute : le premier est gardé)
    [IO.File]::WriteAllText($f, $Texte, (New-Object Text.UTF8Encoding($true)))   # (avec BOM : le Bloc-notes lit bien les accents)
    Write-Log "Rapport enregistré : $f"
    if (-not $EnTest) { try { Start-Process -FilePath $f } catch { Write-Log "Rapport non ouvert : $_" } }
    $f
}

# ---------------------------------------------------------------------------
# La carte vivante dans l'accueil de Mindset (voir systeme.ps1, Write-EtatMindset) : la dernière enquête
# ---------------------------------------------------------------------------
# Mindset coupe chaque ligne de sa carte à 80 caractères : chaque ligne tient dans 80, coupée au dernier mot entier
function Get-Court([string]$Texte, [int]$Max = 80) {
    if ($Texte.Length -le $Max) { return $Texte }
    $t = $Texte.Substring(0, $Max - 3)
    $i = $t.LastIndexOf(' ')
    if ($i -gt $Max / 2) { $t = $t.Substring(0, $i) }
    $t.TrimEnd(' ', ',', ':', ';', '(') + '...'
}

# La réponse d'une enquête finie, sur une ligne de la carte : la consigne utile ne doit jamais être coupée
function Get-PhraseCarte($Info) {
    $p = [string]$Info.Phrase
    if (-not $Info.Suspect -and $p -eq (Get-VerdictTitre $null $null ([pscustomobject]@{ Type = 'rien' }))) { return 'Aucune piste trouvée : montre le rapport à un réparateur' }
    if ($p.Length -le 80) { return $p }
    if (-not $Info.Suspect) { return 'Plusieurs pistes : un réparateur départagera' }
    Get-Court ($p -split ' : ', 2)[0]   # (« ce qui a été trouvé : le détail » : ce qui a été trouvé)
}

function Get-EtatCarte {
    $carte = @{ Titre = 'Dernier diagnostic'; Scores = @(); Lignes = @(@{ Texte = "Aucun diagnostic pour l'instant."; Etat = 'info' }) }
    if (-not $script:Historique.Count -or -not $script:Donnees) { return $carte }
    $d = $script:Historique[0]
    $i = Get-DiagnosticInfo $d
    if (-not $i.Panne) { return $carte }
    $quand = Format-Date ([string]$d.Debut)
    # (la panne, puis la réponse sur sa propre ligne : une seule phrase « panne : réponse » coupait la réponse)
    $carte.Lignes = if (-not $d.Fini) {
        @(@{ Texte = Get-Court ([string]$i.Panne.Titre); Etat = 'info' }, @{ Texte = Get-Court "Pas finie (commencée le $quand), à reprendre dans Mindcare."; Etat = 'info' })
    } else {
        @(@{ Texte = Get-Court ([string]$i.Panne.Titre); Etat = 'info' }
          @{ Texte = Get-PhraseCarte $i; Etat = $(if ($i.Suspect -and $i.Suspect.Urgent) { 'urgent' } else { 'attention' }) }
          @{ Texte = "Le $quand"; Etat = 'info' })
    }
    $carte
}
