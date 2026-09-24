//! Ou Oyant range ses donnees.
//!
//! ⛔ **Une seule implementation, utilisee par l'interface ET par la ligne de commande.** La
//! ligne de commande n'a pas d'application Tauri sous la main, donc elle ne peut pas appeler
//! `app.path()`. Si chacune calculait son chemin de son cote, `oyant fichier.wav` chercherait
//! les modeles ailleurs que la ou l'interface les a telecharges, et le message dirait « modele
//! absent » sur une machine ou 3 Go de modeles sont parfaitement installes.
//!
//! ⛔ **Profil LOCAL et jamais itinerant sur Windows.** Dans un domaine, le profil itinerant est
//! recopie sur le reseau a chaque ouverture de session : 3 Go de modeles le rendraient
//! inutilisable. Invisible sur une machine personnelle, redhibitoire en entreprise.
//!
//! ⛔ **Et jamais le repertoire d'installation**, que la desinstallation emporte. C'est ce que
//! faisait l'ancienne version, et c'est ce qui a produit la collision du 2026-09-17.

use std::path::{Path, PathBuf};

/// Identifiant de l'application, celui du manifeste Tauri.
///
/// ⚠️ Recopie ici parce que la ligne de commande n'a pas acces au contexte Tauri. Le test
/// `l_identifiant_est_celui_du_manifeste` compare les deux a la compilation, donc les deux ne
/// peuvent pas diverger sans que quelque chose le dise.
const IDENTIFIANT: &str = "org.breizhzion.oyant.desktop";

/// L'identifiant porte jusqu'au renommage du produit, le 2026-09-24.
///
/// ⛔ **Il ne sert qu'a la migration, jamais a ranger quoi que ce soit.** Le produit s'appelait
/// Dictum ; une application homonyme et de meme nature existait deja sur l'App Store, d'ou le
/// changement de nom. Voir [`migrer_ancien_nom`] pour ce qui en depend.
const IDENTIFIANT_PRECEDENT: &str = "org.breizhzion.dictum.desktop";

/// Repertoire de donnees de l'application.
pub fn base() -> Result<PathBuf, String> {
    base_pour(IDENTIFIANT)
}

/// Le repertoire de donnees d'un identifiant DONNE.
///
/// ⚠️ **Parametre plutot que recopie**, parce que la migration a besoin du meme calcul sur
/// l'ancien identifiant : deux implementations de cette logique de plateforme finiraient par
/// diverger, et c'est precisement ce que l'en-tete de ce module interdit.
fn base_pour(identifiant: &str) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let local = std::env::var("LOCALAPPDATA")
            .map_err(|_| "Variable LOCALAPPDATA absente.".to_string())?;
        Ok(PathBuf::from(local).join(identifiant))
    }

    #[cfg(target_os = "macos")]
    {
        let maison = std::env::var("HOME").map_err(|_| "Variable HOME absente.".to_string())?;
        Ok(PathBuf::from(maison)
            .join("Library")
            .join("Application Support")
            .join(identifiant))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Convention XDG : la variable si elle est posee, sinon son defaut.
        if let Ok(donnees) = std::env::var("XDG_DATA_HOME") {
            if !donnees.is_empty() {
                return Ok(PathBuf::from(donnees).join(identifiant));
            }
        }
        let maison = std::env::var("HOME").map_err(|_| "Variable HOME absente.".to_string())?;
        Ok(PathBuf::from(maison)
            .join(".local")
            .join("share")
            .join(identifiant))
    }
}

/// Repertoire de CONFIGURATION, distinct du repertoire de donnees.
///
/// ⚠️ **Ce n'est pas le meme que `base()` sur Windows, et la distinction est voulue.** La
/// configuration va dans le profil ITINERANT : quelques kilo-octets de reglages qui suivent
/// l'utilisateur d'un poste a l'autre, c'est exactement ce que ce profil sert a faire. Les
/// modeles, eux, pesent des giga-octets et restent en LOCAL.
///
/// ⛔ Les mettre au meme endroit obligerait a choisir entre perdre ses reglages en changeant de
/// poste et recopier 3 Go sur le reseau a chaque ouverture de session.
pub fn configuration() -> Result<PathBuf, String> {
    configuration_pour(IDENTIFIANT)
}

/// Le repertoire de configuration d'un identifiant DONNE. Meme motif que [`base_pour`].
fn configuration_pour(identifiant: &str) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let itinerant =
            std::env::var("APPDATA").map_err(|_| "Variable APPDATA absente.".to_string())?;
        Ok(PathBuf::from(itinerant).join(identifiant))
    }

    #[cfg(target_os = "macos")]
    {
        // macOS ne distingue pas les deux, comme Tauri lui-meme.
        base_pour(identifiant)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(config) = std::env::var("XDG_CONFIG_HOME") {
            if !config.is_empty() {
                return Ok(PathBuf::from(config).join(identifiant));
            }
        }
        let maison = std::env::var("HOME").map_err(|_| "Variable HOME absente.".to_string())?;
        Ok(PathBuf::from(maison).join(".config").join(identifiant))
    }
}

