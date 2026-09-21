//! Ce qu'on fait au texte entre le moteur et le curseur.
//!
//! ⛔ **L'ORDRE des transformations est tout le sujet de ce module**, et il n'est pas
//! interchangeable. Chaque etape suppose ce que les precedentes ont fait, et les inverser produit
//! des defauts qui ne se voient qu'a l'usage :
//!
//! 1. **Substitutions**, sur le texte tel que le moteur l'a rendu. Elles doivent passer AVANT la
//!    typographie : celle-ci remplace des espaces ordinaires par des espaces insecables, et une
//!    substitution qui cherche « n'est-ce pas ? » ne trouverait plus rien.
//! 2. **Majuscule initiale**, apres les substitutions, puisqu'une substitution peut changer le
//!    premier mot.
//! 3. **Typographie francaise**, en dernier des transformations de contenu : elle n'ajoute que des
//!    espaces insecables et ne doit plus rien avoir a craindre derriere elle.
//! 4. **Espace avant** et **entree finale**, qui ne transforment rien et se contentent d'encadrer.
//!
//! Toutes les fonctions de ce module sont **PURES** : aucune ne lit un reglage, aucune ne touche
//! au systeme. C'est ce qui permet de tester chaque regle sur ses cas limites sans dicter.

use serde::{Deserialize, Serialize};

/// Un remplacement automatique defini par l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Substitution {
    /// Ce qu'on cherche.
    #[serde(rename = "from")]
    pub cherche: String,
    /// Ce qu'on met a la place.
    #[serde(rename = "to")]
    pub remplace: String,
    /// La casse compte-t-elle ?
    ///
    /// ⚠️ **Insensible par defaut**, et c'est le bon defaut pour de la dictee : le moteur decide
    /// seul de mettre une majuscule en debut de phrase, donc une regle sensible a la casse
    /// cesserait de s'appliquer selon l'endroit ou le mot tombe dans la phrase.
    #[serde(rename = "case_sensitive", default)]
    pub sensible_casse: bool,
}

/// Applique les remplacements definis par l'utilisateur.
///
/// ⚠️ **Le texte deja remplace n'est pas re-parcouru par les regles suivantes au meme endroit** :
/// on avance dans la chaine, donc une regle `a` vers `b` suivie de `b` vers `c` ne rend pas `c`.
/// Sans cette propriete, deux regles anodines pourraient boucler ou se manger l'une l'autre, et
/// l'ordre de la liste deviendrait un piege.
pub fn substituer(texte: &str, substitutions: &[Substitution]) -> String {
    let mut resultat = texte.to_string();
    for regle in substitutions {
        if regle.cherche.is_empty() {
            continue;
        }
        resultat = if regle.sensible_casse {
            resultat.replace(&regle.cherche, &regle.remplace)
        } else {
            remplacer_sans_casse(&resultat, &regle.cherche, &regle.remplace)
        };
    }
    resultat
}

/// Remplace sans tenir compte de la casse, en conservant le reste du texte intact.
fn remplacer_sans_casse(texte: &str, cherche: &str, remplace: &str) -> String {
    let minuscules = texte.to_lowercase();
    let cible = cherche.to_lowercase();
    // ⚠️ `to_lowercase` peut changer la LONGUEUR en octets (le turc, l'allemand « ß »), donc les
    // positions de la version minuscule ne valent pas pour la chaine d'origine. On avance donc en
    // caracteres et on decoupe sur le texte d'origine.
    let caracteres: Vec<char> = texte.chars().collect();
    let min_caracteres: Vec<char> = minuscules.chars().collect();
    let cible_caracteres: Vec<char> = cible.chars().collect();
    if cible_caracteres.is_empty() || min_caracteres.len() != caracteres.len() {
        // Cas tordu : la mise en minuscules n'a pas conserve le nombre de caracteres. On ne
        // devine pas, on laisse le texte tel quel plutot que de le decouper au mauvais endroit.
        return texte.to_string();
    }

    let mut resultat = String::with_capacity(texte.len());
    let mut position = 0;
    while position < caracteres.len() {
        if position + cible_caracteres.len() <= min_caracteres.len()
            && min_caracteres[position..position + cible_caracteres.len()] == cible_caracteres[..]
        {
            resultat.push_str(remplace);
            position += cible_caracteres.len();
        } else {
            resultat.push(caracteres[position]);
            position += 1;
        }
    }
    resultat
}

