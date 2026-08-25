//! La souscription **dérivée** de la `ContextView` — `W24.a`, ADR 0026 décision 4.
//!
//! # La borne que la source omet
//!
//! Un mécanisme de publication-souscription sémantique peut **choisir** un destinataire dans un
//! ensemble autorisé ; il ne détermine **jamais** l'autorisation. La source dont l'ADR 0026 tire ce
//! mécanisme fait aligner la souscription sur « le *system prompt* configuré quand chaque agent
//! rejoint le réseau » — c'est-à-dire que l'agent déclare lui-même ce qu'il veut recevoir.
//!
//! Transposé ici, cela ferait négocier aux agents leur propre accès à l'information, ce qui casse
//! d'un seul geste l'isolation informationnelle du worker (`repos/canterel/SPEC_V1.md` §12.4),
//! l'invariant 11 — aveuglement du reviewer — et §16.6.
//!
//! # Une seule porte d'entrée, et c'est le type qui la tient
//!
//! [`Subscription`] n'a **qu'un** constructeur, [`Subscription::derived_from`], et il prend une
//! [`ContextView`]. Pas de `new`, pas de `Default`, pas de champ public, pas de `with_*`, aucun
//! chemin de désérialisation — `packages/review` ne dépend de `serde` sous aucune forme, donc il n'y
//! existe aucun type désérialisable, donc a fortiori aucun qui laisserait un agent en fabriquer une.
//!
//! La propriété voulue n'est pas « personne n'a encore écrit sa souscription » — ça se relit à chaque
//! revue — mais « personne ne **peut** ». C'est le même arbitrage que `W23.a`, dont le test lit
//! `Cargo.toml` plutôt que de chercher un `derive` oublié.
//!
//! # Élargir passe par ailleurs, et les deux chemins ne se rejoignent pas
//!
//! Un agent qui veut davantage émet `context.extension_requested` — la demande existe, côté worker,
//! et son module le dit lui-même : « il n'existe volontairement aucune fonction qui l'accorde : la
//! décision n'appartient pas au worker, et offrir un `grantExtension()` local serait offrir le moyen
//! de contourner exactement ce que §12.4 protège ».
//!
//! Ce module tient l'autre moitié de la même promesse. Il n'expose **aucune** fonction qui prendrait
//! une demande et rendrait une souscription plus large. Le seul moyen d'élargir est de reconstruire
//! la `ContextView`, ce qui exige les candidats et le destinataire — deux choses que l'agent ne
//! fournit pas. La demande est donc une **entrée du plan de contrôle**, jamais une entrée de la
//! souscription, et un test exhibe les deux chemins pour montrer qu'ils ne se touchent pas.
//!
//! Et la demande n'est pas dupliquée ici : `CLAUDE.md` refuse la duplication cross-repo des
//! contrats, et un jumeau Rust de `context.extension_requested` serait exactement ça. Ce qui traverse
//! est un événement, pas un type partagé.

use std::collections::BTreeSet;

use locus_domain::{Confidentiality, ContentHash, RevisionId};

use crate::contamination::rank;
use crate::context_view::ContextView;

/// Ce qu'un agent a le droit de recevoir.
///
/// # Pourquoi elle porte le condensat de sa vue
///
/// Une souscription sans provenance ne se vérifie pas : deux vues différentes peuvent inclure les
/// mêmes révisions à un instant donné, et confondre les souscriptions qui en dérivent ferait survivre
/// une autorisation à la vue qui la fondait. Le condensat rend la question décidable — celle-ci
/// vient-elle bien de cette vue-là.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    revisions: BTreeSet<RevisionId>,
    ceiling: Confidentiality,
    watermark: u64,
    view: ContentHash,
}

impl Subscription {
    /// Dériver la souscription d'une vue.
    ///
    /// **L'unique constructeur.** Tout le reste de ce type est en lecture, et c'est la forme que
    /// prend ici « la souscription n'est jamais déclarée par l'agent » : il n'y a pas de règle à
    /// respecter, il n'y a rien d'autre à appeler.
    #[must_use]
    pub fn derived_from(view: &ContextView) -> Self {
        Self {
            revisions: view.included().iter().copied().collect(),
            ceiling: view.confidentiality_ceiling(),
            watermark: view.source_event_watermark(),
            view: view.content_hash().clone(),
        }
    }

    /// Cette révision est-elle souscrite ?
    #[must_use]
    pub fn admits(&self, revision: &RevisionId) -> bool {
        self.revisions.contains(revision)
    }

    /// Ce niveau de confidentialité passe-t-il le plafond ?
    ///
    /// Un plafond, pas une liste : §16.2 porte `confidentiality_ceiling`, et un ordre croissant de
    /// sensibilité rend la comparaison décidable sans énumération exhaustive.
    ///
    /// L'ordre vient de [`crate::contamination`], qui le porte déjà et dit pourquoi il n'en faut
    /// qu'une copie : « un `match` recopié ailleurs finit par en changer l'ordre sans qu'on s'en
    /// aperçoive ». Le recopier ici aurait produit exactement ce que cette phrase annonce.
    #[must_use]
    pub fn clears(&self, level: Confidentiality) -> bool {
        rank(level) <= rank(self.ceiling)
    }

    /// Les révisions souscrites.
    #[must_use]
    pub const fn revisions(&self) -> &BTreeSet<RevisionId> {
        &self.revisions
    }

    /// Le plafond hérité de la vue.
    #[must_use]
    pub const fn ceiling(&self) -> Confidentiality {
        self.ceiling
    }

    /// L'instant du journal auquel la vue était arrêtée.
    ///
    /// Une souscription ne voit pas plus loin que sa vue : §16.2 refuse qu'une vue contienne
    /// l'avenir, et une souscription qui le ferait rendrait ce refus inopérant d'un cran plus bas.
    #[must_use]
    pub const fn watermark(&self) -> u64 {
        self.watermark
    }

    /// Le condensat de la vue dont elle dérive.
    #[must_use]
    pub const fn view(&self) -> &ContentHash {
        &self.view
    }
}
