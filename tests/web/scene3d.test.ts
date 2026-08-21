import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { readView, render, SceneRefused, type Scene, type View } from "@locus/web";
import type { View as Wire } from "@locus/lep";

/**
 * Test de sortie de `W9.e` — **la scène 3D du graphe épistémique.**
 *
 * Quatre propriétés, celles du tableau de `docs/10` :
 *
 * 1. la scène rend la `View` hashée de `W9.a` et n'en détient **aucune** copie modifiable ;
 * 2. une hyperarête se distingue visuellement d'une relation binaire, et le test le tient sur la
 *    **structure rendue** et non sur des pixels ;
 * 3. le graphe de **coordination** reste en 2D, et la scène 3D le refuse **en le nommant** ;
 * 4. toute interaction repart par l'API de commandes, comme `W9.c` l'exige.
 */

const root = join(import.meta.dirname, "..", "..");

function document(): Wire {
  return JSON.parse(
    readFileSync(join(root, "schemas", "examples", "view-argument-map.json"), "utf8"),
  ) as Wire;
}

function epistemique(): View {
  return readView(document());
}

/**
 * La raison du refus, ou l'échec du test.
 *
 * Même précaution que `tests/web/view.test.ts` : `assert.throws` ne rend pas l'erreur attrapée, et
 * `const e = assert.throws(...)` vaut `undefined` — le test passerait alors sur la mauvaise moitié
 * de son assertion.
 */
function refus(run: () => unknown): string {
  try {
    run();
  } catch (error) {
    assert.ok(error instanceof SceneRefused, `refus attendu, reçu ${String(error)}`);
    return error.reason;
  }
  assert.fail("aucun refus");
}

// ---------------------------------------------------------------------------------------------
// 1 — la scène est dérivée, jamais détenue
// ---------------------------------------------------------------------------------------------

test("la scène porte le condensat de la vue et n'en garde aucune copie modifiable", () => {
  const vue = epistemique();
  const scene = render(vue);

  assert.equal(scene.digest, vue.digest, "la scène dit de quelle vue elle est le rendu");
  assert.equal(scene.vertices.length, vue.nodes.length);

  // Tout est gelé : ni la scène, ni ses tableaux, ni ses éléments ne s'éditent en place.
  assert.ok(Object.isFrozen(scene));
  assert.ok(Object.isFrozen(scene.vertices));
  assert.ok(Object.isFrozen(scene.links));
  for (const sommet of scene.vertices) assert.ok(Object.isFrozen(sommet));

  assert.throws(() => {
    (scene.vertices as Vertex[]).push({ id: "intrus", x: 0, y: 0, z: 0 });
  }, "une scène qu'on peut allonger est un second graphe que personne n'a validé");
});

test("deux rendus de la même vue donnent la même scène", () => {
  const vue = epistemique();
  assert.deepEqual(render(vue), render(vue));

  // Et l'égalité survit à un aller-retour JSON : sans l'arrondi, elle dépendrait du dernier bit
  // d'un flottant, donc serait vraie en pratique et fausse en droit.
  const aller = JSON.parse(JSON.stringify(render(vue))) as Scene;
  assert.deepEqual(aller, render(vue));
});

test("la scène n'est pas une grille plate déguisée", () => {
  // Sans ce test, une disposition qui laisserait `z` à zéro partout rendrait une scène identique à
  // la 2D et ferait passer l'item pour livré sans qu'il le soit.
  const profondeurs = new Set(render(epistemique()).vertices.map((v) => v.z));
  assert.ok(profondeurs.size > 1, `les nœuds se répartissent en profondeur : ${[...profondeurs]}`);
});

// ---------------------------------------------------------------------------------------------
// 2 — une hyperarête se lit dans la structure
// ---------------------------------------------------------------------------------------------

