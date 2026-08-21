//! `critical_path_length` — la plus longue chaîne de dépendances. `W21.g`, ADR 0024.
//!
//! # Un compte d'étapes, jamais une durée
//!
//! La mesure rend le nombre d'**étapes** de la plus longue chaîne : ce qu'aucune parallélisation ne
//! raccourcira, parce que chaque étape attend la précédente. Aucune signature de ce module ne parle
//! de temps, et c'est délibéré — une durée dépendrait de ce que chaque étape coûte, que ce module
//! ne connaît pas et n'a pas à connaître.
//!
//! # Les dépendances entrent comme **données**
//!
//! Ce module ne va chercher aucun graphe. Il reçoit des couples « celui-ci avant celui-là » et
//! calcule. C'est ce qui le rend juste quelle que soit la provenance, et c'est aussi ce qui l'a fait
//! atterrir ici plutôt que dans `locus-coordination`.
//!
//! Le test de sortie de `W21.g` annonçait que le graphe de tâches était « celui de `task.rs` et
//! `barrier.rs` ». Vérification faite, il n'existe pas : `Task` porte un état et des assignations,
//! jamais de dépendance, et `Barrier` **n'expose délibérément aucun nœud** — « un accesseur qui
//! rendrait des identités ferait écrire, un jour, *barrer aussi ceux-là* ». Quant aux relations
//! `depends_on` et `blocked_by`, elles vivent dans `packages/graph`, que la sixième frontière de la
//! CI interdit au domaine de coordination d'importer.
//!
//! Construire une structure de dépendances **pour** avoir quelque chose à mesurer aurait été
//! construire une fonctionnalité afin de justifier une métrique. Recevoir la relation en données est
//! l'inverse : un sous-système complet et testé, dont l'appelant viendra quand il existera — ce que
//! la décision 0 de l'ADR 0022 appelle une capacité, et qu'elle autorise explicitement.
//!
//! Conséquence utile : `locus-evaluation` n'a **aucune** dépendance, donc l'impossibilité de
//! calculer cette métrique sur le graphe de coordination est tenue par le graphe de paquets, et non
//! par une recherche de texte. Un test lit le `Cargo.toml` et le vérifie.
//!
//! # Un cycle est refusé en le nommant, jamais parcouru
//!
//! `R3` a déjà montré qu'une version de coordination peut être cyclique, et une métrique qui ne
//! termine pas emporte son appelant. Le tri topologique s'arrête donc dès qu'il reste des nœuds
//! qu'aucun ordre ne peut placer, et le refus **liste** ces nœuds.
//!
//! Rendre un refus muet obligerait à chercher le cycle à la main dans un graphe dont on vient
//! d'apprendre qu'il en contient un — c'est-à-dire au pire moment.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Une relation de dépendance, reçue comme donnée.
///
/// Un couple `(avant, après)` se lit « `avant` doit être fini pour que `après` commence ».
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dependencies {
    edges: BTreeSet<(String, String)>,
}

impl Dependencies {
    /// Rassembler des couples.
    ///
    /// Les nœuds se déduisent des couples : déclarer une liste de nœuds à côté ferait deux vérités
    /// sur la même question, et la première dépendance vers un nœud non déclaré les ferait diverger.
    #[must_use]
    pub fn between<A, B>(pairs: impl IntoIterator<Item = (A, B)>) -> Self
    where
        A: Into<String>,
        B: Into<String>,
    {
        Self {
            edges: pairs
                .into_iter()
                .map(|(before, after)| (before.into(), after.into()))
                .collect(),
        }
    }

    /// Les nœuds impliqués, déduits des couples.
    #[must_use]
    pub fn nodes(&self) -> BTreeSet<&str> {
        let mut nodes = BTreeSet::new();
        for (before, after) in &self.edges {
            nodes.insert(before.as_str());
            nodes.insert(after.as_str());
        }
        nodes
    }

