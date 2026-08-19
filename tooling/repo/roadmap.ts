import { readFile } from "node:fs/promises";
import { join } from "node:path";

import type { Finding } from "../lib/findings.ts";

/**
 * Le registre de ce qui a été livré, et le plan de ce qui reste. Ils doivent dire la même chose.
 *
 * # Le défaut que cette garde rend impossible
 *
 * `IMPLEMENTATION_LEDGER.md` consigne chaque item livré, avec son test de sortie et ce qu'il a
 * coûté à trouver. `docs/10_V1_ROADMAP.md` porte le plan, et marque **fait** les lignes achevées.
 * Rien ne les reliait, et ils ont divergé jusqu'à cent items d'écart : le ledger en enregistrait
 * plus de cent, la roadmap en marquait vingt-neuf.
 *
 * Ce n'est pas un défaut cosmétique. « Règle de session » dit de prendre **le premier item non
 * terminé dont les dépendances sont satisfaites** — une instruction qui n'a plus de sens si le
 * tableau ne dit pas ce qui est terminé. Une session qui la suit refait du travail déjà livré, ou
 * saute par-dessus un item ouvert en le croyant clos.
 *
 * # Pourquoi une garde et pas une consigne
 *
 * Marquer la ligne après chaque merge est une **discipline**, c'est-à-dire quelque chose qui tient
 * jusqu'à ce qu'il ne tienne plus — et il n'a pas tenu cent fois. La garde, elle, se vérifie à
 * chaque passage.
 *
 * # Les deux sens, parce qu'un seul laisserait passer l'autre moitié
 *
 * - **Livré mais non marqué** : la roadmap sous-estime l'avancement, et une session refait le
 *   travail.
 * - **Marqué mais non livré** : la roadmap sur-estime l'avancement, et une session saute un item
 *   qui n'existe pas. C'est le plus coûteux des deux, parce qu'il ne se découvre qu'en aval.
 *
 * Ni l'un ni l'autre ne se répare en regardant un seul des deux fichiers.
 *
 * # Les quatre registres, parce qu'une garde d'un dépôt sur quatre ment de la même façon
 *
 * La roadmap est celle du chantier, pas celle de ce dépôt : `W2.*` est livré dans `canterel`,
 * `W10.*` dans `xiiif`, `W0.1` et `W0.10` dans les quatre. Une première version ne lisait que le
 * registre de `locusolus` et concluait « ok » — vingt-quatre lignes faites et non marquées lui
 * étaient structurellement invisibles. Les identifiants sont uniques sur l'ensemble du chantier :
 * la garde cherche donc chaque item dans **les quatre** registres, les dépôts voisins étant
 * attendus à côté de celui-ci.
 *
 * # Ce que cette garde ne verra jamais
 *
 * Elle confronte deux documents : elle attrape leur **désaccord**, pas leur **silence commun**. Un
 * item livré dans le code, sans entrée au registre et sans marque au tableau, lui est indiscernable
 * d'un item jamais commencé — et c'est le cas normal d'un item à faire, donc rien ne peut être
 * conclu dessus. Le premier passage en a trouvé un exemplaire vivant : `W7.a` n'est ni marqué ni
 * consigné, et `packages/review/src/dossier.rs` existe, écrit par le sprint de `W7.b` qui en avait
 * besoin. Seul le test de sortie de l'item tranche, et il ne se déduit d'aucun des deux documents.
 * La garde réduit donc la fenêtre de mensonge, elle ne la ferme pas.
 */

/** Les dépôts voisins où un item de la roadmap peut avoir été livré. */
const SIBLINGS = ["canterel", "xiiif", "emacs-config"] as const;

/** Le motif d'un titre d'entrée de ledger : `## 2026-08-19 — W5.q — …`. */
const ledgerHeading = /^## \d{4}-\d{2}-\d{2} — (W\d+\.[a-z0-9]+)\b([^\n]*)$/gm;

