//! Appairage d'un telephone avec cet ordinateur.
//!
//! ⛔ **Le pire cas de ce module n'est pas une panne, c'est quelqu'un qui tape du texte arbitraire
//! dans l'ordinateur de l'utilisateur**, et sa voix en clair sur le wifi. Tout ce qui suit decoule
//! de ca, et pas d'un gout pour la ceinture et les bretelles.
//!
//! Le protocole de coworking de `justmakeQ` sert de base — il resout deja « un telephone pilote une
//! application de bureau » des deux cotes — mais **trois de ses choix sont refuses ici**, parce que
//! son pire cas a lui est de deranger une conduite de spectacle :
//!
//! | Chez justmakeQ | Ici |
//! |---|---|
//! | `ws://` en clair | TLS, certificat epingle a l'appairage |
//! | l'identite est une chaine que le client DECLARE lui-meme | empreinte d'une cle publique |
//! | aucun nombre a faire correspondre | quatre chiffres affiches des deux cotes |
//!
//! ⛔ **Le deuxieme est le plus grave** : une permission memorisee sur une valeur auto-declaree
//! signifie qu'une fois un appareil autorise, **n'importe qui peut se faire passer pour lui** en
//! renvoyant la meme valeur, sans jamais redeclencher de demande.
//!
//! ⚠️ Ce module ne contient volontairement **aucun serveur** pour l'instant : il porte les
//! decisions qui se testent sans reseau. Le serveur viendra dessus, pas l'inverse — c'est ce qui
//! permet de prouver ces regles par des tests plutot que par un appareil sous la main.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use rand::RngCore;
use sha2::{Digest, Sha256};

/// Port d'ecoute. Fixe et non configurable : la decouverte annonce le port, et un port que
/// l'utilisateur choisit est un reglage de plus a expliquer pour aucun gain.
pub const PORT: u16 = 44881;

/// Duree de validite d'une demande d'appairage.
///
/// ⛔ Passe ce delai la demande est REFUSEE, jamais acceptee par lassitude. Une demande qui expire
/// en autorisant serait exactement le defaut qu'on cherche a eviter.
pub const DELAI_APPAIRAGE_S: u64 = 60;

/// Ou l'ordinateur doit ecouter, selon les reglages.
///
/// ⛔ **Boucle locale par defaut, toutes interfaces jamais sans geste delibere.** Regle deja
/// appliquee a BeamMeUp et a Hublot dans le parc, et elle vaut encore plus ici : ce serveur fait
/// ecrire du texte dans les applications de l'utilisateur.
///
/// ⚠️ La boucle locale ne sert evidemment a rien pour un telephone, et c'est VOULU : tant que
/// l'utilisateur n'a pas dit oui, le service existe sans etre joignable. Le defaut protege celui
/// qui n'a rien demande, pas celui qui s'en sert.
pub fn adresse_ecoute(toutes_interfaces: bool) -> SocketAddr {
    let ip = if toutes_interfaces {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    };
    SocketAddr::new(ip, PORT)
}

/// Empreinte d'une cle publique, telle que l'hote la retient.
///
/// ⛔ **C'est ca, l'identite d'un appareil appaire** : une empreinte de ce qu'il a PROUVE posseder,
/// et jamais un nom qu'il affirme. Un nom sert a l'afficher dans la liste, pas a l'autoriser.
pub fn empreinte(cle_publique: &[u8]) -> String {
    let resume = Sha256::digest(cle_publique);
    resume.iter().map(|o| format!("{o:02x}")).collect()
}

/// Comparaison a temps constant de deux empreintes.
///
/// ⚠️ Ecrite a la main plutot que tiree d'une caisse, pour trois lignes : le volume ne justifie pas
/// une dependance de plus. ⛔ **Mais elle doit rester a temps constant** — un `==` sur des chaines
/// s'arrete au premier octet different, ce qui laisse mesurer combien de tete est juste.
pub fn empreintes_egales(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut ecart = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        ecart |= x ^ y;
    }
    ecart == 0
}

