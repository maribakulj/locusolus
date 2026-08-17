/**
 * Le pli des documents `lep/1.0` en graphe d'exécution — W13.b, `docs/13` §2.
 *
 * # Ce que ce module est, et où il vit
 *
 * Un **pli**, pas une projection : il lit des documents de protocole et rend un graphe, sans état,
 * sans journal, sans base. Il vit sous `tests/` et non sous `packages/` parce que ce qu'il produit
 * n'est encore consommé par personne — c'est une lecture, écrite pour répondre à une question
 * avant que W13.f engage une projection sur la réponse.
 *
 * La question : **le graphe d'exécution est-il dérivable de `lep/1.0` tel quel ?** Si oui, W13.f
 * s'écrit contre un protocole inchangé. Sinon, il faut d'abord faire évoluer `lep`, et le
 * découvrir après avoir écrit la projection l'aurait coûtée.
 *
 * # Aucun champ n'est ajouté au protocole
 *
 * Ce fichier ne lit que ce que les schémas déclarent déjà. C'est la contrainte de l'item, et c'est
 * elle qui rend la réponse utilisable : un pli qui aurait supposé un champ absent aurait répondu
 * « oui » à une question qu'il aurait lui-même truquée.
 */

/** Ce qu'un nœud du graphe d'exécution peut être. */
export type NodeKind = "task" | "attempt" | "worker" | "lease" | "tool" | "artifact" | "run";

/**
 * Les arêtes, orientées de la partie vers ce dont elle dépend.
 *
 * `produced_by` et `executed_by` pointent donc vers le producteur et l'exécutant : un graphe de
 * provenance se lit en remontant, et c'est le sens dans lequel on demande « d'où vient ceci ».
 */
export type EdgeKind =
  | "belongs_to"
  | "executed_by"
  | "granted_for"
  | "invoked_in"
  | "produced_by"
  | "consumed_by"
  | "derived_from"
  | "recorded_for";

export type GraphNode = {
  readonly id: string;
  readonly kind: NodeKind;
};

export type GraphEdge = {
  readonly from: string;
  readonly to: string;
  readonly kind: EdgeKind;
};

export type ExecutionGraph = {
  readonly nodes: readonly GraphNode[];
  readonly edges: readonly GraphEdge[];
};

/** Une arête dont une extrémité ne désigne aucun nœud. */
export type OrphanEdge = {
  readonly edge: GraphEdge;
  readonly missing: readonly string[];
};

type Mutable = {
  readonly nodes: Map<string, GraphNode>;
  readonly edges: Map<string, GraphEdge>;
};

/**
 * Les identifiants sont **préfixés par leur sorte**.
 *
 * Un `task_id` et un `run_id` peuvent porter la même chaîne sans désigner la même chose ; sans
 * préfixe, deux nœuds distincts fusionneraient en silence et le graphe aurait l'air plus connexe
 * qu'il ne l'est.
 */
export function nodeId(kind: NodeKind, key: string | number): string {
  return `${kind}:${key}`;
}

function put(graph: Mutable, kind: NodeKind, key: string | number): string {
  const id = nodeId(kind, key);
  if (!graph.nodes.has(id)) graph.nodes.set(id, { id, kind });
  return id;
}

/**
 * Une arête est un **fait**, pas une occurrence.
 *
 * `task-nominal#1 appartient à task-nominal` est écrit par la mission, par la lease, par l'attempt
 * et par chaque événement. Empiler quatre arêtes identiques ferait d'un graphe de dépendances un
 * histogramme de mentions, et le premier calcul de degré s'en trouverait faux.
 */
function link(graph: Mutable, from: string, to: string, kind: EdgeKind): void {
  graph.edges.set(`${kind}|${from}|${to}`, { from, to, kind });
}

/** Le document, tel qu'il arrive : une forme JSON quelconque, à interroger prudemment. */
type Document = Record<string, unknown>;

function text(document: Document, field: string): string | undefined {
  const value = document[field];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function count(document: Document, field: string): number | undefined {
  const value = document[field];
  return typeof value === "number" && Number.isInteger(value) ? value : undefined;
}

function record(value: unknown): Document | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Document)
    : undefined;
}

function list(value: unknown): readonly Document[] {
  if (!Array.isArray(value)) return [];
  return (value as unknown[]).flatMap((item): Document[] => {
    const entry = record(item);
    return entry === undefined ? [] : [entry];
  });
}

/**
 * Plier une collection de documents en un graphe.
 *
 * Chaque document est reconnu par les champs qu'il porte, pas par un nom de fichier : le corpus de
 * fixtures est nommé par ce qu'il démontre, et lier le pli à ces noms le casserait au premier
 * exemple ajouté.
 */
export function fold(documents: readonly Document[]): ExecutionGraph {
  const graph: Mutable = { nodes: new Map(), edges: new Map() };
  for (const document of documents) {
    absorb(graph, document);
  }
  return { nodes: [...graph.nodes.values()], edges: [...graph.edges.values()] };
}

