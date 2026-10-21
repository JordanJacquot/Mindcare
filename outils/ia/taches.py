# Prépare les « scénarios » d'entraînement de l'IA de Mindcare à partir du catalogue (enquetes.json) : pour chaque panne,
# des situations au hasard (la panne seule, ou la panne avec une ou deux réponses déjà connues). Le Qwen3 4B écrira
# ensuite des phrases d'utilisateur pour chaque situation (phrases.py), et le 0.6B apprendra à retrouver la panne et ces
# réponses à partir de la phrase.
#
# Ce que l'IA doit répondre (la « cible ») : {"panne": "<id>", "reponses": {"<id de l'étape>": <numéro de la réponse>}}.
# Elle ne choisit que dans le catalogue : elle n'invente jamais une panne ni une manipulation.
#
# Usage : python outils/ia/taches.py <enquetes.json> <sortie.jsonl> [situations par panne] [graine]
import json
import random
import sys

catalogue_f, sortie = sys.argv[1], sys.argv[2]
par_panne = int(sys.argv[3]) if len(sys.argv) > 3 else 30
graine = int(sys.argv[4]) if len(sys.argv) > 4 else 1

SYSTEME = ("Tu es Mindcare. Une personne décrit le problème de son ordinateur. Réponds seulement en JSON : "
           "la panne du catalogue qui correspond, et les réponses qu'elle donne déjà.")

d = json.load(open(catalogue_f, encoding="utf-8-sig"))
familles = {f["Id"]: f["Titre"] for f in d["Familles"]}


def etapes(panne):
    # (comme Get-Etapes : un test partagé « Ref », puis les champs propres à l'étape par-dessus)
    res = []
    for e in panne["Etapes"]:
        o = {}
        if e.get("Ref"):
            o.update(d["Tests"][e["Ref"]])
            o["Id"] = e["Ref"]
        o.update({k: v for k, v in e.items() if k != "Ref"})
        res.append(o)
    return res


def questions(panne):
    # Les questions qu'une phrase peut déjà régler (pas la question « sur ce PC ? », ni les tests à faire)
    q = []
    for e in etapes(panne):
        if e.get("Type") != "question":
            continue
        reps = e.get("Reponses") or []
        if any(r.get("Note") == "cepc" for r in reps) or len(reps) < 2 or e.get("Id") == "danger":   # (la sécurité est toujours demandée)
            continue
        q.append(e)
    return q


hasard = random.Random(graine)
n = 0
with open(sortie, "w", encoding="utf-8", newline="\n") as f:
    for p in d["Pannes"]:
        qs = questions(p)
        for k in range(par_panne):
            m = hasard.choices([0, 1, 2], weights=[4, 4, 2])[0] if qs else 0
            choisies = hasard.sample(qs, min(m, len(qs)))
            faits, reponses = [], {}
            for e in choisies:
                i = hasard.randrange(len(e["Reponses"]))
                reponses[e["Id"]] = i
                faits.append(f"« {e['Texte']} » : {e['Reponses'][i]['Texte']}")
            cible = json.dumps({"panne": p["Id"], "reponses": reponses}, ensure_ascii=False, separators=(",", ":"))
            f.write(json.dumps({
                "panne": p["Id"], "famille": familles.get(p["Famille"], ""), "titre": p["Titre"], "phrase": p["Phrase"],
                "mots": p.get("Mots", [])[:8], "faits": faits, "systeme": SYSTEME, "cible": cible,
            }, ensure_ascii=False) + "\n")
            n += 1
print(f"{n} situations pour {len(d['Pannes'])} pannes")
