//! Le port d'une projection — `docs/SPEC_V1.md` §9.5.

use std::fmt;

use locus_event_store::Envelope;

/// Où en est une projection du journal — §9.5, « chaque projection expose son dernier
/// `event_sequence` appliqué ».
///
/// `0` veut dire « rien appliqué », et c'est aussi l'état d'une projection qu'on vient de
/// détruire. Les deux se ressemblent parce que ce sont la même chose : §9.5 dit qu'« une
/// projection peut être détruite et reconstruite », et une projection détruite n'a plus d'histoire
/// — elle a un point de départ.
pub type Watermark = u64;

/// Pourquoi une projection refuse un événement.
///
/// Le refus porte la **position**, parce que c'est elle qui permet de reprendre : sans elle, la
/// quarantaine de §9.5 dirait qu'une projection est cassée sans dire où.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionError {
    /// La position dans le flux global.
    pub position: u64,
    /// Ce qui n'allait pas.
    pub reason: String,
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "projection en défaut à la position {} : {}",
            self.position, self.reason
        )
    }
}

impl std::error::Error for ProjectionError {}

/// Ce qu'une projection doit savoir faire — §9.5.
///
/// # Pourquoi `reset` est dans le trait
///
/// « Une projection peut être détruite et reconstruite. » Ce n'est pas une facilité d'opérateur :
/// c'est la propriété qui rend une projection **secondaire**. Une projection qu'on ne saurait pas
/// reconstruire serait une seconde source de vérité, ce que §9.1 réserve à `PostgreSQL` et au
/// journal. La mettre dans le port oblige chaque projection à répondre à la question.
///
/// # Pourquoi `checksum`
///
/// §9.5 : « des checksums de segments détectent la corruption silencieuse ». Une projection qui ne
/// sait pas résumer son état ne peut pas être comparée à une reconstruction, et
/// [`crate::verify::verify`] n'aurait rien à comparer.
pub trait Projection {
    /// Le nom de la projection, pour les diagnostics et la quarantaine.
    fn name(&self) -> &'static str;

    /// Appliquer un événement.
    ///
    /// # Errors
    ///
    /// Rend [`ProjectionError`] quand l'événement est inapplicable. §9.5 : l'erreur met la
    /// projection en quarantaine **sans bloquer l'écriture canonique** — c'est
    /// [`crate::runner::ProjectionRunner`] qui tient cette promesse, pas cette méthode.
    fn apply(&mut self, position: u64, event: &Envelope) -> Result<(), ProjectionError>;

    /// Le dernier `event_sequence` appliqué.
    fn watermark(&self) -> Watermark;

    /// Détruire l'état et repartir de zéro.
    ///
    /// Après cet appel, [`Projection::watermark`] rend `0`. Une implémentation qui garderait quoi
    /// que ce soit rendrait la reconstruction non comparable à l'état courant — et c'est
    /// exactement ce que le test de sortie de W1.d vérifie.
    fn reset(&mut self);

    /// Un résumé de l'état, pour comparer une reconstruction à l'état courant.
    ///
    /// Deux états égaux **doivent** rendre le même résumé. Deux états différents devraient en
    /// rendre des différents ; ce n'est pas garanti par le type, et c'est pourquoi
    /// [`crate::verify::verify`] compare aussi les watermarks.
    fn checksum(&self) -> String;
}
