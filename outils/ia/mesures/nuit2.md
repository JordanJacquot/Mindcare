# Mesure de l'IA de Mindcare (2026-10-22 04:20)

Modèle : `mindcare-0.6b-nuit2-q8_0.gguf` ; 2084 phrases jamais vues (229 pannes).

| | Bonne panne en 1er | Bonne panne dans les 3 premières |
|---|---|---|
| L'IA | 98.6 % | |
| L'IA, puis la recherche par mots | | 99.2 % |
| La recherche par mots actuelle | 76.9 % | 88.2 % |

L'IA trouve aussi exactement les réponses déjà données dans 77.0 % des phrases.
Temps de réponse de l'IA sur le PC : 0.06 s (médiane).

## Confusions les plus fréquentes

- signalcoupe (L'écran perd le signal quelques secondes, puis revient) pris pour piloteaffichage (« Le pilote d'affichage a cessé de répondre » ou écran noir quelques secondes) : 2 fois
- curseurdisparu (Le curseur de la souris a disparu) pris pour souris (La souris ou le pavé tactile ne marche plus) : 2 fois
- fige (Il se fige) pris pour cliquetis (Des cliquetis ou des grattements) : 1 fois
- programme (Un programme ou un jeu plante) pris pour dx12 (« DirectX 12 n'est pas pris en charge » ou « pilote graphique trop ancien ») : 1 fois
- sortveille (Il ne sort pas de la veille) pris pour arretlong (Il met très longtemps à s'éteindre) : 1 fois
- voyantclignote (Le voyant clignote mais rien ne démarre (portable)) pris pour capslockhp (Verr. Maj (ou le voyant) clignote et le portable HP ne démarre pas) : 1 fois
- noirlogo (Écran noir après le logo) pris pour installation (Un programme ne s'installe pas) : 1 fois
- ecrannoir (Écran noir, PC allumé) pris pour noirlogo (Écran noir après le logo) : 1 fois
- couleurs (Les couleurs sont bizarres) pris pour pixels (Des points ou des taches sur l'écran) : 1 fois
- toutlent (Tout est lent) pris pour internetlent (Internet est très lent) : 1 fois
- ventilobruyant (Un ventilateur très bruyant) pris pour cliquetis (Des cliquetis ou des grattements) : 1 fois
- plusinternet (Plus d'internet) pris pour proxy (Le navigateur dit « pas de connexion », mais d'autres applis marchent) : 1 fois
- plusinternet (Plus d'internet) pris pour imprimante (L'imprimante ne marche plus) : 1 fois
- wifiabsent (Le Wi-Fi n'apparaît plus) pris pour partage (Le partage de connexion du téléphone ne marche pas) : 1 fois
- wificoupe (Le Wi-Fi coupe souvent) pris pour plusinternet (Plus d'internet) : 1 fois

## Exemples d'erreurs

- « PC freeze au lancement de Minecraft, clique + grattage. Toujours pareil. Aide ? » : attendu fige, l'IA dit cliquetis
- « Du coup, je me suis demandé si c’était toujours le même jeu qui plante… genre, chaque fois que je le démarre, il ne répond plus, et je me demande si c’est un problème de compatibilité ou si c’est juste un bug. » : attendu programme, l'IA dit dx12
- « L'ordinateur s'éteint automatiquement après quelques heures, et quand je le redémarre, l'écran reste noir, il faut appuyer plusieurs fois sur le bouton d'alimentation pour qu'il se relance, sinon rien ne se passe. » : attendu sortveille, l'IA dit arretlong
- « C’est bizarre, ça clignote comme un code, un rouge puis un blanc, et quand je le touche, il ne démarre pas du tout, comme s’il était en freeze mais sans réagir à rien. » : attendu voyantclignote, l'IA dit capslockhp
- « Euh, genre, le logo de Windows apparaît, puis tout devient noir… et parfois, je vois une flèche qui reste là, comme si le bureau était en train de s’installer, mais rien ne se charge vraiment. » : attendu noirlogo, l'IA dit installation
- « écran noir tout de suite après boot, mais si je bouge la souris ou tape, ça revient, voilà le logo qui passe, ça clignote, pas de freeze ni de lag, juste un noir total au début. » : attendu ecrannoir, l'IA dit noirlogo
- « Problème d'écran : il s'arrête de montrer l'image pendant quelques secondes, puis reprend sans avertissement. » : attendu signalcoupe, l'IA dit piloteaffichage
- « Je l'ai laissé marcher un moment et tout s'est arrêté, puis il a rejoint le noir pendant deux secondes, puis il a repris, c'est bizarre. » : attendu signalcoupe, l'IA dit piloteaffichage
- « Je suis bloqué, l’écran est en noir et blanc mais avec des teintes jaunes qui s’empêchent de disparaître, ça fait une heure que je suis coincé. » : attendu couleurs, l'IA dit pixels
- « Curseur disparu sur mon PC, rien ne réagit quand je déplace la souris. » : attendu curseurdisparu, l'IA dit souris
- « Curseur perdu sur Windows, rien ne réagit quand je déplace la souris » : attendu curseurdisparu, l'IA dit souris
- « Mon fils a vraiment du mal avec son PC, il me dit qu’il rame quand il ouvre ses cours ou ses vidéos, et qu’il a souvent des pauses de 10 secondes entre chaque action, comme si le tout était en train de ralentir. » : attendu toutlent, l'IA dit internetlent
- « Mon PC fait un bruit terrible, comme si un moteur de voiture était à l'intérieur, et ça claque de temps en temps, surtout quand je l'ouvre ou que je fais des vidéos, il me fait vraiment peur ! » : attendu ventilobruyant, l'IA dit cliquetis
- « Euh, du coup je suis coincée, je ne arrive pas à ouvrir mes sites web, c'est comme si mon ordi était complètement hors ligne... » : attendu plusinternet, l'IA dit proxy
- « Mon fils ne peut plus ouvrir les sites web, il dit qu’il est « hors ligne » et que le réseau ne répond plus. » : attendu plusinternet, l'IA dit imprimante
- « C'est absolument folie ! Mon téléphone voit le wifi, mon portable le voit, mais mon PC ne montre aucune connexion, comme si le tout était tombé en panne ! » : attendu wifiabsent, l'IA dit partage
- « Le Wi-Fi de la maison s'arrête tout seul et personne n’a internet, même le téléphone sans fil ? » : attendu wificoupe, l'IA dit plusinternet
- « Aucun internet sur le téléphone, les écrans ou les appareils de la maison, même avec Wi-Fi actif » : attendu cablereseau, l'IA dit plusinternet
- « Mon ordinateur est vraiment en freeze, les pages ne chargent jamais, c’est comme si le réseau était coincé ! Mais sur mon téléphone, le Wi-Fi marche bien, donc je crois que c’est seulement le PC qui a un problème. » : attendu internetlent, l'IA dit plusinternet
- « clavier crash : a→q, 1→2, touche 5 morte. pas de liquide, pas de miettes. freeze sur le clavier parfois. souris ok » : attendu touches, l'IA dit clavier
- « clavier & souris w/o = freeze tout le temps, des touches passent à null, pire, parfois j’arrive pas à cliquer sur rien, ça me fait perdre 10 min par jour à relancer le truc ! » : attendu sansfilcoupe, l'IA dit clavierconnexion
- « Salut, j’ai un problème avec mon téléphone, je veux transférer mes photos sur le PC mais il ne les affiche pas, tu as une idée ? » : attendu telephone, l'IA dit heic
- « Salut ! J’ai perdu un fichier important sur mon PC, un document que j’ai ouvert ce matin, il est disparau, je cherche une solution vite fait. » : attendu fichiersupprime, l'IA dit fichierintrouvable
- « Mon PC a un problème terrible, il a effacé toutes mes photos de vacances, j’ai vérifié la corbeille, le dossier de vidéos, même le disque dur, rien ne se voit, c’est comme s’il avait tout effacé ! » : attendu fichiersupprime, l'IA dit corbeillecachee
- « Mon fils ne trouve plus son devoir de maths qu’il a fait hier soir, il le cherchait dans le dossier « Études » mais il est disparu, on s’est mis à chercher partout sans rien trouver. » : attendu fichierintrouvable, l'IA dit fichiersupprime
- « Les fichiers ne répondent plus, j'appuie deux fois et rien ne se passe, comme si ils étaient bloqués. » : attendu fichiersnouvrent, l'IA dit doubleclic
- « Euh, du coup je me suis rendu compte que l'appli boutique ne charge plus, genre, je clique sur le bouton et ça reste figé comme un mur. » : attendu store, l'IA dit unsite
- « Écran noir 2s en jeu → revient tout de suite, ça ne se passe pas quand je regarde des vidéos ou ouvre des sites » : attendu jeu, l'IA dit piloteaffichage
- « Le jeu freeze tout d’un coup, mais quand je le lance, c’est bon, pas de lag, pas de bruit des ventilo, il reste frais… » : attendu jeu, l'IA dit jeunelancepas
- « Mon fils a du mal à jouer sur son portable, le jeu s’arrête et ça fait des pics de ralentissement, il dit que ça « freeze » quand il active les textures en haut » : attendu memoirevideojeu, l'IA dit jeu
