import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import { inspectRoadmap, readReconciliation, reconcile } from "../../tooling/repo/roadmap.ts";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const scratch: string[] = [];

after(async () => {
  await Promise.all(scratch.map((path) => rm(path, { recursive: true, force: true })));
});

/**
 * **Le dépôt lui-même.**
 *
 * Le verdict est le même partout, sa profondeur non : ici les trois dépôts voisins sont sur le
 * disque et les deux règles s'appliquent ; en CI ils sont absents, et « marqué sans entrée » est
 * suspendue. Un tableau juste passe dans les deux cas — c'est un tableau faux qui passerait
 * seulement en CI, et `registres non lus` est imprimé pour que personne ne lise l'un pour l'autre.
 */
test("le dépôt lui-même est réconcilié", async () => {
  assert.deepEqual(await inspectRoadmap(repoRoot), []);
});

/**
 * **Un item livré dont la ligne ne porte pas sa marque.**
 *
 * C'est le sens qui a divergé de cent-sept items : le ledger enregistrait, la roadmap ne marquait
 * pas, et « le premier item non terminé » désignait du travail déjà fait.
 */
test("un item livré mais non marqué est rapporté", () => {
  const findings = reconcile({
    delivered: new Set(["W5.q"]),
    marked: new Set(),
    blocked: new Set(),
    decided: new Set(),
    planned: new Set(["W5.q"]),
    frontier: [],
    recognised: 0,
    unrecognised: [],
    unread: [],
    awaiting: new Map(),
  });
  const [seul] = findings;
  assert.equal(findings.length, 1);
  assert.equal(seul?.rule, "livre-non-marque");
  assert.match(seul?.message ?? "", /W5\.q/);
});

/**
 * **Un item marqué dont le ledger ne dit rien.**
 *
 * L'autre sens, et le plus coûteux : une session saute un item qui n'existe pas, et ne le découvre
 * qu'en aval. C'est celui que la garde a trouvé en premier sur ce dépôt — `W5.t`, livré dans la PR
 * d'un autre et sans entrée à lui.
 */
test("un item marqué sans entrée au ledger est rapporté", () => {
  const findings = reconcile({
    delivered: new Set(),
    marked: new Set(["W5.t"]),
    blocked: new Set(),
    decided: new Set(),
    planned: new Set(["W5.t"]),
    frontier: [],
    recognised: 0,
    unrecognised: [],
    unread: [],
    awaiting: new Map(),
  });
  const [seul] = findings;
  assert.equal(findings.length, 1);
  assert.equal(seul?.rule, "marque-non-livre");
  assert.match(seul?.message ?? "", /W5\.t/);
});

/**
 * **Un registre non lu suspend l'accusation, pas la vérification.**
 *
 * Sans registre `canterel`, un `W2.4` marqué est indiscernable d'un `W2.4` inventé — et la CI de ce
 * dépôt ne voit qu'un checkout sur quatre. Accuser reviendrait à faire rougir la CI sur du travail
 * livré. Le sens inverse, lui, reste sûr : ce qui a été lu a été lu, et un item trouvé livré et non
 * marqué l'est quel que soit le nombre de registres manquants.
 */
test("un registre non lu suspend « marqué sans entrée », jamais l'inverse", () => {
  const findings = reconcile({
    delivered: new Set(["W5.q"]),
    marked: new Set(["W2.4"]),
    blocked: new Set(),
    decided: new Set(),
    planned: new Set(["W5.q", "W2.4"]),
    frontier: [],
    recognised: 0,
    unrecognised: [],
    unread: ["canterel"],
    awaiting: new Map(),
  });
  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ["livre-non-marque"],
  );
  assert.match(findings[0]?.message ?? "", /W5\.q/);
});

/**
 * **Un item livré que la roadmap ne connaît pas n'est pas un écart.**
 *
 * `W5.g` à `W5.u` sont nés en cours de route, découverts par le sprint précédent. Exiger qu'ils
 * figurent au plan *avant* d'exister interdirait à un sprint de trouver quoi que ce soit.
 */
test("un item livré hors du plan ne compte pas comme un écart", () => {
  assert.deepEqual(
    reconcile({
      delivered: new Set(["W5.z"]),
      marked: new Set(),
      blocked: new Set(),
      decided: new Set(),
      planned: new Set(),
      frontier: [],
      recognised: 0,
      unrecognised: [],
      unread: [],
      awaiting: new Map(),
    }),
    [],
  );
});

/**
 * **Le ledger peut nommer un item futur sans le livrer.**
 *
 * C'est même ainsi qu'un sprint transmet ce qu'il a trouvé — « c'est `W5.r` », « le sujet de
 * `W5.j` ». Seul le **titre** d'une entrée atteste une livraison, et la garde doit lire les titres,
 * pas le corps.
 */
