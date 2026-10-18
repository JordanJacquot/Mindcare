# Mindcare : les pannes et leurs questions (questions.json, le même fichier pour le PC et le futur téléphone), et le
# calcul de la conclusion. Rien n'est affiché ici (contenu.ps1 s'en charge) : outils\tester.ps1 s'en sert aussi tel quel.
# Chargé par Mindcare.ps1 avant contenu.ps1.
#
# Un parcours, ce sont les choix faits dans l'ordre : le numéro de la réponse (0, 1...), ou -1 pour « Je ne sais pas ».
# Chaque réponse donne des points à des causes ; la conclusion les classe. Les mêmes choix donnent toujours la même
# conclusion (pas d'IA, pas de hasard).

$QuestionsFichier = Join-Path $AppDir 'questions.json'

function Read-Pannes([string]$Fichier = $QuestionsFichier) { [IO.File]::ReadAllText($Fichier, [Text.Encoding]::UTF8) | ConvertFrom-Json }

function Get-Panne($Donnees, [string]$Id) { foreach ($p in $Donnees.Pannes) { if ($p.Id -eq $Id) { return $p } }; $null }
function Get-Question($Panne, [string]$Id) { foreach ($q in $Panne.Questions) { if ($q.Id -eq $Id) { return $q } }; $null }

# Où mène une réponse ($null : « Je ne sais pas ») : la sienne si elle en a une, sinon celle de la question
function Get-Suite($Question, $Reponse) { if ($Reponse -and $Reponse.Suite) { [string]$Reponse.Suite } else { [string]$Question.Suite } }

# Rejoue un parcours : la question où on en est ($null une fois à la conclusion), les points de chaque cause, les mots
# retenus (portable, fixe) et le résumé (chaque question et sa réponse)
function Get-Parcours($Panne, [int[]]$Choix = @()) {
    $points = @{}; foreach ($c in $Panne.Causes) { $points[[string]$c.Id] = 0 }
    $notes = @(); $resume = @(); $fait = 0
    $q = Get-Question $Panne $Panne.Debut
    foreach ($n in $Choix) {
        if (-not $q) { break }
        $r = if ($n -ge 0 -and $n -lt $q.Reponses.Count) { $q.Reponses[$n] } else { $null }
        if ($r) {
            if ($r.Points) { foreach ($pr in $r.Points.PSObject.Properties) { $points[$pr.Name] += [int]$pr.Value } }
            if ($r.Note) { $notes += [string]$r.Note }
        }
        $resume += [pscustomobject]@{ Question = [string]$q.Texte; Court = [string]$q.Court; Reponse = $(if ($r) { [string]$r.Texte } else { 'Je ne sais pas' }) }
        $fait++
        $suite = Get-Suite $q $r
        $q = if ($suite -eq 'conclusion') { $null } else { Get-Question $Panne $suite }
    }
    [pscustomobject]@{ Question = $q; Choix = @($Choix | Select-Object -First $fait); Points = $points; Notes = $notes; Resume = $resume }
}

# Les questions qui viennent sans doute après celle-ci (en suivant la suite par défaut) : la liste des étapes à venir.
# Une estimation : une réponse peut mener ailleurs.
function Get-Prochaines($Panne, $Question) {
    $vues = @{}; $q = $Question
    while ($q) {
        $suite = Get-Suite $q $null
        if ($suite -eq 'conclusion' -or $vues[$suite]) { break }
        $vues[$suite] = $true
        $q = Get-Question $Panne $suite
        if ($q) { $q }
    }
}

# Les causes retenues, la plus probable d'abord (puis au plus 2 autres pistes d'au moins 2 points). À égalité, l'ordre
# du fichier. Une cause « Seule » (le danger) s'affiche sans les autres. Aucune : rien de clair.
function Get-Conclusion($Panne, $Parcours) {
    $liste = @(); $i = 0
    foreach ($c in $Panne.Causes) {
        $pt = $Parcours.Points[[string]$c.Id]
        $ecartee = $false
        foreach ($s in $c.Sauf) { if ($s -and $Parcours.Notes -contains $s) { $ecartee = $true } }
        if ($pt -gt 0 -and -not $ecartee) { $liste += [pscustomobject]@{ Cause = $c; Points = $pt; Ordre = $i } }
        $i++
    }
    if (-not $liste) { return @() }
    $liste = @($liste | Sort-Object @{ Expression = 'Points'; Descending = $true }, @{ Expression = 'Ordre'; Descending = $false })
    if ($liste[0].Cause.Seule) { return @($liste[0]) }
    @($liste[0]) + @($liste | Select-Object -Skip 1 | Where-Object { $_.Points -ge 2 } | Select-Object -First 2)
}

