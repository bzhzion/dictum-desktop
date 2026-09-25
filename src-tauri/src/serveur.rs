//! Le serveur d'appairage : TLS, WebSocket, et la poignee de main.
//!
//! ⚠️ **Il ne DECIDE rien.** Toute la regle d'autorisation vit dans `appairage::decider`, qui est
//! pure et testee a part. Ce module transporte, borne et journalise ; melanger les deux aurait
//! remis la securite dans une boucle asynchrone, ou elle ne se teste qu'avec un vrai telephone.
//!
//! ⛔ **Le consentement passe par un CANAL, pas par un rappel.** L'hote envoie la demande a qui
//! sait afficher quelque chose (l'interface, ou un test), et attend une reponse sur un canal a
//! usage unique. C'est ce qui permet d'eprouver le serveur **sans appareil** : un test joue le role
//! de l'utilisateur, y compris quand celui-ci ne repond pas.

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_rustls::TlsAcceptor;

use crate::appairage::{Bonjour, Consentement, Contexte, Motif, Reponse, decider};
use crate::reseau::{Appaires, AppareilAppaire, DELAI_APPAIRAGE_S, IdentiteTls, defi};

/// Ce que l'hote demande a l'utilisateur d'accepter.
#[derive(Debug)]
pub struct DemandeAppairage {
    /// Nom declare par l'appareil. ⚠️ Sert a l'affichage et a rien d'autre.
    pub nom: String,
    /// Les quatre chiffres que l'utilisateur doit retrouver sur son telephone.
    pub code: String,
    /// Par ou repondre. ⛔ Si ce canal est abandonne, la demande est traitee comme un REFUS.
    pub reponse: oneshot::Sender<bool>,
}

/// Construit la configuration TLS a partir de notre identite auto-signee.
fn config_tls(identite: &IdentiteTls) -> Result<Arc<rustls::ServerConfig>, String> {
    let certificats = rustls_pemfile::certs(&mut identite.certificat_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>();
    let certificats = certificats.map_err(|e| format!("Certificat illisible : {e}"))?;
    let cle = rustls_pemfile::private_key(&mut identite.cle_pem.as_bytes())
        .map_err(|e| format!("Clé illisible : {e}"))?
        .ok_or("Clé privée absente du PEM.")?;
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificats, cle)
        .map_err(|e| format!("Configuration TLS refusée : {e}"))?;
    Ok(Arc::new(config))
}

/// Sert les connexions jusqu'a ce que le port soit ferme.
///
/// ⚠️ Rend l'adresse REELLEMENT ecoutee, qui n'est pas forcement celle demandee : avec un port 0
/// le systeme en choisit un. Le tests s'en servent, et ca evite la course « lire le port avant
/// qu'il soit ouvert ».
pub async fn servir(
    adresse: std::net::SocketAddr,
    identite: IdentiteTls,
    appaires: Arc<Mutex<Appaires>>,
    demandes: mpsc::Sender<DemandeAppairage>,
) -> Result<(std::net::SocketAddr, tokio::task::JoinHandle<()>), String> {
    let acceptateur = TlsAcceptor::from(config_tls(&identite)?);
    let ecoute = TcpListener::bind(adresse)
        .await
        .map_err(|e| format!("Port {} indisponible : {e}", adresse.port()))?;
    let reelle = ecoute
        .local_addr()
        .map_err(|e| format!("Adresse d'écoute illisible : {e}"))?;

    let tache = tokio::spawn(async move {
        loop {
            let Ok((flux, _)) = ecoute.accept().await else {
                continue;
            };
            let acceptateur = acceptateur.clone();
            let appaires = Arc::clone(&appaires);
            let demandes = demandes.clone();
            // Une connexion qui se comporte mal ne doit jamais empecher les suivantes.
            tokio::spawn(async move {
                let _ = servir_une_connexion(flux, acceptateur, appaires, demandes).await;
            });
        }
    });
    Ok((reelle, tache))
}