test("un item nommé dans le corps du ledger n'est pas réputé livré", async () => {
  const root = await fixture({
    ledger: [
      "# Ledger",
      "",
      "## 2026-08-19 — W5.q — ce que le runtime écrit",
      "",
      "La suite est structurelle et devient `W5.r`.",
      "",
    ].join("\n"),
    roadmap: [
      "| # | Commit | Test |",
      "|---|---|---|",
      "| W5.q `[R]` **fait** | lu | oui |",
      "| W5.r `[R]` | à faire | non |",
      "",
    ].join("\n"),
  });

  assert.deepEqual(await inspectRoadmap(root), []);
});

/**
 * **La roadmap est celle du chantier, pas celle de ce dépôt.**
 *
 * `W2.*` est livré dans `canterel`, `W10.*` dans `xiiif`. Une première version ne lisait que le
 * registre local et concluait « ok » sur vingt-deux lignes faites et non marquées — dont les dix-neuf
 * de W2. Les identifiants étant uniques sur le chantier, un item trouvé chez un voisin est livré.
 */
test("un item livré dans un dépôt voisin est vu, et son absence est nommée", async () => {
  const roadmap = [
    "| # | Commit | Test |",
    "|---|---|---|",
    "| W2.4 `[R]` | identité | oui |",
    "",
  ].join("\n");
  const ledger = "# Ledger\n";
  const entry = "# Ledger\n\n## 2026-08-18 — W2.4 — identité persistante\n";

  const isole = await fixture({ ledger, roadmap });
  const etat = await readReconciliation(isole);
  assert.deepEqual(etat.unread, ["canterel", "xiiif", "emacs-config"]);
  assert.equal(etat.delivered.has("W2.4"), false);
  assert.deepEqual(reconcile(etat), []);

  const chantier = await fixture({ ledger, roadmap, siblings: { canterel: entry } });
  const complet = await readReconciliation(chantier);
  assert.deepEqual(complet.unread, ["xiiif", "emacs-config"]);
  assert.equal(complet.delivered.has("W2.4"), true);
  assert.deepEqual(
    reconcile(complet).map((finding) => finding.rule),
    ["livre-non-marque"],
  );
});

/**
 * **Le registre local, lui, n'est jamais « non lu ».**
 *
 * Son absence est un checkout cassé, pas un voisin manquant. Le confondre avec une lecture
 * suspendue ferait conclure « ok » à une garde qui n'a rien lu du tout — la faute même que
 * `CLAUDE.md` retient du premier réveil de cette session.
 */
test("un registre local absent fait échouer la garde, il ne la suspend pas", async () => {
  const root = await mkdtemp(join(tmpdir(), "locus-roadmap-"));
  scratch.push(root);
  await mkdir(join(root, "docs"), { recursive: true });
  await writeFile(join(root, "docs/10_V1_ROADMAP.md"), "| # |\n");

  await assert.rejects(readReconciliation(root), /ENOENT/);
});

/**
 * **« ok » ne doit jamais pouvoir se lire « tout a été vérifié ».**
 *
 * La suspension de « marqué sans entrée » est invisible dans le verdict : la garde sort 0 dans les
 * deux cas. Ce qui distingue une vérification complète d'une vérification partielle est **une ligne
 * imprimée**, et une ligne que rien n'exerce est une ligne qui disparaît au premier remaniement.
 */
test("le runner nomme les registres qu'il n'a pas lus", async () => {
  const root = await fixture({
    ledger: "# Ledger\n\n## 2026-08-18 — W5.q — lu\n",
    roadmap: "| # |\n|---|\n| W5.q `[R]` **fait** |\n",
  });
  const runner = fileURLToPath(new URL("../../tooling/repo/check-roadmap.ts", import.meta.url));

  const sans = await run(runner, root);
  assert.equal(sans.code, 0);
  assert.match(sans.out, /registres non lus \(canterel, xiiif, emacs-config\)/);
  assert.match(sans.out, /suspendue/);

  const chantier = await fixture({
    ledger: "# Ledger\n\n## 2026-08-18 — W5.q — lu\n",
    roadmap: "| # |\n|---|\n| W5.q `[R]` **fait** |\n",
    siblings: { canterel: "# Ledger\n", xiiif: "# Ledger\n", "emacs-config": "# Ledger\n" },
  });
  const avec = await run(runner, chantier);
  assert.equal(avec.code, 0);
  assert.doesNotMatch(avec.out, /registres non lus/);
});

/** Lancer la garde comme `npm run check:roadmap` la lance, et lire ce qu'elle dit. */
function run(runner: string, root: string): Promise<{ code: number | null; out: string }> {
  return new Promise((resolve, reject) => {
    execFile(process.execPath, [runner, root], (error, stdout, stderr) => {
      const failed = error as (Error & { code?: number }) | null;
      if (failed && typeof failed.code !== "number") {
        reject(failed);
        return;
      }
      resolve({ code: failed?.code ?? 0, out: `${stdout}${stderr}` });
    });
  });
}