/**
 * Le motif d'une entrée qui consigne une **décision**, pas une livraison.
 *
 * `## 2026-08-18 — W15.f — Bloqué : le rôle n'a pas de chemin jusqu'à son lecteur` — « Aucun code
 * écrit. L'item est consigné bloqué plutôt que livré incomplet. » C'est la bonne pratique : le
 * sprint a instruit la question, trouvé pourquoi elle ne se répond pas encore, et l'a écrit. Mais
 * une garde qui compte les entrées comptait celle-ci comme une livraison, et la ligne de `W15.f`
 * portait **fait** sans que rien ne proteste.
 *
 * L'ancrage est un **préfixe de titre**, pas la présence du mot. L'entrée de `W0.12` s'intitule
 * « `« Bloqué »` n'est pas `« à faire »` » et livre bel et bien : chercher le mot dans le titre
 * l'aurait effacée. Après le tiret cadratin, le premier mot décide.
 *
 * La fin du motif est une **anti-lettre Unicode**, pas `\b`. Écrite `Bloqué\b`, la règle était
 * inerte : en regex JavaScript `\w` reste `[A-Za-z0-9_]`, donc `é` n'est pas un caractère de mot,
 * donc il n'y a pas de frontière entre `é` et l'espace qui suit, donc rien ne matchait jamais. La
 * garde répondait `ok` sur une règle qui ne s'exécutait pas — elle n'a été prise que parce qu'un
 * écart connu **n'a pas** été rapporté. Une règle neuve se regarde échouer avant d'être crue.
 */
const decisionHeading = /—\s*(Bloqué|Reporté)(?!\p{L})/u;

/** Le motif d'une ligne d'item de roadmap : `| W5.q `[R]` **fait** | … |`. */
const roadmapRow = /^\| (W\d+\.[a-z0-9]+) ([^|]*)\|/gm;

/**
 * Ce que les deux documents affirment, et ce qui n'a pas pu être lu.
 *
 * `unread` n'est pas un détail de journalisation : c'est ce qui **suspend** une des deux règles.
 * Un item marqué dont l'entrée vit dans un registre non lu ressemble trait pour trait à un item
 * marqué qui n'existe pas, et la CI de ce dépôt ne voit qu'un checkout sur quatre. Rendre l'un
 * pour l'autre ferait rougir la CI sur du travail réellement livré — « pas vérifié n'est jamais
 * réussi » vaut aussi dans ce sens : une absence de lecture ne conclut pas non plus à la faute.
 *
 * Le registre de `locusolus` lui-même, en revanche, n'est jamais « non lu » : son absence est un
 * checkout cassé, pas un dépôt voisin manquant, et `readReconciliation` échoue dessus.
 */
export type Reconciliation = {
  readonly delivered: ReadonlySet<string>;
  readonly blocked: ReadonlySet<string>;
  readonly marked: ReadonlySet<string>;
  readonly decided: ReadonlySet<string>;
  readonly planned: ReadonlySet<string>;
  readonly frontier: readonly string[];
  readonly unread: readonly string[];
};

/**
 * Les trois états qu'une ligne du plan peut déclarer, et pourquoi ils sont trois.
 *
 * Une première lecture n'en connaissait que deux — marqué **fait**, ou non — et elle a failli
 * envoyer un sprint sur `W16.d`, dont la ligne dit **bloqué** depuis qu'ADR 0017 a constaté qu'il
 * lui manque un consommateur. « Bloqué » et « à faire » se ressemblent dans un tableau et ne se
 * ressemblent pas du tout dans une session : l'un attend une décision extérieure, l'autre attend
 * qu'on l'écrive. `reporté` est du même genre — `W18.f` attend un hôte, pas du travail.
 *
 * D'où la **frontière** : les lignes qui ne déclarent rien. C'est la seule liste sur laquelle
 * « le premier item non terminé dont les dépendances sont satisfaites » a un sens, et la calculer
 * évite de la lire à l'œil — ce qui, à l'œil, a déjà produit une erreur.
 */
const DECIDED = ["bloqué", "reporté"] as const;

/**
 * Les items qu'un registre atteste avoir livrés.
 *
 * Une entrée de ledger peut **nommer** un item futur — « c'est `W5.r` », « le sujet de `W5.j` » —
 * et c'est même souhaitable : c'est ainsi qu'un sprint transmet ce qu'il a trouvé. Seul le
 * **titre** d'une entrée atteste une livraison, et c'est pourquoi le motif ancre sur
 * `## <date> — <item> —`.
 */
function deliveredIn(ledger: string): { delivered: string[]; blocked: string[] } {
  const delivered: string[] = [];
  const blocked: string[] = [];
  for (const [, item, rest] of ledger.matchAll(ledgerHeading)) {
    if (item === undefined) {
      continue;
    }
    (decisionHeading.test(rest ?? "") ? blocked : delivered).push(item);
  }
  return { delivered, blocked };
}

