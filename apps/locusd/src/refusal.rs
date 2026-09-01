//! Ce qu'un refus d'admission **du worker** fait à la mission — `W19.c`, ADR 0037.
//!
//! # Le trou, tel que la chaîne réelle l'a rendu
//!
//! `runLoop` réclame, l'admission dit non, et la boucle rend la main **sans rien dire au plan de
//! contrôle**. La mission reste sous bail jusqu'à expiration, et « le worker a refusé » se confond
//! avec « le worker est mort ». C'est la paire de silences que ce dépôt refuse partout ailleurs, et
//! `W12.d.4` l'a trouvée en exécutant la chaîne, pas en la lisant.
//!
//! # Écrire un fait n'aurait rien ajouté, et c'est ce qui fixe la forme de ce module
//!
//! Le chemin générique de [`crate::lep::Report`] écrit déjà un fait pour **n'importe quel** type
//! d'événement. Un `task.refused` qui n'aurait fait que cela serait une valeur d'énumération sans
//! effet — une promesse au sens de l'ADR 0022 décision 0, que l'ADR 0037 rappelle à sa dernière
//! décision. La conséquence est donc la **remise en file** : la mission redevient réclamable au lieu
//! d'attendre qu'un bail que plus personne n'honore finisse par expirer.
//!
//! # D'où la mission revient, et pourquoi pas d'une seconde copie
//!
//! Le daemon ne la conserve pas : `lep_claim` la retire de la file et n'en garde rien. Elle est dans
//! le **journal** — le fait `task.proposed` porte la proposition entière, et son commentaire dit
//! pourquoi : « l'invariant 2 dit que le journal est la vérité institutionnelle ; il faut donc qu'il
//! porte de quoi reconstruire ce qu'on y a proposé ». C'est exactement la lecture que
//! `Runtime::lep_queue` fait déjà pour mettre en file, et ce module la refait plutôt que d'inventer
//! un cache que rien ne survivrait à un redémarrage.
//!
//! # Le worker peut refuser la même mission indéfiniment, et c'est voulu
//!
//! La file de référence est FIFO et ne filtre pas par worker : une mission remise revient donc au
//! même worker, qui la refusera encore, et chaque tour écrit un fait de plus. Une première rédaction
//! voulait l'éviter en marquant la mission « refusée par ce worker » pour que la file la saute.
//!
//! C'est ce qui a été **refusé**, et pour la raison inverse de celle qu'on attend : sauter est plus
//! silencieux. Une mission écartée de la circulation ne produit plus rien à lire, et un exploitant
//! qui la cherche ne trouve qu'une file qui l'ignore. Un refus répété, lui, écrit à chaque tour un
//! fait qui **nomme son code** — `model_unavailable`, `sandbox_unavailable` — et dit donc ce qui
//! manque à cette installation. Ce dépôt préfère partout la panne bruyante à la panne muette ; il n'y
//! a pas de raison d'inverser ici.
//!
//! Le refus n'est d'ailleurs pas définitif : un worker qui installe le modèle qui manquait acceptera
//! la mission au tour suivant. L'écarter par avance ferait de ce qui est vrai maintenant une
//! décision permanente.

use crate::error::CommandError;
use locus_lep::Event;

/// Ce qu'un `task.refused` dit, une fois relu.
///
/// Le `code` n'est pas relu comme [`locus_lep::RefusalCode`] : le schéma l'a déjà vérifié à
/// l'admission du document, et le retyper ici obligerait ce module à connaître les quatorze codes
/// pour n'en utiliser aucun. Ce que la remise en file demande est le `task_id` ; le code voyage pour
/// que la note d'exploitation le nomme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// La tâche dont la mission revient en file.
    pub task_id: String,
    /// Le code de refus, sous le nom de `repos/canterel/SPEC_V1.md` §10.2.
    pub code: String,
}

/// Le type d'événement que ce module reconnaît — `W19.c`, gardé par `refusal-events`.
pub const REFUSED: &str = "task.refused";

/// Reconnaître un refus d'admission dans un événement de fil, ou rendre `None`.
///
/// `None` pour tout le reste, c'est-à-dire pour les événements de progression de §15.6, qui gardent
/// le chemin générique. La reconnaissance se fait sur le **type**, jamais sur la présence d'un champ :
/// un événement de progression qui porterait par hasard un `code` dans sa charge ne doit pas remettre
/// une mission en file.
///
/// Un `task.refused` sans `code` lisible rend `None` lui aussi, et ce n'est pas une indulgence : le
/// schéma rend `code` obligatoire, donc un document qui en manque n'a pas été validé, et remettre une
/// mission en file sur la foi d'un document qu'on n'a pas su lire serait agir sans savoir pourquoi.
/// Le fait, lui, s'écrira quand même par le chemin générique — la trace ne se perd pas.
#[must_use]
pub fn refused(event: &Event, task_id: &str) -> Option<Refused> {
    if event.event_type != REFUSED {
        return None;
    }
    let code = event
        .payload
        .as_ref()?
        .get("code")?
        .as_str()
        .filter(|code| !code.is_empty())?;
    Some(Refused {
        task_id: task_id.to_owned(),
        code: code.to_owned(),
    })
}

impl<S: locus_event_store::EventStore> crate::Runtime<S> {
    /// Remettre en file la mission qu'un worker vient de refuser.
    ///
    /// # Ce qui est relu, et d'où
    ///
    /// La proposition, dans le stream de la tâche — la même lecture que
    /// [`crate::Runtime::lep_queue`] fait pour mettre en file la première fois. Le daemon ne garde
    /// aucune copie de la mission après l'avoir servie, et lui en donner une ici aurait produit deux
    /// vérités pour un même fait, dont l'une ne survivrait pas à un redémarrage.
    ///
    /// # Le rang d'attempt vient de la proposition, et non du bail
    ///
    /// C'est le même choix que `lep_queue`, et pour la même raison : le rang appartient à ce qui a
    /// été proposé. Le prendre du bail qu'on vient de perdre ferait repartir la mission sous un rang
    /// que l'institution lirait comme une tentative de plus, alors qu'aucune n'a eu lieu — l'admission
    /// a dit non **avant** toute exécution.
    ///
    /// # Errors
    ///
    /// [`CommandError`] quand aucune proposition ne se relit pour cette tâche. Le cas ne devrait pas
    /// se produire — une mission qu'un worker a réclamée est passée par la file, donc par une
    /// proposition —, et il est rendu plutôt que tu : un refus qui ne remet rien en file laisserait
    /// la mission nulle part, ce qui est précisément le silence que cet item retire.
    pub(crate) fn requeue_refused(&self, refused: &Refused) -> Result<(), CommandError> {
        let stream = crate::lep::stream_of_task(&refused.task_id);
        let (proposal, _) = self.proposed(&stream, &refused.task_id)?;
        self.lep().queue().enqueue(crate::lep::Queued {
            mission: proposal.envelope(),
            attempt: proposal.attempt,
        });
        Ok(())
    }
}
