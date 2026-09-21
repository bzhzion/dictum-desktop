//! Ou Dictum range ses donnees.
//!
//! ⛔ **Une seule implementation, utilisee par l'interface ET par la ligne de commande.** La
//! ligne de commande n'a pas d'application Tauri sous la main, donc elle ne peut pas appeler
//! `app.path()`. Si chacune calculait son chemin de son cote, `dictum fichier.wav` chercherait
//! les modeles ailleurs que la ou l'interface les a telecharges, et le message dirait « modele
//! absent » sur une machine ou 3 Go de modeles sont parfaitement installes.
//!
//! ⛔ **Profil LOCAL et jamais itinerant sur Windows.** Dans un domaine, le profil itinerant est
//! recopie sur le reseau a chaque ouverture de session : 3 Go de modeles le rendraient
//! inutilisable. Invisible sur une machine personnelle, redhibitoire en entreprise.
//!
//! ⛔ **Et jamais le repertoire d'installation**, que la desinstallation emporte. C'est ce que
//! faisait l'ancienne version, et c'est ce qui a produit la collision du 2026-09-17.

use std::path::PathBuf;

/// Identifiant de l'application, celui du manifeste Tauri.
///
/// ⚠️ Recopie ici parce que la ligne de commande n'a pas acces au contexte Tauri. Le test
/// `l_identifiant_est_celui_du_manifeste` compare les deux a la compilation, donc les deux ne
/// peuvent pas diverger sans que quelque chose le dise.
const IDENTIFIANT: &str = "org.breizhzion.dictum.desktop";

/// Repertoire de donnees de l'application.
pub fn base() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let local = std::env::var("LOCALAPPDATA")
            .map_err(|_| "Variable LOCALAPPDATA absente.".to_string())?;
        Ok(PathBuf::from(local).join(IDENTIFIANT))
    }

    #[cfg(target_os = "macos")]
    {
        let maison = std::env::var("HOME").map_err(|_| "Variable HOME absente.".to_string())?;
        Ok(PathBuf::from(maison)
            .join("Library")
            .join("Application Support")
            .join(IDENTIFIANT))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Convention XDG : la variable si elle est posee, sinon son defaut.
        if let Ok(donnees) = std::env::var("XDG_DATA_HOME") {
            if !donnees.is_empty() {
                return Ok(PathBuf::from(donnees).join(IDENTIFIANT));
            }
        }
        let maison = std::env::var("HOME").map_err(|_| "Variable HOME absente.".to_string())?;
        Ok(PathBuf::from(maison)
            .join(".local")
            .join("share")
            .join(IDENTIFIANT))
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
    #[cfg(target_os = "windows")]
    {
        let itinerant =
            std::env::var("APPDATA").map_err(|_| "Variable APPDATA absente.".to_string())?;
        Ok(PathBuf::from(itinerant).join(IDENTIFIANT))
    }

    #[cfg(target_os = "macos")]
    {
        // macOS ne distingue pas les deux, comme Tauri lui-meme.
        base()
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(config) = std::env::var("XDG_CONFIG_HOME") {
            if !config.is_empty() {
                return Ok(PathBuf::from(config).join(IDENTIFIANT));
            }
        }
        let maison = std::env::var("HOME").map_err(|_| "Variable HOME absente.".to_string())?;
        Ok(PathBuf::from(maison).join(".config").join(IDENTIFIANT))
    }
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

    /// ⚠️ Le chemin doit porter l'identifiant : sans lui, Dictum ecrirait a la racine du
    /// repertoire de donnees de l'utilisateur, au milieu de celui de toutes les autres
    /// applications.
    #[test]
    fn le_chemin_porte_l_identifiant() {
        assert!(base().unwrap().to_string_lossy().contains(IDENTIFIANT));
    }
}
