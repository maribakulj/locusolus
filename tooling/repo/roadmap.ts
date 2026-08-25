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
 * # Les deux familles, et le jour où le garde n'en lisait qu'une
 *
 * Le plan porte deux familles d'identifiants : les `W<phase>.<item>` du chemin critique, et les
 * `R<n>` de recherche. Le garde ne connaissait que la première — dans les **deux** documents à la
 * fois, puisque le même alphabet ancrait le titre d'entrée et la ligne de tableau.
 *
 * Les six items de recherche ont été livrés le 2026-08-18 : `packages/graph/src/consensus.rs`,
 * `credit.rs`, `regret.rs`, `metrics.rs`, `counterfactual.rs`, `evolution.rs`, chacun avec ses tests
 * et sa passe de mutants, chacun avec son entrée au registre sous la forme qui atteste. Le garde a
 * répondu `frontière vide` et `ok` par-dessus, et la section « Recherche » a continué de les décrire
 * au présent comme des travaux à considérer. C'est le premier des deux sens — la roadmap
 * sous-estime, et une session refait —, et il a failli s'exercer : la session qui écrit ceci a lu
 * « le moins cher des items de recherche, publiable seul » et s'apprêtait à écrire une détection de
 * consensus circulaire qui existe depuis trois jours.
 *
 * Ce n'est **pas** l'angle mort documenté plus bas. Celui-là est un silence commun : ni entrée, ni
 * marque, et rien à en conclure. Ici le registre parlait, dans la forme exacte que le garde exige,
 * et le garde ne l'entendait pas. Un angle mort qu'une garde énonce est une limite ; un angle mort
 * dans son alphabet est un défaut, et il se répare.
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
 * Cette unicité était **affirmée ici et vérifiée nulle part**, et quatre paires y contrevenaient —
 * trois posées par les sessions qui ont écrit les items juste au-dessus. La règle
 * `identifiant-partage` la tient désormais (`W0.17`) : elle est ce dont dépendent les quatre
 * registres, puisqu'un identifiant qui désigne deux items ne peut pas être cherché dans un registre.
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

/**
 * Un identifiant d'item, des deux familles : `W5.q` du chemin critique, `R1` de recherche — et la
 * **sous-décomposition** `W4.d.1`, qu'un item trop gros reçoit quand un sprint le découpe.
 *
 * Écrit une fois et partagé par les quatre motifs. Les avoir écrits quatre fois est précisément ce
 * qui a permis à une famille entière d'échapper aux quatre en même temps : une alternance ajoutée à
 * trois motifs sur quatre laisserait le garde lire les lignes sans lire les entrées, ou l'inverse,
 * et il conclurait alors « livré nulle part » sur six items livrés.
 *
 * # La troisième forme, et pourquoi elle manquait
 *
 * `W0.17` avait ajouté la famille `R<n>` après qu'elle eut échappé aux quatre motifs. La réparation
 * a corrigé ce qu'elle avait vu et **n'a pas demandé si d'autres formes existaient**. Il y en avait
 * une : huit lignes portent un second point — `W4.d.1` à `W4.d.4`, `W4.e.1`, `W4.f.1`, `W4.g.1`,
 * `W4.g.2` — et le motif s'arrêtait devant lui. Elles n'entraient dans aucun ensemble, donc aucune
 * règle ne s'y appliquait, donc rien ne protestait qu'elles n'aient pas de marqueur alors que le
 * registre porte pour chacune une entrée de livraison complète.
 *
 * Une garde qui ne voit pas une ligne ne la déclare pas manquante : elle la déclare **inexistante**,
 * et aucun décompte ne baisse. C'est ce qui rend cette faute-là indétectable par relecture, et c'est
 * pourquoi `unrecognised` existe désormais à côté du motif : **le motif ne se corrige plus seul.**
 */
const WITHIN = String.raw`\.[a-z0-9]+(?:\.\d+)?`;

/**
 * Ce qui suit un numéro de workstream dans un identifiant d'item : `.d`, ou `.d.1`.
 *
 * Extrait pour que `ITEM` et `awaits` ne puissent plus **diverger**. Ils divergeaient : le premier
 * lisait `W4.d.1`, le second s'arrêtait à `W4.d` et rendait donc un **autre item** sur
 * `attend:W4.d.1`, sans que rien ne le signale. Une troncature silencieuse est de la même famille
 * que la cécité qu'`ITEM` vient de corriger, et l'écrire deux fois est ce qui l'a produite — comme
 * l'avoir écrit quatre fois avait produit celle de `W0.17`.
 */
const ITEM = String.raw`(?:W\d+${WITHIN}|R\d+)`;

