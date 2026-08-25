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

/**
 * Le token d'amorçage d'enrôlement, et les variables qui le portent — `W20.ab`, `W20.z`.
 *
 * Le worker de la chaîne s'enrôle **pour de vrai** : `canterel worker enroll` consomme ce token et
 * reçoit une créance. Sans cela, il démarrerait sans identité et son tour se lirait comme un constat
 * de configuration, ce qui n'exerce rien de §15.2.
 *
 * Le workspace, le principal et le projet sont ceux de [`ADMINISTRATION`] : c'est le **même**
 * grant, donc les faits d'un worker et ceux d'un exploitant atterrissent au même endroit, ce qui est
 * ce qu'une chaîne d'une seule institution doit produire.
 */
export const ENROLLMENT = {
  token: "jeton-e2e",
  env: {
    token: "LOCUSD_ENROLLMENT_TOKEN",
    workspace: "LOCUSD_ENROLLMENT_WORKSPACE",
    principal: "LOCUSD_ENROLLMENT_PRINCIPAL",
    project: "LOCUSD_ENROLLMENT_PROJECT",
  },
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
  /**
   * Ce qu'un processus de la chaîne a écrit jusqu'ici — `W5.x`.
   *
   * # Pourquoi ça sort du harnais
   *
   * Jusque-là, tout ce que les trois processus annonçaient n'existait que dans
   * [`HarnessFailure`] : gardé pour le rapport d'échec, **jeté sur un tour vert**. Le job `e2e`
   * démarre pourtant le seul `locus-execd` de toute la CI qui monte par le harnais, et son log ne
   * portait rien de ce que ce broker dit de son hôte — pas même l'empreinte que `W5.w` a fait
   * imprimer exprès pour qu'un exploitant puisse la lire.
   *
   * Un démarrage qui ne se voit que lorsqu'il rate est un démarrage qu'on ne mesure jamais.
   *
   * @throws {HarnessFailure} sur un nom qui n'est pas celui d'un processus de cette chaîne. Rendre
   * `""` ferait lire une faute de frappe comme « ce processus n'a rien dit », et un appelant
   * affirmerait alors sur un silence qu'il a fabriqué lui-même.
   */
  annonce(processus: string): string;
  /**
   * Faire faire **un** tour au worker, et rendre ce qu'il a écrit — `W12.d`.
   *
   * `runLoop` de `canterel` fait exactement un tour puis sort ; ce n'est pas une boucle malgré son
   * nom. Un worker démarré avant qu'aucune mission soit en file tombe donc toujours sur « aucune
   * mission à réclamer », et c'est ce que le harnais faisait sans le dire. Déclencher le tour
   * **après** la mise en file est la seule façon d'exercer un placement.
   *
   * @throws {HarnessFailure} quand le tour sort sur autre chose que `0`, avec ce qu'il a écrit.
   */
  tourDeWorker(): Promise<string>;
  /** Tout arrêter. Idempotent. */
  stop(): Promise<void>;
};

/**
 * Attendre qu'un processus sorte, et rendre son code — ou `null` s'il n'a pas fini à temps.
 *
 * `null` plutôt qu'un `throw` : l'appelant sait, lui, si un tour qui n'a pas fini est une panne ou
 * l'attendu, et lever ici lui retirerait la sortie du processus, c'est-à-dire le seul renseignement
 * utile.
 */
function attendreSortie(started: Started, timeoutMs = BOOT_TIMEOUT_MS): Promise<number | null> {
  return new Promise((resolve) => {
    if (started.child.exitCode !== null) {
      resolve(started.child.exitCode);
      return;
    }
    const minuterie = setTimeout(() => resolve(null), timeoutMs);
    started.child.on("exit", (code) => {
      clearTimeout(minuterie);
      resolve(code ?? -1);
    });
  });
}

/**
 * Ce qu'un processus nommé a écrit, ou une panne qui nomme ceux qui existent — `W5.x`.
 *
 * Une fonction de module plutôt qu'une fermeture dans `startChain` : son refus est la moitié qui
 * compte, et une fermeture ne s'exerce qu'en montant les trois processus. `npm test` la tient donc
 * seule, comme il tient déjà les refus de [`workerRepo`] et de [`builtBinary`].
 *
 * @throws {HarnessFailure} sur un nom qui n'est pas celui d'un processus de la chaîne.
 */
