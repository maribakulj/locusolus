import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
// ajv ships CommonJS: the class is a named export both at runtime and in its types, while
// ajv-formats only exposes a default. Importing each the way it is actually published keeps
// `verbatimModuleSyntax` honest instead of papering over it with a cast.
import { Ajv, type AnySchemaObject, type ValidateFunction } from "ajv";
import * as ajvFormats from "ajv-formats";

// ajv-formats publishes CommonJS with a default export. Without `esModuleInterop` — which this
// repository deliberately does not enable, and turning on would change how every future import
// type-checks — TypeScript models that default as the module object rather than the callable it
// is at runtime. The assertion is confined to this line, and the test "une date mal formée est
// refusée" is what proves the call took effect rather than being assumed.
const addFormats = ajvFormats.default as unknown as (ajv: Ajv, formats: string[]) => void;
import type { Finding } from "../lib/findings.ts";

/**
 * The outcome vocabulary lives in the registry, not here.
 *
 * Most outcomes are *semantic* — what admission or the scheduler decides — and all of those are
 * schema-valid: a mission that admission will refuse is still a well-formed mission, and
 * conflating "the scheduler says no" with "this is not a MissionEnvelope" would make the refusal
 * fixture unable to test what it exists to test. Only `invalid` claims the document itself is
 * malformed.
 *
 * Keeping the list as data means adding a scenario outcome is a registry edit that documents what
 * the word means, rather than a constant nobody explains.
 */
export type Expectation = {
  readonly schema: "valid" | "invalid";
  readonly note: string;
};

export type Registry = {
  readonly draft: string;
  readonly expectations: Readonly<Record<string, Expectation>>;
  readonly documents: readonly { readonly schema: string; readonly examples: readonly string[] }[];
  readonly shared: readonly string[];
  readonly pending: readonly {
    readonly example: string;
    readonly covered_by: string;
    readonly reason: string;
  }[];
};

export function readRegistry(schemasDir: string): Registry {
  return JSON.parse(readFileSync(join(schemasDir, "registry.json"), "utf8")) as Registry;
}

/**
 * Build an Ajv instance holding every schema the registry names, keyed by its `$id`.
 *
 * `strict` stays on: a typo in a keyword is silently ignored by a permissive validator, which is
 * the failure mode where a schema looks like it constrains something and does not.
 */
export function compile(
  schemasDir: string,
  registry: Registry,
): { readonly ajv: Ajv; readonly findings: Finding[] } {
  // `strict` stays on so a mistyped keyword is an error rather than a keyword silently ignored.
  // `strictRequired` is the one exception: it wants every `required` name declared in the same
  // subschema, which no `if`/`then` conditional can satisfy — the property is declared in the
  // parent. Keeping it on would forbid conditional requirements outright.
  const ajv = new Ajv({
    strict: true,
    strictRequired: false,
    allErrors: true,
    allowUnionTypes: true,
  });
  // `format` is annotation-only in draft-07 unless a validator asserts it. On a wire contract a
  // date that is not a date is a defect, so it is asserted.
  addFormats(ajv, ["date-time", "uri", "email"]);
  const findings: Finding[] = [];
  // A schema can legitimately be both: referenced by others AND validated against its own
  // examples. Registering it twice is what Ajv refuses, not declaring it in both lists — so the
  // duplicate is dropped here rather than forbidden in the registry, where forbidding it would
  // mean choosing between "other schemas may reference it" and "its examples are checked".
  const files = [
    ...new Set([...registry.shared, ...registry.documents.map((entry) => entry.schema)]),
  ];
  for (const file of files) {
    const where = `schemas/${file}`;
    let schema: AnySchemaObject;
    try {
      schema = JSON.parse(readFileSync(join(schemasDir, file), "utf8")) as AnySchemaObject;
    } catch (error) {
      findings.push({ rule: "schema-unreadable", where, message: String(error) });
      continue;
    }
    if (schema.$schema !== registry.draft) {
      findings.push({
        rule: "schema-draft",
        where,
        message: `déclare ${String(schema.$schema)}, le registre fixe ${registry.draft}`,
      });
    }
    try {
      ajv.addSchema(schema);
    } catch (error) {
      findings.push({ rule: "schema-invalid", where, message: String(error) });
    }
  }
  return { ajv, findings };
}

