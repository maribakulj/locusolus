//! Les queries de `SPEC_V1.md` §22.4, paginées par les cursors de §22.6.
//!
//! # Trois collections, et pas dix-neuf
//!
//! §22.4 énumère dix-neuf routes. Trois sont servies ici — celles qui ont de quoi l'être : le flux
//! du journal, les workers vus par le graphe d'exécution, les conflits ouverts. Les seize autres
//! attendent leurs agrégats, et les déclarer ferait passer pour servies des collections vides.
//! C'est la règle de `CLAUDE.md` sur les énumérations, appliquée aux routes comme aux verbes.
//!
//! # « Stable dans une fenêtre cohérente »
//!
//! §22.6 demande des cursors stables. Une pagination est stable quand une insertion ne déplace pas
//! ce qui précède — sans quoi la page 2 saute ou répète des éléments selon ce qui est arrivé entre
//! les deux appels, et le client ne peut pas s'en apercevoir.
//!
//! Deux façons de l'obtenir, et les deux sont ici :
//!
//! - le **flux** est indexé par sa position globale, qui ne bouge jamais : un événement écrit après
//!   la page 1 porte une position plus grande, donc apparaît après ;
//! - les **projections** sont des ensembles, sans ordre naturel. Elles sont donc paginées dans un
//!   ordre **canonique** — le tri lexicographique de leur clé — et le cursor porte un rang dans cet
//!   ordre. Une insertion peut décaler ce qui suit, jamais ce qui précède, tant que la clé insérée
//!   est nouvelle ; c'est la fenêtre de cohérence que §22.6 nomme, et ce module ne prétend pas à
//!   davantage.

use locus_event_store::{EventStore, Sequenced};
use locus_projections::{ConflictEntry, NodeKind};

use crate::composition::Runtime;
use locus_domain::RevisionId;
use locus_projections::Dossier;

use crate::cursor::{Collection, Cursor, CursorError};

/// Le nombre d'éléments qu'une page rend par défaut.
///
/// Une constante et non un défaut implicite : une page sans borne est une page qui rend le journal
/// entier le jour où il grossit, et personne ne l'a demandé.
pub const DEFAULT_LIMIT: usize = 50;

/// Le plafond qu'un client ne peut pas dépasser, quoi qu'il demande.
pub const MAX_LIMIT: usize = 500;

/// Une page de résultats, et de quoi demander la suivante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    /// Les éléments, dans l'ordre de la collection.
    pub items: Vec<T>,
    /// Le cursor de la page suivante, ou `None` s'il n'y a plus rien.
    ///
    /// `None` **signifie la fin**, et non « redemandez pour voir » : un client qui reçoit `None`
    /// s'arrête. Rendre un cursor jusqu'à l'infini ferait boucler tout client qui suit le contrat.
    pub next: Option<Cursor>,
}

impl<T> Page<T> {
    /// Vrai quand cette page est la dernière.
    #[must_use]
    pub const fn is_last(&self) -> bool {
        self.next.is_none()
    }
}

/// Ce qu'une entrée de timeline rend — §22.4, `GET /timeline`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    /// Sa position globale, celle que le cursor suit.
    pub position: u64,
    /// Le type d'événement, tel que §10.3 le nomme.
    pub event_type: String,
    /// Le stream d'où il vient.
    pub stream_id: String,
}

impl From<&Sequenced> for TimelineEntry {
    fn from(sequenced: &Sequenced) -> Self {
        Self {
            position: sequenced.position,
            event_type: sequenced.event.event_type.to_string(),
            stream_id: sequenced.event.stream_id.clone(),
        }
    }
}

