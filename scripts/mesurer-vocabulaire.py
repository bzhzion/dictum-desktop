"""Mesure ce que le vocabulaire donne au moteur apporte VRAIMENT, sur un vrai enregistrement.

⛔ **Le meme fichier audio sert aux deux passes.** Comparer deux enregistrements differents ne
mesurerait pas le vocabulaire mais la façon de parler du second : c'est le seul protocole qui
isole la variable.

Trois choses sont mesurees, et la troisieme est la plus importante :

1. **Le gain** : combien de fois chaque terme du vocabulaire est ecrit correctement, sans le
   prompt puis avec.
2. **Le report** : les termes du vocabulaire abimes par le prompt, s'il y en a.
3. ⛔ **Le sur-biais** : des mots PRONONCES mais volontairement ABSENTS du vocabulaire, et
   phonetiquement proches d'un terme present. Si `Levetiracetam` devient `Levothyrox` parce que le
   second est dans la liste, le mecanisme est dangereux sur ce public et il faut le savoir. Une
   mesure qui ne regarde que le gain passerait entierement a cote.

Usage :
  python scripts/mesurer-vocabulaire.py <audio> [--modele CHEMIN] [--moteur CHEMIN]

L'audio peut etre dans n'importe quel format : il est converti en 16 kHz mono par ffmpeg, qui est
ce que le moteur attend.
"""

import argparse
import re
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path

RACINE = Path(__file__).resolve().parent.parent

# ── Le texte a lire a voix haute ───────────────────────────────────────────────────────────────
#
# ⚠️ Il vit ICI, a cote du barometre qui le note, pour que les deux ne divergent jamais. Des
# phrases naturelles et non une liste de mots : on lit une liste autrement qu'on dicte, et c'est
# la dictee qu'on mesure.
TEXTE_A_LIRE = """
1. Monsieur Kowalczyk est venu en consultation ce matin, et je lui ai renouvelé son Lévothyrox.
2. Il habite Villeurbanne, à dix minutes du cabinet.
3. Attention, ne pas confondre avec le Lévétiracétam, qui est un antiépileptique.
4. J'ai prescrit de l'amoxicilline à sa fille pour une angine.
5. Le dossier de madame Kowalczyk sera transmis à Valbonne la semaine prochaine.
6. L'association Breizhzion, à Villeurbanne, édite le logiciel que j'utilise pour dicter.
7. Le renouvellement d'amoxicilline est noté, ainsi que l'arrêt du Lévothyrox.
8. Breizhzion n'a aucun accès à ce que je dicte, et c'est la raison pour laquelle je m'en sers.
"""

# Le vocabulaire qu'on donnera au moteur, et le nombre de fois que chaque terme est prononce.
VOCABULAIRE = {
    "Kowalczyk": 2,
    "Lévothyrox": 2,
    "Villeurbanne": 2,
    "Breizhzion": 2,
    "amoxicilline": 2,
}

# ⛔ Prononces dans le texte et volontairement ABSENTS du vocabulaire, chacun phonetiquement
# proche d'un terme present. Ils doivent rester eux-memes : s'ils se font aspirer vers leur
# voisin, le mecanisme corrige a tort.
PIEGES = {
    "Lévétiracétam": "Lévothyrox",
    "Valbonne": "Villeurbanne",
}


def replier(texte: str) -> str:
    """Minuscules sans accents : on note l'orthographe du terme, pas la casse d'une phrase."""
    sans_accents = "".join(
        c for c in unicodedata.normalize("NFD", texte) if unicodedata.category(c) != "Mn"
    )
    return sans_accents.lower()


def occurrences(texte: str, terme: str) -> int:
    """Compte les occurrences du terme, sur des frontieres de mot, accents et casse ignores."""
    motif = re.compile(rf"\b{re.escape(replier(terme))}\b")
    return len(motif.findall(replier(texte)))


def convertir(source: Path, destination: Path) -> None:
    """16 kHz mono, ce que le moteur attend. ffmpeg accepte a peu pres tout en entree."""
    resultat = subprocess.run(
        ["ffmpeg", "-y", "-i", str(source), "-ar", "16000", "-ac", "1", str(destination)],
        capture_output=True,
        text=True,
    )
    if resultat.returncode != 0:
        derniere = (resultat.stderr or "").strip().splitlines()[-1:] or ["sans details"]
        raise SystemExit(f"ffmpeg a echoue : {derniere[0]}")


