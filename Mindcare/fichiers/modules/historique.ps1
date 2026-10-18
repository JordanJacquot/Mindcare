# Mindcare : les diagnostics gardés (diagnostics.json, dans le dossier de données) : les 10 derniers, finis ou non, pour
# en reprendre un, le revoir, ou en faire un résumé pour le réparateur. Le dernier s'affiche sur la carte de Mindset.
# Chargé par Mindcare.ps1 avant contenu.ps1.
#
#   Un diagnostic : { Id, Panne, Choix (voir diagnostic.ps1), Debut, Maj (dates), Fini, Causes (Id des causes retenues) }

$HistoriqueFichier = Join-Path $DataDir 'diagnostics.json'
$HistoriqueMax     = 10
$script:Historique = @()   # du plus récent au plus ancien
$script:Fr         = [Globalization.CultureInfo]::GetCultureInfo('fr-FR')

function Read-Historique {
    $j = Read-JsonFile $HistoriqueFichier
    $script:Historique = @(if ($j) { foreach ($d in $j.Diagnostics) { if ($d.Id -and $d.Panne) { $d } } })
}

# Enregistre où en est un diagnostic, qui passe en tête de liste. Sans aucune réponse, il n'est pas gardé.
# Revoir un diagnostic sans rien y changer ne le déplace pas.
function Save-Diagnostic([string]$Id, [string]$PanneId, [int[]]$LesChoix, [bool]$EstFini, $LesCauses) {
    $ancien = @($script:Historique | Where-Object { $_.Id -eq $Id }) | Select-Object -First 1
    if ($ancien -and (@($ancien.Choix) -join ',') -eq (@($LesChoix) -join ',') -and [bool]$ancien.Fini -eq $EstFini) { return }
    $liste = @($script:Historique | Where-Object { $_.Id -ne $Id })
    if ($LesChoix.Count) {
        $maintenant = Get-Date -Format s
        $d = [pscustomobject][ordered]@{
            Id = $Id; Panne = $PanneId; Choix = @($LesChoix)
            Debut = $(if ($ancien) { [string]$ancien.Debut } else { $maintenant }); Maj = $maintenant
            Fini = $EstFini; Causes = @($LesCauses | Where-Object { $_ } | ForEach-Object { [string]$_ })
        }
        $liste = @($d) + $liste
    }
    $script:Historique = @($liste | Select-Object -First $HistoriqueMax)
    try { Save-JsonFile $HistoriqueFichier ([ordered]@{ Version = 1; Diagnostics = $script:Historique }) } catch { Write-Log "Diagnostics non enregistrés : $_" }
    Write-EtatMindset
}

function Format-Date([string]$Iso, [string]$Modele = "d MMMM 'à' HH'h'mm") {
    try {
        $t = [datetime]::Parse($Iso, [Globalization.CultureInfo]::InvariantCulture).ToString($Modele, $script:Fr)
        if ($t -match '^1 ') { $t = '1er' + $t.Substring(1) }   # (le 1er janvier, pas le 1 janvier)
        $t
    } catch { $Iso }
}

# Ce que dit un diagnostic gardé : sa panne, sa cause la plus probable, et une phrase (la cause, ou « pas fini »)
function Get-DiagnosticInfo($D) {
    $p = Get-Panne $script:Pannes ([string]$D.Panne)
    $cause = $null
    if ($p -and @($D.Causes).Count) { foreach ($c in $p.Causes) { if ([string]$c.Id -eq [string]@($D.Causes)[0]) { $cause = $c } } }
    $phrase = if (-not $D.Fini) { 'pas fini' } elseif ($cause) { [string]$cause.Titre } else { 'pas de cause claire' }
    [pscustomobject]@{ Panne = $p; Cause = $cause; Phrase = $phrase }
}

# ---------------------------------------------------------------------------
# Le résumé pour le réparateur : un texte simple (la panne, la conclusion, les réponses, le modèle du PC)
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

