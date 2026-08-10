import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { report } from "../lib/findings.ts";
import { inspectBoundaries, type RuleStatus } from "./analyze.ts";
import type { EmacsMode } from "./emacs.ts";
import { loadContract } from "./rules.ts";

const args = process.argv.slice(2);
const emacs: EmacsMode = args.includes("--require-emacs") ? "required" : "auto";
const root = args.find((argument) => !argument.startsWith("--")) ?? defaultRoot();

const contract = await loadContract(join(root, "boundaries.json"));
const { findings, statuses } = await inspectBoundaries(root, contract, { emacs });

process.stdout.write(`${statuses.map(line).join("\n")}\n`);
process.exitCode = report("boundaries", findings);

/**
 * One line per rule, always — including the rules that found nothing to look at.
 *
 * On an empty repository most rules scan zero files. Printing that is the point: it is the
 * difference between "vérifié" and "il n'y avait rien à vérifier", and only one of the two is a
 * guarantee.
 */
function line(status: RuleStatus): string {
  const state = {
    enforced: `vérifiée sur ${status.scanned} fichier(s)`,
    "not-applicable": "sans objet",
    skipped: "NON VÉRIFIÉE",
  }[status.state];
  const note = status.note ? ` — ${status.note}` : "";
  return `  ${status.rule.claudeMd}. ${status.rule.id.padEnd(38)} ${state}${note}`;
}

function defaultRoot(): string {
  return fileURLToPath(new URL("../..", import.meta.url));
}
