# Mesure de l'IA de Mindcare (2026-10-23 05:50)

Modèle : `mindcare-0.6b-nuit4-q8_0.gguf` ; 160 phrases jamais vues (145 pannes).

| | Bonne panne en 1er | Bonne panne dans les 3 premières |
|---|---|---|
| L'IA | 88.1 % | |
| L'IA, puis la recherche par mots | | 95.6 % |
| La recherche par mots actuelle | 77.5 % | 89.4 % |

L'IA trouve aussi exactement les réponses déjà données dans 61.9 % des phrases.
Temps de réponse de l'IA sur le PC : 0.05 s (médiane).

## Confusions les plus fréquentes

- rien (Rien ne s'allume) pris pour plusieursappuis (Il faut appuyer plusieurs fois pour qu'il s'allume) : 1 fois
- ecrannoir (Écran noir, PC allumé) pris pour memoiretraining (Écran noir très long au premier démarrage) : 1 fois
- seteint (Il s'éteint d'un coup) pris pour ecranseteint (L'écran s'éteint trop vite) : 1 fois
- toutlent (Tout est lent) pris pour processeurbride (Le processeur reste bloqué à basse vitesse) : 1 fois
- ledcartemere (Un voyant reste allumé sur la carte mère (CPU, DRAM, VGA ou BOOT)) pris pour ledgpu (Une lumière rouge sur la carte graphique) : 1 fois
- plusinternet (Plus d'internet) pris pour proxy (Le navigateur dit « pas de connexion », mais d'autres applis marchent) : 1 fois
- saccades (Les jeux saccadent (peu d'images par seconde)) pris pour frequence (L'écran reste à 60 Hz au lieu de 144 Hz (ou plus)) : 1 fois
- jeu (Ça plante seulement en jeu) pris pour programme (Un programme ou un jeu plante) : 1 fois
- touches (Des touches ne marchent plus (ou tapent autre chose)) pris pour toucherepete (Des lettres se répètent ou le clavier tape tout seul) : 1 fois
- imprimante (L'imprimante ne marche plus) pris pour impressionmauvaise (L'imprimante imprime mal (pâle, traînées, couleurs fausses)) : 1 fois
- necharge (Il ne charge plus) pris pour chargelente (Il charge très lentement) : 1 fois
- barretaches (La barre des tâches ou le menu Démarrer ne répond plus) pris pour pubswindows (Des applis ou des pubs apparaissent dans le menu Démarrer) : 1 fois
- fige (Il se fige) pris pour reinitialiser (Je veux remettre Windows à zéro (réinitialiser le PC)) : 1 fois
- taille (Tout est trop grand ou trop petit) pris pour modesansechec (Windows démarre toujours en mode sans échec) : 1 fois
- eteintbatterie (Il s'éteint alors qu'il reste de la batterie) pris pour secteurseul (Le portable ne marche que branché sur le chargeur) : 1 fois

## Exemples d'erreurs

- « mon pc s'allume plus du tout » : attendu rien, l'IA dit plusieursappuis
- « les ventilos tournent mais rien a l'ecran » : attendu ecrannoir, l'IA dit memoiretraining
- « le pc s'éteint tout seul au bout de 5 min » : attendu seteint, l'IA dit ecranseteint
- « mon pc est hyper lent depuis quelques jours » : attendu toutlent, l'IA dit processeurbride
- « y a une lumiere rouge sur la carte mere a coté de dram » : attendu ledcartemere, l'IA dit ledgpu
- « il dit connecté sans internet » : attendu plusinternet, l'IA dit proxy
- « j'ai que 30 fps dans mes jeux » : attendu saccades, l'IA dit frequence
- « mon jeu crash tout le temps au bout de 10 min » : attendu jeu, l'IA dit programme
- « certaines touches écrivent pas » : attendu touches, l'IA dit toucherepete
- « mon imprimante imprime plus » : attendu imprimante, l'IA dit impressionmauvaise
- « mon portable charge plus » : attendu necharge, l'IA dit chargelente
- « le menu démarrer s'ouvre plus » : attendu barretaches, l'IA dit pubswindows
- « le pc freeze complètement » : attendu fige, l'IA dit reinitialiser
- « tout est en gros sur l'écran » : attendu taille, l'IA dit modesansechec
- « il s'éteint quand je débranche le chargeur » : attendu eteintbatterie, l'IA dit secteurseul
- « accès refusé quand je veux supprimer un dossier » : attendu accesrefuse, l'IA dit suppressionimpossible
- « je retrouve plus mon document » : attendu fichierintrouvable, l'IA dit fichiersupprime
- « depuis que j'ai nettoyé la tour elle démarre plus » : attendu apresnettoyage, l'IA dit reparationboucle
- « quand je tape le texte part n'importe ou sur mon portable » : attendu paumepave, l'IA dit touchewindows
