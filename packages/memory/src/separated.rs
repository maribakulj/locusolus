//! Deux retrievals séparés, épistémique et organisationnel — `docs/10` W17, `docs/SPEC_V1.md` §16.
//!
//! # Ce que « séparés » veut dire ici
//!
//! Ils répondent à deux questions qui n'ont pas le même sujet. L'épistémique répond « **que
//! savait-on** » et rend des révisions ; l'organisationnel répond « **qui a travaillé** » et rend
//! des agents. Le même corpus produit deux réponses, et aucune des deux ne se déduit de l'autre :
//! savoir qu'un agent a beaucoup produit ne dit rien de ce que ses productions valent, et
//! l'inverse non plus.
//!
//! # La séparation est tenue par les identités, pas par une garde
//!
//! `packages/protocol` fait du préfixe une **partie de l'identité** : « `evt_01ARZ…` et
//! `cmd_01ARZ…` ne sont pas le même identifiant, et le type les empêche d'être confondus à la
//! compilation ». `Id::parse` refuse un préfixe étranger.
//!
//! Une conversion d'un résultat en l'autre devrait donc fabriquer une identité qu'elle n'a pas, et
//! elle ne peut pas la fabriquer : ni directement, puisque les types diffèrent, ni par un
//! aller-retour en chaîne de caractères, puisque `rev_…` ne se relit pas comme un `agt_…`. C'est
//! plus fort qu'un lint, parce que c'est une impossibilité à la compilation plutôt qu'un motif
//! qu'on cherche.
//!
//! # Aucun trait ne les factorise
//!
//! Un trait « ce qui peut être cherché et classé » sur les deux serait la conversion reconstruite :
//! dès qu'un appelant écrit une fonction sur `impl Searchable`, les deux domaines se retraversent
//! sans qu'aucune ligne ne s'appelle « convertir ». C'est l'argument de l'ADR 0016 décision 9 pour
//! les familles d'objection, et il vaut mot pour mot ici.
//!
//! # Ce qui **est** partagé, et pourquoi c'est bien
//!
//! Le moteur : l'habilitation, le budget et le classement de [`crate::retrieval`]. Deux moteurs
//! divergeraient, et l'un des deux finirait par laisser passer ce que l'autre refuse — c'est
//! exactement la faute que la duplication doit éviter, à l'opposé de celle qu'elle sert à éviter
//! pour les résultats. **Partager le calcul, séparer les réponses.**
//!
//! Ce qui traverse le moteur le traverse **entier** : la marque de résultat négatif entre et
//! ressort. L'aplatir ici reviendrait à taire les résultats négatifs une couche plus haut que le
//! moteur, où plus personne ne regarde — et l'invariant 12 tomberait sans qu'aucun filtre ne
//! s'écrive.

use std::collections::BTreeMap;

use locus_domain::{Confidentiality, RevisionId};
use locus_protocol::{Id, id::Agent};

use crate::genre::Genre;
use crate::plan::Plan;
use crate::retrieval::{Candidate, Ranking, retrieve};

/// Ce qu'on cherche du côté **épistémique**.
#[derive(Debug, Clone, PartialEq)]
pub struct EpistemicEntry {
    /// L'objet épistémique.
    pub revision: RevisionId,
    /// Sa classification.
    pub classification: Confidentiality,
    /// Vrai quand il porte un résultat négatif — jamais une raison de l'écarter (invariant 12).
    pub is_negative: bool,
    /// Son score, facteurs compris.
    pub ranking: Ranking,
}

/// Ce qu'on cherche du côté **organisationnel**.
#[derive(Debug, Clone, PartialEq)]
pub struct OrganisationalEntry {
    /// L'instance d'agent.
    pub agent: Id<Agent>,
    /// Sa classification.
    pub classification: Confidentiality,
    /// Vrai quand ce qu'elle porte est un résultat négatif.
    pub is_negative: bool,
    /// Son score, facteurs compris.
    pub ranking: Ranking,
}

/// Ce qu'un retrieval **épistémique** rend : une révision, et pourquoi elle est là.
#[derive(Debug, Clone, PartialEq)]
pub struct EpistemicHit {
    /// L'objet épistémique trouvé.
    pub revision: RevisionId,
    /// Vrai quand il porte un résultat négatif.
    ///
    /// Porté jusqu'au résultat, et pas seulement jusqu'au moteur : l'aplatir ici reviendrait à
    /// taire les résultats négatifs une couche plus haut, ce que l'invariant 12 refuse.
    pub is_negative: bool,
    /// Le score, facteurs compris.
    pub ranking: Ranking,
}

