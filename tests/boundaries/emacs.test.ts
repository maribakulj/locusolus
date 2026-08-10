import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";

import { inspectBoundaries } from "../../tooling/boundaries/analyze.ts";
import { emacsAvailable } from "../../tooling/boundaries/emacs.ts";
import { loadContract } from "../../tooling/boundaries/rules.ts";
import { loadFixtures, repoRoot, verdicts } from "./fixtures.ts";

const contract = await loadContract(join(repoRoot, "boundaries.json"));
const skip = (await emacsAvailable()) ? false : "emacs absent de cette machine";

for (const fixture of await loadFixtures("emacs")) {
  test(`${fixture.name} — ${fixture.expected.title}`, { skip }, async () => {
    const report = await inspectBoundaries(fixture.root, contract, { emacs: "required" });
    assert.deepEqual(verdicts(report.findings), verdicts(fixture.expected.violations));
  });
}
