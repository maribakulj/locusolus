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
  | { readonly kind: "union"; readonly name: string }
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

/**
 * Une union **discriminée** : plusieurs formes distinguées par la valeur d'une propriété commune.
 *
 * Le premier document qui en demande une est le refus d'admission (ADR 0017 §5.2) : sept motifs qui
 * ne portent pas les mêmes données — un niveau exigé et un meilleur niveau pour l'un, un genre
 * d'accélérateur pour l'autre, rien du tout pour un troisième. Un objet unique aux champs tous
 * facultatifs le dirait aussi mal qu'un `Value` non typé : le lecteur devrait deviner quelle
 * combinaison est licite, et le schéma cesserait de la lui dire.
 *
 * Les variantes portent leurs champs **en propre** plutôt que de renvoyer à des structures
 * nommées, et l'étiquette n'est **pas** dans ces champs. C'est ce qui permet aux deux émetteurs
 * d'être idiomatiques sans se contredire : Rust met l'étiquette dans `#[serde(tag)]` et n'en veut
 * pas dans la variante, TypeScript la remet comme type littéral parce que c'est **elle** qui
 * discrimine à la lecture. Une structure partagée aurait forcé l'un des deux à mentir.
 */
export type Variant = {
  /** La valeur de l'étiquette sur le fil — `"level_unavailable"`. */
  readonly tag: string;
  /** Le nom du constructeur — `LevelUnavailable`. */
  readonly name: string;
  readonly doc: string | undefined;
  readonly fields: readonly Field[];
};

export type Union = {
  readonly name: string;
  readonly doc: string | undefined;
  /** La propriété qui discrimine, sous son nom de fil — `code`. */
  readonly tag: string;
  readonly variants: readonly Variant[];
};

export type Feature = {
  readonly name: string;
  readonly since: string;
  readonly note: string;
};

export type Model = {
  readonly structs: readonly Struct[];
  readonly aliases: readonly Alias[];
  readonly unions: readonly Union[];
  /** Documents a peer can send or receive, in the order the registry lists them. */
  readonly documents: readonly string[];
  /** Negotiable features, from schemas/lep/1.0/features.json. */
  readonly features: readonly Feature[];
  /**
   * Known confinement mechanisms, from schemas/lep/1.0/mechanisms.json.
   *
   * Names only: what a consumer needs is « do I know this one », and the notes explaining each
   * choice belong to the register, which is the single place they are kept.
   */
  readonly mechanisms: readonly string[];
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
  const unions: Union[] = [];
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
    collectDefinitions(file, schema, aliases, structs, unions, findings);
    if (file.endsWith("vocabulary.schema.json")) continue;
    if (registry.documents.some((entry) => entry.schema === file)) documents.push(name);
    collectStruct(file, name, schema, structs, unions, findings, docOf(schema));
  }
  const features = JSON.parse(readFileSync(join(schemasDir, "lep/1.0/features.json"), "utf8")) as {
    features: Feature[];
  };
  const register = JSON.parse(
    readFileSync(join(schemasDir, "lep/1.0/mechanisms.json"), "utf8"),
  ) as { mechanisms: { name: string }[] };
  return {
    model: {
      structs,
      aliases,
      unions,
      documents,
      features: features.features,
      mechanisms: register.mechanisms.map((mechanism) => mechanism.name),
    },
    findings,
  };
}

function schemaFiles(registry: Registry): string[] {
  // Shared first so the vocabulary aliases exist before anything refers to them. Deduplicated
  // because a schema can be both: referenced by others AND validated against its own examples.
  // Emitting it twice would produce two definitions of the same struct, which does not compile —
  // a failure loud enough to find, but only after a regeneration nobody expected to break.
  return [...new Set([...registry.shared, ...registry.documents.map((entry) => entry.schema)])];
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
  unions: Union[],
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
    const type = resolve(file, `definitions/${key}`, definition, structs, unions, findings, name);
    // Une définition qui **est** une structure ou une union n'a pas d'alias : `resolve` l'a déjà
    // frappée sous ce nom, et l'aliaser produirait `pub type X = X;`, qui ne compile pas. Le défaut
    // dormait depuis W0.8 — aucune définition n'était un objet inline jusqu'ici.
    if ((type.kind === "object" || type.kind === "union") && type.name === name) continue;
    aliases.push({ name, doc: docOf(definition), type });
  }
}

function collectStruct(
  file: string,
  name: string,
  schema: Record<string, unknown>,
  structs: Struct[],
  unions: Union[],
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
      type: resolve(
        file,
        `${name}.${key}`,
        property,
        structs,
        unions,
        findings,
        `${name}${typeName(key)}`,
      ),
      required: required.has(key),
      doc: docOf(property),
    });
  }
  structs.push({ name, doc, fields });
}

/**
 * Lire un `oneOf` comme une union étiquetée, ou rendre `undefined` s'il n'en est pas une.
 *
 * Les conditions sont toutes nécessaires et se vérifient ensemble : chaque branche est un objet à
 * `properties`, **une même** propriété y est épinglée par un `const` textuel, et deux branches ne
 * partagent pas la même valeur. Relâcher la première laisserait passer un `$ref` dont l'étiquette
 * vit ailleurs ; relâcher la deuxième produirait une union qu'aucun lecteur ne sait discriminer ;
 * relâcher la troisième produirait deux variantes de même nom, et la seconde écraserait la première
 * à la génération.
 *
 * Le nom de la variante vient de la **valeur** de l'étiquette, pas d'un `title` : c'est elle qui
 * voyage, et deux noms qui divergeraient rendraient illisible un document lu à la main.
 */