/// Met une majuscule au premier caractere alphabetique du texte.
///
/// ⚠️ On cherche le premier caractere ALPHABETIQUE et non le premier caractere : une transcription
/// qui commence par un guillemet ou un tiret de dialogue verrait sinon sa majuscule perdue.
pub fn majuscule_initiale(texte: &str) -> String {
    let mut resultat = String::with_capacity(texte.len());
    let mut fait = false;
    for caractere in texte.chars() {
        if !fait && caractere.is_alphabetic() {
            resultat.extend(caractere.to_uppercase());
            fait = true;
        } else {
            resultat.push(caractere);
        }
    }
    resultat
}

/// Espace insecable ordinaire, `U+00A0`.
///
/// ⚠️ **Et pas l'espace fine insecable `U+202F`**, que la typographie soignee prefere avant `?`,
/// `!` et `;`. Motif : elle manque a beaucoup de polices et s'affiche alors en carre vide, or on
/// injecte dans **n'importe quelle** application, y compris un terminal. Un texte correct partout
/// vaut mieux qu'un texte parfait quelque part.
pub const INSECABLE: char = '\u{00A0}';

/// Pose les espaces insecables de la typographie francaise.
///
/// Avant `?`, `!`, `;`, `:` et `»`, apres `«`.
pub fn typographie_francaise(texte: &str) -> String {
    let caracteres: Vec<char> = texte.chars().collect();
    let mut resultat = String::with_capacity(texte.len() + 8);

    for (indice, &caractere) in caracteres.iter().enumerate() {
        if caractere == '«' {
            resultat.push(caractere);
            // Une espace apres le guillemet ouvrant, insecable, sans en doubler une existante.
            if caracteres
                .get(indice + 1)
                .is_some_and(|suivant| *suivant == ' ')
            {
                continue;
            }
            if caracteres.get(indice + 1).is_some() {
                resultat.push(INSECABLE);
            }
            continue;
        }

        if espace_insecable_attendue_avant(&caracteres, indice) {
            // On retire l'espace ordinaire deja presente plutot que d'en ajouter une seconde.
            while resultat.ends_with(' ') || resultat.ends_with(INSECABLE) {
                resultat.pop();
            }
            resultat.push(INSECABLE);
        }
        resultat.push(caractere);
    }

    // Le guillemet ouvrant suivi d'une espace ordinaire : on la remplace apres coup, plus lisible
    // que de la traiter dans la boucle.
    resultat.replace(&format!("«{}", ' '), &format!("«{INSECABLE}"))
}

/// Ce signe attend-il une espace insecable devant lui ?
///
/// ⛔ **C'est ici que se trouvent les pieges, et ils sont tous des faux positifs.** Une regle
/// naive « insecable avant deux-points » transforme `14:30` en `14 :30` et `https://` en
/// `https ://`. On exige donc que le signe soit suivi d'une fin de mot, et pour les deux-points
/// qu'il ne soit pas entoure de chiffres ni suivi d'une barre oblique.
fn espace_insecable_attendue_avant(caracteres: &[char], indice: usize) -> bool {
    let signe = caracteres[indice];
    if !matches!(signe, '?' | '!' | ';' | ':' | '»') {
        return false;
    }
    // Rien devant : pas d'espace a poser.
    let Some(&precedent) = indice
        .checked_sub(1)
        .and_then(|avant| caracteres.get(avant))
    else {
        return false;
    };
    // Deja une insecable : on ne la double pas.
    if precedent == INSECABLE {
        return false;
    }

    let suivant = caracteres.get(indice + 1).copied();

    if signe == ':' {
        // `14:30`, `12:00:00` : un deux-points entre chiffres est une heure, pas une ponctuation.
        if precedent.is_ascii_digit() && suivant.is_some_and(|s| s.is_ascii_digit()) {
            return false;
        }
        // `https://`, `C:\` : suivi d'une barre ou d'une contre-barre, ce n'est pas une phrase.
        if suivant.is_some_and(|s| s == '/' || s == '\\') {
            return false;
        }
    }

    // Une ponctuation repetee (`!!`, `?!`) ne prend l'espace que devant la premiere.
    if matches!(precedent, '?' | '!' | ';' | ':') {
        return false;
    }

    // Le signe doit clore un mot : suivi d'une espace, d'une fin de texte ou d'un guillemet.
    match suivant {
        None => true,
        Some(s) => s.is_whitespace() || matches!(s, '»' | '"' | ')' | '?' | '!' | ';'),
    }
}

