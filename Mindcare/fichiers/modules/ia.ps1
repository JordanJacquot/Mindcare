# Mindcare : l'assistant (une petite IA entraînée sur le catalogue, outils\ia\) qui comprend la phrase de la personne,
# choisit la panne du catalogue et les réponses que la phrase donne déjà. Elle ne répond qu'avec le catalogue (une
# grammaire l'y oblige) : elle n'invente jamais de panne ni de manipulation. Sans elle, la recherche par mots marche.
#
# Installé à la demande (bandeau de l'accueil, environ 700 Mo, une seule fois) dans le dossier de données de Mindcare :
# llama.cpp (version Vulkan : carte graphique NVIDIA, AMD ou Intel, sinon le processeur) et le modèle.
# Lancé seulement quand on s'en sert (on écrit dans la recherche, ou Entrée) : il charge son modèle dans la carte
# graphique, et il est arrêté après $IaRepos minutes sans question pour la rendre (un jeu peut en avoir besoin). Arrêté
# aussi à la fermeture (Stop-Ia), et quand Mindcare est arrêtée de force (natif.cs, ArreterAvecMindcare). Il n'écoute
# que ce PC (127.0.0.1) et ne va pas sur Internet.
# Chargé par Mindcare.ps1 avant contenu.ps1.

$IaDir    = Join-Path $DataDir 'ia'
$IaServeur = if ($EnTest -and $env:MINDCARE_IA_SERVEUR) { $env:MINDCARE_IA_SERVEUR } else { Join-Path $IaDir 'llama-server.exe' }
$IaModele  = if ($EnTest -and $env:MINDCARE_IA_MODELE) { $env:MINDCARE_IA_MODELE } else { Join-Path $IaDir 'mindcare-0.6b-v3-q8_0.gguf' }
$IaFichiers = @(
    @{ Nom = 'moteur de l''assistant'; Url = 'https://github.com/ggml-org/llama.cpp/releases/download/b11541/llama-b11541-bin-win-vulkan-x64.zip'
       Sha256 = '37BBA511D120222F13C3D520DD875F739A530CC124E78F7CBCE0026DE0173D8A'; Taille = 33464528
       Extraire = '^(llama-server\.exe|llama-server-impl\.dll|llama\.dll|llama-common\.dll|ggml[^/]*\.dll|mtmd\.dll|libomp\.dll)$'; Garder = (Join-Path $IaDir 'llama-server.exe') }
    @{ Nom = 'cerveau de l''assistant'; Url = 'https://github.com/JordanJacquot/Mindcare/releases/download/ia-v3/mindcare-0.6b-v3-q8_0.gguf'
       Sha256 = 'C9598811845651239AFCEFFEC641B89D01B96F5DD9D9721334D22562D58E6670'; Taille = 639446912; Garder = (Join-Path $IaDir 'mindcare-0.6b-v3-q8_0.gguf') }
)
$IaSysteme = "Tu es Mindcare. Une personne décrit le problème de son ordinateur. Réponds seulement en JSON : la panne du catalogue qui correspond, et les réponses qu'elle donne déjà."
$IaRepos = 5   # minutes sans question : l'assistant est arrêté (il rend la mémoire de la carte graphique)
# Les .dll sans lesquelles llama-server ne démarre pas : une installation coupée pendant l'extraction (Mindcare fermée)
# peut avoir laissé llama-server.exe sans elles. Le moteur compte alors comme manquant : Mindcare propose de le réparer.
$IaMoteurDll = 'llama.dll', 'ggml.dll'
$script:Ia = @{ Process = $null; Port = 0; Pret = $false; Derniere = [datetime]::MinValue; Lance = [datetime]::MinValue; Minuterie = $null }
$script:IaDl = $null
$script:IaPause = 3   # secondes d'attente avant de réessayer un téléchargement coupé (multipliées par le nombre d'essais)