export function annonceDe(
  demarres: readonly { readonly name: string; output(): string }[],
  processus: string,
): string {
  const started = demarres.find((candidat) => candidat.name === processus);
  if (started === undefined) {
    throw new HarnessFailure(
      processus,
      "n'est pas un processus de cette chaîne, qui en porte " +
        `${demarres.length} : ${demarres.map((candidat) => candidat.name).join(", ")}. ` +
        "Rendre une sortie vide ferait lire cette faute de frappe comme « ce processus n'a rien " +
        "dit », et l'appelant affirmerait sur un silence qu'il a fabriqué lui-même",
    );
  }
  return started.output();
}

/**
 * Le préfixe sous lequel `locus-execd` imprime l'empreinte de son hôte au démarrage.
 *
 * Transcrit de `apps/locus-execd/src/main.rs`, et **exporté** pour la même raison que
 * [`WORKER_HOME_ENV`] : c'est un couplage entre deux langages, et un couplage qu'on ne voit pas est
 * un couplage que personne ne vérifie.
 */
export const EMPREINTE_PREFIXE = "empreinte de cet hôte :";

/**
 * L'empreinte d'hôte que `locus-execd` a annoncée — `W5.x`.
 *
 * # Ce que cette lecture tient
 *
 * `W5.z` a livré la lecture des attestations, `W5.aa` leur dépôt, `W5.w` l'empreinte imprimée au
 * démarrage pour que l'exploitant qui prépare un fichier sache à quel hôte le lier. Les trois
 * reposent sur la **même** phrase, et rien ne vérifiait qu'un binaire réel la produise : les tests
 * de `attestation.rs` exercent la fonction, pas le démarrage.
 *
 * Refuser ici quand elle manque est donc la garde qui manquait : le jour où ce `println!` disparaît
 * dans un remaniement, le job `e2e` rougit au lieu de laisser le remède de `W5.w` devenir muet.
 *
 * @throws {HarnessFailure} quand la ligne est absente, ou présente et vide. Les deux sont
 * distingués : une ligne absente est un binaire qui ne dit plus rien, une ligne vide est une
 * empreinte qui ne décide plus de rien — et elles ne se réparent pas au même endroit.
 */
export function empreinte(annonce: string): string {
  const ligne = annonce
    .split("\n")
    .map((brute) => brute.trim())
    .find((brute) => brute.startsWith(EMPREINTE_PREFIXE));

  if (ligne === undefined) {
    throw new HarnessFailure(
      EMPREINTE_PREFIXE,
      "absente de ce que `locus-execd` a écrit au démarrage. `W5.w` l'imprime pour que qui " +
        "prépare un fichier d'attestations sache à quel hôte le lier ; sans elle, un refus " +
        "d'attestation nomme le problème et cache le remède",
      annonce,
    );
  }

  const valeur = ligne.slice(EMPREINTE_PREFIXE.length).trim();
  if (valeur === "") {
    throw new HarnessFailure(
      EMPREINTE_PREFIXE,
      "annoncée vide. Une empreinte qui ne porte aucun fait ne distingue plus deux hôtes, et " +
        "toute attestation la vérifiant serait honorée partout",
      annonce,
    );
  }
  return valeur;
}

/**
 * La variable par laquelle une campagne dépose, et par laquelle `locus-execd` relit — `W5.ab`.
 *
 * Deux variables distinctes côté produit (`W5.aa` : une campagne ne doit pas écraser le fichier
 * qu'un daemon est en train de lire). Le harnais n'a besoin que de la **lecture** : c'est le job de
 * CI qui pose le fichier, en le descendant du job `sandbox`.
 *
 * Nommée ici pour la même raison que [`WORKER_HOME_ENV`] — un couplage entre deux langages qu'on ne
 * voit pas est un couplage que personne ne vérifie.
 */
export const ATTESTATIONS_ENV = "LOCUS_EXECD_ATTESTATIONS";

/**
 * La variable par laquelle le workflow dit au harnais **qu'il a posé un fichier** — `W5.ab`.
 *
 * # Pourquoi elle existe, plutôt que de lire l'autre
 *
 * `LOCUS_EXECD_ATTESTATIONS` non renseignée peut vouloir dire deux choses très différentes : le job
 * `sandbox` n'a rien déposé ce tour-ci (état extérieur, légitime), ou le câblage a cessé de la
 * poser (régression, à voir rougir). Les lire l'un pour l'autre est la faute que ce dépôt nomme
 * partout ; le workflow **déclare** donc ce qu'il a trouvé, et le test compare la déclaration au
 * constat au lieu de deviner.
 */
