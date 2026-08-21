/**
 * La garde de cohérence descriptive — `W22.d`, ADR 0025.
 *
 * # Ce qu'elle cherche, et pourquoi ce n'est pas un mot
 *
 * `apps/locus-execd/src/main.rs` a imprimé pendant des mois :
 *
 * > `locus-execd : aucun driver de runtime n'est encore branché (W4.d).`
 *
 * pendant que son propre crate exportait `SystemRunner`, la seule fonction du dépôt qui exécute
 * `podman`. Un exploitant qui lançait le binaire en concluait que la fabric d'exécution n'existait
 * pas.
 *
 * La première formulation de cet item cherchait « un motif de refus dont le crate exporte le symbole
 * déclaré manquant ». **Elle n'aurait pas attrapé ce message-là** : il ne nomme aucun symbole, il dit
 * « aucun driver ». Et chercher la tournure — « aucun », « pas encore » — mordrait sur le message
 * parfaitement légitime de `locusd`, qui dit « le port n'est pas ouvert » sur une condition qu'il
 * vient de **calculer**.
 *
 * Le signal est ailleurs, et il est net : **le message cite un item du plan**, `(W4.d)`.
 *
 * # La règle
 *
 * Un programme qui tourne n'a pas à citer la roadmap. Un identifiant d'item dans un message
 * d'exécution est une affirmation sur l'état du **dépôt**, et le dépôt change sans que le message
 * change — c'est la définition même de la dérive que l'ADR 0025 nomme.
 *
 * Ce que le binaire a le droit de dire est ce qu'il a **calculé** : « cet hôte plafonne à `S1` »
 * vieillit avec l'hôte, pas avec le plan.
 *
 * C'est bien un **couple** — déclaration d'exécution ↔ identifiant de plan — et non une recherche de
 * mots, ce que la décision 4 de l'ADR 0025 écarte. L'addendum de cet ADR dit pourquoi la
 * formulation « déclaration/symbole » a dû devenir « déclaration/item ».
 *
 * # Ce qu'elle ne verra jamais
 *
 * Un message qui affirmerait une absence **sans** citer d'item. Rien de mécanique ne le distingue de
 * « le port n'est pas ouvert », qui est vrai et calculé. La garde réduit la fenêtre, elle ne la ferme
 * pas — et `W22.c` a fermé le cas de `locus-execd` par un test local qui refuse toute tournure
 * d'absence dans ce point d'entrée précis, ce qu'on peut faire fichier par fichier et pas en général.
 *
 * # Les points d'entrée sont découverts, jamais listés
 *
 * Ils viennent des `[[bin]]` des manifestes. Une liste écrite à la main aurait la même infirmité que
 * le motif d'identifiant de `W22.a` : elle serait aveugle au binaire suivant, et son silence se
 * lirait « rien à signaler ». Un décompte nul est donc un **échec** : il ne veut pas dire que tout va
 * bien, il veut dire que la garde n'a rien regardé.
 */

import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

import type { Finding } from "../lib/findings.ts";

/** Les répertoires où un crate du workspace peut vivre. */
const WORKSPACES = ["apps", "packages"] as const;

/**
 * Un identifiant d'item du plan, dans la forme que `W22.a` a fixée.
 *
 * Écrit ici plutôt qu'importé de la garde de roadmap : les deux gardes ne partagent pas d'état, et
 * un import créerait une dépendance dont la seule justification serait d'économiser une ligne. La
 * forme, elle, est la même, et le test d'accord entre les deux le tient.
 *
 * Les bornes sont des **non-lettres** de part et d'autre : sans elles, `W5` matcherait dans `W50`,
 * et `SW4.d` — un identifiant qui n'existe pas — passerait pour `W4.d`.
 */
const PLAN_ITEM = /(?<![A-Za-z0-9])(?:W\d+\.[a-z0-9]+(?:\.\d+)?|R\d+)(?![A-Za-z0-9])/;

/** Ce que la garde a examiné, et ce qu'elle y a trouvé. */
export type Coherence = {
  /** Les points d'entrée réellement lus, chemins relatifs à la racine. */
  readonly examined: readonly string[];
  readonly findings: readonly Finding[];
};

/** Lire les points d'entrée déclarés par les manifestes du workspace. */
export async function entryPoints(root: string): Promise<string[]> {
  const found: string[] = [];
  for (const workspace of WORKSPACES) {
    const crates = await readdir(join(root, workspace), { withFileTypes: true }).catch(() => []);
    for (const crate of crates) {
      if (!crate.isDirectory()) {
        continue;
      }
      const manifest = await readFile(
        join(root, workspace, crate.name, "Cargo.toml"),
        "utf8",
      ).catch(() => undefined);
      if (manifest === undefined) {
        continue;
      }
      for (const path of binaryPaths(manifest)) {
        found.push(`${workspace}/${crate.name}/${path}`);
      }
    }
  }
  return found.sort();
}

