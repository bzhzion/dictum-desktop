"""Pose la version du tag dans `tauri.conf.json`, juste avant la construction.

Convention du parc : **la version vit dans les tags git**, jamais commitee dans un manifeste. Le
binaire la derive deja tout seul (`src-tauri/build.rs`, par `git describe`), mais **Tauri ne lit
pas `build.rs`** : le nom et la version du `.deb`, du `.pkg` et de l'installateur NSIS viennent de
`tauri.conf.json`, et de lui seul.

⛔ **Sans cette etape, la panne est entierement silencieuse.** Le binaire s'annonce `0.1.0` et
passe le controle de version, pendant que le `.deb` publie s'appelle `Oyant_0.0.0_amd64.deb` :
`apt` enregistre `0.0.0`, et **aucune mise a jour ne sera jamais proposee**, quel que soit le
nombre de tags poses ensuite. Rien n'echoue, ni a la construction, ni a la publication.

Hors tag le fichier n'est pas touche : un `workflow_dispatch` doit produire exactement ce que
produit une construction locale.

Usage :
  python scripts/poser-version.py --version 0.1.0
  python scripts/poser-version.py --verifier      dit ce que le manifeste annonce
"""

import argparse
import json
import re
import sys
from pathlib import Path

RACINE = Path(__file__).resolve().parent.parent
MANIFESTE = RACINE / "src-tauri" / "tauri.conf.json"
# La ligne est remplacee telle quelle plutot que par une reecriture du JSON : re-serialiser
# reformaterait tout le fichier, et le diff d'une construction deviendrait illisible.
ANCRE = re.compile(r'^(\s*)"version":\s*"[^"]*"(,?)$', re.MULTILINE)
SEMVER = re.compile(r"^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")


def main() -> int:
    analyseur = argparse.ArgumentParser(description=__doc__)
    analyseur.add_argument("--version", help="version sans le v initial")
    analyseur.add_argument("--verifier", action="store_true")
    options = analyseur.parse_args()

    texte = MANIFESTE.read_text(encoding="utf-8")

    if options.verifier or not options.version:
        print(json.loads(texte)["version"])
        return 0

    if options.version.startswith("v"):
        print("erreur : --version s'attend a 0.1.0 et pas a v0.1.0", file=sys.stderr)
        return 1
    # ⚠️ Tauri refuse une version non semver, et l'erreur arrive bien plus loin, pendant
    # l'empaquetage : un nom de branche passe ici par megarde couterait toute la compilation.
    if not SEMVER.match(options.version):
        print(f"erreur : '{options.version}' n'est pas une version semver", file=sys.stderr)
        return 1

    remplace, nombre = ANCRE.subn(
        lambda m: f'{m.group(1)}"version": "{options.version}"{m.group(2)}', texte, count=1
    )
    if nombre != 1:
        print("erreur : aucune ligne \"version\" dans le manifeste", file=sys.stderr)
        return 1

    MANIFESTE.write_text(remplace, encoding="utf-8", newline="")
    relu = json.loads(MANIFESTE.read_text(encoding="utf-8"))["version"]
    if relu != options.version:
        print(f"erreur : le manifeste annonce '{relu}' apres ecriture", file=sys.stderr)
        return 1

    print(f"tauri.conf.json pose en {relu}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
