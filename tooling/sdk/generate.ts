import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import prettier from "prettier";

import { report, type Finding } from "../lib/findings.ts";
import { emitTypeScript } from "./emit-ts.ts";
import { emitRust } from "./emit-rust.ts";
import { buildModel } from "./ir.ts";

/**
 * Generate both SDKs from the schemas, or check that the committed ones are current.
 *
 * Generated code lives in the repository rather than in a build step: `packages/protocol` is a
 * Rust crate and `tooling/` is Node, so a build that had to run before either could compile would
 * make the two ecosystems wait on each other. The price is drift, and `--check` is what makes
 * drift impossible rather than merely unlikely.
 */
const args = process.argv.slice(2);
const checkOnly = args.includes("--check");
const root = args.find((argument) => !argument.startsWith("--")) ?? defaultRoot();

const targets = [
  { path: "packages/lep/src/generated.ts", emit: emitTypeScript, format: true },
  { path: "packages/lep/src/generated.rs", emit: emitRust, format: false },
] as const;

const { model, findings } = buildModel(join(root, "schemas"));
for (const target of targets) {
  // Le fichier généré passe par prettier, comme n'importe quel fichier du dépôt. Sans cela
  // `check:format` et `check:generated` se contrediraient : l'un exigerait un reformatage que
  // l'autre signalerait comme une dérive. Une seule forme canonique, et une montée de version de
  // prettier se voit comme une régénération à committer — ce qui est exactement ce qu'elle est.
  const raw = target.emit(model);
  const absolute = join(root, target.path);
  // La configuration se résout depuis le FICHIER, pas depuis la racine : `resolveConfig` répond
  // « quelle config s'applique à ce chemin », et lui donner un répertoire rend une autre réponse
  // — d'où une sortie que `check:format` reformatait aussitôt.
  const rendered = target.format
    ? await prettier.format(raw, {
        ...(await prettier.resolveConfig(absolute)),
        parser: "typescript",
      })
    : raw;
  if (!checkOnly) {
    mkdirSync(join(root, target.path, ".."), { recursive: true });
    writeFileSync(absolute, rendered);
    continue;
  }
  findings.push(...compare(absolute, target.path, rendered));
}

process.exitCode = report(checkOnly ? "generated" : "sdk", findings);
if (!checkOnly && findings.length === 0) {
  process.stdout.write(
    `sdk: ${model.structs.length} structures, ${model.aliases.length} alias, ` +
      `${model.documents.length} documents\n`,
  );
}

function compare(absolute: string, where: string, expected: string): Finding[] {
  let actual: string;
  try {
    actual = readFileSync(absolute, "utf8");
  } catch {
    return [
      {
        rule: "generated-missing",
        where,
        message: "absent : lancer `npm run sdk` et committer le résultat",
      },
    ];
  }
  if (actual === expected) return [];
  return [
    {
      rule: "generated-stale",
      where,
      message: "diffère de ce que les schémas produisent : lancer `npm run sdk` et committer",
    },
  ];
}

function defaultRoot(): string {
  return fileURLToPath(new URL("../..", import.meta.url));
}