    /// Vrai quand aucune dépendance n'est déclarée.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// La plus longue chaîne — `critical_path_length` proprement dit.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::Cycle`] en **nommant** les nœuds qu'aucun ordre ne place. Une
    /// dépendance d'un nœud vers lui-même en est un.
    pub fn critical_path(&self) -> Result<CriticalPath, CriticalPathError> {
        let nodes = self.nodes();
        if nodes.is_empty() {
            return Ok(CriticalPath { steps: 0 });
        }

        let mut waiting_on: BTreeMap<&str, usize> = nodes.iter().map(|node| (*node, 0)).collect();
        let mut unlocks: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        // Tout passe par `entry`, jamais par `get_mut(..).expect(..)`. Les deux sont équivalents ici
        // — les nœuds viennent des couples, donc la clé existe toujours — mais l'un porte un chemin
        // de panique et l'autre non. Le documenter par une section `# Panics` décrirait un état que
        // rien ne peut produire, ce que `W21.f` a déjà refusé pour une variante d'énumération : une
        // panique inexprimable vaut mieux qu'une panique documentée.
        for (before, after) in &self.edges {
            unlocks
                .entry(before.as_str())
                .or_default()
                .push(after.as_str());
            *waiting_on.entry(after.as_str()).or_insert(0) += 1;
        }

        // Tri topologique de Kahn, **itératif** : une version récursive déborderait la pile
        // exactement sur les graphes profonds, c'est-à-dire ceux dont le chemin critique est long.
        let mut ready: Vec<&str> = waiting_on
            .iter()
            .filter(|(_, waiting)| **waiting == 0)
            .map(|(node, _)| *node)
            .collect();
        let mut steps: BTreeMap<&str, usize> = nodes.iter().map(|node| (*node, 1)).collect();
        let mut placed = 0_usize;

        while let Some(node) = ready.pop() {
            placed += 1;
            let reached = steps.get(node).copied().unwrap_or(1);
            for next in unlocks.get(node).map(Vec::as_slice).unwrap_or_default() {
                let step = steps.entry(next).or_insert(1);
                // `max`, et non une affectation : un nœud atteint par plusieurs prédécesseurs
                // prendrait sinon la profondeur du **dernier traité**, et la valeur dépendrait de
                // l'ordre d'itération. Un mutant qui retire ce `max` a survécu jusqu'à ce qu'un test
                // fasse converger deux branches de longueurs différentes.
                *step = (*step).max(reached + 1);
                let waiting = waiting_on.entry(next).or_insert(0);
                *waiting = waiting.saturating_sub(1);
                if *waiting == 0 {
                    ready.push(next);
                }
            }
        }

        if placed < nodes.len() {
            return Err(CriticalPathError::Cycle {
                members: waiting_on
                    .iter()
                    .filter(|(_, waiting)| **waiting > 0)
                    .map(|(node, _)| (*node).to_owned())
                    .collect(),
            });
        }

        Ok(CriticalPath {
            steps: steps.values().copied().max().unwrap_or(0),
        })
    }
}

/// La plus longue chaîne, en étapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CriticalPath {
    steps: usize,
}

impl CriticalPath {
    /// Le nombre d'étapes.
    ///
    /// Un nœud sans dépendance vaut **une** étape : il faut bien le faire. Zéro n'est rendu que
    /// lorsqu'il n'y a aucun nœud.
    #[must_use]
    pub const fn steps(self) -> usize {
        self.steps
    }
}

impl fmt::Display for CriticalPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} étapes", self.steps)
    }
}

/// Pourquoi la plus longue chaîne ne se calcule pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CriticalPathError {
    /// Des nœuds qu'aucun ordre ne place — ils dépendent, directement ou non, d'eux-mêmes.
    Cycle {
        /// Lesquels.
        members: Vec<String>,
    },
}

impl fmt::Display for CriticalPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cycle { members } => write!(
                formatter,
                "aucun ordre ne place {} : {}. Une chaîne n'a pas de longueur dans un cycle, et la \
                 parcourir ne terminerait pas",
                members.len(),
                members.join(", ")
            ),
        }
    }
}

impl std::error::Error for CriticalPathError {}
