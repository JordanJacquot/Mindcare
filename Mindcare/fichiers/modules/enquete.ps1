# Mindcare : le moteur des enquêtes (enquetes.json, le même fichier pour le PC et le futur téléphone). Rien n'est affiché
# ici (contenu.ps1 s'en charge) : outils\tester.ps1 s'en sert aussi tel quel. Chargé par Mindcare.ps1 avant contenu.ps1.
#
# Une enquête part des suspects d'une panne et les élimine, étape par étape : des questions, des vérifications
# automatiques (ce que Windows a noté, si Mindcare est ouvert sur le PC en panne) et des tests à faire. Elle finit quand
# un suspect est confirmé, ou quand il n'y a plus d'étape : un seul suspect restant est la réponse, plusieurs vont au
# réparateur avec le rapport. Un parcours, ce sont les réponses données dans l'ordre : le numéro de la réponse (0, 1...),
# ou -1 pour « Je ne sais pas ». Les mêmes réponses donnent toujours la même enquête (pas d'IA, pas de hasard).

$EnquetesFichier = Join-Path $AppDir 'enquetes.json'
$IndicesConnus = 'coupures', 'ecransbleus', 'bsod-memoire', 'bsod-pilote', 'bsod-disque', 'bsod-materiel', 'materiel',
                 'testmemoire-erreurs', 'testmemoire-ok', 'disque', 'pilotegraphique', 'majredemarrage', 'freinage', 'placelibre', 'allume', 'usurebatterie'

function Read-Enquetes([string]$Fichier = $EnquetesFichier) { [IO.File]::ReadAllText($Fichier, [Text.Encoding]::UTF8) | ConvertFrom-Json }

function Get-Panne($Donnees, [string]$Id) { foreach ($p in $Donnees.Pannes) { if ($p.Id -eq $Id) { return $p } }; $null }
function Get-Famille($Donnees, [string]$Id) { foreach ($f in $Donnees.Familles) { if ($f.Id -eq $Id) { return $f } }; $null }
function Get-Suspect($Donnees, [string]$Id) { if ($Id) { $Donnees.Suspects.$Id } else { $null } }

# Les étapes d'une panne, prêtes : une étape « Ref » prend le test partagé du même nom, et ses propres champs par-dessus
function Get-Etapes($Donnees, $Panne) {
    foreach ($e in $Panne.Etapes) {
        $base = if ($e.Ref) { $Donnees.Tests.([string]$e.Ref) } else { $null }
        $o = [ordered]@{}
        if ($base) { foreach ($pr in $base.PSObject.Properties) { $o[$pr.Name] = $pr.Value }; $o['Id'] = [string]$e.Ref }
        foreach ($pr in $e.PSObject.Properties) { if ($pr.Name -ne 'Ref') { $o[$pr.Name] = $pr.Value } }
        [pscustomobject]$o
    }
}

