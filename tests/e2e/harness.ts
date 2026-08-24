/**
 * Le harnais de `e2e/minimal_science` — `W12.f`, ADR 0033.
 *
 * # Ce qu'il fait, et ce qu'il ne fait jamais
 *
 * Il démarre les **trois** processus de la chaîne — `locusd`, `locus-execd`, et le worker
 * `canterel` réel — et les arrête. Il ne simule aucun des trois : `packages/testing` joue le
 * serveur par construction (`W0.9`), donc il ne peut pas jouer le client, et un faux worker
 * prouverait que `locusd` parle à quelque chose plutôt que la chaîne tient.
 *
 * **Il ne se saute jamais.** Un prérequis absent — le dépôt worker, un binaire non construit, un
 * processus qui refuse de démarrer — est une **panne**, pas une raison de se déclarer non
 * applicable. C'est la règle du dépôt, et `W20.i` a montré ce qu'elle coûte quand on l'oublie : un
 * test qui se saute lui-même rend vert un dossier que personne n'a exercé, et le rouge attendu
 * n'arrive jamais.
 *
 * # Pourquoi il vit ici et pas dans `canterel`
 *
 * ADR 0033, mesures à l'appui. Trois raisons : la CI de `locusolus` a déjà une sandbox `podman`
 * réelle et celle de `canterel` n'en a pas ; le verdict appartient à l'endroit où la clause est
 * écrite ; et la suite amont de `canterel` s'est révélée instable — trois exécutions sur un arbre
 * identique pour un seul tour entièrement vert. Un verdict qui peut rougir pour une raison
 * étrangère cesse d'être lu.
 */

