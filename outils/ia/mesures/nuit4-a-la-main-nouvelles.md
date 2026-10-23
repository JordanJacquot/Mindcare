# Mesure de l'IA de Mindcare (2026-10-23 05:50)

Modèle : `mindcare-0.6b-nuit4-q8_0.gguf` ; 41 phrases jamais vues (41 pannes).

| | Bonne panne en 1er | Bonne panne dans les 3 premières |
|---|---|---|
| L'IA | 97.6 % | |
| L'IA, puis la recherche par mots | | 100.0 % |
| La recherche par mots actuelle | 90.2 % | 97.6 % |

L'IA trouve aussi exactement les réponses déjà données dans 68.3 % des phrases.
Temps de réponse de l'IA sur le PC : 0.05 s (médiane).

## Confusions les plus fréquentes

- paumepave (Le texte s'écrit ailleurs pendant que je tape (portable)) pris pour touchewindows (La touche Windows ne marche plus) : 1 fois

## Exemples d'erreurs

- « quand je tape le texte part n'importe ou sur mon portable » : attendu paumepave, l'IA dit touchewindows
