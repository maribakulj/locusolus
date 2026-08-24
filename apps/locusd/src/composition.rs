//! Le composition root — `W20.d`. Ce qui câble, et rien qui décide.
//!
//! # Ce qu'un composition root est, et ce qu'il ne doit pas devenir
//!
//! Il assemble : journal, transaction, projections, moteur de politique. Il ne contient **aucune**
//! règle métier, et c'est ce qui le rend relisable — un lecteur qui veut savoir « qu'est-ce qui est
//! branché à quoi » lit ce fichier et n'a pas besoin d'en lire un autre.
//!
//! La tentation qu'il faut nommer pour l'écarter : un composition root finit toujours par recevoir
//! « juste une petite décision », parce qu'il est le seul endroit qui voit tout. C'est précisément
//! pour cela qu'il ne doit rien décider — le seul endroit qui voit tout est le pire endroit pour
//! cacher une règle.
//!
//! # Le sens de lecture, imposé par les types
//!
//! [`Transaction`] possède le journal (`W20.b`). Les projections ont besoin de le **lire**, et le
//! seul accès qui existe est [`Transaction::store`], qui rend une référence partagée. Il n'y a donc
//! aucun assemblage possible où une projection écrirait : non parce que le composition root fait
//! attention, mais parce que la seule poignée qu'il puisse leur passer est immuable.
//!
//! # Le composition root ne connaît pas le transport
//!
//! `W20.d` l'a livré sans surface HTTP ; `W20.g` a donné cette surface au binaire, dans
//! `crate::http`. Ce module n'en sait rien et n'a pas à en savoir : il assemble le domaine, et
//! `main` décide s'il l'expose. C'est pour cela que [`Readiness`] ne dit plus un mot du transport —
//! elle l'a fait tant que la réponse était « aucun », et cette phrase est devenue fausse le jour où
//! le port s'est ouvert. Un rapport qui affirme ce que son émetteur ne peut pas savoir finit
//! toujours par mentir ; celui-ci ne parle que des projections, qu'il tient.

use std::fmt;

use locus_event_store::{EventStore, MemoryEventStore};
use locus_policy::{Facts, Policy, Run};
use locus_projections::{
    ConflictRegistry, ExecutionGraph, Health, OrganisationGraph, ProjectionRunner, ValidationState,
};

use crate::transaction::Transaction;

/// Le daemon assemblé — §4.1, l'autorité transactionnelle et ce qu'elle alimente.
///
/// Générique sur le journal : `W20.d` assemble avec [`MemoryEventStore`], parce que le driver
/// `PostgreSQL` n'existe pas encore et que l'ADR 0012 a posé « le port avant le driver ». Le jour où
/// il existe, il se substitue **sans toucher à ce fichier** — c'est la seule chose que le paramètre
/// de type est là pour garantir.
pub struct Runtime<S> {
    transaction: Transaction<S>,
    execution: ProjectionRunner<ExecutionGraph>,
    organisation: ProjectionRunner<OrganisationGraph>,
    conflicts: ProjectionRunner<ConflictRegistry>,
    validation: ProjectionRunner<ValidationState>,
    policy: Policy,
    readiness: Readiness,
    lep: crate::lep::Desk,
}

impl Runtime<MemoryEventStore> {
    /// L'assemblage du profil `personal-local` (`docs/05`) : tout en mémoire, rien à installer.
    #[must_use]
    pub fn in_memory() -> Self {
        Self::assemble(MemoryEventStore::new(), Policy::new())
    }
}

impl<S: EventStore> Runtime<S> {
    /// Câbler un journal et un jeu de politiques.
    ///
    /// Les quatre projections de §9.5 sont enregistrées ici, en dur. Une table de configuration qui
    /// permettrait d'en omettre une laisserait démarrer un daemon dont une projection manque, et
    /// personne ne s'en apercevrait avant qu'une query rende un résultat vide plutôt qu'une erreur.
    pub fn assemble(store: S, policy: Policy) -> Self {
        Self {
            transaction: Transaction::new(store),
            execution: ProjectionRunner::new(ExecutionGraph::new()),
            organisation: ProjectionRunner::new(OrganisationGraph::new()),
            conflicts: ProjectionRunner::new(ConflictRegistry::new()),
            validation: ProjectionRunner::new(ValidationState::new()),
            policy,
            readiness: Readiness {
                projections: Vec::new(),
            },
            // Une file vide et un registre vide : un daemon sans ordonnanceur répond `204` à toute
            // réclamation et refuse toute créance, ce qui est exact. `W20.k` livre les ports ; ce
            // qui les remplira est `W23.c`, nommé plutôt que simulé ici.
            lep: crate::lep::Desk::default(),
        }
    }

