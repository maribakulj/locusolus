import { readFile } from "node:fs/promises";

/**
 * `boundaries.json` — the opposable form of the five rules of `CLAUDE.md`, section
 * « Frontières vérifiées par la CI ».
 *
 * A malformed contract throws rather than reporting a finding: a guard that cannot read its own
 * rules must stop the build, not quietly enforce a subset of them.
 */

export type ImportRule = {
  readonly kind: "imports";
  readonly id: string;
  readonly claudeMd: number;
  readonly statement: string;
  /** Paths the rule watches. */
  readonly scope: readonly string[];
  /** Paths carved out of `scope` — the places the rule explicitly allows. */
  readonly except: readonly string[];
  /** Catalogue names whose specifiers are forbidden inside the scope. */
  readonly deny: readonly string[];
};

export type EmacsRule = {
  readonly kind: "emacs-isolation";
  readonly id: string;
  readonly claudeMd: number;
  readonly statement: string;
  /** Repo-relative directory of the Emacs unit. */
  readonly unit: string;
};

export type Rule = ImportRule | EmacsRule;

export type Contract = {
  readonly exclude: readonly string[];
  readonly analysable: ReadonlySet<string>;
  readonly ignored: ReadonlySet<string>;
  readonly catalogues: ReadonlyMap<string, readonly string[]>;
  readonly rules: readonly Rule[];
};

export async function loadContract(path: string): Promise<Contract> {
  const source: unknown = JSON.parse(await readFile(path, "utf8"));
  const root = object(source, path);
  const extensions = object(root["extensions"], `${path}: extensions`);
  const catalogues = readCatalogues(object(root["catalogues"], `${path}: catalogues`), path);
  const rules = array(root["rules"], `${path}: rules`).map((entry, index) =>
    readRule(object(entry, `${path}: rules[${index}]`), `${path}: rules[${index}]`),
  );
  const contract: Contract = {
    exclude: strings(root["exclude"], `${path}: exclude`),
    analysable: new Set(strings(extensions["analysable"], `${path}: extensions.analysable`)),
    ignored: new Set(strings(extensions["ignored"], `${path}: extensions.ignored`)),
    catalogues,
    rules,
  };
  assertCataloguesResolve(contract, path);
  return contract;
}

/** Every specifier pattern a rule forbids, catalogues already resolved. */
export function denied(contract: Contract, rule: ImportRule): string[] {
  return rule.deny.flatMap((name) => [...(contract.catalogues.get(name) ?? [])]);
}

function readCatalogues(source: Record<string, unknown>, path: string): Map<string, string[]> {
  const catalogues = new Map<string, string[]>();
  for (const [name, value] of Object.entries(source)) {
    if (name.startsWith("$")) continue;
    const entry = object(value, `${path}: catalogues.${name}`);
    catalogues.set(name, strings(entry["patterns"], `${path}: catalogues.${name}.patterns`));
  }
  return catalogues;
}

function readRule(source: Record<string, unknown>, where: string): Rule {
  const id = string(source["id"], `${where}.id`);
  const claudeMd = source["claudeMd"];
  if (typeof claudeMd !== "number") throw new Error(`${where}.claudeMd must be a number`);
  const common = { id, claudeMd, statement: string(source["statement"], `${where}.statement`) };
  const kind = string(source["kind"], `${where}.kind`);
  if (kind === "imports") {
    return {
      kind,
      ...common,
      scope: strings(source["scope"], `${where}.scope`),
      except: source["except"] === undefined ? [] : strings(source["except"], `${where}.except`),
      deny: strings(source["deny"], `${where}.deny`),
    };
  }
  if (kind === "emacs-isolation") {
    return { kind, ...common, unit: string(source["unit"], `${where}.unit`) };
  }
  throw new Error(`${where}.kind: unknown rule kind ${JSON.stringify(kind)}`);
}

function assertCataloguesResolve(contract: Contract, path: string): void {
  for (const rule of contract.rules) {
    if (rule.kind !== "imports") continue;
    for (const name of rule.deny) {
      if (!contract.catalogues.has(name)) {
        throw new Error(
          `${path}: rule ${rule.id} denies unknown catalogue ${JSON.stringify(name)}`,
        );
      }
    }
  }
}

function object(value: unknown, where: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${where} must be an object`);
  }
  return value as Record<string, unknown>;
}

function array(value: unknown, where: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`${where} must be an array`);
  return value;
}

function string(value: unknown, where: string): string {
  if (typeof value !== "string") throw new Error(`${where} must be a string`);
  return value;
}

function strings(value: unknown, where: string): string[] {
  return array(value, where).map((entry, index) => string(entry, `${where}[${index}]`));
}
