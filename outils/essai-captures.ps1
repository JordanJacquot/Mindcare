# Captures de CHAQUE écran de Mindcare en Rust, dans plusieurs thèmes (étape 5.5), avec la CONSTRUCTION D'ESSAI. À
# lancer SEULEMENT sur un bureau Windows à part (outils\sur-bureau.ps1) :
#
#   & '.\outils\sur-bureau.ps1' -Script '.\outils\essai-captures.ps1' -Arguments "-Exe <Mindset.exe d'essai> -Resultats <dossier>" -Secondes 1500
#
# Chaque écran est atteint sans clavier : le script écrit les messages que la page enverrait (actions.demande, lu
# seulement par la construction d'essai), puis demande une capture (capture.demande). Les enquêtes jouées sont des
# parcours exportés du vrai Mindcare PowerShell (juges du moteur) : une réponse finale de chaque genre. Les captures
# vont dans <Resultats>\<thème>\NN-nom.png, avec <Resultats>\index.txt (fichier, écran, ce qu'on doit y voir).
#
# Les thèmes : Néon, Crépuscule, Terminal, Arcade, Rubis et un thème perso d'ESSAI (« pack:essai-ciel » : un pack
# inventé ici, dans le dossier d'essai, avec une image de fond dessinée par le script et ses couleurs P/S/T ; aucun
# vrai pack). Ses captures vont dans <Resultats>\perso-essai-ciel.
#
# Le PC : celui, inventé, de la construction d'essai (un PC fixe monté ASUS, src\essai.rs) ; le vrai matériel n'est
# jamais lu. Les captures de « Ton PC » montrent ce PC, puis un autre PC choisi (un portable HP), puis le retour.
#
# Rien du vrai PC : un dossier d'essai dans %TEMP% (données, faux %LOCALAPPDATA%, faux Bureau, faux presse-papiers),
# les verrou et signal d'ESSAI (Local\MindcareEssai-...), jamais ceux de la vraie Mindcare ni de Mindset. Windows est
# seulement lu (le journal Système, sans droits administrateur), comme dans essai-ecrans.ps1.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Resultats,
    [string[]]$Themes = @('neon', 'crepuscule', 'terminal', 'arcade', 'rubis', 'pack:essai-ciel'),
    # Seulement ces captures (leur nom, par exemple « reparateur-avant ») ; rien : toutes
    [string[]]$Seulement = @()
)
$ErrorActionPreference = 'Stop'
$echecs = New-Object System.Collections.Generic.List[string]
function Ok([string]$t) { Write-Host "OK : $t" }
function Echec([string]$t) { Write-Host "ÉCHEC : $t"; $echecs.Add($t) }
function Verifier([bool]$c, [string]$t) { if ($c) { Ok $t } else { Echec $t } }

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class EssaiCaptures
{
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int x, y; }
    public static string Classe(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
    public static string Titre(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
    public static IntPtr[] Fenetres(int pid)
    {
        var l = new List<IntPtr>();
        EnumWindows(delegate (IntPtr h, IntPtr x) { int p; GetWindowThreadProcessId(h, out p); if (p == pid) l.Add(h); return true; }, IntPtr.Zero);
        return l.ToArray();
    }
    public static IntPtr[] Enfants(IntPtr parent, string classe)
    {
        var l = new List<IntPtr>();
        EnumChildWindows(parent, delegate (IntPtr h, IntPtr x) { if (Classe(h) == classe) l.Add(h); return true; }, IntPtr.Zero);
        return l.ToArray();
    }
    // La molette (WM_MOUSEWHEEL : position en coordonnées de l'écran) : la page défile
    public static void Molette(IntPtr h, int x, int y, int crans)
    {
        var p = new POINT(); p.x = x; p.y = y; ClientToScreen(h, ref p);
        IntPtr pos = (IntPtr)((p.y << 16) | (p.x & 0xFFFF));
        PostMessage(h, 0x0200, IntPtr.Zero, (IntPtr)((y << 16) | (x & 0xFFFF)));
        for (int i = 0; i < Math.Abs(crans); i++) PostMessage(h, 0x020A, (IntPtr)((crans < 0 ? -120 : 120) << 16), pos);
    }
    public static void Touche(IntPtr h, int vk)
    {
        PostMessage(h, 0x0100, (IntPtr)vk, (IntPtr)1);                       // WM_KEYDOWN
        PostMessage(h, 0x0101, (IntPtr)vk, (IntPtr)unchecked((int)0xC0000001)); // WM_KEYUP
    }
}
'@

$Exe = (Resolve-Path -LiteralPath $Exe).Path
$texte = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Exe))
if (-not $texte.Contains('actions.demande')) { Write-Output "ÉCHEC : $Exe n'est pas la construction d'essai (ou trop ancienne) : rien n'est lancé."; exit 1 }
$depot = Split-Path $PSScriptRoot -Parent
New-Item -ItemType Directory -Force -Path $Resultats | Out-Null
$Resultats = (Resolve-Path -LiteralPath $Resultats).Path
$index = New-Object System.Collections.Generic.List[string]
$utf8 = New-Object Text.UTF8Encoding($false)

