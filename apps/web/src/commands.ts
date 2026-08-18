import type { View } from "@locus/lep";

export type { View };

/**
 * Ce que le workspace envoie au service — les trois de §23.
 *
 * Le miroir TypeScript de `locus_visualization::ViewerCommand`. Les deux ne partagent aucun code :
 * ils se rencontrent sur le nom qui part sur le fil, et un test tient la liste.
 */
export type ViewerCommand =
  | { readonly kind: "focus"; readonly node: string; readonly depth: number }
  | { readonly kind: "filter"; readonly nodeKinds: readonly string[] }
  | { readonly kind: "select"; readonly nodes: readonly string[] };

/** Les noms que le fil transporte, dans l'ordre où §23 les donne. */
export const VIEWER_COMMANDS = ["focus", "filter", "select"] as const;
