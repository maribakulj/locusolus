//! Le fil d'événements clients — `SPEC_V1.md` §22.1, « événements clients : WebSocket/SSE avec
//! cursor », et la coalescence de §18.3.
//!
//! # SSE, et non WebSocket — la condition 4 de l'ADR 0018, tranchée
//!
//! L'ADR 0018 a laissé le choix ouvert à dessein, en mesurant ce qu'il met en jeu : la feature `ws`
//! d'`axum` coûte **vingt paquets** — `tungstenite`, `sha1`, `rand` et leurs tours de traits, parce
//! que le handshake WebSocket exige un SHA-1 et un générateur aléatoire. La question était de savoir
//! si le besoin les justifie.
//!
//! Il ne les justifie pas, et pour une raison de forme plutôt que de coût :
//!
//! 1. **Le fil client est unidirectionnel.** §22.1 fait voyager les commandes par JSON-RPC ou des
//!    endpoints typés, et les queries par REST. Ce qui reste pour ce fil est serveur → client. La
//!    bidirectionnalité de WebSocket serait payée et inutilisée.
//! 2. **SSE porte déjà la reprise.** `Last-Event-ID` est renvoyé par le client à la reconnexion, et
//!    c'est exactement « reprend depuis sa séquence » — la clause que `W20.f` demande. Avec
//!    WebSocket il faudrait la réinventer dans le protocole applicatif, donc l'écrire et la tester ;
//!    ici elle est dans la norme, et les navigateurs la mettent en œuvre.
//!
//! Le coût de sortie reste borné : le jour où un besoin bidirectionnel apparaît — une commande sur
//! le même socket — la décision se réexamine, et [`Frame`] est la seule chose à réécrire.
//!
//! # Pourquoi il n'y a **pas** de spool par client
//!
//! `W2.12` a donné au worker un spool durable, parce qu'un worker **produit** des événements que
//! rien d'autre ne détient : les perdre, c'est les perdre pour de bon.
//!
//! Le fil client est l'exact inverse. Le journal détient déjà tout, durablement et dans l'ordre. Un
//! spool par client dupliquerait un stockage durable — et deux stockages durables du même fait sont
//! deux vérités, qui divergent le jour où l'une est purgée. Un client lent perd donc des
//! **notifications**, jamais des événements, et rattrape en relisant par son cursor. C'est ce que
//! `W20.f` demande mot pour mot : « ce qu'il n'a pas pu recevoir se relit ».

use locus_event_store::{EventStore, Sequenced};

use crate::composition::Runtime;
use crate::cursor::{Collection, Cursor, CursorError};

/// Le nombre d'événements qu'un passage rend au plus.
///
/// Un fil sans borne rendrait le journal entier au premier abonné qui se reconnecte après une
/// semaine, dans une seule réponse. La borne le fait revenir, avec son cursor.
pub const DELIVERY: usize = 100;

/// Les types coalescibles de §18.3, **et rien d'autre**.
///
/// La règle est écrite en deny-by-default, comme dans `canterel` (`W2.12`), et l'orientation n'est
/// pas un détail de style : « tout est coalescible sauf ceci » rendrait fusionnable par défaut un
/// type ajouté demain — et un coût ou une alerte perdus dans une fusion sont perdus pour de bon.
pub const COALESCIBLE: [&str; 5] = ["progress", "log", "token", "resource.sampled", "heartbeat"];

/// Vrai quand ce verbe d'événement peut être fusionné avec le précédent.
///
/// Le **verbe**, pas le namespace : §10.3 nomme `task.progress` et `run.log`, et c'est la seconde
/// moitié qui dit la nature de l'événement.
#[must_use]
pub fn is_coalescible(event_type: &str) -> bool {
    let verb = event_type
        .split_once('.')
        .map_or(event_type, |(_, verb)| verb);
    COALESCIBLE.contains(&verb)
}

/// Un événement tel qu'il part vers un client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientEvent {
    /// Sa position globale — l'identifiant que SSE renvoie en `Last-Event-ID`.
    pub position: u64,
    /// Son type, tel que §10.3 le nomme.
    pub event_type: String,
    /// Le stream d'où il vient.
    pub stream_id: String,
    /// Combien d'événements identiques ont été fusionnés **dans** celui-ci, lui compris.
    ///
    /// `1` pour un événement qui n'a rien absorbé. Le compte voyage pour que le client sache qu'il
    /// a reçu un résumé et non un fait isolé — une barre de progression qui saute de 10 % à 80 %
    /// sans le dire fait douter du serveur.
    pub coalesced: usize,
}

impl From<&Sequenced> for ClientEvent {
    fn from(sequenced: &Sequenced) -> Self {
        Self {
            position: sequenced.position,
            event_type: sequenced.event.event_type.to_string(),
            stream_id: sequenced.event.stream_id.clone(),
            coalesced: 1,
        }
    }
}

