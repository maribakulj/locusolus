import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import { inspectNaming, retiredName } from "../../tooling/repo/naming.ts";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const scratch: string[] = [];

after(async () => {
  await Promise.all(scratch.map((path) => rm(path, { recursive: true, force: true })));
});

test("le dépôt ne porte aucune occurrence non justifiée du nom retiré", async () => {
  assert.deepEqual(await inspectNaming(repoRoot), []);
});

test("une occurrence non justifiée est signalée avec sa ligne", async () => {
  const root = await tree({
    "docs/notes.md": `# titre\n\nvoir le dépôt ${retiredName} pour la suite\n`,
  });
  assert.deepEqual(
    (await inspectNaming(root, new Map())).map((finding) => finding.where),
    ["docs/notes.md:3"],
  );
});

test("une occurrence justifiée ne l'est pas", async () => {
  const root = await tree({ "docs/adr/0009.md": `remplace ${retiredName}-emacs\n` });
  const allowed = new Map([["docs/adr/0009.md", "consigne le nom auquel l'ADR se substitue"]]);
  assert.deepEqual(await inspectNaming(root, allowed), []);
});

test("une dérogation qui ne correspond plus à rien est signalée", async () => {
  const root = await tree({ "docs/notes.md": "# rien à signaler\n" });
  const allowed = new Map([["docs/notes.md", "raison devenue caduque"]]);
  assert.deepEqual(
    (await inspectNaming(root, allowed)).map((finding) => finding.rule),
    ["retired-name-stale-exemption"],
  );
});

async function tree(files: Record<string, string>): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "locus-naming-"));
  scratch.push(root);
  for (const [path, content] of Object.entries(files)) {
    const target = join(root, path);
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, content);
  }
  return root;
}
