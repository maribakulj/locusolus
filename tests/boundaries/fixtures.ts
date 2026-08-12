import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

/** A deliberate violation, and the finding the guard owes us for it. */
export type Expectation = {
  readonly title: string;
  readonly violations: readonly { readonly rule: string; readonly where: string }[];
};

export type Fixture = {
  readonly name: string;
  readonly root: string;
  readonly expected: Expectation;
};

export const repoRoot = fileURLToPath(new URL("../..", import.meta.url));

export async function loadFixtures(family: string): Promise<Fixture[]> {
  const base = join(repoRoot, "tests", "boundaries", "fixtures", family);
  const entries = await readdir(base, { withFileTypes: true });
  const names = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();
  return Promise.all(
    names.map(async (name) => {
      const root = join(base, name);
      return { name, root, expected: await readExpectation(join(root, "expect.json")) };
    }),
  );
}

/** Compare findings by rule and location only — wording is free to change, the verdict is not. */
export function verdicts(
  entries: readonly { readonly rule: string; readonly where: string }[],
): string[] {
  return entries.map((entry) => `${entry.rule} @ ${entry.where}`).sort();
}

async function readExpectation(path: string): Promise<Expectation> {
  const parsed: unknown = JSON.parse(await readFile(path, "utf8"));
  if (!isRecord(parsed)) throw new Error(`${path}: expected a JSON object`);
  const title = parsed["title"];
  const violations = parsed["violations"];
  if (typeof title !== "string") throw new Error(`${path}: "title" must be a string`);
  if (!Array.isArray(violations)) throw new Error(`${path}: "violations" must be an array`);
  return { title, violations: violations.map((entry) => readViolation(path, entry)) };
}

function readViolation(path: string, entry: unknown): { rule: string; where: string } {
  if (!isRecord(entry)) throw new Error(`${path}: each violation must be an object`);
  const rule = entry["rule"];
  const where = entry["where"];
  if (typeof rule !== "string" || typeof where !== "string") {
    throw new Error(`${path}: each violation needs a string "rule" and "where"`);
  }
  return { rule, where };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