# Rejoue une enquête : l'étape où on en est ($null une fois finie), les suspects restants et écartés, les preuves, ce que
# Windows a noté (Auto), les réponses données (Faits), et la réponse finale (Verdict : trouve, elimination, reparateur, rien)
function Get-Enquete($Donnees, $Panne, [int[]]$Choix = @(), $Indices = $null) {
    $etapes = @(Get-Etapes $Donnees $Panne)
    $st = @{ Restants = (New-Object 'Collections.Generic.List[string]'); Ecartes = @(); Notes = @(); Preuves = @(); Confirme = $null; Detail = '' }
    foreach ($s in $Panne.Suspects) { $st.Restants.Add([string]$s) }
    $faits = @(); $auto = @(); $faites = @{}; $n = 0; $courante = $null

    # Une réponse (ou une vérification) : ses mots retenus, les suspects qu'elle écarte, garde ou confirme, et sa preuve
    $applique = {
        param($Effet, [string]$Preuve)
        if (-not $Effet) { return }
        $agit = $false
        if ($Effet.Note) { $st.Notes += [string]$Effet.Note }
        foreach ($id in @($Effet.Ecarte)) { if ($id -and $st.Restants.Remove([string]$id)) { $st.Ecartes += [pscustomobject]@{ Id = [string]$id; Raison = $Preuve }; $agit = $true } }
        if ($Effet.Garde) {
            foreach ($id in @($st.Restants)) { if (@($Effet.Garde) -notcontains $id) { [void]$st.Restants.Remove($id); $st.Ecartes += [pscustomobject]@{ Id = $id; Raison = $Preuve }; $agit = $true } }
        }
        if ($Effet.Confirme -and $st.Restants.Contains([string]$Effet.Confirme)) { $st.Confirme = [string]$Effet.Confirme; $st.Detail = [string]$Effet.Detail; $agit = $true }
        if ($agit -and $Preuve -and $st.Preuves -notcontains $Preuve) { $st.Preuves += $Preuve }
    }

    while (-not $st.Confirme) {
        # Les suspects qui ne concernent pas ce PC (un portable n'a pas de bloc d'alimentation...)
        foreach ($id in @($st.Restants)) {
            $sus = Get-Suspect $Donnees $id
            if (@($sus.Sauf | Where-Object { $_ -and $st.Notes -contains $_ })) { [void]$st.Restants.Remove($id); $st.Ecartes += [pscustomobject]@{ Id = $id; Raison = 'Ne concerne pas ce type de PC' } }
        }
        $et = $null
        foreach ($x in $etapes) {
            if ($faites[[string]$x.Id]) { continue }
            if ($x.Pour -and -not @($x.Pour | Where-Object { $st.Restants.Contains([string]$_) })) { continue }
            if ($x.SiNote -and -not @($x.SiNote | Where-Object { $st.Notes -contains $_ })) { continue }
            if ($x.SaufNote -and @($x.SaufNote | Where-Object { $st.Notes -contains $_ })) { continue }
            $et = $x; break
        }
        if (-not $et) { break }
        $faites[[string]$et.Id] = $true
        if ($et.Type -eq 'auto') {
            # Ce que Windows a noté : seulement si Mindcare est ouvert sur le PC en panne
            if (-not $Indices -or $st.Notes -notcontains 'cepc') { continue }
            $ind = $Indices[[string]$et.Indice]
            if (-not $ind) { continue }
            $texte = if ($ind.Vu) { $ind.Texte } else { $ind.TexteOk }
            if ($texte -and -not @($auto | Where-Object { $_.Texte -eq $texte })) { $auto += [pscustomobject]@{ Texte = [string]$texte; Vu = [bool]$ind.Vu } }
            & $applique $(if ($ind.Vu) { $et.SiVu } else { $et.SiNonVu }) ([string]$texte)
            continue
        }
        if ($n -ge $Choix.Count) { $courante = $et; break }
        $c = $Choix[$n]; $n++
        $r = if ($c -ge 0 -and $c -lt @($et.Reponses).Count) { @($et.Reponses)[$c] } else { $null }
        $faits += [pscustomobject]@{ Etape = $et; Reponse = $(if ($r) { [string]$r.Texte } else { 'Je ne sais pas' }); Court = [string]$et.Court; Texte = [string]$et.Texte }
        if ($r) { & $applique $r $(if ($r.SansPreuve) { '' } elseif ($r.Preuve) { [string]$r.Preuve } else { "$($et.Court) : $($r.Texte)" }) }
    }

    # Les étapes qui peuvent encore venir après celle-ci (au plus) : pour « Étape 2 sur 5 »
    $aVenir = 0
    if ($courante) {
        foreach ($x in $etapes) {
            if ($faites[[string]$x.Id] -or $x.Type -eq 'auto') { continue }
            if ($x.Pour -and -not @($x.Pour | Where-Object { $st.Restants.Contains([string]$_) })) { continue }
            if ($x.SiNote -and -not @($x.SiNote | Where-Object { $st.Notes -contains $_ })) { continue }
            if ($x.SaufNote -and @($x.SaufNote | Where-Object { $st.Notes -contains $_ })) { continue }
            $aVenir++
        }
    }

    $verdict = if ($courante) { $null }
               elseif ($st.Confirme) { [pscustomobject]@{ Type = 'trouve'; Suspect = $st.Confirme; Detail = $st.Detail; Restants = @($st.Confirme) } }
               elseif ($st.Restants.Count -eq 1) { [pscustomobject]@{ Type = 'elimination'; Suspect = $st.Restants[0]; Detail = ''; Restants = @($st.Restants) } }
               elseif ($st.Restants.Count -eq 0) { [pscustomobject]@{ Type = 'rien'; Suspect = $null; Detail = ''; Restants = @() } }
               else { [pscustomobject]@{ Type = 'reparateur'; Suspect = $null; Detail = ''; Restants = @($st.Restants) } }
    [pscustomobject]@{
        Etape = $courante; Choix = @($Choix | Select-Object -First $n); Restants = @($st.Restants); Ecartes = $st.Ecartes
        Notes = $st.Notes; Preuves = $st.Preuves; Faits = $faits; Auto = $auto; Verdict = $verdict; AVenir = $aVenir
    }
}