/// Les quatre chiffres a faire correspondre, derives du materiel d'appairage.
///
/// ⛔ **Ils ne sont pas decoratifs.** Sans eux, un voisin sur le meme wifi qui lance une demande au
/// meme moment peut se faire accepter A LA PLACE du bon telephone : l'utilisateur voit une demande,
/// il attend une demande, il dit oui. Le nombre est ce qui lie la demande affichee a l'appareil
/// qu'on a en main.
///
/// ⚠️ **Derives et non tires au hasard separement** : les deux cotes doivent afficher le meme sans
/// se l'echanger en clair, donc ils le calculent chacun a partir de ce qu'ils connaissent tous les
/// deux. Un nombre transmis serait un nombre qu'un tiers peut lire, donc rejouer.
pub fn code_visuel(empreinte_client: &str, defi_hote: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(empreinte_client.as_bytes());
    h.update(defi_hote);
    let resume = h.finalize();
    // 4 chiffres depuis les deux premiers octets. Le modulo introduit un biais negligeable ici :
    // ce code n'est pas un secret, il lie une demande a un appareil pendant une minute.
    let valeur = u16::from(resume[0]) << 8 | u16::from(resume[1]);
    format!("{:04}", valeur % 10_000)
}

/// Un defi d'appairage, tire a chaque demande.
///
/// ⛔ **Doit venir d'un generateur CRYPTOGRAPHIQUE.** Un defi previsible laisse calculer a l'avance
/// le code a quatre chiffres qui sera affiche, donc preparer une demande qui « tombe juste » —
/// exactement ce que ce code est cense empecher.
pub fn defi() -> [u8; 32] {
    let mut octets = [0u8; 32];
    rand::rng().fill_bytes(&mut octets);
    octets
}

/// Ou vit le certificat auto-signe de cette machine.
pub fn chemin_certificat() -> Result<PathBuf, String> {
    Ok(crate::chemins::configuration()?.join("reseau-certificat.pem"))
}

/// Ou vit sa cle privee.
///
/// ⚠️ **Fichier distinct du certificat**, pour que la cle ne parte jamais par accident avec lui :
/// le certificat s'affiche, se compare, se montre dans un QR ; la cle ne sort jamais de la machine.
pub fn chemin_cle() -> Result<PathBuf, String> {
    Ok(crate::chemins::configuration()?.join("reseau-cle.pem"))
}

/// Le certificat et sa cle, au format PEM.
pub struct IdentiteTls {
    pub certificat_pem: String,
    pub cle_pem: String,
}

/// Fabrique une identite TLS auto-signee pour cette machine.
///
/// ⛔ **Auto-signe et EPINGLE, ce n'est pas un pis-aller.** Aucune autorite publique ne peut signer
/// un certificat pour une adresse de reseau local qui change d'un wifi a l'autre. Ce qui protege
/// ici n'est pas une chaine de confiance, c'est que le telephone retienne **cette** empreinte au
/// moment de l'appairage et refuse toute autre ensuite.
///
/// ⚠️ **Les noms sont volontairement pauvres** : ce certificat n'identifie pas un domaine, il porte
/// une cle. Y mettre le nom de la machine ferait fuiter le nom de l'ordinateur de l'utilisateur a
/// quiconque sonde le port.
pub fn fabriquer_identite_tls() -> Result<IdentiteTls, String> {
    let certifie = rcgen::generate_simple_self_signed(vec!["oyant.local".to_string()])
        .map_err(|erreur| format!("Certificat impossible à générer : {erreur}"))?;
    Ok(IdentiteTls {
        certificat_pem: certifie.cert.pem(),
        cle_pem: certifie.key_pair.serialize_pem(),
    })
}

/// Charge l'identite TLS, en la fabriquant au premier appel.
///
/// ⚠️ **Idempotent** : deux appels rendent la meme identite, sans quoi chaque redemarrage
/// invaliderait tous les appairages deja faits — le telephone a epingle une empreinte, pas un
/// serveur.
pub fn identite_tls() -> Result<IdentiteTls, String> {
    let (c, k) = (chemin_certificat()?, chemin_cle()?);
    if c.is_file() && k.is_file() {
        return Ok(IdentiteTls {
            certificat_pem: std::fs::read_to_string(&c)
                .map_err(|e| format!("Certificat illisible : {e}"))?,
            cle_pem: std::fs::read_to_string(&k).map_err(|e| format!("Clé illisible : {e}"))?,
        });
    }
    let identite = fabriquer_identite_tls()?;
    if let Some(parent) = c.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Répertoire impossible : {e}"))?;
    }
    std::fs::write(&c, &identite.certificat_pem)
        .map_err(|e| format!("Certificat non écrit : {e}"))?;
    std::fs::write(&k, &identite.cle_pem).map_err(|e| format!("Clé non écrite : {e}"))?;
    Ok(identite)
}