/// Reprend les donnees laissees sous l'ancien nom du produit, s'il y en a.
///
/// ⛔ **Sans ca, le renommage orpheline jusqu'a 3,1 Go de modeles sur le disque de quelqu'un**, et
/// le force a les retelecharger. C'est la seule raison d'etre de cette fonction : l'historique et
/// les reglages sont legers, les modeles ne le sont pas.
///
/// ⚠️ **On DEPLACE, on ne copie pas.** Copier doublerait temporairement 3 Go sur un disque qui
/// peut ne pas les avoir. `rename` est par ailleurs atomique ici, l'ancien et le nouveau chemin
/// partageant toujours le meme parent.
///
/// ⚠️ **Le nouveau repertoire gagne toujours.** S'il existe deja, on ne touche a rien et on ne
/// signale pas d'erreur : quelqu'un qui a lance la nouvelle version avant de migrer a produit des
/// reglages neufs, et les ecraser par les anciens serait une perte silencieuse.
///
/// Idempotent : le deuxieme appel ne trouve plus l'ancien repertoire et ne fait rien.
pub fn migrer_ancien_nom() -> Vec<String> {
    let mut faits = Vec::new();
    let couples = [
        (
            base_pour(IDENTIFIANT_PRECEDENT),
            base_pour(IDENTIFIANT),
            "données",
        ),
        (
            configuration_pour(IDENTIFIANT_PRECEDENT),
            configuration_pour(IDENTIFIANT),
            "réglages",
        ),
    ];

    for (ancien, nouveau, quoi) in couples {
        let (Ok(ancien), Ok(nouveau)) = (ancien, nouveau) else {
            continue;
        };
        match migrer_repertoire(&ancien, &nouveau) {
            Ok(true) => faits.push(format!("{quoi} repris depuis {}", ancien.display())),
            Ok(false) => {}
            // ⚠️ On ne fait pas echouer le demarrage pour ca : ne pas migrer laisse quelqu'un
            // avec une installation vide mais fonctionnelle, refuser de demarrer le laisse sans
            // rien du tout.
            Err(message) => faits.push(format!("reprise des {quoi} impossible : {message}")),
        }
    }
    faits
}

/// Deplace `ancien` vers `nouveau`. Rend `true` si quelque chose a bouge.
fn migrer_repertoire(ancien: &Path, nouveau: &Path) -> Result<bool, String> {
    if nouveau.exists() || !ancien.is_dir() {
        return Ok(false);
    }
    if let Some(parent) = nouveau.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|erreur| format!("{} : {erreur}", parent.display()))?;
    }
    std::fs::rename(ancien, nouveau)
        .map_err(|erreur| format!("{} vers {} : {erreur}", ancien.display(), nouveau.display()))?;
    Ok(true)
}

pub fn reglages() -> Result<PathBuf, String> {
    Ok(configuration()?.join("reglages.json"))
}

/// Le moteur EMBARQUE, livre par l'installateur a cote de l'executable.
///
/// ⛔ **Derive de l'executable courant et pas de Tauri**, pour la meme raison que le reste de ce
/// module : la ligne de commande n'a pas d'application Tauri sous la main, et deux calculs
/// separes finiraient par designer deux repertoires differents.
///
/// ⚠️ En developpement (`cargo run`), l'executable est dans `target/`, ou la ressource n'est pas
/// copiee : le moteur embarque n'existe donc que dans une version INSTALLEE. C'est assume, toute
/// verification de ce projet passe par l'installateur.
pub fn moteur_embarque() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|erreur| format!("Exécutable courant introuvable : {erreur}"))?;
    let parent = exe
        .parent()
        .ok_or("Répertoire de l'exécutable introuvable.")?;
    Ok(parent.join("moteur"))
}

pub fn modeles() -> Result<PathBuf, String> {
    Ok(base()?.join("modeles"))
}