# Un nom en minuscule au début (« la mémoire (RAM) »), sans toucher aux noms propres (Windows, RAM)
function Get-Minuscule([string]$Texte) { if ($Texte) { $Texte.Substring(0, 1).ToLowerInvariant() + $Texte.Substring(1) } else { '' } }

# La phrase de la réponse finale : « Une barrette de mémoire (RAM) est abîmée », « Il chauffe trop : un ventilateur ne tourne plus »
function Get-VerdictTitre($Donnees, $Panne, $Verdict) {
    if (-not $Verdict) { return '' }
    switch ($Verdict.Type) {
        'trouve'      { $s = Get-Suspect $Donnees $Verdict.Suspect; if ($Verdict.Detail) { "$($s.Trouve) : $($Verdict.Detail)" } else { [string]$s.Trouve } }
        'elimination' { [string](Get-Suspect $Donnees $Verdict.Suspect).Trouve }
        'reparateur'  { "Il reste $(@($Verdict.Restants | ForEach-Object { Get-Minuscule ([string](Get-Suspect $Donnees $_).Titre) }) -join ' ou ') : un réparateur départagera" }
        default       { 'Aucune piste ne correspond : montre ce rapport à un réparateur' }
    }
}

# La recherche par mots (sans internet, toujours les mêmes résultats) : les pannes dont le nom, la phrase ou les mots
# contiennent les mots tapés, les plus proches d'abord
function ConvertTo-Simple([string]$Texte) { ((($Texte.Normalize([Text.NormalizationForm]::FormD) -replace '\p{Mn}', '').ToLowerInvariant() -replace "[^a-z0-9 ]", ' ') -replace '\s+', ' ').Trim() }
function Find-Pannes($Donnees, [string]$Requete) {
    $vides = 'il', 'le', 'la', 'les', 'un', 'une', 'de', 'du', 'des', 'ne', 'pas', 'plus', 'mon', 'ma', 'mes', 'pc', 'ordi', 'ordinateur',
             'et', 'ou', 'est', 'en', 'sur', 'qui', 'que', 'se', 'ca', 'tout', 'au', 'aux', 'je', 'j', 'me', 'quand', 'avec'
    $mots = @((ConvertTo-Simple $Requete) -split '\s+' | Where-Object { $_.Length -ge 2 -and $vides -notcontains $_ })
    if (-not $mots) { return @() }
    $phrase = ((ConvertTo-Simple $Requete) -replace '\s+', ' ').Trim()
    $res = foreach ($p in $Donnees.Pannes) {
        $fam = Get-Famille $Donnees ([string]$p.Famille)
        $titre = ConvertTo-Simple "$($p.Titre)"
        $tout = ConvertTo-Simple "$($p.Titre) $($p.Phrase) $(@($p.Mots) -join ' ') $($fam.Titre)"
        $score = 0
        foreach ($m in $mots) {
            $racine = if ($m.Length -gt 5) { $m.Substring(0, $m.Length - 1) } else { $m }   # (« redemarre » trouve « redemarrage »)
            if ($titre -match "\b$([regex]::Escape($racine))") { $score += 3 } elseif ($tout -match "\b$([regex]::Escape($racine))") { $score += 2 }
        }
        # La phrase tapée telle quelle (« pas de son ») compte plus que des mots épars
        if ($phrase.Length -ge 4) { if ($titre.Contains($phrase)) { $score += 6 } elseif ($tout.Contains($phrase)) { $score += 4 } }
        if ($score) { [pscustomobject]@{ Panne = $p; Score = $score } }
    }
    @($res | Sort-Object @{ Expression = 'Score'; Descending = $true } | ForEach-Object { $_.Panne })
}