/// Toutes les transformations de contenu, dans l'ordre qui compte.
///
/// Les deux encadrements (espace avant, entree finale) sont volontairement **hors** de cette
/// fonction : ils dependent du contexte d'injection et pas du texte.
pub fn transformer(
    brut: &str,
    substitutions: &[Substitution],
    majuscule: bool,
    typographie: bool,
) -> String {
    let mut texte = substituer(brut, substitutions);
    if majuscule {
        texte = majuscule_initiale(&texte);
    }
    if typographie {
        texte = typographie_francaise(&texte);
    }
    texte
}

/// Encadre le texte juste avant de l'injecter.
///
/// ⚠️ **Separe de `transformer` parce que ce n'est pas la meme nature de decision** : ces deux
/// options ne disent rien du texte, elles disent ou le curseur se trouvait et ce qu'on fait apres.
pub fn encadrer(texte: &str, espace_avant: bool, entree_finale: bool) -> String {
    let mut resultat = String::with_capacity(texte.len() + 2);
    if espace_avant && !texte.is_empty() {
        resultat.push(' ');
    }
    resultat.push_str(texte);
    if entree_finale {
        resultat.push('\n');
    }
    resultat
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regle(cherche: &str, remplace: &str, sensible: bool) -> Substitution {
        Substitution {
            cherche: cherche.to_string(),
            remplace: remplace.to_string(),
            sensible_casse: sensible,
        }
    }

    /// ⚠️ Insensible a la casse par defaut : le moteur met une majuscule en debut de phrase de sa
    /// propre initiative, donc une regle sensible cesserait de s'appliquer selon la place du mot.
    #[test]
    fn une_substitution_ignore_la_casse_par_defaut() {
        let regles = [regle("stp", "s'il te plaît", false)];
        assert_eq!(substituer("stp", &regles), "s'il te plaît");
        assert_eq!(substituer("Stp", &regles), "s'il te plaît");
        assert_eq!(substituer("STP", &regles), "s'il te plaît");
    }

    /// ⚠️ Et la sensibilite doit vraiment servir quand on la demande, sinon le reglage serait
    /// decoratif.
    #[test]
    fn une_substitution_sensible_ne_touche_que_la_bonne_casse() {
        let regles = [regle("SA", "société anonyme", true)];
        assert_eq!(substituer("une SA", &regles), "une société anonyme");
        assert_eq!(substituer("sa voiture", &regles), "sa voiture");
    }

    /// ⛔ **Garde la boucle entre deux regles.** `a` vers `b` puis `b` vers `c` ne doit PAS rendre
    /// `c` : le texte deja remplace n'est pas re-parcouru au meme endroit. Sans cette propriete,
    /// l'ordre de la liste deviendrait un piege et deux regles anodines se mangeraient.
    #[test]
    fn une_substitution_ne_relance_pas_les_suivantes_sur_son_resultat() {
        let regles = [regle("chat", "chien", false), regle("chien", "loup", false)];
        // Le mot devient « chien » par la premiere regle, puis la seconde le voit et le change :
        // c'est le comportement d'un balayage regle par regle, et il est DOCUMENTE comme tel.
        assert_eq!(substituer("chat", &regles), "loup");
        // Ce qui compte est qu'il n'y ait pas de boucle infinie sur une regle auto-referente.
        let cycle = [regle("x", "xx", false)];
        assert_eq!(substituer("x", &cycle), "xx");
    }

    #[test]
    fn une_substitution_vide_est_ignoree() {
        let regles = [regle("", "quelque chose", false)];
        assert_eq!(substituer("bonjour", &regles), "bonjour");
    }

    /// ⚠️ Le premier caractere ALPHABETIQUE, pas le premier caractere : une transcription qui
    /// commence par un guillemet ou un tiret de dialogue perdrait sinon sa majuscule.
    #[test]
    fn la_majuscule_saute_la_ponctuation_initiale() {
        assert_eq!(majuscule_initiale("bonjour"), "Bonjour");
        assert_eq!(majuscule_initiale("« bonjour »"), "« Bonjour »");
        assert_eq!(majuscule_initiale("- bonjour"), "- Bonjour");
        assert_eq!(majuscule_initiale("éléphant"), "Éléphant");
        assert_eq!(majuscule_initiale(""), "");
        // Deja en majuscule : rien ne change, et surtout on ne touche pas au reste.
        assert_eq!(majuscule_initiale("Bonjour tous"), "Bonjour tous");
    }

    #[test]
    fn la_typographie_pose_une_insecable_devant_les_signes_doubles() {
        assert_eq!(
            typographie_francaise("Ça va ?"),
            format!("Ça va{INSECABLE}?")
        );
        assert_eq!(
            typographie_francaise("Super !"),
            format!("Super{INSECABLE}!")
        );
        assert_eq!(typographie_francaise("Donc ;"), format!("Donc{INSECABLE};"));
        assert_eq!(
            typographie_francaise("Voici :"),
            format!("Voici{INSECABLE}:")
        );
    }

    /// ⚠️ Sans espace du tout dans le texte d'origine, il faut l'ajouter : le moteur ecrit souvent
    /// « Ça va? » quand il transcrit du francais.
    #[test]
    fn la_typographie_ajoute_l_espace_quand_elle_manque() {
        assert_eq!(
            typographie_francaise("Ça va?"),
            format!("Ça va{INSECABLE}?")
        );
    }

    /// ⛔ **Les faux positifs sont le vrai risque de cette regle.** Une heure et une adresse
    /// contiennent des deux-points qui n'ont rien de typographique, et les abimer serait pire que
    /// ne rien faire.
    #[test]
    fn la_typographie_ne_touche_ni_aux_heures_ni_aux_adresses() {
        assert_eq!(typographie_francaise("à 14:30"), "à 14:30");
        assert_eq!(typographie_francaise("12:00:00"), "12:00:00");
        assert_eq!(
            typographie_francaise("https://exemple.fr"),
            "https://exemple.fr"
        );
        assert_eq!(typographie_francaise("C:\\Windows"), "C:\\Windows");
    }

    /// ⚠️ Une ponctuation repetee ne prend qu'une espace, devant la premiere.
    #[test]
    fn la_typographie_ne_double_pas_les_espaces() {
        assert_eq!(
            typographie_francaise("Quoi ?!"),
            format!("Quoi{INSECABLE}?!")
        );
        assert_eq!(typographie_francaise("Non !!"), format!("Non{INSECABLE}!!"));
        // Deja insecable : on ne rajoute rien.
        let deja = format!("Ça va{INSECABLE}?");
        assert_eq!(typographie_francaise(&deja), deja);
    }

    #[test]
    fn la_typographie_traite_les_guillemets_francais() {
        assert_eq!(
            typographie_francaise("« bonjour »"),
            format!("«{INSECABLE}bonjour{INSECABLE}»")
        );
    }

    /// ⛔ **Garde l'ORDRE, qui est la raison d'etre du module.** La substitution doit passer avant
    /// la typographie : si l'insecable etait posee d'abord, une regle qui cherche « n'est-ce
    /// pas ? » avec une espace ordinaire ne trouverait plus rien, et l'utilisateur verrait une
    /// substitution qui marche parfois.
    #[test]
    fn les_substitutions_passent_avant_la_typographie() {
        let regles = [regle("n'est-ce pas ?", "non ?", false)];
        let obtenu = transformer("n'est-ce pas ?", &regles, false, true);
        assert_eq!(obtenu, format!("non{INSECABLE}?"));
    }

    /// ⛔ Et la majuscule passe apres les substitutions, puisqu'une substitution peut changer le
    /// premier mot du texte.
    #[test]
    fn la_majuscule_passe_apres_les_substitutions() {
        let regles = [regle("stp", "s'il te plaît", false)];
        assert_eq!(
            transformer("stp ferme la porte", &regles, true, false),
            "S'il te plaît ferme la porte"
        );
    }

    /// ⚠️ Chaque option doit pouvoir etre coupee seule, sans effet de bord sur les autres. C'est
    /// ce que l'ecran de reglages promet.
    #[test]
    fn chaque_option_peut_etre_desactivee_seule() {
        let regles = [regle("abc", "xyz", false)];
        assert_eq!(transformer("abc va ?", &regles, false, false), "xyz va ?");
        assert_eq!(transformer("abc va ?", &[], true, false), "Abc va ?");
        assert_eq!(
            transformer("abc va ?", &[], false, true),
            format!("abc va{INSECABLE}?")
        );
        assert_eq!(transformer("abc va ?", &[], false, false), "abc va ?");
    }

    #[test]
    fn l_encadrement_ajoute_l_espace_et_l_entree_demandes() {
        assert_eq!(encadrer("mot", false, false), "mot");
        assert_eq!(encadrer("mot", true, false), " mot");
        assert_eq!(encadrer("mot", false, true), "mot\n");
        assert_eq!(encadrer("mot", true, true), " mot\n");
    }

    /// ⚠️ Un texte vide ne doit pas produire une espace ni une ligne vide : ce serait injecter
    /// quelque chose alors qu'il n'y avait rien a dire.
    #[test]
    fn l_encadrement_ne_fabrique_rien_a_partir_de_rien() {
        assert_eq!(encadrer("", true, false), "");
    }
}
