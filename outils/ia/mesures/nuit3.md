# Mesure de l'IA de Mindcare (2026-10-22 05:55)

Modèle : `mindcare-0.6b-nuit3-q8_0.gguf` ; 2084 phrases jamais vues (229 pannes).

| | Bonne panne en 1er | Bonne panne dans les 3 premières |
|---|---|---|
| L'IA | 99.2 % | |
| L'IA, puis la recherche par mots | | 99.6 % |
| La recherche par mots actuelle | 76.9 % | 88.2 % |

L'IA trouve aussi exactement les réponses déjà données dans 83.1 % des phrases.
Temps de réponse de l'IA sur le PC : 0.06 s (médiane).

## Confusions les plus fréquentes

- fige (Il se fige) pris pour cliquetis (Des cliquetis ou des grattements) : 1 fois
- sortveille (Il ne sort pas de la veille) pris pour arretlong (Il met très longtemps à s'éteindre) : 1 fois
- voyantclignote (Le voyant clignote mais rien ne démarre (portable)) pris pour capslockhp (Verr. Maj (ou le voyant) clignote et le portable HP ne démarre pas) : 1 fois
- cliquetis (Des cliquetis ou des grattements) pris pour ventilobruyant (Un ventilateur très bruyant) : 1 fois
- wifiabsent (Le Wi-Fi n'apparaît plus) pris pour partage (Le partage de connexion du téléphone ne marche pas) : 1 fois
- wificoupe (Le Wi-Fi coupe souvent) pris pour plusinternet (Plus d'internet) : 1 fois
- cablereseau (Le câble réseau ne marche pas) pris pour partage (Le partage de connexion du téléphone ne marche pas) : 1 fois
- internetlent (Internet est très lent) pris pour plusinternet (Plus d'internet) : 1 fois
- clavier (Le clavier ne répond plus) pris pour toucherepete (Des lettres se répètent ou le clavier tape tout seul) : 1 fois
- fichiersupprime (Un fichier supprimé par erreur) pris pour fichierintrouvable (Je ne retrouve plus un fichier enregistré) : 1 fois
- wordperdu (J'ai fermé Word (ou Excel) sans enregistrer) pris pour fichierintrouvable (Je ne retrouve plus un fichier enregistré) : 1 fois
- activation (Windows n'est pas activé) pris pour office (Word ou Excel demande une activation (« Produit sans licence »)) : 1 fois
- store (Le Microsoft Store ne marche pas) pris pour unsite (Un seul site ou une seule appli ne marche pas) : 1 fois
- jeu (Ça plante seulement en jeu) pris pour ecranseteint (L'écran s'éteint trop vite) : 1 fois
- jeu (Ça plante seulement en jeu) pris pour piloteaffichage (« Le pilote d'affichage a cessé de répondre » ou écran noir quelques secondes) : 1 fois

## Exemples d'erreurs

- « PC freeze au lancement de Minecraft, clique + grattage. Toujours pareil. Aide ? » : attendu fige, l'IA dit cliquetis
- « L'ordinateur s'éteint automatiquement après quelques heures, et quand je le redémarre, l'écran reste noir, il faut appuyer plusieurs fois sur le bouton d'alimentation pour qu'il se relance, sinon rien ne se passe. » : attendu sortveille, l'IA dit arretlong
- « C’est bizarre, ça clignote comme un code, un rouge puis un blanc, et quand je le touche, il ne démarre pas du tout, comme s’il était en freeze mais sans réagir à rien. » : attendu voyantclignote, l'IA dit capslockhp
- « J'ai l'impression que mon PC grince comme un vieux mécanisme, surtout quand je clique sur mes fichiers, et parfois ça fait un petit tic-tac qui me fait vraiment peur, comme si c'était en panne profonde. » : attendu cliquetis, l'IA dit ventilobruyant
- « C'est absolument folie ! Mon téléphone voit le wifi, mon portable le voit, mais mon PC ne montre aucune connexion, comme si le tout était tombé en panne ! » : attendu wifiabsent, l'IA dit partage
- « Le Wi-Fi de la maison s'arrête tout seul et personne n’a internet, même le téléphone sans fil ? » : attendu wificoupe, l'IA dit plusinternet
- « Aucun internet sur le téléphone, les écrans ou les appareils de la maison, même avec Wi-Fi actif » : attendu cablereseau, l'IA dit partage
- « Mon ordinateur est vraiment en freeze, les pages ne chargent jamais, c’est comme si le réseau était coincé ! Mais sur mon téléphone, le Wi-Fi marche bien, donc je crois que c’est seulement le PC qui a un problème. » : attendu internetlent, l'IA dit plusinternet
- « Y a-t-il un liquide ou des miettes sur le clavier qui rendent les touches inactives ? » : attendu clavier, l'IA dit toucherepete
- « Salut ! J’ai perdu un fichier important sur mon PC, un document que j’ai ouvert ce matin, il est disparau, je cherche une solution vite fait. » : attendu fichiersupprime, l'IA dit fichierintrouvable
- « Mon PC a tout simplement stoppé au moment où je voulais enregistrer mon document, j'ai cru que c'était perdu, j'ai tout relancé mais il ne réapparaît pas, est-ce qu'il est possible de le récupérer ? » : attendu wordperdu, l'IA dit fichierintrouvable
- « Mon fils a un ordi qui bloque dès qu’il veut ouvrir un fichier, et il a un message en bas : 'Activation requise', il ne peut pas l’utiliser. » : attendu activation, l'IA dit office
- « Euh, du coup je me suis rendu compte que l'appli boutique ne charge plus, genre, je clique sur le bouton et ça reste figé comme un mur. » : attendu store, l'IA dit unsite
- « Écran noir 2s en jeu → revient tout de suite, ça ne se passe pas quand je regarde des vidéos ou ouvre des sites » : attendu jeu, l'IA dit ecranseteint
- « Lorsque mon fils joue à un fps, l’écran s’arrête pendant 2 secondes, puis reprend, mais il n’y a pas de crash, juste un noir court » : attendu jeu, l'IA dit piloteaffichage
- « Du coup, quand je m’installe dans un jeu, le PC s’arrête soudainement… euh… genre, rien ne répond, c’est un freeze, mais il n’est pas chaud du tout, les fans sont tranquilles, je me demande si c’est la carte graphique qui a un problème. » : attendu jeu, l'IA dit installation