/**
 * Ce qui fait d'une ligne de tableau une **ligne d'item**, indépendamment de son identifiant.
 *
 * Toute ligne du plan déclare son type entre accents graves — `[R]` ou `[M]` —, et c'est la seule
 * marque que les 193 lignes partagent sans exception. Les autres tableaux du document (les tranches
 * du mineur, l'état des dépôts, les passages d'une sonde) n'en portent aucune.
 *
 * C'est donc la définition qui permet de compter les lignes **sans passer par le motif qu'on veut
 * vérifier** — sans quoi le compteur serait aveugle exactement où le motif l'est, et confirmerait
 * chaque cécité au lieu de la dénoncer.
 */
const typedRow = /^\| (\S+) [^|]*`\[(?:R|M)\]`/gm;

/** Le motif d'un titre d'entrée de ledger : `## 2026-08-19 — W5.q — …`. */
const ledgerHeading = new RegExp(String.raw`^## \d{4}-\d{2}-\d{2} — (${ITEM})\b([^\n]*)$`, "gm");

/**
 * Le motif d'une entrée qui consigne une **décision**, pas une livraison.
 *
 * `## 2026-08-18 — W15.f — Bloqué : le rôle n'a pas de chemin jusqu'à son lecteur` — « Aucun code
 * écrit. L'item est consigné bloqué plutôt que livré incomplet. » C'est la bonne pratique : le
 * sprint a instruit la question, trouvé pourquoi elle ne se répond pas encore, et l'a écrit. Mais
 * une garde qui compte les entrées comptait celle-ci comme une livraison, et la ligne de `W15.f`
 * portait **fait** sans que rien ne proteste.
 *
 * Trois préfixes, et ils disent trois choses différentes : `Bloqué` attend une décision ou un fait
 * extérieur, `Reporté` attend une machine, `Partiel` attend le reste du même travail. Les fondre
 * en un seul « pas fini » ferait perdre à chacun ce qui dit **quoi attendre**.
 *
 * `Partiel` est le troisième, et il est né d'un item qui traverse deux dépôts : `W15.f` livre son
 * lecteur dans `canterel` avant son opération dans `locusolus`, parce que la décision 4 d'ADR 0016
 * exige cet ordre. La moitié livrée mérite son entrée — c'est ce qu'un registre est pour — mais une
 * entrée titrée `W15.f` sans plus vaudrait livraison de l'item entier, et la roadmap porterait
 * **fait** sur un tiers de travail. C'est le même défaut que `W15.f` a révélé sous sa forme
 * « bloqué », rencontré dans l'autre sens quinze minutes plus tard.
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
const decisionHeading = /—\s*(Bloqué|Reporté|Partiel)(?!\p{L})/u;

/** Le motif d'une ligne d'item de roadmap : `| W5.q `[R]` **fait** | … |`. */
const roadmapRow = new RegExp(String.raw`^\| (${ITEM}) ([^|]*)\|`, "gm");

/** La ligne entière, pour lire la raison d'un blocage — le troisième champ. */
const wholeRow = new RegExp(String.raw`^\| (${ITEM}) [^|]*\|([^\n]*)$`, "gm");

/**
 * Ce qu'une ligne bloquée déclare attendre, sous une forme que le garde sait lire.
 *
 * # Pourquoi un marqueur, et pas la prose
 *
 * La raison d'un blocage est une phrase, et une phrase ne se vérifie pas. `W17.f` disait « attend
 * `locusd`, décomposé en **W20** » alors que `W20` était livré : le blocage avait expiré, la ligne
 * disait encore « ne va pas là », et la frontière calculée était **vide** alors qu'un item était
 * prêt. Une session s'est arrêtée dessus, et c'est exactement l'erreur que `W0.11` devait rendre
 * impossible — dans l'autre sens.
 *
 * Deviner l'identifiant dans la prose ne marche pas non plus, et l'essayer l'a montré : la raison de
 * `W18.f` **cite** `W5.f` — « `W5.f` a rendu la condition précise » — sans l'attendre le moins du
 * monde. Une règle qui lirait « tout identifiant mentionné » aurait donc crié au blocage périmé sur
 * une ligne qui attend un hôte que personne ne peut fournir.
 *
 * D'où `attend:<quoi>`, écrit exprès. `attend:externe` dit « rien dans ce plan ne le débloquera » et
 * ne se vérifie pas ; `attend:W20` se vérifie, et se périme tout seul.
 */
const awaits = new RegExp(String.raw`attend:(W\d+(?:${WITHIN})?|R\d+|externe)`, "g");

