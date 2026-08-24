/**
 * La chaîne montée pour de vrai — `W12.f`, ADR 0033.
 *
 * # Pourquoi `.chain.ts` et pas `.test.ts`
 *
 * `npm test` ramasse `tests/**\/*.test.ts` ; ce fichier échappe donc au glob, et c'est délibéré.
 * Il demande un second dépôt, une chaîne Bun et deux binaires construits. Le faire entrer dans
 * `npm run check` rendrait rouge, sur la machine d'un contributeur qui vient d'écrire trois lignes
 * de domaine, un contrôle qui n'a rien à voir avec ce qu'il a écrit.
 *
 * Conséquence assumée et nommée dans l'ADR : **une session qui ne lance que `npm run check`
 * n'exerce pas la chaîne**. C'est la CI qui la tient, par `npm run e2e`.
 *
 * # Ce que ce fichier prouve, et ce qu'il ne prouve pas
 *
 * Il prouve que les trois processus **démarrent, se voient et s'arrêtent**. Il ne prouve rien sur
 * la science : `e2e/minimal_science` — la mission qui traverse, les artefacts hashés, le graphe
 * servi — est le travail de `W12.d`, qui s'écrira ici même en s'appuyant sur ce harnais.
 *
 * La séparation est écrite plutôt que subie : un fichier qui prétendrait faire les deux rendrait
 * vert sur « les processus démarrent » le jour où la mission cesserait de traverser.
 */

import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { after, describe, it } from "node:test";

import { ADMINISTRATION, startChain, type Chain } from "./harness.ts";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** La tâche que la question ouvre. Figée, pour la même raison que les identifiants du harnais. */
const TACHE = "task_01HF7YAT000000000000000005";

/**
 * La proposition, **transcrite** de `apps/locusd/tests/commands.rs` et non réinventée.
 *
 * Les deux tests décrivent la même question, ce qui est le seul moyen de savoir que ce qui échoue
 * ici est le câblage du binaire et non une proposition écrite de travers. Une fixture propre à ce
 * fichier aurait donné un second corps à maintenir, et un `400` sur un champ oublié se serait lu
 * comme un refus du daemon.
 */
function proposition() {
  return {
    statement: "Le catalyseur A tient-il au-delà de 300 °C ?",
    success_conditions: ["une mesure reproductible à trois essais"],
    task_id: TACHE,
    attempt_id: "att_1",
    attempt: 3,
    branch_id: "br_principal",
    context_view_id: "ctx_1",
    context_view_hash: `sha256:${"ab".repeat(32)}`,
    environment_id: "env_linux",
    // `S2` et non `S3` : ADR 0033 et `W12.e` séparent le confinement qu'un runner peut tenir de
    // celui qui est **attesté**, et ce test tourne sur le premier.
    sandbox_level: "S2",
    network: "deny",
    resources: { cpu: 2.0, memory_mb: 4096, disk_mb: 8192, wall_time_seconds: 900 },
    budget: { max_model_calls: 40, max_input_tokens: 200_000, max_output_tokens: 40_000 },
    output_contract: "un rapport et ses mesures",
  };
}

