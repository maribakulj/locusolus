import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";

import { inspectBoundaries } from "../../tooling/boundaries/analyze.ts";
import { loadContract } from "../../tooling/boundaries/rules.ts";
import { loadFixtures, repoRoot, verdicts } from "./fixtures.ts";

const contract = await loadContract(join(repoRoot, "boundaries.json"));

for (const fixture of await loadFixtures("imports")) {
  test(`${fixture.name} — ${fixture.expected.title}`, async () => {
    const report = await inspectBoundaries(fixture.root, contract, { emacs: "off" });
    assert.deepEqual(verdicts(report.findings), verdicts(fixture.expected.violations));
  });
}
