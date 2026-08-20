import { readFile } from "node:fs/promises";
import { extname, join } from "node:path";

import type { Finding } from "../lib/findings.ts";
import { walkFiles } from "../lib/walk.ts";

/**
 * The retired project name, and the test of sortie of W0.1 made permanent.
 *
 * `docs/10_V1_ROADMAP.md` states the test as « `grep -r "locus-solus"` ne renvoie rien hors
 * historique Git ». Taken literally it can never pass, because the documents that *record* the
 * rename necessarily quote the old name — the roadmap quotes its own test, ADR 0009 records the
 * superseded name, the ledger records the rename itself. Those are the opposite of a violation.
 *
 * So the guard forbids every occurrence and requires each surviving one to be named here with a
 * reason. A mention nobody justified is a leftover; a justification nobody can find a mention
 * for is stale, and reported too.
 */

export const retiredName = "locus-solus";

export type Allowlist = ReadonlyMap<string, string>;

/** Occurrences that are the record of the rename rather than a use of the old name. */
export const historicalMentions: Allowlist = new Map([
  ["START_HERE_CLAUDE.md", "énumère les noms qui ne sont plus normatifs"],
  ["docs/10_V1_ROADMAP.md", "cite le test de sortie de W0.1, qui contient le motif"],
  ["docs/adr/0009-client-emacs-monorepo.md", "consigne le nom auquel l'ADR se substitue"],
  ["IMPLEMENTATION_LEDGER.md", "consigne le renommage à l'entrée d'étape 0"],
  ["tooling/repo/naming.ts", "porte le motif recherché"],
  ["tooling/README.md", "cite le test de sortie que cette garde rend permanent"],
]);

const binary = new Set([".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".pdf", ".woff2"]);

/**
 * Ce que la garde ne lit pas, parce que ce n'est pas dans le dépôt.
 *
 * `target/` est la sortie de build de Cargo — gitignorée, et pleine de sources de crates tiers.
 * Une occurrence du nom retiré dans un crate vendu serait un constat que personne ne peut réparer,
 * et l'unique réparation possible serait d'inscrire une dérogation pour un fichier qui n'existe pas
 * chez le voisin.
 *
 * Le coût a rendu l'oubli visible avant l'argument : `target/` pèse plusieurs gigaoctets sur une
 * machine qui a compilé, et la garde les lisait ligne à ligne. En CI le répertoire est vide au
 * moment où elle passe, ce qui est exactement pourquoi personne ne l'avait vu — la vitesse d'un
 * garde dépendait de l'état de build local, ce que `CLAUDE.md` refuse sous le nom de « dépendance
 * implicite à une machine de développeur ».
 *
 * `boundaries.json` excluait déjà les deux ; cette liste ne fait que rattraper le même oubli.
 */
const notInTheRepository = [
  "node_modules/**",
  "**/node_modules/**",
  "target/**",
  "**/target/**",
] as const;

export async function inspectNaming(
  root: string,
  allowed: Allowlist = historicalMentions,
): Promise<Finding[]> {
  const findings: Finding[] = [];
  const seen = new Set<string>();

  for (const path of await walkFiles(root, notInTheRepository)) {
    if (binary.has(extname(path).toLowerCase())) continue;
    const lines = await occurrences(join(root, path));
    if (lines.length === 0) continue;
    seen.add(path);
    const reason = allowed.get(path);
    if (reason) continue;
    findings.push(...lines.map((line) => leftover(path, line)));
  }

  for (const [path, reason] of allowed) {
    if (!seen.has(path)) {
      findings.push({
        rule: "retired-name-stale-exemption",
        where: path,
        message: `plus aucune occurrence de "${retiredName}" ici : retirer la dérogation « ${reason} »`,
      });
    }
  }
  return findings;
}

function leftover(path: string, line: number): Finding {
  return {
    rule: "retired-name",
    where: `${path}:${line}`,
    message: `"${retiredName}" est le nom retiré du projet ; écrire "locusolus"`,
  };
}

async function occurrences(path: string): Promise<number[]> {
  const text = await readFile(path, "utf8").catch(() => "");
  return text.split("\n").flatMap((line, index) => (line.includes(retiredName) ? [index + 1] : []));
}
