# Mindcare : l'assistant (une petite IA entraînée sur le catalogue, outils\ia\) qui comprend la phrase de la personne,
# choisit la panne du catalogue et les réponses que la phrase donne déjà. Elle ne répond qu'avec le catalogue (une
# grammaire l'y oblige) : elle n'invente jamais de panne ni de manipulation. Sans elle, la recherche par mots marche.
#
# Installé à la demande (bandeau de l'accueil, environ 700 Mo, une seule fois) dans le dossier de données de Mindcare :
# llama.cpp (version Vulkan : carte graphique NVIDIA, AMD ou Intel, sinon le processeur) et le modèle.
# Lancé en arrière-plan à l'ouverture de Mindcare s'il est installé, arrêté à la fermeture (Stop-Ia).
# Chargé par Mindcare.ps1 avant contenu.ps1.

$IaDir    = Join-Path $DataDir 'ia'
$IaServeur = if ($EnTest -and $env:MINDCARE_IA_SERVEUR) { $env:MINDCARE_IA_SERVEUR } else { Join-Path $IaDir 'llama-server.exe' }
$IaModele  = if ($EnTest -and $env:MINDCARE_IA_MODELE) { $env:MINDCARE_IA_MODELE } else { Join-Path $IaDir 'mindcare-0.6b-v2-q8_0.gguf' }
$IaFichiers = @(
    @{ Nom = 'moteur de l''assistant'; Url = 'https://github.com/ggml-org/llama.cpp/releases/download/b11541/llama-b11541-bin-win-vulkan-x64.zip'
       Sha256 = '37BBA511D120222F13C3D520DD875F739A530CC124E78F7CBCE0026DE0173D8A'; Taille = 33464528
       Extraire = '^(llama-server\.exe|llama-server-impl\.dll|llama\.dll|llama-common\.dll|ggml[^/]*\.dll|mtmd\.dll|libomp\.dll)$'; Garder = (Join-Path $IaDir 'llama-server.exe') }
    @{ Nom = 'cerveau de l''assistant'; Url = 'https://github.com/JordanJacquot/Mindcare/releases/download/ia-v2/mindcare-0.6b-v2-q8_0.gguf'
       Sha256 = '13246AB9F9F7D25E22066682A81BE9744E715DDC4A1B789DF5330E4D5BE0DE5'; Taille = 639446912; Garder = (Join-Path $IaDir 'mindcare-0.6b-v2-q8_0.gguf') }
)
$IaSysteme = "Tu es Mindcare. Une personne décrit le problème de son ordinateur. Réponds seulement en JSON : la panne du catalogue qui correspond, et les réponses qu'elle donne déjà."
$script:Ia = @{ Process = $null; Port = 0; Pret = $false }
$script:IaDl = $null

# Le modèle à lancer : celui de cette version de Mindcare, sinon un plus ancien déjà installé (il sert en attendant la
# mise à jour de l'assistant)
function Get-IaModeleLance {
    if (Test-Path -LiteralPath $IaModele) { return $IaModele }
    if ($EnTest -and $env:MINDCARE_IA_MODELE) { return $null }
    Get-ChildItem -LiteralPath $IaDir -Filter 'mindcare-*.gguf' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1 -ExpandProperty FullName
}
function Test-IaInstallee { (Test-Path -LiteralPath $IaServeur) -and [bool](Get-IaModeleLance) }
function Test-IaAJour { (Test-Path -LiteralPath $IaServeur) -and (Test-Path -LiteralPath $IaModele) }

# Après une mise à jour de l'assistant : les anciens modèles sont effacés (640 Mo chacun)
function Remove-IaAnciens {
    if (-not (Test-Path -LiteralPath $IaModele)) { return }
    foreach ($f in Get-ChildItem -LiteralPath $IaDir -Filter 'mindcare-*.gguf' -ErrorAction SilentlyContinue) {
        if ($f.FullName -ne $IaModele) { try { Remove-Item -LiteralPath $f.FullName -Force; Write-Log "Ancien modèle effacé : $($f.Name)" } catch { Write-Log "Ancien modèle non effacé : $_" } }
    }
}
function Get-IaManquants { @($IaFichiers | Where-Object { -not (Test-Path -LiteralPath $_.Garder) }) }