/**
 * **« Bloqué » n'est pas « à faire », et les confondre coûte un sprint.**
 *
 * La première version de la garde ne connaissait que deux états, et la première lecture de la
 * frontière qu'elle a permise a désigné `W16.d` — une ligne qui dit **bloqué** depuis qu'ADR 0017 a
 * constaté qu'il lui manque un consommateur. Les deux se ressemblent dans un tableau et pas du tout
 * dans une session : l'une attend une décision extérieure, l'autre attend qu'on l'écrive.
 */
test("bloqué et reporté ne sont pas la frontière", async () => {
  const root = await fixture({
    ledger: "# Ledger\n",
    roadmap: [
      "| # | Commit | Test |",
      "|---|---|---|",
      "| W16.d `[M]` **bloqué** | il manque un consommateur | non |",
      "| W18.f `[M]` **reporté** | attend un hôte capable | non |",
      "| W20.a `[R]` | le CommandEnvelope | non |",
      "",
    ].join("\n"),
  });

  const state = await readReconciliation(root);
  assert.deepEqual(state.frontier, ["W20.a"]);
  assert.deepEqual([...state.decided].sort(), ["W16.d", "W18.f"]);
  assert.deepEqual(reconcile(state), []);
});

/**
 * **Un item livré n'attend plus rien.**
 *
 * `bloqué` avec une entrée au registre est une contradiction, et la plus trompeuse des trois : la
 * ligne dit « n'y va pas » d'un travail déjà fait, donc personne ne va vérifier, donc elle reste.
 */
test("un item décidé bloqué qui a son entrée au ledger est rapporté", () => {
  const findings = reconcile({
    delivered: new Set(["W16.d"]),
    marked: new Set(),
    blocked: new Set(),
    decided: new Set(["W16.d"]),
    planned: new Set(["W16.d"]),
    frontier: [],
    recognised: 0,
    unrecognised: [],
    unread: [],
    awaiting: new Map(),
  });
  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ["decide-et-livre"],
  );
  assert.match(findings[0]?.message ?? "", /W16\.d/);
});

/**
 * **La frontière s'imprime, sinon elle se relit à l'œil.**
 *
 * Et une lecture à l'œil a déjà produit une erreur. Le runner la nomme à chaque passage, y compris
 * quand elle est vide — un silence se lirait « je n'ai pas regardé » aussi bien que « il n'y a
 * rien ».
 */
test("le runner nomme la frontière, vide ou non", async () => {
  const runner = fileURLToPath(new URL("../../tooling/repo/check-roadmap.ts", import.meta.url));

  const ouverte = await run(
    runner,
    await fixture({
      ledger: "# Ledger\n",
      roadmap: "| # |\n|---|\n| W20.a `[R]` |\n| W16.d `[M]` **bloqué** |\n",
    }),
  );
  assert.equal(ouverte.code, 0);
  assert.match(ouverte.out, /frontière — W20\.a$/m);

  const close = await run(
    runner,
    await fixture({
      ledger: "# Ledger\n\n## 2026-08-18 — W20.a — fait\n",
      roadmap: "| # |\n|---|\n| W20.a `[R]` **fait** |\n",
    }),
  );
  assert.equal(close.code, 0);
  assert.match(close.out, /frontière vide/);
});

/**
 * **Une entrée qui consigne un blocage n'est pas une livraison.**
 *
 * `W15.f` en avait une — « Aucun code écrit. L'item est consigné bloqué plutôt que livré
 * incomplet. » — et sa ligne portait **fait** sans que rien ne proteste, parce qu'une garde qui
 * compte les entrées comptait celle-là. Le sprint suivant l'a lue comme un prérequis satisfait de
 * `W19.a`, dont la roadmap dit qu'il en dépend.
 *
 * Le test tient aussi le **piège de l'accent** : écrite `Bloqué\b`, la règle était inerte — `\w`
 * reste `[A-Za-z0-9_]` en regex JavaScript, donc `é` n'est pas un caractère de mot et il n'y a pas
 * de frontière entre lui et l'espace suivant. La garde répondait `ok` sur une règle qui ne
 * s'exécutait pas.
 */
test("une entrée qui consigne un blocage n'est pas une livraison", async () => {
  const root = await fixture({
    ledger: [
      "# Ledger",
      "",
      "## 2026-08-18 — W15.f — Bloqué : le rôle n'a pas de chemin jusqu'à son lecteur",
      "",
      "Aucun code écrit.",
      "",
    ].join("\n"),
    roadmap: "| # |\n|---|\n| W15.f `[M]` |\n",
  });

  const state = await readReconciliation(root);
  assert.equal(state.delivered.has("W15.f"), false);
  assert.equal(state.blocked.has("W15.f"), true);
  assert.deepEqual(state.frontier, ["W15.f"]);
  assert.deepEqual(reconcile(state), []);
});

