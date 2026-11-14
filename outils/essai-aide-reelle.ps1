# Mindcare : l'Aide de Mindcare (MindcareAide.exe, tâche SYSTEM « \Mindset\Mindcare (aide) ») essayée en vrai, sur la
# machine jetable de GitHub Actions (workflow .github\workflows\aide-reelle.yml, lancé à la main). Vraies tâches
# planifiées, vrais comptes Windows, vraie installation par l'installateur de Mindset, vraies réparations : JAMAIS sur
# un vrai PC. Le script refuse de tourner hors de GitHub Actions. Modèle : outils\essai-aide-reelle.ps1 de Nevermind.
#
#   .\outils\essai-aide-reelle.ps1 -Cas 0 -Aide <dossier>   préparation (le dossier construit par le workflow :
#                                                            MindcareAide.exe, Mindcare.exe et client_aide.exe)
#   .\outils\essai-aide-reelle.ps1 -Cas N                    un cas (1 à 9), voir la liste plus bas
#
# Le client est le VRAI client de Mindcare (rust\mindcare-appli\examples\client_aide.rs, qui se sert de
# windows\aide.rs : état de la tâche, réveil par schtasks, canal vérifié, journal recopié), mis dans le paquet d'essai
# à la place de Mindset.exe (le seul programme que l'Aide accepte), lancé sous le compte voulu. La clé de signature est
# une clé RSA de TEST créée ici, ajoutée aux clés publiques de la copie d'essai de Mindset ; son apps.json (copie
# d'essai seulement) reçoit les champs « aide » et « aide_exe » de Mindcare, comme la liste en ligne les donnera.
#
#   1. installation pour un compte standard au nom accentué (« Élodie Test ») : tâche SYSTEM, -Sid d'Élodie, droits
#   2. son Mindset.exe trouve la tâche (presente), la réveille, et « aide.bonjour » répond : SYSTEM, session 0, une
#      seule Aide, qui s'arrête seule une fois Mindcare fermé
#   3. un autre compte (Bruno) : ni la tâche ni le canal ; un autre programme d'Élodie : refusé, noté au journal
#   4. energie.par-defaut : un mode d'alimentation ajouté avant a disparu après, « Utilisation normale » actif
#   5. maj.vider deux fois (la seconde avec un .old déjà là) : dossier mis de côté, services comme avant
#   6. wifi.oublier sur une machine sans Wi-Fi : réponse propre
#   7. ip.renouveler (en dernier dans le workflow : la machine perd le réseau un instant) : nouvelle adresse, réseau revenu
#   8. désinstallation de Mindcare par l'installateur de Mindset : plus de dossier, plus de tâche (attend le correctif
#      de Mindset du 14/11 : l'action « supprimer » doit retirer la tâche \Mindset\<aide> prise dans apps.json)
#   9. la tâche SYSTEM ne voit pas l'environnement d'Élodie (variable témoin dans HKCU\Environment, contre-épreuve)
# Code de sortie 1 si un cas échoue.
param([int]$Cas = -1, [string]$Aide = '')

if ($env:GITHUB_ACTIONS -ne 'true') {
    Write-Host 'Refusé : cet essai crée des comptes Windows et des tâches planifiées, installe Mindcare et fait de vraies réparations de Windows. Il ne tourne que sur la machine jetable de GitHub Actions.' -ForegroundColor Red
    exit 2
}
$ErrorActionPreference = 'Stop'
$Racine = 'C:\essai'
$PowerShellExe = Join-Path ([Environment]::GetFolderPath('System')) 'WindowsPowerShell\v1.0\powershell.exe'
$Schtasks = Join-Path ([Environment]::GetFolderPath('System')) 'schtasks.exe'
$Powercfg = Join-Path ([Environment]::GetFolderPath('System')) 'powercfg.exe'
$NomTache = 'Mindcare (aide)'
$Depot = Split-Path $PSScriptRoot -Parent                 # le dépôt de Mindcare (checkout)
$MindsetGit = Join-Path $Racine 'mindset'                 # le dépôt de Mindset (checkout du workflow)
$MindsetDepot = Join-Path $Racine 'mindset-depot'         # sa copie d'essai, avec la clé de TEST
$Installateur = Join-Path $MindsetDepot 'Mindset\MindsetInstallateur.exe'
$Paquet = Join-Path $Racine 'paquet\Mindcare.zip'
$Sorties = Join-Path $Racine 'sorties'
$ProgrammesC = Join-Path $env:ProgramFiles 'Mindset'
$MindcareC = Join-Path $ProgrammesC 'Mindcare'
$ClientAilleurs = Join-Path $Racine 'appli\client_aide.exe'  # le même client, hors du dossier de Mindcare
$Equilibre = '381b4222-f694-41f0-9685-ff5bb260df2e'          # « Utilisation normale », le mode d'origine de Windows
$Windows = [Environment]::GetFolderPath('Windows')

