# Mesure l'IA de Mindcare sur des phrases jamais vues (le jeu « mesure » de phrases.py, écrit avec d'autres styles et
# une autre graine que l'entraînement), et la compare à la recherche par mots actuelle de Mindcare (Find-Pannes).
# L'IA tourne comme elle tournera dans Mindcare : llama-server, avec une grammaire qui l'oblige à répondre une panne du
# catalogue.
# Usage : python outils/ia/mesurer.py <mesure.jsonl> <modèle .gguf> <llama-server.exe> <enquetes.json> <rapport.md>
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.request
from collections import Counter

mesure_f, modele, serveur, catalogue_f, rapport = sys.argv[1:6]
port = 8097
ici = os.path.dirname(os.path.abspath(__file__))
cat = json.load(open(catalogue_f, encoding="utf-8-sig"))
ids = [p["Id"] for p in cat["Pannes"]]
titres = {p["Id"]: p["Titre"] for p in cat["Pannes"]}
ex = [json.loads(l) for l in open(mesure_f, encoding="utf-8")]
schema = {"type": "object", "properties": {"panne": {"type": "string", "enum": ids}, "reponses": {"type": "object", "additionalProperties": {"type": "integer"}}},
          "required": ["panne", "reponses"]}

# 1. La recherche par mots actuelle (le vrai moteur de Mindcare PC)
with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False, encoding="utf-8-sig") as t:
    t.write("\n".join(e["messages"][1]["content"].replace("\n", " ") for e in ex))
sortie_mots = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", os.path.join(ici, "recherche-mots.ps1"), catalogue_f, t.name],
                             capture_output=True, text=True, encoding="utf-8").stdout.splitlines()
mots = [l.split("\t") for l in sortie_mots]

# 2. L'IA
p = subprocess.Popen([serveur, "-m", modele, "--host", "127.0.0.1", "--port", str(port), "-ngl", "99", "-c", "8192", "-np", "4", "--no-webui"],
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env={**os.environ, "LLAMA_ARG_CHAT_TEMPLATE_KWARGS": '{"enable_thinking":false}'})
try:
    pret = False
    for _ in range(480):
        if p.poll() is not None:
            break
        try:
            if urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=2).status == 200:
                pret = True
                break
        except Exception:
            time.sleep(0.5)
    if not pret:
        sys.exit("llama-server n'a pas démarré avec ce modèle")

    def demander(e):
        corps = {"messages": e["messages"][:2], "temperature": 0, "max_tokens": 120,
                 "response_format": {"type": "json_schema", "json_schema": {"name": "r", "schema": schema}}}
        req = urllib.request.Request(f"http://127.0.0.1:{port}/v1/chat/completions", json.dumps(corps).encode("utf-8"), {"Content-Type": "application/json"})
        t0 = time.time()
        r = json.loads(json.load(urllib.request.urlopen(req, timeout=120))["choices"][0]["message"]["content"])
        return r, time.time() - t0

    ia_pannes = ia_reponses = mots1 = mots3 = mixte3 = 0
    erreurs, confusions, durees = [], Counter(), []
    for k, e in enumerate(ex):
        cible = json.loads(e["messages"][2]["content"])
        try:
            r, du = demander(e)
            durees.append(du)
        except Exception as x:
            r = {"panne": "", "reponses": {}}
        ok = r.get("panne") == cible["panne"]
        ia_pannes += ok
        ia_reponses += ok and {k2: int(v) for k2, v in (r.get("reponses") or {}).items()} == cible["reponses"]
        m = mots[k] if k < len(mots) else []
        mots1 += bool(m) and m[0] == cible["panne"]
        mots3 += cible["panne"] in m[:3]
        # (ce que Mindcare montrerait : la panne de l'IA, puis les propositions de la recherche, 3 en tout)
        propositions = [r.get("panne")] + [x for x in m if x and x != r.get("panne")]
        mixte3 += cible["panne"] in propositions[:3]
        if not ok:
            erreurs.append(f"- « {e['messages'][1]['content']} » : attendu {cible['panne']}, l'IA dit {r.get('panne')}")
            confusions[(cible["panne"], r.get("panne"))] += 1
    n = len(ex)
    pc = lambda x: f"{100 * x / n:.1f} %"
    lignes = [f"# Mesure de l'IA de Mindcare ({time.strftime('%Y-%m-%d %H:%M')})", "",
              f"Modèle : `{os.path.basename(modele)}` ; {n} phrases jamais vues ({len(set(e['panne'] for e in ex))} pannes).", "",
              "| | Bonne panne en 1er | Bonne panne dans les 3 premières |", "|---|---|---|",
              f"| L'IA | {pc(ia_pannes)} | |", f"| L'IA, puis la recherche par mots | | {pc(mixte3)} |", f"| La recherche par mots actuelle | {pc(mots1)} | {pc(mots3)} |", "",
              f"L'IA trouve aussi exactement les réponses déjà données dans {pc(ia_reponses)} des phrases.",
              f"Temps de réponse de l'IA sur le PC : {sorted(durees)[len(durees) // 2]:.2f} s (médiane).", "",
              "## Confusions les plus fréquentes", ""] + [f"- {a} ({titres.get(a, '')}) pris pour {b} ({titres.get(b, '')}) : {c} fois" for (a, b), c in confusions.most_common(15)] + \
             ["", "## Exemples d'erreurs", ""] + erreurs[:60]
    open(rapport, "w", encoding="utf-8").write("\n".join(lignes) + "\n")
    print("\n".join(lignes[:12]))
finally:
    p.terminate()
