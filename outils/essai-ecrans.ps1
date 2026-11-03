# Essai des écrans de Mindcare en Rust (étape 5.3), avec la CONSTRUCTION D'ESSAI. À lancer SEULEMENT sur un bureau
# Windows à part (outils\sur-bureau.ps1) :
#
#   & '.\outils\sur-bureau.ps1' -Script '.\outils\essai-ecrans.ps1' -Arguments "-Exe <Mindset.exe d'essai> -Resultats <dossier>" -Secondes 600
#
# Une vraie enquête jouée au clavier (messages de clavier envoyés à la fenêtre de WebView2, comme une manette :
# flèches, Entrée, Échap) jusqu'à la réponse finale, puis « Pour le réparateur », « Enregistrer le rapport sur le
# Bureau » (le FAUX Bureau de l'essai) et « Copier le rapport » (le FAUX presse-papiers de l'essai). Une capture à
# chaque écran. Puis une enquête laissée en cours (« Je le ferai plus tard »), l'appli relancée, et l'enquête reprise
# à la même question. etat.json (la carte de Mindcare dans Mindset) doit dire la vraie situation.
#
# Rien du vrai PC : un dossier d'essai dans %TEMP% (données, faux %LOCALAPPDATA%, faux Bureau, faux presse-papiers),
# les verrou et signal d'ESSAI (Local\MindcareEssai-...), jamais ceux de la vraie Mindcare ni de Mindset.
#
# -Etapes : seulement une suite de touches, pour explorer (D G H B : flèches droite, gauche, haut, bas ; E : Entrée ;
# X : Échap ; C<nom> : capture ; W<ms> : attendre ; K<x>:<y> : un clic), séparées par des virgules ; « apres, » en tête :
# après l'enquête « Rien ne s'allume ».
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Resultats,
    [string]$Etapes = ''
)
$ErrorActionPreference = 'Stop'
$echecs = New-Object System.Collections.Generic.List[string]
# (Write-Host : les fonctions qui rendent une valeur ne mélangent pas leurs messages à leur résultat)
function Ok([string]$t) { Write-Host "OK : $t" }
function Echec([string]$t) { Write-Host "ÉCHEC : $t"; $echecs.Add($t) }
function Verifier([bool]$c, [string]$t) { if ($c) { Ok $t } else { Echec $t } }

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class EssaiEcrans
{
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int l, t, r, b; }
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
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
    public static int Largeur(IntPtr h) { RECT r; GetClientRect(h, out r); return r.r - r.l; }
    public static int Hauteur(IntPtr h) { RECT r; GetClientRect(h, out r); return r.b - r.t; }
    public static void Clic(IntPtr h, int x, int y)
    {
        IntPtr pos = (IntPtr)((y << 16) | (x & 0xFFFF));
        PostMessage(h, 0x0200, IntPtr.Zero, pos);          // WM_MOUSEMOVE
        PostMessage(h, 0x0201, (IntPtr)1, pos);            // WM_LBUTTONDOWN (MK_LBUTTON)
        PostMessage(h, 0x0202, IntPtr.Zero, pos);          // WM_LBUTTONUP
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
if (-not $texte.Contains('MINDCARE_ESSAI_NOM')) { Write-Output "ÉCHEC : $Exe n'est pas la construction d'essai : rien n'est lancé."; exit 1 }
$depot = Split-Path $PSScriptRoot -Parent
$racine = Join-Path $env:TEMP "mindcare-essai-ecrans-$PID"
if (Test-Path -LiteralPath $racine) { Remove-Item -LiteralPath $racine -Recurse -Force }
$programmes = Join-Path $racine 'Programmes\Mindcare'
$donnees = Join-Path $racine 'Donnees\Mindcare'
$local = Join-Path $racine 'Local'
New-Item -ItemType Directory -Force -Path $programmes, $donnees, (Join-Path $local 'Mindset') | Out-Null
$exeEssai = Join-Path $programmes 'Mindset.exe'
Copy-Item -LiteralPath $Exe -Destination $exeEssai
# (le catalogue des enquêtes, à côté du programme comme dans le dossier construit)
Copy-Item -LiteralPath (Join-Path $depot 'Mindcare\fichiers\enquetes.json') -Destination (Join-Path $programmes 'enquetes.json')
New-Item -ItemType Directory -Force -Path $Resultats | Out-Null
$nomEssai = "ecrans-$PID"
$j = [ordered]@{ theme = 'neon'; micro = ''; casque = ''; applis = [ordered]@{ mindcare = $programmes }; modifie = (Get-Date -Format s) }
[IO.File]::WriteAllText((Join-Path $local 'Mindset\suite.json'), ($j | ConvertTo-Json -Depth 4), (New-Object Text.UTF8Encoding($false)))
$journal = Join-Path $donnees 'journal.txt'
$bureau = Join-Path $local 'Bureau'
$pressePapiers = Join-Path $local 'presse-papiers.txt'
$historique = Join-Path $donnees 'diagnostics.json'
$etatJson = Join-Path $donnees 'etat.json'

# Le journal lu sans gêner l'appli qui y écrit (FileShare.ReadWrite : ReadAllLines le bloquerait en écriture le temps
# de la lecture, et la ligne écrite à ce moment-là serait perdue)
function Get-Journal {
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
function Lire-Json([string]$f) { if (Test-Path -LiteralPath $f) { [IO.File]::ReadAllText($f, [Text.Encoding]::UTF8).TrimStart([char]0xFEFF) | ConvertFrom-Json } else { $null } }
$script:nCapture = 0
function Capturer([string]$Nom) {
    $script:nCapture++
    $fichier = '{0:D2}-{1}.png' -f $script:nCapture, $Nom
    $c = Join-Path $donnees 'capture.png'
    if (Test-Path -LiteralPath $c) { Remove-Item -LiteralPath $c -Force }
    Start-Sleep -Milliseconds 900   # (les animations d'entrée du kit)
    [IO.File]::WriteAllText((Join-Path $donnees 'capture.demande'), '1')
    $ok = Attendre { Test-Path -LiteralPath $c } 5000
    Verifier $ok "capture $fichier"
    if ($ok) { Start-Sleep -Milliseconds 300; Copy-Item -LiteralPath $c -Destination (Join-Path $Resultats $fichier) -Force; Write-Host "Capture : $(Join-Path $Resultats $fichier)" }
}

# La dernière action du kit notée au journal (« Écran : action <id> (écran <écran>, valeur <valeur>) »)
function Derniere-Action {
    $l = @(Get-Journal | Where-Object { $_ -like '*Écran : action *' })
    if ($l.Count) { $l[-1] } else { '' }
}
$script:p = $null
$script:cible = [IntPtr]::Zero
function Lancer([string]$Quoi) {
    $psi = New-Object Diagnostics.ProcessStartInfo $exeEssai
    $psi.Arguments = '-Demarrage'
    $psi.UseShellExecute = $false
    $psi.WorkingDirectory = $programmes
    $psi.EnvironmentVariables['MINDCARE_DONNEES'] = $donnees
    $psi.EnvironmentVariables['MINDCARE_ESSAI_LOCAL'] = $local
    $psi.EnvironmentVariables['MINDCARE_ESSAI_NOM'] = $nomEssai
    $prets = Compter 'Écran prêt'
    $script:p = [Diagnostics.Process]::Start($psi)
    Write-Host "$Quoi : processus $($script:p.Id)"
    $pid2 = $script:p.Id
    $vu = Attendre { @([EssaiEcrans]::Fenetres($pid2) | Where-Object { [EssaiEcrans]::Classe($_) -ceq 'Tauri Window' -and [EssaiEcrans]::Titre($_) -ceq 'Mindcare' }).Count -ge 1 } 20000
    Verifier $vu "$Quoi : fenêtre « Mindcare »"
    $f = @([EssaiEcrans]::Fenetres($pid2) | Where-Object { [EssaiEcrans]::Classe($_) -ceq 'Tauri Window' -and [EssaiEcrans]::Titre($_) -ceq 'Mindcare' })[0]
    Verifier (Attendre { (Compter 'Écran prêt') -gt $prets } 20000) "$Quoi : le kit est prêt"
    # Le signal d'ESSAI (Local\MindcareEssai-...) la montre
    $s = [Threading.EventWaitHandle]::OpenExisting("Local\MindcareEssai-$nomEssai-Afficher"); [void]$s.Set(); $s.Dispose()
    Verifier (Attendre { [EssaiEcrans]::IsWindowVisible($f) } 5000) "$Quoi : montrée par le signal d'essai"
    $script:cible = [IntPtr]::Zero
    [void](Attendre { $script:cible = @([EssaiEcrans]::Enfants($f, 'Chrome_RenderWidgetHostHWND'))[0]; $null -ne $script:cible -and $script:cible -ne [IntPtr]::Zero } 5000)
    Verifier ($script:cible -ne [IntPtr]::Zero) "$Quoi : fenêtre de rendu de WebView2"
    Start-Sleep -Milliseconds 3000
}
function Quitter {
    [IO.File]::WriteAllText((Join-Path $donnees 'quitter.demande'), "$PID")
    if (-not $script:p.WaitForExit(10000)) { try { $script:p.Kill() } catch {} ; Echec 'l''appli ne quitte pas sur quitter.demande' }
    Start-Sleep -Milliseconds 1500
}
$VK = @{ D = 0x27; G = 0x25; H = 0x26; B = 0x28; E = 0x0D; X = 0x1B }
function Touche([string]$k) { [EssaiEcrans]::Touche($script:cible, $VK[$k]); Start-Sleep -Milliseconds 350 }
# Des touches, puis la nouvelle action attendue au journal (rien : $null)
function Agir([string[]]$Touches, [int]$Ms = 3000) {
    $avant = @(Get-Journal | Where-Object { $_ -like '*Écran : action *' }).Count
    foreach ($k in $Touches) { Touche $k }
    if (Attendre { @(Get-Journal | Where-Object { $_ -like '*Écran : action *' }).Count -gt $avant } $Ms) { Derniere-Action } else { $null }
}

# Une enquête jouée au clavier jusqu'à la réponse finale : à chaque étape, la réponse choisie par $Choix (« D » : la
# deuxième réponse, « E » : la première) ; sur une manipulation (gestes à cocher), les gestes sont passés jusqu'aux
# réponses. Une capture par étape. Vrai si la réponse finale est atteinte (« Réponse ... » au journal).
# Une réponse à l'étape montrée : ces touches, puis (une manipulation : des gestes à cocher d'abord) bas et Entrée
# jusqu'à une réponse. Rend la ligne de l'action au journal (rien : aucune réponse donnée).
function Repondre([string[]]$Cles) {
    $a = Agir $Cles
    for ($k = 0; $k -lt 15 -and -not ($a -match 'action (rep|nsp) '); $k++) { $a = Agir @('B', 'E') }
    if ($a -match 'action (rep|nsp) ') { $a } else { $null }
}

function Jouer-Enquete([string]$Nom, [scriptblock]$Choix, [int]$Debut = 1) {
    $reponses = Compter 'Réponse '
    for ($n = $Debut; $n -le 40; $n++) {
        Capturer "$Nom-etape$n"
        $a = Repondre $(if ((& $Choix $n) -eq 'E') { @('E') } else { @('D', 'E') })
        Write-Host "Étape $n : $a"
        if (-not $a) { Echec "$Nom : étape $n sans réponse possible au clavier"; return $false }
        if (Attendre { (Compter 'Réponse ') -gt $reponses } 400) { Start-Sleep -Milliseconds 500; return $true }
    }
    Echec "$Nom : pas de réponse finale en 40 étapes"
    $false
}

function Parcours-Rien {
    Capturer 'accueil'
    # « Ça concerne quoi ? », la famille Allumage, la panne « Rien ne s'allume »
    Touche 'D'; Touche 'E'
    Verifier (Attendre { (Compter 'Écran : montre familles') -ge 1 } 3000) 'tuile « Ça concerne quoi ? » : les familles'
    Capturer 'familles'
    $a = Agir @('E'); Verifier ($a -like '*action famille:allumage*') "famille Allumage ($a)"
    Capturer 'famille-allumage'
    $a = Agir @('E'); Verifier ($a -like '*action panne:rien*') "panne « Rien ne s'allume » ($a)"
    Verifier ((Compter 'Enquête : rien') -ge 1) 'enquête commencée (journal)'
    $fini = Jouer-Enquete 'rien' { param($n) 'D' }
    Verifier $fini 'enquête « Rien ne s''allume » jouée jusqu''à la réponse finale'
    Capturer 'rien-reponse'
}

function Lire-Texte([string]$f) { [IO.File]::ReadAllText($f, [Text.Encoding]::UTF8).TrimStart([char]0xFEFF) }

# Pour le réparateur : le rapport sur le FAUX Bureau, puis dans le FAUX presse-papiers (le même texte)
function Parcours-Rapport {
    # (« Pour le réparateur », en haut à droite de la réponse : un clic)
    [EssaiEcrans]::Clic($script:cible, 964, 115)
    Verifier (Attendre { (Compter 'Écran : montre reparateur') -ge 1 } 4000) '« Pour le réparateur » ouvre sa page (journal)'
    Capturer 'reparateur'
    $a = Agir @('E'); Verifier ($a -like '*action bureau (écran reparateur*') "« Pour le réparateur » ouvert, puis « Enregistrer le rapport sur le Bureau » ($a)"
    $fichiers = @(Get-ChildItem -LiteralPath $bureau -Filter '*.txt' -ErrorAction SilentlyContinue)
    Verifier ($fichiers.Count -eq 1) "un rapport sur le faux Bureau ($(@($fichiers | ForEach-Object Name) -join ', '))"
    Capturer 'reparateur-enregistre'
    if ($fichiers.Count -eq 1) {
        $o = [IO.File]::ReadAllBytes($fichiers[0].FullName)
        Verifier ($o[0] -eq 0xEF -and $o[1] -eq 0xBB -and $o[2] -eq 0xBF) 'rapport en UTF-8 avec BOM'
        $rapport = Lire-Texte $fichiers[0].FullName
        Verifier ($rapport.StartsWith("Mindcare : rapport de l'enquête`r`n") -and $rapport.Contains("Panne : Rien ne s'allume")) 'texte du rapport'
        Verifier ($fichiers[0].Name -like 'Mindcare, diagnostic du *.txt') "nom du rapport ($($fichiers[0].Name))"
        Verifier ((Compter "Rapport enregistré : $($fichiers[0].FullName)") -eq 1) 'rapport enregistré (journal)'
    }
    $a = Agir @('B', 'E')
    if (-not ($a -like '*action copier *')) { [EssaiEcrans]::Clic($script:cible, 213, 621); $a = Derniere-Action }
    Verifier ($a -like '*action copier *') "« Copier le rapport » ($a)"
    Verifier (Attendre { Test-Path -LiteralPath $pressePapiers } 3000) 'le rapport dans le faux presse-papiers'
    if ((Test-Path -LiteralPath $pressePapiers) -and $fichiers.Count -eq 1) {
        # (sauf la ligne « Date : », écrite au moment de chaque copie, comme Get-ResumeTexte : la minute peut changer)
        $sansDate = { param($t) ($t -split "`r`n" | Where-Object { $_ -notlike 'Date : *' }) -join "`r`n" }
        Verifier ((& $sansDate (Lire-Texte $pressePapiers)) -ceq (& $sansDate $rapport)) 'presse-papiers : le même texte que le fichier'
    }
    Capturer 'reparateur-copie'
    # La carte de Mindcare dans Mindset : la dernière enquête, finie
    $carte = Lire-Json $etatJson
    $lignes = @($carte.Lignes | ForEach-Object { $_.Texte }) -join ' | '
    Write-Host "etat.json : $lignes"
    Verifier ($lignes -like "*Rien ne s'allume*" -and $lignes -notlike '*Aucun diagnostic*') 'etat.json : la dernière enquête (avec le catalogue)'
}

# « Ça concerne quoi ? » dans la barre du haut (un clic), puis une famille et sa première panne
function Vers-Panne([int]$Famille, [string]$Nom) {
    $familles = Compter 'Écran : montre familles'
    [EssaiEcrans]::Clic($script:cible, 443, 31)
    Verifier (Attendre { (Compter 'Écran : montre familles') -gt $familles } 3000) 'les familles (barre du haut)'
    Start-Sleep -Milliseconds 800
    $cles = @(for ($i = 0; $i -lt $Famille; $i++) { 'D' }) + 'E'
    $a = Agir $cles; Verifier ($a -like '*action famille:*') "une famille ($a)"
    $a = Agir @('E'); Verifier ($a -like '*action panne:*') "une panne ($a)"
    Capturer "$Nom-debut"
}

function Parcours {
    Parcours-Rien
    Parcours-Rapport

    # 2. Sur ce PC : Windows lu (le journal Système, sans droits administrateur ; rien n'est modifié), au plus 30 s
    Vers-Panne 4 'plantages'
    $a = Repondre @('D', 'E'); Write-Host "Étape 1 : $a"
    Capturer 'plantages-etape2'
    $a = Repondre @('E'); Write-Host "Étape 2 : $a"
    Verifier ($a -like '*action rep *"0"*') "« Oui, sur ce PC-là » ($a)"
    if ((Compter 'Journaux de Windows') -eq 0) { Capturer 'plantages-attente-windows' }
    $t = [Diagnostics.Stopwatch]::StartNew()
    Verifier (Attendre { (Compter 'Journaux de Windows') -ge 1 } 35000) "Windows lu, ou laissé, au plus tard au bout de 30 s ($([math]::Round($t.Elapsed.TotalSeconds, 1)) s)"
    Get-Journal | Where-Object { $_ -like '*Journaux de Windows*' } | ForEach-Object { Write-Host "  $_" }
    $fini = Jouer-Enquete 'plantages' { param($n) 'E' } 3
    Verifier $fini 'enquête « Plantages » jouée jusqu''à la réponse finale'
    Capturer 'plantages-reponse'

    # 3. Une enquête laissée en cours, l'appli relancée, l'enquête reprise à la même question
    Vers-Panne 0 'laissee'
    [void](Repondre @('D', 'E')); [void](Repondre @('D', 'E'))
    Start-Sleep -Milliseconds 800
    Capturer 'laissee-avant-relance'
    $h = Lire-Json $historique
    $avant = @($h.Diagnostics)[0]
    Verifier ($avant -and -not $avant.Fini -and @($avant.Choix).Count -eq 2) "enquête en cours gardée (choix $(@($avant.Choix) -join ','))"
    Quitter
    $carte = Lire-Json $etatJson
    $lignes = @($carte.Lignes | ForEach-Object { $_.Texte }) -join ' | '
    Write-Host "etat.json : $lignes"
    Verifier ($lignes -like '*Pas finie*') 'etat.json : l''enquête en cours, à reprendre'
    Lancer 'Relancée'
    Capturer 'relance-accueil'
    $a = Agir @('G', 'G', 'G', 'D', 'D', 'E')
    Verifier ($a -like '*action reprendre *') "tuile « Reprendre l'enquête » ($a)"
    Verifier (Attendre { (Compter "Reprise de l'enquête $($avant.Id)") -ge 1 } 3000) 'reprise de la même enquête (journal)'
    Capturer 'reprise'
    $apres = @((Lire-Json $historique).Diagnostics)[0]
    Verifier ($apres.Id -eq $avant.Id -and (@($apres.Choix) -join ',') -eq (@($avant.Choix) -join ',')) 'la même enquête, aux mêmes réponses'
}

try {
    Lancer 'Lancée cachée'
    if ($Etapes) {
        if ($Etapes.StartsWith('apres,')) { Parcours-Rien; $Etapes = $Etapes.Substring(6) }
        Capturer 'depart'
        foreach ($e in $Etapes.Split(',')) {
            $e = $e.Trim()
            if ($e -match '^C(.+)$') { Capturer $Matches[1] }
            elseif ($e -match '^W(\d+)$') { Start-Sleep -Milliseconds ([int]$Matches[1]) }
            elseif ($e -match '^K(\d+):(\d+)$') { [EssaiEcrans]::Clic($script:cible, [int]$Matches[1], [int]$Matches[2]); Start-Sleep -Milliseconds 600; Write-Host "Clic $e : $(Derniere-Action)" }
            elseif ($VK.ContainsKey($e)) { Touche $e; Write-Host "Touche $e : $(Derniere-Action)" }
        }
    } else {
        Parcours
    }
}
catch { Echec "erreur du script : $_ ($($_.InvocationInfo.PositionMessage))" }
finally {
    if ($script:p -and -not $script:p.HasExited) { Quitter }
    Get-Journal | Set-Content -LiteralPath (Join-Path $Resultats 'journal-ecrans.txt') -Encoding UTF8
    foreach ($f in @($historique, $etatJson, $pressePapiers)) { if (Test-Path -LiteralPath $f) { Copy-Item -LiteralPath $f -Destination $Resultats -Force } }
    if (Test-Path -LiteralPath $bureau) { Get-ChildItem -LiteralPath $bureau | Copy-Item -Destination $Resultats -Force }
    Write-Output '--- Journal ---'
    Get-Journal | ForEach-Object { Write-Output "  $_" }
    $restants = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $exeEssai })
    $restants += @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" | Where-Object { $_.CommandLine -like "*$racine*" })
    Verifier ($restants.Count -eq 0) "plus aucun processus de l'essai"
    foreach ($r in $restants) { try { Stop-Process -Id $r.ProcessId -Force -ErrorAction Stop } catch {} }
    try { Remove-Item -LiteralPath $racine -Recurse -Force -ErrorAction Stop } catch { Write-Output "Dossier d'essai pas encore effacé : $racine" }
}
if ($echecs.Count) { Write-Output "$($echecs.Count) échec(s)."; exit 1 }
Write-Output 'Tout est bon.'
exit 0
