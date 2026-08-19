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
    planned: new Set(["W5.q"]),
    unread: [],
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
    planned: new Set(["W5.t"]),
    unread: [],
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
    planned: new Set(["W5.q", "W2.4"]),
    unread: ["canterel"],
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
      planned: new Set(),
      unread: [],
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