# Ce qui ne va pas dans le fichier des enquêtes (une liste vide : tout est bon)
function Test-Enquetes($Donnees, [string]$Texte = '') {
    $pb = @()
    if ($Texte.IndexOfAny([char[]](0x2013, 0x2014)) -ge 0) { $pb += 'Tiret long dans le fichier' }
    $suspects = @($Donnees.Suspects.PSObject.Properties | ForEach-Object { $_.Name })
    foreach ($s in $Donnees.Suspects.PSObject.Properties) {
        if (-not $s.Value.Titre -or -not $s.Value.Trouve -or -not $s.Value.Pourquoi -or -not $s.Value.AFaire -or -not $s.Value.Pro) { $pb += "Suspect $($s.Name) : Titre, Trouve, Pourquoi, AFaire ou Pro manquant" }
    }
    $familles = @($Donnees.Familles | ForEach-Object { [string]$_.Id })
    $ids = @()
    foreach ($p in $Donnees.Pannes) {
        $nom = "Panne $($p.Id)"
        if ($ids -contains $p.Id) { $pb += "$nom : en double" }; $ids += $p.Id
        if (-not $p.Titre -or -not $p.Phrase) { $pb += "$nom : Titre ou Phrase manquant" }
        if ($familles -notcontains [string]$p.Famille) { $pb += "$nom : famille inconnue « $($p.Famille) »" }
        foreach ($s in $p.Suspects) { if ($suspects -notcontains $s) { $pb += "$nom : suspect inconnu « $s »" } }
        foreach ($e in $p.Etapes) { if ($e.Ref -and -not $Donnees.Tests.([string]$e.Ref)) { $pb += "$nom : test partagé inconnu « $($e.Ref) »" } }
        $etapes = @(Get-Etapes $Donnees $p)
        $vus = @{}
        foreach ($e in $etapes) {
            $ou = "$nom, étape $($e.Id)"
            if (-not $e.Id) { $pb += "$nom : une étape sans Id" } elseif ($vus[[string]$e.Id]) { $pb += "$ou : en double" }; $vus[[string]$e.Id] = $true
            if ('question', 'test', 'auto' -notcontains $e.Type) { $pb += "$ou : type inconnu « $($e.Type) »" }
            foreach ($s in @($e.Pour)) { if ($s -and @($p.Suspects) -notcontains $s) { $pb += "$ou : suspect « $s » absent de la panne" } }
            $effets = @()
            if ($e.Type -eq 'auto') {
                if ($IndicesConnus -notcontains [string]$e.Indice) { $pb += "$ou : indice de Windows inconnu « $($e.Indice) »" }
                $effets = @($e.SiVu, $e.SiNonVu) | Where-Object { $_ }
            } else {
                if (-not $e.Texte -or -not $e.Court) { $pb += "$ou : Texte ou Court manquant" }
                if (@($e.Reponses).Count -lt 2) { $pb += "$ou : moins de 2 réponses" }
                if ($e.Type -eq 'test' -and -not $e.Gestes) { $pb += "$ou : un test sans gestes" }
                foreach ($r in $e.Reponses) { if (-not $r.Texte) { $pb += "$ou : une réponse sans texte" } }
                $effets = @($e.Reponses)
            }
            foreach ($f in $effets) {
                foreach ($s in @(@($f.Ecarte) + @($f.Garde) + @($f.Confirme))) { if ($s -and @($p.Suspects) -notcontains $s) { $pb += "$ou : suspect « $s » absent de la panne" } }
            }
        }
    }
    foreach ($f in $Donnees.Familles) { if (-not $f.Id -or -not $f.Titre -or -not $f.Phrase -or -not $f.Icone) { $pb += "Famille $($f.Id) : Id, Titre, Phrase ou Icone manquant" } }
    if (-not $ids) { $pb += 'Aucune panne' }
    $pb
}