/**
 * **Et un blocage consigné chez un voisin en est un aussi.**
 *
 * Le chemin des registres voisins refait le tri que fait le chemin local, et c'est exactement là
 * qu'une duplication cesse d'être tenue : un mutant qui versait les blocages des voisins dans les
 * livraisons **survivait** à toute la suite. `W2.*` et `W10.*` se consignent chez eux — un item de
 * `canterel` bloqué et marqué **fait** ici est le même défaut que `W15.f`, à un fichier près.
 */
test("le blocage d'un voisin est un blocage", async () => {
  const ledger = "# Ledger\n";
  const roadmap = "| # |\n|---|\n| W2.11 `[R]` **fait** |\n";
  const bloque = "# Ledger\n\n## 2026-08-18 — W2.11 — Bloqué : l'amont ne l'expose pas\n";

  const partiel = await readReconciliation(
    await fixture({ ledger, roadmap, siblings: { canterel: bloque } }),
  );
  assert.equal(partiel.blocked.has("W2.11"), true);
  assert.equal(partiel.delivered.has("W2.11"), false);
  assert.deepEqual(
    reconcile(partiel),
    [],
    "deux registres manquent, et l'un d'eux pourrait livrer ce que l'autre bloque",
  );

  const complet = await readReconciliation(
    await fixture({
      ledger,
      roadmap,
      siblings: { canterel: bloque, xiiif: ledger, "emacs-config": ledger },
    }),
  );
  assert.deepEqual(complet.unread, []);
  assert.deepEqual(
    reconcile(complet).map((finding) => finding.rule),
    ["marque-mais-bloque"],
  );
});

/**
 * **Trois préfixes, et ils disent trois choses différentes.**
 *
 * `Bloqué` attend une décision ou un fait extérieur, `Reporté` attend une machine, `Partiel` attend
 * le reste du même travail. Le troisième est né d'un item qui traverse deux dépôts : `W15.f` livre
 * son lecteur dans `canterel` avant son opération dans `locusolus`, parce que la décision 4 d'ADR
 * 0016 exige cet ordre. La moitié livrée mérite son entrée ; elle ne mérite pas de valoir l'item.
 */
test("les trois préfixes empêchent une entrée de valoir livraison", async () => {
  for (const prefixe of ["Bloqué", "Reporté", "Partiel"]) {
    const root = await fixture({
      ledger: `# Ledger\n\n## 2026-08-19 — W15.f — ${prefixe} : la moitié qui manque\n`,
      roadmap: "| # |\n|---|\n| W15.f `[M]` **fait** |\n",
    });
    const state = await readReconciliation(root);
    assert.equal(state.blocked.has("W15.f"), true, prefixe);
    assert.equal(state.delivered.has("W15.f"), false, prefixe);
  }
});

/**
 * **Le mot dans le titre ne suffit pas : c'est sa place qui décide.**
 *
 * L'entrée de `W0.12` s'intitule « `« Bloqué »` n'est pas `« à faire »` » et livre bel et bien.
 * Une règle qui aurait cherché le mot dans le titre l'aurait effacée du registre — et aurait
 * ensuite exigé qu'on démarque une ligne juste, ce qui est la faute inverse et tout aussi coûteuse.
 */
test("un titre qui cite le mot livre quand même", async () => {
  const root = await fixture({
    ledger: "# Ledger\n\n## 2026-08-19 — W0.12 — « Bloqué » n'est pas « à faire »\n",
    roadmap: "| # |\n|---|\n| W0.12 `[R]` **fait** |\n",
  });

  const state = await readReconciliation(root);
  assert.equal(state.delivered.has("W0.12"), true);
  assert.equal(state.blocked.has("W0.12"), false);
  assert.deepEqual(reconcile(state), []);
});

/**
 * **Le constat doit dire ce qu'il faut faire.**
 *
 * « Marqué sans entrée au ledger » enverrait écrire une entrée qui existe déjà. Le défaut n'est pas
 * une absence, c'est un **désaccord** : la ligne dit fait, l'entrée dit bloqué, et c'est la ligne
 * qui doit céder.
 */
test("une ligne faite dont l'entrée dit bloqué a son constat à elle", () => {
  const findings = reconcile({
    delivered: new Set(),
    blocked: new Set(["W15.f"]),
    marked: new Set(["W15.f"]),
    decided: new Set(),
    planned: new Set(["W15.f"]),
    frontier: [],
    recognised: 0,
    unrecognised: [],
    unread: [],
    awaiting: new Map(),
  });
  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ["marque-mais-bloque"],
  );
  assert.match(findings[0]?.message ?? "", /W15\.f/);
});

/**
 * **Un blocage dont ce qu'il attend est livré a expiré.**
 *
 * C'est le défaut qui a failli arrêter une session : `W17.f` disait « attend `locusd`, décomposé en
 * W20 » alors que `W20` était livré. La frontière calculée était **vide**, la roadmap disait « ne va
 * pas là », et un item était pourtant prêt. `W0.11` rendait impossible qu'une ligne prétende ne pas
 * être faite ; celle-ci rend impossible qu'elle prétende être bloquée après coup.
 */
