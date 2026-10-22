# Mesure de l'IA de Mindcare (2026-10-22 04:20)

Modèle : `mindcare-0.6b-nuit2-q8_0.gguf` ; 119 phrases jamais vues (104 pannes).

| | Bonne panne en 1er | Bonne panne dans les 3 premières |
|---|---|---|
| L'IA | 86.6 % | |
| L'IA, puis la recherche par mots | | 95.0 % |
| La recherche par mots actuelle | 76.5 % | 89.1 % |

L'IA trouve aussi exactement les réponses déjà données dans 60.5 % des phrases.
Temps de réponse de l'IA sur le PC : 0.05 s (médiane).

## Confusions les plus fréquentes

- rien (Rien ne s'allume) pris pour allumeseul (Il s'allume tout seul) : 1 fois
- ecrannoir (Écran noir, PC allumé) pris pour lumiere (Une lumière, mais rien d'autre) : 1 fois
- seteint (Il s'éteint d'un coup) pris pour arretlong (Il met très longtemps à s'éteindre) : 1 fois
- seteint (Il s'éteint d'un coup) pris pour eteintbatterie (Il s'éteint alors qu'il reste de la batterie) : 1 fois
- toutlent (Tout est lent) pris pour cpu100 (Le processeur est à 100 %) : 1 fois
- ledcartemere (Un voyant reste allumé sur la carte mère (CPU, DRAM, VGA ou BOOT)) pris pour ledgpu (Une lumière rouge sur la carte graphique) : 1 fois
- sonfort (Le son est beaucoup trop fort ou déformé) pris pour songresille (Le son grésille ou coupe) : 1 fois
- plusinternet (Plus d'internet) pris pour proxy (Le navigateur dit « pas de connexion », mais d'autres applis marchent) : 1 fois
- saccades (Les jeux saccadent (peu d'images par seconde)) pris pour frequence (L'écran reste à 60 Hz au lieu de 144 Hz (ou plus)) : 1 fois
- jeu (Ça plante seulement en jeu) pris pour rameheures (Il rame au bout de quelques heures) : 1 fois
- touches (Des touches ne marchent plus (ou tapent autre chose)) pris pour toucherepete (Des lettres se répètent ou le clavier tape tout seul) : 1 fois
- imprimante (L'imprimante ne marche plus) pris pour impressionmauvaise (L'imprimante imprime mal (pâle, traînées, couleurs fausses)) : 1 fois
- necharge (Il ne charge plus) pris pour chargelente (Il charge très lentement) : 1 fois
- fichiersupprime (Un fichier supprimé par erreur) pris pour suppressionimpossible (Impossible de supprimer ou déplacer un fichier) : 1 fois
- eteintbatterie (Il s'éteint alors qu'il reste de la batterie) pris pour secteurseul (Le portable ne marche que branché sur le chargeur) : 1 fois

## Exemples d'erreurs

- « mon pc s'allume plus du tout » : attendu rien, l'IA dit allumeseul
- « les ventilos tournent mais rien a l'ecran » : attendu ecrannoir, l'IA dit lumiere
- « le pc s'éteint tout seul au bout de 5 min » : attendu seteint, l'IA dit arretlong
- « il s'éteint d'un coup quand je joue » : attendu seteint, l'IA dit eteintbatterie
- « mon pc est hyper lent depuis quelques jours » : attendu toutlent, l'IA dit cpu100
- « y a une lumiere rouge sur la carte mere a coté de dram » : attendu ledcartemere, l'IA dit ledgpu
- « le son crache a fond » : attendu sonfort, l'IA dit songresille
- « il dit connecté sans internet » : attendu plusinternet, l'IA dit proxy
- « j'ai que 30 fps dans mes jeux » : attendu saccades, l'IA dit frequence
- « mon jeu crash tout le temps au bout de 10 min » : attendu jeu, l'IA dit rameheures
- « certaines touches écrivent pas » : attendu touches, l'IA dit toucherepete
- « mon imprimante imprime plus » : attendu imprimante, l'IA dit impressionmauvaise
- « mon portable charge plus » : attendu necharge, l'IA dit chargelente
- « j'ai supprimé un fichier sans faire exprès » : attendu fichiersupprime, l'IA dit suppressionimpossible
- « il s'éteint quand je débranche le chargeur » : attendu eteintbatterie, l'IA dit secteurseul
- « accès refusé quand je veux supprimer un dossier » : attendu accesrefuse, l'IA dit suppressionimpossible
