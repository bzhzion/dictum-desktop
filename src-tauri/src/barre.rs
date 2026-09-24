//! Icone de la zone de notification : c'est la qu’Oyant vit reellement.
//!
//! La fenetre n'est qu'une facette de l'application, convoquee depuis ici. Fermer la fenetre ne
//! doit donc jamais arreter le programme, sans quoi le raccourci global cesserait de repondre
//! alors que l'icone serait toujours visible.
//!
//! ⛔ **Ce menu ne porte que des ACTIONS, jamais de reglage** (arbitre par painteau le
//! 2026-09-17). Un menu d'icone sert a faire quelque chose en deux gestes sans ouvrir la fenetre :
//! lancer un enregistrement, traduire. **Les reglages de l'application vivent uniquement dans
//! l'ecran de reglages**, et nulle part ailleurs.
//!
//! Le motif n'est pas esthetique. Un reglage present a deux endroits finit par y etre affiche
//! differemment : c'est exactement le defaut que « relire l'etat depuis le systeme » evitait deja
//! pour le demarrage automatique, et le dupliquer dans deux surfaces le reintroduirait par une
//! autre porte. « Demarrer avec le systeme » etait ici jusqu'au 2026-09-17 ; il est parti dans
//! `reglages.rs`.
//!
//! Il doit aussi rester **court** : les actions arrivent a leur etape, pas avant. Une entree
//! grisee pour une fonctionnalite qui n'existe pas encore encombrerait sans rien rendre.

use std::sync::Mutex;

use tauri::{
    AppHandle, Manager,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};

/// Etat affiche par l'icone.
///
/// ⚠️ Chaque etat porte SON LIBELLE, et l'infobulle le rend visible. Une icone de zone de
/// notification ne peut pas afficher de texte, donc l'infobulle est le seul endroit ou l'etat est
/// nomme : sans elle, la couleur porterait l'information seule, ce que WCAG 1.4.1 interdit et ce
/// qui rendrait l'application inutilisable a qui distingue mal le rouge du vert.
// ⚠️ `Ecoute` reste inutilise : il est prevu pour la transcription en direct, qui attend son
// etape. Les autres sont branches depuis l'etape 7.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etat {
    Repos,
    Ecoute,
    Enregistrement,
    Transcription,
}

impl Etat {
    /// Icone teintee correspondante, produite par `scripts/preparer-icones-barre.py`.
    ///
    /// Les images sont EMBARQUEES a la compilation : une icone de barre lue depuis le disque a
    /// l'execution disparaitrait si le fichier manquait, sans rien signaler.
    fn octets(self) -> &'static [u8] {
        match self {
            Etat::Repos => include_bytes!("../icons/barre/repos.png"),
            Etat::Ecoute => include_bytes!("../icons/barre/ecoute.png"),
            Etat::Enregistrement => include_bytes!("../icons/barre/enregistrement.png"),
            Etat::Transcription => include_bytes!("../icons/barre/transcription.png"),
        }
    }

    /// Ce que dit l'infobulle. Toujours prefixe du nom du logiciel : dans une barre des taches
    /// encombree, une infobulle qui dirait seulement « Pret » ne designerait rien.
    fn infobulle(self) -> &'static str {
        match self {
            Etat::Repos => "Oyant : prêt",
            Etat::Ecoute => "Oyant : écoute",
            Etat::Enregistrement => "Oyant : enregistrement",
            Etat::Transcription => "Oyant : transcription en cours",
        }
    }
}

/// Affiche la fenetre et lui donne le focus.
///
/// ⚠️ `show` ne suffit pas : une fenetre masquee puis reaffichee peut revenir DERRIERE les autres,
/// ce qui se lit comme un clic sans effet. `unminimize` est necessaire en plus parce qu'une
/// fenetre reduite n'est pas « masquee » au sens de Tauri.
pub fn montrer_fenetre(app: &AppHandle) {
    if let Some(fenetre) = app.get_webview_window("main") {
        let _ = fenetre.unminimize();
        let _ = fenetre.show();
        let _ = fenetre.set_focus();
    }
}