export const ATTESTATIONS_ATTENDUES_ENV = "LOCUS_E2E_ATTESTATIONS";

/** Ce que `locus-execd` a fait du fichier d'attestations, tel qu'il l'annonce. */
export type Attestations =
  /** Aucun fichier nommé : rien ne sera placé au-dessus de `S0`. */
  | { readonly kind: "aucune" }
  /** Un fichier lu, et ce qu'il en reste pour **cet** hôte. */
  | {
      readonly kind: "lues";
      readonly honorees: number;
      readonly etrangeres: number;
      /** L'empreinte de cet hôte, que l'annonce ne porte **que** si des attestations sont écartées. */
      readonly hote?: string;
    };

/** Les trois phrases de `attestation::annonce` et de `main`, transcrites. */
const AUCUNE = "attestations : aucune";
const LUES = /^attestations : (\d+) retenue\(s\) pour cet hôte(?:, (\d+) écartée\(s\))?/;
const HOTE = /dont l'empreinte est « ([^»]+) »/;

/**
 * Ce que le broker a fait du fichier d'attestations — `W5.ab`.
 *
 * # Ce que cette lecture tient
 *
 * `W5.z` a livré la lecture des attestations, `W5.aa` leur dépôt, et **aucun assemblage ne joignait
 * les deux** : le fichier écrit par le job `sandbox` n'était lu par aucun `locus-execd`. La mesure
 * de `W5.x` a levé ce qui l'empêchait — les deux runners rendent la même empreinte, caractère pour
 * caractère —, et le convoyeur devient donc justifié plutôt que supposé.
 *
 * La garde est ici : le broker **dit toujours** ce qu'il a fait du fichier, qu'il en ait eu un ou
 * non. Un silence n'est pas « aucune attestation » — c'est un binaire qui a cessé de rendre compte,
 * et le lire comme un `S0` nominal ferait chercher une campagne pendant que le câblage est mort.
 *
 * @throws {HarnessFailure} quand aucune des deux phrases n'apparaît.
 */
export function attestations(annonce: string): Attestations {
  const lignes = annonce.split("\n").map((brute) => brute.trim());

  if (lignes.some((ligne) => ligne.startsWith(AUCUNE))) {
    return { kind: "aucune" };
  }

  for (const ligne of lignes) {
    const lu = LUES.exec(ligne);
    if (lu === null) {
      continue;
    }
    const hote = HOTE.exec(ligne)?.[1];
    return {
      kind: "lues",
      honorees: Number(lu[1]),
      etrangeres: Number(lu[2] ?? 0),
      ...(hote === undefined ? {} : { hote }),
    };
  }

  throw new HarnessFailure(
    "attestations",
    "`locus-execd` n'a rien dit du fichier d'attestations. Ni « aucune », ni un compte : un " +
      "silence n'est pas un `S0` nominal, c'est un binaire qui a cessé de rendre compte, et le " +
      "lire comme l'autre ferait chercher une campagne pendant que le câblage est mort",
    annonce,
  );
}

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
 * Enrôler le worker de la chaîne, et **échouer bruyamment** s'il ne s'enrôle pas — `W20.z`.
 *
 * # Pourquoi c'est une commande et non un appel HTTP
 *
 * Poster nous-mêmes sur `/lep/v1/enroll` serait plus court et prouverait moins : la moitié cliente
 * de §7.2 — la paire de clés, la signature, l'écriture de la créance dans le répertoire d'état —
 * est du code `canterel`, et c'est elle qu'on veut exercer. Un harnais qui l'imiterait dirait que
 * `locusd` répond à ce que le harnais sait écrire.
 *
 * @throws {HarnessFailure} quand l'enrôlement échoue, avec ce que la commande a écrit — sans quoi
 * le worker démarrerait sans identité et le tour suivant se lirait comme un banal `inert`, à deux
 * étages de sa cause.
 */
