/**
 * Test de sortie de W13.b — **le pli des fixtures `lep/1.0` rend un graphe d'exécution sans arête
 * orpheline, et rien dans l'événement LEP ne dit quel agent a agi.**
 *
 * Les deux moitiés répondent à la même question, posée avant que W13.f engage une projection
 * dessus : de quoi `lep/1.0` est-il le journal ? La réponse tient en une phrase — **il journalise
 * une exécution, pas une organisation** — et les deux moitiés en sont les deux faces.
 */

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import test from "node:test";

import { stripFixture } from "../../tooling/schemas/validate.ts";
import {
  edgesOfKind,
  fold,
  nodeId,
  nodesOfKind,
  orphanEdges,
  type ExecutionGraph,
} from "./fold.ts";

const root = fileURLToPath(new URL("../..", import.meta.url));
const examples = join(root, "schemas/examples");
const schemas = join(root, "schemas/lep/1.0");

/**
 * Tout le corpus, sans exception déclarée.
 *
 * Les fixtures `invalid-*` sont incluses : elles sont mal formées **au sens du schéma**, ce qui ne
 * les empêche pas d'être pliées — et un pli qui casserait dessus casserait aussi sur un document
 * partiel venu du fil, ce qui est le cas normal d'une projection qui rattrape un journal.
 */
function corpus(): Record<string, unknown>[] {
  return readdirSync(examples)
    .filter((name) => name.endsWith(".json"))
    .map((name) => stripFixture(JSON.parse(readFileSync(join(examples, name), "utf8"))).body)
    .map((body) => body as Record<string, unknown>);
}

function graph(): ExecutionGraph {
  return fold(corpus());
}

//
// Le pli rend un graphe
//

test("le pli ne laisse aucune arête orpheline", () => {
  // La garantie du pli. Un graphe où une arête pointe dans le vide se parcourt sans erreur et ment
  // à chaque parcours : le nœud manquant ne se signale nulle part, il n'est simplement jamais
  // atteint.
  assert.deepEqual(orphanEdges(graph()), []);
});

/**
 * Le détecteur d'orphelines, mis à l'épreuve sur une orpheline.
 *
 * Trouvé par mutation : neutraliser `orphanEdges` pour qu'il rende toujours la liste vide laissait
 * la suite **verte**. Un détecteur muet est indiscernable d'un graphe sain, et le test qui
 * l'emploie ne dit alors plus rien — c'est la troisième fois de ce chantier qu'une garde passe
 * sans être elle-même vérifiée, et la forme est toujours la même : on teste ce que la garde
 * protège, jamais qu'elle protège.
 */
test("une arête qui pointe dans le vide est signalée, et le nœud manquant est nommé", () => {
  const dangling = {
    nodes: [{ id: nodeId("attempt", "t#1"), kind: "attempt" as const }],
    edges: [
      {
        from: nodeId("attempt", "t#1"),
        to: nodeId("task", "disparue"),
        kind: "belongs_to" as const,
      },
    ],
  };
  const found = orphanEdges(dangling);
  assert.equal(found.length, 1);
  assert.deepEqual(found[0]?.missing, [nodeId("task", "disparue")]);

  // Et les deux extrémités comptent, pas seulement la cible.
  const backwards = {
    nodes: [{ id: nodeId("task", "t"), kind: "task" as const }],
    edges: [
      { from: nodeId("attempt", "absent#1"), to: nodeId("task", "t"), kind: "belongs_to" as const },
    ],
  };
  assert.deepEqual(orphanEdges(backwards)[0]?.missing, [nodeId("attempt", "absent#1")]);
});

test("le pli trouve les trois sortes que W13.b nomme", () => {
  const folded = graph();
  for (const kind of ["attempt", "tool", "artifact"] as const) {
    assert.ok(
      nodesOfKind(folded, kind).length > 0,
      `aucun nœud « ${kind} » : le corpus ou le pli ne dit pas ce qu'on croit`,
    );
  }
});

