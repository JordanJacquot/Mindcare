# Mindcare

Trouve pourquoi ton PC est en panne, en répondant à quelques questions simples.

## Installer

Mindcare s'installe avec **[Mindset](https://github.com/JordanJacquot/Mindset/releases/latest)**, le portail de la suite :
installe Mindset, puis Mindcare depuis Mindset, qui la garde ensuite à jour et l'affiche dans sa fenêtre.

## Ce qu'elle sait faire (0.8.0)

- Un catalogue de 222 pannes rangé en 14 familles (allumage, démarrage de Windows, écran, plantages, jeux vidéo, lenteur,
  bruits et chaleur, son, internet, clavier et souris, batterie, virus et arnaques, fichiers, Windows et compte), et une recherche par mots (« bip », « wifi »,
  « écran bleu »...) qui marche sans internet.
- L'accueil en questions guidées : « 1. Ça concerne quoi ? » (12 familles), puis « 2. Qu'est-ce qui se passe ? » ;
  ou on écrit son problème avec ses mots, tout en haut.
- Une enquête par élimination : les suspects à gauche, barrés au fur et à mesure ; une question ou un test à la fois
  (le test de mémoire de Windows, le nettoyage, les mises à jour, la multiprise...), avec « Étape 2 sur 5 » et une case
  à cocher par geste, jusqu'à une réponse nette
  (« Trouvé : une barrette de mémoire est abîmée »), ou ce qui reste pour un réparateur, avec le rapport.
- Ouverte sur le PC en panne, elle lit ce que Windows a noté (redémarrages brutaux, écrans bleus, test de mémoire,
  disque...) et les barrettes de mémoire, sans droits administrateur et sans rien modifier.
- Elle guide sans rien faire à ta place : quand la panne se règle dans Windows (le service audio, les réglages du
  son, la file d'impression...), elle fait faire les manipulations une à une avant d'accuser le matériel.
- Quand une commande règle la panne (oublier le Wi-Fi enregistré, vider le carnet d'adresses d'internet, réparer les
  fichiers de Windows), elle l'affiche avec un bouton « Copier » et explique comment la coller dans le Terminal ; elle ne
  la lance jamais elle-même.
- L'enquête en cours se reprend (par exemple après un test qui redémarre le PC), un rapport pour le réparateur (Bureau ou
  copie), la carte de Mindset avec la dernière réponse.
- Tous les thèmes de la suite, thèmes perso compris, et un logo animé (un pouls).

## Plan

- Refonte du 19 octobre 2026 (après la relecture de Jordan) : enquête par élimination, catalogue de 12 familles et
  72 pannes (maquettes validées). Fait : le moteur, les écrans, la famille Plantages (0.4.0), puis les 11 autres
  familles (0.5.0). Ensuite : les cas que Jordan rencontrera.

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
      - 0.1.0 relue le 2026-10-18 : écrans trop bruts, conclusion illisible, RAM oubliée. Maquettes au choix (question C,
        conclusion B), logo animé, fond revu : 0.2.0.
- [x] 7. Première publication : Mindcare 0.1.0, puis Mindset qui la propose (Mindset 1.1.0, le 2026-10-18 : Jordan
      préfère tout tester dans Mindset, la relecture se fait donc sur la version publiée).

## Pour les développeurs

- `Mindcare\fichiers\enquetes.json` : les familles, les pannes, leurs suspects et leurs étapes (questions,
  vérifications automatiques, tests) ; chaque réponse écarte, garde ou confirme des suspects. **Catalogue partagé avec
  Mindcare Mobile** (avec `images\`) : il ne s'écrit qu'ici, voir `CLAUDE.md`.
- `Mindcare\fichiers\Mindcare.ps1` : le démarrage ; `modules\` : le code découpé par partie.
  - `contenu.ps1` : **ce que fait l'appli** : les écrans du diagnostic ;
  - `enquete.ps1` : le moteur des enquêtes (sans affichage) et la recherche par mots ;
  - `historique.ps1` : les diagnostics gardés (`diagnostics.json`), le résumé pour le réparateur, la carte de Mindset ;
  - `journaux.ps1` : ce que Windows a noté (lu en arrière-plan au démarrage) ;
  - `images.ps1` : les dessins de `fichiers\images\` (SVG simples, le même format servira au téléphone), dans le thème ;
  - `mindset.ps1`, `donnees.ps1`, `themes.ps1`, `interface.ps1`, `systeme.ps1`, `evenements.ps1` : la vie dans la suite
    (Mindset, données, thème, fenêtre, fermeture, carte vivante), déjà prêts.
- `outils\tester.ps1` : les tests automatiques (hors de l'écran, données à part).
- `outils\construire.ps1` : `Mindcare.exe` et `release\Mindcare.zip`.
- `outils\publier.ps1 -Version 0.1.0 -Notes "..."` : vérifications, construction, tests, signature, Release GitHub.
- Copie de travail (ce dossier, avec `.git`) : lancée directement, elle n'est pas bloquée et garde ses données à part
  (`%LOCALAPPDATA%\Mindset\Mindcare (travail)`).