# Le modèle à lancer : celui de cette version de Mindcare, sinon un plus ancien déjà installé (il sert en attendant la
# mise à jour de l'assistant)
function Get-IaModeleLance {
    if (Test-Path -LiteralPath $IaModele) { return $IaModele }
    if ($EnTest -and $env:MINDCARE_IA_MODELE) { return $null }
    Get-ChildItem -LiteralPath $IaDir -Filter 'mindcare-*.gguf' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1 -ExpandProperty FullName
}
function Test-IaMoteur {
    if (-not (Test-Path -LiteralPath $IaServeur)) { return $false }
    $dossier = Split-Path $IaServeur -Parent
    -not @($IaMoteurDll | Where-Object { -not (Test-Path -LiteralPath (Join-Path $dossier $_)) }).Count
}
function Test-IaInstallee { (Test-IaMoteur) -and [bool](Get-IaModeleLance) }
function Test-IaAJour { (Test-IaMoteur) -and (Test-Path -LiteralPath $IaModele) }

# Après une mise à jour de l'assistant : les anciens modèles sont effacés (640 Mo chacun)
function Remove-IaAnciens {
    if (-not (Test-Path -LiteralPath $IaModele)) { return }
    foreach ($f in Get-ChildItem -LiteralPath $IaDir -Filter 'mindcare-*.gguf' -ErrorAction SilentlyContinue) {
        if ($f.FullName -ne $IaModele) { try { Remove-Item -LiteralPath $f.FullName -Force; Write-Log "Ancien modèle effacé : $($f.Name)" } catch { Write-Log "Ancien modèle non effacé : $_" } }
    }
}
function Get-IaManquants { @($IaFichiers | Where-Object { if ($_.Extraire) { -not (Test-IaMoteur) } else { -not (Test-Path -LiteralPath $_.Garder) } }) }
# Il manque seulement une partie (installation coupée) : le bandeau propose de la réparer
function Test-IaIncomplete { (Test-Path -LiteralPath $IaServeur) -and -not (Test-IaMoteur) }

# La place libre (octets) sur le disque d'un dossier, -1 si on ne sait pas (dossier réseau...)
function Get-PlaceLibre([string]$Dossier) {
    try { (New-Object IO.DriveInfo ([IO.Path]::GetPathRoot([IO.Path]::GetFullPath($Dossier)))).AvailableFreeSpace } catch { -1L }
}