function asTaggedUnion(
  file: string,
  where: string,
  branches: readonly Record<string, unknown>[],
  structs: Struct[],
  unions: Union[],
  findings: Finding[],
  nested: string | undefined,
): Type | undefined {
  if (branches.length === 0 || !nested) return undefined;

  // Une branche par `$ref` est refusée **par son nom**, parce que la réponse n'est pas « le
  // générateur ne sait pas » mais « écris la branche ici ». Une variante n'a pas d'existence hors
  // de son union : la nommer ailleurs produirait une structure autonome que personne n'instancie,
  // et deux endroits où corriger le jour où la forme change.
  if (branches.some((branch) => typeof branch["$ref"] === "string")) {
    findings.push({
      rule: "sdk-union-branch-by-ref",
      where: `schemas/${file} — ${where}`,
      message:
        "une branche d'union désignée par `$ref` : les écrire inline, une variante n'existe pas hors de son union",
    });
    return { kind: "unknown" };
  }

  const tag = discriminant(branches);
  if (!tag) return undefined;

  const variants: Variant[] = [];
  for (const branch of branches) {
    const properties = branch["properties"] as Record<string, Record<string, unknown>>;
    const value = properties[tag]?.["const"] as string;
    if (variants.some((variant) => variant.tag === value)) {
      findings.push({
        rule: "sdk-duplicate-variant",
        where: `schemas/${file} — ${where}`,
        message: `deux branches portent « ${value} » : la seconde écraserait la première`,
      });
      return { kind: "unknown" };
    }
    const required = new Set(
      Array.isArray(branch["required"]) ? (branch["required"] as string[]) : [],
    );
    const name = typeName(value);
    const fields: Field[] = [];
    for (const [key, property] of Object.entries(properties)) {
      // L'étiquette elle-même n'est pas un champ de la variante : Rust la met dans `#[serde(tag)]`
      // et TypeScript la remet comme type littéral. La garder ici la ferait écrire deux fois côté
      // Rust, et serde refuse.
      if (key === tag) continue;
      fields.push({
        name: key,
        type: resolve(
          file,
          `${where}/${value}.${key}`,
          property,
          structs,
          unions,
          findings,
          `${name}${typeName(key)}`,
        ),
        required: required.has(key),
        doc: docOf(property),
      });
    }
    variants.push({ tag: value, name, doc: docOf(branch), fields });
  }

  unions.push({ name: nested, doc: undefined, tag, variants });
  return { kind: "union", name: nested };
}

/**
 * La propriété que **toutes** les branches épinglent par un `const` textuel, s'il y en a une.
 *
 * Cherchée sur la première branche puis confirmée sur les autres, plutôt que devinée par un nom
 * conventionnel comme `type` ou `kind` : une convention se contredit le jour où un document choisit
 * un autre mot, et elle échoue alors en silence.
 */
function discriminant(branches: readonly Record<string, unknown>[]): string | undefined {
  const first = branches[0]?.["properties"];
  if (typeof first !== "object" || first === null) return undefined;
  for (const candidate of Object.keys(first)) {
    const pinned = branches.every((branch) => {
      const properties = branch["properties"];
      if (typeof properties !== "object" || properties === null) return false;
      const property = (properties as Record<string, unknown>)[candidate];
      if (typeof property !== "object" || property === null) return false;
      return typeof (property as Record<string, unknown>)["const"] === "string";
    });
    if (pinned) return candidate;
  }
  return undefined;
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
  unions: Union[],
  findings: Finding[],
  nested?: string,
): Type {
  const ref = node["$ref"];
  if (typeof ref === "string") return { kind: "ref", name: refName(ref) };

  // Trois sortes de `oneOf`, et deux seulement se modélisent. L'alias de content-hash est un choix
  // de motifs, donc une chaîne. Un `oneOf` dont chaque branche épingle la **même** propriété à une
  // constante est une union discriminée. Tout le reste reste un `finding` : un `oneOf` non
  // étiqueté ne se lit qu'en essayant les branches une à une, et un générateur qui rendrait un
  // type flou pour ça produirait des lecteurs qui devinent.
  if (Array.isArray(node["oneOf"])) {
    const branches = node["oneOf"] as Record<string, unknown>[];
    if (branches.every((branch) => typeof branch["pattern"] === "string"))
      return { kind: "string" };
    const union = asTaggedUnion(file, where, branches, structs, unions, findings, nested);
    if (union) return union;
    findings.push({
      rule: "sdk-unsupported-oneof",
      where: `schemas/${file} — ${where}`,
      message:
        "un `oneOf` qui n'est ni un choix de motifs ni une union étiquetée demanderait au lecteur d'essayer les branches",
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
          unions,
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
            unions,
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
      collectStruct(file, nested, node, structs, unions, findings, docOf(node));
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
