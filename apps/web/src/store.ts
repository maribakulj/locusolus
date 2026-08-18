import type { View as Wire, ViewerCommand } from "./commands.ts";
import { readView, type View } from "./view.ts";

/**
 * Ce que l'application détient — `docs/10` : « le service produit une projection, jamais une copie
 * mutable du graphe ».
 *
 * # Il n'y a pas de graphe local
 *
 * Le store ne contient qu'une [`View`] gelée et le journal des commandes qu'il reste à envoyer.
 * Interagir ne modifie **rien** : `select`, `focus` et `filter` produisent une commande destinée au
 * service, et la vue affichée reste identique jusqu'à ce qu'un nouveau document arrive.
 *
 * C'est la moitié web de l'invariant « aucun frontend n'écrit directement dans le graphe ». Un
 * store qui appliquerait la commande localement afficherait, entre la demande et la réponse, un
 * graphe que personne n'a validé — et si la réponse n'arrivait jamais, il l'afficherait pour
 * toujours.
 */
export interface Store {
  /** La vue affichée. */
  readonly view: View;
  /** Les commandes produites, dans l'ordre. */
  readonly outbox: readonly ViewerCommand[];
}

/** Ouvrir un document vérifié. */
export function openView(document: Wire): Store {
  return Object.freeze({ view: readView(document), outbox: Object.freeze([]) });
}

/**
 * Enregistrer une intention d'interaction.
 *
 * Rend un **nouveau** store dont la vue est celle de l'ancien — la même référence, pas une copie
 * retouchée — et dont la boîte d'envoi porte la commande de plus. Rien n'est appliqué ici.
 */
export function dispatch(store: Store, command: ViewerCommand): Store {
  return Object.freeze({
    view: store.view,
    outbox: Object.freeze([...store.outbox, command]),
  });
}

/**
 * Adopter le document que le service a renvoyé.
 *
 * C'est le seul chemin par lequel ce qui est affiché change. La boîte d'envoi est vidée : les
 * commandes qui ont produit ce document n'ont plus à être renvoyées.
 */
export function adopt(document: Wire): Store {
  return openView(document);
}