# ---------------------------------------------------------------------------
# L'installation : téléchargement en arrière-plan (reprise si la connexion se coupe, empreinte vérifiée), comme la
# voix d'Evermind. La fenêtre suit l'avancement grâce à $script:IaDl.
# ---------------------------------------------------------------------------
function Start-IaInstall {
    if ($script:IaDl -and -not $script:IaDl.Fini) { return }
    $manquants = Get-IaManquants
    $script:IaDl = [hashtable]::Synchronized(@{ Recu = 0L; Total = [long](($manquants | ForEach-Object { $_.Taille }) | Measure-Object -Sum).Sum; Etape = ''; Erreur = ''; Fini = $false })
    $ps = [PowerShell]::Create()
    [void]$ps.AddScript({
        param($Etat, $Fichiers, $Dossier)
        try {
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
            New-Item -ItemType Directory -Force -Path $Dossier | Out-Null
            $deja = 0L
            foreach ($f in $Fichiers) {
                $Etat.Etape = $f.Nom
                $tmp = Join-Path $Dossier ([IO.Path]::GetFileName($f.Url) + '.telechargement')
                $coupures = 0; $refait = $false
                $buf = New-Object byte[] 65536
                while ($true) {
                    $existant = if (Test-Path -LiteralPath $tmp) { (Get-Item -LiteralPath $tmp).Length } else { 0L }
                    if ($existant -gt $f.Taille) { Remove-Item -LiteralPath $tmp -Force; $existant = 0L }
                    if ($existant -lt $f.Taille) {
                        $resp = $null; $in = $null; $out = $null
                        try {
                            $req = [Net.HttpWebRequest]::Create($f.Url)
                            $req.UserAgent = 'Mindcare'; $req.Timeout = 30000; $req.ReadWriteTimeout = 60000
                            if ($existant) { $req.AddRange([long]$existant) }
                            $resp = $req.GetResponse()
                            if ($existant -and [int]$resp.StatusCode -ne 206) { $existant = 0L }
                            $in = $resp.GetResponseStream()
                            $out = New-Object IO.FileStream($tmp, $(if ($existant) { [IO.FileMode]::Append } else { [IO.FileMode]::Create }), [IO.FileAccess]::Write)
                            while (($n = $in.Read($buf, 0, $buf.Length)) -gt 0) { $out.Write($buf, 0, $n); $Etat.Recu = $deja + $out.Length }
                        } catch {
                            $coupures++
                            if ($coupures -ge 6) { throw "le téléchargement de « $($f.Nom) » s'est coupé plusieurs fois (connexion instable ?). Réessaie : il reprendra là où il s'est arrêté" }
                        } finally { if ($out) { $out.Dispose() }; if ($in) { $in.Dispose() }; if ($resp) { $resp.Dispose() } }
                        if ((Get-Item -LiteralPath $tmp -ErrorAction SilentlyContinue).Length -lt $f.Taille) {
                            if ($coupures -ge 6) { throw "le téléchargement de « $($f.Nom) » s'est coupé plusieurs fois (connexion instable ?). Réessaie : il reprendra là où il s'est arrêté" }
                            $coupures++; Start-Sleep -Seconds ([math]::Min(20, 3 * $coupures))
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
                    try {
                        foreach ($e in $zip.Entries) {
                            if ($e.Name -notmatch $f.Extraire) { continue }
                            [IO.Compression.ZipFileExtensions]::ExtractToFile($e, (Join-Path $Dossier $e.Name), $true)
                        }
                    } finally { $zip.Dispose() }
                    Remove-Item -LiteralPath $tmp -Force
                } else { Move-Item -LiteralPath $tmp -Destination $f.Garder -Force }
            }
        } catch { $Etat.Erreur = $_.Exception.Message }
        $Etat.Fini = $true
    }).AddArgument($script:IaDl).AddArgument($manquants).AddArgument($IaDir)
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

# Lance llama-server caché (si l'assistant est installé et pas déjà lancé) ; il est prêt quelques secondes après
function Start-Ia {
    if ($script:Ia.Process -and -not $script:Ia.Process.HasExited) { return }
    if (-not (Test-IaInstallee)) { return }
    try {
        $script:Ia.Port = Get-PortLibre
        $si = New-Object Diagnostics.ProcessStartInfo $IaServeur
        $modeleLance = Get-IaModeleLance
        $si.Arguments = "-m `"$modeleLance`" --host 127.0.0.1 --port $($script:Ia.Port) -ngl 99 -c 2048 -np 1 --no-webui"
        $si.UseShellExecute = $false; $si.CreateNoWindow = $true
        $si.RedirectStandardOutput = $true; $si.RedirectStandardError = $true
        $si.EnvironmentVariables['LLAMA_ARG_CHAT_TEMPLATE_KWARGS'] = '{"enable_thinking":false}'
        $script:Ia.Process = [Diagnostics.Process]::Start($si)
        $script:Ia.Process.BeginOutputReadLine(); $script:Ia.Process.BeginErrorReadLine()   # (sinon ses messages bloquent)
        $script:Ia.Pret = $false
        Write-Log "Assistant lancé (port $($script:Ia.Port), $([IO.Path]::GetFileName($modeleLance)))"
    } catch { Write-Log "Assistant : lancement impossible ($_)"; $script:Ia.Process = $null }
}

function Stop-Ia {
    try { if ($script:Ia.Process -and -not $script:Ia.Process.HasExited) { $script:Ia.Process.Kill(); [void]$script:Ia.Process.WaitForExit(3000) } } catch {}
    try { if ($script:IaDl -and $script:IaDl.PS) { $script:IaDl.PS.Stop() } } catch {}
}

function Test-IaLancee { [bool]($script:Ia.Process -and -not $script:Ia.Process.HasExited) }

# Pose la question en arrière-plan ; renvoie une tâche à suivre avec Get-IaReponse (sans figer la fenêtre)
function Start-IaQuestion([string]$Phrase, [string[]]$Pannes) {
    if ($EnTest -and $env:MINDCARE_TEST_IA) { return @{ Faux = $env:MINDCARE_TEST_IA; Debut = Get-Date } }
    if (-not (Test-IaLancee)) { return $null }
    $corps = @{
        messages = @(@{ role = 'system'; content = $IaSysteme }, @{ role = 'user'; content = $Phrase })
        temperature = 0; max_tokens = 120
        response_format = @{ type = 'json_schema'; json_schema = @{ name = 'r'; schema = @{
            type = 'object'; required = @('panne', 'reponses')
            properties = @{ panne = @{ type = 'string'; enum = @($Pannes) }; reponses = @{ type = 'object'; additionalProperties = @{ type = 'integer' } } } } } }
    } | ConvertTo-Json -Depth 10 -Compress
    $ps = [PowerShell]::Create()
    [void]$ps.AddScript({
        param($Port, $Corps)
        # (l'assistant peut encore charger son modèle : on attend qu'il soit prêt, 30 secondes au plus)
        for ($i = 0; $i -lt 60; $i++) {
            try { if ((Invoke-WebRequest "http://127.0.0.1:$Port/health" -UseBasicParsing -TimeoutSec 2).StatusCode -eq 200) { break } } catch { Start-Sleep -Milliseconds 500 }
        }
        $octets = [Text.Encoding]::UTF8.GetBytes($Corps)
        $r = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/v1/chat/completions" -Method Post -Body $octets -ContentType 'application/json; charset=utf-8' -TimeoutSec 30
        $r.choices[0].message.content
    }).AddArgument($script:Ia.Port).AddArgument($corps)
    @{ PS = $ps; Res = $ps.BeginInvoke(); Debut = Get-Date }
}

# La réponse si elle est arrivée : $null (pas encore), ou { Panne, Reponses (hashtable id d'étape -> n°), Erreur }
function Get-IaReponse($Tache) {
    if (-not $Tache) { return [pscustomobject]@{ Panne = ''; Reponses = @{}; Erreur = 'pas d''assistant' } }
    $texte = $null
    if ($Tache.Faux) { $texte = $Tache.Faux }
    else {
        if (-not $Tache.Res.IsCompleted) {
            if (((Get-Date) - $Tache.Debut).TotalSeconds -lt 40) { return $null }
            try { $Tache.PS.Stop() } catch {}
            return [pscustomobject]@{ Panne = ''; Reponses = @{}; Erreur = 'trop long' }
        }
        try { $texte = [string](@($Tache.PS.EndInvoke($Tache.Res)) | Select-Object -Last 1) } catch { $texte = $null }
        try { $Tache.PS.Dispose() } catch {}
    }
    try {
        $j = $texte | ConvertFrom-Json
        $rep = @{}
        if ($j.reponses) { foreach ($pr in $j.reponses.PSObject.Properties) { $rep[$pr.Name] = [int]$pr.Value } }
        [pscustomobject]@{ Panne = [string]$j.panne; Reponses = $rep; Erreur = '' }
    } catch { [pscustomobject]@{ Panne = ''; Reponses = @{}; Erreur = "réponse illisible ($texte)" } }
}
