# Entraîne le Qwen3 0.6B pour Mindcare sur les phrases de phrases.py (même méthode que l'assistant d'Evermind Mobile :
# LoRA, seule la réponse JSON est apprise), puis l'enregistre fusionné, prêt à être converti en GGUF.
# Usage : python outils/ia/entrainer.py <exemples.jsonl> <modèle de base> <dossier de sortie> [époques] [rang LoRA]
import json
import math
import os
import random
import sys
import time

import torch
from peft import LoraConfig, get_peft_model
from transformers import AutoModelForCausalLM, AutoTokenizer

exemples_f, base, sortie = sys.argv[1], sys.argv[2], sys.argv[3]
epoques = float(sys.argv[4]) if len(sys.argv) > 4 else 3
rang = int(sys.argv[5]) if len(sys.argv) > 5 else 64
LONGUEUR_MAX = 512
JETONS_PAR_LOT = int(os.environ.get("MINDCARE_JETONS", "16384"))
torch.manual_seed(0)
random.seed(0)

tok = AutoTokenizer.from_pretrained(base)
modele = AutoModelForCausalLM.from_pretrained(base, dtype=torch.bfloat16, device_map="cuda")
modele.gradient_checkpointing_enable()
modele.enable_input_require_grads()
modele = get_peft_model(modele, LoraConfig(r=rang, lora_alpha=rang * 2, lora_dropout=0.05, target_modules="all-linear", task_type="CAUSAL_LM"))
modele.print_trainable_parameters()


def invite(messages):
    return tok.apply_chat_template(messages[:2], tokenize=False, add_generation_prompt=True, enable_thinking=False)


donnees = []
for l in open(exemples_f, encoding="utf-8"):
    d = json.loads(l)
    p = tok(invite(d["messages"]), add_special_tokens=False)["input_ids"]
    r = tok(d["messages"][2]["content"] + "<|im_end|>\n", add_special_tokens=False)["input_ids"]
    if len(p) + len(r) > LONGUEUR_MAX:
        continue
    donnees.append((p, r, d))
random.shuffle(donnees)
nv = min(max(100, len(donnees) // 50), max(1, len(donnees) // 5))
valid, entr = donnees[:nv], donnees[nv:]
print(f"{len(entr)} exemples d'entraînement, {len(valid)} de validation, {sum(len(p) + len(r) for p, r, _ in entr) / len(entr):.0f} jetons en moyenne", flush=True)


def lots(ex, melanger):
    ex = sorted(ex, key=lambda e: len(e[0]) + len(e[1]))
    res, cur, lmax = [], [], 0
    for e in ex:
        n = len(e[0]) + len(e[1])
        if cur and max(lmax, n) * (len(cur) + 1) > JETONS_PAR_LOT:
            res.append(cur)
            cur, lmax = [], 0
        cur.append(e)
        lmax = max(lmax, n)
    if cur:
        res.append(cur)
    if melanger:
        random.shuffle(res)
    return res


def tenseurs(lot):
    n = max(len(p) + len(r) for p, r, _ in lot)
    ids = torch.full((len(lot), n), tok.pad_token_id)
    lab = torch.full((len(lot), n), -100)
    att = torch.zeros((len(lot), n), dtype=torch.long)
    for i, (p, r, _) in enumerate(lot):
        s = p + r
        ids[i, :len(s)] = torch.tensor(s)
        lab[i, len(p):len(s)] = torch.tensor(r)
        att[i, :len(s)] = 1
    return ids.cuda(), lab.cuda(), att.cuda()


def calculer_perte(ids, lab, att):
    interne = modele.get_base_model()
    h = interne.model(input_ids=ids, attention_mask=att).last_hidden_state
    garde = lab[:, 1:] != -100
    logits = interne.lm_head(h[:, :-1][garde]).float()
    return torch.nn.functional.cross_entropy(logits, lab[:, 1:][garde])


def perte_validation():
    modele.eval()
    tot = n = 0
    with torch.no_grad():
        for lot in lots(valid, False):
            ids, lab, att = tenseurs(lot)
            tot += calculer_perte(ids, lab, att).item() * len(lot)
            n += len(lot)
    modele.train()
    return tot / n


pas_par_epoque = len(lots(entr, False))
total = int(pas_par_epoque * epoques)
opt = torch.optim.AdamW([p for p in modele.parameters() if p.requires_grad], lr=2e-4, weight_decay=0.0)
chauffe = min(100, total // 20)
planif = torch.optim.lr_scheduler.LambdaLR(opt, lambda s: (s + 1) / chauffe if s < chauffe else 0.5 * (1 + math.cos(math.pi * (s - chauffe) / max(1, total - chauffe))))
print(f"{total} pas ({pas_par_epoque} par époque)", flush=True)

modele.train()
pas = 0
debut = time.time()
moy = None
while pas < total:
    for lot in lots(entr, True):
        if pas >= total:
            break
        ids, lab, att = tenseurs(lot)
        perte = calculer_perte(ids, lab, att)
        perte.backward()
        torch.nn.utils.clip_grad_norm_(modele.parameters(), 1.0)
        opt.step()
        planif.step()
        opt.zero_grad(set_to_none=True)
        pas += 1
        moy = perte.item() if moy is None else 0.98 * moy + 0.02 * perte.item()
        if pas % 50 == 0:
            print(f"pas {pas}/{total}  perte {moy:.4f}  {time.time() - debut:.0f} s", flush=True)
        if pas % 500 == 0 or pas == total:
            print(f"  validation : {perte_validation():.4f}", flush=True)

# La panne exacte sur des exemples de validation (sans la grammaire qui sera utilisée dans Mindcare)
modele.eval()
justes = 0
essais = valid[:200]
with torch.no_grad():
    for p, r, d in essais:
        out = modele.generate(input_ids=torch.tensor([p]).cuda(), max_new_tokens=80, do_sample=False)
        dit = tok.decode(out[0][len(p):], skip_special_tokens=True).strip()
        try:
            juste = json.loads(dit)["panne"] == json.loads(d["messages"][2]["content"])["panne"]
        except Exception:
            juste = False
        justes += juste
print(f"Validation : {justes}/{len(essais)} pannes exactes", flush=True)

modele = modele.merge_and_unload()
modele.save_pretrained(sortie)
tok.save_pretrained(sortie)
print("Enregistré dans", sortie, flush=True)
