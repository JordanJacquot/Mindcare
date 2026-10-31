# Exporte les juges des données de Mindcare en Rust (rust\mindcare-donnees\tests\juges\) : les VRAIS modules
# PowerShell de Mindcare (Mindcare\fichiers\modules\donnees.ps1, historique.ps1, systeme.ps1, contenu.ps1, mindset.ps1)
# rejouent des scénarios sur des dossiers de données d'essai (historiques v2 et anciens, fichiers abîmés, vides, coupés,
# copies .bak, fichiers pris par un autre programme, reprises d'enquêtes, réglages fusionnés, cartes de Mindset,
# rapports), et des cas sans fichier (dates, lignes coupées, cartes, rapports, réponses en attente, indices, chemins,
# suite.json). Les tests Rust (cargo test, dans rust\) rejouent exactement les mêmes et doivent trouver pareil.
#
# Divergences voulues (constat 23 : un fichier vide n'est jamais rangé en copie de secours ; une copie .bak abîmée est
# déplacée de côté, pas copiée) : chaque scénario est joué deux fois, avec les vrais modules puis avec Read-JsonFile et
# Save-JsonFile tels que le Rust les fait (json_sur de suite-commun : le fichier vide ou abîmé est déplacé de côté et sa
# copie remise en place ; seul un fichier qui contient un objet JSON est poussé dans .bak ; une copie abîmée est
# déplacée de côté). Un scénario où les deux réponses diffèrent va dans divergences.json, avec les deux réponses.
#
# Rien n'est lu ni modifié sur le PC : tout se passe dans un dossier temporaire neuf, effacé à la fin (aucune fenêtre,
# aucun réseau, aucun registre). L'heure et le hasard sont figés (Get-Date et Get-Random remplacés), le modèle du PC et
# les barrettes sont donnés par le scénario (Get-CimInstance remplacé).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File outils\exporter-juges-donnees.ps1      (quelques minutes)
#
# Les cas au hasard sont tirés avec la graine écrite dans chaque juge ($Graine).
param(
    [int]$Graine = 20261031,
    [int]$Scenarios = 300,   # scénarios au hasard (en plus des scénarios nommés)
    [int]$Cas = 300          # cas au hasard par juge sans fichier
)
$ErrorActionPreference = 'Stop'
$racine = Split-Path $PSScriptRoot -Parent
$AppDir = Join-Path $racine 'Mindcare\fichiers'
$Modules = Join-Path $AppDir 'modules'
$sortie = Join-Path $racine 'rust\mindcare-donnees\tests\juges'
New-Item -ItemType Directory -Force -Path $sortie | Out-Null
Get-ChildItem -LiteralPath $sortie -Filter 'scenarios-*.json' | Remove-Item -Force   # (les anciens découpages)
$travail = Join-Path ([IO.Path]::GetTempPath()) "mindcare-juges-donnees-$PID"
if (Test-Path -LiteralPath $travail) { Remove-Item -LiteralPath $travail -Recurse -Force }
New-Item -ItemType Directory -Force -Path $travail | Out-Null

$AppNom = 'Mindcare'; $AppId = 'mindcare'
$AppVersion = [regex]::Match([IO.File]::ReadAllText((Join-Path $AppDir 'Mindcare.ps1')), "\`$AppVersion = '([^']+)'").Groups[1].Value
$EnTest = $true
. (Join-Path $Modules 'enquete.ps1')
$d = Read-Enquetes
$Fr = [Globalization.CultureInfo]::GetCultureInfo('fr-FR')

function Get-Empreinte([string]$Fichier) {
    $h = [long]2166136261
    foreach ($b in [IO.File]::ReadAllBytes($Fichier)) { if ($b -ne 13) { $h = (($h -bxor $b) * 16777619) % 4294967296 } }
    '{0:x8}' -f $h
}
$catalogue = [ordered]@{ Fichier = 'Mindcare/fichiers/enquetes.json'; Empreinte = (Get-Empreinte $EnquetesFichier); Pannes = @($d.Pannes).Count }

# ---------------------------------------------------------------------------
# L'écriture des juges (comme exporter-juges.ps1 : chaque champ déjà en JSON)
# ---------------------------------------------------------------------------
function J($x) {
    if ($null -eq $x) { return 'null' }
    if ($x -is [psobject] -and $x.PSObject.BaseObject -is [array]) { $x = $x.PSObject.BaseObject }
    ConvertTo-Json -InputObject $x -Compress -Depth 30
}
function JListe($Elements) { '[' + ((@($Elements) | ForEach-Object { J $_ }) -join ',') + ']' }
function Write-Juge([string]$Nom, $Champs) {
    $morceaux = foreach ($k in $Champs.Keys) { (J $k) + ':' + $Champs[$k] }
    $json = ('{' + ($morceaux -join ',') + '}').Replace([string][char]0x2013, ('\u' + '2013')).Replace([string][char]0x2014, ('\u' + '2014'))
    [IO.File]::WriteAllText((Join-Path $sortie $Nom), $json, (New-Object Text.UTF8Encoding($false)))
    "$Nom : $([math]::Round((Get-Item -LiteralPath (Join-Path $sortie $Nom)).Length / 1KB)) Ko"
}
function Write-JugeCas([string]$Nom, [string]$Explication, $Liste) {
    Write-Juge $Nom ([ordered]@{ Catalogue = (J $catalogue); Graine = (J $Graine); Explication = (J $Explication); Cas = (JListe $Liste) })
}

# ---------------------------------------------------------------------------
# L'heure, le hasard et le PC figés
# ---------------------------------------------------------------------------
$script:Maintenant = [datetime]::new(2026, 10, 30, 14, 5, 0)
$script:Hasard = 0
function Get-Date { param([string]$Format) if ($Format) { $script:Maintenant.ToString($Format, [Globalization.CultureInfo]::InvariantCulture) } else { $script:Maintenant } }
function Get-Random { param($Maximum) $script:Hasard }
$script:Cim = $null   # (Get-InfosPC : { Erreur, Fabricant, Modele, Systeme })
function Get-CimInstance {
    param([string]$ClassName, $ErrorAction)
    if ($script:Cim.Erreur -and ($ClassName -eq $script:Cim.Erreur)) { throw "Classe $ClassName illisible" }
    if ($ClassName -eq 'Win32_ComputerSystem') { [pscustomobject]@{ Manufacturer = $script:Cim.Fabricant; Model = $script:Cim.Modele } }
    else { [pscustomobject]@{ Caption = $script:Cim.Systeme } }
}

