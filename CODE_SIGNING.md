# Code signing policy

*Version française plus bas : [Politique de signature du code](#politique-de-signature-du-code).*

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

> **Status:** the application to SignPath Foundation is in progress. Until it is accepted, the releases are not
> Authenticode signed. Mindset only installs or updates Mindcare from a `Mindcare.zip` that carries a valid RSA
> signature of the maintainer (`Mindcare.zip.sig`).

## What is signed

- `Mindcare.exe` and the PowerShell scripts (`.ps1`) of Mindcare (`fichiers\Mindcare.ps1` and `fichiers\modules\*.ps1`),
  all built from this repository.
- The other files of the app are data, not programs, and are not signed: the catalogue of breakdowns
  (`enquetes.json`), the drawings (`.svg`), the window layout (`.xaml`), the public keys of the suite (`.xml`) and a
  small C# source file (`natif.cs`) that Windows compiles when Mindcare starts. `catalogue.zip`, published with each
  version for the phone app Mindcare Mobile, only contains `enquetes.json` and the drawings.
- Files from other authors are never signed with this certificate, and they are not in `Mindcare.zip`. They are only
  downloaded when the user asks for the assistant (see the privacy policy below):
  - llama.cpp (`llama-server.exe` and its DLLs), official Windows build `b11541` (Vulkan, x64) of
    [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp), MIT license. Mindcare checks its SHA-256 before using it.
  - The model of the assistant (`mindcare-0.6b-v2-q8_0.gguf`), a data file trained for Mindcare from
    [Qwen3 0.6B](https://huggingface.co/Qwen/Qwen3-0.6B) (Qwen team, Apache License 2.0), published in the
    [ia-v2](../../releases/tag/ia-v2) release of this repository under the same Apache License 2.0. Mindcare checks its
    SHA-256 before using it.

## How a release is built and signed

1. The version is tagged in this repository (`v<version>`).
2. The [Release workflow](.github/workflows/release.yml) builds `Mindcare.exe` and `catalogue.zip` on a GitHub hosted
   runner (`outils\construire.ps1`), from the tagged source only, and uploads the `Mindcare` folder as a workflow
   artifact (`catalogue.zip` is uploaded as a separate artifact and is not sent for signing).
3. The workflow sends a signing request to SignPath. SignPath checks that the artifact comes from this workflow and
   this repository. **Every signing request is approved manually** by an approver (see below).
4. The maintainer downloads the signed `Mindcare.zip` produced by the workflow, adds the detached RSA signature
   `Mindcare.zip.sig` (Mindset uses it to verify installations and updates; it does not change any file inside the
   zip) and publishes the Release.

## Team and roles

| Role | Members |
|---|---|
| Committers and reviewers | [Jordan Jacquot](https://github.com/JordanJacquot) |
| Approvers | [Jordan Jacquot](https://github.com/JordanJacquot) |

- Committers can change the code of this repository. Changes proposed by anyone else (pull requests) are reviewed
  by a committer before they are merged.
- Every signing request is approved by an approver.
- All team members use multi-factor authentication on GitHub and on SignPath.

## Privacy policy

This program will not transfer any information to other networked systems unless specifically requested by the user
or the person installing or operating it.

Mindcare has no account, no advertising, no telemetry and no statistics. It never connects to the Internet by itself:
there is no automatic connection to turn off. Installing and updating Mindcare is done by Mindset (see
[Mindset's own policy](https://github.com/JordanJacquot/Mindset/blob/main/CODE_SIGNING.md)). The table below lists
everything that Mindcare can send, always after a click.

### Only when you ask for it (a button)

| What | When | What is sent |
|---|---|---|
| Download of the assistant: llama.cpp from `github.com/ggml-org/llama.cpp` (about 33 MB) and the model from `github.com/JordanJacquot/Mindcare` (about 640 MB). GitHub serves the files from its download servers (`*.githubusercontent.com`) | When you press "Installer" (or "Mettre à jour", or "Réessayer") on the banner of the home page. The banner gives the size before you press it | Standard HTTPS requests for these two files (user agent `Mindcare`); nothing about you or your PC |
| Mindset's download page, `github.com/JordanJacquot/Mindset/releases/latest` | Only when Mindcare is opened without Mindset installed, and you press "Télécharger Mindset" | It opens in your web browser |

What never leaves your PC:
- The assistant runs on your PC only. `llama-server` only listens on `127.0.0.1` (this PC): the sentences you type
  never leave the PC, and the assistant works without Internet once installed.
- What Windows has recorded (unexpected restarts, blue screens, memory test, disks) and the list of memory modules are
  read on your PC, without administrator rights, and only shown in Mindcare and in its report.
- The report for the repairer is saved on your Desktop or copied, only when you press the button; you decide who
  receives it.

Third party services: the downloads above come from GitHub, covered by the
[GitHub General Privacy Statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).

## System changes and uninstallation

- Mindcare changes nothing in Windows. It only reads what Windows has recorded. When a command can fix a breakdown, it
  is shown with a "Copier" button and an explanation: Mindcare never runs it itself.
- Its files: the program in Mindset's programs folder, and its data (settings, saved diagnostics, journal, and the
  assistant when it is installed, about 700 MB) in `%LOCALAPPDATA%\Mindset\Mindcare` (or `X:\Mindset\Donnees\Mindcare`
  when Mindset is installed on another drive `X:`).
- The assistant (`llama-server`) stops with Mindcare, even when Mindcare is closed by force (Mindset does this before
  an update or an uninstallation).
- Mindcare is installed, updated and uninstalled by Mindset: "Désinstaller" on the card of Mindcare in Mindset removes
  the program; its data folder is kept, so that a new installation finds the saved diagnostics. The uninstaller of
  Mindset (`Desinstaller Mindset.exe`, also in "Installed apps" of Windows) removes Mindcare with Mindset, and its data
  folder too (with the assistant) when you choose to delete the data.

---

# Politique de signature du code

Signature gratuite du code fournie par [SignPath.io](https://about.signpath.io), certificat de la
[SignPath Foundation](https://signpath.org).

> **Où on en est :** la demande auprès de la SignPath Foundation est en cours. En attendant, les versions ne sont pas
> signées (Authenticode). Mindset n'installe ou ne met à jour Mindcare qu'à partir d'un `Mindcare.zip` qui porte une
> signature RSA valide du responsable (`Mindcare.zip.sig`).

## Ce qui est signé

- `Mindcare.exe` et les scripts PowerShell (`.ps1`) de Mindcare (`fichiers\Mindcare.ps1` et `fichiers\modules\*.ps1`),
  tous construits depuis ce dépôt.
- Les autres fichiers de l'appli sont des données, pas des programmes, et ne sont pas signés : le catalogue des pannes
  (`enquetes.json`), les dessins (`.svg`), la mise en page de la fenêtre (`.xaml`), les clés publiques de la suite
  (`.xml`) et un petit fichier source C# (`natif.cs`) que Windows compile au démarrage de Mindcare. `catalogue.zip`,
  publié avec chaque version pour l'appli de téléphone Mindcare Mobile, ne contient que `enquetes.json` et les dessins.
- Les fichiers d'autres auteurs ne sont jamais signés avec ce certificat, et ne sont pas dans `Mindcare.zip`. Ils ne
  sont téléchargés que si l'utilisateur demande l'assistant (voir la confidentialité plus bas) :
  - llama.cpp (`llama-server.exe` et ses DLL), version Windows officielle `b11541` (Vulkan, x64) de
    [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp), licence MIT. Mindcare vérifie son empreinte SHA-256
    avant de s'en servir.
  - Le modèle de l'assistant (`mindcare-0.6b-v2-q8_0.gguf`), un fichier de données entraîné pour Mindcare à partir de
    [Qwen3 0.6B](https://huggingface.co/Qwen/Qwen3-0.6B) (équipe Qwen, licence Apache 2.0), publié dans la version
    [ia-v2](../../releases/tag/ia-v2) de ce dépôt sous la même licence Apache 2.0. Mindcare vérifie son empreinte
    SHA-256 avant de s'en servir.

## Comment une version est construite et signée

1. La version reçoit son étiquette dans ce dépôt (`v<version>`).
2. Le [workflow Release](.github/workflows/release.yml) construit `Mindcare.exe` et `catalogue.zip` sur une machine de
   GitHub (`outils\construire.ps1`), seulement depuis le code de cette étiquette, et dépose le dossier `Mindcare` comme
   artefact (`catalogue.zip` est déposé à part, et n'est pas envoyé à la signature).
3. Le workflow envoie une demande de signature à SignPath, qui vérifie que l'artefact vient bien de ce workflow et de
   ce dépôt. **Chaque demande est approuvée à la main** par un approbateur (voir plus bas).
4. Le responsable récupère le `Mindcare.zip` signé construit par le workflow, ajoute la signature RSA séparée
   `Mindcare.zip.sig` (Mindset s'en sert pour vérifier les installations et les mises à jour ; elle ne change aucun
   fichier du zip) et publie la version.

## Équipe et rôles

| Rôle | Membres |
|---|---|
| Auteurs et relecteurs | [Jordan Jacquot](https://github.com/JordanJacquot) |
| Approbateurs | [Jordan Jacquot](https://github.com/JordanJacquot) |

- Les auteurs peuvent modifier le code de ce dépôt. Les changements proposés par quelqu'un d'autre (pull requests)
  sont relus par un auteur avant d'être acceptés.
- Chaque demande de signature est approuvée par un approbateur.
- Tous les membres de l'équipe utilisent la double authentification sur GitHub et sur SignPath.

## Confidentialité

Ce programme n'envoie aucune information vers d'autres systèmes en réseau, sauf si l'utilisateur, ou la personne qui
l'installe ou s'en sert, le demande.

Mindcare n'a ni compte, ni publicité, ni télémétrie, ni statistiques. Elle ne se connecte jamais à Internet d'elle-même :
il n'y a aucune connexion automatique à couper. L'installation et les mises à jour de Mindcare sont faites par Mindset
(voir [la politique de Mindset](https://github.com/JordanJacquot/Mindset/blob/main/CODE_SIGNING.md)). Voici tout ce
que Mindcare peut envoyer, toujours après un clic.

### Seulement quand tu le demandes (un bouton)

| Quoi | Quand | Ce qui est envoyé |
|---|---|---|
| Téléchargement de l'assistant : llama.cpp depuis `github.com/ggml-org/llama.cpp` (environ 33 Mo) et le modèle depuis `github.com/JordanJacquot/Mindcare` (environ 640 Mo). GitHub donne les fichiers depuis ses serveurs de téléchargement (`*.githubusercontent.com`) | Quand tu appuies sur « Installer » (ou « Mettre à jour », ou « Réessayer ») dans le bandeau de l'accueil. Le bandeau donne la taille avant | Des demandes HTTPS ordinaires pour ces deux fichiers (nom du programme `Mindcare`) ; rien sur toi ni sur ton PC |
| La page de téléchargement de Mindset, `github.com/JordanJacquot/Mindset/releases/latest` | Seulement quand Mindcare est ouverte sans que Mindset soit installé, et que tu appuies sur « Télécharger Mindset » | Elle s'ouvre dans ton navigateur |

Ce qui ne quitte jamais ton PC :
- L'assistant tourne seulement sur ton PC. `llama-server` n'écoute que `127.0.0.1` (ce PC) : les phrases que tu écris
  ne sortent pas du PC, et l'assistant marche sans Internet une fois installé.
- Ce que Windows a noté (redémarrages brutaux, écrans bleus, test de mémoire, disques) et la liste des barrettes de
  mémoire sont lus sur ton PC, sans droits administrateur, et seulement montrés dans Mindcare et dans son rapport.
- Le rapport pour le réparateur est enregistré sur ton Bureau ou copié, seulement quand tu appuies sur le bouton ;
  c'est toi qui choisis à qui tu l'envoies.

Services d'autres entreprises : les téléchargements ci-dessus viennent de GitHub, qui relève de sa
[déclaration de confidentialité](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).

## Ce qui change sur le PC, et la désinstallation

- Mindcare ne change rien dans Windows. Elle lit seulement ce que Windows a noté. Quand une commande peut régler une
  panne, elle est affichée avec un bouton « Copier » et une explication : Mindcare ne la lance jamais elle-même.
- Ses fichiers : le programme dans le dossier des programmes de Mindset, et ses données (réglages, diagnostics gardés,
  journal, et l'assistant s'il est installé, environ 700 Mo) dans `%LOCALAPPDATA%\Mindset\Mindcare` (ou
  `X:\Mindset\Donnees\Mindcare` quand Mindset est installé sur un autre disque `X:`).
- L'assistant (`llama-server`) s'arrête avec Mindcare, même quand Mindcare est fermée de force (Mindset le fait avant
  une mise à jour ou une désinstallation).
- Mindcare est installée, mise à jour et désinstallée par Mindset : « Désinstaller » sur la carte de Mindcare dans
  Mindset supprime le programme ; son dossier de données est gardé, pour qu'une nouvelle installation retrouve les
  diagnostics. Le désinstalleur de Mindset (`Desinstaller Mindset.exe`, aussi dans « Applications installées » de
  Windows) supprime Mindcare avec Mindset, et aussi son dossier de données (avec l'assistant) quand tu choisis de
  supprimer les données.
