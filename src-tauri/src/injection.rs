//! Ecrire le texte transcrit la ou se trouve le curseur, dans n'importe quelle application.
//!
//! ⛔ **C'est la fonction la plus intrusive du produit : elle tape du texte arbitraire dans
//! l'ordinateur de quelqu'un.** Tout ce qui la declenche doit etre volontaire, et tout ce qu'elle
//! ecrit doit etre exactement ce que le moteur a rendu.
//!
//! Sur Windows on passe par `SendInput` en mode Unicode, et **jamais** par des codes de touches
//! virtuelles. La raison est simple : un code de touche depend de la disposition du clavier, si
//! bien qu'un `a` tape sur une disposition AZERTY programmee en QWERTY ressort en `q`. Le mode
//! Unicode envoie le CARACTERE, ce qui rend la disposition sans objet, accents compris.

/// Envoie le texte au curseur.
///
/// `delai_ms` est une pause **entre chaque touche**, pas avant la première.
///
/// ⛔ **Son defaut est ZERO, et il valait 50 ms jusqu'au 2026-09-18.** A 50 ms par caractere, une
/// phrase de cent caracteres mettait **cinq secondes** a s'ecrire, ce que painteau a constate au
/// premier essai reel sans pouvoir en identifier la cause. Pire, le libelle de l'ecran annoncait
/// « le temps que Dictum attend **avant** de taper, pour laisser la fenetre redevenir active » :
/// un reglage qui decrit une chose et en fait une autre est plus couteux qu'un reglage absent,
/// puisqu'on le tourne dans le mauvais sens en croyant bien faire.
///
/// L'attente que ce libelle promettait est ailleurs, dans `redonner_le_focus`. Ce delai-ci ne sert
/// qu'aux applications qui perdent des caracteres arrivant trop vite : terminaux, machines
/// virtuelles, jeux. Elles sont l'exception, donc zero est le bon defaut.
pub fn ecrire(texte: &str, delai_ms: u32) -> Result<(), String> {
    if texte.is_empty() {
        return Ok(());
    }
    implementation::ecrire(texte, delai_ms)
}

/// La fenetre dans laquelle le texte devrait atterrir, vue depuis notre propre interface.
///
/// ⛔ **Existe pour le menu de l'icone, et pour lui seul.** Ouvrir ce menu donne le focus a
/// Dictum : sans cette fonction, le texte serait injecte dans notre propre fenetre ou dans le
/// vide. Le raccourci global, lui, ne touche jamais au focus, donc il ne s'en sert pas : y
/// forcer un retour ecraserait un changement de fenetre deliberement fait pendant qu'on parle.
pub fn fenetre_cible() -> Option<isize> {
    implementation::fenetre_cible()
}

/// Ramene la fenetre au premier plan avant d'y ecrire.
pub fn redonner_le_focus(cible: isize) -> bool {
    implementation::redonner_le_focus(cible)
}

/// Decoupe le texte en unites de code UTF-16, l'alphabet de `SendInput`.
///
/// ⛔ **Un caractere hors du plan de base occupe DEUX unites** (emoji, ideogrammes rares), et
/// chacune doit partir dans son propre evenement, dans l'ordre. Envoyer le point de code brut
/// ferait disparaitre ces caracteres en silence, ce qui est precisement le genre de perte qu'on
/// ne remarque qu'une fois le texte ecrit chez quelqu'un d'autre.
///
/// Fonction PURE, testable sans toucher au clavier de la machine qui lance les tests.
///
/// ⚠️ **Inutilisee hors de Windows, et c'est normal** : `SendInput` est la seule API qui parle en
/// unites UTF-16. Les tests l'exercent sur toutes les plateformes, mais la compilation du binaire
/// seul la voit morte sur macOS et Linux, ou `clippy` la refuse. L'autorisation est donc posee
/// **uniquement la ou elle est vraie**, jamais en general : un `allow` sans condition masquerait
/// le jour ou elle deviendrait reellement morte partout.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn unites_utf16(texte: &str) -> Vec<u16> {
    texte.encode_utf16().collect()
}

