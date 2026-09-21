# Dictum

**Dictée vocale dont la reconnaissance tourne sur votre propre ordinateur.** Vous maintenez une
touche, vous parlez, le texte s'écrit là où se trouve votre curseur. Aucun enregistrement n'est
envoyé nulle part, et le logiciel fonctionne sans connexion internet.

Windows et Linux. Gratuit, sans compte. Édité par [BREIZHZION](https://breizhzion.com),
association d'intérêt général.

**[dictum.breizhzion.com](https://dictum.breizhzion.com)** pour la présentation et le
téléchargement.

---

## Installer

**Windows** : un seul fichier, aucun mot de passe administrateur demandé.

> [Dictum-Setup-x64.exe](https://dl.breizhzion.com/dictum-desktop/Dictum-Setup-x64.exe)

**Debian et Ubuntu**, par le dépôt de l'association :

```sh
curl -fsSL https://apt.breizhzion.com/KEY.gpg \
  | sudo gpg --dearmor -o /usr/share/keyrings/breizhzion.gpg
echo "deb [signed-by=/usr/share/keyrings/breizhzion.gpg] https://apt.breizhzion.com stable main" \
  | sudo tee /etc/apt/sources.list.d/breizhzion.list
sudo apt update && sudo apt install dictum
```

**Arch** :

```sh
printf '\n[breizhzion]\nSigLevel = Required DatabaseRequired\nServer = https://pacman.breizhzion.com/$arch\n' \
  | sudo tee -a /etc/pacman.conf
curl -fsSL https://pacman.breizhzion.com/KEY.gpg | sudo pacman-key --add -
sudo pacman-key --lsign-key apt@breizhzion.com
sudo pacman -Syu dictum-bin
```

⚠️ **Sous Linux, la dictée fonctionne sur les sessions X11 et pas encore sur Wayland**, qui est le
choix par défaut des versions récentes d'Ubuntu et de Fedora. **Il n'y a pas de version pour
macOS** : le binaire se compile et est publié, la dictée n'y est pas implémentée.

## Ce que le logiciel fait

- La reconnaissance vocale s'exécute en local, par [whisper.cpp](https://github.com/ggml-org/whisper.cpp).
  Coupez le réseau : la dictée continue de fonctionner, et c'est la façon la plus simple de
  vérifier la promesse.
- Le texte est saisi dans l'application au premier plan, comme au clavier.
- **Repli sur le presse-papiers quand la fenêtre cible refuse la saisie simulée** (champ de mot de
  passe, application élevée), avec un message qui dit quoi faire plutôt qu'une dictée perdue en
  silence.
- Raccourci clavier global, deux bips de début et de fin, icône de zone de notification.
- Mise en forme du français, et substitutions définies par l'utilisateur.
- Français, anglais, ou détection automatique. Pas davantage pour l'instant.
- Historique local dont la taille est réglable, et peut valoir zéro.

Ce que le logiciel **ne** fait pas : pas de transcription en continu, pas de reformulation par un
modèle de langue, pas d'API HTTP locale, pas d'extension d'éditeur. Une première version du projet
en avait ; celle-ci est repartie de zéro et fait volontairement moins.

## Modèles et moteurs

Le modèle de reconnaissance vocale et les moteurs accélérés se téléchargent depuis le logiciel, et
chaque fichier est vérifié par son empreinte SHA-256 avant d'être utilisé.

| Modèle | Poids | Durée mesurée sur processeur |
|---|---|---|
| Small (par défaut) | 488 Mo | environ 7 s pour 30 s de parole |
| Medium | 1,5 Go | environ 18 s |
| Large v3 | 3,1 Go | à réserver à une carte graphique |

**L'installateur Windows embarque le moteur processeur**, donc on peut dicter dès la fin de
l'installation. C'est délibéré : si le catalogue distant est injoignable, on perd la vitesse,
jamais la capacité à dicter. Deux moteurs accélérés sont proposés en plus, Vulkan pour n'importe
quelle carte graphique (21 Mo, mesuré 3,4 fois plus rapide que le processeur) et CUDA pour NVIDIA,
ce dernier n'étant proposé que si un pilote NVIDIA est détecté.

## Où sont les données

Sous l'identifiant `org.breizhzion.dictum.desktop` dans votre profil utilisateur, jamais dans le
répertoire d'installation :

| Système | Emplacement |
|---|---|
| Windows | `%LOCALAPPDATA%\org.breizhzion.dictum.desktop` |
| Linux | `$XDG_DATA_HOME/...`, sinon `~/.local/share/...` |
| macOS | `~/Library/Application Support/...` |

C'est le seul endroit où le texte des dictées existe.

---

## Développer

Écrit en **Rust**, interface en **Tauri** (cœur Rust, frontend web).

```sh
npm ci
npm run build                 # typage TypeScript et frontend, AVANT cargo
cd src-tauri
cargo fmt --all --check
cargo clippy --all-targets    # avec RUSTFLAGS=-D warnings
cargo test
```

⛔ **Pour lancer l'application, `npm run tauri build` et JAMAIS `cargo build` seul.** Sans la
fonctionnalité `custom-protocol` que seul `tauri build` active, le binaire va chercher l'interface
sur localhost au lieu des fichiers embarqués : la fenêtre s'ouvre sur un « localhost refused to
connect », la ligne de commande marche, et rien dans la compilation ne le signale. Un garde-fou
refuse désormais de démarrer dans ce cas.

⚠️ **Sous Windows, `src-tauri/moteur/` et `src-tauri/runtime/` doivent exister avant toute
compilation** : ils sont ignorés par git et produits par `scripts/preparer-moteur-embarque.py` et
`scripts/preparer-runtime-vcpp.py`. `build.rs` échoue en nommant le script à lancer.

### Structure

```
index.html              coquille de la fenetre, bandeau compris
src/                    interface : pilotage de la fenetre, reglages, modeles, texte
src/styles/             socle visuel et polices EMBARQUEES
src-tauri/src/
  main.rs               point d'entree, ligne de commande, interface
  dictee.rs             enchainement complet d'une dictee
  audio.rs              capture et conversion en 16 kHz mono
  moteur.rs             catalogue des moteurs et sonde du materiel
  modeles.rs            catalogue des modeles et verification d'integrite
  injection.rs          saisie du texte dans la fenetre active
  texte.rs              mise en forme du francais, fonctions pures
  historique.rs         journal local des dictees
  platform/             ce que la plateforme courante permet reellement
scripts/                preparation des artefacts et essais manuels
packaging/pacman/       gabarit PKGBUILD
```

### Ce qui n'est pas négociable dans ce dépôt

- **La version vit dans les tags git**, jamais dans un manifeste. `Cargo.toml` et
  `tauri.conf.json` restent à `0.0.0` ; `build.rs` dérive la version du tag, et la chaîne de
  publication pose celle du tag dans `tauri.conf.json` au moment de construire. Un numéro écrit à
  la main à côté du code qu'il décrit finit toujours par mentir.
- ⛔ **Un artefact distribuable ne se compile jamais avec les options par défaut de whisper.cpp** :
  `GGML_NATIVE=ON` lie le binaire au processeur de la machine de construction, et les deux modes de
  panne sont invisibles depuis cette machine (instruction illégale ailleurs, refus de démarrer sans
  le pilote). Les scripts de préparation posent `GGML_NATIVE=OFF` et `GGML_BACKEND_DL=ON`.
- **Un test importe le code qu'il protège**, jamais une copie de sa logique, et se prouve rouge par
  mutation avant d'être gardé.
- **Un réglage affiché doit piloter quelque chose.** Le test `reglages_utilises` refuse un réglage
  qui ne fait rien, et refuse aussi une exclusion périmée.

### Convention

Branche `main`, `CHANGELOG.md` au format Keep a Changelog, hooks dans `.githooks/`.

Après tout clone, git ne transporte jamais la configuration des hooks :

```sh
git config core.hooksPath .githooks
```

## Licence

Code sous **BZ-1.1** (`LICENSE.md`), une licence en accès libre : usage personnel par une personne
physique autorisé, usage commercial et prestation pour un tiers réservés à l'association. Ce n'est
**pas** une licence open source au sens de l'OSI, et nous préférons le dire plutôt que d'emprunter
le mot.

whisper.cpp, ggml, les modèles Whisper d'OpenAI et le runtime Visual C++ redistribués restent sous
leur propre licence, reproduite dans `NOTICE.md`.
