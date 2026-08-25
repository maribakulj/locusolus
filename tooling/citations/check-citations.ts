import { fileURLToPath } from "node:url";

import { report } from "../lib/findings.ts";
import { inspectCitations } from "./citations.ts";

const root = process.argv[2] ?? fileURLToPath(new URL("../..", import.meta.url));
const { examined, citations, findings } = await inspectCitations(root);

/**
 * Ce qui a été lu se dit, **avant** le verdict — la règle que `W22.a` a payée.
 *
 * Le nombre de citations compte autant que le nombre de fichiers : une garde qui lirait tous les
 * documents et n'y trouverait aucune citation rendrait « ok » avec la même sérénité qu'une garde qui
 * les a toutes confrontées. Les deux se distinguent ici, et c'est la seule façon de s'apercevoir que
 * l'expression a cessé de reconnaître ce qu'elle cherche.
 */
process.stdout.write(
  `citations: ${citations} citation(s) nue(s) confrontée(s) au spec, dans ${examined.length} document(s)\n`,
);

process.exitCode = report("citations", findings);