describe("la chaîne monte, se voit et s'arrête — W12.f", () => {
  let chain: Chain | undefined;

  after(async () => {
    await chain?.stop();
  });

  it("les trois processus démarrent, dans l'ordre du harnais", async () => {
    // Aucun `try`/`catch` : si `startChain` lève, le test échoue **avec son message**, qui nomme le
    // processus fautif et porte sa sortie. L'attraper pour le retraduire perdrait exactement ce que
    // le harnais a pris soin de garder.
    chain = await startChain({ root: ROOT, port: 8789 });

    assert.deepEqual(chain.processes, ["locus-execd", "locusd", "canterel worker"]);
  });

  it("`locusd` sert ses projections, et aucune n'est en quarantaine", async () => {
    assert.ok(chain, "la chaîne est montée par le test précédent");

    const response = await fetch(`${chain.controlPlane}/projections/status`);
    assert.equal(response.status, 200);

    const body = (await response.json()) as {
      readonly ready: boolean;
      readonly projections: readonly { readonly name: string; readonly healthy: boolean }[];
    };
    // `ready` **et** la liste : `Readiness::is_ready` rend `false` sur une liste vide, mais un
    // `ready: true` seul ne dirait pas combien de projections l'ont produit. Les cinq sont nommées
    // ailleurs (`apps/locusd/tests/composition.rs`) ; ici on vérifie qu'il y en a, et qu'aucune ne
    // sert des lectures périmées.
    assert.equal(body.ready, true);
    assert.ok(body.projections.length > 0, "un daemon sans projection câblée n'est pas prêt");
    assert.deepEqual(
      body.projections.filter((projection) => !projection.healthy),
      [],
    );
  });

  /**
   * **La première clause de `W12.d`, contre un daemon réel — `W20.y`.**
   *
   * Elle était tenue par `apps/locusd/tests/commands.rs` sur un `Runtime` monté dans le test, dont
   * le registre d'administration était rempli **par le test lui-même**. Ce que ce test-ci ajoute est
   * la seule chose qui manquait, et elle manquait entièrement : que le **binaire** câble ce
   * registre. Il ne le faisait pas — `NoAdministrators` refusait §22.3 à toute créance, et la
   * réponse observée sur un daemon réel était :
   *
   * ```text
   * 403 { "family": "authorization",
   *       "detail": "« commander §22.3 sans autorité d'administration reconnue » n'est pas permis" }
   * ```
   *
   * C'est la différence entre un port testé et un produit assemblé, et rien d'autre que cette
   * chaîne ne pouvait la voir.
   */
  it("une question posée à un daemon réel produit une mission — W12.d, première clause", async () => {
    assert.ok(chain, "la chaîne est montée par le premier test");

    const propose = await fetch(`${chain.controlPlane}/commands/task/propose`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${ADMINISTRATION.credential}`,
      },
      body: JSON.stringify({
        idempotency_key: "e2e-propose-1",
        project_id: ADMINISTRATION.projectId,
        proposal: proposition(),
      }),
    });

    // Le corps est lu dans les deux cas : un `assert.equal(202)` seul rendrait « attendu 202, reçu
    // 403 » sans dire lequel des refus typés de §22.5 a parlé, et c'est le `detail` qui distingue
    // une autorité absente d'un champ mal formé.
    assert.equal(propose.status, 202, await propose.text());

    const queue = await fetch(`${chain.controlPlane}/commands/task/queue`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${ADMINISTRATION.credential}`,
      },
      body: JSON.stringify({
        idempotency_key: "e2e-queue-1",
        project_id: ADMINISTRATION.projectId,
        task_id: TACHE,
      }),
    });
    assert.equal(queue.status, 202, await queue.text());

    // Les deux faits sont au journal, sous leurs noms de §22.1. Vérifier la réponse `202` seule
    // dirait que le daemon a accepté, pas qu'il a **écrit** — et c'est l'écriture qui est la clause.
    const timeline = await fetch(`${chain.controlPlane}/timeline?limit=100`);
    assert.equal(timeline.status, 200);
    const faits = ((await timeline.json()) as { readonly items: readonly { event_type: string }[] })
      .items;
    const types = faits.map((item) => item.event_type);
    assert.ok(types.includes("task.proposed"), `faits : ${types.join(", ")}`);
    assert.ok(types.includes("task.queued"), `faits : ${types.join(", ")}`);
  });

  /**
   * **Le pendant négatif : une autre créance n'administre rien.**
   *
   * Sans lui, le test précédent passerait tout aussi bien sur un daemon qui aurait remplacé
   * `NoAdministrators` par « tout le monde administre » — un amorçage qui accorderait à quiconque
   * atteint le port serait la porte d'entrée que `W20.y` existe pour ne pas être.
   */
  it("une créance qui n'est pas celle de l'amorçage n'administre rien", async () => {
    assert.ok(chain, "la chaîne est montée par le premier test");

    const refus = await fetch(`${chain.controlPlane}/commands/task/queue`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: "Bearer une-creance-que-personne-n-a-posee",
      },
      body: JSON.stringify({ idempotency_key: "e2e-refus", task_id: TACHE }),
    });

    assert.equal(refus.status, 403);
    const corps = (await refus.json()) as { readonly family: string };
    assert.equal(corps.family, "authorization");
  });

  it("tout s'arrête, et le port se libère", async () => {
    assert.ok(chain, "la chaîne est montée par le premier test");

    await chain.stop();
    chain = undefined;

    // Un harnais qui laisserait un `locusd` derrière lui ferait échouer l'exécution suivante sur un
    // port occupé, et le second message cacherait le premier. On le vérifie plutôt que de l'espérer.
    await assert.rejects(
      fetch("http://127.0.0.1:8789/projections/status", { signal: AbortSignal.timeout(2_000) }),
      "le daemon ne répond plus",
    );
  });
});
