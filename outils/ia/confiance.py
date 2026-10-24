# Quand l'assistant peut-il lancer l'enquête tout seul ? Pour chaque phrase : sa panne et sa confiance (la probabilité
# des jetons du nom de la panne, comme Mindcare la calcule), puis sa deuxième idée (même question, sa première panne
# interdite). On en tire, par seuil de confiance : la part des phrases où il lance l'enquête seul, ses erreurs dans ce
# cas, et, quand il hésite, la part où la bonne panne est dans ses deux idées (la question « Pour être sûr »).
# Usage : python outils/ia/confiance.py <modèle .gguf> <llama-server.exe> <enquetes.json> <rapport.md> <mesure.jsonl>...
import json
import math
import os
import subprocess
import sys
import time
import urllib.request

modele, serveur, catalogue_f, rapport = sys.argv[1:5]
fichiers = sys.argv[5:]
port = 8098
cat = json.load(open(catalogue_f, encoding="utf-8-sig"))
ids = [p["Id"] for p in cat["Pannes"]]


def schema(pannes):
    return {"type": "object", "properties": {"panne": {"type": "string", "enum": pannes}, "reponses": {"type": "object", "additionalProperties": {"type": "integer"}}},
            "required": ["panne", "reponses"]}


def confiance(contenu, jetons):
    # (les jetons qui écrivent le nom de la panne : entre les guillemets qui suivent "panne":)
    debut = contenu.index('"panne"')
    debut = contenu.index('"', contenu.index(':', debut)) + 1
    fin = contenu.index('"', debut)
    pos, lp = 0, 0.0
    for j in jetons:
        t = j["token"]
        if pos < fin and pos + len(t) > debut:
            lp += j["logprob"]
        pos += len(t)
    return math.exp(lp)


p = subprocess.Popen([serveur, "-m", modele, "--host", "127.0.0.1", "--port", str(port), "-ngl", "99", "-c", "8192", "-np", "4", "--no-webui"],
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env={**os.environ, "LLAMA_ARG_CHAT_TEMPLATE_KWARGS": '{"enable_thinking":false}'})
try:
    for _ in range(480):
        try:
            if urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=2).status == 200:
                break
        except Exception:
            time.sleep(0.5)

    def demander(messages, pannes):
        corps = {"messages": messages, "temperature": 0, "max_tokens": 120, "logprobs": True,
                 "response_format": {"type": "json_schema", "json_schema": {"name": "r", "schema": schema(pannes)}}}
        req = urllib.request.Request(f"http://127.0.0.1:{port}/v1/chat/completions", json.dumps(corps).encode("utf-8"), {"Content-Type": "application/json"})
        c = json.load(urllib.request.urlopen(req, timeout=120))["choices"][0]
        contenu = c["message"]["content"]
        return json.loads(contenu)["panne"], confiance(contenu, c["logprobs"]["content"])

    lignes = [f"# Confiance de l'assistant ({time.strftime('%Y-%m-%d %H:%M')})", "", f"Modèle : `{os.path.basename(modele)}`", ""]
    for f in fichiers:
        res = []
        for l in open(f, encoding="utf-8"):
            e = json.loads(l)
            cible = json.loads(e["messages"][2]["content"])["panne"]
            p1, c1 = demander(e["messages"][:2], ids)
            p2, _ = demander(e["messages"][:2], [i for i in ids if i != p1])
            res.append((cible, p1, c1, p2))
        n = len(res)
        lignes += [f"## {os.path.basename(f)} ({n} phrases)", "",
                   "| Seuil | Lance seul | Erreurs quand il lance seul | Quand il hésite : bonne panne dans ses 2 idées | Total juste (seul, ou après la question) |",
                   "|---|---|---|---|---|"]
        for s in (0.5, 0.6, 0.7, 0.8, 0.85, 0.9, 0.95, 0.98):
            seul = [r for r in res if r[2] >= s]
            hes = [r for r in res if r[2] < s]
            err = sum(r[0] != r[1] for r in seul)
            deux = sum(r[0] in (r[1], r[3]) for r in hes)
            juste = sum(r[0] == r[1] for r in seul) + deux
            lignes.append(f"| {s:.2f} | {100 * len(seul) / n:.1f} % | {err} ({100 * err / max(1, len(seul)):.1f} %) | {deux}/{len(hes)} | {100 * juste / n:.1f} % |")
        lignes += ["", "Erreurs avec une confiance d'au moins 0,9 :", ""]
        lignes += [f"- « {json.loads(l)['messages'][1]['content']} » : attendu {r[0]}, dit {r[1]} ({r[2]:.2f})"
                   for l, r in zip(open(f, encoding="utf-8"), res) if r[0] != r[1] and r[2] >= 0.9][:30]
        lignes.append("")
    open(rapport, "w", encoding="utf-8").write("\n".join(lignes) + "\n")
    print("\n".join(lignes))
finally:
    p.terminate()
