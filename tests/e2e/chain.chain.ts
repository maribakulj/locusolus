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

import { startChain, type Chain } from "./harness.ts";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");

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
