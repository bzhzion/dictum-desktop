//! Le protocole d'appairage, et la decision qui l'accompagne.
//!
//! ⚠️ **Separe de `reseau.rs` a dessein.** Celui-la porte les primitives (empreintes, code visuel,
//! certificat), celui-ci porte les messages et **la decision d'autoriser ou non**. Les melanger
//! aurait donne un fichier ou la regle de securite se cherche entre deux fonctions utilitaires.
//!
//! ⛔ **Toute la decision vit dans `decider`, une fonction PURE.** C'est delibere : une regle
//! d'autorisation enfouie dans une boucle asynchrone ne se teste qu'avec un vrai reseau et un vrai
//! telephone, donc en pratique ne se teste pas. Ici elle se prouve par des tests, y compris les
//! cas qu'on n'a pas envie de fabriquer a la main — mauvaise version, appareil revoque, silence de
//! l'utilisateur.

use serde::{Deserialize, Serialize};

use crate::reseau::{Appaires, code_visuel, empreinte};

/// Version du protocole, comparee STRICTEMENT.
///
/// ⛔ **Refusee avant meme la demande d'autorisation**, motif repris de `justmakeQ`. Laisser entrer
/// un client d'une autre version puis decouvrir qu'il se comporte de travers, c'est avoir deja
/// demande a l'utilisateur d'autoriser quelque chose qu'on ne sait pas interpreter.
pub const VERSION_PROTOCOLE: &str = "1";

/// Ce que le telephone envoie en premier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bonjour {
    pub version: String,
    /// Nom affiche. ⚠️ Declare par le client, donc **jamais** utilise pour autoriser.
    pub nom: String,
    /// Cle publique du telephone, encodee. C'est elle qui l'identifie.
    pub cle_publique: String,
}

/// Ce que l'hote repond, et qui resume la decision prise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reponse {
    /// Appareil deja appaire : la session s'ouvre sans rien demander a personne.
    Bienvenue { empreinte: String },
    /// Appareil inconnu : l'utilisateur doit accepter SUR L'ORDINATEUR, avec ce code sous les yeux.
    AutorisationDemandee { code: String },
    /// Refus, avec sa raison. ⚠️ La raison est destinee a l'UTILISATEUR, pas au client : elle doit
    /// nommer ce qu'il peut faire, pas ce que le serveur a decide.
    Refus { motif: Motif },
}

/// Pourquoi une connexion a ete refusee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motif {
    /// Le telephone et l'ordinateur ne parlent pas la meme version.
    VersionIncompatible,
    /// Le premier message n'est pas conforme, ou dépasse les bornes.
    MessageInvalide,
    /// Personne n'a repondu a la demande d'autorisation.
    ///
    /// ⛔ **Le silence REFUSE.** Une demande qui expire en autorisant serait un appairage qu'on
    /// obtient en attendant que l'utilisateur s'eloigne de son clavier.
    SansReponse,
    /// L'utilisateur a dit non.
    Refuse,
}

/// Ce que l'hote sait au moment de decider.
pub struct Contexte<'a> {
    pub appaires: &'a Appaires,
    pub defi: &'a [u8],
}

/// Ce que l'utilisateur a repondu, quand on a eu besoin de le lui demander.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consentement {
    /// Personne n'a encore ete sollicite : l'appareil etait deja connu.
    NonSollicite,
    Accepte,
    Refuse,
    /// Le delai est passe sans reponse.
    Expire,
}

/// Bornes sur le premier message.
///
/// ⛔ **Un champ non borne est une facon de faire tomber le serveur avant toute authentification.**
/// Les longueurs viennent de l'usage : un nom d'appareil tient largement en 100 caracteres, une
/// cle publique encodee en 1000.
const NOM_MAX: usize = 100;
const CLE_MAX: usize = 1000;

