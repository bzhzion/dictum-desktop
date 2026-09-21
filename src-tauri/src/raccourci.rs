//! Le raccourci global : la seule chose qui declenche une dictee.
//!
//! ⛔ **Il doit rendre l'appui ET le relachement.** Le raccourci de Dictum se TIENT enfonce : on
//! parle pendant, et relacher lance la transcription. Un raccourci a bascule laisserait un
//! microphone ouvert sans que rien ne le rappelle, dans une application dont la fenetre n'est
//! meme pas affichee.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::dictee;

/// Traduit le libelle des reglages vers ce que le greffon attend.
///
/// ⚠️ **Les deux vocabulaires ne coincident pas** : les reglages ecrivent `Ctrl+Alt+Space`,
/// hérité de l'ancienne version, la ou le greffon veut `CommandOrControl+Alt+Space`. Traduire
/// ici plutot que de changer le format du fichier de configuration, qui est lu par des gens et
/// recopie d'une machine a l'autre.
///
/// Fonction PURE : elle se teste sans enregistrer quoi que ce soit aupres du systeme.
pub fn normaliser(libelle: &str) -> String {
    libelle
        .split('+')
        .map(str::trim)
        .filter(|partie| !partie.is_empty())
        .map(|partie| match partie.to_ascii_lowercase().as_str() {
            // `Ctrl` seul n'est pas compris ; `CommandOrControl` vaut Ctrl ici et Cmd sur macOS,
            // ce qui evite d'ecrire deux reglages pour une seule intention.
            "ctrl" | "control" => "CommandOrControl".to_string(),
            "cmd" | "command" | "super" | "meta" | "win" => "Super".to_string(),
            "alt" | "option" => "Alt".to_string(),
            "shift" | "maj" => "Shift".to_string(),
            // ⚠️ On rend `partie` et NON la version abaissee sur laquelle on a filtre : le
            // greffon attend `Space` et `F9`, pas `space` ni `f9`. Rendre la chaine du `match`
            // transformait silencieusement toutes les touches en minuscules, et aucun raccourci
            // ne s'enregistrait plus.
            _ => partie.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Enregistre le raccourci des reglages aupres du systeme.
pub fn installer(app: &AppHandle) -> Result<(), String> {
    let reglages = crate::reglages::lire_sans_application();
    enregistrer(app, &reglages.raccourci)
}

/// Enregistre un raccourci precis, en retirant d'abord ceux qui sont poses.
///
/// ⚠️ Idempotent : appele deux fois avec le meme libelle, il ne laisse qu'un enregistrement.
/// Sans ca, changer de raccourci dans les reglages empilerait l'ancien et le nouveau, et
/// l'ancien continuerait a declencher des dictees sans apparaitre nulle part.
pub fn enregistrer(app: &AppHandle, libelle: &str) -> Result<(), String> {
    let normalise = normaliser(libelle);
    let raccourci: Shortcut = normalise
        .parse()
        .map_err(|_| format!("Le raccourci « {libelle} » n'est pas valide."))?;

    let gestionnaire = app.global_shortcut();
    let _ = gestionnaire.unregister_all();

    // ⚠️ **Une trace a l'enregistrement, et une a chaque declenchement.** Sans elles, un raccourci
    // qui ne repond pas est indistinguable d'un raccourci jamais enregistre : les deux se
    // manifestent par une application parfaitement silencieuse. Constate le 2026-09-21 en
    // eprouvant X11, ou le journal etait vide et ne permettait aucun diagnostic.
    eprintln!("raccourci : « {libelle} » enregistré sous « {normalise} »");

    gestionnaire
        .on_shortcut(raccourci, move |app, _raccourci, evenement| {
            match evenement.state() {
                ShortcutState::Pressed => {
                    eprintln!("raccourci : appui");
                    dictee::commencer(app);
                }
                ShortcutState::Released => {
                    eprintln!("raccourci : relâchement");
                    dictee::terminer(app);
                }
            }
        })
        .map_err(|erreur| {
        format!(
            "Le raccourci « {libelle} » n'a pas pu être enregistré : il est peut-être déjà pris \
             par une autre application. ({erreur})"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⛔ **Garde l'ecart de vocabulaire entre le fichier de reglages et le greffon.** Les
    /// reglages ecrivent `Ctrl`, que le greffon ne comprend pas : sans traduction, le raccourci
    /// n'est jamais enregistre et la dictee ne se declenche **jamais**, sans message puisque
    /// l'application demarre normalement.
    #[test]
    fn ctrl_devient_le_nom_que_le_greffon_comprend() {
        assert_eq!(normaliser("Ctrl+Alt+Space"), "CommandOrControl+Alt+Space");
        assert_eq!(normaliser("Control+F9"), "CommandOrControl+F9");
    }

    /// ⚠️ Le fichier de reglages est edite a la main : il arrive avec des espaces et des casses
    /// variees, qui ne doivent pas faire echouer l'enregistrement.
    #[test]
    fn la_casse_et_les_espaces_ne_changent_rien() {
        assert_eq!(
            normaliser("ctrl + alt + Space"),
            "CommandOrControl+Alt+Space"
        );
        assert_eq!(normaliser("SHIFT+Insert"), "Shift+Insert");
    }

    /// ⚠️ Les synonymes des touches de commande existent parce que trois systemes les nomment
    /// differemment, et que le meme fichier de reglages voyage de l'un a l'autre.
    #[test]
    fn les_synonymes_des_modificateurs_sont_acceptes() {
        for libelle in ["Cmd+Space", "Command+Space", "Super+Space", "Win+Space"] {
            assert_eq!(normaliser(libelle), "Super+Space", "{libelle}");
        }
        assert_eq!(normaliser("Option+F1"), "Alt+F1");
    }

    /// ⚠️ Une touche inconnue passe telle quelle : c'est au greffon de la refuser, avec son
    /// vocabulaire a lui. La traduire au hasard produirait un raccourci silencieusement different
    /// de celui qui est ecrit dans les reglages.
    #[test]
    fn une_touche_inconnue_n_est_pas_devinee() {
        assert_eq!(normaliser("Ctrl+Inconnue"), "CommandOrControl+Inconnue");
    }
}
