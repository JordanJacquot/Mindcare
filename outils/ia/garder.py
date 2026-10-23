# Garde les phrases d'une nuit précédente encore justes pour le catalogue actuel : la panne existe toujours, elle n'a
# pas changé (pas dans la liste de changees.py), et chaque réponse réglée d'avance existe toujours.
# Usage : python outils/ia/garder.py <anciens exemples.jsonl> <enquetes.json> <pannes changées.txt> <sortie.jsonl>
import json
import sys

anciens, catalogue_f, changees_f, sortie = sys.argv[1:5]
d = json.load(open(catalogue_f, encoding="utf-8-sig"))
changees = set(open(changees_f, encoding="utf-8").read().split())
pannes = {p["Id"]: p for p in d["Pannes"]}


def reponses(p):
    n = {}
    for e in p["Etapes"]:
        o = dict(d["Tests"][e["Ref"]]) if e.get("Ref") else {}
        if e.get("Ref"):
            o["Id"] = e["Ref"]
        o.update({k: v for k, v in e.items() if k != "Ref"})
        if o.get("Type") == "question":
            n[o["Id"]] = len(o.get("Reponses") or [])
    return n


gardes = jetes = 0
with open(sortie, "w", encoding="utf-8", newline="\n") as f:
    for ligne in open(anciens, encoding="utf-8"):
        ex = json.loads(ligne)
        p = pannes.get(ex["panne"])
        ok = p is not None and ex["panne"] not in changees
        if ok:
            cible = json.loads(ex["messages"][-1]["content"])
            n = reponses(p)
            ok = cible.get("panne") == ex["panne"] and all(k in n and 0 <= v < n[k] for k, v in cible.get("reponses", {}).items())
        if ok:
            f.write(ligne if ligne.endswith("\n") else ligne + "\n")
            gardes += 1
        else:
            jetes += 1
print(f"{gardes} phrases gardées, {jetes} écartées")