test("une hyperarête se distingue d'une relation binaire par sa structure", () => {
  const vue = epistemique();

  // La conclusion et ses prémisses viennent des **arêtes réelles** de la vue, jamais de l'ordre des
  // nœuds. Une première version prenait `nodes[0]` comme conclusion : or `readView` réordonne en
  // forme canonique, et ce nœud-là ne recevait aucune arête. Le faisceau n'absorbait donc rien, et
  // l'assertion « aucun segment ne double le faisceau » portait sur un ensemble **vide** — elle
  // passait quoi qu'il arrive. Un mutant qui supprimait l'absorption y a survécu.
  const conclusion = vue.edges[0]?.to ?? "";
  const premisses = vue.edges.filter((e) => e.to === conclusion).map((e) => e.from);
  assert.ok(premisses.length >= 2, "la fixture doit porter au moins deux arêtes vers un même nœud");

  const scene = render(vue, new Map([[conclusion, premisses]]));

  const faisceaux = scene.links.filter((l) => l.shape === "bundle");
  const segments = scene.links.filter((l) => l.shape === "segment");
  assert.equal(faisceaux.length, 1);

  const faisceau = faisceaux[0];
  assert.ok(faisceau !== undefined && faisceau.shape === "bundle");
  assert.deepEqual([...faisceau.sources], premisses);
  assert.equal(faisceau.to, conclusion);

  // La jonction est **entre** les prémisses et la conclusion — c'est ce qui la rend lisible comme
  // un point de convergence plutôt que comme un nœud de plus.
  const positions = [...premisses, conclusion].map(
    (id) => scene.vertices.find((v) => v.id === id) ?? { x: 0, y: 0, z: 0 },
  );
  const zMin = Math.min(...positions.map((p) => p.z));
  const zMax = Math.max(...positions.map((p) => p.z));
  assert.ok(faisceau.junction.z >= zMin && faisceau.junction.z <= zMax);

  // Et **aucun segment ne double le faisceau** : dessiner les deux ferait voir une redondance qui
  // n'existe pas dans le graphe.
  for (const segment of segments) {
    assert.ok(
      segment.shape === "segment" &&
        !(premisses.includes(segment.from) && segment.to === conclusion),
      `« ${JSON.stringify(segment)} » double le faisceau`,
    );
  }
});

test("sans hyperarête déclarée, tout est segment — la structure ne s'infère pas", () => {
  // Le schéma de §23.3 ne porte que des arêtes binaires. Deviner un faisceau depuis leur forme
  // ferait lire une structure que personne n'a écrite, comme `Diff::between` refuse d'inventer un
  // `REPLACE_NODE`.
  const scene = render(epistemique());
  assert.ok(scene.links.every((l) => l.shape === "segment"));
  assert.equal(scene.links.length, epistemique().edges.length);
});

test("une hyperarête dont la conclusion est absente ne produit rien", () => {
  const vue = epistemique();
  const scene = render(vue, new Map([["fantome", [vue.nodes[0]?.id ?? ""]]]));
  assert.ok(scene.links.every((l) => l.shape === "segment"));
});

// ---------------------------------------------------------------------------------------------
// 3 — la coordination reste plate, et le refus la nomme
// ---------------------------------------------------------------------------------------------

test("la scène 3D refuse le graphe de coordination en le nommant", () => {
  const societe: Wire = { ...document(), kind: "agent_society" };
  const vue = { ...readView(document()), kind: societe.kind } as View;

  assert.equal(
    refus(() => render(vue)),
    "coordination-is-flat",
  );

  // Le refus **dit pourquoi**, et pas seulement qu'il refuse : un motif absent ferait chercher un
  // bug là où il y a une décision.
  try {
    render(vue);
  } catch (error) {
    assert.ok(error instanceof SceneRefused);
    assert.match(error.message, /agent_society/);
    assert.match(error.message, /occlusion/);
  }
});

test("le refus vise la sorte de vue, pas la taille du graphe", () => {
  // Un graphe épistémique minuscule passe quand même : ce qui décide est ce que la vue **est**, pas
  // combien elle porte. Un seuil de taille aurait fait basculer un graphe d'un mode à l'autre au
  // gré d'un ajout, ce qu'aucun lecteur n'aurait pu anticiper.
  const vue = epistemique();
  const minuscule = { ...vue, nodes: vue.nodes.slice(0, 1), edges: [] } as View;
  assert.equal(render(minuscule).vertices.length, 1);
});

// ---------------------------------------------------------------------------------------------
// 4 — aucune interaction ne court-circuite l'API de commandes
// ---------------------------------------------------------------------------------------------

test("la scène n'expose aucun chemin d'interaction qui contourne les commandes", () => {
  // `W9.c` : toute interaction repart par l'API de commandes. Le module ne doit donc offrir aucune
  // fonction qui prendrait une scène pour en rendre une autre — ce serait un état local que
  // personne n'a validé, exactement ce que `W9.d` a refusé côté store.
  const source = readFileSync(join(root, "apps", "web", "src", "scene3d.ts"), "utf8");
  for (const interdit of [
    "export function select",
    "export function focus",
    "export function filter",
    "scene: Scene",
    "Scene): Scene",
  ]) {
    assert.ok(!source.includes(interdit), `« ${interdit} » dans scene3d.ts contourne W9.c`);
  }
});

/** Le type local du test, pour l'assertion de gel. */
interface Vertex {
  id: string;
  x: number;
  y: number;
  z: number;
}
