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

import {
  ADMINISTRATION,
  ATTESTATIONS_ATTENDUES_ENV,
  attestations,
  empreinte,
  startChain,
  type Chain,
} from "./harness.ts";

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
   * La roadmap de `W5.aa` laisse une question ouverte, écrite dans le workflow : le fichier déposé
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
    // tampon que le harnais ne rend que sur un échec — et la question de `W5.aa` resterait ouverte
    // sur un tour vert, ce qui est précisément la façon dont elle a survécu à trois items.
    t.diagnostic(`empreinte d'hôte du runner e2e : ${vue}`);
  });

  /**
   * **Ce que le broker fait du fichier d'attestations du job `sandbox`** — `W5.ab`.
   *
   * # Les deux moitiés qui ne se rencontraient pas
   *
   * `W5.z` a livré la **lecture** des attestations, `W5.aa` leur **dépôt**, et aucun assemblage ne
   * joignait les deux : le fichier écrit par le job `sandbox` n'était lu par aucun `locus-execd`. Le
   * workflow le disait lui-même — le consommateur vit dans un autre job, donc sur un autre runner, et
   * rien n'établissait que deux runners rendent la même empreinte.
   *
   * `W5.x` l'a mesuré : ils rendent la **même**, caractère pour caractère. Le convoyeur devient donc
   * justifié par une lecture au lieu d'être supposé, et ce test est ce qui l'exerce.
   *
   * # Ce qui est exigé, et ce qui est seulement rapporté
   *
   * Le broker **dit toujours** ce qu'il a fait du fichier — un silence est une panne, jamais un `S0`
   * nominal. Le workflow déclare par ailleurs s'il a **posé** un fichier, et le constat doit lui
   * correspondre : c'est la seule façon de distinguer « le job `sandbox` n'a rien déposé ce tour-ci »,
   * qui est un état extérieur légitime, de « le câblage a cessé de poser la variable », qui est une
   * régression. Les lire l'un pour l'autre est la faute que ce dépôt nomme partout.
   *
   * # Pourquoi une attestation écartée **échoue**
   *
   * Sur un parc dont on a mesuré l'homogénéité, un enregistrement écarté veut dire l'une de deux
   * choses, et le message les sépare : ou bien le chemin d'enregistrement s'est cassé — c'est notre
   * défaut, et c'est exactement ce qu'on veut voir rougir —, ou bien le parc a cessé d'être homogène,
   * ce qui est une nouvelle sur laquelle repose tout le dessin de `W5.z`. Les deux méritent d'être
   * apprises bruyamment ; les taire rendrait vert un convoyeur qui ne convoie plus rien.
   */
  it("le broker dit ce qu'il a fait du fichier d'attestations — W5.ab", (t) => {
    assert.ok(chain, "la chaîne est montée par le test précédent");

    const annonce = chain.annonce("locus-execd");
    const lues = attestations(annonce);
    const pose = process.env[ATTESTATIONS_ATTENDUES_ENV] === "1";

    t.diagnostic(
      `attestations : ${JSON.stringify(lues)} (le workflow en a ${pose ? "posé" : "posé aucune"})`,
    );

    if (!pose) {
      // Aucun fichier posé — hors CI, ou un job `sandbox` qui n'a rien déposé ce tour-ci. Le broker
      // doit le dire, et ne rien placer au-dessus de `S0`. C'est le cas nominal d'une machine de
      // développeur, et il reste une affirmation entière : un broker qui compterait des attestations
      // sans fichier posé serait tout aussi faux qu'un broker muet.
      assert.equal(lues.kind, "aucune", `aucun fichier posé, et pourtant : ${annonce}`);
      return;
    }

    assert.equal(lues.kind, "lues", `le workflow a posé un fichier, et le broker dit : ${annonce}`);
    assert.ok(lues.kind === "lues");

    assert.equal(
      lues.etrangeres,
      0,
      "des attestations sont écartées alors que `W5.x` a mesuré les deux runners identiques. " +
        `Le broker dit que cet hôte est « ${lues.hote ?? "—"} » ; si c'est bien celui du job ` +
        "`sandbox`, le chemin d'enregistrement est cassé ; sinon le parc a cessé d'être homogène, " +
        "et c'est l'hypothèse sur laquelle repose tout le dessin de `W5.z`",
    );
    assert.ok(
      lues.honorees > 0,
      "un fichier posé et zéro attestation retenue : la campagne du job `sandbox` a déposé un " +
        "fichier que ce broker lit sans rien y trouver",
    );
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

/**
 * Le tour d'un worker sur une mission réellement en file — `W12.d`, troisième clause.
 *
 * # Pourquoi une seconde chaîne, sur son propre port
 *
 * `runLoop` de `canterel` fait **un** tour — réclamer, planifier, ouvrir la session, faire remonter,
 * rendre — puis le processus sort. Ce n'est pas une boucle malgré son nom, et `W2.24` l'a rendu
 * visible en faisant dire au worker ce que son tour a fait : contre la chaîne du bloc précédent, il
 * annonce « tour : aucune mission à réclamer », **parce que le harnais le démarre avant qu'aucune
 * mission ne soit en file**.
 *
 * Ce n'était pas un défaut du worker, et la chaîne du bloc précédent reste juste : elle décrit
 * `W12.f`, « les trois processus démarrent, se voient et s'arrêtent ». Exercer un **placement**
 * demande l'ordre inverse — mettre en file, puis faire le tour —, et les deux ordres ne peuvent pas
 * vivre dans la même chaîne.
 *
 * # Ce que cette clause tient
 *
 * Que le tour **atteigne la réclamation**, et que ce qu'il en rapporte soit lisible. Ce qu'elle ne
 * tient pas encore : que la mission traverse jusqu'au bout — session ouverte, événements remontés,
 * artefacts hashés. Ces termes sont la suite de `W12.d`, et les affirmer ici rendrait vert sur
 * « le worker a réclamé » le jour où l'exécution cesserait d'aboutir.
 */
describe("un worker fait son tour sur une mission en file — W12.d, troisième clause", () => {
  let chain: Chain | undefined;

  after(async () => {
    await chain?.stop();
  });

  it("la chaîne monte sans worker, et la mission est mise en file d'abord", async () => {
    chain = await startChain({ root: ROOT, port: 8790, worker: "à la demande" });

    // Deux processus, pas trois : c'est le sens de « à la demande », et l'affirmer ici est ce qui
    // empêche l'option de redevenir silencieusement le défaut.
    assert.deepEqual(chain.processes, ["locus-execd", "locusd"]);

    for (const [route, corps] of [
      [
        "/commands/task/propose",
        {
          idempotency_key: "e2e-tour-propose",
          project_id: ADMINISTRATION.projectId,
          proposal: proposition(),
        },
      ],
      [
        "/commands/task/queue",
        { idempotency_key: "e2e-tour-queue", project_id: ADMINISTRATION.projectId, task_id: TACHE },
      ],
    ] as const) {
      const reponse = await fetch(`${chain.controlPlane}${route}`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          authorization: `Bearer ${ADMINISTRATION.credential}`,
        },
        body: JSON.stringify(corps),
      });
      assert.equal(reponse.status, 202, `${route} — ${await reponse.text()}`);
    }

    assert.ok((await attendre(chain.controlPlane, "task.queued")).includes("task.queued"));
  });

  /**
   * **La décision de placement est lisible — d'un côté ou de l'autre, jamais des deux silences.**
   *
   * # Ce que la première rédaction affirmait, et que l'exécution a démentie
   *
   * Elle exigeait que le tour **réclame** la mission, sur l'hypothèse que le harnais démarrait
   * simplement le worker trop tôt. L'hypothèse était fausse, et une seule exécution l'a montré : la
   * mission était bien en file, le tour a bien eu lieu après, et le worker a quand même annoncé
   * « aucune mission à réclamer ». La cause est dans la sortie de `locusd`, et elle est **correcte** :
   *
   * ```text
   * « task_… » retourne en file : aucun des 1 worker(s) soumis ne convient — « canterel-… » :
   *   confinement S2 exigé, l'hôte ne sait pas dépasser S1 — changer de machine ;
   *   l'hôte ne sait pas appliquer le mode réseau Deny ;
   *   confinement S2 annoncé mais jamais prouvé, aucune campagne n'a conclu — lancer les self-tests
   * ```
   *
   * Le worker s'est bien soumis, et le daemon a refusé pour trois motifs de §10.2 tous exacts sur
   * cette machine. Exiger la réclamation aurait donc fait de cette clause une affirmation sur
   * **l'hôte** — rouge sur un conteneur de développement, verte sur un runner capable —, ce que
   * `W5.x` a refusé d'écrire quelques heures plus tôt pour l'empreinte.
   *
   * # Ce qui est affirmé à la place, et qui vaut sur toute machine
   *
   * La décision **se lit**. Ou le worker rapporte la mission qu'il a prise, ou le daemon dit
   * pourquoi elle retourne en file, avec ses motifs nommés. Les deux silences à la fois seraient le
   * défaut : une mission en file, un worker soumis, et personne qui dise ce qui a été décidé.
   *
   * C'est la propriété que cette session a vue enfreinte cinq fois — `W20.aa` pour le `204`, `W5.w`
   * pour l'empreinte, `W5.x` pour le harnais, `W2.24` pour le tour du worker. Ici elle est tenue des
   * deux côtés à la fois, ce qu'aucun des quatre ne pouvait faire seul.
   */
  it("une mission en file est soumise, et la décision de placement se lit", async (t) => {
    assert.ok(chain, "la chaîne est montée par le test précédent");

    const dit = await chain.tourDeWorker();
    const tour = dit
      .split("\n")
      .map((ligne) => ligne.trim())
      .filter((ligne) => ligne.startsWith("tour :"));

    const daemon = chain
      .annonce("locusd")
      .split("\n")
      .map((ligne) => ligne.trim())
      .filter((ligne) => ligne.includes(TACHE));

    t.diagnostic(`le worker : ${tour.join(" | ") || "(rien)"}`);
    t.diagnostic(`le daemon : ${daemon.join(" | ") || "(rien)"}`);

    // Le worker rend compte de son tour, quel qu'il soit — `W2.24`. Son silence voudrait dire que le
    // pin de `WORKER-PINNED.json` précède cet item, et le message le dit plutôt que de laisser
    // chercher.
    assert.ok(
      tour.length > 0,
      "le worker n'a rien dit de son tour. Si le pin de `WORKER-PINNED.json` précède `W2.24`, " +
        `c'est attendu et le bump est le remède. Ce qu'il a écrit : ${dit}`,
    );

    const reclame = !tour.some((ligne) => ligne.includes("aucune mission à réclamer"));
    const refuse = daemon.some((ligne) => ligne.includes("retourne en file"));

    // La clause : **l'un des deux**, jamais aucun. Un placement sans trace et un refus sans motif se
    // ressemblent trait pour trait dans un log, et c'est cette confusion qui est interdite ici.
    assert.ok(
      reclame || refuse,
      "une mission était en file, un worker s'est soumis, et ni le worker ni le daemon ne disent " +
        `ce qui a été décidé. Le worker : ${tour.join(" | ")}. Le daemon : ${daemon.join(" | ")}`,
    );

    // Et quand c'est un refus, il **nomme** ses motifs — `W20.aa`. Un « ne convient pas » nu
    // enverrait chercher sur trois machines à la fois.
    if (refuse) {
      const note = daemon.find((ligne) => ligne.includes("retourne en file")) ?? "";
      assert.match(note, /ne convient|:/, `le refus de placement ne nomme aucun motif : ${note}`);
    }
  });
});
