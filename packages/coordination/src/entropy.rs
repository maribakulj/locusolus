//! `degree_entropy` — la dispersion structurelle d'une version. `W21.f`, ADR 0024.
//!
//! # Ce que le nom d'origine ne disait pas
//!
//! La matrice d'acceptation demandait `topology_entropy`, sans dire **de quelle distribution**.
//! Au moins quatre candidates existaient — degrés, charge de travail, types d'arêtes, tailles des
//! partitions de visibilité — et elles ne classent pas les organisations dans le même ordre. L'ADR
//! 0024 décision 2 a donc arrêté `degree_entropy`, qui nomme la sienne.
//!
//! C'est l'entropie de Shannon de la distribution `p_i = degré(i) / somme des degrés` : la
//! probabilité qu'une extrémité d'arête prise au hasard appartienne au nœud `i`. Le degré compte
//! les arêtes **incidentes**, entrantes et sortantes, toutes sortes de relation confondues : ce
//! qu'on mesure est la forme du graphe de coordination, pas celle d'une de ses couches.
//!
//! # Sans normalisation, on compare des tailles en croyant comparer des structures
//!
//! L'entropie brute croît mécaniquement avec le nombre de nœuds : une organisation de trente agents
//! a presque toujours une entropie supérieure à une organisation de cinq, quelle que soit leur
//! forme. Diviser par `ln n` ramène la mesure sur `[0, 1]`, où `1` est la dispersion parfaite —
//! tous les nœuds de même degré.
//!
//! La propriété qui rend cela vérifiable : un cycle de trois nœuds et un cycle de trente rendent la
//! **même** valeur. C'est le test qui porte l'item.
//!
//! # Pourquoi la valeur est arrondie
//!
//! Deux mesures de la même forme doivent être **égales**, pas presque égales, sinon la comparaison
//! qui justifie la normalisation ne se fait pas : personne ne compare deux organisations à epsilon
//! près, et un lecteur qui voit `0.999999999999` et `1.0` conclut à une différence.
//!
//! L'arrondi est à `1e-9`, très en dessous de toute différence structurelle qu'on voudrait lire, et
//! très au-dessus de l'erreur d'accumulation d'une somme de quelques milliers de termes.
//!
//! # Ce que cette métrique ne mesure **pas**
//!
//! **L'équité de la charge.** Une étoile — un nœud relié à tous les autres — a une entropie de
//! degrés élevée et une charge complètement concentrée. La concentration est mesurée par
//! `busiest_reviewer_load`, livrée par `R3` dans [`crate::metrics`], et c'est elle qu'on veut quand
//! on cherche un goulot. Un test exhibe la fixture où les deux répondent différemment, plutôt que
//! de laisser la phrase se croire sur parole.
//!
//! Elle ne juge pas non plus : ni seuil, ni note, ni verdict — décision 9 de l'ADR 0024. Une
//! entropie basse n'est pas une faute ; une organisation en étoile est parfois exactement ce qu'on
//! veut.

use std::collections::BTreeMap;
use std::fmt;

use locus_protocol::{Id, id::Agent};

use crate::version::Version;

/// La précision à laquelle la valeur est arrêtée. Voir la documentation du module.
const PRECISION: f64 = 1e9;

/// L'entropie de degrés d'une version, ou la raison pour laquelle elle n'en a pas.
///
/// **Deux** absences, et elles ne se fondent pas : elles décrivent deux organisations différentes,
/// et un appelant qui lirait « pas de valeur » sans savoir laquelle ne saurait pas quoi regarder.
///
/// # La troisième absence n'existe pas, et c'est mieux qu'un cas rendu
///
/// Une première rédaction portait une variante « aucun membre ». Elle est **inatteignable** :
/// [`Version::root`] refuse une version sans membre, et retirer le dernier nœud échoue de la même
/// façon. La variante annonçait donc un cas que rien ne peut produire — une promesse, au sens de la
/// décision 0 de l'ADR 0022, et le genre de valeur d'énumération que `CLAUDE.md` refuse.
///
/// Le cas est **inexprimable**, ce qui est plus fort que rendu : il n'y a pas de branche à tester,
/// pas de message à écrire, et pas de lecteur à qui expliquer un état qu'il ne verra jamais. Un test
/// tient les deux chemins qui pourraient y mener.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DegreeEntropy {
    /// La valeur, normalisée sur `[0, 1]`.
    Measured(f64),
    /// Un seul membre. `ln 1 = 0`, et il n'y a rien à disperser — la question ne se pose pas,
    /// plutôt qu'elle se pose et n'a pas de réponse.
    SingleMember,
    /// Des membres, mais aucune arête. Les degrés sont tous nuls, donc il n'existe aucune
    /// distribution à mesurer. Distinct de `SingleMember` : ici la structure pourrait exister, elle
    /// est simplement vide.
    NoEdges,
}

impl DegreeEntropy {
    /// Mesurer une version.
    #[must_use]
    pub fn of(version: &Version) -> Self {
        let members = version.members();
        if members.len() <= 1 {
            return Self::SingleMember;
        }

        let mut degrees: BTreeMap<Id<Agent>, usize> =
            members.iter().map(|member| (*member, 0)).collect();
        for relation in version.relations() {
            for endpoint in [relation.from, relation.to] {
                if let Some(degree) = degrees.get_mut(&endpoint) {
                    *degree += 1;
                }
            }
        }

        let total: usize = degrees.values().sum();
        if total == 0 {
            return Self::NoEdges;
        }

        #[expect(
            clippy::cast_precision_loss,
            reason = "un degré et leur somme ne franchissent pas 2^53"
        )]
        let total = total as f64;
        let mut entropy = 0.0_f64;
        for degree in degrees.values().copied().filter(|degree| *degree > 0) {
            #[expect(clippy::cast_precision_loss, reason = "un degré ne franchit pas 2^53")]
            let share = degree as f64 / total;
            entropy -= share * share.ln();
        }

        #[expect(
            clippy::cast_precision_loss,
            reason = "un nombre de membres ne franchit pas 2^53"
        )]
        let ceiling = (members.len() as f64).ln();
        Self::Measured(round(entropy / ceiling))
    }

    /// La valeur, si elle existe.
    ///
    /// Les deux absences se lisent sur la variante, jamais ici : un appelant qui n'a besoin que du
    /// nombre ne doit pas avoir à savoir pourquoi il manque, mais celui qui veut le savoir ne doit
    /// pas en être privé.
    #[must_use]
    pub const fn value(self) -> Option<f64> {
        match self {
            Self::Measured(value) => Some(value),
            _ => None,
        }
    }
}

impl fmt::Display for DegreeEntropy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Measured(value) => write!(formatter, "{value:.3}"),
            Self::SingleMember => formatter.write_str("un seul membre — rien à disperser"),
            Self::NoEdges => formatter.write_str("aucune arête — aucune distribution de degrés"),
        }
    }
}

/// Arrêter la valeur à `1e-9`, pour que deux mesures de la même forme soient **égales**.
fn round(value: f64) -> f64 {
    (value * PRECISION).round() / PRECISION
}