# La partie enregistrée de Show-Verdict, tirée de contenu.ps1 lui-même (tout ce qui précède Clear-Contenu), puis ce
# que le Rust en garde (le rapport, la ligne « Avant d'acheter »)
$astContenu = [Management.Automation.Language.Parser]::ParseFile((Join-Path $Modules 'contenu.ps1'), [ref]$null, [ref]$null)
$fnVerdict = $astContenu.Find({ param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Show-Verdict' }, $true)
$corps = $fnVerdict.Body.Extent.Text
$coupe = $corps.IndexOf('Clear-Contenu')
if ($coupe -lt 0) { throw 'Show-Verdict : Clear-Contenu introuvable' }
$DefShowVerdict = 'function Show-Verdict($E, [switch]$Reaffiche) ' + $corps.Substring(0, $coupe) + @'
    $piece = $suspect -and $suspect.Piece
    if ($piece -and -not $Reaffiche) { Write-Log "Avant d'acheter : pièce $(if ($supposee) { 'supposée (pas testée)' } else { 'confirmée' })" }
    $script:Vu = [ordered]@{ Ecran = 'reponse'; Titre = [string]$titre }
}
'@
if ($corps -notmatch 'Write-Log "Avant d''acheter : pièce') { throw 'Show-Verdict : la ligne « Avant d''acheter » a changé, mettre à jour l''exporteur' }

# Les écrans remplacés (rien ne s'affiche) : seulement ce qui est montré est noté
$Remplacements = {
    function Set-BulleMascotte {}
    function Show-Accueil { $script:Vu = [ordered]@{ Ecran = 'accueil' } }
    function Show-Famille([string]$Id) { $script:Vu = [ordered]@{ Ecran = 'famille'; Famille = $Id } }
    function Show-EtapeEnquete($E) { $script:Vu = [ordered]@{ Ecran = 'etape'; Etape = [string]$E.Etape.Id } }
    function Wait-Journaux($E) { $false }
    function Test-JournauxLus { $true }
    function Get-Indices { $script:IndicesJour }
    function Get-Barrettes { @($script:Barrettes) }
    . ([scriptblock]::Create($DefShowVerdict))
}

# Read-JsonFile et Save-JsonFile tels que le Rust les fait (mindcare-donnees\src\fichiers.rs, avec json_sur) : la
# réponse voulue du constat 23
$RemplacementsVoulus = {
    function Test-ObjetJson([string]$Texte) {
        if ([string]::IsNullOrWhiteSpace($Texte) -or $Texte.Trim() -eq 'null') { return $false }
        try { $o = $Texte | ConvertFrom-Json; return ($o -is [Management.Automation.PSCustomObject]) } catch { return $false }
    }
    function Test-TexteVide([string]$Texte) { [string]::IsNullOrWhiteSpace($Texte) -or $Texte.Trim() -eq 'null' }
    # Pourquoi un texte n'est pas un objet JSON ($null : c'en est un)
    function Get-RaisonAbime([string]$Texte) {
        if (Test-TexteVide $Texte) { return 'fichier vide' }
        try { $o = $Texte | ConvertFrom-Json } catch { return "$_" }
        if ($o -is [Management.Automation.PSCustomObject]) { $null } else { 'pas un objet' }
    }
    # Le texte d'un fichier, 5 essais à 300 ms s'il est pris (Texte, Pris, Erreur)
    function Read-TexteEssais([string]$P) {
        $r = @{ Texte = $null; Pris = $false; Erreur = $null }
        for ($essai = 1; $essai -le 5; $essai++) {
            try { $r.Texte = [IO.File]::ReadAllText($P, [Text.Encoding]::UTF8); $r.Pris = $false; break }
            catch { $r.Pris = Test-ErreurFichierPris $_.Exception; $r.Erreur = $_; if (-not $r.Pris) { break }; if ($essai -lt 5) { Start-Sleep -Milliseconds 300 } }
        }
        $r
    }
    # json_sur : un nom libre <fichier>.abime-<date>, puis -2, -3... (deux mises de côté dans la même seconde ne
    # s'écrasent jamais)
    function Get-NomDeCote([string]$Fichier) {
        $base = "$Fichier.abime-$(Get-Date -Format yyyyMMdd-HHmmss)"
        $de = $base; $n = 1
        while ([IO.File]::Exists($de)) { $n++; $de = "$base-$n" }
        $de
    }
    # json_sur : un fichier abîmé est déplacé de côté (jamais effacé) ; le journal de json_sur le note
    function Move-DeCote([string]$Fichier, [string]$Ligne) {
        $de = Get-NomDeCote $Fichier
        [IO.File]::Move($Fichier, $de)
        Write-Log "$Ligne $(Split-Path $de -Leaf)"
    }
    function Read-JsonFile([string]$Path) {
        $script:FichiersPris.Remove($Path)
        $nom = Split-Path $Path -Leaf
        if ([IO.File]::Exists($Path)) {
            $r = Read-TexteEssais $Path
            if ($r.Pris) { Write-Log "Fichier pris par un autre programme, en lecture seule pour l'instant : $Path"; $script:FichiersPris[$Path] = $true }
            elseif ($null -eq $r.Texte) { Write-Log "Fichier illisible ($Path) : $($r.Erreur)" }
            else {
                $abime = Get-RaisonAbime $r.Texte
                if ($null -eq $abime) { return ($r.Texte | ConvertFrom-Json) }
                # json_sur::lire_avec : déplacé de côté, sa copie reprise et remise en place ; une copie abîmée est
                # déplacée de côté elle aussi
                Write-Log "Fichier illisible ($Path) : $abime"
                Move-DeCote $Path "$nom abîmé (illisible) : mis de côté dans"
                $bak = "$Path.bak"
                if ([IO.File]::Exists($bak)) {
                    $rb = Read-TexteEssais $bak
                    if ($rb.Pris) {
                        Write-Log "$nom : sa copie ($nom.bak) est prise par un autre programme"
                        Write-Log "Fichier pris par un autre programme, en lecture seule pour l'instant : $bak"
                        return $null
                    }
                    if ($null -eq $rb.Texte) { Write-Log "$nom : sa copie ($nom.bak) ne peut pas être lue ($($rb.Erreur))" }
                    elseif ($null -eq (Get-RaisonAbime $rb.Texte)) {
                        Write-Log "$nom abîmé : repris de sa copie ($nom.bak)"
                        if (-not [IO.File]::Exists($Path)) { Copy-Item -LiteralPath $bak -Destination $Path }
                        return ($rb.Texte | ConvertFrom-Json)
                    } else {
                        Write-Log "$nom : sa copie ($nom.bak) est illisible aussi"
                        Move-DeCote $bak "$nom.bak mis de côté dans"
                    }
                }
                Write-Log "$nom : aucune copie lisible, il repart de zéro"
                return $null
            }
        }
        # La sauvegarde d'un fichier absent, pris ou illisible : les lignes de Mindcare 0.15.1, puis json_sur::lire_copie
        # déplace de côté une copie abîmée (PowerShell la copiait)
        $p = "$Path.bak"
        if (-not [IO.File]::Exists($p)) { return $null }
        $r = Read-TexteEssais $p
        if ($r.Pris) { Write-Log "Fichier pris par un autre programme, en lecture seule pour l'instant : $p"; return $null }
        if ($null -eq $r.Texte) { Write-Log "Fichier illisible ($p) : $($r.Erreur)"; return $null }
        # (« repris » est noté avant de lire le JSON, comme Mindcare 0.15.1 : suivi de « illisible » s'il est abîmé)
        if (-not (Test-TexteVide $r.Texte)) { Write-Log "Fichier repris de sa sauvegarde : $p" }
        $abime = Get-RaisonAbime $r.Texte
        if ($null -eq $abime) { return ($r.Texte | ConvertFrom-Json) }
        Write-Log "Fichier illisible ($p) : $abime"
        Write-Log "$nom : sa copie ($nom.bak) est illisible aussi"
        Move-DeCote $p "$nom.bak mis de côté dans"
        $null
    }
    function Save-JsonFile([string]$Path, $Object) {
        if ($script:FichiersPris[$Path]) { throw "$(Split-Path $Path -Leaf) est en lecture seule pour l'instant (un autre programme le garde ouvert)" }
        $tmp = "$Path.tmp"
        [IO.File]::WriteAllText($tmp, (ConvertTo-Json -InputObject $Object -Depth 8), $Utf8NoBom)
        # json_sur::ecrire_avec : l'ancien ne va dans .bak que s'il contient un objet JSON (constat 23) ; vide, il est
        # simplement remplacé ; abîmé, il est d'abord copié de côté
        if ([IO.File]::Exists($Path)) {
            $t = [IO.File]::ReadAllText($Path, [Text.Encoding]::UTF8)
            if ($null -eq (Get-RaisonAbime $t)) { [IO.File]::Replace($tmp, $Path, "$Path.bak"); return }
            if (-not (Test-TexteVide $t)) { [IO.File]::Copy($Path, (Get-NomDeCote $Path)) }
            [IO.File]::Delete($Path)
        }
        [IO.File]::Move($tmp, $Path)
    }
}

# ---------------------------------------------------------------------------
# Ce qu'on compare : les textes avec le dossier d'essai remplacé par « <D> », les dates des fichiers mis de côté par
# « * », la raison d'un fichier illisible par « * » (le message de .NET), la liste « déjà su » rangée
# ---------------------------------------------------------------------------
function Get-Normal([string]$Texte) {
    $t = $Texte.Replace($script:Dir, '<D>')
    $t = [regex]::Replace($t, '\.abime-\d{8}-\d{6}', '.abime-*')
    $t = [regex]::Replace($t, '(?s)^(\S+ Fichier illisible \(.*?\)) : .*$', '$1 : *')
    $t = [regex]::Replace($t, '\(déjà su : ([^)]*)\)', { param($m) $l = [string[]]@($m.Groups[1].Value -split ', '); [Array]::Sort($l, [StringComparer]::OrdinalIgnoreCase); '(déjà su : ' + ($l -join ', ') + ')' })
    $t
}

function Get-Fichiers {
    $f = [ordered]@{}; $ab = New-Object 'Collections.Generic.List[object]'
    foreach ($x in @(Get-ChildItem -LiteralPath $script:Dir -File | Sort-Object Name)) {
        if ($x.Name -eq 'journal.txt' -or $x.Name -like '*.tmp') { continue }
        $t = [IO.File]::ReadAllText($x.FullName, [Text.Encoding]::UTF8)
        if ($x.Name -match '^(.*)\.abime-\d{8}-\d{6}(-\d+)?$') { $ab.Add([ordered]@{ De = $Matches[1]; Texte = $t }) } else { $f[$x.Name] = $t }
    }
    [ordered]@{ Fichiers = $f; Abimes = @($ab.ToArray() | Sort-Object { $_.De }, { $_.Texte }) }
}

# Les indices d'un scénario (une liste { Id, Vu, Texte, TexteOk, Affiche, Partiel }) en table, comme Get-Indices
function ConvertTo-TableIndices($Liste) {
    $h = @{}
    foreach ($i in @($Liste)) { if ($i) { $h[[string]$i.Id] = [pscustomobject]@{ Vu = [bool]$i.Vu; Texte = [string]$i.Texte; TexteOk = [string]$i.TexteOk; Affiche = [bool]$i.Affiche; Partiel = [bool]$i.Partiel } } }
    $h
}

function Get-EtatSession {
    $pr = $null
    if ($null -ne $script:PreRempli) {
        $cles = [string[]]@($script:PreRempli.Keys); [Array]::Sort($cles, [StringComparer]::OrdinalIgnoreCase)
        $pr = @(foreach ($k in $cles) { , @($k, [int]$script:PreRempli[$k]) })
    }
    [ordered]@{ Vu = $script:Vu; DiagId = [string]$script:DiagId; Choix = [int[]]@($script:ChoixFaits | ForEach-Object { [int]$_ }); PreRempli = $pr; Reprise = [bool](Test-Reprise) }
}

# ---------------------------------------------------------------------------
# Un scénario : des fichiers de départ, des opérations, et ce qu'on obtient. $Choisir (le hasard) décide de l'opération
# suivante d'après l'état (la première fois) ; les opérations sont notées et rejouées telles quelles (réponse voulue).
# ---------------------------------------------------------------------------
function Invoke-Scenario($Sc, [bool]$Voulu, [scriptblock]$Choisir) {
    $script:Dir = Join-Path $travail ('s' + [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Force -Path $script:Dir | Out-Null
    foreach ($k in $Sc.Fichiers.Keys) { [IO.File]::WriteAllBytes((Join-Path $script:Dir $k), [Convert]::FromBase64String($Sc.Fichiers[$k])) }
    if ($Sc.JournalGenere) {
        $sb = New-Object Text.StringBuilder
        for ($i = 0; $i -lt $Sc.JournalGenere.Lignes; $i++) { [void]$sb.Append("ancienne ligne $i ").Append('x' * ($i % $Sc.JournalGenere.Largeur)).Append($(if ($i % 7 -eq 3) { "`n" } else { "`r`n" })) }
        [IO.File]::WriteAllText((Join-Path $script:Dir 'journal.txt'), $sb.ToString(), (New-Object Text.UTF8Encoding($false)))
    }
    $script:Verrous = @{}
    foreach ($n in @($Sc.Pris)) { if ($n) { $script:Verrous[$n] = [IO.File]::Open((Join-Path $script:Dir $n), 'Open', 'Read', 'None') } }
    $script:Maintenant = [datetime]::ParseExact($Sc.Maintenant, 's', [Globalization.CultureInfo]::InvariantCulture)
    $script:Hasard = 0; $script:IndicesJour = @{}; $script:Barrettes = @($Sc.Barrettes); $script:Vu = $null
    $env:MINDCARE_DONNEES = $script:Dir
    $ops = New-Object 'Collections.Generic.List[object]'; $res = New-Object 'Collections.Generic.List[object]'
    $faites = if ($Choisir) { $null } else { @($Sc.Ops) }
    $n = 0
    try {
        while ($true) {
            $op = if ($Choisir) { & $Choisir $n } else { if ($n -lt $faites.Count) { $faites[$n] } else { $null } }
            if ($null -eq $op) { break }
            $n++; $ops.Add($op)
            $r = $null
            switch ($op.Op) {
                'Demarrer' {
                    $script:EtatEcrit = $null; $script:InfosPC = [string]$Sc.InfosPc
                    $script:ResumeDe = $null; $script:Reprise = $null; $script:PreRempli = $null; $script:IndicesGardes = $null
                    if ($Voulu) {
                        # (la lecture des réglages au chargement passe déjà par la lecture voulue)
                        $texteDonnees = [IO.File]::ReadAllText((Join-Path $Modules 'donnees.ps1'), [Text.Encoding]::UTF8)
                        $i = $texteDonnees.IndexOf('$script:Settings = @{}')
                        if ($i -lt 0) { throw 'donnees.ps1 : la lecture des réglages a changé' }
                        . ([scriptblock]::Create($texteDonnees.Substring(0, $i) + "`n. `$RemplacementsVoulus`n" + $texteDonnees.Substring($i)))
                    } else { . (Join-Path $Modules 'donnees.ps1') }
                    . (Join-Path $Modules 'systeme.ps1')
                    . (Join-Path $Modules 'historique.ps1')
                    . (Join-Path $Modules 'contenu.ps1')
                    . $Remplacements
                    if ($Voulu) { . $RemplacementsVoulus }
                    $script:Donnees = $d
                }
                'LireHistorique' { Read-Historique }
                'Prendre' { $script:Verrous[$op.Fichier] = [IO.File]::Open((Join-Path $script:Dir $op.Fichier), 'Open', 'Read', 'None') }
                'Liberer' { if ($script:Verrous[$op.Fichier]) { $script:Verrous[$op.Fichier].Dispose(); $script:Verrous.Remove($op.Fichier) } }
                'Ecrire' { [IO.File]::WriteAllText((Join-Path $script:Dir $op.Fichier), $op.Texte, (New-Object Text.UTF8Encoding($false))) }
                'Effacer' { [IO.File]::Delete((Join-Path $script:Dir $op.Fichier)) }
                'Heure' { $script:Maintenant = [datetime]::ParseExact($op.Maintenant, 's', [Globalization.CultureInfo]::InvariantCulture) }
                'IndicesDuJour' { $script:IndicesJour = ConvertTo-TableIndices $op.Indices }
                'Reglage' { $r = Get-Setting $op.Nom ($op.Defaut | ConvertFrom-Json) }
                'ChangerReglage' { Set-Setting $op.Nom ($op.Valeur | ConvertFrom-Json) }
                'LireJson' { $r = Read-JsonFile (Join-Path $script:Dir $op.Fichier) }
                'Relire' { $f = Get-FichierRelu (Join-Path $script:Dir $op.Fichier); $r = [ordered]@{ Libre = [bool]$f.Libre; Relu = [bool]$f.Relu; Contenu = $f.Contenu } }
                'Sauver' {
                    $ind = if ($null -ne $op.Indices) { ConvertTo-TableIndices $op.Indices } else { $null }
                    $rep = @(foreach ($x in @($op.Reponses)) { if ($x) { [ordered]@{ Etape = [string]$x[0]; Reponse = [int]$x[1] } } })
                    Save-Diagnostic $op.Id $op.Panne ([int[]]@($op.Choix)) ([bool]$op.Fini) $op.Suspect ([string]$op.Titre) $rep $ind -Reprise:([bool]$op.Reprise)
                }
                'Carte' { $c = Get-EtatCarte; $r = [ordered]@{ Titre = [string]$c.Titre; Scores = @($c.Scores); Lignes = @($c.Lignes | ForEach-Object { [ordered]@{ Texte = [string]$_.Texte; Etat = [string]$_.Etat } }) } }
                'EcrireEtat' { Write-EtatMindset }
                'Commencer' {
                    $script:Hasard = [int]$op.Hasard
                    $pr = $null
                    if ($null -ne $op.PreRempli) { $pr = @{}; foreach ($x in @($op.PreRempli)) { if ($x) { $pr[[string]$x[0]] = [int]$x[1] } } }
                    Start-Diagnostic $op.Panne $pr
                    $r = Get-EtatSession
                }
                'Ouvrir' { Open-Diagnostic $op.Id; $r = Get-EtatSession }
                'Repondre' { Add-Choix ([int]$op.N); $r = Get-EtatSession }
                'Retour' { Undo-Choix; $r = Get-EtatSession }
                # (aucune réponse montrée : aucun rapport)
                'Rapport' { if ($script:ResumeDe) { $r = Get-ResumeEnCours } }
                'EnregistrerRapport' { if ($script:ResumeDe) { $r = Split-Path (Save-Resume (Get-ResumeEnCours)) -Leaf } }
                default { throw "opération inconnue : $($op.Op)" }
            }
            $res.Add((J $r))
        }
        # (les fichiers encore pris sont libérés pour lire ce qu'ils contiennent)
        foreach ($v in @($script:Verrous.Values)) { $v.Dispose() }
        $script:Verrous = @{}
        $fi = Get-Fichiers
        # (une ligne du journal qui ne commence pas par une date continue la précédente : un message sur plusieurs lignes)
        $lignes = New-Object 'Collections.Generic.List[string]'
        if ([IO.File]::Exists($LogFile)) { foreach ($l in [IO.File]::ReadAllLines($LogFile, [Text.Encoding]::UTF8)) { if ($lignes.Count -and $l -notmatch '^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d ') { $lignes[$lignes.Count - 1] += "`n" + $l } else { $lignes.Add($l) } } }
        $journal = @(foreach ($l in $lignes) { Get-Normal $l })
        $pris = @($script:FichiersPris.Keys | Where-Object { $script:FichiersPris[$_] } | ForEach-Object { Split-Path $_ -Leaf } | Sort-Object)
        $resultat = '{"Resultats":[' + (($res.ToArray() | ForEach-Object { Get-Normal $_ }) -join ',') + ']' +
            ',"Historique":' + (J ([object[]]@($script:Historique))) +
            ',"Reglages":' + (J $script:Settings) +
            ',"Fichiers":' + (Get-Normal (J $fi.Fichiers)) + ',"Abimes":' + (Get-Normal (J ([object[]]$fi.Abimes))) +
            ',"Journal":' + (J ([string[]]$journal)) + ',"Pris":' + (J ([string[]]$pris)) + '}'
        [ordered]@{ Ops = $ops.ToArray(); Resultat = $resultat }
    } finally {
        foreach ($v in @($script:Verrous.Values)) { $v.Dispose() }
        $script:Verrous = @{}
        Remove-Item -LiteralPath $script:Dir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# ---------------------------------------------------------------------------
# Des contenus de fichiers et des enquêtes au hasard (avec le vrai moteur)
# ---------------------------------------------------------------------------
$Utf8 = New-Object Text.UTF8Encoding($false)
function B64([string]$Texte) { [Convert]::ToBase64String($Utf8.GetBytes($Texte)) }
function B64Bom([string]$Texte) { [Convert]::ToBase64String([byte[]](@(0xEF, 0xBB, 0xBF) + $Utf8.GetBytes($Texte))) }
function B64Utf16([string]$Texte) { [Convert]::ToBase64String([byte[]](@(0xFF, 0xFE) + [Text.Encoding]::Unicode.GetBytes($Texte))) }
$pannes = @($d.Pannes)
$script:H = New-Object Random $Graine

# Les fonctions sans écran des vrais modules, tirées de leur fichier (rien d'autre n'est exécuté) : pour préparer
# les scénarios et pour les cas sans fichier. Dans un scénario, celles des modules chargés passent devant.
function Import-Fonctions([string]$Fichier, [string[]]$Noms) {
    $ast = [Management.Automation.Language.Parser]::ParseFile((Join-Path $Modules $Fichier), [ref]$null, [ref]$null)
    foreach ($nom in $Noms) {
        $f = $ast.Find({ param($x) $x -is [Management.Automation.Language.FunctionDefinitionAst] -and $x.Name -eq $nom }.GetNewClosure(), $true)
        if (-not $f) { throw "$Fichier : $nom introuvable" }
        $f.Extent.Text
    }
}
$textes = @(Import-Fonctions 'historique.ps1' @('Get-Paires', 'ConvertTo-Indices', 'Format-Date', 'Get-DiagnosticInfo', 'Get-InfosPC', 'Get-ResumeTexte', 'Get-Court', 'Get-PhraseCarte', 'Get-EtatCarte')) +
          @(Import-Fonctions 'donnees.ps1' @('Get-DonneesMindset')) +
          @(Import-Fonctions 'contenu.ps1' @('New-DiagId', 'Test-Reprise', 'Get-PairesEnAttente')) +
          @(Import-Fonctions 'mindset.ps1' @('Get-CheminNormal', 'Test-SuitePrise', 'Read-Suite', 'Test-GereeParMindset'))
. ([scriptblock]::Create(($textes -join "`n")))

function New-Moment($h) { [datetime]::new(2026, 1, 1).AddMinutes($h.Next(0, 525600)).AddSeconds($h.Next(60)) }
function New-IdHasard($h) { '{0:yyyyMMdd-HHmmss}-{1:x4}' -f (New-Moment $h), $h.Next(65536) }

# Une enquête au hasard : une panne, des réponses jusqu'à un arrêt au hasard (ou la fin)
function New-Parcours($h, $Panne = $null, [int]$Max = 14) {
    $p = if ($Panne) { $Panne } else { $pannes[$h.Next($pannes.Count)] }
    $choix = New-Object 'Collections.Generic.List[int]'
    $stop = $h.Next(1, $Max)
    for ($t = 0; $t -lt 40; $t++) {
        $e = Get-Enquete $d $p $choix.ToArray()
        if (-not $e.Etape -or $choix.Count -ge $stop) { break }
        $nb = @($e.Etape.Reponses).Count
        $choix.Add($(if ($h.Next(10) -eq 0) { -1 } else { $h.Next($nb) }))
    }
    $e = Get-Enquete $d $p $choix.ToArray()
    @{ Panne = $p; Choix = [int[]]@($e.Choix | ForEach-Object { [int]$_ }); E = $e }
}

function New-IndicesHasard($h) {
    $l = New-Object 'Collections.Generic.List[object]'
    foreach ($id in $IndicesConnus) {
        if ($h.Next(3) -ne 0) { continue }
        $l.Add([ordered]@{ Id = $(if ($h.Next(8) -eq 0) { $id.ToUpperInvariant() } else { $id }); Vu = ($h.Next(2) -eq 0); Texte = "Vu : $id"; TexteOk = $(if ($h.Next(4) -eq 0) { '' } else { "Rien : $id" }); Affiche = ($h.Next(4) -ne 0); Partiel = ($h.Next(3) -eq 0) })
    }
    if ($h.Next(3) -eq 0) { $l.Add([ordered]@{ Id = 'journal'; Vu = $false; Texte = ''; TexteOk = 'le journal de Windows ne remonte qu''au 12 octobre'; Affiche = $false; Partiel = $true }) }
    , $l.ToArray()
}

# Une enquête gardée, au format v2 (avec ou sans Reponses) ou v1
function New-Diag($h, [string]$Format) {
    $x = New-Parcours $h
    $e = $x.E
    $fini = -not $e.Etape
    $debut = (New-Moment $h).ToString('s')
    $paires = New-Object 'Collections.Generic.List[object]'
    foreach ($pr in @(Get-Paires $e)) { if ($pr) { $paires.Add([ordered]@{ Etape = $pr.Etape; Reponse = $pr.Reponse }) } }
    # (des réponses perturbées : une question disparue, un numéro hors de la liste, « Je ne sais pas »)
    if ($paires.Count -and $h.Next(4) -eq 0) {
        $k = $h.Next($paires.Count)
        switch ($h.Next(3)) { 0 { $paires[$k].Etape = 'question-disparue' } 1 { $paires[$k].Reponse = 99 } 2 { $paires[$k].Reponse = -1 } }
    }
    # (des réponses pour plus tard : une enquête plus longue sur la même panne)
    if ($h.Next(5) -eq 0) {
        $plus = New-Parcours $h $x.Panne 20
        foreach ($pr in @(Get-Paires $plus.E)) { if ($pr -and -not @($paires | Where-Object { $_.Etape -eq $pr.Etape })) { $paires.Add([ordered]@{ Etape = $pr.Etape; Reponse = $pr.Reponse }) } }
    }
    $o = [ordered]@{ Id = (New-IdHasard $h); Panne = [string]$x.Panne.Id; Choix = [int[]]$x.Choix }
    if ($Format -eq 'v1') {
        $o.Debut = $debut; $o.Maj = $debut; $o.Fini = $fini; $o.Causes = @('memoire')
    } else {
        if ($Format -eq 'v2') { $o.Reponses = $paires.ToArray() }
        $o.Debut = $debut; $o.Maj = $debut; $o.Fini = $fini
        $o.Suspect = [string]$e.Verdict.Suspect
        $o.Titre = $(if ($fini) { Get-VerdictTitre $d $x.Panne $e.Verdict } else { '' })
        if ($Format -eq 'v2' -and $fini -and $h.Next(2) -eq 0) {
            $ind = [ordered]@{}; foreach ($i in (New-IndicesHasard $h)) { $ind[$i.Id] = [ordered]@{ Vu = $i.Vu; Texte = $i.Texte; TexteOk = $i.TexteOk; Affiche = $i.Affiche; Partiel = $i.Partiel } }
            $o.Indices = $ind
        }
    }
    if ($h.Next(15) -eq 0) { $o.Champ = 'gardé tel quel' }   # (un champ inconnu : réécrit à l'identique)
    $o
}

function New-TexteHistorique($h, [string]$Sorte) {
    switch ($Sorte) {
        'vide' { return '' }
        'espaces' { return " `r`n  " }
        'null' { return 'null' }
        'tableau' { return $(if ($h.Next(2)) { '[1,2]' } else { '42' }) }
        'sansliste' { return '{"Version":2}' }
    }
    $n = $h.Next(0, 7)
    $liste = New-Object 'Collections.Generic.List[object]'
    for ($k = 0; $k -lt $n; $k++) {
        $f = switch ($Sorte) { 'v1' { 'v1' } 'vieux2' { 'v2a' } default { if ($h.Next(4) -eq 0) { 'v2a' } else { 'v2' } } }
        $liste.Add((New-Diag $h $f))
    }
    if ($Sorte -eq 'melange') {
        $liste.Add([ordered]@{ Id = ''; Panne = 'redemarre'; Choix = @(1) })
        $liste.Add([ordered]@{ Id = 'sans-panne'; Choix = @(1) })
        $liste.Add('pas un objet')
        $liste.Add([ordered]@{ Id = (New-IdHasard $h); Panne = 'panne-disparue'; Choix = @(0, 1); Fini = $true; Titre = 'Une ancienne réponse' })
    }
    $t = ConvertTo-Json -InputObject ([ordered]@{ Version = $(if ($Sorte -eq 'v1') { 1 } else { 2 }); Diagnostics = $liste.ToArray() }) -Depth 10
    if ($Sorte -eq 'coupe') { $t = $t.Substring(0, $h.Next(1, $t.Length - 1)) }
    if ($Sorte -eq 'abime') { $t = if ($h.Next(2)) { $t.Insert(1, '#') } else { $t.Substring(0, $t.LastIndexOf('}')) + ']' } }
    $t
}

function New-TexteReglages($h, [string]$Sorte) {
    switch ($Sorte) {
        'vide' { return '' }
        'null' { return 'null' }
        'tableau' { return '[1,2]' }
        'coupe' { return '{"Theme":"neon","Volume":' }
    }
    $o = [ordered]@{}
    if ($h.Next(4)) { $o.Theme = @('neon', 'arcade', 'rubis', 'pack:essai')[$h.Next(4)] }
    if ($h.Next(2)) { $o.Volume = $h.Next(0, 101) }
    if ($h.Next(2)) { $o.Dictee = ($h.Next(2) -eq 0) }
    if ($h.Next(3) -eq 0) { $o.Fenetre = [ordered]@{ X = $h.Next(800); Y = $h.Next(600) } }
    if ($h.Next(4) -eq 0) { $o.Recents = @('a', 'b', 'c') }
    ConvertTo-Json -InputObject $o -Depth 5
}

# (Get-Setting rend un tableau d'un seul élément déroulé, comme toute fonction PowerShell : pas de tableau de 0 ou 1
# élément ici, voir le rapport)
$ValeursReglages = @('"neon"', '"arcade"', '3', '0', 'true', 'false', 'null', '""', '{"X":10,"Y":20}', '"accentué é"', '2.5')
$NomsReglages = @('Theme', 'theme', 'Volume', 'Dictee', 'Fenetre', 'Inconnu', 'Recents')

function New-Op([string]$Op, $Champs = $null) { $o = [ordered]@{ Op = $Op }; if ($Champs) { foreach ($k in $Champs.Keys) { $o[$k] = $Champs[$k] } }; $o }

# Une opération Sauver au hasard (une nouvelle enquête, ou une gardée qui change ou pas)
function New-OpSauver($h) {
    $existant = @($script:Historique)
    if ($existant.Count -and $h.Next(2) -eq 0) {
        $a = $existant[$h.Next($existant.Count)]
        $choix = [int[]]@($a.Choix | ForEach-Object { [int]$_ })
        $rep = New-Object 'Collections.Generic.List[object]'
        foreach ($r in @($a.Reponses)) { if ($r) { $rep.Add(@([string]$r.Etape, [int]$r.Reponse)) } }
        switch ($h.Next(4)) {
            0 { }   # (la même : rien ne bouge)
            1 { $choix = [int[]](@($choix) + 0); $rep.Add(@('nouvelle', 0)) }
            2 { $choix = [int[]]@() }
            3 { $rep.Add(@('autre', 1)) }
        }
        $fini = if ($h.Next(3) -eq 0) { -not [bool]$a.Fini } else { [bool]$a.Fini }
        $titre = if ($h.Next(4) -eq 0) { ([string]$a.Titre).ToUpperInvariant() } else { [string]$a.Titre }
        return (New-Op 'Sauver' ([ordered]@{ Id = [string]$a.Id; Panne = [string]$a.Panne; Choix = $choix; Fini = $fini; Suspect = [string]$a.Suspect; Titre = $titre; Reponses = $rep.ToArray(); Indices = $null; Reprise = ($h.Next(3) -eq 0) }))
    }
    $x = New-Parcours $h
    $fini = -not $x.E.Etape
    $rep = New-Object 'Collections.Generic.List[object]'
    foreach ($pr in @(Get-Paires $x.E)) { if ($pr) { $rep.Add(@([string]$pr.Etape, [int]$pr.Reponse)) } }
    $ind = if ($fini -and $h.Next(2) -eq 0) { New-IndicesHasard $h } else { $null }
    New-Op 'Sauver' ([ordered]@{ Id = (New-IdHasard $h); Panne = [string]$x.Panne.Id; Choix = $x.Choix; Fini = $fini; Suspect = [string]$x.E.Verdict.Suspect; Titre = $(if ($fini) { Get-VerdictTitre $d $x.Panne $x.E.Verdict } else { '' }); Reponses = $rep.ToArray(); Indices = $ind; Reprise = ($h.Next(4) -eq 0) })
}

# L'opération suivante d'un scénario au hasard (appelée dans Invoke-Scenario : elle voit l'état des modules)
$script:Plan = $null
$ChoisirHasard = {
    param([int]$N)
    $h = $script:H; $pl = $script:Plan
    if ($N -eq 0) { return (New-Op 'Demarrer') }
    if ($N -eq 1) { return (New-Op 'LireHistorique') }
    if ($N -ge $pl.Longueur) { return $null }
    $vu = if ($script:Vu) { $script:Vu.Ecran } else { '' }
    $t = $h.Next(100)
    if ($script:Verrous.Count -and $t -lt 12) { return (New-Op 'Liberer' @{ Fichier = @($script:Verrous.Keys)[0] }) }
    if ($t -lt 14 -and -not $pl.Heures) { $pl.Heures = $true; return (New-Op 'Heure' @{ Maintenant = $pl.Heure.AddMinutes($h.Next(1, 3000)).ToString('s') }) }
    if ($vu -eq 'etape' -and $t -lt 60) {
        $e = Get-EnqueteEnCours
        $nb = @($e.Etape.Reponses).Count
        if ($h.Next(8) -eq 0) { return (New-Op 'Retour') }
        return (New-Op 'Repondre' @{ N = $(switch ($h.Next(12)) { 0 { -1 } 1 { 99 } default { $h.Next($nb) } }) })
    }
    if ($vu -eq 'reponse' -and $t -lt 40) {
        switch ($h.Next(4)) { 0 { return (New-Op 'Rapport') } 1 { return (New-Op 'EnregistrerRapport') } 2 { return (New-Op 'Retour') } default { return (New-Op 'Carte') } }
    }
    switch ($h.Next(13)) {
        0 { return (New-OpSauver $h) }
        1 { return (New-OpSauver $h) }
        2 { return (New-Op 'ChangerReglage' @{ Nom = $NomsReglages[$h.Next($NomsReglages.Count)]; Valeur = $ValeursReglages[$h.Next($ValeursReglages.Count)] }) }
        3 { return (New-Op 'Reglage' @{ Nom = $NomsReglages[$h.Next($NomsReglages.Count)]; Defaut = $ValeursReglages[$h.Next($ValeursReglages.Count)] }) }
        4 { return (New-Op 'Carte') }
        5 { return (New-Op 'EcrireEtat') }
        6 {
            $p = $pannes[$h.Next($pannes.Count)]
            $pr = $null
            if ($h.Next(2) -eq 0) {
                $x = New-Parcours $h $p 8
                $l = New-Object 'Collections.Generic.List[object]'
                foreach ($q in @(Get-Paires $x.E)) { if ($q -and $h.Next(3)) { $l.Add(@([string]$q.Etape, [int]$q.Reponse)) } }
                if ($h.Next(4) -eq 0) { $l.Add(@('danger', 0)) }
                $pr = $l.ToArray()
            }
            return (New-Op 'Commencer' @{ Panne = [string]$p.Id; PreRempli = $pr; Hasard = $h.Next(65536) })
        }
        7 {
            $l = @($script:Historique)
            $id = if ($l.Count -and $h.Next(8)) { [string]$l[$h.Next($l.Count)].Id } else { 'enquete-inconnue' }
            if ($h.Next(10) -eq 0) { $id = $id.ToUpperInvariant() }
            return (New-Op 'Ouvrir' @{ Id = $id })
        }
        8 { return (New-Op 'IndicesDuJour' @{ Indices = (New-IndicesHasard $h) }) }
        9 {
            # (un fichier vidé par un autre programme entre la lecture et l'écriture : une seule fois)
            if ($pl.Vide) { return (New-Op 'Carte') }
            $pl.Vide = $true
            $f = @('diagnostics.json', 'reglages.json')[$h.Next(2)]
            if ($script:Verrous[$f]) { return (New-Op 'Carte') }
            return (New-Op 'Ecrire' @{ Fichier = $f; Texte = @('', ' ', 'null')[$h.Next(3)] })
        }
        10 {
            if ($pl.Autre) { return (New-Op 'Carte') }
            $pl.Autre = $true
            return (New-Op 'LireJson' @{ Fichier = 'autre.json' })
        }
        default {
            if (@($script:Historique).Count) { return (New-Op 'Ouvrir' @{ Id = [string]@($script:Historique)[0].Id }) }
            return (New-OpSauver $h)
        }
    }
}

# (le plus souvent des fichiers sains : un fichier vide ou abîmé rend le scénario divergent, constat 23)
$SortesHistorique = @('absent', 'absent', 'v2', 'v2', 'v2', 'v2', 'v2', 'v2', 'v2', 'v2', 'v1', 'v1', 'vieux2', 'vieux2', 'melange', 'sansliste', 'bom', 'utf16', 'vide', 'espaces', 'null', 'coupe', 'abime', 'tableau')
$SortesReglages = @('absent', 'absent', 'normal', 'normal', 'normal', 'normal', 'normal', 'normal', 'normal', 'normal', 'vide', 'null', 'coupe', 'tableau')

function New-ScenarioHasard([int]$K) {
    $h = $script:H
    $f = [ordered]@{}
    $sorte = $SortesHistorique[$h.Next($SortesHistorique.Count)]
    if ($sorte -ne 'absent') {
        $base = if ($sorte -in 'bom', 'utf16') { 'v2' } else { $sorte }
        $t = New-TexteHistorique $h $base
        $f['diagnostics.json'] = switch ($sorte) { 'bom' { B64Bom $t } 'utf16' { B64Utf16 $t } default { B64 $t } }
    }
    $bak = $SortesHistorique[$h.Next($SortesHistorique.Count)]
    if ($bak -in 'bom', 'utf16') { $bak = 'v2' }
    if ($bak -ne 'absent' -and $h.Next(3)) { $f['diagnostics.json.bak'] = B64 (New-TexteHistorique $h $bak) } else { $bak = 'absent' }
    $sr = $SortesReglages[$h.Next($SortesReglages.Count)]
    if ($sr -ne 'absent') { $f['reglages.json'] = B64 (New-TexteReglages $h $sr) }
    if ($h.Next(3) -eq 0) { $f['reglages.json.bak'] = B64 (New-TexteReglages $h 'normal') }
    if ($h.Next(4) -eq 0) { $f['autre.json'] = B64 (@('{"a":1}', '', '{"a":', '[]', '{"b":{"c":[1,2]}}')[$h.Next(5)]) }
    $pris = @()
    if ($f.Contains('diagnostics.json') -and $h.Next(14) -eq 0) { $pris += 'diagnostics.json' }
    if ($f.Contains('reglages.json') -and $h.Next(18) -eq 0) { $pris += 'reglages.json' }
    $heure = New-Moment $h
    $script:Plan = @{ Longueur = $h.Next(4, 14); Heure = $heure; Heures = $false; Vide = $false; Autre = $false }
    [ordered]@{
        Nom = "hasard $K ($sorte, copie $bak, réglages $sr$(if ($pris) { ', pris : ' + ($pris -join ' ') }))"
        Fichiers = $f; Pris = [string[]]$pris; Maintenant = $heure.ToString('s'); InfosPc = 'Lenovo IdeaPad 5, Microsoft Windows 11 Famille'
        Barrettes = [string[]]@(if ($h.Next(2)) { '8 Go Kingston KF3200 (DIMM1)'; '8 Go Kingston KF3200 (DIMM2)' })
        JournalGenere = $null
    }
}

# ---------------------------------------------------------------------------
# Les scénarios nommés : chaque règle importante au moins une fois
# ---------------------------------------------------------------------------
function TexteHisto($Liste, [int]$Version = 2) { ConvertTo-Json -InputObject ([ordered]@{ Version = $Version; Diagnostics = [object[]]@($Liste) }) -Depth 10 }
function Find-Diag([int]$Graine, [scriptblock]$Condition, [string]$Format = 'v2') {
    $h = New-Object Random $Graine
    for ($k = 0; $k -lt 3000; $k++) { $x = New-Diag $h $Format; if (& $Condition $x) { return $x } }
    throw "aucune enquête trouvée pour $Condition"
}
function Op([string]$Op, $Champs = $null) { New-Op $Op $Champs }
function New-Nomme([string]$Nom, $Fichiers, $Ops, [string[]]$Pris = @(), [string]$Maintenant = '2026-10-30T14:05:00', $JournalGenere = $null) {
    $f = [ordered]@{}; foreach ($k in $Fichiers.Keys) { $f[$k] = $Fichiers[$k] }
    [ordered]@{ Nom = $Nom; Fichiers = $f; Pris = $Pris; Maintenant = $Maintenant; InfosPc = 'Dell Inspiron 15, Microsoft Windows 11 Famille'; Barrettes = [string[]]@('16 Go Crucial CT16G4 (ChannelA-DIMM0)'); JournalGenere = $JournalGenere; Ops = @($Ops) }
}
$sauverNeuf = { param($Id, $Choix = @(0, 1)) Op 'Sauver' ([ordered]@{ Id = $Id; Panne = 'redemarre'; Choix = [int[]]$Choix; Fini = $false; Suspect = ''; Titre = ''; Reponses = @(); Indices = $null; Reprise = $false }) }
$debut = @((Op 'Demarrer'), (Op 'LireHistorique'))

$dFini = Find-Diag 101 { param($x) $x.Fini -and $x.Reponses.Count -ge 2 -and $x.Indices }
$dEnCours = Find-Diag 102 { param($x) -not $x.Fini -and $x.Reponses.Count -ge 3 -and -not @($x.Reponses | Where-Object { $_.Etape -eq 'question-disparue' -or $_.Reponse -eq 99 }) }
$dV1 = Find-Diag 103 { param($x) $x.Choix.Count -ge 2 } 'v1'
$dVieux = Find-Diag 104 { param($x) $x.Choix.Count -ge 2 } 'v2a'
$dDisparue = Find-Diag 105 { param($x) -not $x.Fini -and $x.Reponses.Count -ge 2 }
$dDisparue.Reponses[0].Etape = 'question-disparue'
$dHors = Find-Diag 106 { param($x) -not $x.Fini -and $x.Reponses.Count -ge 3 }
$dHors.Reponses[0].Reponse = 99
$dHors2 = Find-Diag 107 { param($x) -not $x.Fini -and $x.Reponses.Count -ge 3 }
$dHors2.Reponses[1].Reponse = 42
$histo3 = TexteHisto @($dEnCours, $dFini, $dVieux)
$histoBak = TexteHisto @($dFini)
$urgent = @($d.Suspects.PSObject.Properties | Where-Object { $_.Value.Urgent })[0]
$panneUrgent = @($d.Pannes | Where-Object { @($_.Suspects) -contains $urgent.Name })[0]
function DiagCarte($Champs) { $o = [ordered]@{ Id = '20261030-140500-0001'; Panne = 'redemarre'; Choix = @(1); Debut = '2026-10-30T09:07:00'; Maj = '2026-10-30T09:07:00'; Fini = $true; Suspect = ''; Titre = '' }; foreach ($k in $Champs.Keys) { $o[$k] = $Champs[$k] }; $o }
$rien = Get-VerdictTitre $d $null ([pscustomobject]@{ Type = 'rien' })
$long = 'Le disque dur fatigue et ralentit tout : sauvegarde tes fichiers importants dès aujourd''hui, puis fais-le remplacer'
$douze = @(for ($k = 0; $k -lt 12; $k++) { $x = New-Diag (New-Object Random (300 + $k)) 'v2'; $x.Id = "20261001-0000$('{0:00}' -f $k)-000$($k % 10)"; $x })

$Nommes = @(
    (New-Nomme 'v2 : lu, carte, reprise de l''enquête en cours, réponses' @{ 'diagnostics.json' = (B64 $histo3) } ($debut + @((Op 'Carte'), (Op 'Ouvrir' @{ Id = $dEnCours.Id }), (Op 'Repondre' @{ N = 0 }), (Op 'Retour'), (Op 'Repondre' @{ N = 1 }), (Op 'EcrireEtat'), (Op 'EcrireEtat'), (Op 'Carte'))))
    (New-Nomme 'v2 : enquête finie revue avec les indices gardés, rapport' @{ 'diagnostics.json' = (B64 $histo3) } ($debut + @((Op 'IndicesDuJour' @{ Indices = @([ordered]@{ Id = 'journal'; Vu = $false; Texte = ''; TexteOk = 'le journal ne remonte qu''au 3 octobre'; Affiche = $false; Partiel = $true }) }), (Op 'Ouvrir' @{ Id = $dFini.Id }), (Op 'Rapport'), (Op 'EnregistrerRapport'), (Op 'EnregistrerRapport'), (Op 'EnregistrerRapport'), (Op 'Retour'), (Op 'Repondre' @{ N = 0 }))))
    (New-Nomme 'ancien format v1 (Choix et Causes) : lu, carte, rejoué par la place' @{ 'diagnostics.json' = (B64 (TexteHisto @($dV1) 1)) } ($debut + @((Op 'Carte'), (Op 'Ouvrir' @{ Id = $dV1.Id }), (Op 'Repondre' @{ N = 0 }), (Op 'Carte'))))
    (New-Nomme 'v2 sans Reponses : rejouée par la place' @{ 'diagnostics.json' = (B64 (TexteHisto @($dVieux))) } ($debut + @((Op 'Ouvrir' @{ Id = $dVieux.Id.ToUpperInvariant() }), (Op 'Repondre' @{ N = 0 }))))
    (New-Nomme 'reprise : une question disparue est sautée' @{ 'diagnostics.json' = (B64 (TexteHisto @($dDisparue))) } ($debut + @((Op 'Ouvrir' @{ Id = $dDisparue.Id }), (Op 'Repondre' @{ N = 0 }), (Op 'Repondre' @{ N = 0 }))))
    (New-Nomme 'reprise : première réponse hors de la liste, rien de rejoué, l''enquête reste telle quelle' @{ 'diagnostics.json' = (B64 (TexteHisto @($dHors))) } ($debut + @((Op 'Ouvrir' @{ Id = $dHors.Id }), (Op 'Carte'), (Op 'Repondre' @{ N = 0 }), (Op 'Repondre' @{ N = 0 }))))
    (New-Nomme 'reprise : une réponse hors de la liste au milieu, les suivantes gardées en attente' @{ 'diagnostics.json' = (B64 (TexteHisto @($dHors2))) } ($debut + @((Op 'Ouvrir' @{ Id = $dHors2.Id }), (Op 'Repondre' @{ N = 0 }), (Op 'Retour'), (Op 'Retour'))))
    (New-Nomme 'enquête inconnue' @{ 'diagnostics.json' = (B64 $histo3) } ($debut + @((Op 'Ouvrir' @{ Id = 'pas-la' }), (Op 'Retour'))))
    (New-Nomme 'enquête commencée avec des réponses déjà connues (assistant, « danger » toujours posé)' @{} ($debut + @((Op 'Commencer' @{ Panne = 'redemarre'; PreRempli = @(, @('danger', 0)); Hasard = 4660 }), (Op 'Commencer' @{ Panne = 'redemarre'; PreRempli = @(@('cepc', 1), @('inconnue', 2)); Hasard = 255 }), (Op 'Repondre' @{ N = 0 }), (Op 'Commencer' @{ Panne = 'pas-une-panne'; PreRempli = $null; Hasard = 1 }))))
    (New-Nomme 'constat 23 : diagnostics.json vide, bonne copie' @{ 'diagnostics.json' = (B64 ''); 'diagnostics.json.bak' = (B64 $histoBak) } ($debut + @((Op 'Carte'), (& $sauverNeuf '20261030-140500-aaaa'))))
    (New-Nomme 'constat 23 : diagnostics.json en espaces, bonne copie' @{ 'diagnostics.json' = (B64 " `r`n "); 'diagnostics.json.bak' = (B64 $histoBak) } ($debut + @((& $sauverNeuf '20261030-140500-aaab'))))
    (New-Nomme 'constat 23 : diagnostics.json « null », bonne copie' @{ 'diagnostics.json' = (B64 'null'); 'diagnostics.json.bak' = (B64 $histoBak) } ($debut + @((& $sauverNeuf '20261030-140500-aaac'))))
    (New-Nomme 'constat 23 : vide sans copie' @{ 'diagnostics.json' = (B64 '') } ($debut + @((& $sauverNeuf '20261030-140500-aaad'), (& $sauverNeuf '20261030-140500-aaae'))))
    (New-Nomme 'constat 23 : vidé par un autre programme entre la lecture et l''écriture' @{ 'diagnostics.json' = (B64 $histo3); 'diagnostics.json.bak' = (B64 $histoBak) } ($debut + @((Op 'Ecrire' @{ Fichier = 'diagnostics.json'; Texte = '' }), (& $sauverNeuf '20261030-140500-aaaf'))))
    (New-Nomme 'constat 23 : reglages.json vide, bonne copie' @{ 'reglages.json' = (B64 ''); 'reglages.json.bak' = (B64 '{"Theme":"arcade","Volume":40}') } @((Op 'Demarrer'), (Op 'Reglage' @{ Nom = 'Theme'; Defaut = '"neon"' }), (Op 'ChangerReglage' @{ Nom = 'Volume'; Valeur = '55' })))
    (New-Nomme 'coupé, copie bonne' @{ 'diagnostics.json' = (B64 $histo3.Substring(0, 200)); 'diagnostics.json.bak' = (B64 $histoBak) } ($debut + @((Op 'Carte'), (& $sauverNeuf '20261030-140500-bbbb'))))
    (New-Nomme 'coupé, copie abîmée aussi' @{ 'diagnostics.json' = (B64 $histo3.Substring(0, 120)); 'diagnostics.json.bak' = (B64 '{"Version":2,"Diag') } ($debut + @((Op 'Carte'), (& $sauverNeuf '20261030-140500-bbbc'))))
    (New-Nomme 'copie seule (le fichier manque)' @{ 'diagnostics.json.bak' = (B64 $histoBak) } ($debut + @((Op 'Carte'), (& $sauverNeuf '20261030-140500-cccc'))))
    (New-Nomme 'copie seule abîmée (le fichier manque) : mise de côté une seule fois' @{ 'diagnostics.json.bak' = (B64 '{"Version":2,"Diag'); 'reglages.json.bak' = (B64 '') } @((Op 'Demarrer'), (Op 'LireHistorique'), (Op 'Demarrer'), (Op 'LireHistorique'), (& $sauverNeuf '20261030-140500-ccca')))
    (New-Nomme 'pris au lancement, copie abîmée : mise de côté' @{ 'diagnostics.json' = (B64 $histo3); 'diagnostics.json.bak' = (B64 '[1,2]') } ($debut + @((Op 'Carte'), (Op 'Liberer' @{ Fichier = 'diagnostics.json' }), (& $sauverNeuf '20261030-140500-cccb'), (Op 'Carte'))) @('diagnostics.json'))
    (New-Nomme 'avec BOM UTF-8' @{ 'diagnostics.json' = (B64Bom $histo3) } ($debut + @((Op 'Carte'))))
    (New-Nomme 'en UTF-16' @{ 'diagnostics.json' = (B64Utf16 $histo3); 'reglages.json' = (B64Utf16 '{"Theme":"rubis"}') } ($debut + @((Op 'Carte'), (Op 'Reglage' @{ Nom = 'THEME'; Defaut = 'null' }), (& $sauverNeuf '20261030-140500-dddd'))))
    (New-Nomme 'un tableau au lieu d''un objet' @{ 'diagnostics.json' = (B64 '[1,2]'); 'diagnostics.json.bak' = (B64 $histoBak); 'reglages.json' = (B64 '[1,2]') } ($debut + @((Op 'Reglage' @{ Nom = 'Count'; Defaut = 'null' }), (& $sauverNeuf '20261030-140500-eeee'))))
    (New-Nomme 'pris au lancement : la copie est montrée, rien n''est écrit, puis relu et fusionné' @{ 'diagnostics.json' = (B64 $histo3); 'diagnostics.json.bak' = (B64 $histoBak) } ($debut + @((Op 'Carte'), (& $sauverNeuf '20261030-140500-ffff'), (Op 'Carte'), (Op 'Liberer' @{ Fichier = 'diagnostics.json' }), (& $sauverNeuf '20261030-140600-ffff' @(1)), (Op 'Carte'))) @('diagnostics.json'))
    (New-Nomme 'pris au lancement, sans copie' @{ 'diagnostics.json' = (B64 $histo3) } ($debut + @((& $sauverNeuf '20261030-140500-ffaa'), (Op 'Relire' @{ Fichier = 'diagnostics.json' }), (Op 'Liberer' @{ Fichier = 'diagnostics.json' }), (Op 'Relire' @{ Fichier = 'diagnostics.json' }), (Op 'Relire' @{ Fichier = 'diagnostics.json' }))) @('diagnostics.json'))
    (New-Nomme 'réglages pris : gardés en mémoire, puis fusionnés avec le fichier' @{ 'reglages.json' = (B64 '{"Theme":"arcade","Volume":40,"Dictee":true}'); 'reglages.json.bak' = (B64 '{"Theme":"neon"}') } @((Op 'Demarrer'), (Op 'Reglage' @{ Nom = 'Volume'; Defaut = '0' }), (Op 'ChangerReglage' @{ Nom = 'Theme'; Valeur = '"rubis"' }), (Op 'Reglage' @{ Nom = 'theme'; Defaut = 'null' }), (Op 'Liberer' @{ Fichier = 'reglages.json' }), (Op 'Ecrire' @{ Fichier = 'reglages.json'; Texte = '{"Theme":"terminal","Volume":70,"Nouveau":1}' }), (Op 'ChangerReglage' @{ Nom = 'Dictee'; Valeur = 'false' }), (Op 'Reglage' @{ Nom = 'Volume'; Defaut = '0' }), (Op 'Reglage' @{ Nom = 'Theme'; Defaut = '0' })) @('reglages.json'))
    (New-Nomme 'réglages pris tout du long' @{ 'reglages.json' = (B64 '{"Theme":"arcade"}') } @((Op 'Demarrer'), (Op 'ChangerReglage' @{ Nom = 'Theme'; Valeur = '"rubis"' }), (Op 'Reglage' @{ Nom = 'Theme'; Defaut = 'null' })) @('reglages.json'))
    (New-Nomme 'réglages : pas de fusion sans fichier pris' @{ 'reglages.json' = (B64 '{"Theme":"arcade","Volume":40}') } @((Op 'Demarrer'), (Op 'Ecrire' @{ Fichier = 'reglages.json'; Texte = '{"Theme":"terminal","Autre":1}' }), (Op 'ChangerReglage' @{ Nom = 'volume'; Valeur = '{"X":1}' }), (Op 'Reglage' @{ Nom = 'Autre'; Defaut = '"rien"' })))
    (New-Nomme 'douze enquêtes : seules les dix dernières sont gardées' @{ 'diagnostics.json' = (B64 (TexteHisto @($douze | Select-Object -First 10))) } ($debut + @((& $sauverNeuf '20261030-140500-1111'), (& $sauverNeuf '20261030-140500-2222'), (Op 'Carte'))))
    (New-Nomme 'revoir sans rien changer ne déplace pas (même titre en majuscules)' @{ 'diagnostics.json' = (B64 $histo3) } ($debut + @(
        (Op 'Sauver' ([ordered]@{ Id = $dFini.Id; Panne = $dFini.Panne; Choix = [int[]]$dFini.Choix; Fini = $true; Suspect = $dFini.Suspect; Titre = $dFini.Titre.ToUpperInvariant(); Reponses = @($dFini.Reponses | ForEach-Object { , @($_.Etape, $_.Reponse) }); Indices = $null; Reprise = $false })),
        (Op 'Sauver' ([ordered]@{ Id = $dFini.Id; Panne = $dFini.Panne; Choix = [int[]]$dFini.Choix; Fini = $true; Suspect = $dFini.Suspect; Titre = $dFini.Titre; Reponses = @(); Indices = $null; Reprise = $false })),
        (Op 'Sauver' ([ordered]@{ Id = $dEnCours.Id; Panne = $dEnCours.Panne; Choix = [int[]]@(); Fini = $false; Suspect = ''; Titre = ''; Reponses = @(); Indices = $null; Reprise = $true })),
        (Op 'Sauver' ([ordered]@{ Id = $dEnCours.Id; Panne = $dEnCours.Panne; Choix = [int[]]@(); Fini = $false; Suspect = ''; Titre = ''; Reponses = @(); Indices = $null; Reprise = $false })),
        (Op 'Carte'))))
    (New-Nomme 'carte : réponse urgente' @{ 'diagnostics.json' = (B64 (TexteHisto @((DiagCarte @{ Panne = [string]$panneUrgent.Id; Suspect = $urgent.Name; Titre = [string]$urgent.Value.Trouve })))) } ($debut + @((Op 'Carte'))))
    (New-Nomme 'carte : réponse longue coupée, aucune piste, plusieurs pistes' @{ 'diagnostics.json' = (B64 (TexteHisto @((DiagCarte @{ Suspect = 'memoire'; Titre = $long }), (DiagCarte @{ Titre = $rien }), (DiagCarte @{ Titre = $long + ' encore' })))) } ($debut + @((Op 'Carte'), (& $sauverNeuf '20261101-000000-0000'), (Op 'Carte'))) @() '2026-11-01T08:00:00')
    (New-Nomme 'carte : premier du mois, date illisible, panne disparue' @{ 'diagnostics.json' = (B64 (TexteHisto @((DiagCarte @{ Debut = '2026-03-01T07:03:09'; Fini = $false }), (DiagCarte @{ Debut = 'hier soir' }), (DiagCarte @{ Panne = 'panne-disparue' })))) } ($debut + @((Op 'Carte'))))
    (New-Nomme 'carte : panne disparue en tête (carte vide)' @{ 'diagnostics.json' = (B64 (TexteHisto @((DiagCarte @{ Panne = 'panne-disparue' })))) } ($debut + @((Op 'Carte'), (Op 'EcrireEtat'))))
    (New-Nomme 'journal de plus de 1 Mo raccourci au démarrage' @{} ($debut + @((& $sauverNeuf '20261030-140500-9999'))) @() '2026-10-30T14:05:00' @{ Lignes = 40000; Largeur = 31 })
    (New-Nomme 'autre fichier : objet, puis abîmé avec copie' @{ 'autre.json' = (B64 '{"a":{"b":[1,2,{"c":null}]}}'); 'reglages.json' = (B64 '{"Theme":"neon"'); 'reglages.json.bak' = (B64 '{"Theme":"arcade"}') } @((Op 'Demarrer'), (Op 'LireJson' @{ Fichier = 'autre.json' }), (Op 'Relire' @{ Fichier = 'autre.json' }), (Op 'LireJson' @{ Fichier = 'absent.json' }), (Op 'Reglage' @{ Nom = 'Theme'; Defaut = 'null' })))
    (New-Nomme 'historique mélangé (enquêtes sans Id, sans panne, pas des objets)' @{ 'diagnostics.json' = (B64 (New-TexteHistorique (New-Object Random 5) 'melange')) } ($debut + @((Op 'Carte'), (& $sauverNeuf '20261030-140500-7777'))))
)

# ---------------------------------------------------------------------------
# Les scénarios joués : avec les vrais modules, puis avec la réponse voulue (constat 23)
# ---------------------------------------------------------------------------
function ConvertTo-CasScenario($Sc, $Ops, [string]$PowerShell, [string]$Voulu) {
    $champs = [ordered]@{
        Nom = (J $Sc.Nom); Fichiers = (J $Sc.Fichiers); Pris = (J ([string[]]@($Sc.Pris | Where-Object { $_ }))); Maintenant = (J $Sc.Maintenant)
        InfosPc = (J $Sc.InfosPc); Barrettes = (J ([string[]]@($Sc.Barrettes))); JournalGenere = (J $Sc.JournalGenere); Ops = (JListe $Ops); PowerShell = $PowerShell
    }
    if ($Voulu) { $champs.Voulu = $Voulu }
    '{' + (($champs.Keys | ForEach-Object { (J $_) + ':' + $champs[$_] }) -join ',') + '}'
}

$egaux = New-Object 'Collections.Generic.List[string]'
$divergents = New-Object 'Collections.Generic.List[string]'
$tous = @($Nommes | ForEach-Object { @{ Sc = $_; Hasard = $false } })
for ($k = 0; $k -lt $tous.Count + $Scenarios; $k++) {
    if ($k -lt $tous.Count) {
        $sc = $tous[$k].Sc
        $vrai = Invoke-Scenario $sc $false $null
    } else {
        $sc = New-ScenarioHasard ($k - $tous.Count)
        $vrai = Invoke-Scenario $sc $false $ChoisirHasard
    }
    $sc.Ops = $vrai.Ops
    $voulu = Invoke-Scenario $sc $true $null
    if ($voulu.Resultat -ceq $vrai.Resultat) { $egaux.Add((ConvertTo-CasScenario $sc $vrai.Ops $vrai.Resultat $null)) }
    else { $divergents.Add((ConvertTo-CasScenario $sc $vrai.Ops $vrai.Resultat $voulu.Resultat)) }
    if ($k % 20 -eq 0) { [Console]::Error.WriteLine("scénario $k : $($sc.Nom)") }
}
$explScenarios = 'Scénarios joués par les vrais modules de Mindcare (donnees.ps1, historique.ps1, systeme.ps1, contenu.ps1 : seuls les écrans sont remplacés) dans un dossier de données d''essai. Fichiers : le contenu de départ (base64). Pris : les fichiers gardés ouverts sans partage par « un autre programme » au départ. Ops : les opérations, dans l''ordre. PowerShell : les résultats de chaque opération, l''historique en mémoire, les réglages, les fichiers à la fin (les fichiers mis de côté dans Abimes), le journal et les fichiers notés pris. Le dossier d''essai est écrit <D>, la date d''un fichier mis de côté *, la raison d''un fichier illisible *.'
$n = 0
for ($i = 0; $i -lt $egaux.Count; $i += 50) {
    $bout = @($egaux | Select-Object -Skip $i -First 50)
    Write-Juge ('scenarios-{0:000}-{1:000}.json' -f $i, ($i + $bout.Count - 1)) ([ordered]@{ Catalogue = (J $catalogue); Graine = (J $Graine); Explication = (J $explScenarios); Scenarios = ('[' + ($bout -join ',') + ']') })
    $n++
}
$explDivergences = 'Divergence voulue avec Mindcare PowerShell, constat 23 de l''audit : un fichier vide n''est jamais rangé en copie de secours. PowerShell (PowerShell) copiait un fichier vide, abîmé ou qui n''est pas un objet JSON à côté (.abime-<date>) mais le laissait en place : l''enregistrement suivant le poussait dans .bak, et la bonne copie était perdue. Le Rust (Voulu) passe par json_sur de suite-commun : le fichier est déplacé de côté et sa copie remise à sa place ; un fichier vidé entre la lecture et l''écriture est retiré avant d''écrire. Un fichier qui n''est pas un objet JSON (un tableau, un nombre) est abîmé pour json_sur (PowerShell le lisait tel quel). Suites de json_sur : une copie .bak abîmée est déplacée de côté (.bak.abime-<date>, avec les lignes de journal de json_sur) au lieu d''être copiée à chaque lecture et laissée en place ; à l''écriture, seul un fichier qui contient un objet JSON est gardé dans .bak (abîmé, il est d''abord copié de côté) ; deux mises de côté dans la même seconde prennent les noms suivants (-2, -3). Voulu est joué par l''exporteur avec Read-JsonFile et Save-JsonFile tels que le Rust les fait.'
Write-Juge 'divergences.json' ([ordered]@{ Catalogue = (J $catalogue); Graine = (J $Graine); Constat = '23'; Explication = (J $explDivergences); Scenarios = ('[' + ($divergents -join ',') + ']') })
[Console]::Error.WriteLine("scénarios : $($egaux.Count) pareils, $($divergents.Count) divergents")

# ---------------------------------------------------------------------------
# Les cas sans fichier : les fonctions des vrais modules, tirées de leur fichier (rien d'autre n'est exécuté)
# ---------------------------------------------------------------------------
$script:Fr = [Globalization.CultureInfo]::GetCultureInfo('fr-FR')
$ThemesSuite = @('neon', 'crepuscule', 'terminal', 'arcade', 'rubis')
$script:Notes = New-Object 'Collections.Generic.List[string]'
function Write-Log([string]$Message) { $script:Notes.Add($Message) }
function Get-Indices { $script:IndicesJour }
function Get-Barrettes { @($script:Barrettes) }
$script:Donnees = $d
$h = New-Object Random ($Graine + 1)

# Dates (Format-Date, les deux modèles)
$formats = @('s', 'yyyy-MM-dd', 'yyyy-MM-dd HH:mm', 'yyyy-M-d H:mm:ss', 'yyyy-MM-ddTHH:mm:ss.fffffff', 'yyyy-MM-ddTHH:mm', 'MM/dd/yyyy HH:mm:ss', 'M/d/yyyy h:mm tt', 'yyyy/MM/dd HH:mm', 'yyyy-MM-ddTHH:mm:ss.fff')
$isos = New-Object 'Collections.Generic.List[string]'
foreach ($x in @('2026-10-30T14:05:00', '2026-01-01T00:00:00', '2026-02-01T23:59:59', '2028-02-29T12:00:00', '2026-02-29T12:00:00', '2026-13-01T10:00:00', '2026-10-32T10:00:00', '2026-10-30T24:00:00', '2026-10-30T14:60:00', '', ' ', 'hier soir', '2026', '30/10/2026', '10/30/2026', '12/31/2026 12:00 AM', '1/1/2026 12:30 PM', '  2026-10-30T14:05:00  ', '2026-10-30T14:05', '2026-1-5 3:4', '0001-01-01T00:00:00', '9999-12-31T23:59:59', '2026-10-30T14:05:00.5', '2026-10-30 14:05:00')) { $isos.Add($x) }
for ($k = 0; $k -lt $Cas; $k++) {
    $m = New-Moment $h
    if ($h.Next(10) -eq 0) { $m = [datetime]::new(2026, $h.Next(1, 13), 1, $h.Next(24), $h.Next(60), 0) }
    $isos.Add($m.ToString($formats[$h.Next($formats.Count)], [Globalization.CultureInfo]::InvariantCulture))
}
$liste = foreach ($iso in $isos) { [ordered]@{ Iso = $iso; Courte = (Format-Date $iso); Longue = (Format-Date $iso "d MMMM yyyy 'à' HH'h'mm") } }
Write-JugeCas 'dates.json' 'Format-Date (historique.ps1) : la date relue comme [datetime]::Parse (culture invariante) et montrée en français, « 1er » pour le premier du mois ; un texte qui n''est pas une date est rendu tel quel. Courte : le modèle par défaut ; Longue : celui du rapport.' $liste

# Lignes coupées (Get-Court)
$mots = @('le', 'PC', 'redémarre', 'tout', 'seul', 'quand', 'je', 'joue', 'à', 'un', 'jeu', 'mémoire', 'écran', 'bleu', 'ventilateur', 'alimentation', 'disque', 'abîmé', ':', ',', ';', '(RAM)', 'Windows', 'pilote', 'graphique', 'surchauffe', 'très', 'longtemps', 'aujourd''hui', 'carte')
$liste = for ($k = 0; $k -lt $Cas; $k++) {
    $n = $h.Next(0, 40)
    $t = (@(for ($i = 0; $i -lt $n; $i++) { $mots[$h.Next($mots.Count)] }) -join ' ')
    if ($h.Next(6) -eq 0) { $t = $t.Replace(' ', '') }
    if ($h.Next(8) -eq 0) { $t += ' (' }
    $max = @(80, 80, 80, 80, 20, 41, 10, 81)[$h.Next(8)]
    [ordered]@{ Texte = $t; Max = $max; R = (Get-Court $t $max) }
}
Write-JugeCas 'courts.json' 'Get-Court (historique.ps1) : une ligne de la carte de Mindset tient dans Max caractères, coupée au dernier mot entier s''il n''est pas trop tôt, suivie de « ... ».' $liste

# Cartes (Get-EtatCarte, avec Get-DiagnosticInfo et Get-PhraseCarte)
$liste = New-Object 'Collections.Generic.List[object]'
$suspectsNoms = @($d.Suspects.PSObject.Properties.Name)
for ($k = 0; $k -lt $Cas; $k++) {
    $x = New-Diag $h @('v2', 'v2', 'v2a', 'v1')[$h.Next(4)]
    switch ($h.Next(12)) {
        0 { $x.Titre = $long }
        1 { $x.Titre = $rien; $x.Suspect = '' }
        2 { $x.Titre = $long + ' ' + $long; $x.Suspect = '' }
        3 { $x.Suspect = $suspectsNoms[$h.Next($suspectsNoms.Count)]; $x.Fini = $true }
        4 { $x.Panne = ([string]$x.Panne).ToUpperInvariant() }
        5 { $x.Panne = 'panne-disparue' }
        6 { $x.Fini = @('false', 0, 1, '')[$h.Next(4)] }
        7 { $x.Debut = @('hier', '', '2026-04-01T10:00:00', '04/01/2026 10:00')[$h.Next(4)] }
        8 { $x.Titre = '' ; $x.Fini = $true }
        9 { $x.Suspect = $urgent.Name; $x.Fini = $true; $x.Titre = [string]$urgent.Value.Trouve }
    }
    $texte = ConvertTo-Json -InputObject $x -Depth 10 -Compress
    $script:Historique = @($texte | ConvertFrom-Json)
    $c = Get-EtatCarte
    $liste.Add([ordered]@{ Diagnostic = $texte; Catalogue = $true; R = [ordered]@{ Titre = [string]$c.Titre; Scores = @($c.Scores); Lignes = @($c.Lignes | ForEach-Object { [ordered]@{ Texte = [string]$_.Texte; Etat = [string]$_.Etat } }) } })
}
$script:Historique = @(); $c = Get-EtatCarte
$liste.Add([ordered]@{ Diagnostic = $null; Catalogue = $true; R = [ordered]@{ Titre = [string]$c.Titre; Scores = @($c.Scores); Lignes = @($c.Lignes | ForEach-Object { [ordered]@{ Texte = [string]$_.Texte; Etat = [string]$_.Etat } }) } })
$script:Donnees = $null; $script:Historique = @(('{"Id":"a","Panne":"redemarre","Fini":true}' | ConvertFrom-Json)); $c = Get-EtatCarte
$liste.Add([ordered]@{ Diagnostic = '{"Id":"a","Panne":"redemarre","Fini":true}'; Catalogue = $false; R = [ordered]@{ Titre = [string]$c.Titre; Scores = @($c.Scores); Lignes = @($c.Lignes | ForEach-Object { [ordered]@{ Texte = [string]$_.Texte; Etat = [string]$_.Etat } }) } })
$script:Donnees = $d
Write-JugeCas 'cartes.json' 'Get-EtatCarte (historique.ps1) : la carte de Mindset pour la dernière enquête gardée (Diagnostic, telle que dans diagnostics.json ; null : aucune). Catalogue faux : enquetes.json illisible.' $liste.ToArray()

# Le modèle du PC (Get-InfosPC, la lecture de Windows remplacée)
$valeurs = @('Dell Inc.', '  LENOVO  ', '', $null, "HP`t", "ASUS$([char]0xA0)", 'Micro-Star International Co., Ltd.', "$([char]0x2003)Acer ")
$liste = for ($k = 0; $k -lt 60; $k++) {
    $script:Cim = [ordered]@{ Fabricant = $valeurs[$h.Next($valeurs.Count)]; Modele = $valeurs[$h.Next($valeurs.Count)]; Systeme = @('Microsoft Windows 11 Famille', ' Microsoft Windows 10 Professionnel ', '', $null)[$h.Next(4)]; Erreur = @('', '', '', 'Win32_ComputerSystem', 'Win32_OperatingSystem')[$h.Next(5)] }
    $script:InfosPC = $null; $script:Notes.Clear()
    $r = Get-InfosPC
    [ordered]@{ Lecture = $script:Cim; R = $r; Journal = [string[]]$script:Notes.ToArray() }
}
Write-JugeCas 'infos-pc.json' 'Get-InfosPC (historique.ps1) sans la lecture de Windows : Lecture donne ce que Win32_ComputerSystem et Win32_OperatingSystem auraient rendu (Erreur : la classe illisible). R : le texte du rapport ; Journal : les lignes notées.' $liste

# Rapports (Get-ResumeTexte, Get-Paires)
function Get-ReponseCepc($Etape) { $k = 0; foreach ($r in @($Etape.Reponses)) { if ($r -and [string]$r.Note -eq 'cepc') { return $k }; $k++ }; -1 }
$liste = for ($k = 0; $k -lt $Cas; $k++) {
    $p = $pannes[$h.Next($pannes.Count)]
    $ind = if ($h.Next(2)) { New-IndicesHasard $h } else { $null }
    $table = if ($null -ne $ind) { ConvertTo-TableIndices $ind } else { $null }
    $choix = New-Object 'Collections.Generic.List[int]'
    $fin = $h.Next(3) -gt 0
    for ($t = 0; $t -lt 60; $t++) {
        $e = Get-Enquete $d $p $choix.ToArray() $table
        if (-not $e.Etape -or (-not $fin -and $h.Next(5) -eq 0)) { break }
        $cepc = Get-ReponseCepc $e.Etape
        $nb = @($e.Etape.Reponses).Count
        $choix.Add($(if ($cepc -ge 0 -and $h.Next(3) -gt 0) { $cepc } elseif ($h.Next(10) -eq 0) { -1 } else { $h.Next($nb) }))
    }
    $e = Get-Enquete $d $p $choix.ToArray() $table
    $jour = if ($h.Next(2)) { New-IndicesHasard $h } else { @() }
    $script:IndicesJour = ConvertTo-TableIndices $jour
    $script:Barrettes = @(@(), @('8 Go Kingston KF3200 (DIMM1)'), @(''), @('16 Go Crucial (A1)', '16 Go Crucial (B1)'))[$h.Next(4)]
    $script:InfosPC = @('', 'Dell Inc. Inspiron 15, Microsoft Windows 11 Famille')[$h.Next(2)]
    $script:Maintenant = New-Moment $h
    if ($h.Next(8) -eq 0) { $script:Maintenant = [datetime]::new(2026, 5, 1, 8, 0, 0) }
    $titre = if ($h.Next(5) -eq 0) { 'À vérifier : la mémoire (RAM)' } else { Get-VerdictTitre $d $p $e.Verdict }
    $AppVersion = @('0.15.1', '0.90.0')[$h.Next(2)]
    $paires = @(foreach ($q in @(Get-Paires $e)) { if ($q) { , @([string]$q.Etape, [int]$q.Reponse) } })
    [ordered]@{
        Panne = [string]$p.Id; Entree = [int[]]$choix.ToArray(); Indices = $ind; IndicesJour = $jour; Barrettes = [string[]]@($script:Barrettes); InfosPc = $script:InfosPC
        Maintenant = $script:Maintenant.ToString('s'); Titre = $titre; Version = $AppVersion
        R = (Get-ResumeTexte $p $e $titre); Paires = $paires
    }
}
Write-JugeCas 'rapports.json' 'Get-ResumeTexte (historique.ps1) : le rapport pour le réparateur d''une enquête (Panne, Entree : les réponses, Indices : ce que Windows a noté pour l''enquête, IndicesJour : Get-Indices pour « À savoir »), avec Get-Barrettes, Get-InfosPC et la date donnés. Paires : Get-Paires de la même enquête.' $liste

# Les réponses en attente d'une enquête reprise (Get-PairesEnAttente, Test-Reprise)
$liste = for ($k = 0; $k -lt $Cas; $k++) {
    $x = New-Parcours $h
    $p = $x.Panne
    $ids = @(Get-Etapes $d $p | ForEach-Object { [string]$_.Id }) + @('question-disparue')
    $rep = New-Object 'Collections.Generic.List[object]'
    for ($i = $h.Next(0, 8); $i -gt 0; $i--) {
        $id = $ids[$h.Next($ids.Count)]
        if ($h.Next(10) -eq 0) { $id = $id.ToUpperInvariant() }
        $rep.Add([ordered]@{ Etape = $id; Reponse = @(0, 1, 2, -1, -2, 5, 99, '1', $null)[$h.Next(9)] })
    }
    $repJson = ConvertTo-Json -InputObject ([object[]]$rep.ToArray()) -Depth 5 -Compress
    $lu = ('{"R":' + $repJson + '}') | ConvertFrom-Json
    $script:Reprise = if ($h.Next(8)) { @{ Id = 'r1'; Reponses = @($lu.R) } } else { $null }
    $script:DiagId = @('r1', 'R1', 'autre')[$h.Next(3)]
    $pr = $null
    if ($h.Next(8)) { $pr = @{}; foreach ($r in $rep) { if ($h.Next(3)) { $pr[[string]$r.Etape] = 0 } }; if ($h.Next(5) -eq 0) { $pr['sans-question'] = 1 } }
    $script:PreRempli = $pr
    $script:PanneEnCours = $p
    $r = @(foreach ($q in @(Get-PairesEnAttente $x.E)) { if ($q) { , @([string]$q.Etape, [int]$q.Reponse) } })
    $cles = $null; if ($null -ne $pr) { $cles = [string[]]@($pr.Keys) }   # (pas « $cles = if ... » : un tableau d'un seul élément serait déroulé)
    [ordered]@{ Panne = [string]$p.Id; Entree = $x.Choix; Reprise = $(if ($script:Reprise) { $repJson } else { $null }); DiagId = $script:DiagId; PreRempli = $cles; TestReprise = [bool](Test-Reprise); R = $r }
}
Write-JugeCas 'attente.json' 'Get-PairesEnAttente et Test-Reprise (contenu.ps1) : l''enquête reprise a l''Id r1 et les réponses gardées Reprise (texte JSON, null : pas une reprise) ; DiagId : l''enquête en cours ; PreRempli : les Id des réponses pas encore rejouées (null : aucune table). Entree : les réponses de l''enquête rejouée. R : les réponses gardées en attente.' $liste

# Les indices gardés relus (ConvertTo-Indices)
$formes = @('{"Vu":true,"Texte":"a","TexteOk":"b","Affiche":true,"Partiel":false}', '{"Vu":"false","Texte":12,"TexteOk":null,"Affiche":1,"Partiel":"x"}', '{}', 'null', '{"Vu":0,"Affiche":"","Partiel":[]}', '"texte"', '{"Texte":["a","b"],"Vu":[true]}', '{"TEXTE":"maj","vu":true}')
$liste = for ($k = 0; $k -lt 120; $k++) {
    $o = [ordered]@{}
    for ($i = $h.Next(0, 6); $i -gt 0; $i--) { $o[$IndicesConnus[$h.Next($IndicesConnus.Count)]] = '@' + $formes[$h.Next($formes.Count)] + '@' }
    $texte = [regex]::Replace((ConvertTo-Json -InputObject $o -Compress), '"@(.*?)@"', { param($m) [regex]::Unescape($m.Groups[1].Value) })
    if ($k -eq 0) { $texte = 'null' }
    $g = $texte | ConvertFrom-Json
    $t = ConvertTo-Indices $g
    $cles = [string[]]@($t.Keys); [Array]::Sort($cles, [StringComparer]::OrdinalIgnoreCase)
    [ordered]@{ Gardes = $texte; R = @(foreach ($c in $cles) { $v = $t[$c]; , @($c, $v.Vu, $v.Texte, $v.TexteOk, $v.Affiche, $v.Partiel) }) }
}
Write-JugeCas 'indices.json' 'ConvertTo-Indices (historique.ps1) : les indices gardés avec une enquête finie, relus de diagnostics.json (Gardes, texte JSON). R : [Nom, Vu, Texte, TexteOk, Affiche, Partiel], rangés par nom.' $liste

# Chemins (Get-CheminNormal, Get-DonneesMindset, Test-GereeParMindset) et Id (New-DiagId)
$liste = New-Object 'Collections.Generic.List[object]'
$chemins = @('C:\a\b', 'c:\a\..\b\', 'C:\a\.\b\\c', 'C:/a/b', 'C:\a\b. ', 'C:\a\b...', 'C:\a\x*', 'C:\a\b:c', '\\serveur\x', 'relatif\x', '', 'C:\', 'C:\\', 'D:\Mindset\Programmes\Mindcare\', 'C:\a\b\..\..\..\..', 'C:\a<b', 'C:\a|b', 'C:\Program Files\Mindset\Mindcare', 'e:\MINDSET\programmes\Mindcare', 'C:\a\ b \c', 'C:\a\b\.', '1:\a', 'C:\a/b\c')
foreach ($c in $chemins) { $liste.Add([ordered]@{ Fonction = 'Get-CheminNormal'; Entree = $c; R = (Get-CheminNormal $c) }) }
$ancienLocal = $env:LOCALAPPDATA; $ancienPf = $env:ProgramFiles; $ancienPw = $env:ProgramW6432
try {
    $env:LOCALAPPDATA = 'C:\Users\Essai\AppData\Local'
    foreach ($r in @('D:\Mindset\Programmes\Mindcare', 'E:\mindset\PROGRAMMES\Mindcare', 'C:\Program Files\Mindset\Mindcare', 'D:\Programmes\Mindcare', 'D:\Mindset\Programmes', 'D:\Mindset', 'D:\', '','D:\X\Mindset\Programmes\Mindcare', 'D:\Mindset\Programme\Mindcare')) {
        $liste.Add([ordered]@{ Fonction = 'Get-DonneesMindset'; Entree = $r; Local = $env:LOCALAPPDATA; R = (Get-DonneesMindset $r) })
    }
    $pfs = @(@('C:\Program Files', 'C:\Program Files'), @('D:\Apps', $null), @($null, $null), @('C:\Program Files (x86)', 'C:\Program Files'))
    foreach ($r in @('D:\Mindset\Programmes\Mindcare', 'C:\Program Files\Mindset\Mindcare', 'D:\Apps\Mindset\Mindcare', 'C:\Users\Jo\Mindcare', '', 'C:\PROGRAM FILES\MINDSET\Mindcare')) {
        foreach ($s in @(@{ Dossier = $null; Pris = $false }, @{ Dossier = $null; Pris = $true }, @{ Dossier = 'c:\users\jo\mindcare'; Pris = $false }, @{ Dossier = 'D:\Autre'; Pris = $true })) {
            $pf = $pfs[$h.Next($pfs.Count)]
            $env:ProgramFiles = $pf[0]; $env:ProgramW6432 = $pf[1]
            $liste.Add([ordered]@{ Fonction = 'Test-GereeParMindset'; Entree = $r; Suite = $s; ProgramFiles = $pf[0]; ProgramW6432 = $pf[1]; R = (Test-GereeParMindset $s $r) })
        }
    }
} finally { $env:LOCALAPPDATA = $ancienLocal; $env:ProgramFiles = $ancienPf; $env:ProgramW6432 = $ancienPw }
for ($k = 0; $k -lt 40; $k++) {
    $script:Maintenant = New-Moment $h; $script:Hasard = @(0, 1, 255, 4096, 65535, $h.Next(65536))[$h.Next(6)]
    $liste.Add([ordered]@{ Fonction = 'New-DiagId'; Maintenant = $script:Maintenant.ToString('s'); Hasard = $script:Hasard; R = (New-DiagId) })
}
Write-JugeCas 'chemins.json' 'Get-CheminNormal et Test-GereeParMindset (mindset.ps1), Get-DonneesMindset (donnees.ps1, LOCALAPPDATA donné par Local ; ProgramFiles et ProgramW6432 donnés), New-DiagId (contenu.ps1, la date et le hasard donnés).' $liste.ToArray()

# suite.json (Read-Suite)
$liste = New-Object 'Collections.Generic.List[object]'
$contenus = @(
    '{"theme":"neon","applis":{"mindcare":"D:\\Mindset\\Programmes\\Mindcare\\"},"micro":"Micro (Realtek)"}', '{"theme":"NEON"}', '{"theme":"pack:mon-pack-2"}', '{"theme":"pack:Mon"}', "{`"theme`":`"pack:abc\n`"}",
    '{"theme":"pack:"}', '{"theme":3}', '{"theme":"inconnu"}', '{"applis":{"MINDCARE":"c:\\a\\..\\b"}}', '{"applis":{"mindcare":"relatif"}}', '{"applis":{"mindcare":5}}', '{"applis":null}',
    ('{"micro":"' + ('m' * 64) + '"}'), ('{"micro":"' + ('m' * 65) + '"}'), '{"micro":"a\tb"}', '{"micro":"Casque é"}', '{"micro":12}', '', '   ', 'null', '{"theme":', '[{"theme":"neon"}]', '[{"theme":"neon"},{"theme":"rubis"}]',
    '42', '"neon"', '{"Theme":"Arcade","Applis":{"Mindcare":"C:\\Program Files\\Mindset\\Mindcare"}}', '{}')
$dossierSuite = Join-Path $travail 'suite'; New-Item -ItemType Directory -Force -Path $dossierSuite | Out-Null
$SuiteFile = Join-Path $dossierSuite 'suite.json'
foreach ($c in $contenus) {
    [IO.File]::WriteAllText($SuiteFile, $c, $Utf8)
    $s = Read-Suite 2
    $liste.Add([ordered]@{ Contenu = (B64 $c); Pris = $false; R = $s })
}
[IO.File]::WriteAllBytes($SuiteFile, [Convert]::FromBase64String((B64Bom '{"theme":"rubis"}'))); $liste.Add([ordered]@{ Contenu = (B64Bom '{"theme":"rubis"}'); Pris = $false; R = (Read-Suite 2) })
[IO.File]::WriteAllBytes($SuiteFile, [Convert]::FromBase64String((B64Utf16 '{"theme":"terminal"}'))); $liste.Add([ordered]@{ Contenu = (B64Utf16 '{"theme":"terminal"}'); Pris = $false; R = (Read-Suite 2) })
[IO.File]::WriteAllText($SuiteFile, '{"theme":"arcade"}', $Utf8)
$verrou = [IO.File]::Open($SuiteFile, 'Open', 'Read', 'None')
try { $liste.Add([ordered]@{ Contenu = (B64 '{"theme":"arcade"}'); Pris = $true; R = (Read-Suite 2) }) } finally { $verrou.Dispose() }
[IO.File]::Delete($SuiteFile); $liste.Add([ordered]@{ Contenu = $null; Pris = $false; R = (Read-Suite 2) })
Write-JugeCas 'suites.json' 'Read-Suite (mindset.ps1), 2 essais : suite.json contient Contenu (base64 ; null : absent) ; Pris : gardé ouvert sans partage par un autre programme pendant la lecture.' $liste.ToArray()

Remove-Item -LiteralPath $travail -Recurse -Force -ErrorAction SilentlyContinue
[Console]::Error.WriteLine('fini')