# ---------------------------------------------------------------------------
# L'installation : téléchargement en arrière-plan (reprise si la connexion se coupe, empreinte vérifiée), comme la
# voix d'Evermind. La fenêtre suit l'avancement grâce à $script:IaDl.
# ---------------------------------------------------------------------------
function Start-IaInstall {
    if ($script:IaDl -and -not $script:IaDl.Fini) { return }
    $manquants = Get-IaManquants
    $script:IaDl = [hashtable]::Synchronized(@{ Recu = 0L; Total = [long](($manquants | ForEach-Object { $_.Taille }) | Measure-Object -Sum).Sum; Etape = ''; Erreur = ''; Fini = $false })
    # Assez de place ? (ce qui reste à télécharger, plus le moteur une fois extrait et une marge)
    $reste = 0L
    foreach ($f in $manquants) {
        $tmp = Join-Path $IaDir ([IO.Path]::GetFileName($f.Url) + '.telechargement')
        $deja = if (Test-Path -LiteralPath $tmp) { (Get-Item -LiteralPath $tmp).Length } else { 0L }
        $reste += [math]::Max(0L, [long]$f.Taille - $deja)
    }
    $besoin = $reste + 200MB
    $libre = Get-PlaceLibre $DataDir
    if ($libre -ge 0 -and $libre -lt $besoin) {
        $script:IaDl.Erreur = "il n'y a pas assez de place sur le disque (il faut environ $([math]::Ceiling($besoin / 1MB)) Mo de libre, il en reste $([math]::Floor($libre / 1MB)) Mo). Fais de la place (la corbeille, les vieux téléchargements), puis réessaie"
        $script:IaDl.Fini = $true
        return
    }
    $ps = [PowerShell]::Create()
    [void]$ps.AddScript({
        param($Etat, $Fichiers, $Dossier, $Pause)
        # (disque plein : ERROR_DISK_FULL ou ERROR_HANDLE_DISK_FULL, cherché dans l'erreur et ses causes)
        $estPlein = { param($Err) for ($x = $Err.Exception; $x; $x = $x.InnerException) { if (($x.HResult -band 0xFFFF) -in 112, 39) { return $true } }; $false }
        try {
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
            New-Item -ItemType Directory -Force -Path $Dossier | Out-Null
            $deja = 0L
            foreach ($f in $Fichiers) {
                $Etat.Etape = $f.Nom
                $tmp = Join-Path $Dossier ([IO.Path]::GetFileName($f.Url) + '.telechargement')
                $coupures = 0; $bloque = 0; $refait = $false
                $buf = New-Object byte[] 65536
                while ($true) {
                    $existant = if (Test-Path -LiteralPath $tmp) { (Get-Item -LiteralPath $tmp).Length } else { 0L }
                    if ($existant -gt $f.Taille) { Remove-Item -LiteralPath $tmp -Force; $existant = 0L }
                    if ($existant -lt $f.Taille) {
                        $resp = $null; $in = $null; $out = $null
                        # (une erreur pendant l'écriture sur le disque n'est pas une coupure de la connexion : disque plein,
                        # ou fichier bloqué par un antivirus)
                        $ecrit = $false; $horsReseau = $false
                        try {
                            $req = [Net.HttpWebRequest]::Create($f.Url)
                            $req.UserAgent = 'Mindcare'; $req.Timeout = 30000; $req.ReadWriteTimeout = 60000
                            if ($existant) { $req.AddRange([long]$existant) }
                            $resp = $req.GetResponse()
                            if ($existant -and [int]$resp.StatusCode -ne 206) { $existant = 0L }
                            $in = $resp.GetResponseStream()
                            $ecrit = $true
                            $out = New-Object IO.FileStream($tmp, $(if ($existant) { [IO.FileMode]::Append } else { [IO.FileMode]::Create }), [IO.FileAccess]::Write)
                            $ecrit = $false
                            while (($n = $in.Read($buf, 0, $buf.Length)) -gt 0) { $ecrit = $true; $out.Write($buf, 0, $n); $ecrit = $false; $Etat.Recu = $deja + $out.Length }
                        } catch {
                            if (& $estPlein $_) { throw "il n'y a plus assez de place sur le disque pour « $($f.Nom) ». Fais de la place (la corbeille, les vieux téléchargements), puis réessaie : il reprendra là où il s'est arrêté" }
                            if ($ecrit) {
                                $bloque++; $horsReseau = $true
                                if ($bloque -ge 3) { throw "Mindcare n'arrive pas à écrire le fichier « $($f.Nom) » sur le disque (un antivirus le bloque peut-être). Réessaie dans un moment : il reprendra là où il s'est arrêté" }
                            } else {
                                $coupures++
                                if ($coupures -ge 6) { throw "le téléchargement de « $($f.Nom) » s'est coupé plusieurs fois (connexion instable ?). Réessaie : il reprendra là où il s'est arrêté" }
                            }
                        } finally { if ($out) { try { $out.Dispose() } catch {} }; if ($in) { try { $in.Dispose() } catch {} }; if ($resp) { try { $resp.Dispose() } catch {} } }
                        if ($horsReseau) { Start-Sleep -Seconds ($Pause * $bloque); continue }
                        if ((Get-Item -LiteralPath $tmp -ErrorAction SilentlyContinue).Length -lt $f.Taille) {
                            if ($coupures -ge 6) { throw "le téléchargement de « $($f.Nom) » s'est coupé plusieurs fois (connexion instable ?). Réessaie : il reprendra là où il s'est arrêté" }
                            $coupures++; Start-Sleep -Seconds ([math]::Min(20, $Pause * $coupures))
                            continue
                        }
                    }
                    $Etat.Etape = "$($f.Nom) (vérification)"
                    $hash = (Get-FileHash -LiteralPath $tmp -Algorithm SHA256).Hash
                    $Etat.Etape = $f.Nom
                    if ($hash -eq $f.Sha256) { break }
                    Remove-Item -LiteralPath $tmp -Force
                    if ($refait) { throw "le fichier « $($f.Nom) » arrive abîmé (deux fois de suite). Un antivirus le modifie peut-être pendant le téléchargement" }
                    $refait = $true
                }
                $deja += $f.Taille
                if ($f.Extraire) {
                    Add-Type -AssemblyName System.IO.Compression.FileSystem
                    $zip = [IO.Compression.ZipFile]::OpenRead($tmp)
                    # (chaque fichier extrait à côté puis renommé : jamais de fichier à moitié écrit sous son vrai nom ; et
                    # llama-server.exe en dernier : s'il est là, ses .dll aussi, même si Mindcare a été fermée pendant)
                    $exe = [IO.Path]::GetFileName($f.Garder)
                    try {
                        foreach ($e in @($zip.Entries | Where-Object { $_.Name -match $f.Extraire } | Sort-Object { $_.Name -eq $exe })) {
                            $cible = Join-Path $Dossier $e.Name
                            [IO.Compression.ZipFileExtensions]::ExtractToFile($e, "$cible.extraction", $true)
                            Move-Item -LiteralPath "$cible.extraction" -Destination $cible -Force -ErrorAction Stop
                        }
                    } finally { $zip.Dispose() }
                    Remove-Item -LiteralPath $tmp -Force
                } else { Move-Item -LiteralPath $tmp -Destination $f.Garder -Force }
            }
        } catch {
            $Etat.Erreur = if (& $estPlein $_) { "il n'y a plus assez de place sur le disque. Fais de la place (la corbeille, les vieux téléchargements), puis réessaie" } else { $_.Exception.Message }
        }
        $Etat.Fini = $true
    }).AddArgument($script:IaDl).AddArgument($manquants).AddArgument($IaDir).AddArgument($script:IaPause)
    $script:IaDl.PS = $ps
    $script:IaDl.Handle = $ps.BeginInvoke()
    Write-Log "Installation de l'assistant ($([math]::Round($script:IaDl.Total / 1MB)) Mo)"
}

