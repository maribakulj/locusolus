//! L'appariement sémantique **dans** l'ensemble autorisé — `W24.b`, ADR 0026 décision 4.
//!
//! # Ce que la décision 4 sépare, et que ce module rend inséparable
//!
//! « Un mécanisme sémantique peut **choisir** un destinataire dans un ensemble autorisé ; il ne
//! détermine **jamais** l'autorisation. » `W24.a` a livré la moitié amont — la souscription dérive de
//! la `ContextView` et d'elle seule. Reste la moitié aval : que l'appariement, si intelligent
//! soit-il, ne puisse pas ajouter un pair.
//!
//! # Tenu par la signature, et pas par une garde
//!
//! [`Audience::best`] rend une référence **empruntée à l'audience**. Un pair non autorisé n'a donc
//! aucune façon de sortir de cette fonction : il faudrait qu'il soit déjà dedans. Ce n'est pas une
//! vérification qu'on pourrait oublier d'écrire, c'est une conséquence du type de retour.
//!
//! La fonction d'affinité, elle, ne reçoit qu'un `&Peer` et rend un score. Elle ne voit pas
//! l'audience, elle n'a rien à quoi l'ajouter, et son type ne lui laisse aucun moyen d'en produire
//! un membre. C'est le sens exact de « calculé **avant** l'appariement » : l'ensemble est clos quand
//! le score commence.
//!
//! # Pourquoi l'affinité n'est pas dans ce module
//!
//! Aucune sémantique n'est écrite ici : pas d'embedding, pas de similarité, pas de seuil. Le
//! mécanisme sémantique est un **paramètre**, et ce module est la borne qu'on lui pose. Écrire une
//! mesure d'affinité maintenant reviendrait à décréter ce que `W23.d` doit mesurer, et à faire entrer
//! dans le domaine un vocabulaire que personne n'a encore eu besoin de lire.
//!
//! # L'aveuglement ne se négocie pas non plus
//!
//! Apparier ne change ni l'aveuglement d'un relecteur ni son indépendance : ce sont des propriétés du
//! **dossier** et des parties, constatées par [`crate::review::attest`]. Ce module ne les touche pas —
//! il ne porte aucune fonction qui les prenne, et l'attestation rendue après un appariement est celle
//! qu'on aurait obtenue sans lui. Un test l'exerce sur une revue indépendante réelle.

use crate::contamination::Recipient;
use crate::subscription::Subscription;

/// Un pair candidat : à qui l'on pourrait s'adresser, et ce qu'il a le droit de recevoir.
///
/// Les deux ensemble, jamais l'un sans l'autre. Un pair sans souscription serait un destinataire dont
/// personne n'a établi l'autorisation, et c'est exactement ce que la décision 4 refuse de rendre
/// exprimable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    recipient: Recipient,
    subscription: Subscription,
}

impl Peer {
    /// Admettre un pair, avec la souscription qui l'autorise.
    ///
    /// La souscription vient de `W24.a` : elle **dérive** d'une `ContextView`, et il n'existe aucun
    /// autre moyen d'en obtenir une. Construire un `Peer` suppose donc qu'une autorisation a déjà
    /// été établie ailleurs, par le plan de contrôle.
    #[must_use]
    pub const fn authorised(recipient: Recipient, subscription: Subscription) -> Self {
        Self {
            recipient,
            subscription,
        }
    }

    /// Le destinataire.
    #[must_use]
    pub const fn recipient(&self) -> &Recipient {
        &self.recipient
    }

    /// Ce qu'il a le droit de recevoir.
    #[must_use]
    pub const fn subscription(&self) -> &Subscription {
        &self.subscription
    }
}

/// L'ensemble autorisé, clos avant tout appariement.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Audience {
    members: Vec<Peer>,
}

impl Audience {
    /// Clore un ensemble de pairs autorisés.
    ///
    /// **Le seul moment où l'on ajoute.** Après cet appel l'audience est en lecture : rien n'y
    /// insère, et c'est ce qui donne son sens à « calculé avant l'appariement ».
    #[must_use]
    pub fn of(members: Vec<Peer>) -> Self {
        Self { members }
    }

    /// Les pairs autorisés.
    #[must_use]
    pub fn members(&self) -> &[Peer] {
        &self.members
    }

    /// Combien.
    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Vrai quand personne n'est autorisé.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Choisir le pair le mieux apparié, **parmi ceux-ci**.
    ///
    /// # Ce que la signature garantit
    ///
    /// Le retour emprunte à `self`. Un pair qui n'est pas dans l'audience ne peut donc pas sortir
    /// d'ici, quelle que soit la fonction d'affinité — elle ne voit qu'un `&Peer` à la fois, et rend
    /// un nombre.
    ///
    /// # L'affinité ne peut pas non plus élargir ce qu'un pair reçoit
    ///
    /// Elle départage des destinataires ; elle ne dit rien de ce qu'ils ont le droit de lire. Ce
    /// dernier point reste celui de la souscription, et `W24.a` le tient.
    ///
    /// # Départage
    ///
    /// À score égal, le **premier** de l'audience — l'appariement doit être reproductible, sans quoi
    /// deux rejeux du même journal choisiraient différemment et la trace ne dirait plus ce qui s'est
    /// passé. Même arbitrage que `place` de `W4.g`, et pour la même raison.
    #[must_use]
    pub fn best<F: Fn(&Peer) -> i64>(&self, affinity: F) -> Option<&Peer> {
        self.members
            .iter()
            .enumerate()
            .max_by_key(|(rank, peer)| {
                (
                    affinity(peer),
                    // Rang décroissant : à score égal, le plus petit rang gagne.
                    std::cmp::Reverse(*rank),
                )
            })
            .map(|(_, peer)| peer)
    }
}