function Get-ResumeTexte($Panne, $Etat, $Causes) {
    $l = New-Object 'Collections.Generic.List[string]'
    $l.Add('Mindcare : résumé du diagnostic'); $l.Add('')
    $l.Add("Date : $(Format-Date (Get-Date -Format s) "d MMMM yyyy 'à' HH'h'mm")")
    $l.Add("Panne : $($Panne.Titre)")
    $pc = Get-InfosPC
    if ($pc) { $l.Add("Ordinateur : $pc") }
    $l.Add('')
    if ($Causes) {
        $l.Add("Cause la plus probable : $($Causes[0].Cause.Titre)")
        foreach ($a in ($Causes | Select-Object -Skip 1)) { $l.Add("Autre piste : $($a.Cause.Titre)") }
    } else { $l.Add("Conclusion : $($script:Pannes.RienTrouve.Titre)") }
    if ($Etat.Journal) {
        $l.Add(''); $l.Add('Ce que Windows a noté sur ce PC (30 derniers jours) :')
        foreach ($j in $Etat.Journal) { $l.Add("- $($j.Texte)") }
    }
    $l.Add(''); $l.Add('Les réponses :')
    foreach ($e in $Etat.Resume) { $l.Add("- $($e.Question)"); $l.Add("  $($e.Reponse)") }
    $l.Add(''); $l.Add("Fait avec Mindcare $AppVersion.")
    $l -join "`r`n"
}

# Enregistre le résumé sur le Bureau (dans le dossier de données pendant les tests) et l'ouvre ; renvoie son chemin
function Save-Resume([string]$Texte) {
    $dossier = if ($EnTest) { $DataDir } else { [Environment]::GetFolderPath('Desktop') }
    $f = Join-Path $dossier "Mindcare, diagnostic du $((Get-Date).ToString('d-MM-yyyy HH''h''mm')).txt"
    [IO.File]::WriteAllText($f, $Texte, (New-Object Text.UTF8Encoding($true)))   # (avec BOM : le Bloc-notes lit bien les accents)
    Write-Log "Résumé enregistré : $f"
    if (-not $EnTest) { try { Start-Process -FilePath $f } catch { Write-Log "Résumé non ouvert : $_" } }
    $f
}

# ---------------------------------------------------------------------------
# La carte vivante dans l'accueil de Mindset (voir systeme.ps1, Write-EtatMindset) : le dernier diagnostic
# ---------------------------------------------------------------------------
function Get-Court([string]$Texte, [int]$Max = 80) { if ($Texte.Length -le $Max) { $Texte } else { $Texte.Substring(0, $Max - 3).TrimEnd() + '...' } }

function Get-EtatCarte {
    $carte = @{ Titre = 'Dernier diagnostic'; Scores = @(); Lignes = @(@{ Texte = "Aucun diagnostic pour l'instant."; Etat = 'info' }) }
    if (-not $script:Historique.Count -or -not $script:Pannes) { return $carte }
    $d = $script:Historique[0]
    $i = Get-DiagnosticInfo $d
    if (-not $i.Panne) { return $carte }
    $quand = Format-Date ([string]$d.Debut)
    $carte.Lignes = if (-not $d.Fini) {
        @(@{ Texte = Get-Court "$($i.Panne.Titre) : pas fini"; Etat = 'info' }, @{ Texte = "Commencé le $quand, à reprendre dans Mindcare."; Etat = 'info' })
    } elseif ($i.Cause) {
        @(@{ Texte = Get-Court "$($i.Panne.Titre) : $($i.Cause.Titre)"; Etat = $(if ($i.Cause.Urgent) { 'urgent' } else { 'attention' }) }, @{ Texte = "Le $quand"; Etat = 'info' })
    } else {
        @(@{ Texte = Get-Court "$($i.Panne.Titre) : pas de cause claire"; Etat = 'info' }, @{ Texte = "Le $quand"; Etat = 'info' })
    }
    $carte
}