test("un blocage dont la dépendance est livrée est rapporté", () => {
  const findings = reconcile({
    delivered: new Set(["W20.a", "W20.b"]),
    marked: new Set(["W20.a", "W20.b"]),
    blocked: new Set(),
    decided: new Set(["W17.f"]),
    planned: new Set(["W17.f", "W20.a", "W20.b"]),
    frontier: [],
    recognised: 0,
    unrecognised: [],
    unread: [],
    awaiting: new Map([["W17.f", ["W20"]]]),
  });

  assert.deepEqual(
    findings.map((finding) => finding.rule),
    ["blocage-perime"],
  );
  assert.match(findings[0]?.message ?? "", /W17\.f/u);
});

/** Une phase dont une ligne reste ouverte ne débloque rien. */
test("un blocage dont la dépendance est incomplète n'est pas rapporté", () => {
  const findings = reconcile({
    delivered: new Set(["W20.a"]),
    marked: new Set(["W20.a"]),
    blocked: new Set(),
    decided: new Set(["W17.f"]),
    planned: new Set(["W17.f", "W20.a", "W20.b"]),
    frontier: ["W20.b"],
    recognised: 0,
    unrecognised: [],
    unread: [],
    awaiting: new Map([["W17.f", ["W20"]]]),
  });

  assert.deepEqual(findings, []);
});

/**
 * **`externe` ne se périme jamais**, et c'est ce qui distingue les lignes bloquées entre elles.
 *
 * `W18.f` attend un hôte capable : ce blocage n'a pas de date, et un garde qui prétendrait en
 * connaître une enverrait une session construire une fonctionnalité pour justifier un test.
 *
 * `W16.e` portait le même marqueur pour une messagerie inter-agents, et l'ADR 0019 l'a levé — non
 * pas parce que le garde s'est trompé, mais parce que `externe` désigne ce qu'aucun item du plan ne
 * débloquera, et **pas** ce qu'aucune décision ne débloquera. Les deux se ressemblent dans un
 * tableau et diffèrent entièrement : le premier attend une machine, le second attendait un
 * arbitrage. Le garde ne sait pas faire la différence, et c'est pour cela qu'il ne se prononce sur
 * aucun des deux.
 */
test("un blocage externe ne se périme pas", () => {
  const findings = reconcile({
    delivered: new Set(["W20.a"]),
    marked: new Set(["W20.a"]),
    blocked: new Set(),
    decided: new Set(["W18.f"]),
    planned: new Set(["W18.f", "W20.a"]),
    frontier: [],
    recognised: 0,
    unrecognised: [],
    unread: [],
    awaiting: new Map([["W18.f", ["externe"]]]),
  });

  assert.deepEqual(findings, []);
});

/**
 * **Citer un item n'est pas l'attendre**, et c'est pour cela que le marqueur est explicite.
 *
 * La raison de `W18.f` mentionne `W5.f` — « `W5.f` a rendu la condition précise » — sans en dépendre.
 * Une première version de la règle lisait tout identifiant présent dans la prose ; elle aurait crié
 * au blocage périmé sur une ligne qui attend un hôte que personne ne peut fournir. Une ligne sans
 * marqueur n'est donc pas vérifiée du tout, ce qui est le seul comportement honnête : le garde ne
 * devine pas une dépendance, il lit celle qu'on a déclarée.
 */
test("une ligne bloquée sans marqueur n'est jamais rapportée", async () => {
  const root = await fixture({
    ledger: "# Ledger\n\n## 2026-08-18 — W5.f — la sonde\n",
    roadmap: [
      "| # | Commit | Test |",
      "|---|---|---|",
      "| W5.f `[R]` **fait** | une sonde | verte |",
      "| W18.f `[M]` **reporté** | l'admission de bout en bout | attend un hôte capable, et `W5.f` a rendu la condition précise |",
    ].join("\n"),
  });

  assert.deepEqual(await inspectRoadmap(root), []);
});

/**
 * **Les deux familles d'identifiants, et le seul test qui prouve que les deux motifs ont bougé.**
 *
 * Le plan porte des `W<phase>.<item>` et des `R<n>`, et le garde ne lisait que les premiers — dans
 * le registre **et** dans le tableau, puisque le même alphabet ancrait les deux motifs. Les six
 * items de recherche ont été livrés le 2026-08-18, avec leur code, leurs tests et leurs mutants ;
 * le garde a répondu `frontière vide` et `ok` par-dessus pendant trois jours.
 *
 * Ce test tient les deux motifs à la fois, et c'est voulu : si seul le motif de registre avait été
 * élargi, `R1` ne serait pas au plan et aucun constat ne sortirait ; si seul celui du tableau
 * l'avait été, `R1` ne serait pas livré et aucun constat ne sortirait non plus. Le constat
 * n'apparaît que lorsque les deux lisent la même famille — c'est pourquoi l'alphabet est écrit une
 * fois et partagé, plutôt que recopié quatre fois.
 */