/// Ce qu'un retrieval **organisationnel** rend : un agent, et pourquoi il est là.
#[derive(Debug, Clone, PartialEq)]
pub struct OrganisationalHit {
    /// L'instance d'agent trouvée.
    pub agent: Id<Agent>,
    /// Vrai quand ce qu'elle porte est un résultat négatif.
    pub is_negative: bool,
    /// Le score, facteurs compris.
    pub ranking: Ranking,
}

/// Chercher « que savait-on ».
///
/// # Panics
///
/// Jamais en pratique, et la raison est locale : le genre est **choisi ici**, parmi deux valeurs
/// dont aucune n'est `Formal`. Or `Candidate::new` ne refuse que le couple `(Formal, Vector)`.
/// Le `expect` est donc une assertion sur du code voisin, pas sur une entrée — et il vaut mieux
/// qu'un `unwrap_or` qui inventerait un candidat le jour où quelqu'un ajouterait un genre ici.
#[must_use]
pub fn epistemic(
    corpus: &[EpistemicEntry],
    clearance: Confidentiality,
    budget: usize,
) -> Vec<EpistemicHit> {
    let by_key: BTreeMap<String, RevisionId> = corpus
        .iter()
        .map(|entry| (entry.revision.to_string(), entry.revision))
        .collect();
    let candidates: Vec<Candidate> = corpus
        .iter()
        .map(|entry| {
            // Le genre se déduit ici d'`is_negative`, et le choix se dit : une entrée épistémique
            // qui ne porte pas de résultat négatif est un claim validé, c'est-à-dire `Semantic`.
            // Faire porter le genre à `EpistemicEntry` serait plus juste et demanderait de le
            // remonter jusqu'aux appelants — un item, pas une correction de passage.
            let genre = if entry.is_negative {
                Genre::Negative
            } else {
                Genre::Semantic
            };
            Candidate::new(
                entry.revision.to_string(),
                entry.classification,
                genre,
                entry.ranking.clone(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("un genre déduit d'`is_negative` n'est jamais `Formal`");

    // Le plan compatible : ces deux retrievals gardent le comportement d'avant `W17.l`, ce qui est
    // le point de `Plan::compatible` — un item additif ne change pas ce que ses appelants font.
    let plan = Plan::compatible(budget.max(1)).expect("un budget minoré à 1 est licite");
    retrieve(&plan, &candidates, clearance)
        .included()
        .iter()
        .filter_map(|found| {
            by_key.get(found.key()).map(|revision| EpistemicHit {
                revision: *revision,
                is_negative: found.is_negative(),
                ranking: found.ranking().clone(),
            })
        })
        .collect()
}

/// Chercher « qui a travaillé ».
///
/// # Panics
///
/// Jamais en pratique, et la raison est locale : le genre est **choisi ici**, parmi deux valeurs
/// dont aucune n'est `Formal`. Or `Candidate::new` ne refuse que le couple `(Formal, Vector)`.
/// Le `expect` est donc une assertion sur du code voisin, pas sur une entrée — et il vaut mieux
/// qu'un `unwrap_or` qui inventerait un candidat le jour où quelqu'un ajouterait un genre ici.
#[must_use]
pub fn organisational(
    corpus: &[OrganisationalEntry],
    clearance: Confidentiality,
    budget: usize,
) -> Vec<OrganisationalHit> {
    let by_key: BTreeMap<String, Id<Agent>> = corpus
        .iter()
        .map(|entry| (entry.agent.to_string(), entry.agent))
        .collect();
    let candidates: Vec<Candidate> = corpus
        .iter()
        .map(|entry| {
            // Le retrieval organisationnel porte sur des agents, pas sur des claims : son genre est
            // `Coordination`, sauf quand l'entrée porte un résultat négatif — « cet agent a échoué
            // ici » est un fait négatif avant d'être un fait d'organisation.
            let genre = if entry.is_negative {
                Genre::Negative
            } else {
                Genre::Coordination
            };
            Candidate::new(
                entry.agent.to_string(),
                entry.classification,
                genre,
                entry.ranking.clone(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("ni `Coordination` ni `Negative` ne sont `Formal`");

    // Le plan compatible : ces deux retrievals gardent le comportement d'avant `W17.l`, ce qui est
    // le point de `Plan::compatible` — un item additif ne change pas ce que ses appelants font.
    let plan = Plan::compatible(budget.max(1)).expect("un budget minoré à 1 est licite");
    retrieve(&plan, &candidates, clearance)
        .included()
        .iter()
        .filter_map(|found| {
            by_key.get(found.key()).map(|agent| OrganisationalHit {
                agent: *agent,
                is_negative: found.is_negative(),
                ranking: found.ranking().clone(),
            })
        })
        .collect()
}