async fn servir_une_connexion(
    flux: tokio::net::TcpStream,
    acceptateur: TlsAcceptor,
    appaires: Arc<Mutex<Appaires>>,
    demandes: mpsc::Sender<DemandeAppairage>,
) -> Result<(), String> {
    let flux = acceptateur
        .accept(flux)
        .await
        .map_err(|e| format!("Poignée de main TLS échouée : {e}"))?;
    let mut ws = tokio_tungstenite::accept_async(flux)
        .await
        .map_err(|e| format!("WebSocket refusé : {e}"))?;

    // ⛔ Delai court sur le PREMIER message : une connexion qui s'ouvre et se tait immobilise une
    // ressource sans jamais s'authentifier. Motif repris de justmakeQ.
    let premier = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        futures_util::StreamExt::next(&mut ws),
    )
    .await
    .map_err(|_| "Premier message jamais arrivé.".to_string())?
    .ok_or("Connexion fermée avant le premier message.")?
    .map_err(|e| format!("Premier message illisible : {e}"))?;

    let texte = premier
        .into_text()
        .map_err(|e| format!("Message non textuel : {e}"))?;
    let bonjour: Bonjour = match serde_json::from_str(&texte) {
        Ok(b) => b,
        Err(_) => {
            repondre(
                &mut ws,
                &Reponse::Refus {
                    motif: Motif::MessageInvalide,
                },
            )
            .await;
            return Ok(());
        }
    };

    let defi_courant = defi();

    // Premiere passe : l'appareil est-il deja connu, ou faut-il demander ?
    let premiere = {
        let liste = appaires.lock().await;
        decider(
            &bonjour,
            &Contexte {
                appaires: &liste,
                defi: &defi_courant,
            },
            Consentement::NonSollicite,
        )
    };

    let finale = match &premiere {
        // Rien a demander : deja connu, ou deja refuse (version, bornes).
        Reponse::Bienvenue { .. } | Reponse::Refus { .. } => premiere,
        Reponse::AutorisationDemandee { code } => {
            let consentement = demander(&demandes, &bonjour.nom, code).await;
            let liste = appaires.lock().await;
            decider(
                &bonjour,
                &Contexte {
                    appaires: &liste,
                    defi: &defi_courant,
                },
                consentement,
            )
        }
    };

    // Un appairage accepte se retient, sinon l'utilisateur devrait redire oui a chaque connexion.
    if let Reponse::Bienvenue { empreinte } = &finale {
        let mut liste = appaires.lock().await;
        if !liste.autorise(empreinte) {
            liste.ajouter(AppareilAppaire {
                empreinte: empreinte.clone(),
                nom: bonjour.nom.clone(),
                appaire_le: horodatage(),
            });
        }
    }

    repondre(&mut ws, &finale).await;
    Ok(())
}

/// Demande a l'utilisateur, et traite tout ce qui n'est pas un « oui » franc comme un refus.
///
/// ⛔ **Trois facons de ne pas dire oui, une seule conclusion.** Le delai expire, le canal ferme
/// parce que l'interface a disparu, ou l'utilisateur dit non : dans les trois cas on refuse. Une
/// seule branche qui autoriserait par defaut suffirait a rendre tout le reste decoratif.
async fn demander(
    demandes: &mpsc::Sender<DemandeAppairage>,
    nom: &str,
    code: &str,
) -> Consentement {
    let (repondeur, attente) = oneshot::channel();
    let demande = DemandeAppairage {
        nom: nom.to_string(),
        code: code.to_string(),
        reponse: repondeur,
    };
    if demandes.send(demande).await.is_err() {
        // Personne n'ecoute : il n'y a pas d'interface pour afficher la demande.
        return Consentement::Expire;
    }
    match tokio::time::timeout(std::time::Duration::from_secs(DELAI_APPAIRAGE_S), attente).await {
        Ok(Ok(true)) => Consentement::Accepte,
        Ok(Ok(false)) => Consentement::Refuse,
        // Canal abandonne : l'interface s'est fermee sans repondre.
        Ok(Err(_)) => Consentement::Expire,
        Err(_) => Consentement::Expire,
    }
}

async fn repondre<S>(ws: &mut tokio_tungstenite::WebSocketStream<S>, reponse: &Reponse)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    if let Ok(texte) = serde_json::to_string(reponse) {
        let _ =
            futures_util::SinkExt::send(ws, tokio_tungstenite::tungstenite::Message::Text(texte))
                .await;
    }
}

