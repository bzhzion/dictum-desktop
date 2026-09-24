//! Derive la version affichee par le binaire, au lieu de la declarer.
//!
//! Convention du parc : **la version vit dans les tags git**, jamais commitee dans le manifeste.
//! `Cargo.toml` reste donc fige a `0.0.0` et c'est la construction qui tranche.
//!
//! ⚠️ Deriver plutot que declarer n'est pas un raffinement. Un numero ecrit a la main a cote du
//! code qu'il decrit finit toujours par mentir : il serait faux des le premier tag, et faux
//! differemment a chaque version oubliee. Ici il ne PEUT pas diverger.
//!
//! Ordre de resolution, du plus explicite au plus general :
//! 1. `OYANT_VERSION` s'il est pose, ce qui laisse la CI forcer la valeur ;
//! 2. `git describe`, qui couvre le developpement local et dit meme si l'arbre est sale ;
//! 3. la version du manifeste, seul cas ou l'on construit hors d'un depot git.

use std::path::Path;
use std::process::Command;

fn main() {
    verifier_les_repertoires_embarques();

    // Genere le contexte Tauri (configuration, icones, permissions). Doit venir en premier :
    // `tauri::generate_context!` echoue a la compilation sans lui.
    tauri_build::build();

    // ⚠️ Sans cela, cargo ne rejoue ce script que si un fichier source change : passer un tag
    // n'y suffirait pas et le binaire garderait l'ancienne version, silencieusement.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");
    println!("cargo:rerun-if-env-changed=OYANT_VERSION");

    let version = version_forcee()
        .or_else(version_git)
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    println!("cargo:rustc-env=OYANT_VERSION={version}");
}

/// Dit quoi lancer quand il manque un des repertoires que l'installateur doit embarquer.
///
/// ⚠️ **Ce controle ne bloque rien que Tauri ne bloquerait deja**, et c'est tout son interet.
/// Ces deux repertoires sont ignores par git et produits par des scripts, donc absents sur une
/// machine fraiche ; `tauri_build` echoue alors sur
/// `glob pattern runtime/* path not found or didn't match any files`, un message qui nomme le
/// symptome et **ni la cause ni le remede**. On passe avant lui pour nommer les deux.
///
/// ⛔ Ne jamais « reparer » ce cas en rendant le glob de `bundle.resources` tolerant : la
/// construction reussirait alors, et l'installateur partirait **sans moteur** ou **sans runtime
/// Visual C++**, defaut qui ne se verrait que chez celui qui installe.
fn verifier_les_repertoires_embarques() {
    // ⛔ **Windows uniquement.** Linux telecharge son moteur au premier usage et macOS n'en a
    // aucun, donc ces deux repertoires n'y existent pas et ne doivent pas y etre exiges. Exiger
    // partout a rendu l'integration continue rouge sur les trois systemes.
    //
    // ⚠️ On lit `CARGO_CFG_TARGET_OS` et non `cfg!(windows)` : un script de construction tourne
    // sur la machine HOTE, donc `cfg!` y decrit l'hote et pas la cible.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let attendus: [(&str, &str); 2] = [
        ("moteur", "python scripts/preparer-moteur-embarque.py"),
        ("runtime", "python scripts/preparer-runtime-vcpp.py"),
    ];

    for (repertoire, remede) in attendus {
        println!("cargo:rerun-if-changed={repertoire}");
        let vide = match std::fs::read_dir(Path::new(repertoire)) {
            Ok(mut entrees) => entrees.next().is_none(),
            Err(_) => true,
        };
        if vide {
            panic!(
                "src-tauri/{repertoire}/ est vide ou absent.\n\
                 Il est ignore par git et produit par un script. Lancer :\n\
                     {remede}"
            );
        }
    }
}

fn version_forcee() -> Option<String> {
    std::env::var("OYANT_VERSION")
        .ok()
        .map(|v| nettoyer(&v))
        .filter(|v| !v.is_empty())
}

/// `git describe` rend par exemple `v0.2.0` sur un tag exact, `v0.2.0-3-gabc1234` trois commits
/// plus loin, et le seul hache court quand aucun tag n'existe encore. Le suffixe `-dirty` signale
/// un arbre modifie, information utile en developpement : on sait qu'on ne teste pas ce qui est
/// commite.
fn version_git() -> Option<String> {
    let sortie = Command::new("git")
        .args([
            "describe", "--tags", "--always", "--dirty", "--match", "v[0-9]*",
        ])
        .output()
        .ok()?;

    if !sortie.status.success() {
        return None;
    }

    let brut = String::from_utf8(sortie.stdout).ok()?;
    let propre = nettoyer(&brut);
    if propre.is_empty() {
        None
    } else {
        Some(propre)
    }
}

/// Retire les espaces et le `v` initial des tags du parc, qui sont de la forme `vX.Y.Z`.
fn nettoyer(brut: &str) -> String {
    brut.trim().trim_start_matches('v').to_string()
}