/// Empreinte du certificat, celle que le telephone epingle.
///
/// ⚠️ Calculee sur le PEM tel qu'il est servi, pour que les deux cotes comparent la meme chose.
pub fn empreinte_certificat(certificat_pem: &str) -> String {
    empreinte(certificat_pem.as_bytes())
}

/// Un appareil autorise, tel qu'il est retenu sur l'ordinateur.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppareilAppaire {
    /// Empreinte de la cle publique : la seule chose qui autorise.
    pub empreinte: String,
    /// Nom affiche, declare par l'appareil. ⚠️ **Ne sert QU'A l'affichage** : il est choisi par le
    /// telephone, donc il ne prouve rien et ne doit jamais entrer dans une decision.
    pub nom: String,
    /// Horodatage ISO de l'appairage, pour que l'utilisateur reconnaisse ce qu'il revoque.
    pub appaire_le: String,
}

/// La liste des appareils autorises, avec revocation.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Appaires {
    pub appareils: Vec<AppareilAppaire>,
}

impl Appaires {
    /// Cet appareil est-il autorise ?
    ///
    /// ⚠️ Compare l'EMPREINTE et rien d'autre. Un appareil qui change de nom reste le meme ; deux
    /// appareils qui se declarent du meme nom restent differents.
    pub fn autorise(&self, empreinte_presentee: &str) -> bool {
        self.appareils
            .iter()
            .any(|a| empreintes_egales(&a.empreinte, empreinte_presentee))
    }

    /// Ajoute un appareil, ou met a jour son nom s'il etait deja connu.
    ///
    /// ⛔ **Un reappairage ne duplique pas** : sans ca, la liste de revocation se remplit de
    /// doublons et revoquer l'un laisse l'autre autoriser, ce qui est pire que ne rien revoquer.
    pub fn ajouter(&mut self, appareil: AppareilAppaire) {
        if let Some(existant) = self
            .appareils
            .iter_mut()
            .find(|a| empreintes_egales(&a.empreinte, &appareil.empreinte))
        {
            existant.nom = appareil.nom;
            return;
        }
        self.appareils.push(appareil);
    }