def transcrire(moteur: Path, modele: Path, wav: Path, prompt: str | None) -> str:
    commande = [str(moteur), "-m", str(modele), "-f", str(wav), "-l", "fr", "-np", "-nt"]
    if prompt:
        commande += ["--prompt", prompt, "--carry-initial-prompt"]
    resultat = subprocess.run(
        commande, capture_output=True, text=True, encoding="utf-8", errors="replace"
    )
    if resultat.returncode != 0:
        derniere = (resultat.stderr or "").strip().splitlines()[-1:] or ["sans details"]
        raise SystemExit(f"le moteur a echoue : {derniere[0]}")
    return " ".join(
        ligne.strip() for ligne in (resultat.stdout or "").splitlines() if ligne.strip()
    )


def main() -> int:
    analyseur = argparse.ArgumentParser(description=__doc__)
    # ⚠️ `audio` et `--modele` sont facultatifs au niveau d'argparse, et exiges plus bas seulement
    # quand on mesure : `--texte` sert justement a lire le texte AVANT d'avoir enregistre quoi que
    # ce soit, donc le reclamer la rendrait cette option inutilisable.
    analyseur.add_argument(
        "audio", type=Path, nargs="?", help="l'enregistrement, dans n'importe quel format"
    )
    analyseur.add_argument(
        "--moteur", type=Path, default=RACINE / "src-tauri/moteur/whisper-cli.exe"
    )
    analyseur.add_argument("--modele", type=Path, help="le fichier ggml-*.bin a utiliser")
    analyseur.add_argument("--texte", action="store_true", help="affiche le texte a lire et sort")
    options = analyseur.parse_args()

    if options.texte:
        print(TEXTE_A_LIRE.strip())
        print("\nVocabulaire donne au moteur :")
        for terme, fois in VOCABULAIRE.items():
            print(f"  {terme}  (prononce {fois} fois)")
        print("\nPieges, prononces mais volontairement ABSENTS du vocabulaire :")
        for piege, voisin in PIEGES.items():
            print(f"  {piege}  (proche de {voisin})")
        return 0

    if options.audio is None or options.modele is None:
        print("il faut un audio et un --modele pour mesurer", file=sys.stderr)
        return 1
    for chemin in (options.audio, options.moteur, options.modele):
        if not chemin.is_file():
            print(f"introuvable : {chemin}", file=sys.stderr)
            return 1

    prompt = ", ".join(VOCABULAIRE)

    with tempfile.TemporaryDirectory() as temporaire:
        wav = Path(temporaire) / "mesure.wav"
        convertir(options.audio, wav)
        print(f"Modele : {options.modele.name}")
        print(f"Prompt : {prompt}\n")

        sans = transcrire(options.moteur, options.modele, wav, None)
        avec = transcrire(options.moteur, options.modele, wav, prompt)

    print("-- Transcription SANS vocabulaire --")
    print(f"  {sans}\n")
    print("-- Transcription AVEC vocabulaire --")
    print(f"  {avec}\n")

    print("-- Gain sur les termes du vocabulaire --")
    gagnes = perdus = 0
    for terme, attendu in VOCABULAIRE.items():
        a, b = occurrences(sans, terme), occurrences(avec, terme)
        if b > a:
            gagnes += b - a
        elif b < a:
            perdus += a - b
        fleche = "mieux" if b > a else ("PIRE" if b < a else "egal"
                                       if a == attendu else "egal, toujours rate")
        print(f"  {terme:16s} attendu {attendu}  sans {a}  avec {b}  -> {fleche}")

    print("\n-- Sur-biais : ces mots doivent rester eux-memes --")
    sur_biais = 0
    for piege, voisin in PIEGES.items():
        a, b = occurrences(sans, piege), occurrences(avec, piege)
        aspire = occurrences(avec, voisin) > occurrences(sans, voisin) and b < a
        if b < a or aspire:
            sur_biais += 1
        verdict = "ABIME" if b < a else "intact"
        print(f"  {piege:16s} (proche de {voisin})  sans {a}  avec {b}  -> {verdict}")

    print("\n== Verdict ==")
    print(f"  termes gagnes : {gagnes}, termes perdus : {perdus}, sur-biais : {sur_biais}")
    if sur_biais:
        print("  ⛔ Le prompt abime des mots qui n'etaient pas dans la liste : a ne pas garder tel quel.")
    elif gagnes == 0:
        print("  ⚠️ Aucun gain mesure. Le biais ne suffit pas : c'est ce qui justifierait la")
        print("     correction approximative apres coup, et pas avant cette mesure.")
    else:
        print("  ✅ Gain net sans sur-biais.")
    return 0


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    raise SystemExit(main())
