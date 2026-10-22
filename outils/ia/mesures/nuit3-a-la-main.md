# Mesure de l'IA de Mindcare (2026-10-22 05:55)

Modèle : `mindcare-0.6b-nuit3-q8_0.gguf` ; 119 phrases jamais vues (104 pannes).

| | Bonne panne en 1er | Bonne panne dans les 3 premières |
|---|---|---|
| L'IA | 89.9 % | |
| L'IA, puis la recherche par mots | | 95.8 % |
| La recherche par mots actuelle | 76.5 % | 89.1 % |

L'IA trouve aussi exactement les réponses déjà données dans 59.7 % des phrases.
Temps de réponse de l'IA sur le PC : 0.05 s (médiane).

## Confusions les plus fréquentes

- rien (Rien ne s'allume) pris pour plusieursappuis (Il faut appuyer plusieurs fois pour qu'il s'allume) : 1 fois
- ecrannoir (Écran noir, PC allumé) pris pour lumiere (Une lumière, mais rien d'autre) : 1 fois
- seteint (Il s'éteint d'un coup) pris pour ecranseteint (L'écran s'éteint trop vite) : 1 fois
- toutlent (Tout est lent) pris pour cpu100 (Le processeur est à 100 %) : 1 fois
- micro (Le micro ne marche pas) pris pour entendmusique (Les autres entendent ma musique ou mon jeu) : 1 fois
- plusinternet (Plus d'internet) pris pour proxy (Le navigateur dit « pas de connexion », mais d'autres applis marchent) : 1 fois
- jeu (Ça plante seulement en jeu) pris pour programme (Un programme ou un jeu plante) : 1 fois
- imprimante (L'imprimante ne marche plus) pris pour impressionmauvaise (L'imprimante imprime mal (pâle, traînées, couleurs fausses)) : 1 fois
- necharge (Il ne charge plus) pris pour chargelente (Il charge très lentement) : 1 fois
- fichiersupprime (Un fichier supprimé par erreur) pris pour suppressionimpossible (Impossible de supprimer ou déplacer un fichier) : 1 fois
- taille (Tout est trop grand ou trop petit) pris pour modesansechec (Windows démarre toujours en mode sans échec) : 1 fois
- eteintbatterie (Il s'éteint alors qu'il reste de la batterie) pris pour secteurseul (Le portable ne marche que branché sur le chargeur) : 1 fois

## Exemples d'erreurs

- « mon pc s'allume plus du tout » : attendu rien, l'IA dit plusieursappuis
- « les ventilos tournent mais rien a l'ecran » : attendu ecrannoir, l'IA dit lumiere
- « le pc s'éteint tout seul au bout de 5 min » : attendu seteint, l'IA dit ecranseteint
- « mon pc est hyper lent depuis quelques jours » : attendu toutlent, l'IA dit cpu100
- « mes potes m'entendent pas sur discord » : attendu micro, l'IA dit entendmusique
- « il dit connecté sans internet » : attendu plusinternet, l'IA dit proxy
- « mon jeu crash tout le temps au bout de 10 min » : attendu jeu, l'IA dit programme
- « mon imprimante imprime plus » : attendu imprimante, l'IA dit impressionmauvaise
- « mon portable charge plus » : attendu necharge, l'IA dit chargelente
- « j'ai supprimé un fichier sans faire exprès » : attendu fichiersupprime, l'IA dit suppressionimpossible
- « tout est en gros sur l'écran » : attendu taille, l'IA dit modesansechec
- « il s'éteint quand je débranche le chargeur » : attendu eteintbatterie, l'IA dit secteurseul
