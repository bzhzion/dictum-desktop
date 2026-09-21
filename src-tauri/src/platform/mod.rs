//! Ce que la plateforme courante permet reellement.
//!
//! Les quatre briques natives de Dictum (capture audio, raccourci global, injection au curseur,
//! icone de zone de notification) n'ont pas la meme implementation partout, et le cas Linux n'en
//! a pas UNE mais DEUX. Ce module isole ce choix pour que le reste du code ne connaisse jamais
//! d'`#[cfg(windows)]` disperse.
//!
//! ⚠️ Le cas Linux est celui qui justifie ce module a lui seul : l'injection de texte n'y a pas
//! une implementation mais deux, `libei` sous GNOME et KDE, et `zwp_virtual_keyboard_v1` sous
//! Hyprland et les compositeurs wlroots, qui ne servent pas le portail `RemoteDesktop`. Le
//! decouvrir apres avoir ecrit une injection Windows en dur couterait une reecriture.

/// Famille de systeme, passee en parametre plutot que lue dans les `cfg!` afin que la logique de
/// decision reste testable depuis n'importe quelle machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Famille {
    Windows,
    MacOs,
    Linux,
}

/// Comment injecter du texte au curseur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Injection {
    /// API native de saisie.
    Windows,
    /// API native, mais conditionnee a l'autorisation d'accessibilite accordee par l'utilisateur.
    MacOsAccessibilite,
    /// X11 : voie historique, sans restriction particuliere.
    X11,
    /// Wayland avec le portail `RemoteDesktop` et `libei` (GNOME >= 46, KDE Plasma >= 6.1).
    /// Ni droits root, ni groupe `input`, ni udev, et compatible Flatpak.
    WaylandLibei,
    /// ⛔ Wayland SANS portail `RemoteDesktop`, cas de Hyprland donc d'Omarchy.
    ///
    /// `xdg-desktop-portal-hyprland` fournit la capture d'entrees mais pas `RemoteDesktop`, si
    /// bien que la voie `libei` n'y emet **rien du tout**. Le repli est le protocole wlroots
    /// `zwp_virtual_keyboard_v1`, qui fonctionne, avec un defaut connu : Hyprland ne declenche
    /// pas les raccourcis a base de SUPER venant d'un clavier virtuel, alors qu'ALT et CTRL
    /// passent.
    WaylandClavierVirtuel,
    /// Session Wayland dont le bureau ne nous dit rien. On ne devine pas : un choix errone
    /// produirait une injection silencieusement inerte, exactement le mode d'echec qu'on veut
    /// supprimer. L'appelant doit le dire a l'utilisateur.
    Indeterminee,
}

/// Bureaux dont on sait qu'ils servent le portail `RemoteDesktop`, donc `libei`.
const BUREAUX_LIBEI: [&str; 2] = ["gnome", "kde"];

/// Bureaux wlroots connus pour ne pas le servir.
const BUREAUX_CLAVIER_VIRTUEL: [&str; 3] = ["hyprland", "sway", "wlroots"];

/// Decide de la strategie d'injection.
///
/// `session` vient de `XDG_SESSION_TYPE`, `bureau` de `XDG_CURRENT_DESKTOP`. Les deux sont
/// optionnels parce qu'ils le sont reellement : une session distante ou minimale peut n'en poser
/// aucun.
pub fn strategie_injection(
    famille: Famille,
    session: Option<&str>,
    bureau: Option<&str>,
) -> Injection {
    match famille {
        Famille::Windows => Injection::Windows,
        Famille::MacOs => Injection::MacOsAccessibilite,
        Famille::Linux => {
            // `XDG_CURRENT_DESKTOP` peut valoir plusieurs valeurs separees par deux-points
            // (par exemple `Hyprland:wlroots`), et sa casse n'est pas garantie.
            let bureau = bureau.unwrap_or_default().to_ascii_lowercase();
            let jetons: Vec<&str> = bureau.split(':').filter(|j| !j.is_empty()).collect();

            match session.unwrap_or_default().to_ascii_lowercase().as_str() {
                "x11" => Injection::X11,
                "wayland" => {
                    if jetons.iter().any(|j| BUREAUX_CLAVIER_VIRTUEL.contains(j)) {
                        Injection::WaylandClavierVirtuel
                    } else if jetons.iter().any(|j| BUREAUX_LIBEI.contains(j)) {
                        Injection::WaylandLibei
                    } else {
                        Injection::Indeterminee
                    }
                }
                // Pas de `XDG_SESSION_TYPE` : on ne suppose pas X11 par confort, une supposition
                // fausse rendrait l'injection inerte sans le dire.
                _ => Injection::Indeterminee,
            }
        }
    }
}