# Ce qui ne va pas dans le fichier des pannes (une liste vide : tout est bon) : textes manquants, question ou cause
# inconnue, question jamais posée, boucle (on tournerait en rond), cause jamais atteinte, renvoi vers une panne
# inconnue, tiret long
function Test-Pannes($Donnees, [string]$Texte = '') {
    $pb = @()
    if ($Texte.IndexOfAny([char[]](0x2013, 0x2014)) -ge 0) { $pb += 'Tiret long dans le fichier' }
    if (-not $Donnees.Securite) { $pb += 'Consignes de sécurité manquantes' }
    if (-not $Donnees.RienTrouve.Titre) { $pb += 'Texte « rien trouvé » manquant' }
    $idsPannes = @(); $voir = @()
    foreach ($p in $Donnees.Pannes) {
        $nom = "Panne $($p.Id)"
        if (-not $p.Id -or -not $p.Titre -or -not $p.Phrase) { $pb += "$nom : Id, Titre ou Phrase manquant" }
        if ($idsPannes -contains $p.Id) { $pb += "$nom : en double" }; $idsPannes += $p.Id
        $causes = @(foreach ($c in $p.Causes) { [string]$c.Id })
        foreach ($c in $p.Causes) {
            if (-not $c.Titre -or -not $c.Resume -or -not $c.Pourquoi -or -not $c.Gestes -or -not $c.Pro) { $pb += "$nom, cause $($c.Id) : Titre, Resume, Pourquoi, Gestes ou Pro manquant" }
            if (-not $c.Seule -and -not ($c.Test.Titre -and $c.Test.Etapes -and $c.Test.Si)) { $pb += "$nom, cause $($c.Id) : pas de test qui la confirme (Test : Titre, Etapes, Si)" }
            if ($c.VoirPanne) { $voir += [pscustomobject]@{ Ou = "$nom, cause $($c.Id)"; Vers = [string]$c.VoirPanne; Depuis = [string]$p.Id } }
        }
        $ids = @(foreach ($q in $p.Questions) { [string]$q.Id })
        if (@($ids | Select-Object -Unique).Count -ne $ids.Count) { $pb += "$nom : deux questions ont le même Id" }
        if ($ids -notcontains $p.Debut) { $pb += "$nom : la première question ($($p.Debut)) n'existe pas" }
        $suites = @{}; $atteintes = @{}
        foreach ($q in $p.Questions) {
            $ou = "$nom, question $($q.Id)"
            if (-not $q.Texte -or -not $q.Court) { $pb += "$ou : pas de texte, ou pas de nom court" }
            if ($q.Reponses.Count -lt 2) { $pb += "$ou : moins de 2 réponses" }
            $s = @(Get-Suite $q $null)
            foreach ($r in $q.Reponses) {
                if (-not $r.Texte) { $pb += "$ou : une réponse sans texte" }
                $s += Get-Suite $q $r
                if ($r.Points) { foreach ($pr in $r.Points.PSObject.Properties) {
                    if ($causes -notcontains $pr.Name) { $pb += "$ou : cause inconnue « $($pr.Name) »" } else { $atteintes[$pr.Name] = $true }
                } }
            }
            foreach ($x in $s) { if ($x -ne 'conclusion' -and $ids -notcontains $x) { $pb += "$ou : mène à une question inconnue « $x »" } }
            $suites[[string]$q.Id] = @($s | Where-Object { $_ -ne 'conclusion' -and $ids -contains $_ } | Select-Object -Unique)
        }
        foreach ($c in $causes) { if (-not $atteintes[$c]) { $pb += "$nom : la cause $c n'est jamais donnée par une réponse" } }
        # Parcours de toutes les questions depuis la première : jamais posée, ou boucle
        $etat = @{}   # 1 : en cours, 2 : fini
        $script:BouclePannes = $false
        function Step-Question([string]$Id) {
            if ($etat[$Id] -eq 2) { return }
            if ($etat[$Id] -eq 1) { $script:BouclePannes = $true; return }
            $etat[$Id] = 1
            foreach ($x in $suites[$Id]) { Step-Question $x }
            $etat[$Id] = 2
        }
        if ($ids -contains $p.Debut) { Step-Question ([string]$p.Debut) }
        if ($script:BouclePannes) { $pb += "$nom : une boucle (on tournerait en rond)" }
        foreach ($id in $ids) { if (-not $etat[$id]) { $pb += "$nom : la question $id n'est jamais posée" } }
    }
    if (-not $idsPannes) { $pb += 'Aucune panne' }
    foreach ($v in $voir) { if ($idsPannes -notcontains $v.Vers -or $v.Vers -eq $v.Depuis) { $pb += "$($v.Ou) : renvoie vers une panne inconnue « $($v.Vers) »" } }
    $pb
}