/** Strip the fixture block; it is test metadata, never part of the LEP document. */
export function stripFixture(document: unknown): {
  readonly body: unknown;
  readonly expect: string | undefined;
} {
  if (typeof document !== "object" || document === null || Array.isArray(document)) {
    return { body: document, expect: undefined };
  }
  const { _fixture, ...body } = document as Record<string, unknown> & { _fixture?: unknown };
  const expect =
    typeof _fixture === "object" && _fixture !== null
      ? (_fixture as Record<string, unknown>).expect
      : undefined;
  return { body, expect: typeof expect === "string" ? expect : undefined };
}

/**
 * Validate every example the registry maps, and account for every example it does not.
 *
 * An example that is neither validated nor declared pending is itself a finding: a fixture nobody
 * checks looks exactly like a fixture that passes.
 */
export function inspectSchemas(root: string): Finding[] {
  const schemasDir = join(root, "schemas");
  const examplesDir = join(schemasDir, "examples");
  const registry = readRegistry(schemasDir);
  const { ajv, findings } = compile(schemasDir, registry);
  if (findings.length > 0) return findings;

  const seen = new Set<string>();
  for (const entry of registry.documents) {
    const validate = validatorFor(ajv, schemasDir, entry.schema, findings);
    if (!validate) continue;
    for (const example of entry.examples) {
      seen.add(example);
      findings.push(
        ...checkExample(examplesDir, example, entry.schema, validate, registry.expectations),
      );
    }
  }
  for (const entry of registry.pending) seen.add(entry.example);

  for (const file of readdirSync(examplesDir)
    .filter((name) => name.endsWith(".json"))
    .sort()) {
    if (seen.has(file)) continue;
    findings.push({
      rule: "example-unaccounted",
      where: `schemas/examples/${file}`,
      message: "ni validé par un schéma ni déclaré `pending` dans schemas/registry.json",
    });
  }
  return findings;
}

function validatorFor(
  ajv: Ajv,
  schemasDir: string,
  file: string,
  findings: Finding[],
): ValidateFunction | undefined {
  const schema = JSON.parse(readFileSync(join(schemasDir, file), "utf8")) as AnySchemaObject;
  // Ajv compiles lazily, so a schema that parses can still throw here.
  try {
    const validate = ajv.getSchema(String(schema.$id));
    if (validate) return validate as ValidateFunction;
    findings.push({
      rule: "schema-missing-id",
      where: `schemas/${file}`,
      message: `aucun schéma enregistré sous ${String(schema.$id)}`,
    });
  } catch (error) {
    findings.push({
      rule: "schema-uncompilable",
      where: `schemas/${file}`,
      message: String(error),
    });
  }
  return undefined;
}

function checkExample(
  examplesDir: string,
  example: string,
  schema: string,
  validate: ValidateFunction,
  expectations: Readonly<Record<string, Expectation>>,
): Finding[] {
  const where = `schemas/examples/${example}`;
  let document: unknown;
  try {
    document = JSON.parse(readFileSync(join(examplesDir, example), "utf8"));
  } catch (error) {
    return [{ rule: "example-unreadable", where, message: String(error) }];
  }
  const { body, expect } = stripFixture(document);
  if (expect === undefined) {
    return [
      {
        rule: "example-without-expect",
        where,
        message: "aucun `_fixture.expect` : ce que l'exemple démontre doit être écrit, pas déduit",
      },
    ];
  }
  const expectation = expectations[expect];
  if (!expectation) {
    return [
      {
        rule: "example-unknown-expect",
        where,
        message: `\`expect\` vaut ${expect}, attendu l'un de ${Object.keys(expectations).sort().join(", ")} — un résultat neuf s'ajoute au registre, avec ce qu'il veut dire`,
      },
    ];
  }
  const wantsValid = expectation.schema === "valid";
  const ok = validate(body) as boolean;
  if (ok === wantsValid) return [];
  if (wantsValid) {
    const detail = (validate.errors ?? [])
      .map((error) => `${error.instancePath || "/"} ${error.message ?? ""}`.trim())
      .join(" ; ");
    return [
      {
        rule: "example-invalid",
        where,
        message: `refusé par ${schema} alors que \`expect\` vaut ${expect} — ${detail}`,
      },
    ];
  }
  return [
    {
      rule: "example-unexpectedly-valid",
      where,
      message: `accepté par ${schema} alors que \`expect\` vaut ${expect}`,
    },
  ];
}
