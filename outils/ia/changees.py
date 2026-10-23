# Les pannes nouvelles ou modifiées depuis un ancien catalogue : seules celles-là ont besoin de phrases neuves, les
# autres gardent les phrases des nuits précédentes (garder.py). Une panne est « modifiée » si son titre, sa phrase, sa
# famille, ses suspects ou ses questions (ce que l'IA apprend à régler d'avance) ont changé.
# Usage : python outils/ia/changees.py <ancien enquetes.json> <nouveau enquetes.json> <sortie.txt>
import json
import sys


def resume(d):
    def etapes(p):
        res = []
        for e in p["Etapes"]:
            o = dict(d["Tests"][e["Ref"]]) if e.get("Ref") else {}
            if e.get("Ref"):
                o["Id"] = e["Ref"]
            o.update({k: v for k, v in e.items() if k != "Ref"})
            res.append(o)
        return res

    out = {}
    for p in d["Pannes"]:
        qs = [(e.get("Id"), e.get("Texte"), [r.get("Texte") for r in e.get("Reponses") or []])
              for e in etapes(p) if e.get("Type") == "question"]
        out[p["Id"]] = (p["Titre"], p["Phrase"], p["Famille"], sorted(p.get("Suspects", [])), qs)
    return out


ancien = resume(json.load(open(sys.argv[1], encoding="utf-8-sig")))
nouveau = resume(json.load(open(sys.argv[2], encoding="utf-8-sig")))
ids = [i for i, v in nouveau.items() if ancien.get(i) != v]
open(sys.argv[3], "w", encoding="utf-8", newline="\n").write("\n".join(ids) + "\n")
print(f"{sum(1 for i in ids if i not in ancien)} pannes nouvelles, {sum(1 for i in ids if i in ancien)} modifiées, "
      f"{len(nouveau) - len(ids)} inchangées")