/// Demarre le serveur si, et seulement si, l'utilisateur l'a demande.
///
/// ⛔ **Le filtre est ICI, au demarrage, et pas dans une case de l'interface.** Un serveur qu'on
/// ouvre puis qu'on « protege » plus loin est un serveur ouvert. Tant que `reseau_actif` est faux,
/// aucun port n'est lie.
///
/// ⚠️ Ne rend jamais d'erreur bloquante : un port indisponible ne doit pas empecher Oyant de
/// dicter. On le dit sur la sortie d'erreur et on continue, comme pour le raccourci global.
pub fn demarrer_si_demande(app: &tauri::AppHandle) {
    let reglages = crate::reglages::lire_sans_application();
    if !reglages.reseau_actif {
        return;
    }
    let adresse = crate::reseau::adresse_ecoute(reglages.reseau_toutes_interfaces);
    let identite = match crate::reseau::identite_tls() {
        Ok(i) => i,
        Err(message) => {
            eprintln!("appairage : {message}");
            return;
        }
    };
    let appaires = Arc::new(Mutex::new(charger_appaires()));
    let empreinte = match crate::reseau::empreinte_certificat(&identite.certificat_pem) {
        Ok(e) => e,
        Err(message) => {
            eprintln!("appairage : {message}");
            return;
        }
    };
    // Canal des demandes : l'interface les affiche, et repond. ⚠️ Si personne ne le consomme,
    // `demander` conclut au refus — voir sa documentation.
    let (envoi, mut reception) = mpsc::channel::<DemandeAppairage>(4);

    let poignee = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(demande) = reception.recv().await {
            // ⛔ La demande est MISE EN ATTENTE, pas refusee tout de suite : c'est l'ecran qui
            // repond, par `reseau_repondre`. Si personne ne repond, le delai de `demander` finit
            // par conclure au refus — le silence refuse, il n'autorise jamais.
            let identifiant = demande.code.clone();
            en_attente()
                .lock()
                .expect("verrou des demandes")
                .insert(identifiant.clone(), demande.reponse);
            let _ = tauri::Emitter::emit(
                &poignee,
                "appairage-demande",
                serde_json::json!({ "id": identifiant, "nom": demande.nom, "code": demande.code }),
            );
        }
    });

    tauri::async_runtime::spawn(async move {
        match servir(adresse, identite, appaires, envoi).await {
            Ok((reelle, _)) => {
                eprintln!("appairage : à l'écoute sur {reelle}");
                // ⚠️ L'annonce vient APRES l'ecoute : annoncer un port qui n'ecoute pas encore
                // ferait echouer la premiere tentative de connexion, et ce genre d'echec se lit
                // comme « ca ne marche pas » plutot que comme une course.
                match annoncer(reelle.port(), &empreinte) {
                    // Le demon est garde vivant par la tache : le laisser tomber retirerait
                    // l'annonce du reseau aussitot.
                    Ok(demon) => {
                        eprintln!("appairage : annoncé en {TYPE_SERVICE}");
                        std::mem::forget(demon);
                    }
                    Err(message) => eprintln!("appairage : {message}"),
                }
            }
            Err(message) => eprintln!("appairage : {message}"),
        }
    });
}

/// Les demandes qui attendent une reponse de l'ecran, par code.
///
/// ⚠️ Un `Mutex` de la bibliotheque standard et non de tokio : il n'est tenu que le temps d'une
/// insertion ou d'un retrait, jamais a travers un `await`.
fn en_attente()
-> &'static std::sync::Mutex<std::collections::HashMap<String, oneshot::Sender<bool>>> {
    static DEMANDES: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, oneshot::Sender<bool>>>,
    > = std::sync::OnceLock::new();
    DEMANDES.get_or_init(Default::default)
}