test("un item de recherche est lu dans le registre comme dans le plan", async () => {
  const ledger = "# Ledger\n\n## 2026-08-18 — R1 — Le consensus circulaire, lu sur le graphe\n";

  const ouvert = await readReconciliation(
    await fixture({ ledger, roadmap: "| # |\n|---|\n| R1 `[R]` | le consensus circulaire |\n" }),
  );
  assert.equal(ouvert.delivered.has("R1"), true, "le titre d'entrée atteste la livraison");
  assert.deepEqual(ouvert.frontier, ["R1"], "la prose ne nommait aucune frontière");
  assert.deepEqual(
    reconcile(ouvert).map((finding) => finding.rule),
    ["livre-non-marque"],
  );

  const marque = await readReconciliation(
    await fixture({
      ledger,
      roadmap: "| # |\n|---|\n| R1 `[R]` **fait** | le consensus circulaire |\n",
    }),
  );
  assert.deepEqual(marque.frontier, []);
  assert.deepEqual(reconcile(marque), []);
});

/**
 * **`attend:R<n>` se périme, sinon il ne devrait pas s'analyser du tout.**
 *
 * `satisfied` tranche sur la présence d'un point : `W15.f` est un item, `W20` une phase dont il faut
 * regarder toutes les lignes. `R1` n'a pas de point et n'est pas une phase pour autant — sans la
 * règle qui le dit, il partait chercher des lignes `R1.<quelque chose>`, n'en trouvait aucune, et
 * `attend:R1` devenait insatisfiable **à jamais**.
 *
 * C'est le pire des trois états : la ligne resterait bloquée sans que rien ne le dise, et le garde
 * aurait l'air de la vérifier. La famille entre donc dans le marqueur et dans `satisfied` du même
 * geste, et les deux sens sont tenus ici — livré, le blocage expire ; ouvert, il tient.
 */
test("un blocage qui attend un item de recherche livré est rapporté", async () => {
  const bloquee =
    "| W9.z `[R]` **bloqué** | la vue 3D des cycles | `attend:R1` — la détection n'existe pas |";

  const perime = await readReconciliation(
    await fixture({
      ledger: "# Ledger\n\n## 2026-08-18 — R1 — Le consensus circulaire\n",
      roadmap: `| # |\n|---|\n| R1 \`[R]\` **fait** | le consensus circulaire |\n${bloquee}\n`,
    }),
  );
  assert.deepEqual(perime.awaiting.get("W9.z"), ["R1"]);
  assert.deepEqual(
    reconcile(perime).map((finding) => finding.rule),
    ["blocage-perime"],
  );

  const tient = await readReconciliation(
    await fixture({
      ledger: "# Ledger\n",
      roadmap: `| # |\n|---|\n| R1 \`[R]\` | le consensus circulaire |\n${bloquee}\n`,
    }),
  );
  assert.deepEqual(reconcile(tient), [], "R1 reste à faire, et le blocage tient");
});

/**
 * **Un item bloqué ne satisfait pas ce qui l'attend, et le dire serait une affirmation fausse.**
 *
 * Le dépôt de l'ADR 0026 a fait la démonstration : `W23.b` attend `W2.20`, qui est lui-même bloqué
 * sur `W20.h`, et le garde a répondu « `W2.20` est livré ». Il ne l'est pas. La chaîne réelle est
 * « `W23.b` attend `W2.20` qui attend `W20.h` », et le seul verdict juste est que le blocage tient.
 *
 * Une phase, elle, garde l'autre règle : « attend `W20` » veut dire « attend qu'il ne reste rien à y
 * faire », et une ligne reportée n'y sera jamais faite. Les deux moitiés sont tenues ici, parce que
 * corriger l'une en cassant l'autre bloquerait indéfiniment un item pour une raison qui ne tombe pas.
 *
 * C'est le motif de l'ADR 0025 rencontré **dans un garde** : une règle qui avait l'air de vérifier
 * quelque chose, et qui affirmait faux sur l'état du système.
 */
test("un blocage qui attend un item lui-même bloqué tient", async () => {
  const chaine = [
    "| # |",
    "|---|",
    "| W20.h `[R]` | la sérialisation des écritures |",
    "| W2.20 `[R]` **bloqué** | la boucle du worker | `attend:W20.h` |",
    "| W23.b `[R]` **bloqué** | les trois compteurs | `attend:W2.20` |",
    "",
  ].join("\n");

  const tient = await readReconciliation(await fixture({ ledger: "# Ledger\n", roadmap: chaine }));
  assert.deepEqual(tient.awaiting.get("W23.b"), ["W2.20"]);
  assert.deepEqual(reconcile(tient), [], "W2.20 est bloqué, donc il ne satisfait rien");

  const livre = await readReconciliation(
    await fixture({
      ledger: "# Ledger\n\n## 2026-08-22 — W2.20 — La boucle du worker\n",
      roadmap: chaine.replace("| W2.20 `[R]` **bloqué** |", "| W2.20 `[R]` **fait** |"),
    }),
  );
  assert.deepEqual(
    reconcile(livre).map((finding) => finding.rule),
    ["blocage-perime"],
    "livré, le blocage de W23.b expire",
  );
});