/// Pose l'icone dans la zone de notification.
/// Libelles de l'entree de dictee, selon qu'une dictee est en cours ou non.
///
/// ⛔ **L'entree doit dire ce qu'elle VA faire, pas ce qu'elle est.** Un libelle fixe « Dicter »
/// qui arrete la dictee une fois sur deux est exactement le genre de menu qui mente, et c'est ce
/// que l'invariant de l'icone interdit deja par ailleurs.
pub const DICTER: &str = "Dicter";
pub const ARRETER: &str = "Arrêter la dictée";

/// L'entree de dictee, conservee pour pouvoir changer son libelle.
///
/// ⚠️ Gardee dans l'etat de l'application parce qu'un `MenuItem` ne se retrouve pas depuis le
/// menu : sans cette reference, le libelle serait fige a sa valeur de construction.
pub struct EntreeDictee(pub Mutex<Option<MenuItem<tauri::Wry>>>);

pub fn installer(app: &AppHandle) -> tauri::Result<()> {
    let ouvrir = MenuItem::with_id(app, "ouvrir", "Ouvrir Oyant", true, None::<&str>)?;
    // ⛔ Le menu ne porte QUE des actions (arbitre par painteau le 2026-09-17), et « Dicter » est
    // la premiere qui existe. Les reglages restent dans l'ecran de reglages.
    let dicter = MenuItem::with_id(app, "dicter", DICTER, true, None::<&str>)?;
    let separateur = PredefinedMenuItem::separator(app)?;
    let quitter = MenuItem::with_id(app, "quitter", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&dicter, &ouvrir, &separateur, &quitter])?;

    app.manage(EntreeDictee(Mutex::new(Some(dicter))));

    TrayIconBuilder::with_id("principale")
        // ⚠️ Sans cette ligne, le menu est construit puis jamais attache : le clic droit
        // n'afficherait rien. Defaut attrape par clippy (« unused variable: menu ») et pas par
        // la relecture, qui voyait un menu bien forme quelques lignes plus haut.
        .menu(&menu)
        .icon(Image::from_bytes(Etat::Repos.octets())?)
        .tooltip(Etat::Repos.infobulle())
        // ⚠️ `false` est delibere : par defaut, un clic GAUCHE ouvre le menu. Or le geste attendu
        // au clic gauche est d'ouvrir la fenetre, le menu restant sur le clic droit, comme le
        // veut l'usage sur les trois systemes.
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|icone, evenement| {
            // ⚠️ On ne filtre PAS sur `button_state`. Un clic gauche depuis le **debordement**
            // de la zone de notification de Windows 11 (le chevron qui replie les icones) ne
            // delivre pas la meme sequence qu'un clic sur une icone epinglee : filtrer sur
            // `Up` seul rendait le clic totalement inerte depuis le debordement, verifie le
            // 2026-09-17, alors que l'entree « Ouvrir Oyant » du menu, qui appelle la MEME
            // fonction, fonctionnait. C'est ce qui a permis de situer le defaut dans
            // l'evenement et pas dans l'ouverture de la fenetre.
            //
            // Reagir aux deux etats est sans risque : `montrer_fenetre` est idempotente, et
            // montrer une fenetre deja visible ne fait rien.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = evenement
            {
                montrer_fenetre(icone.app_handle());
            }
        })
        .on_menu_event(|app, evenement| match evenement.id().as_ref() {
            "ouvrir" => montrer_fenetre(app),
            "dicter" => crate::dictee::basculer(app),
            // Seul chemin qui arrete reellement Oyant. La croix de la fenetre, elle, masque.
            "quitter" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Met le libelle de l'entree de dictee en accord avec ce qu'elle va faire.
pub fn rafraichir_entree_dictee(app: &AppHandle, en_cours: bool) -> tauri::Result<()> {
    if let Some(entree) = app.try_state::<EntreeDictee>()
        && let Ok(verrou) = entree.0.lock()
        && let Some(element) = verrou.as_ref()
    {
        element.set_text(libelle_dictee(en_cours))?;
    }
    Ok(())
}

/// Fonction PURE, pour que la regle « l'entree annonce ce qu'elle va faire » soit testable.
pub fn libelle_dictee(en_cours: bool) -> &'static str {
    if en_cours { ARRETER } else { DICTER }
}

/// Change l'etat affiche par l'icone, image ET infobulle ensemble.
///
/// ⚠️ Les deux sont changes dans la meme fonction pour qu'ils ne puissent pas diverger : une
/// icone rouge sous une infobulle qui dit « pret » serait pire que pas d'indication du tout.
pub fn definir_etat(app: &AppHandle, etat: Etat) -> tauri::Result<()> {
    if let Some(icone) = app.tray_by_id("principale") {
        icone.set_icon(Some(Image::from_bytes(etat.octets())?))?;
        icone.set_tooltip(Some(etat.infobulle()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⛔ **L'entree du menu doit annoncer ce qu'elle VA faire.** Le menu ne peut pas se tenir
    /// enfonce comme le raccourci, donc il bascule ; un libelle fige « Dicter » qui arrete la
    /// dictee une fois sur deux serait un menu qui ment, ce que l'invariant de l'icone interdit
    /// deja par ailleurs.
    #[test]
    fn l_entree_de_dictee_annonce_ce_qu_elle_va_faire() {
        assert_eq!(libelle_dictee(false), DICTER);
        assert_eq!(libelle_dictee(true), ARRETER);
        assert_ne!(DICTER, ARRETER);
        // Le libelle d'arret doit dire qu'il ARRETE, sans quoi il decrirait l'etat courant au
        // lieu de l'action, ce qui se lit exactement a l'envers.
        assert!(ARRETER.to_lowercase().contains("arrêter"));
    }

    /// ⚠️ Chaque etat doit avoir sa propre icone ET sa propre infobulle. Deux etats qui
    /// partageraient l'une ou l'autre seraient indistinguables a l'usage, ce qui est exactement
    /// le defaut que l'indicateur existe pour eviter.
    #[test]
    fn chaque_etat_a_son_icone_et_son_libelle_propres() {
        let etats = [
            Etat::Repos,
            Etat::Ecoute,
            Etat::Enregistrement,
            Etat::Transcription,
        ];

        for (i, a) in etats.iter().enumerate() {
            for b in &etats[i + 1..] {
                assert_ne!(
                    a.octets(),
                    b.octets(),
                    "{a:?} et {b:?} partagent leur icone"
                );
                assert_ne!(
                    a.infobulle(),
                    b.infobulle(),
                    "{a:?} et {b:?} partagent leur infobulle"
                );
            }
        }
    }

    /// Les fichiers sont embarques a la compilation : s'ils etaient vides ou absents, le defaut
    /// ne se verrait qu'a l'execution, sous la forme d'une icone invisible.
    #[test]
    fn les_icones_embarquees_ne_sont_pas_vides() {
        for etat in [
            Etat::Repos,
            Etat::Ecoute,
            Etat::Enregistrement,
            Etat::Transcription,
        ] {
            let octets = etat.octets();
            assert!(octets.len() > 100, "{etat:?} : icone suspecte");
            // Signature PNG, pour attraper un fichier remplace par autre chose.
            assert_eq!(
                &octets[..8],
                b"\x89PNG\r\n\x1a\n",
                "{etat:?} : ce n'est pas un PNG"
            );
        }
    }

    /// L'infobulle nomme toujours le logiciel : dans une barre encombree, « Pret » seul ne
    /// designerait rien.
    #[test]
    fn l_infobulle_nomme_le_logiciel() {
        for etat in [
            Etat::Repos,
            Etat::Ecoute,
            Etat::Enregistrement,
            Etat::Transcription,
        ] {
            assert!(
                etat.infobulle().starts_with("Oyant"),
                "{etat:?} : {}",
                etat.infobulle()
            );
        }
    }
}