/// Reponse de l'utilisateur a une demande d'appairage.
///
/// ⛔ **C'est la seule facon d'autoriser**, et elle part de la machine qui recevra les frappes.
/// Rend `false` si la demande a deja expire : l'ecran ne doit pas laisser croire qu'un appairage
/// a reussi quand le telephone a deja renonce.
#[tauri::command]
pub fn reseau_repondre(id: String, accepte: bool) -> bool {
    let Some(repondeur) = en_attente()
        .lock()
        .expect("verrou des demandes")
        .remove(&id)
    else {
        return false;
    };
    repondeur.send(accepte).is_ok()
}

/// Type de service zeroconf annonce sur le reseau local.
///
/// ⚠️ **Un type DECLARE, et pas une enumeration de tout ce qui passe.** C'est ce qui evite
/// l'autorisation « multicast » d'Apple : `NSBonjourServices` declare ce type, et le telephone n'a
/// alors besoin que de `NSLocalNetworkUsageDescription`. Verifie sur `justmakeq-app`, qui declare
/// les deux et aucune autorisation multicast.
pub const TYPE_SERVICE: &str = "_oyant._tcp.local.";

/// Annonce cet ordinateur sur le reseau local.
///
/// ⛔ **L'empreinte du certificat part dans l'annonce.** C'est elle que le telephone epingle avant
/// meme d'ouvrir la connexion : sans elle il devrait faire confiance au premier certificat
/// presente, ce qui laisserait n'importe qui sur le wifi se placer au milieu du tout premier
/// appairage — le seul moment ou il n'y a encore rien a comparer.
///
/// ⚠️ **L'annonce ne rend pas joignable** : elle dit seulement « je suis la ». Tant que
/// `reseau_toutes_interfaces` est faux, le port reste sur la boucle locale et l'annonce ne mene
/// nulle part. Les deux reglages sont distincts a dessein.
fn annoncer(port: u16, empreinte: &str) -> Result<mdns_sd::ServiceDaemon, String> {
    let demon =
        mdns_sd::ServiceDaemon::new().map_err(|e| format!("Découverte indisponible : {e}"))?;
    let machine = hostname_court();
    let service = mdns_sd::ServiceInfo::new(
        TYPE_SERVICE,
        &machine,
        &format!("{machine}.local."),
        (),
        port,
        &[
            ("empreinte", empreinte),
            ("version", crate::appairage::VERSION_PROTOCOLE),
        ][..],
    )
    .map_err(|e| format!("Annonce impossible : {e}"))?
    .enable_addr_auto();
    demon
        .register(service)
        .map_err(|e| format!("Annonce refusée : {e}"))?;
    Ok(demon)
}

/// Nom de la machine, tel qu'il s'affichera sur le telephone.
///
/// ⚠️ Rend un nom neutre plutot que d'echouer : l'appairage ne doit pas dependre d'une variable
/// d'environnement absente.
fn hostname_court() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Oyant".to_string())
}

/// Ou vit la liste des appareils appaires.
fn chemin_appaires() -> Result<std::path::PathBuf, String> {
    Ok(crate::chemins::configuration()?.join("appaires.json"))
}

/// Charge la liste, en rendant une liste VIDE si quoi que ce soit cloche.
///
/// ⛔ **Un fichier illisible doit vider la liste, jamais l'ignorer en gardant une autorisation.**
/// Se tromper dans ce sens ferait redemander un appairage ; se tromper dans l'autre laisserait
/// entrer un appareil que l'utilisateur croit avoir revoque.
pub fn charger_appaires() -> Appaires {
    let Ok(chemin) = chemin_appaires() else {
        return Appaires::default();
    };
    let Ok(contenu) = std::fs::read_to_string(chemin) else {
        return Appaires::default();
    };
    serde_json::from_str(&contenu).unwrap_or_default()
}

/// Ecrit la liste.
pub fn enregistrer_appaires(liste: &Appaires) -> Result<(), String> {
    let chemin = chemin_appaires()?;
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Répertoire impossible : {e}"))?;
    }
    let texte =
        serde_json::to_string_pretty(liste).map_err(|e| format!("Liste non sérialisable : {e}"))?;
    std::fs::write(&chemin, texte).map_err(|e| format!("Liste non écrite : {e}"))
}

