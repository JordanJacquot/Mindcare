# Mesure de l'IA de Mindcare (2026-10-21 06:01)

Modèle : `mindcare-0.6b-nuit1-q8_0.gguf` ; 119 phrases jamais vues (104 pannes).

| | Bonne panne en 1er | Bonne panne dans les 3 premières |
|---|---|---|
| L'IA | 84.9 % | |
| L'IA, puis la recherche par mots | | 96.6 % |
| La recherche par mots actuelle | 76.5 % | 89.1 % |

L'IA trouve aussi exactement les réponses déjà données dans 67.2 % des phrases.
Temps de réponse de l'IA sur le PC : 0.05 s (médiane).

## Confusions les plus fréquentes

- rien (Rien ne s'allume) pris pour plusieursappuis (Il faut appuyer plusieurs fois pour qu'il s'allume) : 1 fois
- ecrannoir (Écran noir, PC allumé) pris pour lumiere (Une lumière, mais rien d'autre) : 1 fois
- seteint (Il s'éteint d'un coup) pris pour ecranseteint (L'écran s'éteint trop vite) : 1 fois
- ecranbleu (Un écran bleu apparaît) pris pour memoireinsuffisante (Message « mémoire insuffisante ») : 1 fois
- plusinternet (Plus d'internet) pris pour cablereseau (Le câble réseau ne marche pas) : 1 fois
- saccades (Les jeux saccadent (peu d'images par seconde)) pris pour frequence (L'écran reste à 60 Hz au lieu de 144 Hz (ou plus)) : 1 fois
- jeu (Ça plante seulement en jeu) pris pour programme (Un programme ou un jeu plante) : 1 fois
- touches (Des touches ne marchent plus (ou tapent autre chose)) pris pour toucherepete (Des lettres se répètent ou le clavier tape tout seul) : 1 fois
- souris (La souris ou le pavé tactile ne marche plus) pris pour sourissaute (Le curseur saute, tremble ou bouge tout seul) : 1 fois
- imprimante (L'imprimante ne marche plus) pris pour impressionmauvaise (L'imprimante imprime mal (pâle, traînées, couleurs fausses)) : 1 fois
- necharge (Il ne charge plus) pris pour chargeurchauffe (Le chargeur chauffe) : 1 fois
- fichiersupprime (Un fichier supprimé par erreur) pris pour suppressionimpossible (Impossible de supprimer ou déplacer un fichier) : 1 fois
- taille (Tout est trop grand ou trop petit) pris pour modesansechec (Windows démarre toujours en mode sans échec) : 1 fois
- clignote (L'écran clignote) pris pour voyantclignote (Le voyant clignote mais rien ne démarre (portable)) : 1 fois
- artefacts (Des traits, des carrés ou des couleurs bizarres) pris pour portablecasse (Écran du portable cassé ou taché) : 1 fois

## Exemples d'erreurs

- « mon pc s'allume plus du tout » : attendu rien, l'IA dit plusieursappuis
- « les ventilos tournent mais rien a l'ecran » : attendu ecrannoir, l'IA dit lumiere
- « le pc s'éteint tout seul au bout de 5 min » : attendu seteint, l'IA dit ecranseteint
- « j'ai un écran bleu memory management » : attendu ecranbleu, l'IA dit memoireinsuffisante
- « il dit connecté sans internet » : attendu plusinternet, l'IA dit cablereseau
- « j'ai que 30 fps dans mes jeux » : attendu saccades, l'IA dit frequence
- « mon jeu crash tout le temps au bout de 10 min » : attendu jeu, l'IA dit programme
- « certaines touches écrivent pas » : attendu touches, l'IA dit toucherepete
- « ma souris bouge plus » : attendu souris, l'IA dit sourissaute
- « mon imprimante imprime plus » : attendu imprimante, l'IA dit impressionmauvaise
- « mon portable charge plus » : attendu necharge, l'IA dit chargeurchauffe
- « j'ai supprimé un fichier sans faire exprès » : attendu fichiersupprime, l'IA dit suppressionimpossible
- « tout est en gros sur l'écran » : attendu taille, l'IA dit modesansechec
- « l'écran clignote » : attendu clignote, l'IA dit voyantclignote
- « il y a des lignes sur l'écran » : attendu artefacts, l'IA dit portablecasse
- « il s'éteint quand je débranche le chargeur » : attendu eteintbatterie, l'IA dit secteurseul
- « il met une plombe à s'éteindre » : attendu arretlong, l'IA dit nodisque
- « je retrouve plus mon document » : attendu fichierintrouvable, l'IA dit fichiersupprime
