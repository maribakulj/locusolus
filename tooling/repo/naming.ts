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

export async function inspectNaming(
  root: string,
  allowed: Allowlist = historicalMentions,
): Promise<Finding[]> {
  const findings: Finding[] = [];
  const seen = new Set<string>();

  for (const path of await walkFiles(root, ["node_modules/**", "**/node_modules/**"])) {
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
