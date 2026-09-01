import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";

import { buildModel } from "../../tooling/sdk/ir.ts";
import { emitRust } from "../../tooling/sdk/emit-rust.ts";
import { emitTypeScript } from "../../tooling/sdk/emit-ts.ts";

/**
 * **Les unions étiquetées, sur un répertoire de schémas jouet.**
 *
 * Le générateur se teste ici sans document réel, et c'est délibéré : le premier document qui
 * demande une union est le refus d'admission (ADR 0017 §5.2), et le faire entrer dans le même
 * sprint mêlerait un changement d'outillage à un changement de protocole. Un jouet dit la même
 * chose du générateur, et il dit **en plus** ce que les schémas réels ne contiennent pas encore —
 * un `oneOf` non étiqueté, deux branches homonymes.
 */

const scratch: string[] = [];

after(async () => {
  await Promise.all(scratch.map((path) => rm(path, { recursive: true, force: true })));
});

/** Un `oneOf` dont chaque branche épingle `code` : trois formes, trois jeux de champs. */
const TAGGED = {
  $schema: "http://json-schema.org/draft-07/schema#",
  $id: "urn:locus:schema:toy:1.0:refusal",
  type: "object",
  required: ["reason"],
  properties: { reason: { $ref: "#/definitions/reason" } },
  definitions: {
    reason: {
      oneOf: [
        {
          description: "L'hôte ne sait pas confiner aussi fort.",
          type: "object",
          required: ["code", "required"],
          properties: {
            code: { const: "level_unavailable" },
            required: { type: "string" },
            best: { type: "string" },
          },
        },
        {
          description: "La réservation dépasse la capacité.",
          type: "object",
          required: ["code"],
          properties: { code: { const: "capacity_exceeded" } },
        },
        {
          type: "object",
          required: ["code", "kind"],
          properties: { code: { const: "accelerator_unavailable" }, kind: { type: "string" } },
        },
      ],
    },
  },
};

test("une union étiquetée se modélise, sans angle mort", async () => {
  const { model, findings } = buildModel(await toy(TAGGED));
  assert.deepEqual(findings, []);

  const [union] = model.unions;
  assert.equal(model.unions.length, 1);
  assert.equal(union?.tag, "code", "l'étiquette est trouvée, pas devinée par convention");
  assert.deepEqual(
    union?.variants.map((variant) => variant.tag),
    ["level_unavailable", "capacity_exceeded", "accelerator_unavailable"],
  );
});

/**
 * **L'étiquette n'est pas un champ de la variante, côté Rust.**
 *
 * `#[serde(tag)]` l'écrit lui-même ; la laisser dans la variante la ferait écrire deux fois, et
 * serde refuse. C'est la raison pour laquelle l'IR garde les champs des variantes au lieu de
 * renvoyer à une structure nommée — la structure aurait porté l'étiquette, puisqu'elle vient du
 * même `properties`.
 */
