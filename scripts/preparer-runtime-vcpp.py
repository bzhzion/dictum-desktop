"""Rassemble le runtime Visual C++ que l'installateur doit embarquer.

⛔ Le probleme qu'on resout ici n'est visible sur AUCUNE machine de developpement. `dictum.exe`
importe `vcruntime140.dll`, et tout moteur whisper.cpp importe en plus `msvcp140.dll` et
`vcomp140.dll`. **Windows ne fournit aucune des quatre**, et **aucune archive amont de whisper.cpp
ne les embarque**. Sur un Windows neuf sans redistribuable Visual C++, l'application ne demarre
pas, avec une boite de dialogue qui nomme une DLL et rien d'autre.

Pourquoi a cote de `dictum.exe` plutot qu'installe dans le systeme : l'installateur de Dictum
s'execute **sans elevation** (`installMode: currentUser`), donc il ne peut pas poser un
redistribuable machine. Le deploiement dit « app-local » ne demande aucun privilege et Windows
cherche dans le repertoire de l'executable avant le systeme.

Pourquoi pas plutot un CRT statique dans nos moteurs : ca reglerait le cas de ceux qu'on compile,
mais **pas celui du moteur CUDA**, qui vient de ggml-org avec la meme dependance. Une reponse au
niveau de l'application les couvre tous.

Les quatre fichiers forment un groupe clos : verifie a la table d'import, ils ne dependent que
d'eux-memes et des API de Windows.

Usage :
  python scripts/preparer-runtime-vcpp.py             rassemble dans src-tauri/runtime/
  python scripts/preparer-runtime-vcpp.py --verifier   controle ce qui est deja en place
"""

import argparse
import hashlib
import os
import shutil
import sys
from pathlib import Path

RACINE = Path(__file__).resolve().parent.parent
CIBLE = RACINE / "src-tauri" / "runtime"

FICHIERS = [
    "msvcp140.dll",       # bibliotheque standard C++, exigee par whisper.cpp
    "vcruntime140.dll",   # exigee par dictum.exe lui-meme
    "vcruntime140_1.dll",  # gestion des exceptions, tiree par les deux precedentes
    "vcomp140.dll",       # OpenMP, exigee par ggml-base et ggml-cpu
]


def sources_possibles():
    """La voie propre d'abord, le repli ensuite.

    Le repertoire `Redist` des Build Tools est la source officielle de redistribution, mais il
    n'existe que si le composant « C++ Redistributable » a ete installe, ce qui n'est pas le cas
    par defaut. Les copies du systeme sont les memes binaires et appartiennent a la meme liste de
    redistribution ; leur seul defaut est d'etre la version installee sur CETTE machine.
    """
    chemins = []
    for edition in ("18", "2022", "2019"):
        racine = Path(
            f"C:/Program Files (x86)/Microsoft Visual Studio/{edition}/BuildTools/VC/Redist/MSVC"
        )
        if racine.is_dir():
            for version in sorted(racine.iterdir(), reverse=True):
                for crt in sorted(version.glob("x64/Microsoft.VC*.CRT")):
                    chemins.append(("redist", crt))
                for openmp in sorted(version.glob("x64/Microsoft.VC*.OpenMP")):
                    chemins.append(("redist", openmp))
    systeme = Path(os.environ.get("SystemRoot", "C:/Windows")) / "System32"
    chemins.append(("systeme", systeme))
    return chemins


def version_de(chemin):
    """Lit la version de ressource du fichier, pour la consigner a cote de l'empreinte."""
    try:
        import ctypes
        import ctypes.wintypes as w

        taille = ctypes.windll.version.GetFileVersionInfoSizeW(str(chemin), None)
        if not taille:
            return "inconnue"
        tampon = ctypes.create_string_buffer(taille)
        ctypes.windll.version.GetFileVersionInfoW(str(chemin), 0, taille, tampon)
        pointeur = ctypes.c_void_p()
        longueur = w.UINT()
        if not ctypes.windll.version.VerQueryValueW(
            tampon, "\\", ctypes.byref(pointeur), ctypes.byref(longueur)
        ):
            return "inconnue"
        octets = ctypes.string_at(pointeur, longueur.value)
        haut_ms = int.from_bytes(octets[10:12], "little")
        bas_ms = int.from_bytes(octets[8:10], "little")
        haut_ls = int.from_bytes(octets[14:16], "little")
        bas_ls = int.from_bytes(octets[12:14], "little")
        return f"{haut_ms}.{bas_ms}.{haut_ls}.{bas_ls}"
    except Exception:
        return "inconnue"


def empreinte(chemin):
    condensat = hashlib.sha256()
    with open(chemin, "rb") as fichier:
        for bloc in iter(lambda: fichier.read(1024 * 1024), b""):
            condensat.update(bloc)
    return condensat.hexdigest()


def rassembler():
    CIBLE.mkdir(parents=True, exist_ok=True)
    sources = sources_possibles()
    lignes = []

    for nom in FICHIERS:
        trouve = None
        for origine, repertoire in sources:
            candidat = repertoire / nom
            if candidat.exists():
                trouve = (origine, candidat)
                break
        if trouve is None:
            sys.exit(
                f"{nom} introuvable. Installer le composant\n"
                "  Microsoft.VisualStudio.Component.VC.Redist.14.Latest\n"
                "des Build Tools, ou verifier que le redistribuable Visual C++ est present."
            )

        origine, source = trouve
        destination = CIBLE / nom
        # Idempotent : on ne recopie que si le contenu differe.
        if not destination.exists() or empreinte(destination) != empreinte(source):
            shutil.copy2(source, destination)
        lignes.append((nom, version_de(destination), destination.stat().st_size, origine))

    marqueur = CIBLE / "origine.txt"
    marqueur.write_text(
        "\n".join(
            f"{nom}  version {version}  {taille} octets  source {origine}"
            for nom, version, taille, origine in lignes
        )
        + "\n",
        encoding="utf-8",
    )
    return lignes


def decrire():
    if not CIBLE.is_dir():
        sys.exit(f"{CIBLE} absent. Lancer sans --verifier pour le remplir.")
    manquants = [nom for nom in FICHIERS if not (CIBLE / nom).exists()]
    if manquants:
        sys.exit("manquant(s) : " + ", ".join(manquants))
    total = 0
    for nom in FICHIERS:
        chemin = CIBLE / nom
        total += chemin.stat().st_size
        print(f"  {nom:22} version {version_de(chemin):16} {chemin.stat().st_size:>9} octets")
    print(f"  {'total':22} {' ' * 25}{total:>9} octets ({total / 1024 / 1024:.1f} Mo)")


def main():
    analyseur = argparse.ArgumentParser(description=__doc__)
    analyseur.add_argument("--verifier", action="store_true", help="controle sans rien copier")
    arguments = analyseur.parse_args()

    if sys.platform != "win32":
        sys.exit("Ce runtime ne concerne que Windows.")

    if arguments.verifier:
        decrire()
        return

    lignes = rassembler()
    origines = {origine for _, _, _, origine in lignes}
    if origines == {"systeme"}:
        print("  source : copies du systeme (composant redist des Build Tools non installe)")
    print()
    decrire()


if __name__ == "__main__":
    main()