pub fn moteurs() -> Result<PathBuf, String> {
    Ok(base()?.join("moteurs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⛔ Le garde-fou de la source unique : l'identifiant recopie ici doit etre celui du
    /// manifeste. S'ils divergeaient, la ligne de commande et l'interface rangeraient leurs
    /// fichiers dans deux repertoires differents, et chacune dirait que les fichiers de l'autre
    /// n'existent pas.
    #[test]
    fn l_identifiant_est_celui_du_manifeste() {
        let manifeste = include_str!("../tauri.conf.json");
        let attendu = format!("\"identifier\": \"{IDENTIFIANT}\"");
        assert!(
            manifeste.contains(&attendu),
            "tauri.conf.json ne porte pas {IDENTIFIANT}"
        );
    }

    #[test]
    fn les_sous_repertoires_descendent_de_la_base() {
        let base = base().unwrap();
        assert!(modeles().unwrap().starts_with(&base));
        assert!(moteurs().unwrap().starts_with(&base));
        assert!(modeles().unwrap() != moteurs().unwrap());
    }

    /// ⛔ Sur Windows, configuration et donnees NE SONT PAS au meme endroit, et les confondre
    /// couterait cher dans les deux sens : des reglages perdus en changeant de poste, ou 3 Go de
    /// modeles recopies sur le reseau a chaque ouverture de session.
    #[cfg(target_os = "windows")]
    #[test]
    fn la_configuration_et_les_donnees_sont_dans_deux_profils_differents() {
        assert_ne!(base().unwrap(), configuration().unwrap());
        assert!(reglages().unwrap().starts_with(configuration().unwrap()));
        assert!(modeles().unwrap().starts_with(base().unwrap()));
    }

    /// ⚠️ Le chemin doit porter l'identifiant : sans lui, Oyant ecrirait a la racine du
    /// repertoire de donnees de l'utilisateur, au milieu de celui de toutes les autres
    /// applications.
    #[test]
    fn le_chemin_porte_l_identifiant() {
        assert!(base().unwrap().to_string_lossy().contains(IDENTIFIANT));
    }

    // ── La reprise des donnees laissees sous l'ancien nom ───────────────────────────────────

    /// Un bac a sable isole par test : deux tests qui partageraient un repertoire se
    /// marcheraient dessus, `cargo test` les lancant en parallele.
    fn bac_a_sable(nom: &str) -> PathBuf {
        let chemin = std::env::temp_dir().join(format!(
            "oyant-reprise-{nom}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&chemin);
        std::fs::create_dir_all(&chemin).unwrap();
        chemin
    }

    /// Le cas qui justifie la fonction : 3,1 Go de modeles ne se retelechargent pas.
    #[test]
    fn la_reprise_deplace_ce_qui_etait_sous_l_ancien_nom() {
        let sable = bac_a_sable("deplace");
        let ancien = sable.join("org.breizhzion.dictum.desktop");
        let nouveau = sable.join("org.breizhzion.oyant.desktop");
        std::fs::create_dir_all(ancien.join("modeles")).unwrap();
        std::fs::write(ancien.join("modeles").join("small.bin"), b"un modele").unwrap();

        assert_eq!(migrer_repertoire(&ancien, &nouveau), Ok(true));
        assert!(
            !ancien.exists(),
            "l'ancien repertoire aurait du disparaitre"
        );
        assert_eq!(
            std::fs::read(nouveau.join("modeles").join("small.bin")).unwrap(),
            b"un modele"
        );
        let _ = std::fs::remove_dir_all(&sable);
    }

    /// ⛔ Le cas qui protege contre une perte SILENCIEUSE : quelqu'un a deja lance la nouvelle
    /// version, donc il a des reglages neufs, et les ecraser par les anciens serait pire que de
    /// ne rien migrer du tout.
    #[test]
    fn la_reprise_n_ecrase_jamais_ce_qui_existe_deja() {
        let sable = bac_a_sable("ecrase");
        let ancien = sable.join("org.breizhzion.dictum.desktop");
        let nouveau = sable.join("org.breizhzion.oyant.desktop");
        std::fs::create_dir_all(&ancien).unwrap();
        std::fs::write(ancien.join("reglages.json"), b"les anciens").unwrap();
        std::fs::create_dir_all(&nouveau).unwrap();
        std::fs::write(nouveau.join("reglages.json"), b"les neufs").unwrap();

        assert_eq!(migrer_repertoire(&ancien, &nouveau), Ok(false));
        assert_eq!(
            std::fs::read(nouveau.join("reglages.json")).unwrap(),
            b"les neufs",
            "les reglages neufs ont ete ecrases par les anciens"
        );
        let _ = std::fs::remove_dir_all(&sable);
    }

    /// Relancee, elle ne trouve plus rien et ne doit pas se plaindre.
    #[test]
    fn la_reprise_est_idempotente() {
        let sable = bac_a_sable("idempotent");
        let ancien = sable.join("org.breizhzion.dictum.desktop");
        let nouveau = sable.join("org.breizhzion.oyant.desktop");
        std::fs::create_dir_all(&ancien).unwrap();

        assert_eq!(migrer_repertoire(&ancien, &nouveau), Ok(true));
        assert_eq!(migrer_repertoire(&ancien, &nouveau), Ok(false));
        assert_eq!(migrer_repertoire(&ancien, &nouveau), Ok(false));
        let _ = std::fs::remove_dir_all(&sable);
    }

    /// ⚠️ Rien a reprendre est le cas NORMAL, pas une anomalie : c'est celui de toute nouvelle
    /// installation.
    #[test]
    fn ne_rien_trouver_n_est_pas_une_erreur() {
        let sable = bac_a_sable("vide");
        assert_eq!(
            migrer_repertoire(&sable.join("absent"), &sable.join("neuf")),
            Ok(false)
        );
        let _ = std::fs::remove_dir_all(&sable);
    }
}
