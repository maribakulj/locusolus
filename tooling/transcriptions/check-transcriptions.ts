import { fileURLToPath } from "node:url";

import { report } from "../lib/findings.ts";
import { inspectTranscriptions } from "./transcriptions.ts";

const root = process.argv[2] ?? fileURLToPath(new URL("../..", import.meta.url));
const { examined, fields, findings } = await inspectTranscriptions(root);

/**
 * Ce qui a été lu se dit, **avant** le verdict.
 *
 * Le nombre de champs compte autant que le nombre de transcriptions : une garde qui trouverait ses
 * deux fichiers et n'en extrairait aucun champ rendrait « ok » exactement comme une garde qui les a
 * tous confrontés. Les deux se distinguent ici — et les extractions vides échouent par ailleurs sous
 * leur propre nom, plutôt que de se fondre dans un compteur à zéro.
 */
process.stdout.write(
  examined.length > 0
    ? `transcriptions: ${fields} champ(s) confronté(s) — ${examined.join(", ")}\n`
    : "transcriptions: aucune transcription confrontée\n",
);

process.exitCode = report("transcriptions", findings);
