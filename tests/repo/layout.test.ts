import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import { inspectRepo, requiredRoots } from "../../tooling/repo/layout.ts";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const scratch: string[] = [];

after(async () => {
  await Promise.all(scratch.map((path) => rm(path, { recursive: true, force: true })));
});

test("the repository itself satisfies the layout contract", async () => {
  assert.deepEqual(await inspectRepo(repoRoot), []);
});

test("a well-formed empty skeleton is clean", async () => {
  assert.deepEqual(await inspectRepo(await skeleton()), []);
});

test("a missing top-level directory is reported", async () => {
  const root = await skeleton();
  await rm(join(root, "schemas"), { recursive: true });
  assert.deepEqual(await rules(root), ["root-layout"]);
});

test("an undocumented top-level directory is reported", async () => {
  const root = await skeleton();
  await rm(join(root, "tooling", "README.md"));
  assert.deepEqual(await rules(root), ["root-readme"]);
});

test("a unit directory holding only a placeholder is a stub", async () => {
  const root = await skeleton();
  await write(root, "packages/domain/.gitkeep", "");
  assert.deepEqual(await rules(root), ["unit-placeholder"]);
});

test("a workspace manifest must be scoped, explicit about publication, and ESM", async () => {
  const root = await skeleton();
  await write(root, "packages/domain/package.json", JSON.stringify({ name: "domain" }));
  assert.deepEqual(await rules(root), ["unit-module-type", "unit-name", "unit-private"]);
});

test("two units cannot claim the same name", async () => {
  const root = await skeleton();
  await write(root, "apps/twin/package.json", unit("@locus/twin"));
  await write(root, "packages/twin/package.json", unit("@locus/twin"));
  assert.deepEqual(await rules(root), ["unit-name-unique"]);
});

test("a conforming unit is clean", async () => {
  const root = await skeleton();
  await write(root, "packages/domain/package.json", unit("@locus/domain"));
  assert.deepEqual(await inspectRepo(root), []);
});

test("the pinned Node major must match engines.node", async () => {
  const root = await skeleton();
  await write(root, ".nvmrc", "20\n");
  assert.deepEqual(await rules(root), ["node-pin"]);
});

test("the workspace globs must cover both unit roots", async () => {
  const root = await skeleton();
  await write(root, "package.json", rootManifest(["packages/*"]));
  assert.deepEqual(await rules(root), ["workspace-globs"]);
});

test("an unparseable manifest is reported rather than thrown", async () => {
  const root = await skeleton();
  await write(root, "package.json", "{");
  assert.deepEqual(await rules(root), ["manifest-invalid"]);
});

async function rules(root: string): Promise<string[]> {
  const findings = await inspectRepo(root);
  return [...new Set(findings.map((finding) => finding.rule))].sort();
}

async function skeleton(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "locus-layout-"));
  scratch.push(root);
  await write(root, "package.json", rootManifest(["apps/*", "packages/*"]));
  await write(root, ".nvmrc", "22\n");
  for (const name of requiredRoots) await write(root, `${name}/README.md`, `# ${name}\n`);
  return root;
}

function rootManifest(workspaces: string[]): string {
  return JSON.stringify({ name: "locusolus", workspaces, engines: { node: ">=22.18.0" } });
}

function unit(name: string): string {
  return JSON.stringify({ name, private: true, type: "module" });
}

async function write(root: string, path: string, content: string): Promise<void> {
  const target = join(root, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, content);
}
