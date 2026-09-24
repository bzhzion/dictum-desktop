#!/usr/bin/env bash
# Eprouve le raccourci global sous X11, sans micro, sans modele et sans personne devant.
#
# ⛔ **L'astuce qui rend ce test possible : on n'a besoin ni de microphone ni de modele.** Le
# raccourci declenche `commencer`, qui verifie d'abord que le moteur et le modele sont la. Sur une
# machine ou ils ne le sont pas, la dictee s'arrete immediatement **en le disant sur la sortie
# d'erreur**. Cette ligne est la preuve que la touche est bien arrivee jusqu'au coeur.
#
# Sans cette astuce, prouver le raccourci demanderait une carte son, un modele de plusieurs
# centaines de megaoctets et quelqu'un pour parler.
#
# Usage : scripts/essai-raccourci-x11.sh [repertoire-du-projet]
set -euo pipefail

PROJET="${1:-$HOME/oyant}"
ECRAN=":99"
JOURNAL=$(mktemp /tmp/oyant-journal-XXXX.txt)

nettoyer() {
  [[ -n "${PID_APP:-}" ]] && kill "$PID_APP" 2>/dev/null || true
  [[ -n "${PID_X:-}" ]] && kill "$PID_X" 2>/dev/null || true
}
trap nettoyer EXIT

echo "== serveur X virtuel sur $ECRAN =="
Xvfb "$ECRAN" -screen 0 1280x800x24 >/dev/null 2>&1 &
PID_X=$!
for _ in $(seq 1 40); do
  DISPLAY="$ECRAN" xdotool getdisplaygeometry >/dev/null 2>&1 && break
  sleep 0.25
done
DISPLAY="$ECRAN" xdotool getdisplaygeometry >/dev/null 2>&1 || { echo "ECHEC : Xvfb n'a pas demarre"; exit 1; }

echo "== lancement d’Oyant =="
BINAIRE="$PROJET/src-tauri/target/debug/oyant"
[[ -x "$BINAIRE" ]] || { echo "ECHEC : $BINAIRE introuvable, compiler d'abord"; exit 1; }
DISPLAY="$ECRAN" "$BINAIRE" >"$JOURNAL" 2>&1 &
PID_APP=$!

# ⚠️ On attend que le PROCESSUS soit la ET qu'il ait eu le temps d'enregistrer son raccourci.
# Une temporisation fixe trop courte testerait une application qui n'ecoute pas encore, et le
# test conclurait a tort que le raccourci ne marche pas.
for _ in $(seq 1 40); do
  kill -0 "$PID_APP" 2>/dev/null || { echo "ECHEC : l'application s'est arretee"; echo "--- journal ---"; cat "$JOURNAL"; exit 1; }
  grep -qiE 'raccourci global|panic' "$JOURNAL" && break
  sleep 0.5
done
sleep 2

if grep -qi 'raccourci global' "$JOURNAL"; then
  echo "ATTENTION : l'enregistrement du raccourci a echoue"
  grep -i 'raccourci global' "$JOURNAL"
fi
echo "   lancee, journal dans $JOURNAL"

echo "== on TIENT Ctrl+Alt+Espace, puis on relache =="
DISPLAY="$ECRAN" xdotool keydown ctrl+alt+space
sleep 1
DISPLAY="$ECRAN" xdotool keyup ctrl+alt+space
sleep 3

echo "== ce qu’Oyant a fait =="
if grep -q 'dictée' "$JOURNAL"; then
  grep 'dictée' "$JOURNAL" | head -3
  echo "RESULTAT : le raccourci global atteint le coeur sous X11"
else
  echo "--- journal complet ---"
  cat "$JOURNAL"
  echo "RESULTAT : ECHEC, la touche n'a declenche aucune reaction"
  exit 1
fi
