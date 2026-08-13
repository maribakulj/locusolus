import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Finding } from "../lib/findings.ts";
import { readRegistry, type Registry } from "../schemas/validate.ts";

/**
 * The shape a generated SDK needs, read from the schemas and nothing else.
 *
 * The intermediate representation exists so one reading of the schemas feeds both emitters. Two
 * readers would drift, and the drift would show up as a TypeScript client and a Rust server that
 * disagree about the wire — the exact failure `docs/06` invents inter-SDK fixtures to catch.
 *
 * It is deliberately small. Every JSON Schema construct it does not model is a *finding*, never a
 * silent omission: a generator that skips what it does not understand produces types that look
 * complete and are not, and the first person to notice is whoever debugs a field that vanished.
 */
export type Type =
  | { readonly kind: "string" }
  | { readonly kind: "integer" }
  | { readonly kind: "number" }
  | { readonly kind: "boolean" }
  | { readonly kind: "timestamp" }
  | { readonly kind: "enum"; readonly values: readonly string[] }
  | { readonly kind: "array"; readonly items: Type }
  | { readonly kind: "map"; readonly values: Type }
  | { readonly kind: "ref"; readonly name: string }
  | { readonly kind: "object"; readonly name: string }
  | { readonly kind: "unknown" };

export type Field = {
  readonly name: string;
  readonly type: Type;
  readonly required: boolean;
  readonly doc: string | undefined;
};

export type Struct = {
  readonly name: string;
  readonly doc: string | undefined;
  readonly fields: readonly Field[];
};

export type Alias = {
  readonly name: string;
  readonly doc: string | undefined;
  readonly type: Type;
};

export type Feature = {
  readonly name: string;
  readonly since: string;
  readonly note: string;
};

export type Model = {
  readonly structs: readonly Struct[];
  readonly aliases: readonly Alias[];
  /** Documents a peer can send or receive, in the order the registry lists them. */
  readonly documents: readonly string[];
  /** Negotiable features, from schemas/lep/1.0/features.json. */
  readonly features: readonly Feature[];
};

