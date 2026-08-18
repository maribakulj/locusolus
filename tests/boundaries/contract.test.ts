import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { inspectBoundaries } from "../../tooling/boundaries/analyze.ts";
import { emacsAvailable } from "../../tooling/boundaries/emacs.ts";
import { loadContract } from "../../tooling/boundaries/rules.ts";
import { loadFixtures, repoRoot } from "./fixtures.ts";

const contract = await loadContract(join(repoRoot, "boundaries.json"));

/**
 * Les deux assertions sur la règle 5 démarrent un vrai Emacs — y compris celle qui porte sur le
 * cas « sans objet », puisque la garde vérifie la disponibilité d'Emacs avant de regarder les
 * points d'entrée. Elles se sautent donc là où il n'y en a pas, **en le disant**, et tournent dans
 * le job `emacs` de la CI, qui en installe un et lance ce fichier exprès. Un `skip` muet
 * ressemblerait à un succès : c'est le défaut que la règle 5 existe pour ne pas avoir.
 */
const withoutEmacs = (await emacsAvailable())
  ? false
  : "emacs absent : ces assertions tournent dans le job `emacs`";
const frontiers = await claudeMdFrontiers();

test("le contrat porte les frontières de CLAUDE.md, dans l'ordre", () => {
  const numbered = [...frontiers.keys()];
  assert.deepEqual(numbered, [1, 2, 3, 4, 5, 6], "la liste de CLAUDE.md est numérotée sans trou");
  const carried = contract.rules.map((rule) => rule.claudeMd);
  assert.deepEqual([...new Set(carried)], numbered, "chaque frontière a au moins une règle");
  assert.deepEqual(
    [...carried].sort((a, b) => a - b),
    carried,
    "les règles suivent l'ordre de la liste",
  );
});

/**
 * `boundaries.json` dit que si les deux divergent, CLAUDE.md fait foi. Jusqu'ici personne ne le
 * vérifiait : une frontière reformulée d'un côté aurait laissé l'autre décrire une garantie que
 * plus rien ne portait. Une règle écrite en deux entrées — un scope par sens — porte deux fois le
 * même énoncé, et c'est ce que la comparaison exige.
 */
test("l'énoncé de chaque règle est celui de CLAUDE.md, mot pour mot", () => {
  for (const rule of contract.rules) {
    const expected = frontiers.get(rule.claudeMd);
    assert.ok(expected, `CLAUDE.md ne porte aucune frontière ${rule.claudeMd}`);
    assert.equal(plain(rule.statement), plain(expected), `règle ${rule.id}`);
  }
});

/** Les frontières numérotées de « Frontières vérifiées par la CI », dans l'ordre du fichier. */
async function claudeMdFrontiers(): Promise<Map<number, string>> {
  const source = await readFile(join(repoRoot, "CLAUDE.md"), "utf8");
  const section = source.split("## Frontières vérifiées par la CI")[1];
  assert.ok(section, "CLAUDE.md doit porter la section « Frontières vérifiées par la CI »");
  const frontiers = new Map<number, string>();
  for (const line of section.split("\n")) {
    if (line.startsWith("#") || line.trimStart().startsWith("---")) break;
    const match = /^(\d+)\.\s+(.*\S)\s*$/.exec(line);
    if (match?.[1] && match[2]) frontiers.set(Number(match[1]), match[2]);
  }
  return frontiers;
}

/** Le balisage Markdown n'est pas l'énoncé : `boundaries.json` porte le texte nu. */
function plain(statement: string): string {
  return statement.replaceAll("`", "").replaceAll("**", "");
}

test("aucune règle n'est admise sans violation délibérée qui la démontre", async () => {
  const demonstrated = new Set<string>();
  for (const family of ["imports", "emacs"]) {
    for (const fixture of await loadFixtures(family)) {
      for (const violation of fixture.expected.violations) demonstrated.add(violation.rule);
    }
  }
  const unproven = contract.rules.map((rule) => rule.id).filter((id) => !demonstrated.has(id));
  assert.deepEqual(unproven, [], "une règle qu'aucune fixture ne met en défaut ne prouve rien");
});

test("le dépôt lui-même ne franchit aucune frontière", async () => {
  const report = await inspectBoundaries(repoRoot, contract, { emacs: "auto" });
  assert.deepEqual(report.findings, []);
});

/**
 * La propriété tient sur un dépôt où l'unité ne porte aucun point d'entrée — pas sur celui-ci.
 *
 * Ce test employait `repoRoot` comme fixture, et il disait vrai tant qu'`apps/emacs` était vide.
 * W8.a l'a peuplé, donc la fixture a cessé de décrire le cas qu'elle prétendait couvrir : un test
 * dont la prémisse dépend de l'avancement du dépôt s'éteint le jour où le dépôt avance, et
 * l'extinction ressemble à un succès. La prémisse est donc construite ici.
 */
test(
  "une règle sans objet est déclarée comme telle, jamais comptée comme vérifiée",
  { skip: withoutEmacs },
  async () => {
    const empty = await mkdtemp(join(tmpdir(), "locus-boundaries-"));
    await mkdir(join(empty, "apps", "emacs"), { recursive: true });

    const report = await inspectBoundaries(empty, contract, { emacs: "auto" });
    const emacs = report.statuses.find((status) => status.rule.kind === "emacs-isolation");
    assert.ok(emacs, "la règle 5 doit apparaître dans le rapport");
    assert.equal(emacs.state, "not-applicable", "sans point d'entrée, il n'y a rien à vérifier");
    assert.equal(emacs.scanned, 0);
    assert.deepEqual(report.findings, [], "sans objet n'est pas une violation");
  },
);

/**
 * Et le pendant, sur le dépôt réel : depuis W8.a, `apps/emacs` porte des points d'entrée, donc la
 * règle 5 doit être **vérifiée**. Sans cette assertion, la garde pourrait se remettre à sauter la
 * règle — faute d'Emacs, ou parce qu'un renommage l'aurait rendue « sans objet » — et le rapport
 * dirait « ok » d'une frontière que plus rien ne tient.
 */
test(
  "la règle 5 est vérifiée, maintenant qu'apps/emacs porte des points d'entrée",
  { skip: withoutEmacs },
  async () => {
    const report = await inspectBoundaries(repoRoot, contract, { emacs: "required" });
    const emacs = report.statuses.find((status) => status.rule.kind === "emacs-isolation");
    assert.ok(emacs);
    assert.equal(emacs.state, "enforced");
    assert.ok(emacs.scanned > 0, "au moins un point d'entrée est chargé sous emacs -Q");
    assert.deepEqual(report.findings, []);
  },
);
