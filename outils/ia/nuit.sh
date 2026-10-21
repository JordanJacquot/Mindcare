#!/usr/bin/env bash
# Un tour d'entraînement complet de l'IA de Mindcare, à lancer la nuit (le PC est très occupé pendant 3 à 4 heures) :
#   1. les situations (taches.py) et les phrases d'entraînement écrites par le Qwen3 4B, relues par son juge (phrases.py)
#   2. un jeu de mesure à part : d'autres situations, d'autres styles, jamais vu à l'entraînement
#   3. l'entraînement du Qwen3 0.6B (entrainer.py), puis la conversion en GGUF (pour llama.cpp, PC et téléphone)
#   4. la mesure (mesurer.py) : l'IA comparée à la recherche par mots actuelle ; le rapport va dans outils/ia/mesures/
# Usage (Git Bash) : bash outils/ia/nuit.sh <nom du tour> [situations par panne] [époques]
set -u
cd "$(dirname "$0")/../.."
NOM="$1"; PAR="${2:-60}"; EPOQUES="${3:-3}"
E="$LOCALAPPDATA/Mindcare-entrainement"
D="$E/donnees/$NOM"
mkdir -p "$D" outils/ia/mesures
export PYTHONIOENCODING=utf-8
PY="$LOCALAPPDATA/Evermind-entrainement/.venv/Scripts/python.exe"          # (l'environnement Python d'Evermind, déjà prêt)
BASE="$LOCALAPPDATA/Evermind-entrainement/modeles/Qwen3-0.6B"
QWEN4B="$LOCALAPPDATA/Evermind-entrainement/modeles/Qwen3-4B-Instruct-2507-Q4_K_M.gguf"
SERVEUR="$E/llama/llama-server.exe"
C=$(find /d/Projets/Evermind-Mobile/app/.cxx -name convert_hf_to_gguf.py | grep x86_64 | head -1)
CAT="Mindcare/fichiers/enquetes.json"
w() { cygpath -w "$1"; }
echo "[$(date +%H:%M)] tour $NOM : $PAR situations par panne, $EPOQUES époques"

[ -s "$D/taches.jsonl" ] || python outils/ia/taches.py "$CAT" "$(w "$D/taches.jsonl")" "$PAR" 1
[ -s "$D/taches-mesure.jsonl" ] || python outils/ia/taches.py "$CAT" "$(w "$D/taches-mesure.jsonl")" 4 99

echo "[$(date +%H:%M)] phrases d'entraînement"
[ -s "$D/exemples.jsonl" ] || python outils/ia/phrases.py "$(w "$D/taches.jsonl")" "$(w "$D/exemples.jsonl")" "$(w "$SERVEUR")" "$(w "$QWEN4B")" 4 8 1000 entrainement 2>&1 | grep -E "000/|Fini|rreur" | tail -40
[ -s "$D/exemples.jsonl" ] || { echo "aucune phrase écrite"; exit 1; }
echo "[$(date +%H:%M)] phrases de mesure"
[ -s "$D/mesure.jsonl" ] || python outils/ia/phrases.py "$(w "$D/taches-mesure.jsonl")" "$(w "$D/mesure.jsonl")" "$(w "$SERVEUR")" "$(w "$QWEN4B")" 3 8 50000 mesure 2>&1 | grep -E "Fini|rreur" | tail -5

echo "[$(date +%H:%M)] entraînement"
if [ ! -f "$E/modeles/mindcare-0.6b-$NOM/config.json" ]; then
    "$PY" outils/ia/entrainer.py "$(w "$D/exemples.jsonl")" "$(w "$BASE")" "$(w "$E/modeles/mindcare-0.6b-$NOM")" "$EPOQUES" 64 2>&1 \
        | grep -vE "it/s\]|Warning|warn|use_cache" | grep -E "exemples|pas [0-9]+0{3}/|validation|Validation|Enregistré|rror" | tail -30
fi
GGUF="$E/modeles/mindcare-0.6b-$NOM-q8_0.gguf"
[ -f "$GGUF" ] || "$PY" "$C" "$(w "$E/modeles/mindcare-0.6b-$NOM")" --outtype q8_0 --outfile "$(w "$GGUF")" 2>&1 | tail -1
[ -f "$GGUF" ] || { echo "pas de modèle converti"; exit 1; }

echo "[$(date +%H:%M)] mesure"
python outils/ia/mesurer.py "$(w "$D/mesure.jsonl")" "$(w "$GGUF")" "$(w "$SERVEUR")" "$(w "$CAT")" "$(w "outils/ia/mesures/$NOM.md")" 2>&1 | tail -14
echo "[$(date +%H:%M)] tour $NOM fini"