/// Ce qu'un passage du fil rend.
///
/// `Delivery` et non `Batch` : `handler::Batch` est un lot **de commandes**, déclaré atomique ou
/// non. Deux types du même nom pour deux choses différentes finiraient par être importés l'un pour
/// l'autre, et `CLAUDE.md` interdit le vocabulaire parallèle jusque dans les noms de types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// Les événements, dans l'ordre, rafales coalescées.
    pub events: Vec<ClientEvent>,
    /// Où reprendre. `None` quand rien n'a été rendu — le client garde alors le sien.
    pub next: Option<Cursor>,
    /// Vrai quand le journal en avait davantage que la borne.
    ///
    /// Le client rappelle immédiatement plutôt que d'attendre le prochain événement. Sans ce
    /// drapeau, un client qui rattrape un long retard s'arrêterait à la première borne en croyant
    /// être à jour — et resterait en retard jusqu'à ce que le hasard produise un événement de plus.
    pub more: bool,
}

impl<S: EventStore> Runtime<S> {
    /// Le passage suivant du fil d'événements clients.
    ///
    /// # La reprise, et ce qu'elle garantit
    ///
    /// Sans cursor, le fil commence au début du journal. Avec, il reprend **strictement après** la
    /// position donnée : ni trou, ni doublon. C'est la même position que SSE renvoie en
    /// `Last-Event-ID`, ce qui rend la reconnexion d'un client conforme automatique.
    ///
    /// # Errors
    ///
    /// [`CursorError`] si le cursor n'a pas été émis ici, ou vient d'une autre collection.
    pub fn events_since(&self, after: Option<&Cursor>) -> Result<Delivery, CursorError> {
        let from = after.map_or(Ok(0), |cursor| cursor.read(Collection::Events))?;
        let feed = self.transaction_store().feed(from);
        let more = feed.len() > DELIVERY;

        let events = coalesce(feed.iter().take(DELIVERY).map(ClientEvent::from));
        let next = events
            .last()
            .map(|last| Cursor::issue(Collection::Events, last.position));

        Ok(Delivery { events, next, more })
    }
}

/// Fusionner les rafales coalescibles, sans jamais franchir un événement qui ne l'est pas.
///
/// Deux règles, et la seconde est celle qu'on oublie :
///
/// 1. deux événements **de même type** et coalescibles fusionnent, le dernier gagnant — c'est le
///    plus récent qui porte l'état vrai ;
/// 2. un événement **non coalescible coupe la rafale**. Sans cette coupure, un `progress` postérieur
///    à un `artifact.declared` remonterait avant lui, et le client verrait une progression annoncée
///    avant le fait qu'elle décrit.
fn coalesce(events: impl Iterator<Item = ClientEvent>) -> Vec<ClientEvent> {
    let mut out: Vec<ClientEvent> = Vec::new();
    for event in events {
        let fusionnable = is_coalescible(&event.event_type)
            && out
                .last()
                .is_some_and(|previous| previous.event_type == event.event_type);
        if fusionnable {
            let previous = out
                .pop()
                .unwrap_or_else(|| unreachable!("vérifié juste avant"));
            out.push(ClientEvent {
                coalesced: previous.coalesced + 1,
                ..event
            });
        } else {
            out.push(event);
        }
    }
    out
}

/// Le cadrage `text/event-stream` de §22.1.
///
/// Écrit à la main : SSE est un format texte de quatre lignes, et lui consacrer une dépendance
/// coûterait plus que ce qu'il rapporte. `dependencies.json` la refuserait, à raison.
pub struct Frame;

impl Frame {
    /// Le type de contenu que la réponse portera.
    pub const CONTENT_TYPE: &'static str = "text/event-stream";

    /// Un événement, cadré pour SSE.
    ///
    /// `id:` porte le cursor — c'est lui que le client renverra en `Last-Event-ID`, et c'est ce qui
    /// fait de la reprise une propriété du protocole plutôt qu'une convention à documenter.
    #[must_use]
    pub fn event(event: &ClientEvent) -> String {
        let cursor = Cursor::issue(Collection::Events, event.position);
        format!(
            "id: {cursor}\nevent: {}\ndata: {{\"position\":{},\"stream_id\":\"{}\",\"coalesced\":{}}}\n\n",
            event.event_type, event.position, event.stream_id, event.coalesced
        )
    }

    /// Un commentaire de maintien de connexion.
    ///
    /// SSE le prescrit pour empêcher les intermédiaires de couper un fil silencieux. Il commence par
    /// `:` et **n'est pas un événement** : il ne porte pas d'`id`, donc il ne déplace jamais la
    /// reprise du client. Un keep-alive qui avancerait le cursor ferait perdre des événements à
    /// chaque silence — c'est-à-dire précisément quand rien ne permet de s'en apercevoir.
    #[must_use]
    pub fn keep_alive() -> String {
        ": keep-alive\n\n".to_owned()
    }
}