$script:racine = $null; $script:donnees = $null; $script:local = $null; $script:programmes = $null; $script:exeEssai = $null
$script:p = $null; $script:cible = [IntPtr]::Zero; $script:theme = ''; $script:dossierTheme = ''
$nomEssai = "captures-$PID"

function Preparer([string]$Theme, [bool]$AvecCatalogue) {
    $script:racine = Join-Path $env:TEMP "mindcare-essai-captures-$PID"
    if (Test-Path -LiteralPath $script:racine) { Remove-Item -LiteralPath $script:racine -Recurse -Force }
    $script:programmes = Join-Path $script:racine 'Programmes\Mindcare'
    $script:donnees = Join-Path $script:racine 'Donnees\Mindcare'
    $script:local = Join-Path $script:racine 'Local'
    New-Item -ItemType Directory -Force -Path $script:programmes, $script:donnees, (Join-Path $script:local 'Mindset') | Out-Null
    $script:exeEssai = Join-Path $script:programmes 'Mindset.exe'
    Copy-Item -LiteralPath $Exe -Destination $script:exeEssai
    if ($AvecCatalogue) { Copy-Item -LiteralPath (Join-Path $depot 'Mindcare\fichiers\enquetes.json') -Destination (Join-Path $script:programmes 'enquetes.json') }
    if ($Theme -like 'pack:*') { Nouveau-Pack ($Theme.Substring(5)) }
    $j = [ordered]@{ theme = $Theme; micro = ''; casque = ''; applis = [ordered]@{ mindcare = $script:programmes }; modifie = (Get-Date -Format s) }
    [IO.File]::WriteAllText((Join-Path $script:local 'Mindset\suite.json'), ($j | ConvertTo-Json -Depth 4), $utf8)
}

# Un thème perso d'ESSAI, inventé ici (jamais un vrai pack) : une image de fond dessinée (un ciel du soir en dégradé,
# avec des collines), et ses couleurs P/S/T, dans les packs du dossier de données d'essai
function Nouveau-Pack([string]$Id) {
    Add-Type -AssemblyName System.Drawing
    $d = Join-Path $script:donnees "packs\$Id"
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $fond = New-Object Drawing.Bitmap 1280, 720
    $g = [Drawing.Graphics]::FromImage($fond)
    $ciel = New-Object Drawing.Drawing2D.LinearGradientBrush ([Drawing.Point]::new(0, 0)), ([Drawing.Point]::new(0, 720)), ([Drawing.Color]::FromArgb(255, 18, 40, 92)), ([Drawing.Color]::FromArgb(255, 232, 128, 72))
    $g.FillRectangle($ciel, 0, 0, 1280, 720)
    $g.FillEllipse((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(255, 255, 214, 140))), 900, 300, 160, 160)
    $g.FillEllipse((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(255, 34, 70, 60))), -200, 520, 900, 400)
    $g.FillEllipse((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(255, 24, 52, 48))), 500, 560, 1000, 380)
    $g.Dispose()
    $fond.Save((Join-Path $d 'fond.jpg'), [Drawing.Imaging.ImageFormat]::Jpeg); $fond.Dispose()
    $pack = [ordered]@{ Name = 'Ciel d''essai'; Colors = [ordered]@{ P = '#FF9F5A'; S = '#7FC8FF'; T = '#FFD36E'; NH = 210; NS = 0.5 }
        Background = 'fond.jpg'; BackgroundOpacity = 0.45; BackgroundAlign = 'bas' }
    [IO.File]::WriteAllText((Join-Path $d 'pack.json'), ($pack | ConvertTo-Json -Depth 4), $utf8)
}

