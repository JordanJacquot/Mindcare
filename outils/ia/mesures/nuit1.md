# Mesure de l'IA de Mindcare (2026-10-21 06:04)

Modèle : `mindcare-0.6b-nuit1-q8_0.gguf` ; 2069 phrases jamais vues (229 pannes).

| | Bonne panne en 1er | Bonne panne dans les 3 premières |
|---|---|---|
| L'IA | 98.2 % | |
| L'IA, puis la recherche par mots | | 99.0 % |
| La recherche par mots actuelle | 78.2 % | 89.8 % |

L'IA trouve aussi exactement les réponses déjà données dans 81.2 % des phrases.
Temps de réponse de l'IA sur le PC : 0.06 s (médiane).

## Confusions les plus fréquentes

- piloteaffichage (« Le pilote d'affichage a cessé de répondre » ou écran noir quelques secondes) pris pour signalcoupe (L'écran perd le signal quelques secondes, puis revient) : 2 fois
- cablereseau (Le câble réseau ne marche pas) pris pour plusinternet (Plus d'internet) : 2 fois
- tientpeu (La batterie tient peu) pris pour gonfle (La batterie gonfle) : 2 fois
- seteint (Il s'éteint d'un coup) pris pour rameheures (Il rame au bout de quelques heures) : 1 fois
- seteint (Il s'éteint d'un coup) pris pour orage (Plus rien après un orage ou une coupure de courant) : 1 fois
- fige (Il se fige) pris pour toutlent (Tout est lent) : 1 fois
- fige (Il se fige) pris pour erreursdisque (Le disque fait des erreurs) : 1 fois
- ecranbleu (Un écran bleu apparaît) pris pour nodisque (« Aucun disque » au démarrage) : 1 fois
- programme (Un programme ou un jeu plante) pris pour programmesinconnus (Des programmes inconnus) : 1 fois
- programme (Un programme ou un jeu plante) pris pour unsite (Un seul site ou une seule appli ne marche pas) : 1 fois
- programme (Un programme ou un jeu plante) pris pour dpiprog (Un seul programme est flou ou minuscule) : 1 fois
- sortveille (Il ne sort pas de la veille) pris pour ecranseteint (L'écran s'éteint trop vite) : 1 fois
- apresnettoyage (Il ne s'allume plus après avoir été nettoyé, déplacé ou ouvert) pris pour orage (Plus rien après un orage ou une coupure de courant) : 1 fois
- demarragelong (Le démarrage est très long) pris pour apresmaj (Il rame depuis une mise à jour) : 1 fois
- artefacts (Des traits, des carrés ou des couleurs bizarres) pris pour portablecasse (Écran du portable cassé ou taché) : 1 fois

## Exemples d'erreurs

- « Juste avant qu'il ne s'éteigne, le PC devenait très chaud et les ventilateurs géraient un son vraiment fort, comme s'il allait craquer. » : attendu seteint, l'IA dit rameheures
- « Du coup, je me suis dit qu’il y avait une coupure, mais en cherchant, je n’ai rien senti de brûlé ni rien entendu de claquement, juste un silence total. » : attendu seteint, l'IA dit orage
- « C’est hallucinant, mon ordinateur est complètement gelé, comme s’il avait été en freeze et je ne sais même plus si c’est normal ou si j’ai fait une erreur grave ! » : attendu fige, l'IA dit toutlent
- « Il est complet, le PC est gelé, comme s’il était en freeze, et je crois que j’entends des sons de grattage ou de clics quand je tente d’ouvrir un dossier... je commence à paniquer ! » : attendu fige, l'IA dit erreursdisque
- « Salut ! Mon PC est coincé, il affiche un écran bleu et redémarre, j’ai un message genre 'INACCESSIBLE_BOOT_DEVICE' je crois… est-ce que tu pourrais me dire ce qu’il faut faire ? » : attendu ecranbleu, l'IA dit nodisque
- « Quand plusieurs logiciels placent soudainement sans avertissement ? » : attendu programme, l'IA dit programmesinconnus
- « L'appli de streaming s'est figée, elle ne répond plus du tout. » : attendu programme, l'IA dit unsite
- « C’est bizarre, depuis hier, mon navigateur, Word, et aussi un jeu que j’aimais, ça ne répond plus, comme si le tout plafonnait tout seul, je crois que c’est une erreur de système. » : attendu programme, l'IA dit dpiprog
- « Écran qui devient noir pendant 2 secondes puis revient, ça arrive de plus en plus souvent » : attendu piloteaffichage, l'IA dit signalcoupe
- « Salut, j'ai un truc bizarre sur mon PC : il fait des pannes d'écran, ça devient noir quelques secondes puis revient, tu connais ce genre de chose ? » : attendu piloteaffichage, l'IA dit signalcoupe
- « L'ordinateur s'éteint automatiquement après quelques heures, et quand je le reprends, l'écran reste noir, je dois appuyer fortement sur le bouton d'alimentation pour qu'il réactive l'affichage. » : attendu sortveille, l'IA dit ecranseteint
- « Du coup, j'ai ouvert mon PC il y a une heure, il marchait, mais après, il ne veut plus démarrer... euh, genre, rien ne se passe quand je clique sur le bouton d'allumage. » : attendu apresnettoyage, l'IA dit orage
- « Windows update en cours ? j'ai 5 min de chargement tout le temps, genre freeze au milieu » : attendu demarragelong, l'IA dit apresmaj
- « euh, genre depuis un moment, y a des lignes qui sortent de l'écran, du coup même au démarrage, avant Windows, c'est pas normal » : attendu artefacts, l'IA dit portablecasse
- « lol le chrome est tout flou et les lettres sont micro, comme si le screen était en mode ppp » : attendu dpiprog, l'IA dit microgels
- « Mon fils a un vieux PC qui, il y a deux jours, a commencé à être super lent, à ramer quand il ouvre ses cours, et les ventilateurs crient vraiment fort quand il lance des applications. » : attendu toutlent, l'IA dit rameheures
- « C’est absolument fou, l’Explorateur de fichiers s’arrête tout le temps, comme s’il était en freeze, et la barre verte ne bouge jamais, ça dure des heures à chaque fois ! » : attendu dossierslents, l'IA dit explorateurplante
- « J’ai l’impression que mon PC grince un peu comme un vieux tapis, et ce clic tout le temps, c’est vrai que ça me met la peur au cœur, même quand je ne l’utilise pas, il fait ça comme si quelqu’un le frotte doucement » : attendu cliquetis, l'IA dit ventilobruyant
- « Salut, j’ai un grésillement électrique qui sort du chargeur, je me demande si c’est normal ou s’il y a un problème. » : attendu sifflement, l'IA dit etincelles
- « PC: ping ping ping tout le temps, même quand rien ne bouge, help » : attendu bruitsusb, l'IA dit piloteaffichage
- « Mon fils ne peut pas faire ses devoirs en ligne, il dit que le PC est hors ligne et que rien ne s’ouvre. » : attendu plusinternet, l'IA dit cablereseau
- « Même avec Wi-Fi, mon PC freeze sur le chargement du site, rien ne passe, y a internet chez moi ? 📶 » : attendu plusinternet, l'IA dit erreurdns
- « Aucun internet sur le téléphone, les écrans ou les appareils de la maison, même avec Wi-Fi actif » : attendu cablereseau, l'IA dit plusinternet
- « Mon fils a un vieux PC qui ne trouve pas internet, et il a testé avec son téléphone, mais aucun appareil de la maison ne marche en réseau, même pas en mode Wi-Fi. » : attendu cablereseau, l'IA dit plusinternet
- « Je suis super inquiète parce que depuis hier, mon application de musique ne veut pas lancer, mais tout le reste du navigateur et internet fonctionne parfaitement, comme si elle était en freeze. » : attendu unsite, l'IA dit demarragelong
- « Euh… du coup, je sais pas si c'est normal, mais je peux pas ouvrir la page du métro en ligne, ça fait un petit clic et puis rien, genre comme si ça bloquait, mais internet marche ailleurs, j'ai déjà essayé de fermer et relancer. » : attendu unsite, l'IA dit erreurdns
- « Le PC met des heures à passer de l'état d'éteint à l'écran de démarrage, parfois plus de 30 minutes pour voir apparaître le logo, et dans certains cas, l'écran reste noir après un long temps de charge. » : attendu chargelente, l'IA dit demarragelong
- « Mon portable tombe souvent en panne de batterie et il ne tient plus à plat, le dessous se soulève quand je le pose sur la table » : attendu tientpeu, l'IA dit gonfle
- « Euh, du coup le portable ne tient plus à plat, je vois le clavier qui se soulève un peu quand je le pose sur la table... » : attendu tientpeu, l'IA dit gonfle
- « Mon fils a un portable qui, dès qu’il est mis en mode capot ou éteint, perd toute sa batterie. Hier soir, il l’a laissé en mode « fermé », et ce matin, il avait moins de 10 %, alors qu’il ne l’a pas utilisé depuis le soir. » : attendu videeteint, l'IA dit eteintbatterie
- « Du coup, depuis hier, le PC fait des petits sons comme des clics et il devient très lent, genre, comme s’il cherchait à lire un truc qui manque... » : attendu erreursdisque, l'IA dit bruitsusb
- « Problème de programmes par défaut : tout ouvre dans le navigateur ou dans un logiciel bizarre » : attendu mauvaisprogramme, l'IA dit programme
- « Mon fils a un fichier vidéo en MP4 qu’il a téléchargé, mais quand il l’ouvre, rien ne se passe, l’écran reste noir et il ne fait pas de son, il me dit que c’est pareil depuis plusieurs heures. » : attendu video, l'IA dit fichierbloque
- « Mon PC bloque tout de suite au milieu du jeu, il fait comme si il ralentissait, et j’ai l’impression qu’il est chaud mais je ne sens plus le souffle des ventilateurs, c’est bizarre » : attendu saccades, l'IA dit rameheures
- « Mon jeu est en train de s’effondrer, comme si l’écran s’arrêtait et recommençait, et je me demande si ce n’est pas parce que mon PC est en surchauffe, mais je ne vois plus les ventilateurs qui fonctionnent du tout ! » : attendu saccades, l'IA dit jeu
- « PC fixe = freeze en jeu, pas en pause, y a des saccades surtout quand je joue à des jeux en 3D » : attendu jeu, l'IA dit saccades
- « Le jeu s'arrête soudain, et après, Windows reste là, rien ne bouge, comme si le PC avait planté juste en plein match. » : attendu jeu, l'IA dit jeunelancepas
- « J’ai un problème quand je joue, le PC s’éteint tout d’un coup et j’entends un claquement, comme si quelque chose brûlait dedans. » : attendu jeu, l'IA dit seteint
