# Mindcare : à savoir avant de toucher au code

## Le catalogue est PARTAGÉ avec Mindcare Mobile
`Mindcare\fichiers\enquetes.json` et `Mindcare\fichiers\images\` sont la **seule source** du catalogue de pannes, pour
Mindcare PC **et** pour Mindcare Mobile (Android, `D:\Projets\Mindcare-Mobile`, dépôt JordanJacquot/Mindcare-Mobile).

- Une panne, un suspect, un test ou une image s'ajoutent **ici seulement**, jamais dans le dépôt du téléphone.
- Le téléphone lit exactement le même fichier : tout changement doit marcher sur les deux.
  - Ajouter des pannes, suspects, tests, mots ou images avec les champs existants : sans risque.
  - **Nouveau champ ou nouveau sens d'un champ** (comme `Commande` ou `Pour` sur une étape) : il faut aussi l'apprendre à
    Mindcare Mobile, et monter `Version` en tête de `enquetes.json` si un ancien téléphone le lirait mal. Le dire à
    Jordan, et donner le texte à passer à la conversation de Mindcare Mobile si elle travaille en même temps.
- Les images restent dans le sous-ensemble de SVG lu par `modules\images.ps1` (le téléphone a le même lecteur).
- Avant de publier : `outils\tester.ps1` tout OK, puis le test d'équivalence de Mindcare Mobile s'il existe
  (`D:\Projets\Mindcare-Mobile`, voir son README) : le PC et le téléphone doivent donner les mêmes enquêtes.
- La publication de Mindcare PC (`outils\publier.ps1`) publie aussi le catalogue pour le téléphone (une fois l'étape 6
  de Mindcare Mobile faite) : le téléphone le télécharge tout seul.

## Le reste
- Le suivi de la suite : `C:\Users\death\.claude\projects\C--Users-death\memory\portail-suite.md`.
- Tester : `outils\tester.ps1` (100 % OK avant chaque publication) ; publier : `outils\publier.ps1 -Version x.y.z -Notes "..."`.
- Les règles de la suite (français simple, aucun tiret long, .ps1 en UTF-8 avec BOM, thèmes perso jamais sur GitHub)
  sont dans `C:\Users\death\.claude\CLAUDE.md`.
