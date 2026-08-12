import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";

import { inspectBoundaries } from "../../tooling/boundaries/analyze.ts";
import { loadContract } from "../../tooling/boundaries/rules.ts";
import { loadFixtures, repoRoot } from "./fixtures.ts";

const contract = await loadContract(join(repoRoot, "boundaries.json"));

test("le contrat porte les cinq règles de CLAUDE.md, dans l'ordre", () => {
  assert.deepEqual(
    contract.rules.map((rule) => rule.claudeMd),
    [1, 2, 3, 4, 5],
  );
});

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

test("une règle sans objet est déclarée comme telle, jamais comptée comme vérifiée", async () => {
  const report = await inspectBoundaries(repoRoot, contract, { emacs: "auto" });
  const emacs = report.statuses.find((status) => status.rule.kind === "emacs-isolation");
  assert.ok(emacs, "la règle 5 doit apparaître dans le rapport");
  assert.notEqual(
    emacs.state,
    "enforced",
    "apps/emacs n'existe pas encore : la règle ne peut pas être déclarée vérifiée",
  );
});