/**
 * **Une phase reste satisfaite par une ligne décidée, et c'est la moitié qu'il ne faut pas casser.**
 *
 * `attend:W20` veut dire « attend qu'il ne reste rien à faire dans W20 ». Une ligne reportée n'y sera
 * jamais faite : exiger son marqueur bloquerait l'attendant à jamais, ce qui est exactement ce que
 * `W0.17` décrit comme pire que les deux états qu'un marqueur départage.
 */
test("une phase dont une ligne est reportée satisfait quand même ce qui l'attend", async () => {
  const state = await readReconciliation(
    await fixture({
      ledger: "# Ledger\n\n## 2026-08-20 — W12.a — Le registre d'épreuves\n",
      roadmap: [
        "| # |",
        "|---|",
        "| W12.a `[R]` **fait** | le registre d'épreuves |",
        "| W12.e `[M]` **reporté** | l'attestation | `attend:externe` |",
        "| W9.z `[R]` **bloqué** | la vue 3D | `attend:W12` |",
        "",
      ].join("\n"),
    }),
  );

  assert.deepEqual(
    reconcile(state).map((finding) => finding.rule),
    ["blocage-perime"],
    "W12 n'a plus rien à faire, donc le blocage de W9.z a expiré",
  );
});

/**
 * **Une phase dont aucune ligne n'existe n'est pas satisfaite ; sans quoi une coquille dit « vas-y ».**
 *
 * `rows.every(...)` sur un ensemble vide vaut `true` : sans le `rows.length > 0` qui le précède,
 * `attend:W99` — une phase mal orthographiée, ou pas encore écrite — serait rapporté comme un blocage
 * expiré. Le garde enverrait alors une session sur un item bloqué en lui disant que sa dépendance est
 * livrée, ce qui est le plus coûteux des deux sens de `W0.11`.
 *
 * Trouvé par une passe de mutants sur `satisfied` : le seul survivant de la passe était la disparition
 * de cette garde-là, et rien ne l'éprouvait.
 */
test("un blocage qui attend une phase dont aucune ligne n'existe tient", async () => {
  const state = await readReconciliation(
    await fixture({
      ledger: "# Ledger\n",
      roadmap: ["| # |", "|---|", "| W9.z `[R]` **bloqué** | la vue 3D | `attend:W99` |", ""].join(
        "\n",
      ),
    }),
  );

  assert.deepEqual(state.awaiting.get("W9.z"), ["W99"]);
  assert.deepEqual(reconcile(state), [], "aucune ligne de W99 n'a été lue, donc rien n'est conclu");
});

/** Un chantier jouet : le dépôt confronté, et les voisins qu'on veut bien lui donner. */
async function fixture(contents: {
  ledger: string;
  roadmap: string;
  siblings?: Record<string, string>;
}): Promise<string> {
  const chantier = await mkdtemp(join(tmpdir(), "locus-chantier-"));
  scratch.push(chantier);
  const root = join(chantier, "locusolus");
  await mkdir(join(root, "docs"), { recursive: true });
  await writeFile(join(root, "IMPLEMENTATION_LEDGER.md"), contents.ledger);
  await writeFile(join(root, "docs/10_V1_ROADMAP.md"), contents.roadmap);
  for (const [name, ledger] of Object.entries(contents.siblings ?? {})) {
    await mkdir(join(chantier, name), { recursive: true });
    await writeFile(join(chantier, name, "IMPLEMENTATION_LEDGER.md"), ledger);
  }
  return root;
}

/**
 * **Une ligne d'item d'une forme que le motif ne connaît pas est rapportée.**
 *
 * Le test qui porte `W22.a`. C'est le seul écart de cette garde qui invalide tous les autres
 * constats : une ligne non reconnue n'entre dans aucun ensemble, donc aucune règle ne parle d'elle,
 * donc **aucun décompte ne baisse** — et « ok » se lit « tout est vérifié ».
 *
 * Le compteur est dérivé du type `[R]`/`[M]`, que les 193 lignes du plan portent sans exception, et
 * jamais du motif d'identifiant : un compteur qui passerait par le motif serait aveugle exactement
 * là où le motif l'est, et confirmerait chaque cécité au lieu de la dénoncer.
 */