function absorb(graph: Mutable, document: Document): void {
  const taskId = text(document, "task_id");
  const attempt = count(document, "attempt");
  const attemptNode =
    taskId !== undefined && attempt !== undefined ? attemptOf(graph, taskId, attempt) : undefined;

  workerOf(graph, document, attemptNode);
  leaseOf(graph, document, attemptNode);
  toolOf(graph, document, attemptNode);
  artifactOf(graph, document);
  runOf(graph, document, attemptNode);
}

/** Un attempt appartient à sa tâche. Les deux nœuds naissent ensemble. */
function attemptOf(graph: Mutable, taskId: string, attempt: number): string {
  const task = put(graph, "task", taskId);
  const node = put(graph, "attempt", `${taskId}#${attempt}`);
  link(graph, node, task, "belongs_to");
  return node;
}

function workerOf(graph: Mutable, document: Document, attemptNode: string | undefined): void {
  const workerId = text(document, "worker_id");
  if (workerId === undefined) return;
  const worker = put(graph, "worker", workerId);
  if (attemptNode !== undefined) link(graph, attemptNode, worker, "executed_by");
}

function leaseOf(graph: Mutable, document: Document, attemptNode: string | undefined): void {
  const leaseId = text(document, "lease_id");
  if (leaseId === undefined) return;
  const lease = put(graph, "lease", leaseId);
  if (attemptNode !== undefined) link(graph, lease, attemptNode, "granted_for");
}

/**
 * Un appel d'outil, lu dans le payload d'un événement `tool.*`.
 *
 * Le nom de l'outil est la seule identité disponible : `lep/1.0` ne donne pas d'identifiant
 * d'appel. Deux appels du même outil dans le même attempt sont donc **un seul nœud**, et c'est une
 * perte réelle — notée ici plutôt que masquée par un identifiant fabriqué, qui rendrait le graphe
 * plus précis qu'il ne l'est.
 */
function toolOf(graph: Mutable, document: Document, attemptNode: string | undefined): void {
  const eventType = text(document, "event_type");
  if (eventType === undefined || !eventType.startsWith("tool.")) return;
  const tool = text(record(document["payload"]) ?? {}, "tool");
  if (tool === undefined) return;
  const node = put(graph, "tool", tool);
  if (attemptNode !== undefined) link(graph, node, attemptNode, "invoked_in");
}

function artifactOf(graph: Mutable, document: Document): void {
  const artifactId = text(document, "artifact_id");
  const producedBy = record(document["produced_by"]);
  if (artifactId === undefined || producedBy === undefined) return;

  const node = put(graph, "artifact", artifactId);
  const taskId = text(producedBy, "task_id");
  const attempt = count(producedBy, "attempt");
  if (taskId !== undefined && attempt !== undefined) {
    link(graph, node, attemptOf(graph, taskId, attempt), "produced_by");
  }
  for (const parent of list(document["derived_from"])) {
    const parentId = text(parent, "artifact_id");
    if (parentId === undefined) continue;
    link(graph, node, put(graph, "artifact", parentId), "derived_from");
  }
}

function runOf(graph: Mutable, document: Document, attemptNode: string | undefined): void {
  const runId = text(document, "run_id");
  if (runId === undefined) return;
  const node = put(graph, "run", runId);
  if (attemptNode !== undefined) link(graph, node, attemptNode, "recorded_for");

  for (const input of list(document["inputs"])) {
    const artifactId = text(input, "artifact_id");
    if (artifactId === undefined) continue;
    link(graph, put(graph, "artifact", artifactId), node, "consumed_by");
  }
  for (const output of list(document["outputs"])) {
    const artifactId = text(output, "artifact_id");
    if (artifactId === undefined) continue;
    link(graph, put(graph, "artifact", artifactId), node, "produced_by");
  }
}

/**
 * Les arêtes dont une extrémité ne désigne aucun nœud.
 *
 * C'est la garantie du pli : un graphe où une arête pointe dans le vide se parcourt sans erreur et
 * ment à chaque parcours. Le test de sortie exige la liste vide.
 */
export function orphanEdges(graph: ExecutionGraph): OrphanEdge[] {
  const known = new Set(graph.nodes.map((node) => node.id));
  return graph.edges.flatMap((edge) => {
    const missing = [edge.from, edge.to].filter((endpoint) => !known.has(endpoint));
    return missing.length > 0 ? [{ edge, missing }] : [];
  });
}

/** Les nœuds d'une sorte donnée. */
export function nodesOfKind(graph: ExecutionGraph, kind: NodeKind): GraphNode[] {
  return graph.nodes.filter((node) => node.kind === kind);
}

/** Les arêtes d'une sorte donnée. */
export function edgesOfKind(graph: ExecutionGraph, kind: EdgeKind): GraphEdge[] {
  return graph.edges.filter((edge) => edge.kind === kind);
}
