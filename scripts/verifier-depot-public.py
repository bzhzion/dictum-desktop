"""Refuse toute reference a un depot prive dans ce depot, qui est PUBLIC.

⛔ Le motif est concret : au moment de rendre ce depot public, quatre endroits designaient encore
`docs/oyant-desktop.md du depot admin` comme la source d'autorite, et le README renvoyait a
`painteau/Dictum`, devenu prive. Un lecteur tombait donc sur une reference qu'il ne pouvait pas
suivre, et la structure interne de l'organisation fuyait sans aucun benefice.

⚠️ **Retirer ces references ne protege pas de leur retour, seul ce controle le fait.** Elles
reviennent naturellement : elles sont justes du point de vue de quelqu'un qui a les deux depots
ouverts, et c'est precisement le point de vue de celui qui ecrit le commentaire.

Regle a appliquer a la place : **une explication doit se tenir toute seule**. Si un choix technique
merite d'etre justifie, on ecrit le raisonnement ici plutot que de renvoyer ailleurs.

Usage :
  python scripts/verifier-depot-public.py
"""

import re
import subprocess
import sys

# Motifs interdits, avec ce qu'il faut faire a la place. ⚠️ `oyant-web` et `breizhzion.com` sont
# publics : ils ne sont pas dans cette liste, les citer est legitime.
INTERDITS = [
    (r"depot admin", "ecrire le raisonnement sur place plutot que renvoyer a un depot prive"),
    (r"docs/oyant-desktop\.md", "idem : ce fichier vit dans un depot prive"),
    (r"painteau/Dictum\b", "ce depot est prive et archive depuis le 2026-09-21"),
    (r"[A-Z]:[\\/]Git[\\/]admin", "chemin local d'un depot prive"),
    (r"bzhzion/(hae-app|oyant-app|cedule|mome|breme|fourbi|polyptique|maeil|hucheor|ombra)\b",
     "depot prive du parc"),
]

# Ce controle se cite lui-meme : il doit s'exclure, sinon il echoue toujours.
EXCLUS = {"scripts/verifier-depot-public.py"}


def fichiers_suivis() -> list[str]:
    """Les fichiers SUIVIS PAR GIT, donc exactement ce qui est publie."""
    sortie = subprocess.run(
        ["git", "ls-files"], capture_output=True, text=True, check=True, encoding="utf-8"
    )
    return [
        f for f in sortie.stdout.splitlines()
        if f not in EXCLUS
        # Les binaires et les verrous de dependances n'ont pas de prose a auditer.
        and not f.endswith((".woff2", ".png", ".ico", ".icns", ".lock", ".zip"))
        and f != "package-lock.json"
    ]


def main() -> int:
    ecarts = []
    for chemin in fichiers_suivis():
        try:
            texte = open(chemin, encoding="utf-8").read()
        except (UnicodeDecodeError, OSError):
            continue
        for numero, ligne in enumerate(texte.splitlines(), 1):
            for motif, remede in INTERDITS:
                if re.search(motif, ligne, re.IGNORECASE):
                    ecarts.append((chemin, numero, motif, remede, ligne.strip()[:90]))

    if not ecarts:
        print("OK : aucune reference a un depot prive dans les fichiers publies.")
        return 0

    print(f"{len(ecarts)} reference(s) a un depot prive dans un depot PUBLIC :\n")
    for chemin, numero, motif, remede, extrait in ecarts:
        print(f"  {chemin}:{numero}")
        print(f"    motif  : {motif}")
        print(f"    remede : {remede}")
        print(f"    ligne  : {extrait}\n")
    return 1


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    raise SystemExit(main())
