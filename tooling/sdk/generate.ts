import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
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
/**
 * Passer une source Rust par `rustfmt`, en lui parlant sur l'entrée standard.
 *
 * Le binaire vient de la toolchain déjà exigée par `check:rust` : aucune dépendance nouvelle, et
 * la même version que la porte qui vérifiera derrière. Un échec est **fatal** — une sortie non
 * formatée passerait `check:generated` et ferait rougir `cargo fmt --check`, c'est-à-dire
 * exactement la contradiction que ce passage supprime.
 */
function rustfmt(source: string): string {
  const result = spawnSync("rustfmt", ["--edition", "2024", "--emit", "stdout"], {
    input: source,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`rustfmt a échoué : ${result.stderr || result.error?.message || "sans motif"}`);
  }
  return result.stdout;
}

const args = process.argv.slice(2);
const checkOnly = args.includes("--check");
const root = args.find((argument) => !argument.startsWith("--")) ?? defaultRoot();

const targets = [
  { path: "packages/lep/src/generated.ts", emit: emitTypeScript, format: "prettier" },
  { path: "packages/lep/src/generated.rs", emit: emitRust, format: "rustfmt" },
] as const;

const { model, findings } = buildModel(join(root, "schemas"));
for (const target of targets) {
  // Chaque fichier généré passe par le formateur de sa langue, comme n'importe quel fichier du
  // dépôt. Sans cela `check:format` et `check:generated` se contrediraient : l'un exigerait un
  // reformatage que l'autre signalerait comme une dérive. Une seule forme canonique, et une montée
  // de version du formateur se voit comme une régénération à committer — ce qui est exactement ce
  // qu'elle est.
  //
  // Le Rust y est passé tard, et le défaut était réel : `W19.a` a ajouté un `enum` dont rustfmt
  // veut les variantes courtes sur une ligne, là où l'émetteur les écrivait sur trois. Les deux
  // portes se contredisaient, et aucune des deux n'avait tort — c'est l'émetteur qui n'avait pas
  // de forme canonique. Le lui faire imiter à la main aurait été réimplémenter rustfmt.
  const raw = target.emit(model);
  const absolute = join(root, target.path);
  // La configuration se résout depuis le FICHIER, pas depuis la racine : `resolveConfig` répond
  // « quelle config s'applique à ce chemin », et lui donner un répertoire rend une autre réponse
  // — d'où une sortie que `check:format` reformatait aussitôt.
  const rendered =
    target.format === "prettier"
      ? await prettier.format(raw, {
          ...(await prettier.resolveConfig(absolute)),
          parser: "typescript",
        })
      : rustfmt(raw);
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