test("Rust rend un enum étiqueté en interne, sans l'étiquette dans les variantes", async () => {
  const { model } = buildModel(await toy(TAGGED));
  const rust = emitRust(model);

  assert.match(rust, /#\[serde\(tag = "code"\)\]\npub enum Reason \{/);
  assert.match(rust, /#\[serde\(rename = "level_unavailable"\)\]\n {4}LevelUnavailable \{/);
  assert.match(rust, / {8}required: String,/);
  assert.match(
    rust,
    / {8}#\[serde\(skip_serializing_if = "Option::is_none", default\)\]\n {8}best: Option<String>,/,
  );
  assert.match(
    rust,
    /#\[serde\(rename = "capacity_exceeded"\)\]\n {4}CapacityExceeded,/,
    "une variante sans champ n'a pas de corps vide",
  );
  assert.doesNotMatch(rust, / {8}code: /, "l'étiquette est dans l'attribut, pas dans la variante");
  assert.doesNotMatch(rust, /pub type Reason = Reason;/, "pas d'alias auto-référent");
});

/**
 * **L'étiquette est un champ de la variante, côté TypeScript.**
 *
 * L'asymétrie avec Rust n'est pas une incohérence : c'est le type littéral qui permet à un
 * `switch (reason.code)` de rétrécir le type. Sans lui le lecteur testerait la présence des champs
 * pour deviner la forme, ce qu'aucune des deux langues ne devrait demander.
 */
test("TypeScript rend une union discriminée, avec l'étiquette en type littéral", async () => {
  const { model } = buildModel(await toy(TAGGED));
  const typescript = emitTypeScript(model);

  assert.match(typescript, /export type Reason =/);
  assert.match(typescript, /\| \{ readonly code: "level_unavailable"; readonly required: string;/);
  assert.match(typescript, /readonly best\?: string \| undefined;/);
  assert.match(typescript, /\| \{ readonly code: "capacity_exceeded"; \}/);
});

/**
 * **Un `oneOf` que rien ne discrimine reste un `finding`.**
 *
 * C'est la règle d'origine du générateur et elle ne se relâche pas : un lecteur devrait essayer les
 * branches une à une, et un type flou rendu pour ça produirait des consommateurs qui devinent. Le
 * message dit ce qui manque, pour que la réponse soit « étiqueter les branches » et non « le
 * générateur ne sait pas ».
 */
test("un oneOf sans étiquette commune reste refusé", async () => {
  const { findings } = buildModel(
    await toy({
      ...TAGGED,
      definitions: {
        reason: {
          oneOf: [
            { type: "object", required: ["a"], properties: { a: { type: "string" } } },
            { type: "object", required: ["b"], properties: { b: { type: "integer" } } },
          ],
        },
      },
    }),
  );
  assert.equal(findings.length, 1);
  assert.equal(findings[0]?.rule, "sdk-unsupported-oneof");
  assert.match(findings[0]?.message ?? "", /étiquetée/);
});

/**
 * **Deux branches de même étiquette produiraient deux variantes homonymes.**
 *
 * La seconde écraserait la première à la génération, et le document perdrait une forme sans que
 * rien ne le dise. `oneOf` accepterait pourtant les deux : c'est une contrainte du **générateur**,
 * pas du schéma, et elle mérite donc son propre nom plutôt qu'une erreur de compilation en aval.
 */
test("deux branches de même étiquette sont rapportées", async () => {
  const { findings } = buildModel(
    await toy({
      ...TAGGED,
      definitions: {
        reason: {
          oneOf: [
            {
              type: "object",
              required: ["code"],
              properties: { code: { const: "meme" }, a: { type: "string" } },
            },
            {
              type: "object",
              required: ["code"],
              properties: { code: { const: "meme" }, b: { type: "string" } },
            },
          ],
        },
      },
    }),
  );
  assert.equal(findings.length, 1);
  assert.equal(findings[0]?.rule, "sdk-duplicate-variant");
  assert.match(findings[0]?.message ?? "", /meme/);
});

/**
 * **Une branche par `$ref` est refusée par son nom.**
 *
 * La réponse à donner n'est pas « le générateur ne sait pas » mais « écris la branche ici ». Une
 * variante n'a pas d'existence hors de son union : la nommer ailleurs produirait une structure
 * autonome que personne n'instancie, et deux endroits à corriger le jour où la forme change. Le
 * premier document réel a été écrit avec des `$ref` et a rencontré ce refus — c'est de là que vient
 * la règle, et le message dit quoi faire plutôt que ce qui manque.
 */
test("une branche d'union par $ref est refusée, en disant quoi faire", async () => {
  const { findings } = buildModel(
    await toy({
      ...TAGGED,
      definitions: {
        reason: { oneOf: [{ $ref: "#/definitions/a" }, { $ref: "#/definitions/b" }] },
        a: {
          type: "object",
          required: ["code"],
          properties: { code: { const: "a" } },
        },
        b: {
          type: "object",
          required: ["code"],
          properties: { code: { const: "b" } },
        },
      },
    }),
  );
  const branche = findings.filter((finding) => finding.rule === "sdk-union-branch-by-ref");
  assert.equal(branche.length, 1);
  assert.match(branche[0]?.message ?? "", /inline/);
});

/** Un répertoire de schémas minimal : le registre, les features, les mécanismes, et le schéma. */
async function toy(schema: unknown): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "locus-sdk-"));
  scratch.push(root);
  await mkdir(join(root, "toy/1.0"), { recursive: true });
  await mkdir(join(root, "lep/1.0"), { recursive: true });
  await writeFile(join(root, "toy/1.0/refusal.schema.json"), JSON.stringify(schema));
  await writeFile(join(root, "lep/1.0/features.json"), JSON.stringify({ features: [] }));
  await writeFile(join(root, "lep/1.0/mechanisms.json"), JSON.stringify({ mechanisms: [] }));
  await writeFile(
    join(root, "registry.json"),
    JSON.stringify({
      draft: "http://json-schema.org/draft-07/schema#",
      shared: [],
      documents: [{ schema: "toy/1.0/refusal.schema.json", examples: [] }],
    }),
  );
  return root;
}
