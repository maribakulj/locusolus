import { type Model, type Struct, type Type } from "./ir.ts";

const HEADER = `// Généré depuis schemas/ par tooling/sdk/generate.ts — ne pas éditer à la main.
//
// \`npm run check:generated\` régénère et compare : une retouche manuelle fait échouer la CI.
// Ce qui doit changer, ce sont les schémas ; ils sont le contrat, ceci n'en est qu'une lecture.

// Deux dérogations, et elles portent sur du code généré, pas écrit.
//
// \`missing_docs\` : la documentation de ces types EST la description de leur schéma. Un champ dont
// le schéma ne dit rien n'a rien à dire, et inventer une phrase pour satisfaire le lint ajouterait
// du bruit là où le silence est exact. Ce qui manque doit être ajouté au schéma, pas ici.
//
// \`doc_markdown\` : les descriptions sont de la prose française qui cite des identifiants sans les
// mettre entre accents graves. Les réécrire pour le lint reviendrait à éditer le schéma depuis le
// générateur.
#![allow(missing_docs, clippy::doc_markdown)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
`;

/**
 * Rust types that serde round-trips back to the same JSON.
 *
 * Two choices carry that promise. Optional fields are `Option<T>` with `skip_serializing_if`, so
 * a field absent on the way in stays absent on the way out rather than reappearing as `null`. And
 * maps are `BTreeMap`, not `HashMap`: a hash map re-serialises in whatever order it feels like,
 * which would make a byte-comparison of a round-trip fail for no reason at all.
 */
export function emitRust(model: Model): string {
  const parts = [HEADER];
  for (const alias of model.aliases) {
    if (alias.type.kind === "enum") {
      parts.push(emitEnum(alias.name, alias.doc, alias.type.values));
      continue;
    }
    parts.push(`${doc(alias.doc, "")}pub type ${alias.name} = ${rustType(alias.type)};\n`);
  }
  for (const struct of model.structs) parts.push(emitStruct(struct));
  parts.push(
    `${doc("Les documents qu'un pair peut envoyer ou recevoir, dans l'ordre du registre.", "")}` +
      `pub const LEP_DOCUMENTS: [&str; ${model.documents.length}] = [\n` +
      model.documents.map((name) => `    ${JSON.stringify(name)},`).join("\n") +
      "\n];\n",
    `${doc("Les features négociables au handshake, avec le mineur qui les introduit.", "")}` +
      `pub const LEP_FEATURES: [(&str, &str); ${model.features.length}] = [\n` +
      model.features
        .map(
          (feature) => `    (${JSON.stringify(feature.name)}, ${JSON.stringify(feature.since)}),`,
        )
        .join("\n") +
      "\n];\n",
  );
  return parts.join("\n");
}

function emitStruct(struct: Struct): string {
  const lines = [
    doc(struct.doc, ""),
    "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]",
    `pub struct ${struct.name} {`,
  ];
  for (const field of struct.fields) {
    const rendered = rustType(field.type);
    const name = snakeIdentifier(field.name);
    if (name !== field.name) lines.push(`    #[serde(rename = ${JSON.stringify(field.name)})]`);
    if (!field.required) {
      lines.push(`    #[serde(skip_serializing_if = "Option::is_none", default)]`);
    }
    lines.push(doc(field.doc, "    ").trimEnd());
    lines.push(`    pub ${name}: ${field.required ? rendered : `Option<${rendered}>`},`);
  }
  lines.push("}\n");
  return lines.filter((line) => line.length > 0).join("\n");
}

function emitEnum(name: string, documentation: string | undefined, values: readonly string[]) {
  const lines = [
    doc(documentation, ""),
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]",
    `pub enum ${name} {`,
  ];
  for (const value of values) {
    const variant = variantName(value);
    if (variant !== value) lines.push(`    #[serde(rename = ${JSON.stringify(value)})]`);
    lines.push(`    ${variant},`);
  }
  lines.push("}\n");
  return lines.filter((line) => line.length > 0).join("\n");
}

function rustType(type: Type): string {
  switch (type.kind) {
    case "string":
    case "timestamp":
      return "String";
    case "integer":
      return "i64";
    case "number":
      return "f64";
    case "boolean":
      return "bool";
    case "enum":
      // An inline enum has no name of its own; the wire value is what matters and a string keeps
      // the round-trip exact. Named enumerations come through the vocabulary as aliases.
      return "String";
    case "array":
      return `Vec<${rustType(type.items)}>`;
    case "map":
      return `BTreeMap<String, ${rustType(type.values)}>`;
    case "ref":
    case "object":
      return type.name;
    case "unknown":
      return "serde_json::Value";
  }
}

/** `S1` → `S1`; `connector-only` → `ConnectorOnly`; `sha256` stays a pattern, never an enum. */
function variantName(value: string): string {
  return value
    .split(/[-_.]/)
    .filter((part) => part.length > 0)
    .map((part) => part[0]!.toUpperCase() + part.slice(1))
    .join("");
}

/** Rust identifiers are snake_case already in these schemas; only keywords need escaping. */
function snakeIdentifier(name: string): string {
  const keywords = new Set(["type", "ref", "match", "move", "box", "final", "override", "self"]);
  return keywords.has(name) ? `r#${name}` : name;
}

function doc(text: string | undefined, indent: string): string {
  if (!text) return "";
  return `${text
    .split("\n")
    .map((line) => `${indent}/// ${line}`.trimEnd())
    .join("\n")}\n`;
}
