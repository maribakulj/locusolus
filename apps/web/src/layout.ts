import type { View } from "./view.ts";

/**
 * La disposition 2D de référence — `docs/SPEC_V1.md` §23.2.
 *
 * # Déterministe, et pour une raison
 *
 * Deux ouvertures de la même vue donnent les mêmes positions. Une disposition qui bougerait d'une
 * fois sur l'autre rendrait deux captures d'écran incomparables, et c'est précisément en comparant
 * deux états d'un graphe qu'on voit ce qui a changé. Rien ici n'est aléatoire ni horodaté.
 *
 * L'ordre de départ est celui de la forme canonique, pas celui du document : la disposition hérite
 * ainsi de la propriété qui compte, à savoir que l'ordre d'insertion d'un producteur n'a aucun
 * effet visible.
 */

/** Où un nœud est posé. */
export interface Placement {
  readonly id: string;
  readonly x: number;
  readonly y: number;
}

/**
 * Poser les nœuds de `view` sur une grille, en colonnes.
 *
 * `columns` borne la largeur ; une valeur non positive vaut 1, parce qu'une grille sans colonne
 * n'est pas une grille et qu'échouer ici priverait le lecteur de son graphe pour un réglage.
 */
export function layout(view: View, columns = 6): readonly Placement[] {
  const width = Math.max(1, Math.floor(columns));
  return Object.freeze(
    view.nodes.map((node, index) =>
      Object.freeze({ id: node.id, x: index % width, y: Math.floor(index / width) }),
    ),
  );
}
