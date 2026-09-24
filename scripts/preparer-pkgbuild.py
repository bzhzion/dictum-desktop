"""Prepare le repertoire de construction du paquet Arch a partir du gabarit versionne.

`packaging/pacman/oyant-bin/PKGBUILD` porte `_pkgver=0.0.0`. Ce script en depose une copie dans
le repertoire de construction avec la version du tag, a cote du binaire et de l'icone que
`makepkg` trouvera sous leur nom exact.

⚠️ **Les URL de `source` ne sont jamais telechargees** : ce depot est prive, un `makepkg` n'a
aucune authentification. Les fichiers sont poses a la main, et c'est la raison pour laquelle les
`sha256sums` valent `SKIP`. Les URL restent dans le gabarit pour une future soumission AUR.

⛔ **Le remplacement echoue bruyamment si son point d'ancrage a disparu.** Une substitution de
chaine ne peut rien modifier qui n'existe pas deja : si quelqu'un renomme `_pkgver` dans le
gabarit, le script doit s'arreter la, et surtout pas produire un PKGBUILD qui se construirait
sans erreur en annoncant la mauvaise version.

Usage :
  python scripts/preparer-pkgbuild.py --version 0.1.0 --binaire diffusion/oyant-linux-x64 \
      --destination construction-arch
"""

import argparse
import re
import shutil
import sys
from pathlib import Path

RACINE = Path(__file__).resolve().parent.parent
GABARIT = RACINE / "packaging" / "pacman" / "oyant-bin" / "PKGBUILD"
ICONE = RACINE / "src-tauri" / "icons" / "128x128@2x.png"
ANCRE = re.compile(r"^_pkgver=.*$", re.MULTILINE)


def main() -> int:
    analyseur = argparse.ArgumentParser(description=__doc__)
    analyseur.add_argument("--version", required=True, help="version sans le v initial")
    analyseur.add_argument("--binaire", required=True, type=Path)
    analyseur.add_argument("--destination", required=True, type=Path)
    options = analyseur.parse_args()

    for chemin in (GABARIT, ICONE, options.binaire):
        if not chemin.is_file():
            print(f"erreur : {chemin} est introuvable", file=sys.stderr)
            return 1

    if options.version.startswith("v"):
        print("erreur : --version s'attend a 0.1.0 et pas a v0.1.0", file=sys.stderr)
        return 1

    texte = GABARIT.read_text(encoding="utf-8")
    remplace, nombre = ANCRE.subn(f"_pkgver={options.version}", texte)
    if nombre != 1:
        print(
            f"erreur : {nombre} ligne '_pkgver=' dans le gabarit, il en faut exactement une",
            file=sys.stderr,
        )
        return 1

    options.destination.mkdir(parents=True, exist_ok=True)
    # ⛔ `newline=""` n'est pas cosmetique : sans lui, Python traduit `\n` en `\r\n` sur Windows,
    # et `makepkg` refuse net un PKGBUILD en CRLF (« contains CRLF characters and cannot be
    # sourced »). Le defaut ne se voit sur aucune machine Linux, donc pas en CI non plus.
    (options.destination / "PKGBUILD").write_text(
        remplace.replace("\r\n", "\n"), encoding="utf-8", newline=""
    )
    shutil.copyfile(options.binaire, options.destination / "oyant")
    shutil.copyfile(ICONE, options.destination / "oyant.png")

    print(f"PKGBUILD prepare en {options.version} dans {options.destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