/**
 * Les chemins des `[[bin]]` d'un manifeste.
 *
 * Volontairement pas un analyseur TOML — comme `declaredDependencies`, il lit des en-têtes de
 * section et des clés de gauche, ce qu'un chemin de binaire est toujours. Un `[[bin]]` **sans**
 * `path` n'est pas deviné : Cargo le déduirait de `src/main.rs`, mais deviner ici ferait examiner un
 * fichier que le manifeste n'a pas nommé, et une garde qui invente son entrée ne dit plus sur quoi
 * elle a conclu.
 */
function binaryPaths(manifest: string): string[] {
  const paths: string[] = [];
  let inBinary = false;
  for (const raw of manifest.split("\n")) {
    const line = raw.replace(/#.*$/, "").trim();
    if (line === "") {
      continue;
    }
    const header = /^\[\[?\s*([^\]]+?)\s*\]\]?$/.exec(line);
    if (header?.[1] !== undefined) {
      inBinary = header[1] === "bin";
      continue;
    }
    if (!inBinary) {
      continue;
    }
    const path = /^path\s*=\s*["']([^"']+)["']/.exec(line);
    if (path?.[1] !== undefined) {
      paths.push(path[1]);
    }
  }
  return paths;
}

/**
 * Les littéraux de chaîne d'une source Rust, commentaires exclus.
 *
 * Un seul passage plutôt qu'un nettoyage suivi d'une extraction : retirer les commentaires d'abord
 * demanderait de savoir si un `//` est dans une chaîne, c'est-à-dire de faire déjà ce travail. Le
 * scanner suit donc l'état — hors chaîne, en chaîne, en commentaire — et ne rend que ce qui est
 * exécutable.
 *
 * Les chaînes brutes (`r"…"`, `r#"…"#`) sont reconnues : les omettre serait une cécité de plus, et
 * cet item entier existe parce qu'une cécité ne baisse aucun décompte.
 */
export function stringLiterals(source: string): string[] {
  const literals: string[] = [];
  let index = 0;
  while (index < source.length) {
    const two = source.slice(index, index + 2);
    if (two === "//") {
      const end = source.indexOf("\n", index);
      index = end === -1 ? source.length : end + 1;
      continue;
    }
    if (two === "/*") {
      const end = source.indexOf("*/", index + 2);
      index = end === -1 ? source.length : end + 2;
      continue;
    }
    const raw = /^r(#*)"/.exec(source.slice(index, index + 12));
    if (raw?.[1] !== undefined) {
      const fence = `"${raw[1]}`;
      const start = index + raw[0].length;
      const end = source.indexOf(fence, start);
      literals.push(source.slice(start, end === -1 ? source.length : end));
      index = end === -1 ? source.length : end + fence.length;
      continue;
    }
    if (source[index] === '"') {
      let cursor = index + 1;
      while (cursor < source.length && source[cursor] !== '"') {
        cursor += source[cursor] === "\\" ? 2 : 1;
      }
      literals.push(source.slice(index + 1, cursor));
      index = cursor + 1;
      continue;
    }
    index += 1;
  }
  return literals;
}

/** Confronter chaque point d'entrée à la règle, et dire lesquels ont été lus. */
export async function inspectCoherence(root: string): Promise<Coherence> {
  const examined: string[] = [];
  const findings: Finding[] = [];

  for (const path of await entryPoints(root)) {
    const source = await readFile(join(root, path), "utf8").catch(() => undefined);
    if (source === undefined) {
      findings.push({
        rule: "point-d-entree-illisible",
        where: path,
        message: `un manifeste déclare ce binaire et le fichier n'est pas lisible : la garde ne peut pas conclure dessus, et ne prétend pas l'avoir fait`,
      });
      continue;
    }
    examined.push(path);
    for (const literal of stringLiterals(source)) {
      const cited = PLAN_ITEM.exec(literal);
      if (cited === null) {
        continue;
      }
      findings.push({
        rule: "message-qui-cite-le-plan",
        where: path,
        message: `un message d'exécution cite « ${cited[0] ?? ""} » : un programme qui tourne n'a pas à citer la roadmap, parce que le plan change sans que le message change — dire ce qu'on a **calculé** vieillit avec la machine, pas avec le dépôt`,
      });
    }
  }

  if (examined.length === 0) {
    findings.push({
      rule: "aucun-point-d-entree",
      where: "Cargo.toml",
      message:
        "aucun point d'entrée n'a été examiné : un décompte nul ne veut pas dire que tout va bien, il veut dire que la garde n'a rien regardé",
    });
  }

  return { examined, findings };
}