test("une ligne d'item que le motif ne reconnaît pas est rapportée", async () => {
  const inconnue = await readReconciliation(
    await fixture({
      ledger: "# Ledger\n",
      roadmap: "| # |\n|---|\n| W20.a `[R]` **fait** |\n| W4.d.1.7 `[R]` |\n",
    }),
  );

  assert.deepEqual(inconnue.unrecognised, ["W4.d.1.7"]);
  assert.equal(inconnue.recognised, 1, "l'autre ligne, elle, est reconnue");
  assert.equal(inconnue.planned.has("W4.d.1.7"), false, "et elle n'est dans aucun ensemble");

  const findings = reconcile(inconnue);
  assert.equal(
    findings[0]?.rule,
    "ligne-non-reconnue",
    "et c'est le premier constat, pas le dernier",
  );
  assert.match(findings[0]?.message ?? "", /W4\.d\.1\.7/);
});

/**
 * **Un item à deux points est vu par les deux règles, dans les deux sens.**
 *
 * `livre-non-marque` et `marque-non-livre` existaient depuis `W0.12` et `W0.13`, et elles étaient
 * justes. Elles ne voyaient simplement pas les huit lignes que le motif tronquait — la garde n'avait
 * pas de trou de règle, elle avait un trou de vue.
 *
 * Ce test exerce donc les deux règles **sur la forme qui manquait**, ce qui est la seule
 * non-régression qui compte : réparer le motif sans le tenir laisserait la prochaine forme repasser.
 */
test("un item à deux points est vu dans les deux sens", () => {
  const sansMarqueur = reconcile({
    delivered: new Set(["W4.d.1"]),
    marked: new Set(),
    blocked: new Set(),
    decided: new Set(),
    planned: new Set(["W4.d.1"]),
    frontier: [],
    recognised: 1,
    unrecognised: [],
    unread: [],
    awaiting: new Map(),
  });
  assert.equal(sansMarqueur.length, 1);
  assert.equal(sansMarqueur[0]?.rule, "livre-non-marque");
  assert.match(sansMarqueur[0]?.message ?? "", /W4\.d\.1/);

  const sansEntree = reconcile({
    delivered: new Set(),
    marked: new Set(["W4.d.1"]),
    blocked: new Set(),
    decided: new Set(),
    planned: new Set(["W4.d.1"]),
    frontier: [],
    recognised: 1,
    unrecognised: [],
    unread: [],
    awaiting: new Map(),
  });
  assert.equal(sansEntree.length, 1);
  assert.equal(sansEntree[0]?.rule, "marque-non-livre");
});

/**
 * **Un blocage qui nomme un item à deux points le nomme en entier.**
 *
 * Le motif d'`attend:` et celui des lignes doivent s'accorder sur ce qu'est un identifiant. S'ils
 * divergent, `attend:W4.d.1` se lit `W4.d` — un **autre** item — et le blocage se périme ou ne se
 * périme pas pour de mauvaises raisons, sans que rien ne le signale. Une troncature silencieuse est
 * exactement la faute que cet item existe pour supprimer.
 */
test("un blocage qui attend un item à deux points le lit en entier", async () => {
  const state = await readReconciliation(
    await fixture({
      ledger: "# Ledger\n",
      roadmap: [
        "| # |",
        "|---|",
        "| W4.d.1 `[R]` |",
        "| W9.a `[R]` **bloqué** | attend:W4.d.1 |",
        "",
      ].join("\n"),
    }),
  );

  assert.deepEqual(state.awaiting.get("W9.a"), ["W4.d.1"]);
  assert.deepEqual(reconcile(state), [], "W4.d.1 n'est ni marqué ni décidé : le blocage tient");
});

/**
 * **Le runner déclare ce qu'il a reconnu, avant toute conclusion.**
 *
 * Sans cette ligne, une cécité du motif se lit « rien à signaler ». C'est ce qui est arrivé : la
 * garde imprimait « frontière vide — toute ligne du plan est faite » sur un plan dont huit lignes
 * livrées n'avaient pas de marqueur, et le nombre n'était nulle part.
 */
test("le runner déclare le nombre de lignes reconnues", async () => {
  const runner = fileURLToPath(new URL("../../tooling/repo/check-roadmap.ts", import.meta.url));

  const propre = await run(
    runner,
    await fixture({
      ledger: "# Ledger\n\n## 2026-08-18 — W20.a — fait\n",
      roadmap: "| # |\n|---|\n| W20.a `[R]` **fait** |\n",
    }),
  );
  assert.equal(propre.code, 0);
  assert.match(propre.out, /1 ligne\(s\) d'item reconnue\(s\) sur 1/);

  const borgne = await run(
    runner,
    await fixture({
      ledger: "# Ledger\n\n## 2026-08-18 — W20.a — fait\n",
      roadmap: "| # |\n|---|\n| W20.a `[R]` **fait** |\n| W4.d.1.7 `[R]` |\n",
    }),
  );
  assert.notEqual(borgne.code, 0, "une ligne non reconnue fait échouer la garde");
  assert.match(borgne.out, /1 ligne\(s\) d'item reconnue\(s\) sur 2/);
});
