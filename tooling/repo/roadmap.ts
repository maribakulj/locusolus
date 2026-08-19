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
const ledgerHeading = /^## \d{4}-\d{2}-\d{2} — (W\d+\.[a-z0-9]+)\b/gm;

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
  readonly marked: ReadonlySet<string>;
  readonly planned: ReadonlySet<string>;
  readonly unread: readonly string[];
};

/**
 * Les items qu'un registre atteste avoir livrés.
 *
 * Une entrée de ledger peut **nommer** un item futur — « c'est `W5.r` », « le sujet de `W5.j` » —
 * et c'est même souhaitable : c'est ainsi qu'un sprint transmet ce qu'il a trouvé. Seul le
 * **titre** d'une entrée atteste une livraison, et c'est pourquoi le motif ancre sur
 * `## <date> — <item> —`.
 */
function deliveredIn(ledger: string): string[] {
  const items: string[] = [];
  for (const [, item] of ledger.matchAll(ledgerHeading)) {
    if (item !== undefined) {
      items.push(item);
    }
  }
  return items;
}

/** Lire les documents et dire ce que chacun affirme, et ce qui n'a pas pu être lu. */
export async function readReconciliation(root: string): Promise<Reconciliation> {
  const roadmap = await readFile(join(root, "docs/10_V1_ROADMAP.md"), "utf8");
  const own = await readFile(join(root, "IMPLEMENTATION_LEDGER.md"), "utf8");

  const delivered = new Set(deliveredIn(own));
  const unread: string[] = [];
  for (const sibling of SIBLINGS) {
    const path = join(root, "..", sibling, "IMPLEMENTATION_LEDGER.md");
    const ledger = await readFile(path, "utf8").catch(() => undefined);
    if (ledger === undefined) {
      unread.push(sibling);
      continue;
    }
    for (const item of deliveredIn(ledger)) {
      delivered.add(item);
    }
  }

  const marked = new Set<string>();
  const planned = new Set<string>();
  for (const [, item, tail] of roadmap.matchAll(roadmapRow)) {
    if (item === undefined || tail === undefined) {
      continue;
    }
    planned.add(item);
    if (tail.includes("fait")) {
      marked.add(item);
    }
  }

  return { delivered, marked, planned, unread };
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

  for (const item of [...state.delivered].sort()) {
    if (state.planned.has(item) && !state.marked.has(item)) {
      findings.push({
        rule: "livre-non-marque",
        where: "docs/10_V1_ROADMAP.md",
        message: `« ${item} » a son entrée au ledger : sa ligne doit porter **fait**, sinon une session le refera`,
      });
    }
  }

  if (state.unread.length > 0) {
    return findings;
  }

  for (const item of [...state.marked].sort()) {
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