import { spawn, type ChildProcessByStdio } from "node:child_process";
import type { Readable } from "node:stream";
import { existsSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

/** La variable qui dit où le dépôt worker se trouve. */
export const WORKER_REPO_ENV = "LOCUS_E2E_WORKER";

/**
 * La variable par laquelle `canterel` accepte qu'on lui donne un autre répertoire d'état.
 *
 * Nommée et exportée plutôt qu'écrite dans l'appel à `spawn` : c'est un **couplage entre deux
 * dépôts**, et un couplage qu'on ne voit pas est un couplage que personne ne vérifie. La première
 * rédaction posait `XDG_DATA_HOME`, que `canterel` ne lit pas — le harnais partageait alors
 * l'installation de la machine sans le dire, et son verdict dépendait de l'état de cet hôte.
 *
 * Lu dans `backend/cli/src/global/index.ts` : `Global.Path.data` dérive de cette variable, ou du
 * home réel.
 */
export const WORKER_HOME_ENV = "OPENSCIENCE_TEST_HOME";

/**
 * L'autorité d'administration que le harnais amorce sur `locusd` — `W20.y`.
 *
 * # Pourquoi le harnais en a besoin
 *
 * Les deux commandes de §22.3 — proposer et mettre en file — sont servies depuis `W20.s`, et
 * `NoAdministrators` les refusait à **toute** créance tant que rien n'amorçait l'administration.
 * Sans cet amorçage, la première clause de `W12.d` — « une question produit une mission » — ne peut
 * pas s'exécuter contre un daemon réel.
 *
 * # Des identifiants figés, et pas tirés au hasard
 *
 * Un harnais qui tirerait des ULID neufs à chaque exécution rendrait ses échecs irreproductibles :
 * le message d'un refus citerait un identifiant qu'on ne retrouve dans aucune trace. Ceux-ci sont
 * les identifiants de fixture des tests Rust, sous la même graine — un identifiant vu dans une
 * sortie de CI se retrouve donc dans le code.
 *
 * La créance, elle, n'est pas un secret : elle vaut sur un daemon éphémère lié à un port local, et
 * l'écrire ici est ce qui permet à un lecteur de savoir exactement ce que le harnais accorde.
 */
export const ADMINISTRATION = {
  credential: "creance-e2e-locale",
  workspaceId: "ws_01HF7YAT000000000000000002",
  principalId: "agent_01HF7YAT000000000000000003",
  projectId: "prj_01HF7YAT000000000000000004",
} as const;

/** Les variables par lesquelles `locusd` reçoit son amorçage d'administration — `W20.y`. */
export const ADMINISTRATION_ENV = {
  credential: "LOCUSD_ADMIN_CREDENTIAL",
  workspace: "LOCUSD_ADMIN_WORKSPACE",
  principal: "LOCUSD_ADMIN_PRINCIPAL",
} as const;

/** Combien de temps un processus a pour devenir joignable avant qu'on le déclare mort. */
const BOOT_TIMEOUT_MS = 30_000;

/** Ce que le harnais n'a pas pu faire, et pourquoi. */
export class HarnessFailure extends Error {
  /** Ce qui manquait ou n'a pas démarré, sous son nom. */
  readonly subject: string;
  /** Ce que le processus a écrit avant de mourir, s'il a écrit quelque chose. */
  readonly output: string;

  constructor(subject: string, detail: string, output = "") {
    super(
      `harnais e2e — ${subject} : ${detail}` +
        (output.trim() === "" ? "" : `\n--- sortie ---\n${output.trim()}`),
    );
    this.name = "HarnessFailure";
    this.subject = subject;
    this.output = output;
  }
}

/** Un processus démarré par le harnais. */
type Started = {
  readonly name: string;
  /**
   * `stdin` est `null` et les deux sorties sont des flux : c'est exactement ce que
   * `stdio: ["ignore", "pipe", "pipe"]` produit.
   *
   * `ChildProcessWithoutNullStreams` était le premier type écrit, et `npm test` ne l'a pas démenti —
   * Node retire les types sans les vérifier. C'est `npm run typecheck` qui l'a fait, ce qui est un
   * rappel utile : dans ce dépôt, `npm test` ne tient pas les types, et la porte qui les tient est
   * une autre.
   */
  readonly child: ChildProcessByStdio<null, Readable, Readable>;
  /** Tout ce que le processus a écrit, gardé pour le rapport d'échec. */
  output(): string;
};

/** Ce que le harnais a monté, et de quoi le démonter. */
export type Chain = {
  /** L'adresse HTTP de `locusd`. */
  readonly controlPlane: string;
  /** Le répertoire d'état du worker, propre à cette exécution. */
  readonly workerStateDir: string;
  /** Les trois processus, dans l'ordre de démarrage. */
  readonly processes: readonly string[];
  /** Tout arrêter. Idempotent. */
  stop(): Promise<void>;
};

/**
 * Le dépôt worker, ou une panne qui nomme la variable.
 *
 * Rendre `null` aurait laissé l'appelant décider de se sauter, et c'est précisément la décision que
 * ce harnais ne doit offrir à personne.
 */
export function workerRepo(env: NodeJS.ProcessEnv = process.env): string {
  const declared = env[WORKER_REPO_ENV];
  if (declared === undefined || declared.trim() === "") {
    throw new HarnessFailure(
      WORKER_REPO_ENV,
      "non renseignée. Le harnais démarre le worker `canterel` réel ; sans son dépôt il n'a rien " +
        "à démarrer, et se déclarer non applicable rendrait vert un dossier que personne n'a exercé",
    );
  }
  if (!existsSync(join(declared, "backend", "cli", "src", "index.ts"))) {
    throw new HarnessFailure(
      WORKER_REPO_ENV,
      `« ${declared} » ne contient pas \`backend/cli/src/index.ts\` : ce n'est pas un dépôt canterel`,
    );
  }
  return declared;
}

/**
 * Un binaire construit, ou une panne qui dit comment le construire.
 *
 * Le construire ici serait plus serviable et masquerait la vraie question : un harnais qui compile
 * ce qui manque ne dit jamais si la CI l'avait construit. `cargo build` est explicite dans le job.
 */
export function builtBinary(root: string, name: string): string {
  const path = join(root, "target", "debug", name);
  if (!existsSync(path)) {
    throw new HarnessFailure(
      name,
      `binaire absent de \`target/debug\`. Construire d'abord : \`cargo build --bin ${name}\``,
    );
  }
  return path;
}

/** Démarrer un processus, en gardant ce qu'il écrit. */
function start(
  name: string,
  command: string,
  args: readonly string[],
  env: NodeJS.ProcessEnv,
): Started {
  const child = spawn(command, [...args], { env, stdio: ["ignore", "pipe", "pipe"] });
  let seen = "";
  child.stdout.on("data", (chunk: Buffer) => {
    seen += chunk.toString();
  });
  child.stderr.on("data", (chunk: Buffer) => {
    seen += chunk.toString();
  });
  return { name, child, output: () => seen };
}

/**
 * Attendre qu'une adresse réponde, ou échouer en disant ce que le processus a écrit.
 *
 * Le processus est surveillé pendant l'attente : s'il meurt, l'échec arrive tout de suite et porte
 * sa sortie, au lieu d'attendre le délai entier pour annoncer un timeout qui n'apprend rien.
 */
async function waitReachable(
  started: Started,
  url: string,
  timeoutMs = BOOT_TIMEOUT_MS,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let mort: number | null = null;
  started.child.on("exit", (code) => {
    mort = code ?? -1;
  });

  while (Date.now() < deadline) {
    if (mort !== null) {
      throw new HarnessFailure(started.name, `mort au démarrage (code ${mort})`, started.output());
    }
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(1_000) });
      // N'importe quelle réponse HTTP prouve que le processus écoute. Exiger un `200` ferait
      // dépendre le démarrage d'une route particulière, qui peut légitimement changer.
      if (response.status > 0) return;
    } catch {
      // Pas encore joignable : c'est le cas normal des premières centaines de millisecondes.
    }
    await sleep(100);
  }
  throw new HarnessFailure(
    started.name,
    `injoignable sur ${url} après ${timeoutMs} ms`,
    started.output(),
  );
}

