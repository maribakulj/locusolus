import { execFile } from "node:child_process";
import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const run = promisify(execFile);

/**
 * ERT suite for `apps/emacs`, run under `emacs -Q` with only the package on `load-path`.
 *
 * The isolation is not a convenience here, it is the point: a suite run under the author's
 * configuration proves that the package works *there*, which is the one place it was never in
 * doubt. `--require-emacs` mirrors the boundary checker — locally the suite may skip, in CI it
 * may not, because a suite that skips silently is indistinguishable from a suite that passes.
 */

const args = process.argv.slice(2);
const required = args.includes("--require-emacs");
const root = args.find((argument) => !argument.startsWith("--")) ?? defaultRoot();
const unit = join(root, "apps/emacs");
const tests = join(unit, "test");

if (!(await emacsAvailable())) {
  if (required) {
    process.stderr.write("emacs est introuvable et --require-emacs interdit de sauter la suite\n");
    process.exit(1);
  }
  process.stdout.write("emacs-tests: sautée — emacs absent de cette machine\n");
  process.exit(0);
}

const files = await suiteFiles();
if (files.length === 0) {
  process.stdout.write("emacs-tests: sans objet — apps/emacs/test ne porte aucun test\n");
  process.exit(0);
}

/**
 * Native-compiled trampolines are disabled for the suite, and this is a correctness fix, not a
 * speed knob.
 *
 * Several tests redefine primitives with `cl-letf` — `make-network-process`, `open-network-stream`
 * — to poison the network and prove that a render talks to nobody. Advising a subr makes Emacs
 * synthesise a trampoline and native-compile it on the spot, so on a host whose libgccjit cannot
 * link (`ld: library 'emutls_w' not found`, seen on macOS/Homebrew) the *poison itself* fails and
 * the test reports a native-compiler error. The property under test never gets exercised, and the
 * failure names a compiler rather than the code.
 *
 * Interpreting the trampoline instead costs nothing a batch suite can measure and removes a
 * dependency on the host's toolchain — which is exactly what `-Q` with a single `load-path` exists
 * to remove everywhere else.
 */
const emacsArgs = [
  "-Q",
  "--batch",
  "--eval",
  "(setq native-comp-enable-subr-trampolines nil)",
  "-L",
  unit,
  "-L",
  tests,
  "-l",
  "ert",
];
for (const file of files) {
  emacsArgs.push("-l", join(tests, file));
}
emacsArgs.push("-f", "ert-run-tests-batch-and-exit");

try {
  const { stderr } = await run("emacs", emacsArgs);
  process.stdout.write(`${lastLine(stderr)}\n`);
  process.stdout.write(`emacs-tests: ok (${files.length} fichier(s))\n`);
} catch (error) {
  const stderr = typeof error === "object" && error !== null ? Reflect.get(error, "stderr") : "";
  process.stderr.write(`${typeof stderr === "string" ? stderr : String(error)}\n`);
  process.exitCode = 1;
}

async function suiteFiles(): Promise<string[]> {
  try {
    const entries = await readdir(tests, { withFileTypes: true });
    return entries
      .filter((entry) => entry.isFile() && /-tests?\.el$/.test(entry.name))
      .map((entry) => entry.name)
      .sort();
  } catch {
    return [];
  }
}

async function emacsAvailable(): Promise<boolean> {
  return run("emacs", ["--version"]).then(
    () => true,
    () => false,
  );
}

function lastLine(output: string): string {
  const lines = output.trim().split("\n");
  return lines[lines.length - 1] ?? "";
}

function defaultRoot(): string {
  return fileURLToPath(new URL("../..", import.meta.url));
}