/**
 * Un item de recherche est un **item**, jamais une phase.
 *
 * `satisfied` tranche sur la présence d'un point : `W15.f` est un item, `W20` est une phase dont il
 * faut regarder toutes les lignes. `R1` n'a pas de point et n'est pas une phase pour autant — sans
 * cette règle il partait chercher des lignes `R1.<quelque chose>`, n'en trouvait aucune, et
 * `attend:R1` devenait insatisfiable **à jamais**.
 *
 * C'est pour cela que la famille entre dans `awaits` et dans `satisfied` du même geste. Un marqueur
 * qui s'analyse mais ne peut jamais être satisfait est pire que les deux états qu'il départage : la
 * ligne qui le porte reste bloquée sans que rien ne le dise, et le garde a l'air de la vérifier.
 */
const research = /^R\d+$/;

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
  /** Combien de lignes d'item le motif a **reconnues** — voir `typedRow`. */
  readonly recognised: number;
  /**
   * Les lignes d'item que le motif n'a **pas** reconnues, sous leur premier jeton.
   *
   * Non vide veut dire que le plan porte une forme d'identifiant que le garde ne sait pas lire, donc
   * qu'il travaille sur un sous-ensemble en croyant travailler sur le tout. C'est le seul écart de
   * ce fichier qui invalide **tous** les autres constats, et il se rapporte comme tel.
   */
  readonly unrecognised: readonly string[];
  /**
   * Les identifiants que **deux lignes d'item ou plus** portent — `W0.17`.
   *
   * # La propriété que ce fichier affirmait sans jamais la vérifier
   *
   * Le commentaire d'en-tête dit, en toutes lettres : « les identifiants sont uniques sur
   * l'ensemble du chantier ». Rien ne le vérifiait, et quatre paires y avaient contrevenu — trois
   * d'entre elles posées par les deux sessions qui ont précédé cette garde.
   *
   * # Pourquoi c'est un trou dans la garde, et pas une gêne de lecture
   *
   * `delivered`, `marked`, `decided` et `planned` sont des **ensembles**. Deux lignes qui partagent
   * un identifiant s'y confondent, et les deux règles centrales cessent de voir la seconde :
   *
   * - `marque-non-livre` — une ligne marquée **fait** sans entrée à elle passe, parce que l'entrée
   *   de sa jumelle a mis l'identifiant dans `delivered` ;
   * - `livre-non-marque` — une ligne non marquée passe, parce que la marque de sa jumelle a mis
   *   l'identifiant dans `marked`.
   *
   * Autrement dit : un identifiant partagé permet exactement ce que `W0.11` avait retiré — que le
   * plan mente sur son état sans qu'aucune règle ne le dise. Il se rapporte donc **en premier**,
   * comme `unrecognised` et pour la même raison : il n'invalide pas un constat, il invalide la
   * portée de tous les autres.
   */
  readonly duplicated: readonly string[];
  readonly unread: readonly string[];
  /** Pour chaque ligne décidée qui l'a déclaré, ce qu'elle attend — voir `awaits`. */
  readonly awaiting: ReadonlyMap<string, readonly string[]>;
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
  // Compté **avant** l'ajout à `planned` — `W0.17`. C'est le seul endroit où la multiplicité existe
  // encore : la ligne suivante la détruit, et toutes les règles d'après travaillent sur des
  // ensembles où deux lignes homonymes n'en font plus qu'une.
  const vues = new Map<string, number>();
  for (const [, item, tail] of roadmap.matchAll(roadmapRow)) {
    if (item === undefined || tail === undefined) {
      continue;
    }
    vues.set(item, (vues.get(item) ?? 0) + 1);
    planned.add(item);
    if (tail.includes("fait")) {
      marked.add(item);
    } else if (DECIDED.some((state) => tail.includes(state))) {
      decided.add(item);
    } else {
      frontier.push(item);
    }
  }

  const duplicated = [...vues]
    .filter(([, combien]) => combien > 1)
    .map(([item]) => item)
    .sort();

  const recognisable = new RegExp(String.raw`^${ITEM}$`);
  const unrecognised: string[] = [];
  let recognised = 0;
  for (const [, token] of roadmap.matchAll(typedRow)) {
    if (token === undefined) {
      continue;
    }
    if (recognisable.test(token)) {
      recognised += 1;
    } else {
      unrecognised.push(token);
    }
  }

  const awaiting = new Map<string, readonly string[]>();
  for (const [, item, rest] of roadmap.matchAll(wholeRow)) {
    if (item === undefined || rest === undefined || !decided.has(item)) {
      continue;
    }
    const declared = [...rest.matchAll(awaits)]
      .map(([, what]) => what)
      .filter((what): what is string => what !== undefined);
    if (declared.length > 0) {
      awaiting.set(item, declared);
    }
  }

  return {
    delivered,
    blocked,
    marked,
    decided,
    planned,
    frontier,
    recognised,
    unrecognised,
    duplicated,
    unread,
    awaiting,
  };
}