/// La decision, et rien d'autre.
///
/// ⚠️ **Pure** : pas d'entree/sortie, pas d'horloge, pas de reseau. C'est ce qui la rend
/// testable — et c'est la seule raison pour laquelle les cas penibles (version fausse, silence de
/// l'utilisateur, appareil revoque entre-temps) sont couverts.
pub fn decider(bonjour: &Bonjour, contexte: &Contexte, consentement: Consentement) -> Reponse {
    // ⛔ La version AVANT tout le reste : on ne demande pas a l'utilisateur d'autoriser un client
    // qu'on ne saura pas interpreter ensuite.
    if bonjour.version != VERSION_PROTOCOLE {
        return Reponse::Refus {
            motif: Motif::VersionIncompatible,
        };
    }
    if bonjour.nom.is_empty()
        || bonjour.nom.len() > NOM_MAX
        || bonjour.cle_publique.is_empty()
        || bonjour.cle_publique.len() > CLE_MAX
    {
        return Reponse::Refus {
            motif: Motif::MessageInvalide,
        };
    }

    let empreinte_client = empreinte(bonjour.cle_publique.as_bytes());

    // Deja connu : rien a demander, et surtout rien a redemander a chaque connexion.
    if contexte.appaires.autorise(&empreinte_client) {
        return Reponse::Bienvenue {
            empreinte: empreinte_client,
        };
    }

    match consentement {
        Consentement::NonSollicite => Reponse::AutorisationDemandee {
            code: code_visuel(&empreinte_client, contexte.defi),
        },
        Consentement::Accepte => Reponse::Bienvenue {
            empreinte: empreinte_client,
        },
        Consentement::Refuse => Reponse::Refus {
            motif: Motif::Refuse,
        },
        // ⛔ Le silence refuse. Voir `Motif::SansReponse`.
        Consentement::Expire => Reponse::Refus {
            motif: Motif::SansReponse,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reseau::AppareilAppaire;

    fn bonjour(nom: &str, cle: &str) -> Bonjour {
        Bonjour {
            version: VERSION_PROTOCOLE.to_string(),
            nom: nom.to_string(),
            cle_publique: cle.to_string(),
        }
    }

    fn contexte<'a>(appaires: &'a Appaires, defi: &'a [u8]) -> Contexte<'a> {
        Contexte { appaires, defi }
    }

    #[test]
    fn une_mauvaise_version_est_refusee_avant_de_demander_quoi_que_ce_soit() {
        let vides = Appaires::default();
        let mut b = bonjour("iPhone", "cle");
        b.version = "0".into();
        let r = decider(&b, &contexte(&vides, b"defi"), Consentement::NonSollicite);
        // ⛔ Le point du test : PAS d'AutorisationDemandee. Sinon on aurait sollicite
        // l'utilisateur pour un client qu'on ne sait pas interpreter.
        assert_eq!(
            r,
            Reponse::Refus {
                motif: Motif::VersionIncompatible
            }
        );
    }

    #[test]
    fn un_champ_hors_bornes_est_refuse() {
        let vides = Appaires::default();
        for b in [
            bonjour("", "cle"),
            bonjour(&"x".repeat(NOM_MAX + 1), "cle"),
            bonjour("iPhone", ""),
            bonjour("iPhone", &"x".repeat(CLE_MAX + 1)),
        ] {
            let r = decider(&b, &contexte(&vides, b"defi"), Consentement::NonSollicite);
            assert_eq!(
                r,
                Reponse::Refus {
                    motif: Motif::MessageInvalide
                },
                "un champ non borne fait tomber le serveur avant toute authentification"
            );
        }
    }

    #[test]
    fn un_inconnu_declenche_une_demande_avec_son_code() {
        let vides = Appaires::default();
        let b = bonjour("iPhone", "cle");
        let r = decider(&b, &contexte(&vides, b"defi"), Consentement::NonSollicite);
        match r {
            Reponse::AutorisationDemandee { code } => {
                assert_eq!(code.len(), 4);
                assert_eq!(code, code_visuel(&empreinte(b"cle"), b"defi"));
            }
            autre => panic!("attendu une demande, obtenu {autre:?}"),
        }
    }

    #[test]
    fn un_appareil_deja_appaire_entre_sans_rien_demander() {
        let mut liste = Appaires::default();
        liste.ajouter(AppareilAppaire {
            empreinte: empreinte(b"cle"),
            nom: "iPhone".into(),
            appaire_le: "2026-09-25T20:00:00Z".into(),
        });
        let b = bonjour("iPhone", "cle");
        let r = decider(&b, &contexte(&liste, b"defi"), Consentement::NonSollicite);
        assert_eq!(
            r,
            Reponse::Bienvenue {
                empreinte: empreinte(b"cle")
            }
        );
    }

    #[test]
    fn un_appareil_revoque_redevient_un_inconnu() {
        let mut liste = Appaires::default();
        liste.ajouter(AppareilAppaire {
            empreinte: empreinte(b"cle"),
            nom: "iPhone".into(),
            appaire_le: "2026-09-25T20:00:00Z".into(),
        });
        liste.revoquer(&empreinte(b"cle"));
        let b = bonjour("iPhone", "cle");
        let r = decider(&b, &contexte(&liste, b"defi"), Consentement::NonSollicite);
        // ⛔ Une revocation qui laisserait entrer serait pire que pas de revocation du tout :
        // l'utilisateur croirait avoir coupe l'acces.
        assert!(matches!(r, Reponse::AutorisationDemandee { .. }));
    }

    #[test]
    fn le_silence_refuse() {
        let vides = Appaires::default();
        let b = bonjour("iPhone", "cle");
        let r = decider(&b, &contexte(&vides, b"defi"), Consentement::Expire);
        // ⛔ Sinon l'appairage s'obtient en attendant que l'utilisateur s'eloigne de son clavier.
        assert_eq!(
            r,
            Reponse::Refus {
                motif: Motif::SansReponse
            }
        );
    }

    #[test]
    fn un_refus_de_l_utilisateur_est_un_refus() {
        let vides = Appaires::default();
        let b = bonjour("iPhone", "cle");
        assert_eq!(
            decider(&b, &contexte(&vides, b"defi"), Consentement::Refuse),
            Reponse::Refus {
                motif: Motif::Refuse
            }
        );
    }

    #[test]
    fn le_nom_declare_n_ouvre_aucune_porte() {
        // Un appareil est appaire. Un autre se presente avec EXACTEMENT le meme nom, mais une
        // autre cle : il doit rester un inconnu. C'est le defaut de justmakeQ qu'on refuse.
        let mut liste = Appaires::default();
        liste.ajouter(AppareilAppaire {
            empreinte: empreinte(b"vraie-cle"),
            nom: "iPhone de painteau".into(),
            appaire_le: "2026-09-25T20:00:00Z".into(),
        });
        let imposteur = bonjour("iPhone de painteau", "autre-cle");
        let r = decider(
            &imposteur,
            &contexte(&liste, b"defi"),
            Consentement::NonSollicite,
        );
        assert!(
            matches!(r, Reponse::AutorisationDemandee { .. }),
            "un nom identique ne doit jamais suffire a entrer"
        );
    }
}
