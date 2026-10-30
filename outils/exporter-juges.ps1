# Exporte les juges du moteur d'enquête en Rust (rust\mindcare-moteur\tests\juges\) : des enquêtes, des recherches par
# mots, le garde-fou de l'assistant, la validation du catalogue et « chaque suspect identifiable », joués par le VRAI
# moteur PowerShell de Mindcare (Mindcare\fichiers\modules\enquete.ps1, et Select-PreRempli de ia.ps1). Les tests Rust
# (cargo test, dans rust\) rejouent exactement les mêmes et doivent trouver exactement pareil.
#
# Les suites de réponses tirées au hasard sont écrites dans les juges : le Rust ne refait jamais le hasard de .NET.
# Les cas nommés, les recherches et le garde-fou de outils\tester.ps1 sont lus dans tester.ps1 lui-même (toujours à jour).
# À relancer quand le catalogue ou la logique du moteur change (les tests Rust le disent : l'empreinte du catalogue).
# Rien n'est lu ni modifié sur le PC : seulement le dépôt (aucune fenêtre, aucun réseau).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File outils\exporter-juges.ps1                   tout (environ 20 minutes)
#   powershell ... -File outils\exporter-juges.ps1 -Partie parcours -De 0 -A 49                     une partie seulement
#
# Les gros fichiers (parcours) rangent leurs textes une seule fois dans « Textes » : un nombre dans un résultat est le
# numéro d'un texte de cette table, une chaîne est le texte lui-même.
param(
    [ValidateSet('tout', 'cas', 'parcours', 'recherches', 'garde-fou', 'validation', 'identifiables', 'divergences')][string]$Partie = 'tout',
    [int]$ParPanne = 40,      # parcours au hasard sans indices de Windows, par panne
    [int]$AvecIndices = 12,   # parcours au hasard avec des indices de Windows simulés, par panne
    [int]$De = 0, [int]$A = 299,   # (parcours : les pannes de $De à $A, dans l'ordre du catalogue)
    [int]$Graine = 20261030
)
$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$AppDir = Join-Path $racine 'Mindcare\fichiers'
$sortie = Join-Path $racine 'rust\mindcare-moteur\tests\juges'
New-Item -ItemType Directory -Force -Path $sortie | Out-Null
. (Join-Path $AppDir 'modules\enquete.ps1')
# (ia.ps1 ne fait rien au chargement, seulement des variables et des fonctions : son dossier n'est jamais créé ni lu ici)
$EnTest = $true; $DataDir = Join-Path $racine '_test\juges-inutilise'
. (Join-Path $AppDir 'modules\ia.ps1')
$texteCatalogue = [IO.File]::ReadAllText($EnquetesFichier, [Text.Encoding]::UTF8)
$d = Read-Enquetes
$donnees = $d   # (le nom utilisé par les fonctions reprises de tester.ps1)

# L'empreinte du catalogue (FNV-1a sur 32 bits, sans les retours chariot : git peut changer les fins de ligne). Les
# tests Rust la recalculent : si le catalogue a changé depuis l'export, ils demandent de relancer l'export.
function Get-Empreinte([string]$Fichier) {
    $h = [long]2166136261
    foreach ($b in [IO.File]::ReadAllBytes($Fichier)) { if ($b -ne 13) { $h = (($h -bxor $b) * 16777619) % 4294967296 } }
    '{0:x8}' -f $h
}
$catalogue = [ordered]@{ Fichier = 'Mindcare/fichiers/enquetes.json'; Empreinte = (Get-Empreinte $EnquetesFichier); Pannes = @($d.Pannes).Count }

