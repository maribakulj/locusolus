import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { type Finding, report } from "../lib/findings.ts";

/**
 * La surface de dépendances externes du workspace Rust, tenue par une liste close.
 *
 * # Pourquoi une porte, et pas seulement un ADR
 *
 * `docs/10_V1_ROADMAP.md` demande pour `W20.c` que « `Cargo.toml` ne gagne sa première dépendance
 * hors `serde` **qu'après** l'ADR, et que le diff qui l'ajoute cite l'ADR ». La seconde moitié est
 * une promesse que personne ne peut tenir à la relecture : un `Cargo.toml` gagne une ligne dans un
 * diff de deux cents, et le reviewer qui la manque n'a rien manqué de visible.
 *
 * `dependencies.json` la rend mécanique. Une dépendance externe absente de la liste fait échouer la
 * CI, et l'ajouter oblige à écrire l'ADR qui l'autorise — dans le même diff, puisque c'est le même
 * fichier.
 *
 * # Ce qui n'est pas vérifié ici, et pourquoi c'est dit
 *
 * Les dépendances **transitives** ne sont pas dans la liste : `axum` en apporte une cinquantaine, et
 * les énumérer ferait une liste que personne ne relit, donc une liste qui n'arbitre plus rien. La
 * porte tient les dépendances **déclarées**, celles qu'un humain a choisies. Le décompte transitif
 * est l'affaire de l'ADR, qui le mesure et le motive.
 */

const root = process.argv[2] ?? fileURLToPath(new URL("../..", import.meta.url));

type Allowance = {
  readonly crate: string;
  readonly adr: string;
  readonly why: string;
  /** `*` pour tout le workspace, sinon un préfixe de chemin — `apps/locusd`. */
  readonly scope: string;
};

type ForbiddenFeature = {
  readonly crate: string;
  readonly feature: string;
  readonly adr: string;
  readonly why: string;
};

type Policy = {
  readonly allowed: readonly Allowance[];
  readonly forbiddenFeatures: readonly ForbiddenFeature[];
};

const policy: Policy = JSON.parse(await readFile(join(root, "dependencies.json"), "utf8"));

/**
 * Les dépendances déclarées d'un `Cargo.toml`, sans lire un TOML complet.
 *
 * Le format des manifestes de ce dépôt est stable et écrit à la main : une section, une ligne par
 * dépendance. Un parseur TOML serait une dépendance de plus pour lire un fichier qui décide des
 * dépendances, ce qui est exactement le genre de circularité qu'on préfère éviter.
 */
function declaredDependencies(manifest: string): { name: string; spec: string }[] {
  const found: { name: string; spec: string }[] = [];
  let section = "";
  for (const raw of manifest.split("\n")) {
    const line = raw.trim();
    if (line.startsWith("#")) continue;
    if (line.startsWith("[")) {
      section = line.replace(/[[\]]/g, "");
      continue;
    }
    if (!section.endsWith("dependencies")) continue;
    const [, name, spec] = /^([A-Za-z0-9_-]+)\s*=\s*(.+)$/.exec(line) ?? [];
    if (name !== undefined && spec !== undefined) found.push({ name, spec });
  }
  return found;
}

/** Une dépendance `path = "..."` est interne au workspace : elle n'est pas une surface externe. */
function isInternal(spec: string): boolean {
  return spec.includes("path =");
}

async function manifests(directory: string): Promise<string[]> {
  const entries = await readdir(join(root, directory), { withFileTypes: true }).catch(() => []);
  return entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => `${directory}/${entry.name}/Cargo.toml`);
}

const files = ["Cargo.toml", ...(await manifests("apps")), ...(await manifests("packages"))];
const findings: Finding[] = [];

for (const file of files) {
  const manifest = await readFile(join(root, file), "utf8").catch(() => null);
  if (manifest === null) continue;

  for (const { name, spec } of declaredDependencies(manifest)) {
    if (isInternal(spec)) continue;
    if (spec.includes("workspace = true")) continue;

    const allowance = policy.allowed.find((entry) => entry.crate === name);
    if (!allowance) {
      findings.push({
        rule: "dependance-non-autorisee",
        where: file,
        message: `« ${name} » n'est pas dans dependencies.json. Une dépendance externe entre avec l'ADR qui la motive, dans le même diff.`,
      });
      continue;
    }
    if (allowance.scope !== "*" && !file.startsWith(allowance.scope)) {
      findings.push({
        rule: "dependance-hors-perimetre",
        where: file,
        message: `« ${name} » n'est autorisée que sous ${allowance.scope} (ADR ${allowance.adr}).`,
      });
    }
    for (const forbidden of policy.forbiddenFeatures) {
      if (forbidden.crate !== name) continue;
      if (new RegExp(`"${forbidden.feature}"`).test(spec)) {
        findings.push({
          rule: "feature-interdite",
          where: file,
          message: `« ${name}/${forbidden.feature} » est refusée par l'ADR ${forbidden.adr} : ${forbidden.why}`,
        });
      }
    }
  }
}

/**
 * Ce que la porte a effectivement lu.
 *
 * Une porte qui n'a rien lu et rend « ok » est indiscernable d'une porte qui a tout vérifié. La
 * leçon est celle de la règle 3 du « Rythme de session » de `CLAUDE.md` : un compteur qui n'a rien
 * lu ne vaut pas zéro.
 */
process.stdout.write(
  `deps: ${files.length} manifeste(s), ${policy.allowed.length} crate(s) externe(s) autorisé(s)\n`,
);
process.exitCode = report("deps", findings);
