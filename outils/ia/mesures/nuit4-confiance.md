# Confiance de l'assistant (2026-10-25 00:59)

Modèle : `mindcare-0.6b-nuit4-q8_0.gguf`

## a-la-main.jsonl (160 phrases)

| Seuil | Lance seul | Erreurs quand il lance seul | Quand il hésite : bonne panne dans ses 2 idées | Total juste (seul, ou après la question) |
|---|---|---|---|---|
| 0.50 | 96.9 % | 15 (9.7 %) | 1/5 | 88.1 % |
| 0.60 | 93.8 % | 12 (8.0 %) | 3/10 | 88.1 % |
| 0.70 | 87.5 % | 6 (4.3 %) | 8/20 | 88.8 % |
| 0.80 | 84.4 % | 4 (3.0 %) | 12/25 | 89.4 % |
| 0.85 | 82.5 % | 3 (2.3 %) | 14/28 | 89.4 % |
| 0.90 | 80.6 % | 2 (1.6 %) | 16/31 | 89.4 % |
| 0.95 | 76.2 % | 1 (0.8 %) | 23/38 | 90.0 % |
| 0.98 | 70.0 % | 0 (0.0 %) | 33/48 | 90.6 % |

Erreurs avec une confiance d'au moins 0,9 :

- « il s'éteint quand je débranche le chargeur » : attendu eteintbatterie, dit secteurseul (0.97)
- « accès refusé quand je veux supprimer un dossier » : attendu accesrefuse, dit suppressionimpossible (0.90)

## mesure.jsonl (2737 phrases)

| Seuil | Lance seul | Erreurs quand il lance seul | Quand il hésite : bonne panne dans ses 2 idées | Total juste (seul, ou après la question) |
|---|---|---|---|---|
| 0.50 | 99.9 % | 20 (0.7 %) | 3/4 | 99.2 % |
| 0.60 | 99.6 % | 15 (0.6 %) | 8/12 | 99.3 % |
| 0.70 | 99.1 % | 12 (0.4 %) | 18/25 | 99.3 % |
| 0.80 | 98.8 % | 10 (0.4 %) | 26/34 | 99.3 % |
| 0.85 | 98.4 % | 9 (0.3 %) | 35/43 | 99.4 % |
| 0.90 | 98.0 % | 7 (0.3 %) | 44/54 | 99.4 % |
| 0.95 | 97.0 % | 3 (0.1 %) | 70/82 | 99.5 % |
| 0.98 | 95.4 % | 1 (0.0 %) | 112/126 | 99.5 % |

Erreurs avec une confiance d'au moins 0,9 :

- « Du coup, je me demande si c’est normal, mais j’ai l’impression que chaque fois que je bouge la souris, ça fait un petit ding-dong, comme un appareil qui se réveille tout seul. » : attendu bruitsusb, dit bourdonnementson (0.98)
- « Du coup, le PC ne reçoit plus le signal, je vois pas de lumière sur l’Ethernet, c’est bizarre parce que le câble est bien branché et j’ai fait ça plusieurs fois. » : attendu ledreseau, dit cablereseau (0.92)
- « Mon fils a un problème avec son ordinateur : quand il veut regarder une série en streaming, ça ralentit et il me dit que la connexion est trop lente » : attendu ethernet100, dit internetlent (0.92)
- « Mon clavier fait des bêtises, je tape des lettres qui ne sont pas celles que je veux, comme si les touches étaient mélangées, et ça me rend complètement stressé, j’ai même eu un freeze avec le portable ! » : attendu touches, dit toucherepete (0.90)
- « Y a-t-il un disque D: disponible sur ce PC avec de la place libre ? » : attendu cpleindvide, dit disquedisparu (0.93)
- « Y a-t-il un disque D: disponible sur ce PC pour déplacer mes fichiers ? » : attendu cpleindvide, dit disquedisparu (0.99)
- « Lorsque je lance mon PC, l'écran s'ouvre avec l'écran de connexion Windows, mais je ne peux pas saisir le code PIN ou le mot de passe, même après plusieurs tentatives, le système ne réagit pas. » : attendu mdpoublie, dit clavierconnexion (0.97)

