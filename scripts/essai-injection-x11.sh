#!/usr/bin/env bash
# Eprouve l'injection au curseur sous X11, sans ecran et sans personne devant.
#
# ⛔ **Le seul test qui prouve quelque chose sur cette fonction est celui qui RELIT le texte.**
# `enigo` rend un succes des que le serveur X accepte les evenements, exactement comme `SendInput`
# sous Windows : un test qui se contenterait de son code de retour serait vert sans que rien ne
# soit arrive nulle part. C'est deja arrive trois fois sous Windows le 2026-09-17.
#
# Le montage : un serveur X virtuel, un `xterm` qui ecrit ce qu'il recoit dans un fichier, puis
# une comparaison entre ce qu'on a injecte et ce qui est arrive.
#
# Usage : scripts/essai-injection-x11.sh [repertoire-du-projet]
set -euo pipefail

PROJET="${1:-$HOME/dictum}"
ECRAN=":99"
TEMOIN=$(mktemp /tmp/dictum-temoin-XXXX.txt)
ATTENDU="Dictum : e a c u oe << >> fin."

nettoyer() {
  [[ -n "${PID_XTERM:-}" ]] && kill "$PID_XTERM" 2>/dev/null || true
  [[ -n "${PID_X:-}" ]] && kill "$PID_X" 2>/dev/null || true
  rm -f "$TEMOIN"
}
trap nettoyer EXIT

echo "== serveur X virtuel sur $ECRAN =="
Xvfb "$ECRAN" -screen 0 1280x800x24 >/dev/null 2>&1 &
PID_X=$!
# ⚠️ On ATTEND que le serveur reponde au lieu de dormir un temps arbitraire : une temporisation
# fixe est soit trop courte sur une machine chargee, soit du temps perdu sur une machine rapide.
for _ in $(seq 1 40); do
  DISPLAY="$ECRAN" xdotool getdisplaygeometry >/dev/null 2>&1 && break
  sleep 0.25
done
DISPLAY="$ECRAN" xdotool getdisplaygeometry >/dev/null 2>&1 || { echo "ECHEC : Xvfb n'a pas demarre"; exit 1; }
echo "   demarre"

echo "== fenetre cible : un xterm qui note tout ce qu'il recoit =="
DISPLAY="$ECRAN" xterm -e "cat > $TEMOIN" >/dev/null 2>&1 &
PID_XTERM=$!
for _ in $(seq 1 40); do
  DISPLAY="$ECRAN" xdotool search --class xterm >/dev/null 2>&1 && break
  sleep 0.25
done
FENETRE=$(DISPLAY="$ECRAN" xdotool search --class xterm | head -1 || true)
[[ -n "$FENETRE" ]] || { echo "ECHEC : aucun xterm"; exit 1; }
DISPLAY="$ECRAN" xdotool windowactivate --sync "$FENETRE" 2>/dev/null || true
DISPLAY="$ECRAN" xdotool windowfocus "$FENETRE" 2>/dev/null || true
echo "   fenetre $FENETRE au premier plan"

echo "== injection par Dictum =="
cd "$PROJET/src-tauri"
DISPLAY="$ECRAN" cargo test --quiet injection_reelle -- --ignored --nocapture 2>&1 | tail -5

# `cat` n'ecrit qu'a la fin d'une ligne : on envoie une entree pour vider son tampon.
DISPLAY="$ECRAN" xdotool key --window "$FENETRE" Return 2>/dev/null || true
sleep 1

echo "== ce qui est REELLEMENT arrive dans la fenetre =="
if [[ -s "$TEMOIN" ]]; then
  echo "   recu : $(cat "$TEMOIN")"
  echo "RESULTAT : l'injection atteint bien une autre application"
else
  echo "   recu : (rien)"
  echo "RESULTAT : ECHEC, rien n'est arrive dans la fenetre cible"
  exit 1
fi