    /// Substituer les ports de §15.2 — la file de missions et le registre de workers.
    ///
    /// Consomme et rend le runtime plutôt que de muter : un daemon dont la file changerait en cours
    /// de route servirait deux ordonnanceurs sans que rien le dise.
    #[must_use]
    pub fn with_lep(mut self, desk: crate::lep::Desk) -> Self {
        self.lep = desk;
        self
    }

    /// Les ports de §15.2, en lecture — `W20.k`.
    pub const fn lep(&self) -> &crate::lep::Desk {
        &self.lep
    }

    /// La transaction, seul chemin d'écriture — `W20.b`.
    ///
    /// `&self` depuis `W20.h` : la couche HTTP ne tient qu'un `&Runtime` partagé, et exiger `&mut`
    /// ici est ce qui rendait §22.3 inservable. Ce n'est pas un relâchement de la règle de `W20.b` —
    /// la transaction reste le seul chemin d'écriture, et c'est elle qui sérialise, par stream.
    pub const fn transaction(&self) -> &Transaction<S> {
        &self.transaction
    }

    /// Le moteur de politique, en lecture. Il ne décide qu'à partir des faits qu'on lui donne.
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }

    /// Le journal, en lecture — pour les queries de §22.4.
    ///
    /// `pub(crate)` et non `pub` : `W20.b` a fait de la transaction le seul chemin d'écriture, et
    /// exposer le journal au-dehors, même en lecture, inviterait à contourner les queries qui le
    /// présentent. Ce qui sort de ce crate est une page, pas un journal.
    pub(crate) const fn transaction_store(&self) -> &S {
        self.transaction.store()
    }

    /// Le graphe d'exécution, en lecture — §9.5.
    ///
    /// Les projections sortent **en lecture seule**, comme le journal. `W20.e` servira les queries
    /// de §22.4 depuis ces accesseurs ; leur donner une variante mutable ouvrirait un chemin où une
    /// query modifierait ce qu'elle lit.
    pub const fn execution_graph(&self) -> &ExecutionGraph {
        self.execution.projection()
    }

    /// Le graphe d'organisation, en lecture.
    pub const fn organisation_graph(&self) -> &OrganisationGraph {
        self.organisation.projection()
    }

    /// Le registre des conflits, en lecture — invariant 12 : rien n'y est supprimé.
    pub const fn conflict_registry(&self) -> &ConflictRegistry {
        self.conflicts.projection()
    }

    /// L'état de validation, en lecture — §8.1.
    pub const fn validation_state(&self) -> &ValidationState {
        self.validation.projection()
    }

    /// Évaluer une politique sans droit d'agir — §20.2, le chemin `dry`.
    ///
    /// Exposé depuis le composition root parce que c'est lui qui détient le jeu de règles ; le
    /// calcul, lui, est celui de `locus-policy` et n'est pas redit ici.
    #[must_use]
    pub fn simulate(&self, facts: &Facts) -> locus_policy::Simulation {
        Run::dry(&self.policy, facts)
    }

    /// Faire rattraper les quatre projections depuis le journal.
    ///
    /// Aucune ne peut faire échouer cet appel : une faute met la projection en quarantaine (§9.5) et
    /// se lit dans le rapport. C'est la promesse de `W1.d` — une projection fautive ne bloque pas
    /// l'écriture canonique — et le composition root serait le seul endroit d'où l'on pourrait la
    /// trahir, en propageant l'erreur.
    pub fn catch_up(&mut self) -> Readiness {
        let store = self.transaction.store();
        let readiness = Readiness {
            projections: vec![
                wired(&self.execution.catch_up(store).health, ExecutionGraph::NAME),
                wired(
                    &self.organisation.catch_up(store).health,
                    OrganisationGraph::NAME,
                ),
                wired(
                    &self.conflicts.catch_up(store).health,
                    ConflictRegistry::NAME,
                ),
                wired(
                    &self.validation.catch_up(store).health,
                    ValidationState::NAME,
                ),
            ],
        };
        self.readiness = readiness.clone();
        readiness
    }

    /// Le dernier rapport de disponibilité, **sans rattraper**.
    ///
    /// La liaison HTTP ne tient qu'un `&Runtime` : elle ne peut pas rattraper, et c'est voulu — une
    /// query qui ferait avancer les projections rendrait le résultat dépendant de qui a lu en
    /// dernier. Elle lit donc le dernier rapport connu, et un assemblage qui n'a jamais rattrapé
    /// rend une liste **vide** plutôt qu'un « prêt » supposé.
    #[must_use]
    pub fn readiness(&self) -> Readiness {
        self.readiness.clone()
    }
}