# Les morceaux de tester.ps1 repris tels quels (lus dans le fichier : ils restent à jour)
$ast = [Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'tester.ps1'), [ref]$null, [ref]$null)
function Get-Morceau([scriptblock]$Filtre, [string]$Quoi) {
    $n = $ast.Find($Filtre, $true)
    if (-not $n) { throw "tester.ps1 : $Quoi introuvable" }
    $n
}
$fnIndice = Get-Morceau { param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'New-Indice' } 'New-Indice'
. ([scriptblock]::Create($fnIndice.Extent.Text))
$fnTrouvables = Get-Morceau { param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Get-SuspectsTrouvables' } 'Get-SuspectsTrouvables'
. ([scriptblock]::Create($fnTrouvables.Extent.Text))

# ---------------------------------------------------------------------------
# L'écriture des juges
# ---------------------------------------------------------------------------
$script:TxtListe = $null; $script:TxtIndex = $null
function Start-Textes { $script:TxtListe = New-Object 'Collections.Generic.List[string]'; $script:TxtIndex = New-Object 'Collections.Generic.Dictionary[string,int]' ([StringComparer]::Ordinal) }
function Stop-Textes { $script:TxtListe = $null; $script:TxtIndex = $null }
# Un texte d'un résultat : tel quel, ou son numéro dans la table « Textes » (les gros fichiers)
function T($Texte) {
    $s = [string]$Texte
    if ($null -eq $script:TxtIndex) { return $s }
    $k = 0
    if ($script:TxtIndex.TryGetValue($s, [ref]$k)) { return $k }
    $k = $script:TxtListe.Count; $script:TxtListe.Add($s); $script:TxtIndex.Add($s, $k)
    $k
}

# (BaseObject : une liste rendue par une fonction est enveloppée, et PowerShell 5.1 l'écrirait {"value":[...],"Count":n})
function ConvertTo-JsonListe($Elements) { '[' + ((@($Elements) | ForEach-Object { ConvertTo-Json -InputObject $_.PSObject.BaseObject -Depth 12 -Compress }) -join ',') + ']' }
# Un juge : un objet dont chaque champ est déjà en JSON (les grandes listes, élément par élément)
function Write-Juge([string]$Nom, $Champs) {
    $morceaux = foreach ($k in $Champs.Keys) { (ConvertTo-Json -InputObject $k -Compress) + ':' + $Champs[$k] }
    # (les tirets longs des cas « tiret long dans le fichier » sont écrits \u2013 et \u2014 : aucun dans le dépôt)
    $json = ('{' + ($morceaux -join ',') + '}').Replace([string][char]0x2013, '\u2013').Replace([string][char]0x2014, '\u2014')
    [IO.File]::WriteAllText((Join-Path $sortie $Nom), $json, (New-Object Text.UTF8Encoding($false)))
    "$Nom : $([math]::Round((Get-Item -LiteralPath (Join-Path $sortie $Nom)).Length / 1KB)) Ko"
}

# Tout ce que Get-Enquete renvoie, sous une forme comparable
function Get-Resultat($Panne, $e) {
    $ecartes = New-Object 'Collections.Generic.List[object]'
    foreach ($x in $e.Ecartes) { $ecartes.Add(@([string]$x.Id, (T $x.Raison))) }
    $faits = New-Object 'Collections.Generic.List[object]'
    foreach ($f in $e.Faits) { $faits.Add(@([string]$f.Etape.Id, (T $f.Court), (T $f.Reponse), (T $f.Texte))) }
    $auto = New-Object 'Collections.Generic.List[object]'
    foreach ($x in $e.Auto) { $auto.Add(@((T $x.Texte), [bool]$x.Vu)) }
    $preuves = New-Object 'Collections.Generic.List[object]'
    foreach ($x in $e.Preuves) { $preuves.Add((T $x)) }
    $v = $e.Verdict
    $verdict = $null
    if ($v) { $verdict = @([string]$v.Type, [string]$v.Suspect, (T $v.Detail), [string[]]@($v.Restants | ForEach-Object { [string]$_ })) }
    [ordered]@{
        Etape = $(if ($e.Etape) { [string]$e.Etape.Id } else { '' })
        Choix = [int[]]@($e.Choix | ForEach-Object { [int]$_ })
        Restants = [string[]]@($e.Restants | ForEach-Object { [string]$_ })
        Ecartes = $ecartes.ToArray()
        Notes = [string[]]@($e.Notes | ForEach-Object { [string]$_ })
        Preuves = $preuves.ToArray()
        Faits = $faits.ToArray()
        Auto = $auto.ToArray()
        Verdict = $verdict
        Titre = (T (Get-VerdictTitre $d $Panne $v))
        AVenir = [int]$e.AVenir
    }
}

# Des indices de Windows (une table : clé -> { Vu, Texte, TexteOk, Partiel }), sous forme de liste
function ConvertTo-IndicesExport($Indices) {
    if ($null -eq $Indices) { return $null }
    $l = New-Object 'Collections.Generic.List[object]'
    foreach ($k in $Indices.Keys) { $i = $Indices[$k]; $l.Add([ordered]@{ Id = [string]$k; Vu = [bool]$i.Vu; Texte = [string]$i.Texte; TexteOk = [string]$i.TexteOk; Partiel = [bool]$i.Partiel }) }
    , $l.ToArray()
}

# ---------------------------------------------------------------------------
# 1. Les cas nommés de tester.ps1, l'indice ignoré sans « cepc » et l'avancement « étape N sur M »
# ---------------------------------------------------------------------------
if ($Partie -in 'tout', 'cas') {
    Stop-Textes
    $casAst = Get-Morceau { param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -eq '$cas' } 'la liste $cas'
    $casNommes = @(Invoke-Expression $casAst.Right.Extent.Text)
    $liste = foreach ($k in $casNommes) {
        $p = Get-Panne $d $k.Panne
        $e = Get-Enquete $d $p $k.Choix $k.Indices
        [ordered]@{
            Nom = [string]$k.Nom; Panne = [string]$k.Panne; Entree = [int[]]@($k.Choix); Indices = (ConvertTo-IndicesExport $k.Indices)
            Attendu = [ordered]@{ Type = [string]$k.Type; Suspect = [string]$k.Suspect; Restants = [string[]]@($k.Restants | Where-Object { $_ }) }
            R = (Get-Resultat $p $e)
        }
    }
    $pR = Get-Panne $d 'redemarre'
    $indAilleurs = @{ 'testmemoire-erreurs' = (New-Indice $true 'erreurs') }
    $ailleurs = [ordered]@{ Panne = 'redemarre'; Entree = [int[]]@(1, 1); Indices = (ConvertTo-IndicesExport $indAilleurs); R = (Get-Resultat $pR (Get-Enquete $d $pR @(1, 1) $indAilleurs)) }
    $pS = Get-Panne $d 'songresille'
    $avancement = foreach ($c in @(, @()) + @(, @(1)) + @(, @(1, 1, 1, 1, 1))) { [ordered]@{ Panne = 'songresille'; Entree = [int[]]@($c); R = (Get-Resultat $pS (Get-Enquete $d $pS $c)) } }
    Write-Juge 'cas-nommes.json' ([ordered]@{
        Catalogue = (ConvertTo-Json $catalogue -Compress)
        Cas = (ConvertTo-JsonListe $liste)
        Ailleurs = (ConvertTo-Json $ailleurs -Depth 12 -Compress)
        Avancement = (ConvertTo-Json ([ordered]@{ Attendu = '1/8 2/7 6/7'; Cas = @($avancement) }) -Depth 12 -Compress)
    })
    "cas nommés : $($casNommes.Count)"
}

# ---------------------------------------------------------------------------
# 2. Des parcours au hasard sur chaque panne, avec leurs résultats complets
# ---------------------------------------------------------------------------
function Get-ReponseCepc($Etape) { $k = 0; foreach ($r in @($Etape.Reponses)) { if ($r -and [string]$r.Note -eq 'cepc') { return $k }; $k++ }; -1 }
# Des indices de Windows inventés : vus, pas vus, partiels, absents, textes vides ou partagés, clés en majuscules
function New-IndicesHasard($H) {
    $ind = @{}
    foreach ($id in $IndicesConnus) {
        $t = $H.Next(10)
        if ($t -lt 2) { continue }
        $nom = if ($H.Next(8) -eq 0) { $id.ToUpperInvariant() } else { $id }
        $texte = switch ($H.Next(10)) { 0 { '' } 1 { 'Texte commun' } 2 { 'TEXTE COMMUN' } default { "Vu par Windows : $id" } }
        $ok = switch ($H.Next(10)) { 0 { '' } 1 { $null } 2 { 'Texte commun' } default { "Rien de noté : $id" } }
        $ind[$nom] = [pscustomobject]@{ Vu = ($t -lt 5); Texte = $texte; TexteOk = $ok; Partiel = ($H.Next(3) -eq 0) }
    }
    $ind
}
if ($Partie -in 'tout', 'parcours') {
    Start-Textes
    $pannes = @($d.Pannes)
    $tables = New-Object 'Collections.Generic.List[object]'
    $cas = New-Object 'Collections.Generic.List[object]'
    $fin = [math]::Min($A, $pannes.Count - 1)
    $nouveau = { param($P, $No, $Choix, $Pas, $Ind) [ordered]@{ Panne = [string]$P.Id; Indices = $No; Entree = [int[]]@($Choix); Pas = $Pas; R = (Get-Resultat $P (Get-Enquete $d $P ([int[]]@($Choix)) $Ind)) } }
    for ($ip = $De; $ip -le $fin; $ip++) {
        $p = $pannes[$ip]; $h = New-Object Random ($Graine + $ip)
        $cas.Add((& $nouveau $p -1 @() @() $null))
        for ($k = 0; $k -lt $ParPanne + $AvecIndices; $k++) {
            $avec = $k -ge $ParPanne
            $indices = $null; $no = -1
            if ($avec) { $indices = New-IndicesHasard $h; $no = $tables.Count; $tables.Add((ConvertTo-IndicesExport $indices)) }
            $choix = New-Object 'Collections.Generic.List[int]'; $pas = New-Object 'Collections.Generic.List[object]'
            $e = $null
            for ($t = 0; $t -lt 60; $t++) {
                $e = Get-Enquete $d $p $choix.ToArray() $indices
                if (-not $e.Etape) { break }
                $pas.Add(@([string]$e.Etape.Id, [int]$e.AVenir))
                $nb = @($e.Etape.Reponses).Count
                $cepc = if ($avec) { Get-ReponseCepc $e.Etape } else { -1 }
                # (avec des indices : le plus souvent « Mindcare est ouvert sur le PC en panne », sinon ils ne servent pas)
                $c = if ($cepc -ge 0 -and $h.Next(4) -gt 0) { $cepc } elseif ($h.Next(10) -eq 0) { -1 } else { $h.Next($nb) }
                $choix.Add($c)
            }
            if ($e.Etape) { $e = Get-Enquete $d $p $choix.ToArray() $indices }
            $cas.Add([ordered]@{ Panne = [string]$p.Id; Indices = $no; Entree = [int[]]$choix.ToArray(); Pas = $pas.ToArray(); R = (Get-Resultat $p $e) })
            $tout = @($choix.ToArray())
            # Un arrêt en route, des réponses en trop, des numéros hors de la liste
            if ($k % 4 -eq 1 -and $tout.Count -gt 1) { $cas.Add((& $nouveau $p $no @($tout | Select-Object -First ($h.Next($tout.Count - 1) + 1)) @() $indices)) }
            if ($k % 10 -eq 0) {
                $cas.Add((& $nouveau $p $no (@($tout) + 0 + 5) @() $indices))
                if ($tout.Count) { $hors = [int[]]@($tout); $hors[$h.Next($hors.Count)] = $(if ($h.Next(2)) { 99 } else { -7 }); $cas.Add((& $nouveau $p $no $hors @() $indices)) }
            }
        }
    }
    $nom = 'parcours-{0:000}-{1:000}.json' -f $De, $fin
    Write-Juge $nom ([ordered]@{
        Catalogue = (ConvertTo-Json $catalogue -Compress)
        Textes = (ConvertTo-JsonListe $script:TxtListe.ToArray())
        Indices = (ConvertTo-JsonListe $tables.ToArray())
        Parcours = (ConvertTo-JsonListe $cas.ToArray())
    })
    Stop-Textes
    "parcours : $($cas.Count) (pannes $De à $fin)"
}

# ---------------------------------------------------------------------------
# 3. Les recherches par mots : celles de tester.ps1, puis des milliers d'autres, avec le classement complet
# ---------------------------------------------------------------------------
function Get-SansAccents([string]$Texte) { $Texte.Normalize([Text.NormalizationForm]::FormD) -replace '\p{Mn}', '' }
if ($Partie -in 'tout', 'recherches') {
    Stop-Textes
    $h = New-Object Random $Graine
    $rAst = Get-Morceau { param($n) $n -is [Management.Automation.Language.ForEachStatementAst] -and $n.Variable.VariablePath.UserPath -eq 'r' -and $n.Condition.Extent.Text -match 'redémarre tout seul' } 'les recherches'
    $tester = foreach ($r in @(Invoke-Expression $rAst.Condition.Extent.Text)) { [ordered]@{ Q = [string]$r[0]; Attendu = [string]$r[1]; P = [string[]]@(Find-Pannes $d $r[0] | ForEach-Object { [string]$_.Id }) } }
    $vues = New-Object 'Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    $requetes = New-Object 'Collections.Generic.List[string]'
    $ajoute = { param($Q) if ($null -ne $Q -and $vues.Add([string]$Q)) { $requetes.Add([string]$Q) } }
    $motsCatalogue = New-Object 'Collections.Generic.List[string]'
    foreach ($p in $d.Pannes) { foreach ($m in ("$($p.Titre) $($p.Phrase) $(@($p.Mots) -join ' ')" -split '\s+')) { if ($m.Length -ge 3) { $motsCatalogue.Add($m) } } }
    $hasardMot = { $motsCatalogue[$h.Next($motsCatalogue.Count)] }
    $vides = 'il', 'le', 'la', 'les', 'un', 'une', 'de', 'du', 'des', 'ne', 'pas', 'plus', 'mon', 'ma', 'mes', 'pc', 'ordi', 'quand', 'avec', "j'ai", 'ça'
    foreach ($p in $d.Pannes) {
        $titre = [string]$p.Titre
        & $ajoute $titre; & $ajoute ([string]$p.Phrase)
        foreach ($m in $p.Mots) { & $ajoute ([string]$m) }
        $mots = @($titre -split ' ' | Where-Object { $_ })
        if ($mots.Count -gt 2) { & $ajoute (($mots | Select-Object -Skip 1 -First 2) -join ' ') }
        & $ajoute $titre.ToUpperInvariant()
        & $ajoute (Get-SansAccents $titre)
        $longs = @("$titre $($p.Phrase)" -split '\s+' | Where-Object { $_.Length -ge 5 })
        if ($longs.Count) {
            $w = $longs[$h.Next($longs.Count)]; $i = $h.Next($w.Length)
            & $ajoute $w.Remove($i, 1)                                                     # une lettre oubliée
            $w = $longs[$h.Next($longs.Count)]; $i = $h.Next($w.Length - 1)
            & $ajoute ($w.Substring(0, $i) + $w[$i + 1] + $w[$i] + $w.Substring($i + 2))      # deux lettres inversées
            $w = $longs[$h.Next($longs.Count)]; $i = $h.Next($w.Length)
            & $ajoute $w.Insert($i, [string]$w[$i])                                        # une lettre en double
            $w = $longs[$h.Next($longs.Count)]
            & $ajoute $w.Substring(0, 3 + $h.Next([math]::Min(4, $w.Length - 2)))           # un début de mot
            $w = $longs[$h.Next($longs.Count)]
            & $ajoute ($w -replace 'e', 'é' -replace 'a', 'à')                              # des accents en trop
        }
        & $ajoute "$($vides[$h.Next($vides.Count)]) $($vides[$h.Next($vides.Count)]) $(& $hasardMot)"
        & $ajoute "$(& $hasardMot) $(& $hasardMot)"
        & $ajoute "$(& $hasardMot) $($vides[$h.Next($vides.Count)]) $(& $hasardMot) $(& $hasardMot)"
    }
    foreach ($f in $d.Familles) { & $ajoute ([string]$f.Titre); & $ajoute ([string]$f.Phrase) }
    foreach ($q in 'il redémarre', 'mon pc fait des bips', 'le wifi se coupe toutes les 5 minutes', 'écran bleu MEMORY_MANAGEMENT', '0x1A',
                   "j'ai plus de son", 'le son grésille', 'le pc démarre sur le bios', 'ma webcam marche pas', "l'ordi est super lent",
                   'ça chauffe', 'Le PC  ne   démarre PAS !', 'œil', 'ÉCRAN NOIR', 'clé usb', 'a', '', '   ', 'zzzz', 'mémoire insuffisante',
                   'son', 'ecran', 'écran', 'ÉCRAN', 'e' + [char]0x301 + 'cran', 'wi-fi', 'wifi', 'Wi‑Fi', 'usb-c', 'bsod', 'BSOD', '0x0000001A',
                   "l'écran", ('l' + [char]0x2019 + 'écran'),'pc portable', 'portable', 'ordi portable', 'ordinateur portable', 'il il il', 'de de de', 'pas de son pas de son',
                   'redémarre redémarre', 'redemarrage', 'redemarrer', 'redémarré', 'chauf', 'chauffe', 'chauffer', 'surchauffe',
                   "  écran  bleu  ", "écran`tbleu", "écran`r`nbleu", 'écran' + [char]0xA0 + 'bleu', 'écran' + [char]0x202F + 'bleu',
                   'ﬁchier', 'Straße', 'İnternet', 'ǅ', 'Ｗｉｆｉ', 'K' + [char]0x212A, [char]0x212B + 'ngström', 'Ω', 'œuf', 'Œuvre', 'æ',
                   'son 🔊', '🔥 chauffe', 'ça plante 😡', 'naïve', 'NAÏVE', 'noël', 'où', 'çà', 'mémoire', 'MÉMOIRE', 'memoire', 'MEMOIRE',
                   '12', '1', '100%', 'c:', 'C:\Windows', 'http://exemple.fr', 'a b c d e f', 'xx', 'xy zz', 'bi', 'bip', 'bips', 'biiip') { & $ajoute $q }
    $liste = foreach ($q in $requetes) { [ordered]@{ Q = $q; P = [string[]]@(Find-Pannes $d $q | ForEach-Object { [string]$_.Id }) } }
    Write-Juge 'recherches.json' ([ordered]@{
        Catalogue = (ConvertTo-Json $catalogue -Compress)
        Tester = (ConvertTo-JsonListe $tester)
        Recherches = (ConvertTo-JsonListe $liste)
    })
    "recherches : $(@($tester).Count) de tester.ps1, $($requetes.Count) autres"
}

# ---------------------------------------------------------------------------
# 4. Le garde-fou de l'assistant (Select-PreRempli) : les cas de tester.ps1, puis d'autres
# ---------------------------------------------------------------------------
function ConvertTo-PaireListe($Table) {
    $l = New-Object 'Collections.Generic.List[object]'
    foreach ($k in @($Table.Keys | Sort-Object)) { $l.Add(@([string]$k, [int]$Table[$k])) }
    , $l.ToArray()
}
if ($Partie -in 'tout', 'garde-fou') {
    Stop-Textes
    $h = New-Object Random ($Graine + 7)
    $gAst = Get-Morceau { param($n) $n -is [Management.Automation.Language.ForEachStatementAst] -and $n.Variable.VariablePath.UserPath -eq 'cas' -and $n.Condition.Extent.Text -match 'coupure de courant' } 'les cas du garde-fou'
    $liste = New-Object 'Collections.Generic.List[object]'
    $pSet = Get-Panne $d 'seteint'
    foreach ($c in @(Invoke-Expression $gAst.Condition.Extent.Text)) {
        $g = Select-PreRempli $d $pSet $c[1] $c[0]
        $liste.Add([ordered]@{ Panne = 'seteint'; Phrase = [string]$c[0]; Reponses = (ConvertTo-PaireListe $c[1]); Garde = (ConvertTo-PaireListe $g); Attendu = [string]$c[2] })
    }
    $nTester = $liste.Count
    $remplissage = 'bonjour', 'vraiment', 'depuis', 'hier', 'toujours', 'quand', 'avec', 'dans', 'pour', 'elle', 'merci', 'aide', 'moi', 'svp', 'encore', 'souvent', 'jamais', 'oui', 'non'
    $pannes = @($d.Pannes)
    for ($i = 0; $i -lt 130; $i++) {
        $p = $pannes[$h.Next($pannes.Count)]
        $etapes = @(Get-Etapes $d $p)
        $rep = @{}
        $pool = New-Object 'Collections.Generic.List[string]'
        for ($j = 0; $j -le $h.Next(4); $j++) {
            $et = $etapes[$h.Next($etapes.Count)]
            $id = [string]$et.Id
            if ($h.Next(6) -eq 0) { $id = $id.ToUpperInvariant() }   # (la clé se lit sans tenir compte des majuscules)
            $nb = @($et.Reponses).Count
            $n = $h.Next(-1, $nb + 2)
            $rep[$id] = $n
            $r = if ($n -ge 0 -and $n -lt $nb) { @($et.Reponses)[$n] } else { $null }
            foreach ($m in ("$($et.Texte) $($et.Court) $(if ($r) { $r.Texte })" -split '\s+')) { if ($m) { $pool.Add($m) } }
        }
        if ($h.Next(6) -eq 0) { $rep['etape-inconnue'] = 0 }
        foreach ($m in ("$($p.Titre) $($p.Phrase)" -split '\s+')) { if ($m) { $pool.Add($m) } }
        $mots = for ($j = 0; $j -lt 2 + $h.Next(7); $j++) {
            $m = if ($pool.Count -and $h.Next(3) -gt 0) { $pool[$h.Next($pool.Count)] } else { $remplissage[$h.Next($remplissage.Count)] }
            switch ($h.Next(8)) { 0 { $m.ToUpperInvariant() } 1 { Get-SansAccents $m } 2 { if ($m.Length -gt 4) { $m.Substring(0, 4) } else { $m } } default { $m } }
        }
        $phrase = if ($h.Next(25) -eq 0) { '' } else { ($mots -join ' ') }
        $g = Select-PreRempli $d $p $rep $phrase
        $liste.Add([ordered]@{ Panne = [string]$p.Id; Phrase = $phrase; Reponses = (ConvertTo-PaireListe $rep); Garde = (ConvertTo-PaireListe $g); Attendu = $null })
    }
    # Sans panne, ou sans réponses réglées d'avance : rien n'est gardé
    $liste.Add([ordered]@{ Panne = ''; Phrase = 'mon pc fixe'; Reponses = (ConvertTo-PaireListe @{ type = 1 }); Garde = (ConvertTo-PaireListe (Select-PreRempli $d $null @{ type = 1 } 'mon pc fixe')); Attendu = $null })
    $liste.Add([ordered]@{ Panne = 'seteint'; Phrase = 'mon pc fixe'; Reponses = (ConvertTo-PaireListe @{}); Garde = (ConvertTo-PaireListe (Select-PreRempli $d $pSet @{} 'mon pc fixe')); Attendu = $null })
    Write-Juge 'garde-fou.json' ([ordered]@{ Catalogue = (ConvertTo-Json $catalogue -Compress); Cas = (ConvertTo-JsonListe $liste.ToArray()) })
    "garde-fou : $nTester de tester.ps1, $($liste.Count - $nTester) autres"
}

# ---------------------------------------------------------------------------
# 5. La validation du catalogue (Test-Enquetes) : le vrai catalogue, puis un petit catalogue abîmé de mille façons
# ---------------------------------------------------------------------------
function Set-Champ($O, [string]$Nom, $Valeur) { $O | Add-Member -NotePropertyName $Nom -NotePropertyValue $Valeur -Force }
if ($Partie -in 'tout', 'validation') {
    Stop-Textes
    $reel = @(Test-Enquetes $d $texteCatalogue)
    # Le petit catalogue : quelques pannes qui ont tout (étapes partagées, étapes à elles, vérification automatique,
    # commande à copier), avec seulement leurs suspects et leurs tests
    $base = $texteCatalogue | ConvertFrom-Json
    $choisies = New-Object 'Collections.Generic.List[object]'
    $choisies.Add((Get-Panne $base 'redemarre'))
    foreach ($test in @(
            { param($p) @($p.Etapes | Where-Object { -not $_.Ref -and $_.Type -eq 'auto' }).Count },
            { param($p) @($p.Etapes | Where-Object { $_.Ref -and $base.Tests.([string]$_.Ref).Commande }).Count },
            { param($p) @($p.Etapes | Where-Object { -not $_.Ref -and $_.Type -eq 'test' }).Count },
            { param($p) @($p.Etapes | Where-Object { -not $_.Ref -and $_.Type -eq 'question' -and $_.Pour }).Count })) {
        $p = $base.Pannes | Where-Object { (& $test $_) -and @($choisies | ForEach-Object { $_.Id }) -notcontains $_.Id } | Select-Object -First 1
        if (-not $p) { throw 'validation : pas de panne pour le petit catalogue' }
        $choisies.Add($p)
    }
    $sus = [ordered]@{}; $tests = [ordered]@{}
    foreach ($p in $choisies) {
        foreach ($s in $p.Suspects) { $sus[[string]$s] = $base.Suspects.([string]$s) }
        foreach ($e in $p.Etapes) { if ($e.Ref) { $tests[[string]$e.Ref] = $base.Tests.([string]$e.Ref) } }
    }
    Set-Champ $base 'Pannes' $choisies.ToArray()
    Set-Champ $base 'Suspects' ([pscustomobject]$sus)
    Set-Champ $base 'Tests' ([pscustomobject]$tests)
    $baseJson = ConvertTo-Json $base -Depth 40 -Compress
    # Les étapes du petit catalogue qu'on abîme
    $premiere = { param($M, [scriptblock]$Si) foreach ($p in $M.Pannes) { foreach ($e in $p.Etapes) { if (& $Si $e) { return , @($p, $e) } } }; throw 'validation : étape introuvable' }
    $propre = { param($e) -not $e.Ref -and $e.Type -eq 'question' }
    $auto = { param($e) -not $e.Ref -and $e.Type -eq 'auto' }
    $partage = { param($e) [bool]$e.Ref }
    $testCommande = { param($M) foreach ($t in $M.Tests.PSObject.Properties) { if ($t.Value.Commande) { return $t.Value } }; throw 'validation : pas de commande' }
    $mutations = [ordered]@{
        'le petit catalogue, tel quel' = { param($M) }
        'un tiret demi-cadratin dans une phrase' = { param($M) $M.Pannes[0].Phrase = $M.Pannes[0].Phrase + " $([char]0x2013) ici" }
        'un tiret cadratin dans un titre' = { param($M) $M.Pannes[1].Titre = $M.Pannes[1].Titre + " $([char]0x2014)" }
        'un suspect sans Pro' = { param($M) @($M.Suspects.PSObject.Properties)[0].Value.PSObject.Properties.Remove('Pro') }
        'un suspect sans rien à faire (liste vide)' = { param($M) Set-Champ @($M.Suspects.PSObject.Properties)[1].Value 'AFaire' @() }
        'un suspect au titre vide' = { param($M) @($M.Suspects.PSObject.Properties)[2].Value.Titre = '' }
        'une panne en double' = { param($M) Set-Champ $M 'Pannes' (@($M.Pannes) + @(($M.Pannes[0] | ConvertTo-Json -Depth 40 | ConvertFrom-Json))) }
        'une panne en double (majuscules)' = { param($M) $c = $M.Pannes[2] | ConvertTo-Json -Depth 40 | ConvertFrom-Json; $c.Id = ([string]$c.Id).ToUpperInvariant(); Set-Champ $M 'Pannes' (@($M.Pannes) + @($c)) }
        'une panne sans titre' = { param($M) $M.Pannes[1].PSObject.Properties.Remove('Titre') }
        'une panne à la phrase vide' = { param($M) $M.Pannes[3].Phrase = '' }
        'une famille inconnue' = { param($M) $M.Pannes[0].Famille = 'nulle-part' }
        'une famille en majuscules' = { param($M) $M.Pannes[0].Famille = ([string]$M.Pannes[0].Famille).ToUpperInvariant() }
        'un suspect inconnu' = { param($M) Set-Champ $M.Pannes[0] 'Suspects' (@($M.Pannes[0].Suspects) + 'fantome') }
        'un test partagé inconnu' = { param($M) $x = & $premiere $M $partage; $x[1].Ref = 'test-fantome' }
        'un test partagé en majuscules' = { param($M) $x = & $premiere $M $partage; $x[1].Ref = ([string]$x[1].Ref).ToUpperInvariant() }
        'une étape sans Id' = { param($M) $x = & $premiere $M $propre; $x[1].PSObject.Properties.Remove('Id') }
        'une étape en double' = { param($M) $x = & $premiere $M $propre; Set-Champ $x[0] 'Etapes' (@($x[0].Etapes) + @(($x[1] | ConvertTo-Json -Depth 40 | ConvertFrom-Json))) }
        'une étape en double (majuscules)' = { param($M) $x = & $premiere $M $propre; $c = $x[1] | ConvertTo-Json -Depth 40 | ConvertFrom-Json; $c.Id = ([string]$c.Id).ToUpperInvariant(); Set-Champ $x[0] 'Etapes' (@($x[0].Etapes) + @($c)) }
        'un type inconnu' = { param($M) $x = & $premiere $M $propre; $x[1].Type = 'devinette' }
        'un type en majuscules' = { param($M) $x = & $premiere $M $propre; $x[1].Type = 'QUESTION' }
        'Pour : un suspect absent de la panne' = { param($M) $x = & $premiere $M $propre; Set-Champ $x[1] 'Pour' @('fantome', $x[0].Suspects[0]) }
        'Pour : un seul texte' = { param($M) $x = & $premiere $M $propre; Set-Champ $x[1] 'Pour' 'fantome' }
        'une vérification automatique : indice inconnu' = { param($M) $x = & $premiere $M $auto; $x[1].Indice = 'meteo' }
        'une vérification automatique : SiVu avec un suspect absent' = { param($M) $x = & $premiere $M $auto; Set-Champ $x[1] 'SiVu' ([pscustomobject]@{ Ecarte = @('fantome'); Confirme = 'autre-fantome' }) }
        'une question sans Court' = { param($M) $x = & $premiere $M $propre; $x[1].PSObject.Properties.Remove('Court') }
        'une question au texte vide' = { param($M) $x = & $premiere $M $propre; $x[1].Texte = '' }
        'une seule réponse' = { param($M) $x = & $premiere $M $propre; Set-Champ $x[1] 'Reponses' @(@($x[1].Reponses)[0]) }
        'aucune réponse' = { param($M) $x = & $premiere $M $propre; $x[1].PSObject.Properties.Remove('Reponses') }
        'une liste de réponses vide' = { param($M) $x = & $premiere $M $propre; Set-Champ $x[1] 'Reponses' @() }
        'une réponse sans texte' = { param($M) $x = & $premiere $M $propre; @($x[1].Reponses)[1].Texte = '' }
        'une réponse : suspects absents (Ecarte, Garde, Confirme)' = { param($M) $x = & $premiere $M $propre; $r = @($x[1].Reponses)[0]; Set-Champ $r 'Ecarte' @('fantome1', ''); Set-Champ $r 'Garde' @('fantome2'); Set-Champ $r 'Confirme' 'fantome3' }
        'un test sans gestes' = { param($M) $t = & $testCommande $M; Set-Champ $t 'Gestes' @() }
        'une commande sans lignes' = { param($M) $t = & $testCommande $M; Set-Champ $t.Commande 'Lignes' @() }
        'une commande en un seul texte' = { param($M) $t = & $testCommande $M; Set-Champ $t.Commande 'Lignes' 'echo bonjour' }
        'une commande avec des guillemets français' = { param($M) $t = & $testCommande $M; Set-Champ $t.Commande 'Lignes' (@($t.Commande.Lignes) + 'echo « bonjour »' + ('echo ' + [char]0x201C + 'là' + [char]0x201D + ' ' + [char]0x2018 + 'ici' + [char]0x2019)) }
        'des gestes sans « Copier »' = { param($M) $t = & $testCommande $M; Set-Champ $t 'Gestes' @('Ouvre la fenêtre', 'Colle la ligne') }
        'des gestes avec « copier » en minuscules' = { param($M) $t = & $testCommande $M; Set-Champ $t 'Gestes' @('Ouvre la fenêtre', 'clique sur copier') }
        'une famille sans icône' = { param($M) $M.Familles[0].PSObject.Properties.Remove('Icone') }
        'une famille sans Id' = { param($M) $M.Familles[1].Id = '' }
        'aucune panne' = { param($M) Set-Champ $M 'Pannes' @() }
        'plusieurs défauts à la fois' = { param($M) $M.Pannes[0].Famille = 'nulle-part'; $M.Pannes[1].Phrase = ''; $x = & $premiere $M $propre; $x[1].Type = 'devinette'; $M.Familles[2].Titre = '' }
    }
    $liste = foreach ($nom in $mutations.Keys) {
        $m = $baseJson | ConvertFrom-Json
        & $mutations[$nom] $m
        $json = ConvertTo-Json $m -Depth 40 -Compress
        [ordered]@{ Nom = $nom; Json = $json; Problemes = [string[]]@(Test-Enquetes ($json | ConvertFrom-Json) $json) }
    }
    # Le vrai catalogue, avec un tiret long dans le texte du fichier seulement
    $tiret = @(Test-Enquetes $d ($texteCatalogue + [char]0x2014))
    Write-Juge 'validation.json' ([ordered]@{
        Catalogue = (ConvertTo-Json $catalogue -Compress)
        Reel = (ConvertTo-JsonListe ([string[]]$reel))
        ReelTiret = (ConvertTo-JsonListe ([string[]]$tiret))
        Cas = (ConvertTo-JsonListe $liste)
    })
    "validation : $(@($liste).Count) catalogues abîmés, le vrai : $($reel.Count) problème(s)"
}

# ---------------------------------------------------------------------------
# 6. Chaque suspect identifiable (Get-SuspectsTrouvables de tester.ps1) : ce qui est trouvé, panne par panne
# ---------------------------------------------------------------------------
if ($Partie -in 'tout', 'identifiables') {
    Stop-Textes
    $tousVus = @{}; foreach ($i in $IndicesConnus) { $tousVus[$i] = New-Indice $true $i }
    $liste = foreach ($p in $d.Pannes) {
        $t = Get-SuspectsTrouvables $p $null
        $manque = @($p.Suspects | Where-Object { -not $t.ContainsKey([string]$_) })
        $t2 = $null
        if ($manque) { $t2 = Get-SuspectsTrouvables $p $tousVus; $manque = @($manque | Where-Object { -not $t2.ContainsKey([string]$_) }) }
        [ordered]@{ Panne = [string]$p.Id; SansIndices = [string[]]@($t.Keys | Sort-Object); TousVus = $(if ($t2) { [string[]]@($t2.Keys | Sort-Object) } else { $null }); Manque = [string[]]@($manque) }
    }
    Write-Juge 'identifiables.json' ([ordered]@{ Catalogue = (ConvertTo-Json $catalogue -Compress); Pannes = (ConvertTo-JsonListe $liste) })
    "identifiables : $(@($liste).Count) pannes"
}

# ---------------------------------------------------------------------------
# 7. Les divergences voulues (audit du 25/10) : constat 20, une mesure ratée (redémarrage, place libre, batterie)
# ---------------------------------------------------------------------------
# Quand une mesure rate, journaux.ps1 donne aujourd'hui { Vu = faux, Texte = '', TexteOk = '' } : le moteur la prend pour
# « rien vu » et écarte la piste (la batterie usée, le disque plein, le PC jamais redémarré). La réponse voulue : la
# mesure vaut « inconnu » et n'écarte rien, comme si elle n'avait pas été faite. Le moteur sait déjà le faire (un indice
# absent ne fait rien) : la correction est dans la lecture de Windows (étape 3), qui ne doit pas donner d'indice pour
# une mesure ratée. Ce juge garde les deux réponses, jouées par le moteur PowerShell : celle d'aujourd'hui avec l'indice
# raté, et la réponse voulue avec l'indice absent.
if ($Partie -in 'tout', 'divergences') {
    Stop-Textes
    $h = New-Object Random ($Graine + 20)
    $rate = [pscustomobject]@{ Vu = $false; Texte = ''; TexteOk = ''; Affiche = $false; Partiel = $false }   # (journaux.ps1, « catch »)
    $liste = foreach ($cible in @(@('toutlent', 'allume', 'redemarrage'), @('toutlent', 'placelibre', 'disqueplein'), @('demarragelong', 'placelibre', 'disqueplein'),
                                  @('necharge', 'usurebatterie', 'batterieusee'), @('tientpeu', 'usurebatterie', 'batterieusee'))) {
        $p = Get-Panne $d $cible[0]
        $avecRate = @{ $cible[1] = $rate }
        $trouve = $null
        for ($essai = 0; $essai -lt 300 -and -not $trouve; $essai++) {
            $choix = New-Object 'Collections.Generic.List[int]'
            for ($t = 0; $t -lt 40 -and -not $trouve; $t++) {
                $e1 = Get-Enquete $d $p $choix.ToArray() $avecRate
                $e2 = Get-Enquete $d $p $choix.ToArray() @{}
                if (@($e1.Ecartes | Where-Object { $_.Id -eq $cible[2] }).Count -and @($e2.Restants) -contains $cible[2]) { $trouve = $choix.ToArray(); break }
                if (-not $e1.Etape) { break }
                $cepc = Get-ReponseCepc $e1.Etape
                $choix.Add($(if ($cepc -ge 0) { $cepc } else { $h.Next(@($e1.Etape.Reponses).Count) }))
            }
        }
        if ($null -eq $trouve) { throw "divergences : aucun parcours pour $($cible -join ' / ')" }
        [ordered]@{
            Constat = 20; Nom = "mesure ratée « $($cible[1]) » : la piste « $($cible[2]) » reste"
            Panne = $cible[0]; Entree = [int[]]@($trouve); Mesure = $cible[1]; Piste = $cible[2]
            IndicesPowerShell = (ConvertTo-IndicesExport $avecRate); ReponsePowerShell = (Get-Resultat $p (Get-Enquete $d $p $trouve $avecRate))
            IndicesVoulus = @(); ReponseVoulue = (Get-Resultat $p (Get-Enquete $d $p $trouve @{}))
        }
    }
    $explication = 'Divergences voulues avec le moteur PowerShell (audit du 25/10). Constat 20 : une mesure ratée (redémarrage, place libre, batterie) vaut « inconnu » et n''écarte rien. Le moteur pur ne change pas : un indice absent n''écarte déjà rien. La lecture de Windows (étape 3) ne doit donc pas donner d''indice pour une mesure ratée (aujourd''hui elle donne Vu faux et des textes vides, pris pour « rien vu »). Constats 21 et 23 : hors du moteur pur (relecture des indices, copie de secours), rien ici.'
    Write-Juge 'divergences.json' ([ordered]@{ Catalogue = (ConvertTo-Json $catalogue -Compress); Explication = (ConvertTo-Json $explication -Compress); Divergences = (ConvertTo-JsonListe $liste) })
    "divergences : $(@($liste).Count) cas"
}
