# Mindcare

Trouve pourquoi ton PC est en panne, en répondant à quelques questions simples.

## Installer

Mindcare s'installe avec **[Mindset](https://github.com/JordanJacquot/Mindset/releases/latest)**, le portail de la suite :
installe Mindset, puis Mindcare depuis Mindset, qui la garde ensuite à jour et l'affiche dans sa fenêtre.

## Ce qu'elle sait faire (0.1.0)

- Le diagnostic de 5 pannes : rien ne s'allume, l'écran reste noir, il s'éteint ou redémarre tout seul, écran bleu,
  très lent. Les questions une à une, avec « Je ne sais pas » et « Retour », puis la conclusion : la cause la plus
  probable, ce qu'on peut faire soi-même, quand aller chez un réparateur, les consignes de sécurité, et le rappel des
  réponses. Quand la vraie panne est une autre, un bouton passe directement à son diagnostic.
- Les conseils suivent les pages d'aide de Microsoft, Dell et HP.
- Des dessins à côté des questions où ils aident (l'interrupteur de la tour, les prises de l'écran, la lampe sur
  l'écran du portable...), dans les couleurs du thème de la suite.
- Les 10 derniers diagnostics gardés : en reprendre un pas fini (proposé en haut de l'accueil), revoir une conclusion.
- Un résumé pour le réparateur (la panne, la conclusion, les réponses, le modèle du PC), enregistré sur le Bureau ou
  copié pour l'envoyer.
- Dans Mindset, la carte de Mindcare montre le dernier diagnostic.

## Plan

Validé avec Jordan le 2026-10-18 (fiche : [CADRAGE.md](CADRAGE.md)).

- [x] 1. Le squelette dans Mindset : appli vide en orange, affichée dans un onglet de Mindset, carte « Aucun diagnostic
      pour l'instant ».
- [x] 2. Une panne complète, « Il s'éteint tout seul » (le cas du cousin) : choix de la panne, questions une à une avec
      « Je ne sais pas » et retour, conclusion (texte seulement).
- [x] 3. Les 4 autres pannes : rien ne s'allume, écran noir, écran bleu, très lent (conseils vérifiés sur des sources
      sérieuses ; un test parcourt chaque chemin, aucune impasse).
- [x] 4. Les images des questions, dessinées dans le style de la suite.
- [x] 5. Les diagnostics gardés : les 10 derniers, reprise d'un diagnostic pas fini, résumé pour le réparateur, carte de
      Mindset avec le dernier diagnostic.
- [ ] 6. Relecture par Jordan (pannes « pour de faux », dans Mindset) et corrections (0.1.x).
- [x] 7. Première publication : Mindcare 0.1.0, puis Mindset qui la propose (Mindset 1.1.0, le 2026-10-18 : Jordan
      préfère tout tester dans Mindset, la relecture se fait donc sur la version publiée).

## Pour les développeurs

- `Mindcare\fichiers\questions.json` : les pannes, leurs questions et leurs causes (le même fichier pour le futur
  téléphone) ; chaque réponse donne des points à des causes, la conclusion les classe.
- `Mindcare\fichiers\Mindcare.ps1` : le démarrage ; `modules\` : le code découpé par partie.
  - `contenu.ps1` : **ce que fait l'appli** : les écrans du diagnostic ;
  - `diagnostic.ps1` : le calcul de la conclusion (sans affichage) ;
  - `historique.ps1` : les diagnostics gardés (`diagnostics.json`), le résumé pour le réparateur, la carte de Mindset ;
  - `images.ps1` : les dessins de `fichiers\images\` (SVG simples, le même format servira au téléphone), dans le thème ;
  - `mindset.ps1`, `donnees.ps1`, `themes.ps1`, `interface.ps1`, `systeme.ps1`, `evenements.ps1` : la vie dans la suite
    (Mindset, données, thème, fenêtre, fermeture, carte vivante), déjà prêts.
- `outils\tester.ps1` : les tests automatiques (hors de l'écran, données à part).
- `outils\construire.ps1` : `Mindcare.exe` et `release\Mindcare.zip`.
- `outils\publier.ps1 -Version 0.1.0 -Notes "..."` : vérifications, construction, tests, signature, Release GitHub.
- Copie de travail (ce dossier, avec `.git`) : lancée directement, elle n'est pas bloquée et garde ses données à part
  (`%LOCALAPPDATA%\Mindset\Mindcare (travail)`).