/// Ce que l'assemblage a produit — de quoi diagnostiquer sans ouvrir un débogueur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readiness {
    /// Une ligne par projection câblée.
    pub projections: Vec<Wired>,
}

impl Readiness {
    /// Vrai quand les quatre projections sont saines.
    ///
    /// « Prêt » ne veut pas dire « sans quarantaine » par commodité de nommage : une projection en
    /// quarantaine sert des lectures périmées, et un daemon qui se dirait prêt dans cet état ferait
    /// exactement la promesse qu'il ne tient pas.
    ///
    /// **Une liste vide n'est pas prête**, et la précision n'est pas théorique : `all()` sur un
    /// itérateur vide rend `true`, si bien qu'un assemblage qui n'a **jamais rattrapé** se serait
    /// déclaré disponible avec zéro projection câblée. C'est le même mensonge que la quarantaine,
    /// obtenu par un chemin plus discret.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        !self.projections.is_empty() && self.projections.iter().all(|wired| wired.healthy)
    }

    /// Les projections en quarantaine, nommément.
    #[must_use]
    pub fn quarantined(&self) -> Vec<&str> {
        self.projections
            .iter()
            .filter(|wired| !wired.healthy)
            .map(|wired| wired.name)
            .collect()
    }
}

/// L'état d'une projection au terme de l'assemblage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wired {
    /// Son nom, tel que la projection le donne.
    pub name: &'static str,
    /// Saine, ou en quarantaine.
    pub healthy: bool,
}

fn wired(health: &Health, name: &'static str) -> Wired {
    Wired {
        name,
        healthy: matches!(health, Health::Healthy),
    }
}

impl fmt::Display for Readiness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "locusd — {} projection(s) câblée(s)",
            self.projections.len()
        )?;
        for wired in &self.projections {
            let state = if wired.healthy {
                "saine"
            } else {
                "EN QUARANTAINE"
            };
            writeln!(formatter, "  {} : {state}", wired.name)?;
        }
        write!(
            formatter,
            "  prêt : {}",
            if self.is_ready() { "oui" } else { "non" }
        )
    }
}

/// Le nom d'une projection, sans en instancier une.
///
/// `Projection::name` est une méthode d'instance, et le rapport en a besoin avant d'avoir la main
/// sur l'instance — celle-ci est enfermée dans son pilote. Une constante par type évite de
/// réécrire les noms à la main dans le rapport, ce qui les ferait diverger au premier renommage.
trait Named {
    const NAME: &'static str;
}

macro_rules! named {
    ($type:ty, $name:literal) => {
        impl Named for $type {
            const NAME: &'static str = $name;
        }
    };
}

named!(ExecutionGraph, "execution_graph");
named!(OrganisationGraph, "organisation_graph");
named!(ConflictRegistry, "conflict_registry");
named!(ValidationState, "validation_state");

/// Les constantes ci-dessus doivent dire ce que les projections disent d'elles-mêmes.
///
/// Deux sources pour un même nom divergent au premier renommage, et le rapport nommerait alors une
/// projection qui n'existe plus. Ce test est le seul endroit qui les compare.
#[cfg(test)]
mod tests {
    use super::{ConflictRegistry, ExecutionGraph, Named, OrganisationGraph, ValidationState};
    use locus_projections::Projection;

    #[test]
    fn les_noms_du_rapport_sont_ceux_des_projections() {
        assert_eq!(ExecutionGraph::new().name(), ExecutionGraph::NAME);
        assert_eq!(OrganisationGraph::new().name(), OrganisationGraph::NAME);
        assert_eq!(ConflictRegistry::new().name(), ConflictRegistry::NAME);
        assert_eq!(ValidationState::new().name(), ValidationState::NAME);
    }
}
