"""Faconne le maitre d'icone de BUREAU a partir du dessin iOS.

⚠️ Une icone iOS est un CARRE PLEINE PAGE : c'est le systeme qui lui applique son masque arrondi
a l'affichage. macOS ne fait pas ca. Reprise telle quelle, l'icone s'afficherait carree, sans
marge et a angles vifs, au milieu d'un Dock d'icones arrondies. Il faut donc appliquer nous-memes
la forme que le dessin iOS recevait gratuitement.

Grille Apple pour macOS (depuis Big Sur), sur un canevas de 1024 :
  - le carre arrondi occupe 824 x 824, centre, donc 100 de marge de chaque cote ;
  - son rayon de coin vaut 185,4.

La marge n'est pas du vide perdu : c'est elle qui aligne optiquement l'icone avec les autres du
Dock, qui suivent toutes la meme grille.

Le meme maitre sert a Windows et Linux. Ni l'un ni l'autre n'impose de forme, et une icone
arrondie avec un peu d'air y parait moderne la ou un carre pleine page fait date.

Idempotent : relancable sans effet de bord, il reecrit le meme fichier.
"""

from pathlib import Path

from PIL import Image, ImageDraw

RACINE = Path(__file__).resolve().parent.parent
SOURCE = RACINE.parent / "dictum-app" / "assets" / "icon.png"
CIBLE = RACINE / "src-tauri" / "icons" / "maitre-bureau.png"

CANEVAS = 1024
COTE = 824
RAYON = 185.4
# Le masque est dessine quatre fois plus grand puis reduit : c'est ce qui donne un bord lisse.
# Dessine a la taille finale, un coin arrondi sort crenele.
ECHELLE = 4


def masque_arrondi(cote: int, rayon: float, echelle: int) -> Image.Image:
    grand = Image.new("L", (cote * echelle, cote * echelle), 0)
    ImageDraw.Draw(grand).rounded_rectangle(
        (0, 0, cote * echelle - 1, cote * echelle - 1),
        radius=rayon * echelle,
        fill=255,
    )
    return grand.resize((cote, cote), Image.LANCZOS)


def main() -> None:
    if not SOURCE.exists():
        raise SystemExit(f"Dessin source introuvable : {SOURCE}")

    dessin = Image.open(SOURCE).convert("RGBA").resize((COTE, COTE), Image.LANCZOS)

    # Le masque REMPLACE le canal alpha au lieu de s'y multiplier : le dessin iOS est
    # entierement opaque, donc les deux reviennent au meme ici, mais remplacer garde le
    # resultat juste si un jour la source porte deja de la transparence.
    dessin.putalpha(masque_arrondi(COTE, RAYON, ECHELLE))

    canevas = Image.new("RGBA", (CANEVAS, CANEVAS), (0, 0, 0, 0))
    marge = (CANEVAS - COTE) // 2
    canevas.paste(dessin, (marge, marge), dessin)

    CIBLE.parent.mkdir(parents=True, exist_ok=True)
    canevas.save(CIBLE)

    # Controle sur le PRODUIT et pas sur l'intention : les coins doivent etre transparents et le
    # centre opaque, sans quoi le masque n'a pas ete applique.
    px = canevas.load()
    coins = [px[0, 0][3], px[CANEVAS - 1, 0][3], px[0, CANEVAS - 1][3], px[CANEVAS - 1, CANEVAS - 1][3]]
    centre = px[CANEVAS // 2, CANEVAS // 2][3]
    if any(a != 0 for a in coins) or centre != 255:
        raise SystemExit(f"Masque non applique : coins={coins} centre={centre}")

    print(f"{CIBLE.relative_to(RACINE)} : {CANEVAS}x{CANEVAS}, coins transparents, centre opaque")


if __name__ == "__main__":
    main()