async function enroll(repo: string, stateDir: string, controlPlane: string): Promise<void> {
  const commande = start(
    "canterel worker enroll",
    "bun",
    [
      "run",
      join(repo, "backend", "cli", "src", "index.ts"),
      "worker",
      "enroll",
      "--locus",
      controlPlane,
      "--enrollment-token",
      ENROLLMENT.token,
    ],
    { ...process.env, [WORKER_HOME_ENV]: stateDir },
  );

  const code = await new Promise<number>((resolve) => {
    commande.child.on("exit", (statut) => resolve(statut ?? -1));
  });
  if (code !== 0) {
    throw new HarnessFailure("canterel worker enroll", `sorti en ${code}`, commande.output());
  }
  // Le code de sortie ne suffit pas : `worker enroll` attrape les erreurs Locus, les affiche et pose
  // `process.exitCode = 1` — mais un chemin qui rendrait `0` sans rien écrire laisserait la chaîne
  // continuer avec un worker anonyme. Ce que la commande **dit** est donc vérifié aussi.
  if (!commande.output().includes("enrôlé")) {
    throw new HarnessFailure(
      "canterel worker enroll",
      "sorti en 0 sans annoncer d'enrôlement",
      commande.output(),
    );
  }
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
  /**
   * Quand le worker fait son tour — `W12.d`.
   *
   * `« au démarrage »` (le défaut) monte les trois processus, ce que `W12.f` décrit. `« à la
   * demande »` en monte deux et laisse l'appelant déclencher le tour par [`Chain.tourDeWorker`],
   * ce qui est la seule façon d'exercer un **placement** : `runLoop` fait un tour et sort, donc un
   * worker démarré avant qu'aucune mission soit en file tombe toujours sur « aucune mission à
   * réclamer ».
   */
  readonly worker?: "au démarrage" | "à la demande";
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
      // `W20.ab` : l'amorçage d'enrôlement, pour que le worker de la chaîne puisse s'enrôler.
      [ENROLLMENT.env.token]: ENROLLMENT.token,
      [ENROLLMENT.env.workspace]: ADMINISTRATION.workspaceId,
      [ENROLLMENT.env.principal]: ADMINISTRATION.principalId,
      [ENROLLMENT.env.project]: ADMINISTRATION.projectId,
    });
    demarres.push(daemon);
    // `/projections/status` plutôt qu'un `/health` : il n'y en a pas, et en inventer un pour le
    // harnais ajouterait au produit une route dont seul le test aurait besoin. Celle-ci répond
    // toujours et dit quelque chose de vrai sur l'état du daemon.
    await waitReachable(daemon, `${controlPlane}/projections/status`);

    // `W20.z` : le worker s'enrôle **avant** de boucler, et c'est une commande à part — §7.2 veut
    // que le premier enrôlement soit explicite, et `canterel` le tient par une sous-commande.
    // Attendue jusqu'à son terme : la boucle démarrée pendant l'enrôlement lirait un répertoire
    // d'état à moitié écrit, ce qui produirait un `inert` intermittent plutôt qu'une panne.
    await enroll(repo, workerStateDir, controlPlane);

    /**
     * Lancer un tour de worker — `W12.d`.
     *
     * # Pourquoi c'est **un tour**, et non un démarrage
     *
     * `runLoop` de `canterel` fait exactement un tour : réclamer, planifier, ouvrir la session,
     * faire remonter, rendre — puis le processus sort. Ce n'est pas une boucle malgré son nom, et
     * `W2.24` l'a rendu visible en faisant dire au worker ce que son tour a fait.
     *
     * Le harnais le démarrait pourtant **avant** qu'aucune mission soit en file, et le tour tombait
     * donc toujours sur « aucune mission à réclamer ». Une chaîne qui voudrait voir un placement
     * doit mettre la mission en file **d'abord**.
     */
    const tour = () =>
      start(
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

    /** Un tour mené jusqu'à sa fin, et ce qu'il a écrit. */
    const tourDeWorker = async (): Promise<string> => {
      const worker = tour();
      demarres.push(worker);
      const sortie = await attendreSortie(worker);
      // `0` ou rien : un tour qui n'a rien trouvé sort proprement, et c'est le cas nominal d'une
      // file vide. Tout autre code est une panne, et sa sortie est ce qui la diagnostique.
      if (sortie !== 0 && sortie !== null) {
        throw new HarnessFailure("canterel worker", `sorti en ${sortie}`, worker.output());
      }
      return worker.output();
    };

    if (options.worker !== "à la demande") {
      const worker = tour();
      demarres.push(worker);
      await sleep(2_000);
      if (worker.child.exitCode !== null && worker.child.exitCode !== 0) {
        throw new HarnessFailure(
          "canterel worker",
          `mort au démarrage (code ${worker.child.exitCode})`,
          worker.output(),
        );
      }
    }

    return {
      controlPlane,
      workerStateDir,
      processes: demarres.map((started) => started.name),
      annonce: (processus: string) => annonceDe(demarres, processus),
      tourDeWorker,
      stop,
    };
  } catch (erreur) {
    await stop();
    throw erreur;
  }
}