#[cfg(windows)]
mod implementation {
    use std::thread::sleep;
    use std::time::Duration;

    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
        VIRTUAL_KEY,
    };

    pub fn ecrire(texte: &str, delai_ms: u32) -> Result<(), String> {
        let unites = super::unites_utf16(texte);

        for unite in unites {
            // ⚠️ Appui ET relachement. Beaucoup d'applications n'inserent le caractere qu'au
            // relachement ; n'envoyer que l'appui marche dans le Bloc-notes et pas ailleurs,
            // c'est-a-dire que ca marche exactement la ou on teste.
            let entrees = [evenement(unite, false), evenement(unite, true)];

            // SAFETY : `entrees` est un tableau valide de `INPUT` correctement initialises, et
            // la taille passee est celle de la structure, comme l'exige l'API.
            let envoyes = unsafe { SendInput(&entrees, std::mem::size_of::<INPUT>() as i32) };
            if envoyes as usize != entrees.len() {
                return Err(
                    "Windows a refusé la saisie. Une application au premier plan s'exécute \
                     peut-être avec des droits plus élevés que Dictum."
                        .to_string(),
                );
            }

            if delai_ms > 0 {
                sleep(Duration::from_millis(delai_ms as u64));
            }
        }
        Ok(())
    }

    /// La premiere fenetre visible de l'ordre d'empilement qui n'est pas a nous.
    ///
    /// ⚠️ On parcourt l'ordre Z plutot que de lire simplement la fenetre active : au moment ou le
    /// menu de l'icone est ouvert, la fenetre active EST la notre, donc la lire rendrait Dictum
    /// lui-meme. Le premier voisin visible d'un autre processus est la meilleure approximation de
    /// « la ou l'utilisateur ecrivait juste avant ».
    pub fn fenetre_cible() -> Option<isize> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Threading::GetCurrentProcessId;
        use windows::Win32::UI::WindowsAndMessaging::{
            GW_HWNDNEXT, GetForegroundWindow, GetWindow, GetWindowThreadProcessId, IsWindowVisible,
        };

        let nous = unsafe { GetCurrentProcessId() };
        let mut fenetre: HWND = unsafe { GetForegroundWindow() };

        // Plafond volontaire : un parcours sans borne sur une liste circulaire abimee bloquerait
        // l'interface, et aucune machine n'a des centaines de fenetres de premier niveau utiles.
        for _ in 0..200 {
            if fenetre.is_invalid() {
                return None;
            }
            let visible = unsafe { IsWindowVisible(fenetre) }.as_bool();
            let mut proprietaire = 0u32;
            unsafe { GetWindowThreadProcessId(fenetre, Some(&mut proprietaire)) };

            if visible && proprietaire != nous && proprietaire != 0 {
                return Some(fenetre.0 as isize);
            }
            fenetre = match unsafe { GetWindow(fenetre, GW_HWNDNEXT) } {
                Ok(suivante) => suivante,
                Err(_) => return None,
            };
        }
        None
    }

    pub fn redonner_le_focus(cible: isize) -> bool {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            IsWindow, SW_RESTORE, SetForegroundWindow, ShowWindow,
        };

        let fenetre = HWND(cible as *mut std::ffi::c_void);
        // ⚠️ La fenetre a pu etre fermee pendant qu'on transcrivait : ecrire dans une poignee
        // morte ne leve rien, le texte part simplement nulle part.
        if !unsafe { IsWindow(Some(fenetre)) }.as_bool() {
            return false;
        }
        let _ = unsafe { ShowWindow(fenetre, SW_RESTORE) };
        let rendu = unsafe { SetForegroundWindow(fenetre) }.as_bool();
        if rendu {
            // Laisse au systeme le temps d'achever le basculement : injecter dans la foulee
            // enverrait les premieres touches a l'ancienne fenetre.
            std::thread::sleep(Duration::from_millis(120));
        }
        rendu
    }

    fn evenement(unite: u16, relachement: bool) -> INPUT {
        let mut drapeaux = KEYEVENTF_UNICODE;
        if relachement {
            drapeaux |= KEYEVENTF_KEYUP;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                ki: KEYBDINPUT {
                    // ⚠️ Nul en mode Unicode : c'est `wScan` qui porte le caractere. Y mettre
                    // autre chose ferait interpreter l'evenement comme une touche physique.
                    wVk: VIRTUAL_KEY(0),
                    wScan: unite,
                    dwFlags: drapeaux,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }
}

#[cfg(target_os = "linux")]
mod implementation {
    use std::thread::sleep;
    use std::time::Duration;

    use enigo::{Enigo, Keyboard, Settings};

    /// Ecrit le texte au curseur, par le serveur d'affichage.
    ///
    /// ⛔ **`enigo` choisit tout seul entre X11 et Wayland, et les deux ne se valent pas.** Sous
    /// X11 il passe par `XTest`, qui fonctionne sans autorisation particuliere. Sous Wayland il
    /// tente `libei`, qui exige le portail `RemoteDesktop` : celui-ci **n'existe pas sur
    /// Hyprland**, donc la saisie y part dans le vide sans erreur. C'est pourquoi le message
    /// d'echec nomme la session plutot que de parler d'un probleme generique.
    pub fn ecrire(texte: &str, delai_ms: u32) -> Result<(), String> {
        let mut clavier = Enigo::new(&Settings::default()).map_err(|erreur| {
            format!(
                "La saisie au curseur n'a pas pu s'initialiser ({erreur}). Sous Wayland, elle \
                 demande le portail RemoteDesktop, absent de certains bureaux."
            )
        })?;

        // ⚠️ Sans pause demandee, on envoie le texte d'un bloc : `enigo` sait le faire et c'est
        // nettement plus rapide que touche par touche. La pause n'a de sens que pour les
        // applications qui perdent des caracteres, et elle impose alors le decoupage.
        if delai_ms == 0 {
            return clavier
                .text(texte)
                .map_err(|erreur| format!("Saisie refusée : {erreur}"));
        }

        for caractere in texte.chars() {
            clavier
                .text(&caractere.to_string())
                .map_err(|erreur| format!("Saisie refusée : {erreur}"))?;
            sleep(Duration::from_millis(delai_ms as u64));
        }
        Ok(())
    }

    /// ⛔ **Sans objet sous Linux, et ce n'est pas un oubli.** La restauration de fenetre existe
    /// pour le menu de l'icone sous Windows, ou ouvrir le menu vole le focus. Les environnements
    /// Linux ne donnent pas a une application le droit de se remettre au premier plan a volonte,
    /// et un `None` ici fait prendre au code appelant le chemin « pas de cible a restaurer »,
    /// donc l'injection va la ou le focus se trouve. C'est le comportement correct, pas un repli.
    pub fn fenetre_cible() -> Option<isize> {
        None
    }

    pub fn redonner_le_focus(_cible: isize) -> bool {
        false
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod implementation {
    /// Les autres plateformes ont leur etape. ⚠️ Rendre une erreur explicite plutot qu'un
    /// succes vide : une injection silencieusement inerte est le mode d'echec le plus couteux a
    /// diagnostiquer, puisque tout le reste de la chaine a l'air de fonctionner.
    pub fn ecrire(_texte: &str, _delai_ms: u32) -> Result<(), String> {
        Err("L'injection au curseur n'est pas encore implémentée sur macOS.".to_string())
    }

    pub fn fenetre_cible() -> Option<isize> {
        None
    }

    pub fn redonner_le_focus(_cible: isize) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⛔ **Garde la perte silencieuse des caracteres hors plan de base.** Un emoji occupe deux
    /// unites UTF-16 : le traiter comme une seule le ferait disparaitre du texte injecte sans
    /// aucune erreur.
    #[test]
    fn un_caractere_hors_plan_de_base_occupe_deux_unites() {
        assert_eq!(unites_utf16("a").len(), 1);
        // Accent francais : toujours une seule unite, il est dans le plan de base.
        assert_eq!(unites_utf16("é").len(), 1);
        // Emoji : paire de substitution, donc deux unites.
        assert_eq!(unites_utf16("🙂").len(), 2);
        // Hangeul : plan de base, une unite, malgre trois octets en UTF-8.
        assert_eq!(unites_utf16("한").len(), 1);
    }

    /// ⚠️ L'ordre compte autant que le compte : une paire de substitution inversee ne rend pas
    /// le meme caractere, elle rend deux caracteres invalides.
    #[test]
    fn les_unites_sortent_dans_l_ordre_du_texte() {
        let unites = unites_utf16("a🙂b");
        assert_eq!(unites.len(), 4);
        assert_eq!(unites[0], 'a' as u16);
        assert_eq!(unites[3], 'b' as u16);
        // La paire de substitution respecte ses plages : haute puis basse.
        assert!((0xD800..0xDC00).contains(&unites[1]));
        assert!((0xDC00..0xE000).contains(&unites[2]));
    }

    /// ⚠️ Un texte vide ne doit rien envoyer du tout : sur les plateformes sans implementation,
    /// cela evite une erreur sur une action qui n'avait rien a faire.
    /// ⛔ **Garde la lenteur constatee au premier essai reel.** La pause est **entre chaque
    /// touche** : a 50 ms, une phrase de cent caracteres mettait cinq secondes a s'ecrire, et le
    /// libelle de l'ecran annoncait une attente unique avant de taper. Le defaut doit rester a
    /// zero, sans quoi le produit est lent pour tout le monde afin de servir l'exception.
    #[test]
    fn la_pause_entre_les_touches_vaut_zero_par_defaut() {
        let defauts = crate::reglages::Reglages::default();
        assert_eq!(
            defauts.delai_injection_ms, 0,
            "une pause par touche non nulle rend l'ecriture lente sur tout texte long"
        );
    }

    #[test]
    fn un_texte_vide_ne_declenche_aucune_saisie() {
        assert!(ecrire("", 0).is_ok());
        assert!(unites_utf16("").is_empty());
    }

    /// Ecrit reellement dans la fenetre au premier plan. **A lancer a la main**, jamais en
    /// automatique :
    ///
    /// ```text
    /// cargo test injection_reelle -- --ignored --nocapture
    /// ```
    ///
    /// ⛔ **`#[ignore]` n'est pas une commodite, c'est la protection.** Ce test tape du texte
    /// dans l'application active de la machine qui l'execute : lance par une integration continue
    /// sur un poste de travail partage, il ecrirait dans le document de quelqu'un. C'est le seul
    /// test du depot qui ait un effet en dehors du processus.
    ///
    /// Ce qu'il verifie et qu'aucun test de fonction pure ne peut couvrir : que `SendInput`
    /// atteint vraiment une autre application, que les accents passent sans dependre de la
    /// disposition du clavier, et qu'un caractere hors plan de base arrive entier.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    #[ignore = "ecrit dans l'application au premier plan de la machine"]
    fn injection_reelle() {
        let texte = "Dictum : é à ç ù œ « » 한 🙂 fin.";
        println!("injection dans 3 secondes, placez le curseur ou vous voulez le texte");
        std::thread::sleep(std::time::Duration::from_secs(3));

        // ⛔ **`SendInput` rend un succes meme quand Windows jette les evenements.** Constate le
        // 2026-09-17 : trois essais de suite ont rendu un test VERT alors que la session s'etait
        // reverrouillee et que rien n'etait arrive nulle part. Un test qui passe sans avoir rien
        // ecrit est exactement le garde-fou qui ment, donc on refuse de conclure quand la cible
        // ne peut pas recevoir de saisie.
        let cible = fenetre_au_premier_plan();
        println!("fenetre visee : {cible}");
        assert!(
            !cible.is_empty() && !cible.contains("Lock Screen") && !cible.contains("verrouillage"),
            "cible « {cible} » : la saisie serait jetee sans erreur. Sous Windows, deverrouiller \
             la session et placer le curseur dans une application ordinaire ; sous Linux, poser \
             DISPLAY sur un serveur d'affichage joignable."
        );

        ecrire(texte, 2).expect("l'injection doit reussir");
        println!("injecte : {texte}");
    }

    /// Le titre de la fenetre qui a le focus, pour le diagnostic du test ci-dessus.
    #[cfg(windows)]
    fn fenetre_au_premier_plan() -> String {
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};
        let mut tampon = [0u16; 256];
        // SAFETY : `tampon` est un tableau valide et sa longueur est passee telle quelle.
        let lus = unsafe { GetWindowTextW(GetForegroundWindow(), &mut tampon) };
        String::from_utf16_lossy(&tampon[..lus.max(0) as usize])
    }

    /// L'equivalent Linux, et il ne nomme pas une fenetre mais une SESSION.
    ///
    /// ⛔ **Le risque n'est pas le meme des deux cotes, donc la sonde non plus.** Sous Windows la
    /// saisie part dans le vide quand une fenetre ne peut pas la recevoir, d'ou la lecture du
    /// titre. Sous Linux elle part dans le vide quand il n'y a **aucun serveur d'affichage**, ce
    /// qui est le cas ordinaire d'une machine de compilation, ou quand la session est un Wayland
    /// sans portail `RemoteDesktop`. Une chaine vide fait echouer le test plutot que de le laisser
    /// passer sur une machine ou rien ne pouvait arriver.
    #[cfg(target_os = "linux")]
    fn fenetre_au_premier_plan() -> String {
        let x11 = std::env::var("DISPLAY").unwrap_or_default();
        let wayland = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
        if x11.is_empty() && wayland.is_empty() {
            // Aucun affichage : le test doit refuser de conclure.
            return String::new();
        }
        let type_session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "inconnu".into());
        format!("session {type_session}, DISPLAY « {x11} », WAYLAND_DISPLAY « {wayland} »")
    }
}