test("un attempt appartient à sa tâche et nomme son exécutant", () => {
  const folded = graph();
  const attempt = nodeId("attempt", "task-nominal#1");
  assert.ok(folded.nodes.some((node) => node.id === attempt));

  const belongs = edgesOfKind(folded, "belongs_to").filter((edge) => edge.from === attempt);
  assert.deepEqual(
    belongs.map((edge) => edge.to),
    [nodeId("task", "task-nominal")],
    "un attempt appartient à une tâche et à une seule",
  );

  assert.ok(
    edgesOfKind(folded, "executed_by").some((edge) => edge.from === attempt),
    "l'exécutant vient de `worker_id`, que l'attempt et la lease portent tous deux",
  );
});

test("un artefact remonte à l'attempt qui l'a produit", () => {
  const folded = graph();
  const produced = edgesOfKind(folded, "produced_by").filter((edge) =>
    edge.from.startsWith("artifact:"),
  );
  assert.ok(produced.length > 0, "aucun artefact ne remonte à son producteur");
  for (const edge of produced) {
    assert.ok(
      edge.to.startsWith("attempt:") || edge.to.startsWith("run:"),
      `un artefact est produit par un attempt ou consigné par un run, pas par « ${edge.to} »`,
    );
  }
});

test("une dérivation d'artefact est une arête, pas un champ perdu", () => {
  const folded = graph();
  const derived = edgesOfKind(folded, "derived_from");
  assert.ok(
    derived.some((edge) => edge.to === nodeId("artifact", "artifact-measurements")),
    "la dérivation déclarée par `artifact-manifest-promoted` doit apparaître",
  );
});

test("deux identifiants identiques de sortes différentes ne fusionnent pas", () => {
  // `task_id` et `run_id` peuvent porter la même chaîne sans désigner la même chose. Sans préfixe
  // de sorte, les deux nœuds fusionneraient en silence et le graphe aurait l'air plus connexe
  // qu'il ne l'est.
  const folded = fold([
    { task_id: "x", attempt: 1 },
    { run_id: "x", task_id: "x", attempt: 1 },
  ]);
  assert.equal(nodesOfKind(folded, "task").length, 1);
  assert.equal(nodesOfKind(folded, "run").length, 1);
  assert.notEqual(nodeId("task", "x"), nodeId("run", "x"));
  assert.deepEqual(orphanEdges(folded), []);
});

test("un document partiel se plie sans casser et sans inventer", () => {
  // Le cas normal d'une projection qui rattrape un journal : un événement sans `task_id` ne dit
  // pas à quel attempt il appartient, et le pli doit s'abstenir plutôt que d'en fabriquer un.
  const folded = fold([{ protocol: "lep/1.0", event_type: "tool.completed", payload: {} }]);
  assert.deepEqual(folded.nodes, []);
  assert.deepEqual(folded.edges, []);
});

//
// Ce que l'événement LEP ne dit pas — la moitié qui se prouve par l'absence
//

/**
 * La raison d'être de W13, énoncée comme un fait vérifiable.
 *
 * Un `Event` porte `task_id`, `attempt`, `lease_id`, `worker_id` : de quoi reconstruire **qui a
 * exécuté quoi sur quelle machine**. Il ne porte rien qui dise **quel agent** a agi. Le graphe
 * d'exécution est donc dérivable de `lep/1.0` tel quel — c'est la première moitié de ce test — et
 * le graphe organisationnel ne l'est pas.
 *
 * Le test lit le **schéma**, pas les fixtures : une fixture sans champ d'agent ne prouve que le
 * contenu de cette fixture, tandis qu'un schéma sans propriété d'agent prouve qu'aucun producteur
 * conforme n'a d'endroit où en mettre un.
 *
 * # Ce que ce test ne demande pas, et pourquoi
 *
 * Aucun schéma LEP ne porte `additionalProperties: false`, et exiger la clôture ici serait une
 * erreur : `docs/06` fait des champs optionnels compatibles un ajout **mineur**, ce qui suppose
 * qu'un consommateur `1.0` tolère les champs d'un producteur `1.1`. Fermer les documents
 * supprimerait cette compatibilité. Le schéma de l'événement le dit d'ailleurs lui-même à propos
 * de `event_type`, « fermé exprès, **contrairement aux documents** » — la clôture est réservée aux
 * vocabulaires, où un terme inconnu ne peut pas être ignoré sans conséquence.
 *
 * Ce qui protège réellement une projection est ailleurs, et c'est le test suivant.
 */
