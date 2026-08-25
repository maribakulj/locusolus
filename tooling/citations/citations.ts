/**
 * Une citation de section se vérifie — `W0.21`.
 *
 * # Le défaut qui a motivé cette garde
 *
 * `docs/10_V1_ROADMAP.md` et l'ADR 0026 décision 4 écrivent tous deux, mot pour mot : « §12.4
 * (isolation de branche) ». Or `docs/SPEC_V1.md` §12.4 s'appelle **Backpressure**, et ce dépôt n'a
 * nulle part de section d'isolation informationnelle. La section visée est celle de
 * `canterel/docs/locus/SPEC_V1.md`, dont le §12.4 s'appelle bien « Isolation informationnelle ».
 *
 * Les deux documents portent le même nom de fichier et numérotent chacun depuis 1, et ce ne sont
 * **pas** deux copies : sur 147 numéros de section communs, **5** portent le même titre. Ce sont deux
 * spécifications différentes — l'une du plan de contrôle, l'autre du worker.
 *
 * Une citation nue est donc résolue par le lecteur dans le document qu'il a sous les yeux, et une
 * citation qui vise l'autre dépôt y désigne autre chose **sans erreur visible**. C'est la forme la
 * plus discrète du défaut que ce dépôt traque toute la journée : une affirmation vérifiable que
 * personne n'a de raison mécanique de vérifier.
 *
 * # Ce que la garde attrape, et ce qu'elle n'attrape pas
 *
 * Elle attrape une citation **nue** vers un numéro qui n'existe pas dans le spec local. C'est
 * mécanique et sans faux positif.
 *
 * Elle **n'attrape pas** le cas qui l'a motivée. `§12.4` existe localement ; il désigne simplement
 * autre chose que ce que l'auteur avait en tête. Aucune vérification d'existence ne peut voir ça, et
 * je n'en propose pas de plus fine : comparer la glose entre parenthèses au titre de la section
 * demanderait de décider que « isolation de branche » et « Backpressure » ne parlent pas de la même
 * chose, ce qu'aucune règle textuelle ne tranche sans se tromper souvent.
 *
 * Ce que la garde rend donc vrai est plus modeste et se dit sans hédger : **toute citation nue
 * désigne une section qui existe ici**. Le reste tient à la convention, écrite dans `CLAUDE.md` et
 * déjà en usage — une citation vers un autre dépôt **nomme son fichier**, comme
 * `docs/10_V1_ROADMAP.md` le fait déjà pour `repos/canterel/SPEC_V1.md §4`.
 */

import { readFile, readdir } from "node:fs/promises";
import { join, relative } from "node:path";

import type { Finding } from "../lib/findings.ts";

/** Le document dont les citations nues parlent. */
const SPEC = "docs/SPEC_V1.md";

/**
 * Ce qui, devant un `§`, le qualifie — c'est-à-dire nomme le document visé.
 *
 * Un ADR cite ses propres sections (« ADR 0017 §5.1 ») et un renvoi inter-dépôts nomme son fichier
 * (« repos/canterel/SPEC_V1.md §4 »). Les deux sont **corrects** et ne doivent pas rougir : ce que la
 * garde exige est qu'une citation dise où chercher, pas qu'elle vise le spec local.
 */
const QUALIFIE = /(?:ADR\s*\d{4}|[\w/.-]+\.md|`[^`]*\.md`)[^§]{0,40}$/u;

/** Un titre de section numérotée, tel que les deux specs les écrivent. */
const TITRE = /^#{2,4}\s+(\d+(?:\.\d+)*)\.?\s+(.+)$/u;

/** Une citation de section. */
const CITATION = /§\s?(\d+(?:\.\d+)*)/gu;

/** Les sections numérotées d'un document, par numéro. */
export function sections(markdown: string): Map<string, string> {
  const found = new Map<string, string>();
  for (const line of markdown.split("\n")) {
    const match = TITRE.exec(line.trim());
    const numero = match?.[1];
    const titre = match?.[2];
    if (numero !== undefined && titre !== undefined) found.set(numero, titre.trim());
  }
  return found;
}

/** Ce qu'une inspection a lu et trouvé. */
export type Inspection = {
  /** Les fichiers réellement lus — nommés, pas seulement comptés. */
  readonly examined: readonly string[];
  /** Combien de citations ont été confrontées au spec. */
  readonly citations: number;
  /** Ce qui ne va pas. */
  readonly findings: readonly Finding[];
};

/** Les fichiers Markdown d'un répertoire, récursivement. */
async function markdownFiles(root: string, from: string): Promise<string[]> {
  const entries = await readdir(join(root, from), { withFileTypes: true });
  const out: string[] = [];
  for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
    const path = join(from, entry.name);
    if (entry.isDirectory()) out.push(...(await markdownFiles(root, path)));
    else if (entry.name.endsWith(".md")) out.push(path);
  }
  return out;
}

/**
 * Confronter chaque citation nue au spec local.
 *
 * Le spec lui-même est exclu : il se cite abondamment, et ses renvois internes sont l'affaire de sa
 * propre relecture.
 */
export async function inspectCitations(root: string): Promise<Inspection> {
  const spec = sections(await readFile(join(root, SPEC), "utf8"));
  if (spec.size === 0) {
    // Un spec dont on n'a lu aucune section rendrait **toutes** les citations fautives, ce qui se
    // lirait comme une avalanche de fautes plutôt que comme « je n'ai pas su lire le spec ». La
    // règle du dépôt : une garde bâtie sur une lecture distingue « la réponse est zéro » de « il n'y
    // a pas eu de réponse », et échoue bruyamment sur la seconde.
    return {
      examined: [],
      citations: 0,
      findings: [
        {
          rule: "spec-illisible",
          where: SPEC,
          message:
            "aucune section numérotée lue : la garde ne peut rien confronter, et un « ok » ici " +
            "voudrait dire « je n'ai pas regardé »",
        },
      ],
    };
  }

  const files = [...(await markdownFiles(root, "docs")), "CLAUDE.md"].filter(
    (path) => relative(SPEC, path) !== "",
  );
  const findings: Finding[] = [];
  const examined: string[] = [];
  let citations = 0;

  for (const path of files) {
    const text = await readFile(join(root, path), "utf8");
    examined.push(path);
    for (const [index, line] of text.split("\n").entries()) {
      CITATION.lastIndex = 0;
      let match: RegExpExecArray | null;
      while ((match = CITATION.exec(line)) !== null) {
        const numero = match[1] ?? "";
        if (QUALIFIE.test(line.slice(0, match.index))) continue;
        citations += 1;
        if (spec.has(numero)) continue;
        findings.push({
          rule: "citation-sans-section",
          where: `${path}:${index + 1}`,
          message:
            `« §${numero} » n'existe pas dans ${SPEC} : une citation nue désigne le spec de ce ` +
            "dépôt, et une citation vers un autre dépôt nomme son fichier",
        });
      }
    }
  }

  return { examined, citations, findings };
}
