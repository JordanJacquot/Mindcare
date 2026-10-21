# Fait écrire par le Qwen3 4B (llama-server) les phrases d'utilisateur pour chaque situation de taches.py, puis les fait
# relire par un « juge » (le même 4B) : une phrase n'est gardée que si elle décrit bien CETTE panne, et les faits donnés.
# Même principe que l'entraînement de l'assistant d'Evermind Mobile (outils/entrainer/phrases.py de son dépôt).
#
# Usage : python outils/ia/phrases.py <taches.jsonl> <sortie.jsonl> <llama-server.exe> <modèle 4B .gguf>
#         [phrases par situation] [requêtes en parallèle] [graine] [styles : entrainement | mesure]
import concurrent.futures
import json
import random
import re
import subprocess
import sys
import time
import urllib.request

taches_f, sortie, serveur, modele = sys.argv[1:5]
par = int(sys.argv[5]) if len(sys.argv) > 5 else 4
paralleles = int(sys.argv[6]) if len(sys.argv) > 6 else 8
graine0 = int(sys.argv[7]) if len(sys.argv) > 7 else 1000
jeu = sys.argv[8] if len(sys.argv) > 8 else "entrainement"
port = 8099

CONSIGNE = """Tu joues une personne française qui a un souci avec son ordinateur (PC fixe ou portable sous Windows) et qui le décrit à Mindcare, une appli de dépannage, en écrivant dans une barre de recherche ou en parlant.

Le souci (ne le recopie pas, raconte-le avec tes mots) : {titre}. {phrase}
Famille : {famille}. Mots que les gens emploient parfois : {mots}.
{faits}
Écris {n} phrases différentes que cette personne pourrait écrire ou dire, une par style, dans cet ordre :
{styles}
Règles : toujours en français (quelques mots anglais du quotidien comme « freeze » ou « lag » sont permis, jamais une phrase en anglais) ; la personne n'est pas informaticienne ; elle décrit ce qu'elle voit, entend ou vit, pas la cause. N'emploie pas les mots exacts du titre quand tu peux faire autrement. Chaque phrase doit permettre de reconnaître ce souci-là sans hésiter{et_faits}. Ne dis rien qui contredise le souci ou les faits.
Réponds seulement en JSON : {{"phrases": ["...", "..."]}}"""

STYLES = {
    "entrainement": [
        "très court, comme tapé dans une barre de recherche (3 à 7 mots)", "une personne âgée qui ne connaît pas les mots techniques",
        "un ado qui joue aux jeux vidéo, en argot", "avec des fautes d'orthographe et sans accents", "une dictée vocale, sans aucune ponctuation",
        "elle raconte ce qu'elle faisait quand c'est arrivé", "pressée, en style télégraphique", "une question (« pourquoi mon pc... ? »)",
        "elle décrit seulement ce qu'elle voit ou entend", "elle commence par « bonjour » ou « au secours »", "avec un détail inutile en plus",
        "avec des mots anglais du quotidien (« ça freeze », « ça lag », « le pc crash »)", "agacée, ça fait plusieurs jours que ça dure",
        "elle parle pour quelqu'un d'autre (« l'ordi de ma mère »)",
    ],
    "mesure": [
        "un message court à un ami pour demander de l'aide", "une phrase dite au téléphone à un réparateur", "un titre de question sur un forum d'entraide",
        "une personne inquiète qui exagère un peu", "un parent qui décrit le problème de l'ordinateur de son enfant",
        "un SMS plein d'abréviations", "une description très précise et calme", "une phrase avec une hésitation (« euh », « genre », « du coup »)",
    ],
}[jeu]

JUGE = """Un souci d'ordinateur : {titre}. {phrase}
{faits}
Voici des phrases écrites par des personnes. Pour chacune, réponds vrai seulement si un réparateur comprendrait sans hésiter que la personne décrit CE souci{et_faits_juge}. Faux si la phrase est trop vague, décrit plutôt un autre souci, contredit les faits, n'est pas en français, ou n'est pas une description de problème.
{liste}
Réponds seulement en JSON : {{"ok": [true, false, ...]}} (une valeur par phrase, dans l'ordre)."""

