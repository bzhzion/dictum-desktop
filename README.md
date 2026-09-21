# Dictum desktop

Dictee vocale locale et multiplateforme. Reecriture complete de `painteau/Dictum`.

La conception, l'inventaire des fonctionnalites qui fait contrat, les mesures faites sur
l'ancienne version et le plan en 13 etapes vivent dans **`docs/dictum-desktop.md` du depot
admin**, qui fait autorite. Ce README ne decrit que l'etat courant du code.

## Etat

**Etape 2 sur 13 : l'icone de la zone de notification.** Le binaire ne dicte rien encore. Il vit dans la **zone de notification** et ouvre au clic une
fenetre **sans barre de titre**, qui se deplace par son bandeau. Fermer la fenetre la **masque**,
seul « Quitter » du menu de l'icone arrete l'application.

Interface en **Tauri** (frontend web, coeur Rust), comme BeamMeUp et hae-app. Direction visuelle
dans `docs/direction-visuelle.md`, posee avant toute fonctionnalite.

## Langue : interface en francais, ligne de commande en anglais

Decide le 2026-09-17, et la frontiere est nette :

| Surface | Langue | Exemples |
|---|---|---|
| Interface graphique | **francais** | fenetre, menu de l'icone (« Ouvrir Dictum », « Demarrer avec le systeme »), futurs reglages |
| Ligne de commande | **anglais** | noms de drapeaux, sortie de `--help`, messages sur la sortie d'erreur |

Le motif est que **l'anglais sera la langue par defaut du produit**, et que la ligne de commande
est la premiere surface qu'un public non francophone rencontre. L'interface, elle, peut rester en
francais tant qu'elle n'est pas traduite.

⛔ **Renommer un drapeau apres publication serait une rupture, et une rupture silencieuse.**
Celui du demarrage automatique est inscrit par le systeme dans la cle `Run` : un ancien nom encore
present y ferait sortir Dictum en **erreur a chaque ouverture de session, sans que personne ne
voie le message**, et le demarrage automatique paraitrait simplement ne plus marcher. C'est
pourquoi `--au-demarrage` a ete renomme `--autostart` **maintenant**, tant que rien n'est
distribue. Apres la premiere version publiee, tout renommage devra garder l'ancien nom comme
alias accepte.

Le test `les_drapeaux_sont_en_anglais` echoue si un drapeau francais ou accentue reapparait, et
verifie aussi que les anciens noms ne sont plus reconnus.

⚠️ **Les identifiants du code restent en francais** (`barre.rs`, `Demande`, `basculer_demarrage`),
comme les commentaires. Ce n'est pas un oubli : c'est une question distincte, qui se tranchera a
part si elle se pose.

## Pourquoi la verification vient avant les fonctionnalites

L'ancienne version a livre **sept versions mineures** avec **7258 lignes de Rust, zero test**, et
**aucun workflow qui compile le projet**. Elle est inutilisable, et c'est la raison de cette
reecriture.

Donc ici la chaine existe avant la premiere fonctionnalite : c'est le seul moment ou les
compteurs sont a zero, et ou brancher `clippy` en erreurs ne coute rien.

⚠️ **Une CI verte ne prouve rien tant qu'on ne l'a pas vue rouge.** Celle-ci a ete eprouvee sur
ses trois modes de defaillance avant d'etre gardee : une assertion fausse, un avertissement
`clippy`, un ecart de formatage. Les trois sont bien attrapes.

## Verifier en local

```
npm ci
npm run build                 # typage TypeScript + frontend, AVANT cargo
cd src-tauri
cargo fmt --all --check
cargo clippy --all-targets    # avec RUSTFLAGS=-D warnings
cargo test
```

⛔ **Pour lancer l'application, `npm run tauri build` et JAMAIS `cargo build` seul.** Sans la
fonctionnalite `custom-protocol` que seul `tauri build` active, le binaire va chercher l'interface
sur localhost au lieu des fichiers embarques : la fenetre s'ouvre sur un
« localhost refused to connect », la ligne de commande marche, et rien dans la compilation ne le
signale. Piege paye trois fois dans le parc, dont une ici. Un garde-fou refuse desormais de
demarrer dans ce cas.

## Structure

```
index.html          coquille de la fenetre, bandeau compris
src/
  main.ts           pilotage de la fenetre (reduire, masquer) et lecture de l'etat
  styles/           socle visuel et fontes EMBARQUEES
src-tauri/
  src/main.rs       point d'entree, ligne de commande et interface
  src/platform/     ce que la plateforme courante permet reellement
  icons/            jeu d'icones, faconne par scripts/preparer-icone.py
scripts/
  preparer-icone.py faconne le maitre de BUREAU depuis le dessin iOS
```

⚠️ **L'icone iOS ne se reprend pas telle quelle.** C'est un carre pleine page, que le systeme
arrondit lui-meme sur iPhone. macOS ne fait pas ca : reprise en l'etat, elle s'afficherait carree
et sans marge au milieu d'un Dock d'icones arrondies. `scripts/preparer-icone.py` lui applique la
grille Apple (824 sur 1024, rayon 185,4).

⚠️ **Le module `platform` n'est pas une abstraction speculative.** Il existe parce que
l'injection de texte n'a pas UNE implementation par systeme mais **deux rien que sur Linux** :
`libei` sur GNOME et KDE, `zwp_virtual_keyboard_v1` sur Hyprland et les compositeurs wlroots, qui
ne servent pas le portail `RemoteDesktop`. Decouvrir ce point apres avoir ecrit une injection
Windows en dur couterait une reecriture.

## Convention

Branche `main`, `CHANGELOG.md` au format Keep a Changelog, hooks du parc dans `.githooks/`.

Apres tout clone : `git config core.hooksPath .githooks`, que git ne transporte jamais.
