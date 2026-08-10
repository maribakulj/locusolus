import { readdir, readFile, stat } from "node:fs/promises";
import { join } from "node:path";

import type { Finding } from "../lib/findings.ts";

/** Top-level directories this repository commits to, per `docs/SPEC_V1.md` §5. */
export const requiredRoots = ["apps", "packages", "schemas", "tests", "tooling"] as const;

/** Roots whose direct children are build units — one app or one package each. */
export const unitRoots = ["apps", "packages"] as const;

/** npm scope every workspace unit is named under. */
export const workspaceScope = "@locus";

const workspaceGlobs = unitRoots.map((root) => `${root}/*`);

const placeholderNames = new Set([".gitkeep", ".keep", ".placeholder"]);

/**
 * Check the repository layout contract.
 *
 * Every rule holds vacuously on a repository that contains no code yet, and starts biting the
 * moment the first unit lands. That is the point: the contract exists before the code does.
 */
export async function inspectRepo(root: string): Promise<Finding[]> {
  const findings: Finding[] = [];
  const manifest = await readRootManifest(root, findings);
  await checkRequiredRoots(root, findings);
  if (manifest) {
    checkWorkspaceGlobs(manifest, findings);
    await checkNodePin(root, manifest, findings);
  }
  await checkUnits(root, findings);
  return findings;
}

async function checkRequiredRoots(root: string, findings: Finding[]): Promise<void> {
  for (const name of requiredRoots) {
    if (!(await isDirectory(join(root, name)))) {
      findings.push({
        rule: "root-layout",
        where: name,
        message: `required top-level directory "${name}/" is missing`,
      });
      continue;
    }
    if (!(await isFile(join(root, name, "README.md")))) {
      findings.push({
        rule: "root-readme",
        where: `${name}/README.md`,
        message: `"${name}/" must document what it holds and what it excludes`,
      });
    }
  }
}

function checkWorkspaceGlobs(manifest: Record<string, unknown>, findings: Finding[]): void {
  const declared = asStringArray(manifest["workspaces"]) ?? [];
  for (const glob of workspaceGlobs) {
    if (!declared.includes(glob)) {
      findings.push({
        rule: "workspace-globs",
        where: "package.json",
        message: `workspaces must include "${glob}"`,
      });
    }
  }
}

async function checkNodePin(
  root: string,
  manifest: Record<string, unknown>,
  findings: Finding[],
): Promise<void> {
  const engines = asRecord(manifest["engines"]);
  const range = typeof engines?.["node"] === "string" ? engines["node"] : null;
  if (!range) {
    findings.push({
      rule: "node-pin",
      where: "package.json",
      message: "engines.node must pin the supported Node major",
    });
    return;
  }
  const pinned = await readTextFile(join(root, ".nvmrc"));
  if (pinned === null) {
    findings.push({ rule: "node-pin", where: ".nvmrc", message: ".nvmrc is missing" });
    return;
  }
  const required = majorOf(range);
  const local = majorOf(pinned);
  if (required === null || local === null || required !== local) {
    findings.push({
      rule: "node-pin",
      where: ".nvmrc",
      message: `.nvmrc (${pinned.trim()}) and engines.node (${range}) must agree on the Node major`,
    });
  }
}

async function checkUnits(root: string, findings: Finding[]): Promise<void> {
  const seen = new Map<string, string>();
  for (const unitRoot of unitRoots) {
    for (const name of await readSubdirectories(join(root, unitRoot))) {
      await checkUnit(root, `${unitRoot}/${name}`, name, seen, findings);
    }
  }
}

async function checkUnit(
  root: string,
  where: string,
  name: string,
  seen: Map<string, string>,
  findings: Finding[],
): Promise<void> {
  const entries = (await readEntries(join(root, where))) ?? [];
  if (entries.every((entry) => placeholderNames.has(entry.name))) {
    findings.push({
      rule: "unit-placeholder",
      where,
      message: "a unit directory with no content is a stub; delete it or give it behaviour",
    });
    return;
  }
  if (!entries.some((entry) => entry.isFile() && entry.name === "package.json")) return;
  await checkWorkspaceManifest(root, where, name, seen, findings);
}

async function checkWorkspaceManifest(
  root: string,
  where: string,
  dirname: string,
  seen: Map<string, string>,
  findings: Finding[],
): Promise<void> {
  const manifestPath = `${where}/package.json`;
  const manifest = await readJsonFile(root, manifestPath, findings);
  if (!manifest) return;

  const expected = `${workspaceScope}/${dirname}`;
  const name = manifest["name"];
  if (name !== expected) {
    findings.push({
      rule: "unit-name",
      where: manifestPath,
      message: `name must be "${expected}", found ${JSON.stringify(name ?? null)}`,
    });
  }
  if (typeof name === "string") {
    const previous = seen.get(name);
    if (previous) {
      findings.push({
        rule: "unit-name-unique",
        where: manifestPath,
        message: `name "${name}" is already used by ${previous}`,
      });
    }
    seen.set(name, manifestPath);
  }
  if (typeof manifest["private"] !== "boolean") {
    findings.push({
      rule: "unit-private",
      where: manifestPath,
      message: 'publication intent must be explicit: set "private" to true or false',
    });
  }
  if (manifest["type"] !== "module") {
    findings.push({
      rule: "unit-module-type",
      where: manifestPath,
      message: 'every workspace unit is ESM: set "type" to "module"',
    });
  }
}

async function readRootManifest(
  root: string,
  findings: Finding[],
): Promise<Record<string, unknown> | null> {
  return readJsonFile(root, "package.json", findings);
}

async function readJsonFile(
  root: string,
  where: string,
  findings: Finding[],
): Promise<Record<string, unknown> | null> {
  const raw = await readTextFile(join(root, where));
  if (raw === null) {
    findings.push({ rule: "manifest-missing", where, message: `${where} is missing` });
    return null;
  }
  try {
    const parsed: unknown = JSON.parse(raw);
    const record = asRecord(parsed);
    if (!record) {
      findings.push({ rule: "manifest-invalid", where, message: `${where} must be a JSON object` });
    }
    return record;
  } catch (error) {
    findings.push({
      rule: "manifest-invalid",
      where,
      message: `${where} is not valid JSON: ${(error as Error).message}`,
    });
    return null;
  }
}

function majorOf(version: string): number | null {
  const match = /(\d+)/.exec(version.trim());
  return match?.[1] ? Number(match[1]) : null;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function asStringArray(value: unknown): string[] | null {
  return Array.isArray(value) && value.every((item) => typeof item === "string")
    ? (value as string[])
    : null;
}

async function readSubdirectories(path: string): Promise<string[]> {
  const entries = (await readEntries(path)) ?? [];
  return entries
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith("."))
    .map((entry) => entry.name)
    .sort();
}

async function readEntries(path: string) {
  try {
    return await readdir(path, { withFileTypes: true });
  } catch {
    return null;
  }
}

async function readTextFile(path: string): Promise<string | null> {
  try {
    return await readFile(path, "utf8");
  } catch {
    return null;
  }
}

async function isDirectory(path: string): Promise<boolean> {
  try {
    return (await stat(path)).isDirectory();
  } catch {
    return false;
  }
}

async function isFile(path: string): Promise<boolean> {
  try {
    return (await stat(path)).isFile();
  } catch {
    return false;
  }
}
