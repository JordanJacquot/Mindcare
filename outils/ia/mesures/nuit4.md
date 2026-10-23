# Mesure de l'IA de Mindcare (2026-10-23 05:49)

Modèle : `mindcare-0.6b-nuit4-q8_0.gguf` ; 2737 phrases jamais vues (300 pannes).

| | Bonne panne en 1er | Bonne panne dans les 3 premières |
|---|---|---|
| L'IA | 99.2 % | |
| L'IA, puis la recherche par mots | | 99.6 % |
| La recherche par mots actuelle | 76.3 % | 88.8 % |

L'IA trouve aussi exactement les réponses déjà données dans 85.1 % des phrases.
Temps de réponse de l'IA sur le PC : 0.06 s (médiane).

## Confusions les plus fréquentes

- ventilobruyant (Un ventilateur très bruyant) pris pour cliquetis (Des cliquetis ou des grattements) : 2 fois
- cpleindvide (Le disque C: est plein, mais D: est presque vide) pris pour disquedisparu (Un disque (D:, E:) a disparu de « Ce PC ») : 2 fois
- eteintaussitot (Il s'allume puis s'éteint aussitôt) pris pour eteintbouge (Il s'éteint quand on le bouge ou qu'on le touche) : 1 fois
- signalcoupe (L'écran perd le signal quelques secondes, puis revient) pris pour piloteaffichage (« Le pilote d'affichage a cessé de répondre » ou écran noir quelques secondes) : 1 fois
- curseurdisparu (Le curseur de la souris a disparu) pris pour souris (La souris ou le pavé tactile ne marche plus) : 1 fois
- cpu100 (Le processeur est à 100 %) pris pour antimalware (« Antimalware Service Executable » prend tout le processeur) : 1 fois
- vibration (Le PC vibre ou bourdonne) pris pour ventilobruyant (Un ventilateur très bruyant) : 1 fois
- bruitsusb (Des sons de branchement et débranchement sans arrêt) pris pour bourdonnementson (Un bourdonnement dans les haut-parleurs ou le casque) : 1 fois
- ledreseau (Les petites lumières de la prise réseau du PC sont éteintes) pris pour cablereseau (Le câble réseau ne marche pas) : 1 fois
- internetlent (Internet est très lent) pris pour videosaccade (Les vidéos saccadent (YouTube, Netflix...)) : 1 fois
- ethernet100 (Le câble réseau est limité à 100 Mb/s) pris pour internetlent (Internet est très lent) : 1 fois
- touches (Des touches ne marchent plus (ou tapent autre chose)) pris pour toucherepete (Des lettres se répètent ou le clavier tape tout seul) : 1 fois
- antivirusbloque (L'antivirus supprime ou bloque un programme (ou un téléchargement)) pris pour fauxnettoyage (Un programme dit que le PC est infecté et demande de payer) : 1 fois
- fichiersupprime (Un fichier supprimé par erreur) pris pour fichierintrouvable (Je ne retrouve plus un fichier enregistré) : 1 fois
- mdpoublie (Mot de passe ou code PIN oublié) pris pour clavierconnexion (Le clavier ou la souris ne marchent pas sur l'écran de connexion) : 1 fois

## Exemples d'erreurs

- « Du coup, je me demande si c'est un truc de chute ou de liquide, parce que hier soir j'ai eu un petit choc sur le bureau et puis là, ça s'est mis à s'éteindre aussitôt » : attendu eteintaussitot, l'IA dit eteintbouge
- « Écran noir 3-5 secondes, puis revient, comme si il freezait un moment » : attendu signalcoupe, l'IA dit piloteaffichage
- « La souris de mon fils ne marche plus, il ne voit plus le petit point qui suit la souris quand il la bouge. » : attendu curseurdisparu, l'IA dit souris
- « T’as remarqué ? Mon PC est congelé, comme si il était en train de se réchauffer, et j’ai vu dans Windows Update qu’il y a une mise à jour en cours… ça fait un moment que je suis coincé, je ne peux même pas fermer le navigateur. » : attendu cpu100, l'IA dit antimalware
- « Merde, mon PC fait un bruit de claque comme un vieux moteur, il chauffe et je n’arrive plus à travailler. » : attendu ventilobruyant, l'IA dit cliquetis
- « C'est vraiment un vrai vrombit, comme si quelqu'un faisait claquer des serrures au fond du bureau, et ça fait tellement de bruit que je me demande si mon PC va exploser à tout moment ! » : attendu ventilobruyant, l'IA dit cliquetis
- « Mon fils a un pc qui grince et vibre tout le temps, même quand il est en pause, il dit que c’est ça qui le gêne. » : attendu vibration, l'IA dit ventilobruyant
- « Du coup, je me demande si c’est normal, mais j’ai l’impression que chaque fois que je bouge la souris, ça fait un petit ding-dong, comme un appareil qui se réveille tout seul. » : attendu bruitsusb, l'IA dit bourdonnementson
- « Du coup, le PC ne reçoit plus le signal, je vois pas de lumière sur l’Ethernet, c’est bizarre parce que le câble est bien branché et j’ai fait ça plusieurs fois. » : attendu ledreseau, l'IA dit cablereseau
- « Il y a un problème absolument horreur avec la vitesse de mon internet : les vidéos de Netflix prennent 30 secondes à démarrer, et parfois elles s’arrêtent brusquement, comme si le tout était en panne, même si personne d’autre n’utilise la connexion. » : attendu internetlent, l'IA dit videosaccade
- « Mon fils a un problème avec son ordinateur : quand il veut regarder une série en streaming, ça ralentit et il me dit que la connexion est trop lente » : attendu ethernet100, l'IA dit internetlent
- « Mon clavier fait des bêtises, je tape des lettres qui ne sont pas celles que je veux, comme si les touches étaient mélangées, et ça me rend complètement stressé, j’ai même eu un freeze avec le portable ! » : attendu touches, l'IA dit toucherepete
- « Mon PC est devenu un vrai cauchemar ! Il dit que tout ce que je télécharge est un virus, même un logiciel légitime, et hier mon jeu a disparu de l'installateur, le système a même planté pendant 10 minutes ! Est-ce un faux positif ou y a-t-il vraiment un problème ? » : attendu antivirusbloque, l'IA dit fauxnettoyage
- « Y a-t-il un disque D: disponible sur ce PC avec de la place libre ? » : attendu cpleindvide, l'IA dit disquedisparu
- « Y a-t-il un disque D: disponible sur ce PC pour déplacer mes fichiers ? » : attendu cpleindvide, l'IA dit disquedisparu
- « Bonjour, j’ai oublié de sauvegarder une fiche de projet de mon fils, il a supprimé le dossier par accident, il ne voit plus où il l’a mis, il a besoin de le récupérer » : attendu fichiersupprime, l'IA dit fichierintrouvable
- « Lorsque je lance mon PC, l'écran s'ouvre avec l'écran de connexion Windows, mais je ne peux pas saisir le code PIN ou le mot de passe, même après plusieurs tentatives, le système ne réagit pas. » : attendu mdpoublie, l'IA dit clavierconnexion
- « Mon portable est disparu depuis hier, je crois qu’il a été volé dans le métro, j’ai vu une personne s’en aller avec un autre appareil en main, j’ai un compte Microsoft lié et je voulais le localiser. » : attendu perdu, l'IA dit comptepirate
- « Mon fils a dit qu’au réveil, le bureau de son PC est vide, il ne voit plus ses icônes ou ses fichiers, c’est bizarre et il est un peu stressé. » : attendu iconesbureau, l'IA dit profiltemp
- « Panne des raccourcis sur le bureau : le bureau est tout blanc, est-ce que c’est lié à OneDrive ? » : attendu iconesbureau, l'IA dit iconesblanches
- « C’est insupportable ! Mon écran se vide comme ça, je clique sur les paramètres et tout disparaît après 3 secondes, j’ai l’impression que mon PC est en mode freeze, et j’ai peur qu’il s’arrête définitivement » : attendu parametres, l'IA dit videeteint
- « J’ai un problème de fluide quand je joue : le jeu s’arrête soudain, puis reprend, comme un freeze, et l’image s’interrompt. J’ai déjà vérifié que le portable est bien branché au chargeur, et ça marche, mais ça ne règle pas le problème. » : attendu saccades, l'IA dit jeu