/**
 * Monter la chaîne : `locusd`, `locus-execd`, puis le worker réel.
 *
 * # L'ordre porte
 *
 * `locusd` d'abord — il est l'autorité transactionnelle et les deux autres s'y adressent. Le worker
 * en dernier : démarré avant, il réclamerait dans le vide, et son premier tour se lirait comme un
 * `idle` alors que le plan de contrôle n'existait simplement pas encore.
 *
 * @throws {HarnessFailure} dès qu'un prérequis manque ou qu'un processus ne démarre pas. Tout ce qui
 * avait démarré est arrêté avant que l'erreur remonte : un harnais qui laisserait un `locusd`
 * derrière lui ferait échouer l'exécution suivante sur un port occupé, et le second message
 * cacherait le premier.
 */
export async function startChain(options: {
  readonly root: string;
  readonly port?: number;
}): Promise<Chain> {
  const repo = workerRepo();
  const locusd = builtBinary(options.root, "locusd");
  const execd = builtBinary(options.root, "locus-execd");

  const port = options.port ?? 8787;
  const controlPlane = `http://127.0.0.1:${port}`;
  const workerStateDir = await mkdtemp(join(tmpdir(), "locus-e2e-"));
  // La socket du broker vit dans le répertoire de l'exécution : deux harnais concurrents ne se
  // parlent pas, et rien ne survit à `stop()`.
  const brokerSocket = join(workerStateDir, "broker.sock");
  const demarres: Started[] = [];

  const stop = async () => {
    for (const started of demarres.reverse()) {
      started.child.kill("SIGTERM");
    }
    await rm(workerStateDir, { recursive: true, force: true });
  };

  try {
    // Le broker **d'abord**, contrairement à ce que l'intuition suggère. `locusd` sonde son lien au
    // démarrage (`W4.h`, ADR 0028 décision 4) et démarre quand même si le broker manque — mais il
    // annoncerait alors un lien absent, et la chaîne partirait sur un constat faux. Le lancer avant
    // coûte une seconde et rend le constat exact.
    //
    // Le chemin de socket passe par `--listen`, un **argument** et non une variable : sans lui
    // `locus-execd` imprime son constat d'hôte et sort. Deux versions de ce harnais s'y sont
    // trompées avant lecture — une variable d'environnement, puis un argument nu —, et les deux
    // fois le harnais a **dit** ce qui n'allait pas en rendant la sortie du binaire. C'est
    // exactement ce pour quoi il la garde ; un harnais qui n'aurait rapporté que « locus-execd n'a
    // pas démarré » aurait demandé deux sessions de débogage à la place.
    const broker = start("locus-execd", execd, ["--listen", brokerSocket], { ...process.env });
    demarres.push(broker);
    // Il ne sert aucun port HTTP — `W4.h` en a fait un tube. Ce qu'on vérifie est donc qu'il **reste
    // vivant** : un broker qui meurt à la seconde ne se distingue pas d'un broker qui tourne si on
    // ne regarde jamais.
    await sleep(1_000);
    if (broker.child.exitCode !== null) {
      throw new HarnessFailure(
        "locus-execd",
        `mort au démarrage (code ${broker.child.exitCode})`,
        broker.output(),
      );
    }

    const daemon = start("locusd", locusd, [], {
      ...process.env,
      LOCUSD_BIND: `127.0.0.1:${port}`,
      LOCUSD_BROKER_SOCKET: brokerSocket,
      // `W20.y` : l'amorçage d'administration. Sans lui, `NoAdministrators` refuse §22.3 à toute
      // créance, et rien ne peut proposer de mission à ce daemon.
      [ADMINISTRATION_ENV.credential]: ADMINISTRATION.credential,
      [ADMINISTRATION_ENV.workspace]: ADMINISTRATION.workspaceId,
      [ADMINISTRATION_ENV.principal]: ADMINISTRATION.principalId,
    });
    demarres.push(daemon);
    // `/projections/status` plutôt qu'un `/health` : il n'y en a pas, et en inventer un pour le
    // harnais ajouterait au produit une route dont seul le test aurait besoin. Celle-ci répond
    // toujours et dit quelque chose de vrai sur l'état du daemon.
    await waitReachable(daemon, `${controlPlane}/projections/status`);

    const worker = start(
      "canterel worker",
      "bun",
      ["run", join(repo, "backend", "cli", "src", "index.ts"), "worker", "--locus", controlPlane],
      // `OPENSCIENCE_TEST_HOME`, et **pas** `XDG_DATA_HOME`.
      //
      // La première rédaction posait `XDG_DATA_HOME` en croyant isoler le worker. `canterel` ne lit
      // pas cette variable : `Global.Path.data` dérive de `OPENSCIENCE_TEST_HOME` ou du home réel.
      // Le harnais partageait donc l'installation **de la machine**, et son verdict dépendait de
      // l'état de cet hôte — vert tant qu'aucun worker n'y était enrôlé, rouge dès qu'il l'était.
      //
      // C'est la pire espèce de harnais : il croyait isoler, il ne le disait à personne, et il a
      // fallu qu'un enrôlement réel réussisse pour que le mensonge devienne visible. Vérifié en
      // lisant `src/global/index.ts`, pas en supposant qu'une variable XDG standard serait lue.
      { ...process.env, [WORKER_HOME_ENV]: workerStateDir },
    );
    demarres.push(worker);
    await sleep(2_000);
    if (worker.child.exitCode !== null && worker.child.exitCode !== 0) {
      throw new HarnessFailure(
        "canterel worker",
        `mort au démarrage (code ${worker.child.exitCode})`,
        worker.output(),
      );
    }

    return {
      controlPlane,
      workerStateDir,
      processes: demarres.map((started) => started.name),
      stop,
    };
  } catch (erreur) {
    await stop();
    throw erreur;
  }
}
