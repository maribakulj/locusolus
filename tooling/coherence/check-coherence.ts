import { fileURLToPath } from "node:url";

import { report } from "../lib/findings.ts";
import { inspectCoherence } from "./coherence.ts";

const root = process.argv[2] ?? fileURLToPath(new URL("../..", import.meta.url));
const { examined, findings } = await inspectCoherence(root);

/**
 * Ce qui a été lu se dit, **avant** le verdict.
 *
 * `W22.a` a montré ce que coûte un décompte absent : huit lignes du plan étaient invisibles à leur
 * garde, aucune règle ne parlait d'elles, aucun compteur ne baissait, et « ok » se lisait « tout est
 * vérifié ». La même infirmité guette une garde qui découvre ses entrées : si les manifestes
 * cessaient de déclarer leurs `[[bin]]`, elle n'aurait plus rien à regarder et le dirait « ok ».
 *
 * Les chemins sont nommés, pas seulement comptés : « 2 points d'entrée » ne dit pas *lesquels*, et
 * c'est en ne sachant pas lesquels qu'on croit un jour qu'ils sont tous là.
 */
process.stdout.write(
  examined.length > 0
    ? `coherence: ${examined.length} point(s) d'entrée examiné(s) — ${examined.join(", ")}\n`
    : "coherence: aucun point d'entrée examiné\n",
);

process.exitCode = report("coherence", findings);