schema = {"type": "object", "properties": {"phrases": {"type": "array", "items": {"type": "string"}}}, "required": ["phrases"]}
schema_juge = {"type": "object", "properties": {"ok": {"type": "array", "items": {"type": "boolean"}}}, "required": ["ok"]}


def ia(contenu, sch, graine, temperature):
    corps = {"messages": [{"role": "user", "content": contenu}], "temperature": temperature, "top_p": 0.95, "seed": graine, "max_tokens": 700,
             "response_format": {"type": "json_schema", "json_schema": {"name": "r", "schema": sch}}}
    req = urllib.request.Request(f"http://127.0.0.1:{port}/v1/chat/completions", json.dumps(corps).encode("utf-8"), {"Content-Type": "application/json"})
    return json.loads(json.load(urllib.request.urlopen(req, timeout=600))["choices"][0]["message"]["content"])


def textes(t):
    faits = ""
    if t["faits"]:
        faits = "Faits que la phrase doit dire ou laisser clairement comprendre :\n" + "\n".join("- " + x for x in t["faits"]) + "\n"
    return faits


def demander(t, graine):
    styles = random.Random(graine).sample(STYLES, min(par, len(STYLES)))
    faits = textes(t)
    phrases = ia(CONSIGNE.format(titre=t["titre"], phrase=t["phrase"], famille=t["famille"], mots=", ".join(t["mots"]) or "aucun",
                                 faits=faits, n=len(styles), styles="\n".join(f"{i + 1}. {s}" for i, s in enumerate(styles)),
                                 et_faits=", et d'en déduire les faits donnés" if t["faits"] else ""), schema, graine, 1.0)["phrases"][:len(styles)]
    phrases = [" ".join(ph.split()) for ph in phrases if ph and len(ph.split()) <= 60]
    if not phrases:
        return []
    liste = "\n".join(f"{i + 1}. {ph}" for i, ph in enumerate(phrases))
    ok = ia(JUGE.format(titre=t["titre"], phrase=t["phrase"], faits=faits, liste=liste,
                        et_faits_juge=" et donne (ou laisse clairement comprendre) les faits listés" if t["faits"] else ""), schema_juge, graine, 0.0)["ok"]
    return [ph for ph, v in zip(phrases, ok) if v]


def dictee(ph):
    ph = re.sub(r"[,;:!?.]", "", ph)
    return ph.lower() if random.random() < 0.5 else ph


taches = [json.loads(l) for l in open(taches_f, encoding="utf-8")]
p = subprocess.Popen([serveur, "-m", modele, "--host", "127.0.0.1", "--port", str(port), "-ngl", "99", "-c", str(3072 * paralleles),
                      "-np", str(paralleles), "--no-webui"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    for _ in range(480):
        try:
            if urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=2).status == 200:
                break
        except Exception:
            time.sleep(0.5)
    random.seed(graine0)
    gardees = refusees = 0
    debut = time.time()
    with open(sortie, "w", encoding="utf-8", newline="\n") as f, concurrent.futures.ThreadPoolExecutor(paralleles) as ex:
        futurs = {ex.submit(demander, t, graine0 + i): t for i, t in enumerate(taches)}
        for k, fu in enumerate(concurrent.futures.as_completed(futurs)):
            t = futurs[fu]
            try:
                phrases = fu.result()
            except Exception as e:
                print("erreur", e, file=sys.stderr, flush=True)
                continue
            refusees += par - len(phrases)
            for ph in phrases:
                if jeu == "entrainement" and random.random() < 0.1:
                    ph = dictee(ph)
                f.write(json.dumps({"panne": t["panne"], "messages": [
                    {"role": "system", "content": t["systeme"]}, {"role": "user", "content": ph}, {"role": "assistant", "content": t["cible"]},
                ]}, ensure_ascii=False) + "\n")
                gardees += 1
            if k % 200 == 0:
                print(f"{k}/{len(taches)} situations, {gardees} phrases gardées, {refusees} refusées, {time.time() - debut:.0f} s", flush=True)
    print(f"Fini : {gardees} phrases gardées, {refusees} refusées, {time.time() - debut:.0f} s", flush=True)
finally:
    p.terminate()
