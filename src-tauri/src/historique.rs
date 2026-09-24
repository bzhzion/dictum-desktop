//! L'historique des transcriptions.
//!
//! ⛔ **C'est le fichier le plus personnel qu’Oyant ecrit.** Il contient ce que quelqu'un a dit
//! a voix haute chez lui : des mots de passe dictes, des messages prives, des notes medicales. Il
//! est donc **desactive par defaut**, il vit dans le profil LOCAL et jamais dans un profil
//! itinerant, et **le couper efface ce qui a ete garde**.
//!
//! Ce dernier point est le seul qui ne soit pas evident : un reglage « historique : non » qui
//! laisserait le fichier en place ne desactiverait rien du passe, alors que c'est justement ce
//! qu'on demande en le coupant.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::chemins;

/// Une transcription retenue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entree {
    /// Secondes depuis l'epoque Unix. ⚠️ Pas une date formatee : un horodatage lisible dependrait
    /// du fuseau de la machine qui l'ecrit, et l'historique voyage avec le profil.
    #[serde(rename = "at")]
    pub horodatage: u64,
    #[serde(rename = "text")]
    pub texte: String,
}

fn fichier() -> Result<PathBuf, String> {
    Ok(chemins::base()?.join("historique.json"))
}

/// Ce que l'historique contient aujourd'hui, le plus recent en premier.
pub fn lire() -> Vec<Entree> {
    let Ok(chemin) = fichier() else {
        return Vec::new();
    };
    let Ok(contenu) = fs::read_to_string(&chemin) else {
        return Vec::new();
    };
    // ⚠️ Un fichier abime ne doit pas empecher de dicter : on rend une liste vide plutot que de
    // remonter une erreur sur un chemin qui n'est pas critique.
    serde_json::from_str(&contenu).unwrap_or_default()
}

/// Retient une transcription, en respectant le plafond.
///
/// Un plafond a zero veut dire « pas d'historique » : on n'ecrit rien et on efface ce qui reste.
pub fn ajouter(texte: &str, plafond: u32) -> Result<(), String> {
    if plafond == 0 {
        return effacer();
    }
    if texte.trim().is_empty() {
        return Ok(());
    }

    let mut entrees = lire();
    entrees.insert(
        0,
        Entree {
            horodatage: maintenant(),
            texte: texte.to_string(),
        },
    );
    // ⚠️ On APPELLE `plafonner` au lieu de retronquer ici : un `truncate` recopie a cet endroit
    // laisserait le test protéger une fonction que personne n'utilise, ce qui est la façon la plus
    // discrète de n'avoir aucun test du tout.
    ecrire(&plafonner(entrees, plafond))
}

/// Efface l'historique et son fichier.
pub fn effacer() -> Result<(), String> {
    let chemin = fichier()?;
    match fs::remove_file(&chemin) {
        Ok(()) => Ok(()),
        // Deja absent : c'est l'etat voulu, pas une erreur.
        Err(erreur) if erreur.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(erreur) => Err(format!("Historique non effacé : {erreur}")),
    }
}

fn ecrire(entrees: &[Entree]) -> Result<(), String> {
    let chemin = fichier()?;
    if let Some(parent) = chemin.parent() {
        fs::create_dir_all(parent).map_err(|erreur| format!("Répertoire non créé : {erreur}"))?;
    }
    let contenu = serde_json::to_string_pretty(entrees)
        .map_err(|erreur| format!("Historique non sérialisé : {erreur}"))?;

    // ⚠️ Ecriture par fichier temporaire puis renommage, comme les reglages : une coupure au
    // milieu d'une ecriture directe laisserait un JSON tronque, donc un historique perdu en
    // entier plutot qu'une entree manquante.
    let temporaire = chemin.with_extension("json.partiel");
    fs::write(&temporaire, contenu).map_err(|erreur| format!("Historique non écrit : {erreur}"))?;
    fs::rename(&temporaire, &chemin).map_err(|erreur| format!("Historique non posé : {erreur}"))
}

fn maintenant() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duree| duree.as_secs())
        .unwrap_or(0)
}

/// Ne garde que les `plafond` entrees les plus recentes.
///
/// Fonction PURE, pour que la regle du plafond se teste sans ecrire sur le disque.
/// ⚠️ **Pas de cas particulier pour zéro** : `truncate(0)` vide déjà la liste. Une clause de garde
/// y a figuré, et la mutation l'a démasquée comme redondante en restant verte quand on l'enlevait.
/// Le test ci-dessous garde donc le CONTRAT (zéro ne conserve rien) et non cette ligne-là.
pub fn plafonner(entrees: Vec<Entree>, plafond: u32) -> Vec<Entree> {
    let mut entrees = entrees;
    entrees.truncate(plafond as usize);
    entrees
}

/// L'historique, pour l'ecran de reglages.
#[tauri::command]
pub fn etat_historique() -> Vec<Entree> {
    lire()
}

#[tauri::command]
pub fn effacer_historique() -> Result<(), String> {
    effacer()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entree(texte: &str) -> Entree {
        Entree {
            horodatage: 0,
            texte: texte.to_string(),
        }
    }

    /// ⛔ **Un plafond a zero vide l'historique, il ne le garde pas « au cas ou ».** C'est ce que
    /// veut dire couper la fonctionnalite, et un fichier laisse en place ne desactiverait rien du
    /// passe.
    #[test]
    fn un_plafond_nul_ne_garde_rien() {
        let entrees = vec![entree("a"), entree("b")];
        assert!(plafonner(entrees, 0).is_empty());
    }

    /// ⚠️ Le plus recent est en tete : c'est l'ordre d'insertion, et c'est aussi celui qu'un
    /// ecran affiche. Plafonner par la fin garderait les plus ANCIENNES, soit exactement
    /// l'inverse de ce qu'on veut.
    #[test]
    fn le_plafond_garde_les_entrees_les_plus_recentes() {
        let entrees = vec![entree("recent"), entree("moyen"), entree("vieux")];
        let gardees = plafonner(entrees, 2);
        assert_eq!(gardees.len(), 2);
        assert_eq!(gardees[0].texte, "recent");
        assert_eq!(gardees[1].texte, "moyen");
    }

    #[test]
    fn un_plafond_plus_grand_que_la_liste_ne_retire_rien() {
        let entrees = vec![entree("a")];
        assert_eq!(plafonner(entrees, 50).len(), 1);
    }

    /// ⚠️ Les cles du fichier sont en ANGLAIS, comme celles des reglages : le fichier est lu par
    /// des gens et recopie d'une machine a l'autre, et le produit passera a l'anglais par defaut.
    #[test]
    fn le_format_du_fichier_est_en_anglais() {
        let json = serde_json::to_string(&entree("bonjour")).unwrap();
        assert!(json.contains("\"at\""), "{json}");
        assert!(json.contains("\"text\""), "{json}");
        assert!(!json.contains("horodatage"), "{json}");
        assert!(!json.contains("texte"), "{json}");
    }
}