$script:Reussis = 0
$script:Echoues = New-Object Collections.Generic.List[string]
function Write-Partie([string]$Titre) { Write-Host ''; Write-Host $Titre -ForegroundColor Cyan }
function Test-Cas([string]$Nom, [scriptblock]$Condition) {
    $detail = ''
    try { $r = @(& $Condition) } catch { $r = @(); $detail = "$($_.Exception.Message) $($_.ScriptStackTrace)" }
    if ($r.Count -and $r[-1] -is [bool] -and $r[-1]) {
        $script:Reussis++
        Write-Host "  OK     $Nom" -ForegroundColor Green
    } else {
        $script:Echoues.Add($Nom)
        Write-Host "  ÉCHEC  $Nom$(if ($detail) { " ($detail)" })" -ForegroundColor Red
        Show-Diagnostic
    }
}
function Show-Diagnostic {
    try {
        foreach ($t in @(Get-ScheduledTask -TaskPath '\Mindset\' -ErrorAction SilentlyContinue)) {
            $i = Get-ScheduledTaskInfo -TaskPath '\Mindset\' -TaskName $t.TaskName -ErrorAction SilentlyContinue
            Write-Host "         | tâche $($t.TaskName) [$($t.State)] compte $($t.Principal.UserId) ; dernier lancement $($i.LastRunTime) code $($i.LastTaskResult) ; $($t.Actions[0].Execute) $($t.Actions[0].Arguments)" -ForegroundColor DarkGray
        }
        foreach ($p in @(Get-AidesEnCours)) { Write-Host "         | Aide en cours : $($p.Id) compte $($p.Sid) session $($p.Session) $($p.Chemin)" -ForegroundColor DarkGray }
    } catch {}
}
function Read-Json([string]$Fichier) {
    if (-not (Test-Path -LiteralPath $Fichier)) { return $null }
    try { [IO.File]::ReadAllText($Fichier, [Text.Encoding]::UTF8).TrimStart([char]0xFEFF) | ConvertFrom-Json } catch { $null }
}
function Write-Utf8([string]$Fichier, [string]$Texte) { [IO.File]::WriteAllText($Fichier, $Texte, (New-Object Text.UTF8Encoding $false)) }
function Wait-Jusque([scriptblock]$Condition, [int]$Secondes = 60) {
    $limite = (Get-Date).AddSeconds($Secondes)
    while ((Get-Date) -lt $limite) { if (& $Condition) { return $true }; Start-Sleep -Milliseconds 500 }
    [bool](& $Condition)
}
function New-Signature([string]$Fichier) {
    $rsa = New-Object Security.Cryptography.RSACryptoServiceProvider
    try {
        $rsa.PersistKeyInCsp = $false
        $rsa.FromXmlString([IO.File]::ReadAllText((Join-Path $Racine 'cle\privee-test.xml')))
        [IO.File]::WriteAllBytes("$Fichier.sig", $rsa.SignData([IO.File]::ReadAllBytes($Fichier), [Security.Cryptography.CryptoConfig]::MapNameToOID('SHA256')))
    } finally { $rsa.Dispose() }
}

# Un compte Windows local de test (créé, ou mot de passe renouvelé), avec son profil
function Get-Compte([string]$Nom) {
    $mdp = 'Aa1!' + [guid]::NewGuid().ToString('N').Substring(0, 14)
    $s = ConvertTo-SecureString $mdp -AsPlainText -Force
    $u = Get-LocalUser -Name $Nom -ErrorAction SilentlyContinue
    if ($u) { Set-LocalUser -Name $Nom -Password $s } else { $u = New-LocalUser -Name $Nom -Password $s -PasswordNeverExpires -AccountNeverExpires }
    $cred = New-Object Management.Automation.PSCredential(".\$Nom", $s)
    $p = Start-Process -FilePath "$env:SystemRoot\System32\cmd.exe" -ArgumentList '/c', 'exit' -Credential $cred -LoadUserProfile -WorkingDirectory $Racine -PassThru -WindowStyle Hidden
    [void]$p.WaitForExit(60000)
    $sid = $u.SID.Value
    $profil = [string](Get-Item -LiteralPath "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$sid").GetValue('ProfileImagePath')
    @{ Nom = $Nom; Sid = $sid; Cred = $cred; Profil = $profil }
}
$NomElodie = "$([char]0xC9)lodie Test"

# Ce que Windows dit de la tâche (XML exporté, droits, dernier lancement)
function Get-InfoTache {
    $xml = $null
    try { $xml = [xml](Export-ScheduledTask -TaskPath '\Mindset\' -TaskName $NomTache -ErrorAction Stop) } catch { return $null }
    $pr = $xml.Task.Principals.Principal
    $i = Get-ScheduledTaskInfo -TaskPath '\Mindset\' -TaskName $NomTache -ErrorAction SilentlyContinue
    $sddl = ''
    try { $s = New-Object -ComObject Schedule.Service; $s.Connect(); $sddl = $s.GetFolder('\Mindset').GetTask($NomTache).GetSecurityDescriptor(4) } catch {}
    [pscustomobject]@{
        Compte = [string]$pr.UserId; Commande = [string]$xml.Task.Actions.Exec.Command; Arguments = [string]$xml.Task.Actions.Exec.Arguments
        Active = ([string]$xml.Task.Settings.Enabled -ne 'false'); Dernier = $(if ($i) { $i.LastRunTime } else { $null }); Sddl = [string]$sddl
    }
}
function ConvertTo-Sid([string]$Compte) {
    if ($Compte -match '^S-1-') { return $Compte }
    try { (New-Object Security.Principal.NTAccount($Compte)).Translate([Security.Principal.SecurityIdentifier]).Value } catch { '' }
}
# Les droits sur la tâche : le système et les administrateurs, et l'utilisateur $Sid en lecture et lancement ; personne d'autre
function Test-DroitsTache([string]$Sddl, [string]$Sid) {
    $lui = $false
    foreach ($m in [regex]::Matches($Sddl, '\(([A-Z]+);[^;]*;([^;]*);[^;]*;[^;]*;([^)]+)\)')) {
        $type = $m.Groups[1].Value; $droits = $m.Groups[2].Value; $qui = $m.Groups[3].Value
        $qui = switch ($qui) { 'SY' { 'S-1-5-18' } 'BA' { 'S-1-5-32-544' } default { $qui } }
        if ($type -ne 'A' -or $qui -in 'S-1-5-18', 'S-1-5-32-544') { continue }
        if ($qui -eq $Sid -and $droits -in 'FRFX', '0x1200a9') { $lui = $true; continue }
        Write-Host "         | droit de trop sur la tâche : $($m.Value)" -ForegroundColor DarkGray
        return $false
    }
    $lui
}
# Les Aides de Mindcare qui tournent : PID, compte, session
function Get-AidesEnCours {
    foreach ($p in @(Get-CimInstance Win32_Process -Filter "Name='MindcareAide.exe'" -ErrorAction SilentlyContinue)) {
        $sid = ''; try { $sid = [string](Invoke-CimMethod -InputObject $p -MethodName GetOwnerSid -ErrorAction Stop).Sid } catch {}
        [pscustomobject]@{ Id = [int]$p.ProcessId; Sid = $sid; Session = [int]$p.SessionId; Chemin = [string]$p.ExecutablePath }
    }
}
# Le canal de l'Aide : celui de ce compte et de ce dossier (même calcul que mindcare-aide-regles, canal.rs)
function Get-NomCanal([string]$Sid, [string]$Dossier) {
    $d = [IO.Path]::GetFullPath($Dossier).TrimEnd('\').ToLowerInvariant()
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $e = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($d)) } finally { $sha.Dispose() }
    "Mindcare-Aide-$Sid-$((($e[0..5]) | ForEach-Object { $_.ToString('x2') }) -join '')"
}
function Test-Canal([string]$Nom) { [bool]@([IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -like "*\$Nom" }).Count }
function Stop-Aides {
    foreach ($t in @(Get-ScheduledTask -TaskPath '\Mindset\' -ErrorAction SilentlyContinue)) { try { Stop-ScheduledTask -TaskPath '\Mindset\' -TaskName $t.TaskName -ErrorAction SilentlyContinue } catch {} }
    foreach ($p in @(Get-AidesEnCours)) { try { Stop-Process -Id $p.Id -Force -ErrorAction Stop } catch { & "$env:SystemRoot\System32\taskkill.exe" /F /PID $p.Id 2>&1 | Out-Null } }
    $null = Wait-Jusque { -not @(Get-AidesEnCours).Count } 20
}
# Remet la machine à zéro : ni Mindcare, ni tâche \Mindset\
function Reset-Machine {
    Stop-Aides
    foreach ($p in @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)) {
        $c = [string]$p.ExecutablePath
        if ($c -like "$ProgrammesC\*" -or $c -like "$Racine\appli\*") { try { Stop-Process -Id $p.ProcessId -Force } catch {} }
    }
    Start-Sleep -Milliseconds 500
    foreach ($t in @(Get-ScheduledTask -TaskPath '\Mindset\' -ErrorAction SilentlyContinue)) { Unregister-ScheduledTask -TaskPath '\Mindset\' -TaskName $t.TaskName -Confirm:$false -ErrorAction SilentlyContinue }
    if (Test-Path -LiteralPath $ProgrammesC) { & "$env:SystemRoot\System32\takeown.exe" /F $ProgrammesC /R /D Y 2>&1 | Out-Null; & "$env:SystemRoot\System32\icacls.exe" $ProgrammesC /reset /T /C /Q 2>&1 | Out-Null; Remove-Item -LiteralPath $ProgrammesC -Recurse -Force -ErrorAction SilentlyContinue }
    Remove-Item -Path 'HKLM:\SOFTWARE\Mindset' -Recurse -Force -ErrorAction SilentlyContinue
}

# L'installateur de Mindset (MindsetInstallateur.exe, comme après une demande de Windows acceptée)
function Invoke-Installateur([string[]]$Arguments) {
    $f = Join-Path $Sorties "installateur-$([guid]::NewGuid().ToString('N').Substring(0, 8)).json"
    $p = Start-Process -FilePath $Installateur -ArgumentList (@($Arguments) + @('-Resultat', "`"$f`"")) -WorkingDirectory $env:SystemRoot -PassThru -Wait -WindowStyle Hidden
    $r = Read-Json $f
    Write-Host "         | installateur $($Arguments[1]) (code $($p.ExitCode)) : $($r.ok) $($r.message)" -ForegroundColor DarkGray
    $r
}
function Install-Mindcare([string]$Sid) {
    Invoke-Installateur @('-Action', 'appli', '-Appli', 'mindcare', '-Zip', "`"$Paquet`"", '-Signature', "`"$Paquet.sig`"",
        '-Destination', "`"$MindcareC`"", '-DossierZip', 'Mindcare', '-Exe', 'Mindcare.exe', '-TacheAide', "`"$NomTache`"",
        '-ExeAide', 'MindcareAide.exe', '-Sid', $Sid)
}

# Le client (le vrai client de Mindcare, examples\client_aide.rs) sous le compte $Compte : sa sortie JSON
function Invoke-Client($Compte, [string[]]$Arguments, [string]$Exe = (Join-Path $MindcareC 'Mindset.exe'), [switch]$Fond) {
    $sortie = Join-Path $Sorties "client-$([guid]::NewGuid().ToString('N').Substring(0, 8)).json"
    $a = @("`"$sortie`"") + @($Arguments | ForEach-Object { "`"$_`"" })
    $p = Start-Process -FilePath $Exe -ArgumentList $a -Credential $Compte.Cred -LoadUserProfile -WorkingDirectory $env:SystemRoot -PassThru -WindowStyle Hidden
    if ($Fond) { Start-Sleep -Seconds 2; return $p }
    if (-not $p.WaitForExit(420000)) { try { $p.Kill() } catch {}; Write-Host '         | client arrêté (trop long)' -ForegroundColor DarkGray }
    $s = Read-Json $sortie
    Write-Host "         | client $($Arguments -join ' ') ($($Compte.Nom), code $($p.ExitCode)) : $(if ($s) { $s | ConvertTo-Json -Compress -Depth 8 } else { 'pas de sortie' })" -ForegroundColor DarkGray
    $s
}
function Get-Reponse($S, [string]$Action) { if ($S -and $S.reponses -and $S.reponses.PSObject.Properties[$Action]) { $S.reponses.$Action } else { $null } }

# =====================================================================================================================
# 0. Préparation
# =====================================================================================================================
function Initialize-Essai {
    Write-Partie '0. Préparation'
    foreach ($d in $Racine, "$Racine\cle", "$Racine\paquet", $Sorties, "$Racine\appli") { New-Item -ItemType Directory -Force -Path $d | Out-Null }
    # Tout le monde lit C:\essai ; les clients (autres comptes) écrivent leurs sorties
    & "$env:SystemRoot\System32\icacls.exe" $Racine /grant '*S-1-1-0:(OI)(CI)RX' /Q | Out-Null
    & "$env:SystemRoot\System32\icacls.exe" $Sorties /grant '*S-1-1-0:(OI)(CI)M' /Q | Out-Null
    $rsa = New-Object Security.Cryptography.RSACryptoServiceProvider(3072)
    try {
        $rsa.PersistKeyInCsp = $false
        Write-Utf8 (Join-Path $Racine 'cle\privee-test.xml') $rsa.ToXmlString($true)
        Write-Utf8 (Join-Path $Racine 'cle\publique-test.xml') $rsa.ToXmlString($false)
    } finally { $rsa.Dispose() }

    # La copie d'essai de Mindset : son dossier Mindset (apps.json, clés), MindsetInstallateur.exe construit par le
    # workflow, la clé de TEST, et les champs « aide » de Mindcare dans son apps.json (rien n'est changé dans le dépôt
    # de Mindset : seulement cette copie, sur cette machine jetable)
    if (Test-Path -LiteralPath $MindsetDepot) { Remove-Item -LiteralPath $MindsetDepot -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $MindsetDepot | Out-Null
    Copy-Item -LiteralPath (Join-Path $MindsetGit 'Mindset') -Destination $MindsetDepot -Recurse
    if (-not $env:MINDSET_INSTALLATEUR -or -not (Test-Path -LiteralPath $env:MINDSET_INSTALLATEUR -PathType Leaf)) { throw 'MindsetInstallateur.exe absent : la variable MINDSET_INSTALLATEUR doit donner l''exe construit par le workflow.' }
    Copy-Item -LiteralPath $env:MINDSET_INSTALLATEUR -Destination $Installateur -Force
    Copy-Item -LiteralPath (Join-Path $Racine 'cle\publique-test.xml') -Destination (Join-Path $MindsetDepot 'Mindset\fichiers\cles-publiques\essai-test.xml')
    $fa = Join-Path $MindsetDepot 'Mindset\fichiers\apps.json'
    $texte = [IO.File]::ReadAllText($fa, [Text.Encoding]::UTF8)
    $applis = @($texte.TrimStart([char]0xFEFF) | ConvertFrom-Json)
    $mc = @($applis | Where-Object { $_.id -eq 'mindcare' })
    if ($mc.Count -ne 1) { throw 'Mindcare introuvable dans apps.json de Mindset.' }
    foreach ($c in @(@('aide', $NomTache), @('aide_exe', 'MindcareAide.exe'))) {
        if ($mc[0].PSObject.Properties[$c[0]]) { $mc[0].($c[0]) = $c[1] } else { $mc[0] | Add-Member -NotePropertyName $c[0] -NotePropertyValue $c[1] }
    }
    Write-Utf8 $fa (ConvertTo-Json -InputObject $applis -Depth 10)
    Test-Cas "Mindset d'essai prêt ($(& git -C $MindsetGit log --oneline -1)) : MindsetInstallateur.exe, clé de TEST, apps.json avec « aide » pour Mindcare" {
        $a = @(Read-Json $fa | Where-Object { $_.id -eq 'mindcare' })[0]
        (Test-Path -LiteralPath $Installateur) -and $a.aide -eq $NomTache -and $a.aide_exe -eq 'MindcareAide.exe'
    }

    # Le paquet d'essai : le dossier Mindcare d'une version, avec le client à la place de Mindset.exe
    $src = [IO.Path]::GetFullPath($Aide)
    foreach ($e in 'MindcareAide.exe', 'Mindcare.exe', 'client_aide.exe') { if (-not (Test-Path -LiteralPath (Join-Path $src $e) -PathType Leaf)) { throw "$e absent de $src" } }
    $d = Join-Path $Racine 'paquet\Mindcare'
    if (Test-Path -LiteralPath $d) { Remove-Item -LiteralPath $d -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    Copy-Item -LiteralPath (Join-Path $src 'client_aide.exe') -Destination (Join-Path $d 'Mindset.exe')
    Copy-Item -LiteralPath (Join-Path $src 'Mindcare.exe') -Destination $d
    Copy-Item -LiteralPath (Join-Path $src 'MindcareAide.exe') -Destination $d
    $section = ''; $version = $null
    foreach ($l in [IO.File]::ReadAllLines((Join-Path $Depot 'rust\mindcare-appli\Cargo.toml'))) {
        $l = $l.Trim(); if ($l.StartsWith('[')) { $section = $l; continue }
        if ($section -eq '[package]' -and $l -match '^version\s*=\s*"([^"]*)"') { $version = $Matches[1]; break }
    }
    Write-Utf8 (Join-Path $d 'version.json') (@{ appli = 'mindcare'; version = $version; mindset_min = '2.15.0' } | ConvertTo-Json -Compress)
    Copy-Item -LiteralPath (Join-Path $Depot 'Mindcare\LISEZMOI.txt') -Destination $d
    Copy-Item -LiteralPath (Join-Path $Depot 'Mindcare\fichiers\enquetes.json') -Destination $d
    Remove-Item -LiteralPath $Paquet -Force -ErrorAction SilentlyContinue
    Compress-Archive -LiteralPath $d -DestinationPath $Paquet -Force
    New-Signature $Paquet
    Copy-Item -LiteralPath (Join-Path $src 'client_aide.exe') -Destination $ClientAilleurs -Force
    Test-Cas "paquet d'essai signé avec la clé de TEST (Mindcare $version, MindcareAide.exe SHA-256 $((Get-FileHash -LiteralPath (Join-Path $d 'MindcareAide.exe') -Algorithm SHA256).Hash))" {
        (Test-Path -LiteralPath $Paquet) -and (Test-Path -LiteralPath "$Paquet.sig")
    }
}

# =====================================================================================================================
# 1. Installation : la tâche SYSTEM pour Élodie
# =====================================================================================================================
function Test-Cas1 {
    Write-Partie '1. Installation pour un compte standard au nom accentué : la tâche de l''Aide en SYSTEM'
    Reset-Machine
    $e = Get-Compte $NomElodie
    $r = Install-Mindcare $e.Sid
    Test-Cas 'Mindcare installé par l''installateur de Mindset, MindcareAide.exe à côté de Mindset.exe' { $r.ok -and (Test-Path -LiteralPath "$MindcareC\MindcareAide.exe") -and (Test-Path -LiteralPath "$MindcareC\Mindset.exe") }
    Test-Cas 'tâche « \Mindset\Mindcare (aide) » : SYSTEM, MindcareAide.exe -Sid <Élodie>, active, lue et lancée par Élodie seule' {
        $t = Get-InfoTache
        if (-not $t) { return $false }
        Write-Host "         | tâche : $($t.Compte) ; $($t.Commande) $($t.Arguments) ; $($t.Sddl)" -ForegroundColor DarkGray
        (ConvertTo-Sid $t.Compte) -eq 'S-1-5-18' -and $t.Active -and $t.Commande -eq "`"$MindcareC\MindcareAide.exe`"" -and $t.Arguments -ceq "-Sid $($e.Sid)" -and (Test-DroitsTache $t.Sddl $e.Sid)
    }
    Test-Cas 'le dossier de Mindcare n''est modifiable que par les administrateurs (l''exe lancé en SYSTEM ne peut pas être changé par Élodie)' {
        $acl = Get-Acl -LiteralPath $MindcareC
        -not @($acl.Access | Where-Object { $_.AccessControlType -eq 'Allow' -and ([int64]$_.FileSystemRights -band 0x116) -and
            $_.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value -notin 'S-1-5-18', 'S-1-5-32-544', 'S-1-3-0', 'S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464' }).Count
    }
}

# =====================================================================================================================
# 2. Élodie : son Mindset.exe trouve la tâche, la réveille, et l'Aide répond
# =====================================================================================================================
function Test-Cas2 {
    Write-Partie '2. Le Mindset.exe d''Élodie trouve la tâche, la réveille, « aide.bonjour » répond'
    Reset-Machine
    $e = Get-Compte $NomElodie
    $null = Install-Mindcare $e.Sid
    $avant = (Get-InfoTache).Dernier
    $s = Invoke-Client $e @('actions', 'aide.bonjour')
    $b = Get-Reponse $s 'aide.bonjour'
    Test-Cas 'Mindcare voit son Aide (presente : tâche SYSTEM pour Élodie et MindcareAide.exe)' { $s -and $s.presente -eq $true }
    Test-Cas 'la tâche a été lancée (dernier lancement changé), l''Aide répond en SYSTEM avec les droits administrateur' {
        (Get-InfoTache).Dernier -ne $avant -and $b.ok -eq $true -and [string]$b.r.Compte -eq 'S-1-5-18' -and $b.r.Admin -eq $true -and [int]$b.r.Version -eq 1
    }
    $aides = @(Get-AidesEnCours)
    foreach ($a in $aides) { Write-Host "         | Aide : $($a.Id) compte $($a.Sid) session $($a.Session) $($a.Chemin)" -ForegroundColor DarkGray }
    Test-Cas 'une seule Aide, en SYSTEM, dans la session 0 (aucune fenêtre possible sur l''écran d''Élodie)' { $aides.Count -eq 1 -and $aides[0].Sid -eq 'S-1-5-18' -and $aides[0].Session -eq 0 }
    Test-Cas 'son journal est recopié dans celui de Mindcare (démarrage, aide.bonjour)' { @($s.journal | Where-Object { $_ -match 'Aide de Mindcare 1 démarrée en SYSTEM' }).Count -and @($s.journal | Where-Object { $_ -match 'aide\.bonjour : fait' }).Count }
    Test-Cas 'une seconde tâche lancée pendant que l''Aide tourne ne fait pas une seconde Aide' {
        $null = & $Schtasks /Run /TN "\Mindset\$NomTache" 2>&1
        Start-Sleep -Seconds 3
        @(Get-AidesEnCours).Count -le 1
    }
    Test-Cas 'l''Aide s''arrête seule une fois Mindcare fermé (moins d''une minute)' { Wait-Jusque { -not @(Get-AidesEnCours).Count } 60 }
}

# =====================================================================================================================
# 3. Un autre compte, un autre programme : refusés
# =====================================================================================================================
function Test-Cas3 {
    Write-Partie '3. Un autre compte (Bruno) et un autre programme d''Élodie : ni la tâche, ni le canal, ni une réponse'
    Reset-Machine
    $e = Get-Compte $NomElodie
    $b = Get-Compte 'Bruno Essai'
    $null = Install-Mindcare $e.Sid
    $avant = (Get-InfoTache).Dernier
    $s = Invoke-Client $b @('tache')
    Test-Cas 'Bruno ne peut ni lire ni lancer la tâche d''Élodie' { $s -and $null -ne $s.lancer -and [int]$s.lancer -ne 0 -and [int]$s.lire -ne 0 -and (Get-InfoTache).Dernier -eq $avant }
    $s = Invoke-Client $b @('actions', 'aide.bonjour')
    Test-Cas 'le Mindset.exe de Bruno ne voit pas d''Aide pour lui, ne la lance pas, n''obtient rien' { $s.presente -eq $false -and (Get-Reponse $s 'aide.bonjour').ok -eq $false -and (Get-InfoTache).Dernier -eq $avant }
    # L'Aide d'Élodie tourne (Élodie a Mindcare ouvert)
    $garde = Invoke-Client $e @('garder', '120') -Fond
    $null = & $Schtasks /Run /TN "\Mindset\$NomTache" 2>&1
    $canal = Get-NomCanal $e.Sid $MindcareC
    Test-Cas 'l''Aide d''Élodie écoute (en SYSTEM) sur son canal' { (Wait-Jusque { Test-Canal $canal } 60) -and @(Get-AidesEnCours | Where-Object { $_.Sid -eq 'S-1-5-18' }).Count -eq 1 }
    $s = Invoke-Client $b @('brut', $canal, '{"a":"aide.bonjour"}')
    Test-Cas 'Bruno ne peut pas ouvrir le canal d''Élodie (accès refusé)' { $s -and $s.ouvert -eq $false -and [string]$s.erreur -match '\(Some\(5\)\)' }
    $s = Invoke-Client $e @('brut', $canal, '{"a":"aide.bonjour"}') -Exe $ClientAilleurs
    Test-Cas 'un autre programme d''Élodie (hors du dossier de Mindcare) ouvre le canal, mais n''a aucune réponse' { $s -and $s.ouvert -eq $true -and -not [string]$s.reponse }
    $s = Invoke-Client $e @('brut', $canal, '{"a":"maj.vider","p":{"Dossier":"C:\\Users"}}')
    Test-Cas 'contre-épreuve : Mindset.exe d''Élodie a une réponse, et une action avec un paramètre est refusée' { $s.ouvert -eq $true -and [string]$s.reponse -match '"ok":false' -and [string]$s.reponse -match 'aucun param' }
    $s = Invoke-Client $e @('actions', 'aide.journal')
    $j = @((Get-Reponse $s 'aide.journal').r) + @($s.journal)
    Test-Cas 'le refus de l''autre programme est noté au journal de l''Aide' { @($j | Where-Object { $_ -like "*Refusé : demande venant de $ClientAilleurs*" }).Count -gt 0 }
    try { $garde.Kill() } catch {}
}

# =====================================================================================================================
# 4. energie.par-defaut
# =====================================================================================================================
function Test-Cas4 {
    Write-Partie '4. Réglages d''alimentation d''origine : le mode ajouté avant a disparu, « Utilisation normale » actif'
    Reset-Machine
    $e = Get-Compte $NomElodie
    $null = Install-Mindcare $e.Sid
    $sortie = (& $Powercfg -duplicatescheme $Equilibre) -join ' '
    $perso = [regex]::Match($sortie, '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}').Value.ToLowerInvariant()
    & $Powercfg -setactive $perso | Out-Null
    Test-Cas "un mode ajouté pour l'essai ($perso), actif" { $perso -and ((& $Powercfg -getactivescheme) -join ' ') -match $perso }
    $s = Invoke-Client $e @('actions', 'energie.par-defaut')
    $r = Get-Reponse $s 'energie.par-defaut'
    Test-Cas 'l''Aide répond : fait, powercfg code 0, « Utilisation normale » actif' { $r.ok -eq $true -and [int]$r.r.code -eq 0 -and [string]$r.r.actif_apres -eq $Equilibre }
    Test-Cas 'le mode ajouté a disparu, « Utilisation normale » est actif' {
        $liste = (& $Powercfg -list) -join ' '
        $liste -notmatch $perso -and ((& $Powercfg -getactivescheme) -join ' ') -match $Equilibre
    }
}

# =====================================================================================================================
# 5. maj.vider, deux fois
# =====================================================================================================================
function Get-EtatServices { @(Get-Service wuauserv, bits | Sort-Object Name | ForEach-Object { "$($_.Name)=$($_.Status)/$($_.StartType)" }) -join ', ' }
function Test-Cas5 {
    Write-Partie '5. Les mises à jour téléchargées mises de côté, deux fois (la seconde avec un .old déjà là)'
    Reset-Machine
    $e = Get-Compte $NomElodie
    $null = Install-Mindcare $e.Sid
    $sd = Join-Path $Windows 'SoftwareDistribution'
    $old = "$sd.old"
    foreach ($tour in 1, 2) {
        # (le dossier existe : Windows le recrée quand son service repart ; sinon, il est recréé ici)
        New-Item -ItemType Directory -Force -Path (Join-Path $sd 'Download') | Out-Null
        [IO.File]::WriteAllText((Join-Path $sd "Download\temoin-$tour.txt"), "essai $tour")
        $avant = Get-EtatServices
        Write-Host "         | services avant : $avant ; .old déjà là : $(Test-Path -LiteralPath $old)" -ForegroundColor DarkGray
        $s = Invoke-Client $e @('actions', 'maj.vider')
        $r = Get-Reponse $s 'maj.vider'
        Test-Cas "tour $tour : fait, le dossier mis de côté en SoftwareDistribution.old" { $r.ok -eq $true -and [string]$r.r.renomme_en -eq $old -and (Test-Path -LiteralPath (Join-Path $old "Download\temoin-$tour.txt")) }
        if ($tour -eq 2) { Test-Cas 'tour 2 : le .old d''avant a été supprimé d''abord (le témoin du tour 1 n''y est plus)' { -not (Test-Path -LiteralPath (Join-Path $old 'Download\temoin-1.txt')) } }
        $apres = Get-EtatServices
        Write-Host "         | services après : $apres" -ForegroundColor DarkGray
        Test-Cas "tour $tour : les services des mises à jour sont comme avant" { $apres -eq $avant }
    }
}

# =====================================================================================================================
# 6. wifi.oublier sans Wi-Fi
# =====================================================================================================================
function Test-Cas6 {
    Write-Partie '6. Faire oublier les Wi-Fi sur une machine sans Wi-Fi : une réponse propre'
    Reset-Machine
    $e = Get-Compte $NomElodie
    $null = Install-Mindcare $e.Sid
    $s = Invoke-Client $e @('actions', 'wifi.oublier')
    $r = Get-Reponse $s 'wifi.oublier'
    Test-Cas 'l''Aide répond « pas de Wi-Fi sur ce PC », sans planter (elle répond encore après)' {
        $r.ok -eq $false -and [string]$r.e -eq "Ce PC n'a pas de Wi-Fi : il n'y a rien à oublier." -and
            (Get-Reponse (Invoke-Client $e @('actions', 'aide.bonjour')) 'aide.bonjour').ok -eq $true
    }
}

# =====================================================================================================================
# 7. ip.renouveler (en dernier)
# =====================================================================================================================
function Test-Cas7 {
    Write-Partie '7. Redemander une adresse à la box : la carte de la machine, pour de vrai, le réseau revient'
    Reset-Machine
    $e = Get-Compte $NomElodie
    $null = Install-Mindcare $e.Sid
    $s = Invoke-Client $e @('actions', 'ip.renouveler')
    $r = Get-Reponse $s 'ip.renouveler'
    Test-Cas 'l''Aide répond : fait, chaque carte en automatique a une adresse après' { $r.ok -eq $true -and @($r.r.cartes).Count -ge 1 -and -not @($r.r.cartes | Where-Object { -not $_.ok }).Count }
    Test-Cas 'le réseau revient (github.com joignable, 60 s au plus)' {
        Wait-Jusque { try { (New-Object Net.Sockets.TcpClient('github.com', 443)).Dispose(); $true } catch { $false } } 60
    }
}

# =====================================================================================================================
# 8. Désinstallation par Mindset
# =====================================================================================================================
function Test-Cas8 {
    Write-Partie '8. Désinstallation de Mindcare par l''installateur de Mindset : plus de dossier, plus de tâche'
    Reset-Machine
    $e = Get-Compte $NomElodie
    $null = Install-Mindcare $e.Sid
    Test-Cas 'avant : la tâche de l''Aide existe' { [bool](Get-InfoTache) }
    $r = Invoke-Installateur @('-Action', 'supprimer', '-Appli', 'mindcare', '-Destination', "`"$MindcareC`"", '-Exe', 'Mindcare.exe')
    Test-Cas 'Mindcare désinstallé (dossier retiré)' { $r.ok -and -not (Test-Path -LiteralPath $MindcareC) }
    Test-Cas 'plus de tâche « \Mindset\Mindcare (aide) » (correctif de Mindset : la tâche prise dans apps.json)' { -not (Get-InfoTache) }
}

# =====================================================================================================================
# 9. L'environnement d'Élodie n'arrive pas à l'Aide
# =====================================================================================================================
function Get-EnvProcessus([int]$Id) {
    if (-not ('EssaiEnv' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class EssaiEnv
{
    [StructLayout(LayoutKind.Sequential)] struct PBI { public IntPtr R1; public IntPtr Peb; public IntPtr R2a; public IntPtr R2b; public IntPtr Id; public IntPtr R3; }
    [DllImport("ntdll.dll")] static extern int NtQueryInformationProcess(IntPtr h, int classe, ref PBI pbi, int taille, out int lu);
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint acces, bool herite, int pid);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool ReadProcessMemory(IntPtr h, IntPtr adresse, byte[] tampon, IntPtr taille, out IntPtr lu);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    static byte[] Lire(IntPtr h, IntPtr a, int n)
    {
        var b = new byte[n]; IntPtr lu;
        if (!ReadProcessMemory(h, a, b, (IntPtr)n, out lu)) throw new Win32Exception(Marshal.GetLastWin32Error());
        return b;
    }
    // Processus 64 bits : PEB + 0x20 = ProcessParameters ; ProcessParameters + 0x80 = Environment, + 0x3F0 = EnvironmentSize
    public static Dictionary<string, string> Variables(int pid)
    {
        IntPtr h = OpenProcess(0x0410, false, pid);   // PROCESS_QUERY_INFORMATION | PROCESS_VM_READ
        if (h == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
        try
        {
            var pbi = new PBI(); int lu;
            int s = NtQueryInformationProcess(h, 0, ref pbi, Marshal.SizeOf(typeof(PBI)), out lu);
            if (s != 0) throw new InvalidOperationException("NtQueryInformationProcess : " + s.ToString("X"));
            IntPtr pp = (IntPtr)BitConverter.ToInt64(Lire(h, IntPtr.Add(pbi.Peb, 0x20), 8), 0);
            IntPtr env = (IntPtr)BitConverter.ToInt64(Lire(h, IntPtr.Add(pp, 0x80), 8), 0);
            long taille = BitConverter.ToInt64(Lire(h, IntPtr.Add(pp, 0x3F0), 8), 0);
            if (taille <= 0 || taille > 4 * 1024 * 1024) taille = 32768;
            byte[] o = null;
            while (o == null && taille >= 2) { try { o = Lire(h, env, (int)taille); } catch { taille /= 2; } }
            if (o == null) throw new InvalidOperationException("environnement illisible");
            var r = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (string l in Encoding.Unicode.GetString(o).Split('\0'))
            {
                if (l.Length == 0) break;
                int i = l.IndexOf('=', 1);   // (les variables « =C: » commencent par =)
                if (i > 0) r[l.Substring(0, i)] = l.Substring(i + 1);
            }
            return r;
        }
        finally { CloseHandle(h); }
    }
}
'@
    }
    try { [Diagnostics.Process]::EnterDebugMode() } catch {}
    [EssaiEnv]::Variables($Id)
}
function Test-Cas9 {
    Write-Partie '9. Environnement : l''Aide en SYSTEM ne voit pas la variable témoin d''Élodie'
    Reset-Machine
    $e = Get-Compte $NomElodie
    $null = Install-Mindcare $e.Sid
    # Les réglages d'Élodie (HKCU\Environment) : une variable témoin inoffensive
    $code = "[Environment]::SetEnvironmentVariable('MINDCARE_TEMOIN_ENV', '1', 'User')"
    $p = Start-Process -FilePath $PowerShellExe -ArgumentList '-NoProfile', '-EncodedCommand', ([Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($code))) -Credential $e.Cred -LoadUserProfile -WorkingDirectory $Racine -PassThru -WindowStyle Hidden
    [void]$p.WaitForExit(60000)
    # Élodie a Mindcare ouvert ; l'Aide réveillée par sa tâche
    $garde = Invoke-Client $e @('garder', '90') -Fond
    $null = & $Schtasks /Run /TN "\Mindset\$NomTache" 2>&1
    $canal = Get-NomCanal $e.Sid $MindcareC
    $sy = @{ Vars = @{} }
    if (Wait-Jusque { Test-Canal $canal } 60) {
        $a = @(Get-AidesEnCours | Where-Object { $_.Sid -eq 'S-1-5-18' }) | Select-Object -First 1
        if ($a) { $sy.Compte = $a.Sid; try { $sy.Vars = Get-EnvProcessus $a.Id } catch { Write-Host "         | environnement de l'Aide illisible : $($_.Exception.Message)" -ForegroundColor DarkGray } }
    } else { Write-Host "         | l'Aide n'a pas ouvert son canal ($canal)" -ForegroundColor DarkGray }
    # Contre-épreuve : le client lui-même, sous le compte d'Élodie
    $el = @{ Vars = @{} }
    try {
        $el.Compte = [string](Invoke-CimMethod -InputObject (Get-CimInstance Win32_Process -Filter "ProcessId=$($garde.Id)") -MethodName GetOwnerSid).Sid
        $el.Vars = Get-EnvProcessus $garde.Id
    } catch { Write-Host "         | environnement du témoin illisible : $($_.Exception.Message)" -ForegroundColor DarkGray }
    try { $garde.Kill() } catch {}
    foreach ($x in @(@('SYSTEM', $sy), @('Élodie', $el))) { Write-Host "         | $($x[0]) : compte $($x[1].Compte), $($x[1].Vars.Count) variables, MINDCARE_TEMOIN_ENV=$($x[1].Vars['MINDCARE_TEMOIN_ENV']), TEMP=$($x[1].Vars['TEMP'])" -ForegroundColor DarkGray }
    Test-Cas 'contre-épreuve : un programme lancé sous le compte d''Élodie voit sa variable témoin' { $el.Compte -eq $e.Sid -and $el.Vars['MINDCARE_TEMOIN_ENV'] -eq '1' }
    Test-Cas 'l''Aide lancée par la tâche SYSTEM ne voit ni la variable témoin, ni rien du dossier d''Élodie' {
        $sy.Compte -eq 'S-1-5-18' -and $sy.Vars.Count -and -not $sy.Vars.ContainsKey('MINDCARE_TEMOIN_ENV') -and -not @($sy.Vars.Values | Where-Object { $_ -like "*$NomElodie*" }).Count
    }
    Stop-Aides
}

# =====================================================================================================================
switch ($Cas) {
    0 { if (-not $Aide) { throw '-Aide <dossier> est demandé avec -Cas 0' }; Initialize-Essai }
    1 { Test-Cas1 }
    2 { Test-Cas2 }
    3 { Test-Cas3 }
    4 { Test-Cas4 }
    5 { Test-Cas5 }
    6 { Test-Cas6 }
    7 { Test-Cas7 }
    8 { Test-Cas8 }
    9 { Test-Cas9 }
    default { Write-Host 'Cas inconnu (0 à 9).'; exit 2 }
}
Write-Host ''
Write-Host "$script:Reussis vérification(s) réussie(s), $($script:Echoues.Count) échec(s)."
if ($script:Echoues.Count) { $script:Echoues | ForEach-Object { Write-Host "  ÉCHEC  $_" -ForegroundColor Red }; exit 1 }
exit 0
