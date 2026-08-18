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

/**
 * Two catalogues that must never meet in one file.
 *
 * Unlike an `ImportRule`, neither side is forbidden on its own: what is forbidden is seeing both.
 * Rule 7 exists because the danger it names — a conversion between the two objection families —
 * cannot live in either domain crate (rule 6 already forbids that), only in a third file that
 * imports both.
 */
export type NoCoImportRule = {
  readonly kind: "no-co-import";
  readonly id: string;
  readonly claudeMd: number;
  readonly statement: string;
  readonly scope: readonly string[];
  readonly except: readonly string[];
  /** Exactly two catalogue names. */
  readonly families: readonly [string, string];
};

export type EmacsRule = {
  readonly kind: "emacs-isolation";
  readonly id: string;
  readonly claudeMd: number;
  readonly statement: string;
  /** Repo-relative directory of the Emacs unit. */
  readonly unit: string;
};

export type Rule = ImportRule | NoCoImportRule | EmacsRule;

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

/** The patterns of one named catalogue. */
export function catalogue(contract: Contract, name: string): string[] {
  return [...(contract.catalogues.get(name) ?? [])];
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
  if (kind === "no-co-import") {
    const families = strings(source["families"], `${where}.families`);
    if (families.length !== 2 || !families[0] || !families[1]) {
      throw new Error(`${where}.families must name exactly two catalogues`);
    }
    return {
      kind,
      ...common,
      scope: strings(source["scope"], `${where}.scope`),
      except: source["except"] === undefined ? [] : strings(source["except"], `${where}.except`),
      families: [families[0], families[1]],
    };
  }
  if (kind === "emacs-isolation") {
    return { kind, ...common, unit: string(source["unit"], `${where}.unit`) };
  }
  throw new Error(`${where}.kind: unknown rule kind ${JSON.stringify(kind)}`);
}

function assertCataloguesResolve(contract: Contract, path: string): void {
  for (const rule of contract.rules) {
    const named =
      rule.kind === "imports" ? rule.deny : rule.kind === "no-co-import" ? rule.families : [];
    for (const name of named) {
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