test("aucun champ d'agent n'existe dans l'événement LEP", () => {
  const event = JSON.parse(readFileSync(join(schemas, "event.schema.json"), "utf8")) as {
    properties: Record<string, unknown>;
  };
  const organisational = Object.keys(event.properties).filter((name) =>
    /agent|team|role|assign/i.test(name),
  );
  assert.deepEqual(
    organisational,
    [],
    "un champ d'agent dans l'événement rendrait W13.d inutile — et changerait la réponse de ce test",
  );
});

/**
 * Et un champ glissé en fraude n'atteint pas le consommateur.
 *
 * Les documents restent ouverts pour la compatibilité ascendante ; ce qui empêche un `agent_id`
 * non déclaré d'être **lu** est que le SDK est généré depuis le schéma. Un champ que les types ne
 * modélisent pas n'existe pas pour qui les emploie, et le round-trip de W0.8 le vérifie déjà dans
 * l'autre sens : ce qui n'est pas modélisé disparaît au ré-encodage.
 *
 * Le test lit le SDK **généré**, pas une liste recopiée : c'est le fichier que
 * `npm run check:generated` maintient égal aux schémas.
 */
test("le SDK généré ne donne aucun champ d'agent à lire dans un événement", () => {
  const generated = readFileSync(join(root, "packages/lep/src/generated.ts"), "utf8");
  const start = generated.indexOf("export type Event = {");
  assert.ok(start > 0, "le type `Event` doit exister dans le SDK généré");
  const block = generated.slice(start, generated.indexOf("\n};", start));

  const suspicious = block
    .split("\n")
    .filter((line) => /^\s+readonly \w+/.test(line))
    .map((line) => line.trim())
    .filter((line) => /agent|team|role|assign/i.test(line));
  assert.deepEqual(
    suspicious,
    [],
    "un consommateur qui emploie le SDK n'a aucun champ d'agent à lire, déclaré ou non",
  );
});

/**
 * Et pourtant l'`Attempt` en porte un — facultatif.
 *
 * La nuance décide de W13.g. Un `agent_id` existe dans `attempt.schema.json`, mais une projection
 * consomme le **flux d'événements**, pas les documents d'attempt : l'assignation ne lui parvient
 * donc jamais. C'est exactement ce que W13.d comble en faisant de l'assignation un événement, et
 * ce test est ce qui empêche de croire que le champ existant suffisait.
 */
test("l_attempt porte un agent facultatif, que le flux d'événements ne transporte pas", () => {
  const attempt = JSON.parse(readFileSync(join(schemas, "attempt.schema.json"), "utf8")) as {
    properties: Record<string, unknown>;
    required: string[];
  };

  assert.ok("agent_id" in attempt.properties, "le champ existe");
  assert.ok(
    !attempt.required.includes("agent_id"),
    "et il est facultatif : un attempt sans agent est un attempt valide",
  );

  const folded = graph();
  assert.equal(
    nodesOfKind(folded, "attempt").every((node) => node.kind === "attempt"),
    true,
  );
  assert.ok(
    !folded.nodes.some((node) => node.kind.includes("agent" as never)),
    "le pli ne fabrique aucun nœud d'agent : il n'a rien pour le faire",
  );
});
