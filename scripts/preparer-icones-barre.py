"""Produit les icones de la ZONE DE NOTIFICATION, une par etat.

La marque de Dictum est un **anneau et un disque separes par un vide**. Elle est ici **redessinee**
plutot que decoupee dans l'icone d'application, pour deux raisons : a 32 pixels une reduction
donne un trait sale, et l'icone d'application porte son fond marine opaque, qui ferait un carre
sombre dans la barre des taches.

⚠️ **`android-icon-monochrome.png` de l'app iOS n'est PAS cette marque.** Le nom le laisse croire
et je m'y suis fie : c'est un **chevron**, une toute autre forme. Le defaut n'est apparu qu'en
REGARDANT l'image agrandie. Ne pas le reprendre ici.

Proportions **mesurees sur l'icone d'application** (balayage de sa ligne mediane, canevas 1024) et
pas estimees a l'oeil : anneau exterieur 278, interieur 235, disque 129. On les conserve telles
quelles pour que le bureau, le mobile et le site montrent la meme marque.

⚠️ **Les teintes sont eclaircies par rapport a la palette de l'interface.** Une barre des taches
est sombre par defaut sur Windows 11 mais peut etre claire, et l'icone doit rester lisible dans
les deux cas. Les valeurs de la palette, pensees pour du texte sur fond marine, y seraient trop
foncees : `preparation` (#7a5218) disparaitrait sur fond sombre.

⚠️ **La couleur ne porte jamais l'etat seule.** Une icone de barre ne peut pas afficher de texte,
donc c'est l'INFOBULLE qui nomme l'etat, et elle est obligatoire.

Idempotent : relancable sans effet de bord.
"""

from pathlib import Path

from PIL import Image, ImageDraw

RACINE = Path(__file__).resolve().parent.parent
DOSSIER = RACINE / "src-tauri" / "icons" / "barre"

# Servi en 32 px : Windows y pioche ce qu'il lui faut, macOS et Linux s'en accommodent. Notre
# propre reduction est plus nette que celle du systeme depuis un maitre plus grand.
COTE = 32
# Dessine huit fois plus grand puis reduit : a 32 px direct, un anneau de deux pixels sort
# crenele.
ECHELLE = 8

# Proportions relevees sur l'icone d'application, ramenees au rayon exterieur.
ANNEAU_INTERIEUR = 235 / 278
DISQUE = 129 / 278
# Le rayon exterieur occupe presque tout le canevas : contrairement a l'icone d'application, il
# n'y a pas de fond a laisser respirer.
RAYON = 15 / 16

ETATS = {
    "repos": (0x9A, 0x9A, 0xA8),
    "ecoute": (0x5B, 0x9B, 0xD1),
    "enregistrement": (0xE0, 0x5A, 0x45),
    "transcription": (0xD9, 0x9A, 0x3A),
}


def marque(cote: int, echelle: int) -> Image.Image:
    """Masque de la marque : anneau plein, vide, disque. Rendu en niveaux de gris."""
    grand = cote * echelle
    masque = Image.new("L", (grand, grand), 0)
    d = ImageDraw.Draw(masque)
    centre = grand / 2
    exterieur = grand / 2 * RAYON

    def cercle(rayon: float, valeur: int) -> None:
        d.ellipse(
            (centre - rayon, centre - rayon, centre + rayon, centre + rayon),
            fill=valeur,
        )

    # Ordre important : on peint l'anneau plein, on le creuse, puis on repose le disque dans le
    # trou. Dessiner le disque avant le creusement l'effacerait.
    cercle(exterieur, 255)
    cercle(exterieur * ANNEAU_INTERIEUR, 0)
    cercle(exterieur * DISQUE, 255)

    return masque.resize((cote, cote), Image.LANCZOS)


def main() -> None:
    masque = marque(COTE, ECHELLE)
    DOSSIER.mkdir(parents=True, exist_ok=True)

    for etat, couleur in ETATS.items():
        image = Image.new("RGBA", (COTE, COTE), couleur + (0,))
        image.putalpha(masque)
        cible = DOSSIER / f"{etat}.png"
        image.save(cible)

        # Verification du PRODUIT et pas de l'intention.
        px = image.load()
        if px[0, 0][3] != 0:
            raise SystemExit(f"{cible.name} : coin opaque, la marque deborde du canevas.")
        if px[COTE // 2, COTE // 2][3] < 200:
            raise SystemExit(f"{cible.name} : centre vide, le disque n'a pas ete pose.")
        # Le vide entre l'anneau et le disque doit rester transparent : c'est ce qui fait la
        # marque. A mi-chemin entre le centre et le bord, on doit etre dans ce vide.
        mi = int(COTE / 2 * RAYON * (ANNEAU_INTERIEUR + DISQUE) / 2)
        if px[COTE // 2 + mi, COTE // 2][3] > 60:
            raise SystemExit(f"{cible.name} : le vide entre anneau et disque est bouche.")

        pleins = sum(1 for p in image.getdata() if p[3] > 200)
        print(f"  {cible.relative_to(RACINE)} : {COTE}x{COTE}, {pleins} pixels pleins")


if __name__ == "__main__":
    main()