# Garde-fou : une réponse réglée d'avance n'est gardée que si la phrase en parle vraiment (un mot de la question ou de
# la réponse s'y trouve). Sinon la question sera posée : mieux vaut une question de plus qu'une réponse inventée.
function Select-PreRempli($Donnees, $Panne, [hashtable]$Reponses, [string]$Phrase) {
    $garde = @{}
    if (-not $Reponses -or -not $Panne) { return $garde }
    $vides = 'avec', 'dans', 'pour', 'plus', 'tout', 'quand', 'est', 'elle', 'il', 'pas', 'une', 'des', 'les', 'que', 'qui', 'sur', 'ton', 'ses', 'son', 'mon', 'oui', 'non'
    $mots = @((ConvertTo-Simple $Phrase) -split ' ' | Where-Object { $_.Length -ge 4 -and $vides -notcontains $_ })
    # (les mots de la panne elle-même ne comptent pas : « s'éteint » ne dit rien de la question « la maison s'éteint aussi ? »)
    $dePanne = @((ConvertTo-Simple "$($Panne.Titre) $($Panne.Phrase) $(@($Panne.Mots) -join ' ')") -split ' ' | Where-Object { $_.Length -ge 4 } | ForEach-Object { $_.Substring(0, [math]::Min(5, $_.Length)) })
    foreach ($et in (Get-Etapes $Donnees $Panne)) {
        $id = [string]$et.Id
        if (-not $Reponses.ContainsKey($id)) { continue }
        $n = [int]$Reponses[$id]
        if ($n -lt 0 -or $n -ge @($et.Reponses).Count) { continue }
        $texte = ConvertTo-Simple "$($et.Texte) $($et.Court) $([string]@($et.Reponses)[$n].Texte)"
        $cles = @($texte -split ' ' | Where-Object { $_.Length -ge 4 -and $vides -notcontains $_ } | ForEach-Object { $_.Substring(0, [math]::Min(5, $_.Length)) } | Where-Object { $dePanne -notcontains $_ } | Select-Object -Unique)
        if (@($mots | Where-Object { $m = $_; @($cles | Where-Object { $m.StartsWith($_) }).Count }).Count) { $garde[$id] = $n }
    }
    $garde
}