/// Une borne de page, bornée par [`MAX_LIMIT`].
///
/// Le plafond s'applique en silence plutôt qu'en refus : un client qui demande mille éléments veut
/// des éléments, pas une erreur, et la page rend son propre `next` — il aura la suite.
#[must_use]
fn bounded(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

impl<S: EventStore> Runtime<S> {
    /// `GET /timeline` — le flux du journal, repris depuis une séquence connue (§22.6).
    ///
    /// # Errors
    ///
    /// [`CursorError`] si le cursor n'a pas été émis ici, ou vient d'une autre collection.
    pub fn timeline(
        &self,
        after: Option<&Cursor>,
        limit: Option<usize>,
    ) -> Result<Page<TimelineEntry>, CursorError> {
        let from = position_of(after, Collection::Timeline)?;
        let limit = bounded(limit);

        let feed = self.transaction_store().feed(from);
        let items: Vec<TimelineEntry> = feed.iter().take(limit).map(TimelineEntry::from).collect();
        let next = (feed.len() > limit)
            .then(|| {
                items
                    .last()
                    .map(|last| Cursor::issue(Collection::Timeline, last.position))
            })
            .flatten();

        Ok(Page { items, next })
    }

    /// `GET /workers` — les workers que le graphe d'exécution connaît.
    ///
    /// # Errors
    ///
    /// [`CursorError`], comme ci-dessus.
    pub fn workers(
        &self,
        after: Option<&Cursor>,
        limit: Option<usize>,
    ) -> Result<Page<String>, CursorError> {
        let rank = position_of(after, Collection::Workers)?;
        let mut keys: Vec<String> = self.with_execution_graph(|graph| {
            graph
                .of_kind(NodeKind::Worker)
                .into_iter()
                .cloned()
                .collect()
        });
        keys.sort_unstable();
        Ok(paginate(keys, rank, bounded(limit), Collection::Workers))
    }

    /// Les conflits **ouverts** — §9.4, et l'invariant 12 : aucun n'est supprimé pour faire propre.
    ///
    /// # Errors
    ///
    /// [`CursorError`], comme ci-dessus.
    pub fn open_conflicts(
        &self,
        after: Option<&Cursor>,
        limit: Option<usize>,
    ) -> Result<Page<ConflictEntry>, CursorError> {
        let rank = position_of(after, Collection::Conflicts)?;
        let mut entries: Vec<ConflictEntry> =
            self.with_conflict_registry(|registry| registry.open().into_iter().cloned().collect());
        entries.sort_by(|left, right| {
            left.stream_id
                .cmp(&right.stream_id)
                .then_with(|| left.declared_at.cmp(&right.declared_at))
        });
        Ok(paginate(
            entries,
            rank,
            bounded(limit),
            Collection::Conflicts,
        ))
    }

    /// `GET /graph/{revision_id}` — le dossier épistémique d'une conclusion, §9.4, `W20.u`.
    ///
    /// # Les six termes, et pourquoi ils sortent ensemble
    ///
    /// « Le graphe rend la conclusion, ses prémisses, son expérience, ses artefacts, ses objections
    /// et son coût. » Les rendre par six requêtes laisserait un lecteur en composer cinq et oublier
    /// la sixième — et celle qu'on oublie est toujours la même : les objections. L'invariant 12 est
    /// mieux tenu par une réponse qui les porte que par une route qu'il faut penser à appeler.
    ///
    /// # Ce que cette query ne fait pas
    ///
    /// Elle ne rattrape pas les projections. `W20.l` a placé le rattrapage dans le chemin
    /// d'écriture, et une lecture qui ferait avancer l'état rendrait le résultat dépendant de qui a
    /// lu en dernier.
    ///
    /// # Errors
    ///
    /// [`CommandError::Validation`] quand l'identifiant n'est pas une révision lisible. **Jamais
    /// « introuvable »** : une conclusion que rien ne soutient est une réponse de §9.4, et la
    /// changer en `404` ferait relancer sa requête à qui vient précisément d'apprendre quelque
    /// chose.
    pub fn epistemic_dossier(
        &self,
        revision_id: &str,
    ) -> Result<Dossier, crate::error::CommandError> {
        let conclusion = RevisionId::parse(revision_id).map_err(|erreur| {
            crate::error::CommandError::Validation {
                field: "revision_id".to_owned(),
                detail: format!(
                    "« {revision_id} » n'est pas un identifiant de révision : {erreur}"
                ),
            }
        })?;
        Ok(self.with_epistemic_graph(|graph| graph.dossier(&conclusion)))
    }
}

/// La position que ce cursor désigne, ou `0` quand il n'y en a pas.
///
/// « Pas de cursor » veut dire « depuis le début », et non « depuis n'importe où » : un appel sans
/// cursor est une première page, jamais une reprise implicite.
fn position_of(cursor: Option<&Cursor>, collection: Collection) -> Result<u64, CursorError> {
    cursor.map_or(Ok(0), |cursor| cursor.read(collection))
}

/// Paginer une collection ordonnée par le **rang** déjà consommé.
///
/// Le rang plutôt que la clé : deux éléments peuvent partager une clé dans une projection qui
/// n'impose pas l'unicité, et reprendre « après la clé K » en sauterait un. Le rang ne saute rien
/// tant que l'ordre est canonique — ce que les appelants garantissent en triant avant d'appeler.
fn paginate<T>(items: Vec<T>, rank: u64, limit: usize, collection: Collection) -> Page<T> {
    let skipped = usize::try_from(rank).unwrap_or(usize::MAX);
    let total = items.len();
    let page: Vec<T> = items.into_iter().skip(skipped).take(limit).collect();
    let consumed = skipped.saturating_add(page.len());
    let next = (consumed < total).then(|| Cursor::issue(collection, consumed as u64));
    Page { items: page, next }
}
