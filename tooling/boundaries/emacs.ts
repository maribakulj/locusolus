import { execFile } from "node:child_process";
import { readdir, stat } from "node:fs/promises";
import { join } from "node:path";
import { promisify } from "node:util";

import type { Finding } from "../lib/findings.ts";
import type { EmacsRule } from "./rules.ts";

const run = promisify(execFile);

/**
 * Rule 5 — `apps/emacs` starts under `emacs -Q` with only its own `load-path`.
 *
 * This one cannot be read off the source: a package that quietly relies on the author's
 * `init.el` looks perfectly self-contained in a diff. So the check starts an actual Emacs with
 * no user configuration and nothing on `load-path` but the unit itself, loads each entry point,
 * and then compares `load-path` against the baseline — which catches the package that reaches
 * outside at load time instead of at read time.
 */

export type EmacsMode = "auto" | "required" | "off";

export type EmacsOutcome = {
  readonly state: "enforced" | "not-applicable" | "skipped";
  readonly scanned: number;
  readonly note?: string;
  readonly findings: readonly Finding[];
};

let probe: Promise<boolean> | null = null;

export function emacsAvailable(): Promise<boolean> {
  probe ??= run("emacs", ["--version"]).then(
    () => true,
    () => false,
  );
  return probe;
}

export async function checkEmacsIsolation(
  root: string,
  rule: EmacsRule,
  mode: EmacsMode,
): Promise<EmacsOutcome> {
  if (mode === "off") {
    return { state: "skipped", scanned: 0, note: "vérification Emacs désactivée", findings: [] };
  }
  const unit = join(root, rule.unit);
  if (!(await isDirectory(unit))) {
    return { state: "not-applicable", scanned: 0, note: `${rule.unit} n'existe pas`, findings: [] };
  }
  if (!(await emacsAvailable())) {
    if (mode === "auto") {
      return { state: "skipped", scanned: 0, note: "emacs absent de cette machine", findings: [] };
    }
    return {
      state: "enforced",
      scanned: 0,
      findings: [
        {
          rule: rule.id,
          where: rule.unit,
          message: "emacs est introuvable et --require-emacs interdit de sauter cette règle",
        },
      ],
    };
  }

  const sources = await entryPoints(unit);
  const results = await Promise.all(
    sources.map((file) => loadInIsolation(rule, unit, `${rule.unit}/${file}`, join(unit, file))),
  );
  return { state: "enforced", scanned: sources.length, findings: results.flat() };
}

async function loadInIsolation(
  rule: EmacsRule,
  unit: string,
  where: string,
  file: string,
): Promise<Finding[]> {
  try {
    const { stdout } = await run("emacs", [
      "-Q",
      "--batch",
      "-L",
      unit,
      "--eval",
      probeForm(unit, file),
    ]);
    return escapes(stdout).map((directory) => ({
      rule: rule.id,
      where,
      message: `ajoute ${directory} à load-path au chargement : le paquet sort de son répertoire`,
    }));
  } catch (error) {
    return [
      {
        rule: rule.id,
        where,
        message: `ne se charge pas sous \`emacs -Q\` avec sa seule load-path : ${firstLine(error)}`,
      },
    ];
  }
}

function probeForm(unit: string, file: string): string {
  return `(let ((home (file-name-as-directory (expand-file-name ${quote(unit)})))
      (baseline (mapcar (lambda (d) (file-name-as-directory (expand-file-name (or d ".")))) load-path)))
  (load ${quote(file)} nil t)
  (dolist (d load-path)
    (let ((full (file-name-as-directory (expand-file-name (or d ".")))))
      (unless (or (member full baseline) (string-prefix-p home full))
        (princ (format "load-path-escape:%s\\n" full))))))`;
}

function escapes(stdout: string): string[] {
  return stdout
    .split("\n")
    .filter((line) => line.startsWith("load-path-escape:"))
    .map((line) => line.slice("load-path-escape:".length).trim());
}

/**
 * Entry points: the `.el` files a consumer would load, at the root of the unit or of its `lisp/`
 * directory. Test files are excluded — they are allowed to require a test harness.
 */
async function entryPoints(unit: string): Promise<string[]> {
  const lisp = (await isDirectory(join(unit, "lisp"))) ? "lisp" : ".";
  const entries = await readdir(join(unit, lisp), { withFileTypes: true });
  return entries
    .filter((entry) => entry.isFile() && entry.name.endsWith(".el"))
    .map((entry) => entry.name)
    .filter((name) => !/-tests?\.el$/.test(name))
    .sort()
    .map((name) => (lisp === "." ? name : `${lisp}/${name}`));
}

function quote(value: string): string {
  return `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

function firstLine(error: unknown): string {
  const stderr = typeof error === "object" && error !== null ? Reflect.get(error, "stderr") : null;
  const text = typeof stderr === "string" && stderr.trim() ? stderr : String(error);
  return text.trim().split("\n")[0] ?? "échec sans message";
}

async function isDirectory(path: string): Promise<boolean> {
  try {
    return (await stat(path)).isDirectory();
  } catch {
    return false;
  }
}