# ---------------------------------------------------------------------------
# Le lancement et les questions
# ---------------------------------------------------------------------------
function Get-PortLibre {
    $l = New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback, 0)
    $l.Start(); $p = $l.LocalEndpoint.Port; $l.Stop(); $p
}

# Le programme à lancer. Le Gestionnaire des tâches range sur une seule ligne « Mindset » les programmes qui ont le même
# nom : l'assistant est donc lancé depuis une copie de llama-server.exe nommée Mindset.exe, dans le même dossier (il y
# trouve ses .dll). L'empreinte du téléchargement a été vérifiée avant l'extraction ; la copie est refaite dès qu'elle
# n'est plus identique à llama-server.exe (assistant mis à jour, copie abîmée). Si la copie est impossible :
# llama-server.exe, comme avant. Rien ne cherche ce processus par son nom : Mindcare le garde par son numéro (Stop-Ia, job).
function Get-IaServeurLance {
    $copie = Join-Path (Split-Path $IaServeur -Parent) 'Mindset.exe'
    try {
        # (vérifiée une fois par session : ensuite, tant que llama-server.exe et sa copie n'ont pas changé, pas de nouvelle empreinte)
        $s = Get-Item -LiteralPath $IaServeur -ErrorAction Stop; $vu = "$($s.Length)|$($s.LastWriteTimeUtc.Ticks)"
        $c = Get-Item -LiteralPath $copie -ErrorAction SilentlyContinue
        if ($script:IaCopieVue -eq $vu -and $c -and $c.Length -eq $s.Length) { return $copie }
        $empreinte = (Get-FileHash -LiteralPath $IaServeur -Algorithm SHA256 -ErrorAction Stop).Hash
        $c = Get-Item -LiteralPath $copie -ErrorAction SilentlyContinue
        if (-not $c -or $c.Length -ne (Get-Item -LiteralPath $IaServeur).Length -or (Get-FileHash -LiteralPath $copie -Algorithm SHA256 -ErrorAction Stop).Hash -ne $empreinte) {
            Copy-Item -LiteralPath $IaServeur -Destination $copie -Force -ErrorAction Stop
            if ((Get-FileHash -LiteralPath $copie -Algorithm SHA256 -ErrorAction Stop).Hash -ne $empreinte) { throw 'copie différente de llama-server.exe' }
        }
        $script:IaCopieVue = $vu
        $copie
    } catch { Write-Log "Assistant : copie Mindset.exe impossible, llama-server.exe lancé ($($_.Exception.Message))"; $IaServeur }
}

