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

import { ADMINISTRATION, empreinte, startChain, type Chain } from "./harness.ts";

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

/**
 * Les faits au journal, une fois que celui qu'on attend y est — ou après la dernière tentative.
 *
 * Rend la liste **dans tous les cas** plutôt que de lever sur l'attente : l'appelant veut affirmer
 * sur ce qu'il a vu, et un `throw` ici priverait son message des faits réellement écrits — c'est-à-
 * dire du seul renseignement utile quand la clause échoue.
 */
async function attendre(controlPlane: string, fait: string, tours = 40): Promise<string[]> {
  let vus: string[] = [];
  for (let tour = 0; tour < tours; tour += 1) {
    const reponse = await fetch(`${controlPlane}/timeline?limit=100`);
    if (reponse.status === 200) {
      const corps = (await reponse.json()) as { readonly items: readonly { event_type: string }[] };
      vus = corps.items.map((item) => item.event_type);
      if (vus.includes(fait)) return vus;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  return vus;
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

  /**
   * **Le broker nomme son hôte, et ce nom sort d'ici** — `W5.x`.
   *
   * # Ce que ce test tient
   *
   * `W5.w` a fait imprimer l'empreinte d'hôte au démarrage de `locus-execd`, pour que qui prépare
   * un fichier d'attestations sache à quel hôte le lier — un refus qui nomme le problème et cache
   * le remède était le défaut qu'il retirait. Rien ne vérifiait qu'un binaire **réel** produise
   * cette ligne : `attestation.rs` exerce la fonction, jamais le démarrage. Ce test est cette garde.
   *
   * # Et ce qu'il mesure, en passant
   *
   * La roadmap de `W5.u` laisse une question ouverte, écrite dans le workflow : le fichier déposé
   * par le job `sandbox` n'est lu par aucun `locus-execd` de CI, parce que le consommateur vivrait
   * dans un autre job — donc sur un autre runner — et que **rien n'établit que deux runners rendent
   * la même empreinte**. Le job `e2e` démarre le seul `locus-execd` de toute la CI qui monte par le
   * harnais ; son empreinte, sortie ici, se compare à celle que `sandbox` publie. Une exécution
   * répond, là où la conjecture durait depuis trois items.
   *
   * # Pourquoi l'assertion ne porte pas sur les *valeurs*
   *
   * Exiger `cgroup_v2=available` ferait de ce test une affirmation sur les runners GitHub, qui
   * rougirait sur la machine d'un contributeur sans que rien ne soit cassé. Ce qui est exigé est la
   * **forme** : les cinq faits que `fingerprint` compose, chacun sous son nom. Un fait qui
   * disparaîtrait affaiblirait l'empreinte sans que personne ne le voie.
   */
  it("le broker annonce l'empreinte de son hôte — W5.x", (t) => {
    assert.ok(chain, "la chaîne est montée par le test précédent");

    // `annonce` lève sur un nom inconnu plutôt que de rendre `""` : affirmer sur un silence qu'on a
    // fabriqué soi-même est exactement ce que ce test existe pour empêcher.
    const vue = empreinte(chain.annonce("locus-execd"));

    // Les cinq faits, chacun sous son nom et dans l'ordre où `fingerprint` les compose.
    // `controllers` accepte le vide — un hôte sans contrôleur cgroup est un hôte pauvre, pas une
    // empreinte cassée —, les quatre autres non : un verdict est toujours l'un des trois mots.
    assert.match(
      vue,
      /^cgroup_v2=\S+ controllers=\S* userns=\S+ seccomp=\S+ disk_quota=\S+$/,
      `l'empreinte annoncée ne porte pas les cinq faits : « ${vue} »`,
    );

    // La mesure, dans le log du job. Sans cette ligne, l'empreinte de ce runner reste dans un
    // tampon que le harnais ne rend que sur un échec — et la question de `W5.u` resterait ouverte
    // sur un tour vert, ce qui est précisément la façon dont elle a survécu à trois items.
    t.diagnostic(`empreinte d'hôte du runner e2e : ${vue}`);
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

  /**
   * **La deuxième clause de `W12.d` : un worker s'enregistre — `W20.z`.**
   *
   * Ce que ce test ajoute au premier, et qui manquait entièrement : le worker de la chaîne
   * s'enrôle **pour de vrai** — paire de clés, signature, créance écrite —, puis sa boucle
   * s'adresse à §15.2 sous cette créance, et le daemon **écrit** le fait.
   *
   * Trois défauts se tenaient sur ce chemin, et aucun n'était visible autrement :
   *
   * - `runWorker` exigeait un `locus.identity` que l'enrôlement ne remplit pas, donc un worker
   *   correctement enrôlé restait `inert` ;
   * - `lepCall` jetait le corps des refus typés, donc le suivant a demandé un rejeu manuel ;
   * - `/lep/v1/claim` exigeait un `project_id` que le worker n'envoie pas — et n'avait pas à
   *   envoyer, puisque son grant le porte (`W20.w`, généralisé par `W20.z`).
   *
   * Le fait est lu **au journal** et non déduit d'un code HTTP : `202` dit que le daemon a accepté,
   * pas qu'il a écrit, et c'est l'écriture qui est la clause.
   */
  it("un worker réel s'enrôle et s'enregistre — W12.d, deuxième clause", async () => {
    assert.ok(chain, "la chaîne est montée par le premier test");

    // La boucle du worker tourne depuis `startChain` ; le fait arrive de façon asynchrone. On
    // sonde plutôt qu'on attend une durée fixe — une temporisation choisie au jugé rend un test
    // lent sur une machine rapide et intermittent sur une machine lente.
    const registre = await attendre(chain.controlPlane, "worker.registered");

    assert.ok(
      registre.includes("worker.registered"),
      `le worker ne s'est pas enregistré — faits au journal : ${registre.join(", ")}`,
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