    /// Retire un appareil. Rend `true` si quelque chose a ete retire.
    pub fn revoquer(&mut self, empreinte_a_retirer: &str) -> bool {
        let avant = self.appareils.len();
        self.appareils
            .retain(|a| !empreintes_egales(&a.empreinte, empreinte_a_retirer));
        self.appareils.len() != avant
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecoute_sur_la_boucle_locale_par_defaut() {
        let adresse = adresse_ecoute(false);
        assert!(
            adresse.ip().is_loopback(),
            "le defaut doit rester injoignable depuis le reseau"
        );
        assert_eq!(adresse.port(), PORT);
    }

    #[test]
    fn toutes_interfaces_seulement_sur_demande() {
        let adresse = adresse_ecoute(true);
        assert!(!adresse.ip().is_loopback());
    }

    #[test]
    fn empreinte_change_avec_la_cle() {
        assert_ne!(empreinte(b"cle-a"), empreinte(b"cle-b"));
        assert_eq!(empreinte(b"cle-a"), empreinte(b"cle-a"));
        // 32 octets en hexadecimal.
        assert_eq!(empreinte(b"cle-a").len(), 64);
    }

    #[test]
    fn comparaison_d_empreintes() {
        let a = empreinte(b"cle-a");
        assert!(empreintes_egales(&a, &a));
        assert!(!empreintes_egales(&a, &empreinte(b"cle-b")));
        // Longueurs differentes : refuse sans paniquer.
        assert!(!empreintes_egales(&a, "court"));
    }

    #[test]
    fn le_code_visuel_fait_quatre_chiffres() {
        let code = code_visuel(&empreinte(b"cle"), b"defi");
        assert_eq!(code.len(), 4);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn le_code_visuel_lie_l_appareil_au_defi() {
        let defi = b"defi";
        let a = code_visuel(&empreinte(b"telephone-a"), defi);
        let b = code_visuel(&empreinte(b"telephone-b"), defi);
        // C'est TOUT l'interet : deux appareils qui demandent en meme temps n'affichent pas le
        // meme nombre, donc l'utilisateur ne peut pas accepter l'un en croyant accepter l'autre.
        assert_ne!(
            a, b,
            "deux appareils differents doivent donner des codes differents"
        );
        // Et le meme appareil sur un autre defi donne autre chose : un code vu une fois ne se
        // rejoue pas.
        assert_ne!(a, code_visuel(&empreinte(b"telephone-a"), b"autre-defi"));
    }

    #[test]
    fn deux_defis_ne_sont_jamais_les_memes() {
        // ⛔ Un defi previsible laisse calculer a l'avance le code a quatre chiffres, donc preparer
        // une demande qui « tombe juste ». C'est le seul test qui protege cette propriete.
        let (a, b) = (defi(), defi());
        assert_ne!(a, b);
        assert_ne!(
            a, [0u8; 32],
            "un defi nul trahirait un generateur non initialise"
        );
    }

    #[test]
    fn le_certificat_est_utilisable_et_son_empreinte_stable() {
        let identite = fabriquer_identite_tls().expect("génération");
        assert!(identite.certificat_pem.contains("BEGIN CERTIFICATE"));
        assert!(!identite.cle_pem.is_empty());
        // ⚠️ L'empreinte doit etre stable pour un meme certificat : c'est elle que le telephone
        // epingle, donc la recalculer doit redonner la meme valeur.
        let e1 = empreinte_certificat(&identite.certificat_pem);
        assert_eq!(e1, empreinte_certificat(&identite.certificat_pem));
        // Et deux machines differentes ne doivent pas se ressembler.
        let autre = fabriquer_identite_tls().expect("génération");
        assert_ne!(e1, empreinte_certificat(&autre.certificat_pem));
    }

    #[test]
    fn la_cle_privee_ne_vit_pas_avec_le_certificat() {
        // ⛔ Deux fichiers distincts, pour que la cle ne parte jamais par accident avec le
        // certificat — lui s'affiche, se compare, se montre dans un QR.
        let (c, k) = (chemin_certificat(), chemin_cle());
        if let (Ok(c), Ok(k)) = (c, k) {
            assert_ne!(c, k);
        }
    }

    #[test]
    fn un_appareil_inconnu_n_est_pas_autorise() {
        let liste = Appaires::default();
        assert!(!liste.autorise(&empreinte(b"inconnu")));
    }

    #[test]
    fn appairer_puis_revoquer() {
        let mut liste = Appaires::default();
        let e = empreinte(b"telephone");
        liste.ajouter(AppareilAppaire {
            empreinte: e.clone(),
            nom: "iPhone de painteau".into(),
            appaire_le: "2026-09-25T20:00:00Z".into(),
        });
        assert!(liste.autorise(&e));
        assert!(liste.revoquer(&e));
        assert!(
            !liste.autorise(&e),
            "une revocation doit vraiment retirer l'acces"
        );
        // Revoquer deux fois ne ment pas sur ce qui s'est passe.
        assert!(!liste.revoquer(&e));
    }

    #[test]
    fn reappairer_ne_duplique_pas() {
        let mut liste = Appaires::default();
        let e = empreinte(b"telephone");
        for nom in ["Ancien nom", "Nouveau nom"] {
            liste.ajouter(AppareilAppaire {
                empreinte: e.clone(),
                nom: nom.into(),
                appaire_le: "2026-09-25T20:00:00Z".into(),
            });
        }
        assert_eq!(
            liste.appareils.len(),
            1,
            "un doublon rendrait la revocation inoperante"
        );
        assert_eq!(liste.appareils[0].nom, "Nouveau nom");
        // Et surtout : une seule revocation suffit a couper l'acces.
        assert!(liste.revoquer(&e));
        assert!(!liste.autorise(&e));
    }

    #[test]
    fn le_nom_n_autorise_jamais() {
        let mut liste = Appaires::default();
        liste.ajouter(AppareilAppaire {
            empreinte: empreinte(b"vrai-telephone"),
            nom: "iPhone de painteau".into(),
            appaire_le: "2026-09-25T20:00:00Z".into(),
        });
        // Un imposteur qui se declare du meme nom presente une autre cle : il reste dehors.
        // C'est precisement le defaut de justmakeQ qu'on refuse de reprendre.
        assert!(!liste.autorise(&empreinte(b"imposteur")));
    }
}
