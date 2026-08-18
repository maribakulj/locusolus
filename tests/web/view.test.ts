import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import {
  VIEWER_COMMANDS,
  adopt,
  canonicalise,
  dispatch,
  layout,
  openView,
  readView,
  ViewRejected,
  type ViewerCommand,
} from "@locus/web";
import type { View as Wire } from "@locus/lep";

/**
 * Test de sortie de W9.d — **le workspace web ne détient aucun graphe modifiable.**
 *
 * `docs/10` : « le service produit une projection, jamais une copie mutable du graphe. Si une vue
 * devient éditable en place, l'invariant *aucun frontend n'écrit directement dans le graphe* est
 * perdu. »
 *
 * Côté web, la tentation n'est pas d'écrire dans le graphe : c'est d'appliquer localement ce qu'on
 * vient de demander, pour que l'écran réponde tout de suite. Un store qui ferait cela afficherait,
 * entre la demande et la réponse, un graphe que personne n'a validé — et si la réponse n'arrivait
 * jamais, il l'afficherait pour toujours.
 */

const root = join(import.meta.dirname, "..", "..");

function document(): Wire {
  return JSON.parse(
    readFileSync(join(root, "schemas", "examples", "view-argument-map.json"), "utf8"),
  ) as Wire;
}

/**
 * La raison du refus, ou l'échec du test.
 *
 * `assert.throws` ne rend pas l'erreur qu'il a attrapée : écrire `const e = assert.throws(...)`
 * donne `undefined`, et le test passe alors sur la mauvaise moitié de son assertion.
 */
function rejection(run: () => unknown): string {
  try {
    run();
  } catch (error) {
    assert.ok(error instanceof ViewRejected, `refus attendu, reçu ${String(error)}`);
    return error.reason;
  }
  assert.fail("aucun refus");
}

function sha256(text: string): string {
  return `sha256:${createHash("sha256").update(text, "utf8").digest("hex")}`;
}

test("la forme canonique est celle que Rust produit", () => {
  // La même fixture que `packages/visualization/tests/wire.rs` compare de son côté. Aucune des deux
  // implémentations ne lit le code de l'autre : elles se rencontrent ici.
  const attendue = readFileSync(
    join(root, "packages", "visualization", "tests", "fixtures", "argument-map.canonical.txt"),
    "utf8",
  );
  assert.equal(canonicalise(document()), attendue);
});

test("l'ordre du document ne change pas la forme canonique", () => {
  const dansUnSens = document();
  const dansLAutre: Wire = {
    ...dansUnSens,
    nodes: [...dansUnSens.nodes].reverse(),
    edges: [...dansUnSens.edges].reverse(),
  };
  assert.equal(canonicalise(dansUnSens), canonicalise(dansLAutre));
  assert.equal(readView(dansLAutre).digest, readView(dansUnSens).digest);
});

test("une arête écrite deux fois ne compte qu'une fois", () => {
  // La même relation répétée est la même relation. La garder deux fois ferait lire un appui de
  // plus à qui compte les soutiens d'un claim — et le condensat, lui, est calculé sur la forme
  // dédoublonnée, donc les deux documents sont le même.
  const base = document();
  const repetee: Wire = { ...base, edges: [...base.edges, base.edges[0]!] };
  assert.equal(canonicalise(repetee), canonicalise(base));
  assert.equal(readView(repetee).edges.length, base.edges.length);
});

test("un document dont le condensat ne correspond pas est refusé", () => {
  const altere: Wire = {
    ...document(),
    nodes: document().nodes.map((node) =>
      node.id === "claim-a" ? { ...node, label: "Le lemme 3 est faux" } : node,
    ),
  };
  assert.equal(
    rejection(() => readView(altere)),
    "digest-mismatch",
  );
});

test("un document tronqué est refusé, pas relu comme un graphe plus petit", () => {
  // C'est la perte qu'une visualisation rend invisible : on ne voit pas ce qui n'est pas dessiné.
  const tronque: Wire = {
    ...document(),
    nodes: document().nodes.filter((node) => node.kind !== "artifact"),
    edges: document().edges.filter((edge) => edge.from !== "art-1"),
  };
  assert.equal(
    rejection(() => readView(tronque)),
    "digest-mismatch",
  );
});