/// Famille du systeme sur lequel ce binaire tourne.
pub fn famille_courante() -> Famille {
    if cfg!(target_os = "windows") {
        Famille::Windows
    } else if cfg!(target_os = "macos") {
        Famille::MacOs
    } else {
        Famille::Linux
    }
}

/// Strategie d'injection pour la machine courante, lue depuis l'environnement reel.
pub fn strategie_courante() -> Injection {
    let session = std::env::var("XDG_SESSION_TYPE").ok();
    let bureau = std::env::var("XDG_CURRENT_DESKTOP").ok();
    strategie_injection(famille_courante(), session.as_deref(), bureau.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_et_macos_ne_dependent_pas_de_l_environnement() {
        assert_eq!(
            strategie_injection(Famille::Windows, Some("wayland"), Some("gnome")),
            Injection::Windows
        );
        assert_eq!(
            strategie_injection(Famille::MacOs, None, None),
            Injection::MacOsAccessibilite
        );
    }

    #[test]
    fn x11_reste_la_voie_historique() {
        assert_eq!(
            strategie_injection(Famille::Linux, Some("x11"), Some("GNOME")),
            Injection::X11
        );
    }

    #[test]
    fn gnome_et_kde_en_wayland_passent_par_libei() {
        for bureau in ["GNOME", "KDE", "ubuntu:GNOME"] {
            assert_eq!(
                strategie_injection(Famille::Linux, Some("wayland"), Some(bureau)),
                Injection::WaylandLibei,
                "bureau {bureau}"
            );
        }
    }

    /// Le cas Omarchy, celui qui justifie l'existence de ce module.
    #[test]
    fn hyprland_tombe_sur_le_clavier_virtuel_et_jamais_sur_libei() {
        for bureau in ["Hyprland", "hyprland", "Hyprland:wlroots"] {
            assert_eq!(
                strategie_injection(Famille::Linux, Some("wayland"), Some(bureau)),
                Injection::WaylandClavierVirtuel,
                "bureau {bureau}"
            );
        }
    }

    /// ⚠️ Le point qui compte : `Hyprland:wlroots` contient AUSSI un jeton qui pourrait passer
    /// pour un bureau generique. L'ordre des tests dans la fonction doit donner la priorite au
    /// clavier virtuel, sinon Hyprland serait classe en `libei` et n'injecterait rien.
    #[test]
    fn un_bureau_composite_ne_bascule_pas_par_erreur_sur_libei() {
        assert_eq!(
            strategie_injection(Famille::Linux, Some("wayland"), Some("Hyprland:GNOME")),
            Injection::WaylandClavierVirtuel
        );
    }

    #[test]
    fn un_environnement_muet_est_indetermine_et_pas_devine() {
        assert_eq!(
            strategie_injection(Famille::Linux, None, None),
            Injection::Indeterminee
        );
        assert_eq!(
            strategie_injection(Famille::Linux, Some("wayland"), None),
            Injection::Indeterminee
        );
        assert_eq!(
            strategie_injection(Famille::Linux, Some("wayland"), Some("Cinnamon")),
            Injection::Indeterminee
        );
    }
}
