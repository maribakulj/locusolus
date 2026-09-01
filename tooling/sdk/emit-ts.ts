import { type Field, type Model, type Struct, type Type, type Union } from "./ir.ts";

const HEADER = `// Généré depuis schemas/ par tooling/sdk/generate.ts — ne pas éditer à la main.
//
// \`npm run check:generated\` régénère et compare : une retouche manuelle fait échouer la CI.
// Ce qui doit changer, ce sont les schémas ; ils sont le contrat, ceci n'en est qu'une lecture.
`;

/** Every field is readonly: a decoded LEP document is a record of what arrived, not a workspace. */
export function emitTypeScript(model: Model): string {
  const parts = [HEADER];
  for (const alias of model.aliases) {
    parts.push(doc(alias.doc, ""), `export type ${alias.name} = ${tsType(alias.type)};\n`);
  }
  for (const union of model.unions) parts.push(emitUnion(union));
  for (const struct of model.structs) parts.push(emitStruct(struct));
  parts.push(
    doc("Les documents qu'un pair peut envoyer ou recevoir, dans l'ordre du registre.", ""),
    `export const LEP_DOCUMENTS = [\n${model.documents.map((name) => `  ${JSON.stringify(name)},`).join("\n")}\n] as const;\n`,
    `export type LepDocument = (typeof LEP_DOCUMENTS)[number];\n`,
    doc(
      "Les features négociables au handshake. `since` est le mineur qui introduit la feature :\nun pair plus ancien la refuse au lieu de l'accepter sans savoir la tenir.",
      "",
    ),
    `export const LEP_FEATURES = {\n` +
      model.features
        .map(
          (feature) =>
            `${doc(feature.note, "  ")}\n  ${JSON.stringify(feature.name)}: ${JSON.stringify(feature.since)},`,
        )
        .join("\n") +
      `\n} as const;\n`,
    `export type LepFeature = keyof typeof LEP_FEATURES;\n`,
    doc(
      "Les mécanismes de confinement dont ce dépôt sait ce qu'ils désignent — registre\n" +
        "`schemas/lep/1.0/mechanisms.json`, ADR 0035 décision 3. Ce n'est pas une énumération du\n" +
        "fil : `backend` reste une chaîne libre dans les deux schémas qui le portent, et un nom\n" +
        "absent d'ici n'est pas invalide — il est non rapproché, ce qui est un verdict différent de\n" +
        "« ce n'est pas le même mécanisme ».",
      "",
    ),
    `export const LEP_MECHANISMS = [\n${model.mechanisms.map((name) => `  ${JSON.stringify(name)},`).join("\n")}\n] as const;\n`,
    `export type LepMechanism = (typeof LEP_MECHANISMS)[number];\n`,
  );
  return parts.join("\n");
}

/**
 * Une union étiquetée, en union discriminée TypeScript.
 *
 * L'étiquette revient ici comme **type littéral**, là où l'émetteur Rust la sort de la variante :
 * c'est elle qui permet à `switch (reason.code)` de rétrécir le type, et sans elle le lecteur
 * devrait tester la présence des champs pour deviner de quelle forme il tient une valeur. Les deux
 * langages disent la même chose du fil et l'écrivent chacun à sa manière — c'est ce que l'IR permet
 * en gardant les champs des variantes plutôt qu'un nom de structure partagée.
 */
function emitUnion(union: Union): string {
  const lines = [doc(union.doc, ""), `export type ${union.name} =`];
  for (const variant of union.variants) {
    const fields = [
      `readonly ${union.tag}: ${JSON.stringify(variant.tag)};`,
      ...variant.fields.map((field) => fieldSignature(field)),
    ];
    lines.push(doc(variant.doc, "  "), `  | { ${fields.join(" ")} }`);
  }
  return `${lines.filter((line) => line.length > 0).join("\n")};\n`;
}

function emitStruct(struct: Struct): string {
  const lines = [doc(struct.doc, ""), `export type ${struct.name} = {`];
  for (const field of struct.fields) {
    lines.push(doc(field.doc, "  "), `  ${fieldSignature(field)}`);
  }
  lines.push("};\n");
  return lines.filter((line) => line.length > 0).join("\n");
}

/**
 * Optional fields are `?:` *and* `| undefined` — `exactOptionalPropertyTypes` is on in this
 * repository, so the two say different things and a decoded document needs both to be assignable.
 */
function fieldSignature(field: Field): string {
  const key = /^[A-Za-z_$][\w$]*$/.test(field.name) ? field.name : JSON.stringify(field.name);
  return field.required
    ? `readonly ${key}: ${tsType(field.type)};`
    : `readonly ${key}?: ${tsType(field.type)} | undefined;`;
}

function tsType(type: Type): string {
  switch (type.kind) {
    case "string":
      return "string";
    case "timestamp":
      // A canonical ISO-8601 instant. It stays a string here: parsing it into a Date would lose
      // the exact spelling, and `packages/protocol` refuses anything but the canonical one.
      return "string";
    case "integer":
    case "number":
      return "number";
    case "boolean":
      return "boolean";
    case "enum":
      return type.values.map((value) => JSON.stringify(value)).join(" | ");
    case "array":
      return `readonly ${wrap(type.items)}[]`;
    case "map":
      return `Readonly<Record<string, ${tsType(type.values)}>>`;
    case "ref":
    case "object":
    case "union":
      return type.name;
    case "unknown":
      return "unknown";
  }
}

/** Union and array element types need parentheses to read as one element type. */
function wrap(type: Type): string {
  const rendered = tsType(type);
  return type.kind === "enum" ? `(${rendered})` : rendered;
}

function doc(text: string | undefined, indent: string): string {
  if (!text) return "";
  const body = text
    .split("\n")
    .map((line) => `${indent} * ${line}`.trimEnd())
    .join("\n");
  return `${indent}/**\n${body}\n${indent} */`;
}