test("un condensat que ce lecteur ne sait pas calculer est refusé, pas supposé bon", () => {
  const blake: Wire = { ...document(), digest: `blake3:${"ab".repeat(32)}` };
  assert.equal(
    rejection(() => readView(blake)),
    "unsupported-hash",
  );
});

test("deux nœuds de même identité sont refusés", () => {
  const base = document();
  const double: Wire = { ...base, nodes: [...base.nodes, base.nodes[0]!] };
  assert.equal(
    rejection(() => readView(double)),
    "duplicate-node",
  );
});

test("une arête vers un nœud absent est refusée", () => {
  const base = document();
  const pendante: Wire = {
    ...base,
    edges: [...base.edges, { from: "claim-a", to: "fantome", kind: "supports" }],
  };
  assert.equal(
    rejection(() => readView(pendante)),
    "dangling-edge",
  );
});

test("une vue dérivée porte son parent dans la forme canonique", () => {
  const base = document();
  const parent = base.digest;
  const derive: Wire = { ...base, derived_from: parent };
  assert.ok(canonicalise(derive).includes(`derived-from\t${parent}`));
  // Et son condensat n'est pas celui du parent : une vue filtrée ne se fait pas passer pour la
  // projection, même quand elle en garde tout le contenu.
  assert.notEqual(sha256(canonicalise(derive)), parent);
});

test("le store ne détient aucun graphe modifiable", () => {
  const store = openView(document());
  const avant = JSON.stringify(store.view);

  const commandes: ViewerCommand[] = [
    { kind: "select", nodes: ["claim-a"] },
    { kind: "focus", node: "claim-a", depth: 1 },
    { kind: "filter", nodeKinds: ["claim"] },
  ];
  const apres = commandes.reduce(dispatch, store);

  // La vue affichée est la **même référence**, pas une copie retouchée.
  assert.equal(apres.view, store.view);
  assert.equal(JSON.stringify(apres.view), avant);
  assert.deepEqual([...apres.outbox], commandes);
  // Et le store de départ n'a pas bougé : `dispatch` rend un nouveau store.
  assert.equal(store.outbox.length, 0);
});

test("seul un document renvoyé change ce qui est affiché", () => {
  const store = dispatch(openView(document()), { kind: "select", nodes: ["claim-a"] });
  const reponse = adopt(document());
  assert.equal(reponse.outbox.length, 0, "les commandes qui ont produit ce document sont soldées");
  assert.equal(reponse.view.digest, store.view.digest);
});

test("une vue lue est gelée", () => {
  const view = readView(document());
  assert.throws(() => {
    (view.nodes as { length: number }).length = 0;
  }, TypeError);
  assert.throws(() => {
    (view.nodes[0] as { label: string }).label = "autre chose";
  }, TypeError);
});

test("la disposition est déterministe", () => {
  const view = readView(document());
  assert.deepEqual([...layout(view)], [...layout(view)]);
  // Deux ouvertures du même document donnent les mêmes positions : sans cela, deux captures
  // d'écran d'un même graphe ne se comparent pas, et c'est en les comparant qu'on voit bouger.
  assert.deepEqual([...layout(readView(document()))], [...layout(view)]);
});

test("la disposition suit l'ordre canonique, pas celui du document", () => {
  const base = document();
  const renverse: Wire = { ...base, nodes: [...base.nodes].reverse() };
  assert.deepEqual([...layout(readView(renverse))], [...layout(readView(base))]);
});

test("une grille sans colonne ne prive pas le lecteur de son graphe", () => {
  const view = readView(document());
  assert.equal(layout(view, 0).length, view.nodes.length);
  assert.deepEqual([...layout(view, 0)], [...layout(view, 1)]);
});

test("les trois commandes de §23 portent les noms du fil", () => {
  assert.deepEqual([...VIEWER_COMMANDS], ["focus", "filter", "select"]);
});