# Lance llama-server caché (si l'assistant est installé et pas déjà lancé) ; il est prêt quelques secondes après.
# Appelé seulement quand on se sert de l'assistant : il compte aussi comme « on s'en sert » (voir Stop-IaSiInutile).
function Start-Ia {
    $script:Ia.Derniere = Get-Date
    if ($script:Ia.Process -and -not $script:Ia.Process.HasExited) { return }
    if ($script:Ia.Process -and -not $script:Ia.Arrete) {
        # Arrêté tout seul (pas par Mindcare) : la cause au journal une seule fois (son code, ses dernières lignes), et pas
        # de nouvel essai avant une minute (sinon un assistant qui plante serait relancé à chaque lettre tapée)
        if (-not $script:Ia.FinNotee) {
            $script:Ia.FinNotee = $true
            $code = try { $script:Ia.Process.ExitCode } catch { '?' }
            $lignes = if ($script:Ia.Sorties) { @($script:Ia.Sorties.Lignes()) } else { @() }
            Write-Log "Assistant arrêté tout seul (code $code)$(if ($lignes) { " ; ses dernières lignes : $($lignes -join ' | ')" })"
        }
        if (((Get-Date) - $script:Ia.Lance).TotalSeconds -lt 60) { return }
    }
    if (-not (Test-IaInstallee)) { return }
    try {
        $script:Ia.Port = Get-PortLibre
        $programme = Get-IaServeurLance
        $si = New-Object Diagnostics.ProcessStartInfo $programme
        $modeleLance = Get-IaModeleLance
        $si.Arguments = "-m `"$modeleLance`" --host 127.0.0.1 --port $($script:Ia.Port) -ngl 99 -c 2048 -np 1 --no-webui"
        $si.UseShellExecute = $false; $si.CreateNoWindow = $true
        $si.RedirectStandardOutput = $true; $si.RedirectStandardError = $true
        $si.EnvironmentVariables['LLAMA_ARG_CHAT_TEMPLATE_KWARGS'] = '{"enable_thinking":false}'
        # (une clé neuve à chaque lancement : un autre programme du PC ne peut pas questionner l'assistant ; par une variable
        # d'environnement, car la ligne de commande se lit dans le Gestionnaire des tâches ; /health reste ouvert)
        $script:Ia.Cle = -join @(1..2 | ForEach-Object { [guid]::NewGuid().ToString('N') })
        $si.EnvironmentVariables['LLAMA_API_KEY'] = $script:Ia.Cle
        $script:Ia.Process = [Diagnostics.Process]::Start($si)
        $script:Ia.Arrete = $false; $script:Ia.FinNotee = $false
        $script:Ia.Sorties = try { New-Object MindcareSorties $script:Ia.Process, 20 } catch { $null }   # (avant de lire ses sorties)
        # (arrêté avec Mindcare, même si elle est fermée de force : sinon il resterait lancé, sans fenêtre, jusqu'au redémarrage)
        try { if (-not [MindcareNatif]::ArreterAvecMindcare($script:Ia.Process.Handle)) { Write-Log "Assistant : pas relié à Mindcare ($([MindcareNatif]::Erreur))" } } catch { Write-Log "Assistant : pas relié à Mindcare ($_)" }
        $script:Ia.Process.BeginOutputReadLine(); $script:Ia.Process.BeginErrorReadLine()   # (sinon ses messages bloquent)
        $script:Ia.Pret = $false; $script:Ia.Lance = Get-Date
        Write-Log "Assistant lancé (port $($script:Ia.Port), $([IO.Path]::GetFileName($modeleLance)), $([IO.Path]::GetFileName($programme)))"
    } catch { Write-Log "Assistant : lancement impossible ($_)"; $script:Ia.Process = $null; return }
    # Toutes les 30 secondes : arrêté s'il ne sert plus (la minuterie de la fenêtre ; sans fenêtre, dans les tests, rien)
    try {
        if (-not $script:Ia.Minuterie) {
            $t = New-Object System.Windows.Threading.DispatcherTimer
            $t.Interval = [TimeSpan]::FromSeconds(30)
            $action = { Stop-IaSiInutile }
            if (Get-Command Protect-Action -ErrorAction SilentlyContinue) { $action = Protect-Action $action }
            $t.Add_Tick($action)
            $script:Ia.Minuterie = $t
        }
        $script:Ia.Minuterie.Start()
    } catch {}
}

# Arrête llama-server s'il n'a servi à rien depuis $IaRepos minutes : il rend la mémoire de la carte graphique et du PC
# (Mindcare reste souvent ouverte, cachée dans Mindset, pendant qu'on joue). Relancé tout seul à la prochaine question.
function Stop-IaSiInutile([datetime]$Maintenant = (Get-Date)) {
    if (-not (Test-IaLancee)) { if ($script:Ia.Minuterie) { $script:Ia.Minuterie.Stop() }; return $false }
    if (($Maintenant - $script:Ia.Derniere).TotalMinutes -lt $IaRepos) { return $false }
    Write-Log "Assistant arrêté : pas de question depuis $IaRepos minutes (relancé à la prochaine)"
    Stop-IaServeur
    $true
}

function Stop-IaServeur {
    $script:Ia.Arrete = $true   # (arrêté par Mindcare : relancé dès la prochaine question)
    try { if ($script:Ia.Process -and -not $script:Ia.Process.HasExited) { $script:Ia.Process.Kill(); [void]$script:Ia.Process.WaitForExit(3000) } } catch {}
    try { if ($script:Ia.Minuterie) { $script:Ia.Minuterie.Stop() } } catch {}
}

function Stop-Ia {
    Stop-IaServeur
    try { if ($script:IaDl -and $script:IaDl.PS) { $script:IaDl.PS.Stop() } } catch {}
}

function Test-IaLancee { [bool]($script:Ia.Process -and -not $script:Ia.Process.HasExited) }

# La confiance au-dessus de laquelle l'assistant lance l'enquête tout seul (mesurée dans outils\ia\confiance.py : à 0,95,
# 1 erreur sur 122 phrases écrites à la main) ; en dessous, Mindcare pose la question « Pour être sûr » avec son idée et les
# deux premières de la recherche par mots (la bonne panne y est 31 fois sur 38, contre 27 avec ses propres autres idées)
$IaSeuil = 0.95

# Pose la question en arrière-plan ; renvoie une tâche à suivre avec Get-IaReponse (sans figer la fenêtre)
function Start-IaQuestion([string]$Phrase, [string[]]$Pannes) {
    if ($EnTest -and $env:MINDCARE_TEST_IA) { return @{ Faux = $env:MINDCARE_TEST_IA; Debut = Get-Date } }
    Start-Ia   # (lancé maintenant s'il ne l'était pas, ou arrêté faute de questions ; sinon : il sert, il reste)
    if (-not (Test-IaLancee)) { return $null }
    # (juste lancé : il charge encore son modèle, plus long la première fois ou sur un disque lent)
    $attente = if (((Get-Date) - $script:Ia.Lance).TotalSeconds -lt 60) { 100 } else { 40 }
    $ps = [PowerShell]::Create()
    [void]$ps.AddScript({
        param($Port, $Phrase, $Pannes, $Systeme, $Attente, $Cle)
        # (l'assistant peut encore charger son modèle : on attend qu'il soit prêt, le temps de la réponse en moins)
        $fin = (Get-Date).AddSeconds($Attente - 32)
        while ((Get-Date) -lt $fin) {
            try { if ((Invoke-WebRequest "http://127.0.0.1:$Port/health" -UseBasicParsing -TimeoutSec 2).StatusCode -eq 200) { break } } catch { Start-Sleep -Milliseconds 500 }
        }
        # Une idée : la panne (parmi celles permises) et la confiance, la probabilité des jetons qui écrivent son nom
        $idee = {
            param([string[]]$Permises)
            $corps = @{
                messages = @(@{ role = 'system'; content = $Systeme }, @{ role = 'user'; content = $Phrase })
                temperature = 0; max_tokens = 120; logprobs = $true
                response_format = @{ type = 'json_schema'; json_schema = @{ name = 'r'; schema = @{
                    type = 'object'; required = @('panne', 'reponses')
                    properties = @{ panne = @{ type = 'string'; enum = @($Permises) }; reponses = @{ type = 'object'; additionalProperties = @{ type = 'integer' } } } } } }
            } | ConvertTo-Json -Depth 10 -Compress
            $r = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/v1/chat/completions" -Method Post -Headers @{ Authorization = "Bearer $Cle" } -Body ([Text.Encoding]::UTF8.GetBytes($corps)) -ContentType 'application/json; charset=utf-8' -TimeoutSec 30 -ErrorAction Stop
            $c = [string]$r.choices[0].message.content
            $p = 1.0
            try {
                $d = $c.IndexOf('"', $c.IndexOf(':', $c.IndexOf('"panne"'))) + 1; $f = $c.IndexOf('"', $d)
                $pos = 0; $lp = 0.0
                foreach ($j in $r.choices[0].logprobs.content) {
                    $t = [string]$j.token
                    if ($pos -lt $f -and $pos + $t.Length -gt $d) { $lp += [double]$j.logprob }
                    $pos += $t.Length
                }
                $p = [math]::Exp($lp)
            } catch { $p = 0.0 }
            @{ c = $c; p = $p }
        }
        ConvertTo-Json -InputObject @(& $idee $Pannes) -Depth 3 -Compress
    }).AddArgument($script:Ia.Port).AddArgument($Phrase).AddArgument(@($Pannes)).AddArgument($IaSysteme).AddArgument($attente).AddArgument([string]$script:Ia.Cle)
    @{ PS = $ps; Res = $ps.BeginInvoke(); Debut = Get-Date; Attente = $attente }
}

# La réponse si elle est arrivée : $null (pas encore), ou { Panne, Reponses (hashtable id d'étape -> n°), Confiance,
# Autres (d'autres idées, seulement dans les tests : { Panne, Reponses }), Erreur }
function Get-IaReponse($Tache) {
    $vide = { param($Erreur) [pscustomobject]@{ Panne = ''; Reponses = @{}; Confiance = 0.0; Autres = @(); Erreur = $Erreur } }
    if (-not $Tache) { return & $vide 'pas d''assistant' }
    $texte = $null
    if ($Tache.Faux -and $env:MINDCARE_TEST_IA_ATTENTE) { return $null }   # (tests : l'assistant lit encore, pour la capture de l'attente)
    if ($Tache.Faux) { $texte = $Tache.Faux }
    else {
        $script:Ia.Derniere = Get-Date   # (une question en cours : il sert)
        if (-not $Tache.Res.IsCompleted) {
            if (((Get-Date) - $Tache.Debut).TotalSeconds -lt $(if ($Tache.Attente) { $Tache.Attente } else { 40 })) { return $null }
            try { $Tache.PS.Stop() } catch {}
            return & $vide "trop long ($([math]::Round(((Get-Date) - $Tache.Debut).TotalSeconds)) s)"
        }
        # (la vraie cause d'un échec, pour le journal : l'erreur de la requête, ou l'assistant arrêté)
        $cause = ''
        try { $texte = [string](@($Tache.PS.EndInvoke($Tache.Res)) | Select-Object -Last 1) } catch { $texte = $null; $cause = $_.Exception.Message }
        try { if (-not $cause -and $Tache.PS.Streams.Error.Count) { $cause = [string]$Tache.PS.Streams.Error[$Tache.PS.Streams.Error.Count - 1] } } catch {}
        try { $Tache.PS.Dispose() } catch {}
        if (-not $texte) {
            if (-not $cause) { $cause = 'réponse vide' }
            if (-not (Test-IaLancee)) { $cause = "l'assistant s'est arrêté ($cause)" }
            return & $vide "pas de réponse : $cause"
        }
    }
    try {
        # (les tests donnent une seule réponse de l'assistant, donc sûre, ou la liste de ses idées comme le vrai)
        $brut = ConvertFrom-Json $texte -ErrorAction Stop
        $idees = if ($brut -is [array] -or $brut.c) { @($brut) } else { @([pscustomobject]@{ c = $texte; p = 1.0 }) }
        $lues = foreach ($i in $idees) {
            $j = if ($i.c -is [string]) { ConvertFrom-Json $i.c -ErrorAction Stop } else { $i.c }
            if (-not $j.panne) { throw 'pas de panne' }
            $rep = @{}
            if ($j.reponses) { foreach ($pr in $j.reponses.PSObject.Properties) { $rep[$pr.Name] = [int]$pr.Value } }
            [pscustomobject]@{ Panne = [string]$j.panne; Reponses = $rep; Confiance = [double]$i.p }
        }
        $lues = @($lues)
        [pscustomobject]@{ Panne = $lues[0].Panne; Reponses = $lues[0].Reponses; Confiance = $lues[0].Confiance; Autres = @($lues | Select-Object -Skip 1); Erreur = '' }
    } catch { & $vide "réponse illisible ($texte)" }
}