/// L'empreinte du certificat de CETTE machine, a montrer dans les reglages.
///
/// ⛔ **C'est elle que le telephone epingle**, et c'est donc elle qu'un utilisateur compare s'il
/// veut verifier a quoi il se connecte. Elle part aussi dans le QR du repli, avec l'adresse et le
/// port, quand la decouverte ne passe pas — cas de l'isolation des clients.
#[tauri::command]
pub fn reseau_empreinte() -> Result<String, String> {
    let identite = crate::reseau::identite_tls()?;
    crate::reseau::empreinte_certificat(&identite.certificat_pem)
}

/// Les appareils autorises, pour l'ecran de reglages.
#[tauri::command]
pub fn reseau_appareils() -> Vec<AppareilAppaire> {
    charger_appaires().appareils
}

/// Retire un appareil. Rend `true` si quelque chose a ete retire.
#[tauri::command]
pub fn reseau_revoquer(empreinte: String) -> Result<bool, String> {
    let mut liste = charger_appaires();
    let retire = liste.revoquer(&empreinte);
    enregistrer_appaires(&liste)?;
    Ok(retire)
}

/// Horodatage ISO 8601, sans dependance de date supplementaire.
fn horodatage() -> String {
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secondes}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appairage::VERSION_PROTOCOLE;
    use crate::reseau::{empreinte, fabriquer_identite_tls};

    /// Verificateur qui n'accepte QUE le certificat epingle.
    ///
    /// ⛔ **C'est exactement ce que le telephone doit faire**, et c'est pour ca qu'il vit ici
    /// plutot que dans un utilitaire de test anonyme : il documente la contrepartie. Un client qui
    /// accepterait n'importe quel certificat rendrait tout le TLS decoratif, puisque n'importe qui
    /// sur le wifi pourrait se placer au milieu.
    #[derive(Debug)]
    struct Epingle(Vec<u8>);

    impl rustls::client::danger::ServerCertVerifier for Epingle {
        fn verify_server_cert(
            &self,
            end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            if end_entity.as_ref() == self.0.as_slice() {
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            } else {
                Err(rustls::Error::General("certificat non épinglé".into()))
            }
        }

        fn verify_tls12_signature(
            &self,
            _m: &[u8],
            _c: &rustls::pki_types::CertificateDer<'_>,
            _d: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _m: &[u8],
            _c: &rustls::pki_types::CertificateDer<'_>,
            _d: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    /// Joue une connexion complete : TLS epingle, WebSocket, un `bonjour`, et rend la reponse.
    async fn dialoguer(
        adresse: std::net::SocketAddr,
        certificat_der: Vec<u8>,
        nom: &str,
        cle: &str,
        version: &str,
    ) -> Reponse {
        let config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Epingle(certificat_der)))
            .with_no_client_auth();
        let connecteur = tokio_rustls::TlsConnector::from(Arc::new(config));
        let tcp = tokio::net::TcpStream::connect(adresse).await.expect("tcp");
        let nom_serveur = rustls::pki_types::ServerName::try_from("oyant.local").expect("nom");
        let tls = connecteur.connect(nom_serveur, tcp).await.expect("tls");
        let (mut ws, _) = tokio_tungstenite::client_async("ws://oyant.local/", tls)
            .await
            .expect("websocket");

        let bonjour = serde_json::json!({ "version": version, "nom": nom, "cle_publique": cle });
        futures_util::SinkExt::send(
            &mut ws,
            tokio_tungstenite::tungstenite::Message::Text(bonjour.to_string()),
        )
        .await
        .expect("envoi");

        let recu = futures_util::StreamExt::next(&mut ws)
            .await
            .expect("réponse")
            .expect("trame");
        serde_json::from_str(&recu.into_text().expect("texte")).expect("json")
    }

    /// Monte un serveur sur un port libre et rend de quoi lui parler.
    async fn serveur_de_test(
        appaires: Arc<Mutex<Appaires>>,
    ) -> (
        std::net::SocketAddr,
        Vec<u8>,
        mpsc::Receiver<DemandeAppairage>,
    ) {
        let identite = fabriquer_identite_tls().expect("identité");
        let der = rustls_pemfile::certs(&mut identite.certificat_pem.as_bytes())
            .next()
            .expect("un certificat")
            .expect("lisible")
            .as_ref()
            .to_vec();
        let (envoi, reception) = mpsc::channel(4);
        // Port 0 : le systeme en choisit un libre, et `servir` rend celui qui a ete pris. Figer un
        // port ferait echouer ce test des qu'une autre chose ecoute dessus.
        let adresse = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
        let (reelle, _tache) = servir(adresse, identite, appaires, envoi)
            .await
            .expect("serveur");
        (reelle, der, reception)
    }

    #[tokio::test]
    async fn un_inconnu_accepte_par_l_utilisateur_entre_et_reste_retenu() {
        let appaires = Arc::new(Mutex::new(Appaires::default()));
        let (adresse, der, mut demandes) = serveur_de_test(Arc::clone(&appaires)).await;

        // L'utilisateur dit oui. ⚠️ Le test joue son role, ce qui permet d'eprouver le serveur
        // sans appareil — y compris les cas ou il ne repond pas (voir les tests suivants).
        tokio::spawn(async move {
            let demande = demandes.recv().await.expect("une demande");
            assert_eq!(
                demande.code.len(),
                4,
                "l'utilisateur doit voir quatre chiffres"
            );
            assert_eq!(demande.nom, "iPhone");
            let _ = demande.reponse.send(true);
        });

        let reponse = dialoguer(adresse, der, "iPhone", "cle-publique", VERSION_PROTOCOLE).await;
        assert_eq!(
            reponse,
            Reponse::Bienvenue {
                empreinte: empreinte(b"cle-publique")
            }
        );
        // ⛔ Et il est RETENU : sans ca l'utilisateur redirait oui a chaque connexion, et finirait
        // par dire oui sans regarder.
        assert!(appaires.lock().await.autorise(&empreinte(b"cle-publique")));
    }

    #[tokio::test]
    async fn un_inconnu_refuse_reste_dehors_et_n_est_pas_retenu() {
        let appaires = Arc::new(Mutex::new(Appaires::default()));
        let (adresse, der, mut demandes) = serveur_de_test(Arc::clone(&appaires)).await;

        tokio::spawn(async move {
            let demande = demandes.recv().await.expect("une demande");
            let _ = demande.reponse.send(false);
        });

        let reponse = dialoguer(adresse, der, "iPhone", "cle-publique", VERSION_PROTOCOLE).await;
        assert_eq!(
            reponse,
            Reponse::Refus {
                motif: Motif::Refuse
            }
        );
        assert!(!appaires.lock().await.autorise(&empreinte(b"cle-publique")));
    }

    #[tokio::test]
    async fn personne_ne_repond_donc_on_refuse() {
        let appaires = Arc::new(Mutex::new(Appaires::default()));
        // ⛔ Le recepteur est ABANDONNE : c'est le cas « l'interface n'est pas la pour afficher la
        // demande ». Il doit refuser, jamais autoriser par defaut.
        let (adresse, der, demandes) = serveur_de_test(Arc::clone(&appaires)).await;
        drop(demandes);

        let reponse = dialoguer(adresse, der, "iPhone", "cle-publique", VERSION_PROTOCOLE).await;
        assert_eq!(
            reponse,
            Reponse::Refus {
                motif: Motif::SansReponse
            }
        );
        assert!(!appaires.lock().await.autorise(&empreinte(b"cle-publique")));
    }

    #[tokio::test]
    async fn une_mauvaise_version_ne_derange_meme_pas_l_utilisateur() {
        let appaires = Arc::new(Mutex::new(Appaires::default()));
        let (adresse, der, mut demandes) = serveur_de_test(Arc::clone(&appaires)).await;

        let reponse = dialoguer(adresse, der, "iPhone", "cle-publique", "0").await;
        assert_eq!(
            reponse,
            Reponse::Refus {
                motif: Motif::VersionIncompatible
            }
        );
        // ⛔ Le point du test : AUCUNE demande n'a ete envoyee a l'utilisateur. Le deranger pour un
        // client qu'on ne sait pas interpreter, c'est l'habituer a accepter sans lire.
        assert!(
            demandes.try_recv().is_err(),
            "l'utilisateur ne doit pas être sollicité pour une version incompatible"
        );
    }
}