/** `mission-envelope` → `MissionEnvelope`; `lep/1.0/lease.schema.json` → `Lease`. */
export function typeName(source: string): string {
  const base = source
    .replace(/^.*\//, "")
    .replace(/\.schema\.json$/, "")
    .replace(/^\d+\.\d+$/, "");
  return base
    .split(/[-_]/)
    .filter((part) => part.length > 0)
    .map((part) => part[0]!.toUpperCase() + part.slice(1))
    .join("");
}

/** `max_model_calls` → `maxModelCalls`. The wire name is preserved separately. */
export function camel(name: string): string {
  return name.replace(/_([a-z0-9])/g, (_, char: string) => char.toUpperCase());
}

export function buildModel(schemasDir: string): { model: Model; findings: Finding[] } {
  const registry = readRegistry(schemasDir);
  const findings: Finding[] = [];
  const structs: Struct[] = [];
  const aliases: Alias[] = [];
  const documents: string[] = [];

  for (const file of schemaFiles(registry)) {
    const schema = JSON.parse(readFileSync(join(schemasDir, file), "utf8")) as Record<
      string,
      unknown
    >;
    const name = typeName(file);
    // A schema's own `definitions` are collected too, not just the vocabulary's: `$ref` targets
    // like `#/definitions/refs` are local, and a generator that only knew the shared ones would
    // emit a type name nothing defines.
    collectDefinitions(file, schema, aliases, structs, findings);
    if (file.endsWith("vocabulary.schema.json")) continue;
    if (registry.documents.some((entry) => entry.schema === file)) documents.push(name);
    collectStruct(file, name, schema, structs, findings, docOf(schema));
  }
  const features = JSON.parse(readFileSync(join(schemasDir, "lep/1.0/features.json"), "utf8")) as {
    features: Feature[];
  };
  return { model: { structs, aliases, documents, features: features.features }, findings };
}

function schemaFiles(registry: Registry): string[] {
  // Shared first so the vocabulary aliases exist before anything refers to them.
  return [...registry.shared, ...registry.documents.map((entry) => entry.schema)];
}

function docOf(schema: Record<string, unknown>): string | undefined {
  const description = schema["description"];
  return typeof description === "string" ? description : undefined;
}

function collectDefinitions(
  file: string,
  schema: Record<string, unknown>,
  aliases: Alias[],
  structs: Struct[],
  findings: Finding[],
): void {
  const definitions = schema["definitions"];
  if (typeof definitions !== "object" || definitions === null) return;
  for (const [key, value] of Object.entries(definitions as Record<string, unknown>)) {
    const definition = value as Record<string, unknown>;
    const name = typeName(key);
    // Definitions from different files share one namespace in the generated code. A collision
    // would silently overwrite one of them, so it is a finding rather than a last-write-wins.
    if (aliases.some((alias) => alias.name === name) || structs.some((s) => s.name === name)) {
      findings.push({
        rule: "sdk-duplicate-definition",
        where: `schemas/${file} — definitions/${key}`,
        message: `${name} est déjà défini ailleurs : deux définitions homonymes se recouvriraient`,
      });
      continue;
    }
    aliases.push({
      name,
      doc: docOf(definition),
      type: resolve(file, `definitions/${key}`, definition, structs, findings, name),
    });
  }
}

function collectStruct(
  file: string,
  name: string,
  schema: Record<string, unknown>,
  structs: Struct[],
  findings: Finding[],
  doc: string | undefined,
): void {
  const properties = schema["properties"];
  if (typeof properties !== "object" || properties === null) {
    findings.push({
      rule: "sdk-not-an-object",
      where: `schemas/${file}`,
      message: "un document sans `properties` n'a pas de type à générer",
    });
    return;
  }
  const required = new Set(
    Array.isArray(schema["required"]) ? (schema["required"] as string[]) : [],
  );
  const fields: Field[] = [];
  for (const [key, value] of Object.entries(properties as Record<string, unknown>)) {
    const property = value as Record<string, unknown>;
    fields.push({
      name: key,
      type: resolve(file, `${name}.${key}`, property, structs, findings, `${name}${typeName(key)}`),
      required: required.has(key),
      doc: docOf(property),
    });
  }
  structs.push({ name, doc, fields });
}

/**
 * Turn one schema node into a type, minting a nested struct when the node is an inline object.
 *
 * `nested` is the name a minted struct receives. Inline objects are common in these schemas —
 * `MissionEnvelope.objective`, `CapabilityManifest.platform` — and naming them after their owner
 * keeps the generated code readable without asking the schemas to be restructured.
 */
function resolve(
  file: string,
  where: string,
  node: Record<string, unknown>,
  structs: Struct[],
  findings: Finding[],
  nested?: string,
): Type {
  const ref = node["$ref"];
  if (typeof ref === "string") return { kind: "ref", name: refName(ref) };

  // The content-hash alias is a oneOf of patterns; every other oneOf would need a union type,
  // which nothing in these schemas uses yet.
  if (Array.isArray(node["oneOf"])) {
    const branches = node["oneOf"] as Record<string, unknown>[];
    if (branches.every((branch) => typeof branch["pattern"] === "string"))
      return { kind: "string" };
    findings.push({
      rule: "sdk-unsupported-oneof",
      where: `schemas/${file} — ${where}`,
      message: "un `oneOf` qui n'est pas un choix de motifs demanderait un type union",
    });
    return { kind: "unknown" };
  }

  const enumeration = node["enum"];
  if (Array.isArray(enumeration)) {
    if (enumeration.every((value) => typeof value === "string")) {
      return { kind: "enum", values: enumeration as string[] };
    }
    findings.push({
      rule: "sdk-unsupported-enum",
      where: `schemas/${file} — ${where}`,
      message: "une énumération non textuelle n'a pas de forme générée",
    });
    return { kind: "unknown" };
  }

  switch (node["type"]) {
    case "string":
      return node["format"] === "date-time" ? { kind: "timestamp" } : { kind: "string" };
    case "integer":
      return { kind: "integer" };
    case "number":
      return { kind: "number" };
    case "boolean":
      return { kind: "boolean" };
    case "array": {
      const items = node["items"];
      if (typeof items !== "object" || items === null) {
        findings.push({
          rule: "sdk-array-without-items",
          where: `schemas/${file} — ${where}`,
          message: "un tableau sans `items` n'a pas de type d'élément",
        });
        return { kind: "unknown" };
      }
      return {
        kind: "array",
        items: resolve(
          file,
          `${where}[]`,
          items as Record<string, unknown>,
          structs,
          findings,
          nested ? `${nested}Item` : undefined,
        ),
      };
    }
    case "object": {
      const additional = node["additionalProperties"];
      if (typeof additional === "object" && additional !== null) {
        return {
          kind: "map",
          values: resolve(
            file,
            `${where}{}`,
            additional as Record<string, unknown>,
            structs,
            findings,
          ),
        };
      }
      if (typeof node["properties"] !== "object" || node["properties"] === null) {
        // `payload: { type: "object" }` — deliberately opaque on the wire.
        return { kind: "unknown" };
      }
      if (!nested) {
        findings.push({
          rule: "sdk-anonymous-object",
          where: `schemas/${file} — ${where}`,
          message: "un objet imbriqué sans nom dérivable ne peut pas être généré",
        });
        return { kind: "unknown" };
      }
      collectStruct(file, nested, node, structs, findings, docOf(node));
      return { kind: "object", name: nested };
    }
    default:
      return { kind: "unknown" };
  }
}

/** `urn:locus:schema:lep:1.0:vocabulary#/definitions/sandbox_level` → `SandboxLevel`. */
function refName(ref: string): string {
  const fragment = ref.split("#/definitions/")[1];
  if (fragment) return typeName(fragment);
  return typeName(ref.split(":").pop() ?? ref);
}