/**
 * Vrai quand ce que la ligne attend est **entièrement** livré.
 *
 * `externe` ne l'est jamais, et c'est le point : une ligne qui attend un hôte ou un consommateur
 * n'a pas de date, et le garde ne doit pas prétendre en connaître une.
 *
 * Ce qu'`externe` ne dit **pas** : « rien ne le débloquera jamais ». `W16.e` portait ce marqueur
 * pour une messagerie inter-agents, et l'ADR 0019 l'a levé en une décision. Le marqueur dit « aucun
 * item de ce plan », ce qui reste vrai d'un item débloqué par un arbitrage — un arbitrage n'est pas
 * une ligne du tableau. Élargir la règle pour attraper ce cas demanderait au garde de savoir qu'une
 * décision est mûre, ce qu'aucun fichier ne porte.
 *
 * # Un item et une phase ne se satisfont pas de la même chose, et les confondre disait faux
 *
 * Une **phase** — `W20` — est satisfaite quand toutes ses lignes sont marquées **ou décidées** :
 * « attend `locusd`, décomposé en W20 » veut dire « attend qu'il ne reste rien à y faire », et une
 * ligne reportée n'y sera jamais faite. Attendre indéfiniment une phase à cause d'une de ses lignes
 * reportée bloquerait un item pour une raison qui ne tombera pas.
 *
 * Un **item** — `W2.20` — ne l'est que quand il porte son marqueur. La règle valait pour les deux
 * jusqu'à ce que le dépôt de l'ADR 0026 en fasse la démonstration : `W23.b` attend `W2.20`, qui est
 * lui-même bloqué sur `W20.h`, et le garde a répondu **« `W2.20` est livré »**. Il ne l'est pas. La
 * chaîne réelle est « `W23.b` attend `W2.20` qui attend `W20.h` », et le seul verdict juste est que
 * le blocage tient.
 *
 * C'est le motif de l'ADR 0025 rencontré **dans le garde** : une affirmation fausse sur l'état du
 * système, produite par une règle qui avait l'air de vérifier quelque chose. Et c'est la forme la
 * plus coûteuse des deux sens de `W0.11` — le garde disait « vas-y » sur un item qui ne peut pas
 * être fait, ce qui ne se découvre qu'en aval, une fois le travail commencé.
 *
 * Un `R<n>` suit la règle des items : il n'a pas de point mais il n'est pas une phase.
 */
function satisfied(what: string, state: Reconciliation): boolean {
  if (what === "externe") {
    return false;
  }
  if (what.includes(".") || research.test(what)) {
    return state.marked.has(what);
  }
  const rows = [...state.planned].filter((item) => item.startsWith(`${what}.`));
  return rows.length > 0 && rows.every((item) => state.marked.has(item) || state.decided.has(item));
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

  // Avec `ligne-non-reconnue`, et avant tout le reste : un identifiant partagé ne fausse pas un
  // constat, il fausse la **portée** de tous. Deux lignes homonymes se confondent dans les ensembles,
  // et la seconde devient invisible aux deux règles centrales — celle qui exige une entrée au ledger
  // comme celle qui exige une marque au tableau. C'est `W0.11` défait en silence.
  for (const item of [...state.duplicated].sort()) {
    findings.push({
      rule: "identifiant-partage",
      where: "docs/10_V1_ROADMAP.md",
      message: `« ${item} » est porté par plus d'une ligne d'item : les deux se confondent dans les ensembles du garde, et la seconde échappe alors aux règles « livré non marqué » et « marqué non livré » — le plan peut mentir sur son état sans qu'aucune règle ne le dise`,
    });
  }

  // En premier, et sous son nom : une ligne que le motif ne reconnaît pas n'est dans aucun des
  // ensembles, donc aucune des règles suivantes ne parle d'elle. Les rapporter après les autres
  // ferait lire « quelques écarts » là où il faut lire « le garde n'a pas tout regardé ».
  for (const token of [...state.unrecognised].sort()) {
    findings.push({
      rule: "ligne-non-reconnue",
      where: "docs/10_V1_ROADMAP.md",
      message: `« ${token} » est une ligne d'item que le motif d'identifiant ne reconnaît pas : elle n'entre dans aucun ensemble, donc aucune règle ne la vérifie, et son absence de marqueur ne se voit pas`,
    });
  }

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

  for (const [item, declared] of [...state.awaiting].sort()) {
    const pending = declared.filter((what) => !satisfied(what, state));
    if (pending.length === 0) {
      findings.push({
        rule: "blocage-perime",
        where: "docs/10_V1_ROADMAP.md",
        message: `« ${item} » attend ${declared.join(", ")}, qui est livré : le blocage a expiré et la ligne dit encore de ne pas y aller`,
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