/** Lire les documents et dire ce que chacun affirme, et ce qui n'a pas pu être lu. */
export async function readReconciliation(root: string): Promise<Reconciliation> {
  const roadmap = await readFile(join(root, "docs/10_V1_ROADMAP.md"), "utf8");
  const own = await readFile(join(root, "IMPLEMENTATION_LEDGER.md"), "utf8");

  const ours = deliveredIn(own);
  const delivered = new Set(ours.delivered);
  const blocked = new Set(ours.blocked);
  const unread: string[] = [];
  for (const sibling of SIBLINGS) {
    const path = join(root, "..", sibling, "IMPLEMENTATION_LEDGER.md");
    const ledger = await readFile(path, "utf8").catch(() => undefined);
    if (ledger === undefined) {
      unread.push(sibling);
      continue;
    }
    const theirs = deliveredIn(ledger);
    for (const item of theirs.delivered) {
      delivered.add(item);
    }
    for (const item of theirs.blocked) {
      blocked.add(item);
    }
  }

  const marked = new Set<string>();
  const decided = new Set<string>();
  const planned = new Set<string>();
  const frontier: string[] = [];
  for (const [, item, tail] of roadmap.matchAll(roadmapRow)) {
    if (item === undefined || tail === undefined) {
      continue;
    }
    planned.add(item);
    if (tail.includes("fait")) {
      marked.add(item);
    } else if (DECIDED.some((state) => tail.includes(state))) {
      decided.add(item);
    } else {
      frontier.push(item);
    }
  }

  return { delivered, blocked, marked, decided, planned, frontier, unread };
}

/**
 * Confronter les documents, et nommer chaque écart.
 *
 * Un item livré que la roadmap **ne connaît pas** n'est pas une violation : `W5.g` à `W5.u` sont nés
 * en cours de route, et certains ont été écrits dans la roadmap après coup. Seul un item que la
 * roadmap connaît et contredit est un écart.
 *
 * Un registre non lu ne produit pas de faute mais en suspend une : voir `Reconciliation.unread`.
 */
export function reconcile(state: Reconciliation): readonly Finding[] {
  const findings: Finding[] = [];

  // Un item décidé et livré est signalé plus bas, sous son nom propre. Le signaler ici aussi
  // rendrait deux constats pour un fait, et le moins juste des deux en premier : « ajoute **fait** »
  // alors que ce qu'il faut lire est « cette ligne dit de ne pas y aller, et c'est faux ».
  for (const item of [...state.delivered].sort()) {
    if (state.planned.has(item) && !state.marked.has(item) && !state.decided.has(item)) {
      findings.push({
        rule: "livre-non-marque",
        where: "docs/10_V1_ROADMAP.md",
        message: `« ${item} » a son entrée au ledger : sa ligne doit porter **fait**, sinon une session le refera`,
      });
    }
  }

  for (const item of [...state.decided].sort()) {
    if (state.delivered.has(item)) {
      findings.push({
        rule: "decide-et-livre",
        where: "docs/10_V1_ROADMAP.md",
        message: `« ${item} » est déclaré bloqué ou reporté et a pourtant son entrée au ledger : un item livré n'attend plus rien`,
      });
    }
  }

  if (state.unread.length > 0) {
    return findings;
  }

  for (const item of [...state.marked].sort()) {
    if (state.blocked.has(item) && !state.delivered.has(item)) {
      findings.push({
        rule: "marque-mais-bloque",
        where: "docs/10_V1_ROADMAP.md",
        message: `« ${item} » est marqué fait et sa seule entrée au ledger consigne un blocage : la ligne doit dire ce que l'entrée dit`,
      });
      continue;
    }
    if (!state.delivered.has(item)) {
      findings.push({
        rule: "marque-non-livre",
        where: "IMPLEMENTATION_LEDGER.md",
        message: `« ${item} » est marqué fait sans entrée au ledger : soit il n'est pas livré, soit ce qu'il a coûté n'est consigné nulle part`,
      });
    }
  }

  return findings;
}

/** La garde, telle que `npm run check:roadmap` la lance. */
export async function inspectRoadmap(root: string): Promise<readonly Finding[]> {
  return reconcile(await readReconciliation(root));
}