function Get-Journal {
    $journal = Join-Path $script:donnees 'journal.txt'
    if (-not (Test-Path -LiteralPath $journal)) { return @() }
    $f = New-Object IO.FileStream($journal, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    try { $l = (New-Object IO.StreamReader($f, [Text.Encoding]::UTF8)).ReadToEnd() } finally { $f.Dispose() }
    @($l -split "`r`n" | Where-Object { $_ })
}
function Compter([string]$Motif) { @(Get-Journal | Where-Object { $_ -like "*$Motif*" }).Count }
function Attendre([scriptblock]$Condition, [int]$Ms) {
    $t = [Diagnostics.Stopwatch]::StartNew()
    while ($t.ElapsedMilliseconds -lt $Ms) { if (& $Condition) { return $true }; Start-Sleep -Milliseconds 100 }
    [bool](& $Condition)
}

function Lancer([string]$Quoi) {
    $psi = New-Object Diagnostics.ProcessStartInfo $script:exeEssai
    $psi.Arguments = '-Demarrage'
    $psi.UseShellExecute = $false
    $psi.WorkingDirectory = $script:programmes
    $psi.EnvironmentVariables['MINDCARE_DONNEES'] = $script:donnees
    $psi.EnvironmentVariables['MINDCARE_ESSAI_LOCAL'] = $script:local
    $psi.EnvironmentVariables['MINDCARE_ESSAI_NOM'] = $nomEssai
    $prets = Compter 'Écran prêt'
    $script:p = [Diagnostics.Process]::Start($psi)
    Write-Host "$Quoi : processus $($script:p.Id)"
    $pid2 = $script:p.Id
    $vu = Attendre { @([EssaiCaptures]::Fenetres($pid2) | Where-Object { [EssaiCaptures]::Classe($_) -ceq 'Tauri Window' -and [EssaiCaptures]::Titre($_) -ceq 'Mindcare' }).Count -ge 1 } 20000
    Verifier $vu "$Quoi : fenêtre « Mindcare »"
    $f = @([EssaiCaptures]::Fenetres($pid2) | Where-Object { [EssaiCaptures]::Classe($_) -ceq 'Tauri Window' -and [EssaiCaptures]::Titre($_) -ceq 'Mindcare' })[0]
    Verifier (Attendre { (Compter 'Écran prêt') -gt $prets } 20000) "$Quoi : le kit est prêt"
    # Le signal d'ESSAI (Local\MindcareEssai-...) la montre
    $s = [Threading.EventWaitHandle]::OpenExisting("Local\MindcareEssai-$nomEssai-Afficher"); [void]$s.Set(); $s.Dispose()
    Verifier (Attendre { [EssaiCaptures]::IsWindowVisible($f) } 5000) "$Quoi : montrée par le signal d'essai"
    $script:cible = [IntPtr]::Zero
    [void](Attendre { $script:cible = @([EssaiCaptures]::Enfants($f, 'Chrome_RenderWidgetHostHWND'))[0]; $null -ne $script:cible -and $script:cible -ne [IntPtr]::Zero } 5000)
}
function Quitter {
    if (-not $script:p -or $script:p.HasExited) { return }
    [IO.File]::WriteAllText((Join-Path $script:donnees 'quitter.demande'), "$PID")
    if (-not $script:p.WaitForExit(10000)) { try { $script:p.Kill() } catch {} ; Echec 'l''appli ne quitte pas sur quitter.demande' }
    Start-Sleep -Milliseconds 1500
    # (le journal de ce lancement, gardé avec les captures du thème)
    $l = @("--- Processus $($script:p.Id) ---") + @(Get-Journal)
    [IO.File]::AppendAllText((Join-Path $script:dossierTheme 'journal.txt'), (($l -join "`r`n") + "`r`n"), $utf8)
}
# Une touche du clavier envoyée à la page (flèche droite : 0x27)
function Touche([int]$Vk) { [EssaiCaptures]::Touche($script:cible, $Vk); Start-Sleep -Milliseconds 400 }

# Des messages de la page (ou des ordres au kit), écrits d'un coup (un renommage), puis lus par l'appli
function Envoyer([object[]]$Messages, [int]$Pause = 700) {
    $lignes = @($Messages | ForEach-Object { $_ | ConvertTo-Json -Compress -Depth 5 })
    $tmp = Join-Path $script:donnees 'actions.tmp'
    $dest = Join-Path $script:donnees 'actions.demande'
    [IO.File]::WriteAllText($tmp, ($lignes -join "`n"), $utf8)
    Move-Item -LiteralPath $tmp -Destination $dest -Force
    if (-not (Attendre { -not (Test-Path -LiteralPath $dest) } 5000)) { Echec "actions non lues : $($lignes -join ' ; ')" }
    Start-Sleep -Milliseconds $Pause
}
function Action([string]$Id, $Valeur = $null, [string]$Ecran = 'accueil') { [ordered]@{ type = 'action'; id = $Id; valeur = $Valeur; ecran = $Ecran } }
function Rep([int]$n) { if ($n -lt 0) { Action 'nsp' $null 'enquete' } else { Action 'rep' "$n" 'enquete' } }
# Une enquête : la panne, puis les réponses
function Enquete([string]$Panne, [int[]]$Choix) {
    $m = @(Action "panne:$Panne" $null 'raconte')
    foreach ($c in $Choix) { $m += Rep $c }
    Envoyer $m 900
}

$script:n = 0
function Capturer([string]$Nom, [string]$Ecran, [string]$AVoir, [int]$Attente = 900) {
    $script:n++
    if ($Seulement.Count -and $Seulement -notcontains $Nom) { return }
    $fichier = '{0:D2}-{1}.png' -f $script:n, $Nom
    $c = Join-Path $script:donnees 'capture.png'
    if (Test-Path -LiteralPath $c) { Remove-Item -LiteralPath $c -Force }
    Start-Sleep -Milliseconds $Attente   # (les animations d'entrée du kit)
    [IO.File]::WriteAllText((Join-Path $script:donnees 'capture.demande'), '1')
    $ok = Attendre { Test-Path -LiteralPath $c } 5000
    Verifier $ok "capture $($script:theme)\$fichier"
    if ($ok) {
        Start-Sleep -Milliseconds 300
        Copy-Item -LiteralPath $c -Destination (Join-Path $script:dossierTheme $fichier) -Force
        $index.Add("$($script:theme)\$fichier | $Ecran | $AVoir")
    }
}

# Les parcours (exportés du PowerShell, juges du moteur : sans « Je ne sais pas », jamais sur ce PC)
$reponses = @(
    @{ nom = 'reponse-urgent'; panne = 'redemarre'; choix = @(0); badge = 'Urgent' },
    @{ nom = 'reponse-trouve'; panne = 'navplante'; choix = @(0); badge = 'Trouvé' },
    @{ nom = 'reponse-tout-ecarte'; panne = 'cpufan'; choix = @(1); badge = 'Tout le reste est écarté' },
    @{ nom = 'reponse-piste-probable'; panne = 'chute'; choix = @(2); badge = 'La piste la plus probable' },
    @{ nom = 'reponse-aucune-piste'; panne = 'langue'; choix = @(1); badge = 'Aucune piste ne correspond' },
    @{ nom = 'reponse-reparateur'; panne = 'memoiretraining'; choix = @(1, 2); badge = 'À faire vérifier par un réparateur' }
)

function Parcours-Theme {
    Preparer $script:theme $true
    $script:n = 0
    # L'attente de Windows : l'enquête « sur ce PC » (Windows démarre encore, puis « Oui, sur ce PC-là » ; la réponse
    # « revient à chaque démarrage » sauterait la question « Sur ce PC ? ») est demandée AVANT le départ (lue au premier pas de l'appli, quand
    # Windows commence tout juste à être lu)
    $m = @((Action 'panne:ecranbleu' $null 'raconte'), (Rep 0), (Rep 0))
    [IO.File]::WriteAllText((Join-Path $script:donnees 'actions.demande'), (@($m | ForEach-Object { $_ | ConvertTo-Json -Compress -Depth 5 }) -join "`n"), $utf8)
    Lancer "Thème $($script:theme)"
    # (l'écran demandé avant que le kit soit prêt n'est pas suivi : on y va)
    Envoyer @([ordered]@{ kit = 'aller'; ecran = 'enquete' }) 100
    $attente = (Compter 'Journaux de Windows') -eq 0
    Capturer 'attente-windows' 'Enquête « écran bleu » sur ce PC' $(if ($attente) { 'La page d''attente : « Mindcare regarde ce que Windows a noté... », avec Étape précédente et Accueil' } else { 'Windows était déjà lu : l''étape suivante tout de suite (la page d''attente n''a pas pu être provoquée)' }) 150
    Write-Host "Attente de Windows capturée : $attente"
    [void](Attendre { (Compter 'Journaux de Windows') -gt $lus } 35000)
    Envoyer @(Action 'accueil' $null 'enquete')
    # (la pastille « Ton PC » et la bande « Adapté à ton PC » : le PC inventé de l'essai est reconnu au démarrage)
    Verifier (Attendre { (Compter 'PC reconnu : PC fixe ASUS') -ge 1 } 10000) 'le PC de l''essai reconnu (PC fixe ASUS)'

    Capturer 'accueil' 'Accueil' 'Les tuiles Raconte ton problème, Ça concerne quoi ?, Reprendre l''enquête (l''enquête « écran bleu » en cours) ; le rond Comment ça marche en haut à droite'
    Envoyer @(Action 'raconte')
    Capturer 'raconte-vide' 'Raconte ton problème (vide)' 'Le champ de recherche vide, son exemple, la note qui explique'
    Envoyer @(Action 'recherche' 'écran bleu' 'raconte')
    Capturer 'raconte-trouve' 'Raconte ton problème : « écran bleu »' 'Les pannes trouvées, la plus proche en premier'
    Envoyer @(Action 'recherche' 'xyzzy plouf' 'raconte')
    Capturer 'raconte-rien' 'Raconte ton problème : rien trouvé' '« Rien ne correspond à « xyzzy plouf » » et ce qu''on peut faire'
    Envoyer @(Action 'recherche' '' 'raconte'; Action 'familles' $null 'raconte')
    Capturer 'familles' 'Ça concerne quoi ? (les familles)' 'La bande « Adapté à ton PC fixe ASUS : N pannes de PC portable ou d''autres marques sont cachées. » et Tout afficher, les familles en deux colonnes de listes groupées (icône centrée dans sa pastille, titre, phrase, chevron)'
    Envoyer @(Action 'famille:batterie' $null 'familles')
    Capturer 'famille-batterie-triee' 'Une famille triée pour un PC fixe (Batterie et charge)' 'La bande « ... pannes de PC portable sont cachées. », la seule panne qui reste'
    Envoyer @(Action 'tout-afficher' $null 'famille')
    Capturer 'famille-batterie-tout' 'La même famille après « Tout afficher »' 'Toutes les pannes de la famille, la bande « Toutes les pannes sont affichées... » et « Adapter à mon PC »'
    Envoyer @((Action 'adapter-pc' $null 'famille'), (Action 'familles' $null 'famille'))
    # L'écran « Ton PC » : ce PC, puis un autre PC choisi (genre, marque), puis le retour à ce PC
    Envoyer @(Action 'ton-pc' $null 'familles')
    Capturer 'ton-pc' 'Ton PC (ce PC)' 'À gauche ce que Mindcare a reconnu (PC fixe monté, ASUS, carte mère, processeur, mémoire, disque, Windows), chaque mot expliqué ; à droite « Tu regardes une panne pour un autre PC ? » : Un portable, Un PC fixe, Je ne sais pas'
    Envoyer @(Action 'pc-genre:portable' $null 'tonpc')
    Capturer 'ton-pc-marque' 'Ton PC : la marque de l''autre PC' '« Quelle est la marque du portable ? », Je ne sais pas puis les 22 marques en deux colonnes'
    Envoyer @(Action 'pc-marque:HP' $null 'tonpc-marque')
    Capturer 'ton-pc-autre' 'Ton PC, un autre PC choisi (portable HP)' 'La bande « Tu regardes les pannes d''un autre PC : portable HP. » et Revenir à mon PC, la pastille « Autre : Portable HP », « Choisi » sur Un portable'
    Envoyer @(Action 'familles' $null 'tonpc')
    Capturer 'familles-autre-pc' 'Ça concerne quoi ? pour l''autre PC' 'La bande « Adapté au PC choisi (portable HP) : ... pannes de PC fixe ou d''autres marques sont cachées. »'
    Envoyer @((Action 'ton-pc' $null 'familles'), (Action 'pc-mon-pc' $null 'tonpc'), (Action 'familles' $null 'tonpc'))
    Envoyer @(Action 'famille:allumage' $null 'familles')
    Capturer 'famille' 'Une famille (Allumage)' 'Les groupes de pannes en cartes dépliables sur deux colonnes, la première ouverte (bord de plasma) avec ses pannes'
    # (cinq fois vers le bas depuis la première panne : l'en-tête de la carte suivante de la colonne, puis Entrée)
    Touche 0x28; Touche 0x28; Touche 0x28; Touche 0x28; Touche 0x28; Touche 0x0D
    Capturer 'famille-deux-ouvertes' 'Une famille, une deuxième carte ouverte au clavier' 'Deux cartes ouvertes, chacune n''allonge que sa colonne'
    Enquete 'redemarre' @()
    Capturer 'etape-question' 'Une étape question (Il redémarre tout seul)' 'Le fil (famille › panne) et « Étape 1 sur N », les segments, la question en grand, les réponses deux par ligne, Je ne sais pas et Étape précédente en liens, les suspects juste en dessous'
    Enquete 'redemarre' @(1, 1, 0)
    Envoyer @((Action 'geste' ([ordered]@{ valeur = '0'; oui = $true }) 'enquete'), (Action 'geste' ([ordered]@{ valeur = '1'; oui = $true }) 'enquete'))
    Capturer 'etape-manipulation-gestes' 'Une manipulation (test de la mémoire), deux gestes cochés' 'Les gestes à cocher (deux cochés), la sécurité, les réponses, la jauge'
    # (« Les Paramètres de Windows ne s'ouvrent pas », la 2e réponse : l'étape « Réparer les fichiers de Windows », DISM puis sfc)
    Enquete 'parametres' @(1)
    Envoyer @(Action 'commande' '0' 'enquete')
    Capturer 'etape-commande-copiee' 'Une manipulation avec une commande à copier (sfc)' 'Les deux commandes à taper dans le Terminal (DISM, sfc /scannow), le bouton de la première « Copiée »'
    foreach ($r in $reponses) {
        Enquete $r.panne $r.choix
        Capturer $r.nom "Réponse finale : $($r.badge) ($($r.panne))" "La carte « $($r.badge) » (pastille, couleur), la conclusion, Ce qu'il faut faire numéroté avec le conseil pour le réparateur, Comment Mindcare le sait replié, les boutons, la sécurité en bas"
    }
    # (la dernière réponse montrée : « À faire vérifier par un réparateur »)
    Envoyer @(Action 'reparateur' $null 'enquete')
    Capturer 'reparateur-avant' 'Pour le réparateur, avant Enregistrer' 'La panne, la réponse et la date en haut ; à gauche ce qui est sûr, écarté, à vérifier, les questions ; à droite le PC (une enquête pas faite sur ce PC : rien de lu) et ce que Windows a noté ; Copier et Enregistrer sur le Bureau en haut à droite'
    [EssaiCaptures]::Molette($script:cible, 760, 500, -10); Start-Sleep -Milliseconds 600
    Capturer 'reparateur-avant-bas' 'Pour le réparateur, défilé vers le bas' 'La suite du rapport : les réponses une par une (la question, la réponse dessous), Fait avec Mindcare, la note'
    [EssaiCaptures]::Molette($script:cible, 760, 500, 10); Start-Sleep -Milliseconds 400
    Envoyer @(Action 'bureau' $null 'reparateur')
    $fichiers = @(Get-ChildItem -LiteralPath (Join-Path $script:local 'Bureau') -Filter '*.txt' -ErrorAction SilentlyContinue)
    Verifier ($fichiers.Count -eq 1) "un rapport sur le faux Bureau ($(@($fichiers | ForEach-Object Name) -join ', '))"
    Capturer 'reparateur-enregistre' 'Pour le réparateur, après Enregistrer' 'Une seule fois « C''est fait : le fichier ... est sur le Bureau » (en haut du contenu, pas de bulle sur le titre), le rapport en dessous'
    Envoyer @(Action 'copier' $null 'reparateur')
    Capturer 'reparateur-copie' 'Pour le réparateur, après Copier' '« Le rapport est copié... » une seule fois, le bouton « Copié »'
    Envoyer @(Action 'aide' $null 'reparateur')
    Capturer 'aide' 'Comment ça marche' 'Les explications de Mindcare'
    # Une enquête laissée en cours (Je le ferai plus tard), puis l'accueil avec « Reprendre l'enquête »
    Enquete 'memoiretraining' @(1)
    Envoyer @(Action 'plus-tard' $null 'enquete') 300
    Capturer 'accueil-plus-tard' 'Accueil après « Je le ferai plus tard »' 'La bulle « Mindcare s''en souviendra... » et la tuile Reprendre l''enquête' 300
    Start-Sleep -Milliseconds 5000
    Capturer 'accueil-reprendre' 'Accueil avec Reprendre l''enquête' 'La tuile « Reprendre l''enquête » avec la panne en cours'
    Touche 0x27; Touche 0x27
    Capturer 'accueil-reprendre-choisie' 'Accueil, la tuile Reprendre l''enquête choisie' 'La tuile au centre, son titre et sa ligne (la panne, commencée le...) lisibles'
    Envoyer @(Action 'reprendre' $null 'accueil')
    Capturer 'reprise' 'L''enquête reprise' 'La même question qu''avant « plus tard »'
    # Une enquête sur ce PC jusqu'à sa réponse (« Je ne sais pas » à tout), puis son rapport : la partie « Le PC »
    # remplie avec la fiche du PC de l'essai
    $m = @((Action 'panne:ecranbleu' $null 'raconte'), (Rep 0), (Rep 0))
    foreach ($k in 1..15) { $m += Rep -1 }
    Envoyer $m 1500
    Envoyer @(Action 'reparateur' $null 'enquete')
    Capturer 'reparateur-ce-pc' 'Pour le réparateur, une enquête faite sur ce PC' 'À droite, « Le PC » rempli : Type (PC fixe monté), carte mère, processeur, mémoire installée, disque, Windows ; les barrettes'
    Quitter

    # Sans catalogue des enquêtes
    Preparer $script:theme $false
    Lancer "Thème $($script:theme), sans catalogue"
    Capturer 'sans-catalogue-accueil' 'Accueil sans catalogue' 'Une seule tuile rouge « Mindcare n''a pas pu lire ses enquêtes »'
    Envoyer @([ordered]@{ kit = 'aller'; ecran = 'probleme' })
    Capturer 'sans-catalogue-page' 'La page du problème' '« Son fichier est absent ou abîmé. Réinstalle Mindcare depuis Mindset. »'
    Quitter
}

try {
    foreach ($t in $Themes) {
        $script:theme = $t
        $script:dossierTheme = Join-Path $Resultats ($t -replace '^pack:', 'perso-')
        New-Item -ItemType Directory -Force -Path $script:dossierTheme | Out-Null
        if (-not $Seulement.Count) {
            Get-ChildItem -LiteralPath $script:dossierTheme -File | Remove-Item -Force
        }
        try { Parcours-Theme }
        catch { Echec "thème $t : erreur du script : $_ ($($_.InvocationInfo.PositionMessage))" }
        finally { Quitter }
    }
}
finally {
    if ($index.Count) {
        $entete = @('Captures de Mindcare en Rust (étape 5.5), fenêtre de 1100 x 760 sur un bureau Windows à part.', 'fichier | écran | ce qu''on doit y voir', '')
        $vieil = Join-Path $Resultats 'index.txt'
        $deja = @()
        if ($Seulement.Count -and (Test-Path -LiteralPath $vieil)) {
            $nouveaux = @($index | ForEach-Object { ($_ -split ' \| ')[0] })
            $deja = @([IO.File]::ReadAllLines($vieil, [Text.Encoding]::UTF8) | Select-Object -Skip 3 | Where-Object { $_ -and $nouveaux -notcontains (($_ -split ' \| ')[0]) })
        }
        $toutes = @($deja + $index | Sort-Object)
        [IO.File]::WriteAllText($vieil, (($entete + $toutes) -join "`r`n") + "`r`n", (New-Object Text.UTF8Encoding($true)))
    }
    $restants = @()
    if ($script:exeEssai) { $restants = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $script:exeEssai }) }
    if ($script:racine) { $restants += @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" | Where-Object { $_.CommandLine -like "*$($script:racine)*" }) }
    Verifier ($restants.Count -eq 0) "plus aucun processus de l'essai"
    foreach ($r in $restants) { try { Stop-Process -Id $r.ProcessId -Force -ErrorAction Stop } catch {} }
    if ($script:racine) { try { Remove-Item -LiteralPath $script:racine -Recurse -Force -ErrorAction Stop } catch { Write-Output "Dossier d'essai pas encore effacé : $($script:racine)" } }
}
if ($echecs.Count) { Write-Output "$($echecs.Count) échec(s)."; exit 1 }
Write-Output 'Tout est bon.'
exit 0
