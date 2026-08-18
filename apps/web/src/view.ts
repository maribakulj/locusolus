import { createHash } from "node:crypto";

import type { View as Wire } from "@locus/lep";

/**
 * Lire une vue de visualisation — `docs/SPEC_V1.md` §23.3.
 *
 * # Le document ne transporte pas sa preuve
 *
 * Le schéma porte le condensat, pas la forme canonique. C'est délibéré : transporter la forme
 * canonique ferait de la preuve une donnée reçue, et un document tronqué se relirait alors comme un
 * graphe plus petit mais authentique — exactement le genre de perte qu'une visualisation rend
 * invisible, puisqu'on ne voit pas ce qui n'est pas dessiné.
 *
 * Le lecteur **reconstruit** donc la forme canonique et compare. C'est la même règle que partout
 * ailleurs dans ce dépôt : ce qui prouve ne peut pas être ce qui est demandé.
 *
 * # Deux implémentations qui ne se consultent pas
 *
 * `packages/visualization` construit la même forme canonique en Rust. Aucun des deux ne lit le
 * code de l'autre ; ils se rencontrent sur une fixture partagée. Une bibliothèque commune serait
 * d'accord avec elle-même même en ayant tort.
 */

/** Une vue lue et vérifiée. Ses champs ne se modifient pas — c'est un instantané. */
export interface View {
  readonly kind: Wire["kind"];
  readonly watermark: number;
  readonly digest: string;
  readonly derivedFrom: string | undefined;
  readonly nodes: readonly { readonly id: string; readonly kind: string; readonly label: string }[];
  readonly edges: readonly { readonly from: string; readonly to: string; readonly kind: string }[];
}

/** Pourquoi une vue est refusée. */
export type Rejection = "digest-mismatch" | "duplicate-node" | "dangling-edge" | "unsupported-hash";

/** Le refus, avec sa raison — un code, pas une phrase à relire. */
export class ViewRejected extends Error {
  readonly reason: Rejection;

  constructor(reason: Rejection, message: string) {
    super(message);
    this.name = "ViewRejected";
    this.reason = reason;
  }
}

/**
 * La forme canonique d'un document — l'ordre y est fixé, jamais hérité de celui du document.
 *
 * Un producteur qui reconstruit tout et un autre qui rattrape incrémentalement ne remplissent pas
 * leurs tableaux pareil ; s'ils entraient dans la forme, deux viewers montrant la même chose ne
 * pourraient pas le prouver.
 */
export function canonicalise(document: Wire): string {
  const { nodes, edges } = ordered(document);

  const lines = [`view/1`, document.kind, String(document.watermark)];
  if (document.derived_from !== undefined) lines.push(`derived-from\t${document.derived_from}`);
  for (const node of nodes) lines.push(`n\t${node.id}\t${node.kind}\t${node.label}`);
  for (const edge of edges) lines.push(`e\t${edge.from}\t${edge.to}\t${edge.kind}`);
  return `${lines.join("\n")}\n`;
}

/**
 * Lire un document et le vérifier.
 *
 * Refuse un condensat qui ne correspond pas, deux nœuds de même identité — une sélection ne saurait
 * plus lequel elle désigne — et une arête dont une extrémité manque, parce qu'un trait qui mène
 * hors du graphe fait supposer un objet que le graphe n'a pas.
 *
 * Refuse aussi, plutôt que de passer outre, un algorithme de condensat que ce lecteur ne sait pas
 * calculer : rendre « vérifié » ce qu'on n'a pas vérifié serait la seule faute vraiment grave ici.
 */
export function readView(document: Wire): View {
  const identities = new Set<string>();
  for (const node of document.nodes) {
    if (identities.has(node.id)) {
      throw new ViewRejected("duplicate-node", `deux nœuds portent l'identité « ${node.id} »`);
    }
    identities.add(node.id);
  }
  for (const edge of document.edges) {
    for (const end of [edge.from, edge.to]) {
      if (!identities.has(end)) {
        throw new ViewRejected(
          "dangling-edge",
          `l'arête mène à « ${end} », qui n'est pas dans la vue`,
        );
      }
    }
  }

  const [algorithm] = document.digest.split(":");
  if (algorithm !== "sha256" && algorithm !== "sha512") {
    throw new ViewRejected(
      "unsupported-hash",
      `condensat « ${algorithm} » : ce lecteur ne sait pas le calculer, et ne dira pas qu'il a vérifié`,
    );
  }
  const recomputed = `${algorithm}:${createHash(algorithm).update(canonicalise(document), "utf8").digest("hex")}`;
  if (recomputed !== document.digest) {
    throw new ViewRejected(
      "digest-mismatch",
      `la vue annonce ${document.digest} et vaut ${recomputed} : le document ne dit pas ce qu'il prouve`,
    );
  }

  // Les nœuds sortent dans l'**ordre canonique**, pas dans celui du document. Sans cela tout ce
  // qui est construit à partir d'une vue — au premier chef la disposition — hériterait de l'ordre
  // d'insertion d'un producteur, et deux rendus du même contenu ne se superposeraient pas.
  const { nodes, edges } = ordered(document);
  return Object.freeze({
    kind: document.kind,
    watermark: document.watermark,
    digest: document.digest,
    derivedFrom: document.derived_from,
    nodes: Object.freeze(nodes.map((node) => Object.freeze({ ...node }))),
    edges: Object.freeze(edges.map((edge) => Object.freeze({ ...edge }))),
  });
}

/** Le contenu du document, trié et dédoublonné — l'ordre que la forme canonique fixe. */
function ordered(document: Wire): {
  nodes: Wire["nodes"][number][];
  edges: Wire["edges"][number][];
} {
  const nodes = [...document.nodes].sort(
    (left, right) =>
      compare(left.id, right.id) ||
      compare(left.kind, right.kind) ||
      compare(left.label, right.label),
  );
  const edges = dedupe(
    [...document.edges].sort(
      (left, right) =>
        compare(left.from, right.from) ||
        compare(left.to, right.to) ||
        compare(left.kind, right.kind),
    ),
  );
  return { nodes, edges };
}

function compare(left: string, right: string): number {
  // Comparaison par unité de code, comme l'ordre des `String` de Rust : deux implémentations qui
  // trieraient différemment produiraient deux formes canoniques pour un même contenu.
  return left < right ? -1 : left > right ? 1 : 0;
}

function dedupe<T extends { from: string; to: string; kind: string }>(sorted: readonly T[]): T[] {
  return sorted.filter(
    (edge, index) =>
      index === 0 ||
      edge.from !== sorted[index - 1]!.from ||
      edge.to !== sorted[index - 1]!.to ||
      edge.kind !== sorted[index - 1]!.kind,
  );
}
