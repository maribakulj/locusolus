import { access } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { report } from "../lib/findings.ts";
import { compareWithSource, readWorkerPin, WORKER_PIN } from "./worker-pin.ts";

/**
 * La lecture du SDK par le worker est-elle à jour ? — `W0.24`.
 *
 * # Deux modes, et l'un n'est pas une version affaiblie de l'autre
 *
 * **Chemin donné** — le job `e2e`, qui monte déjà `canterel` à la révision épinglée : la garde est
 * stricte, et un pin illisible est une violation. Le checkout a réussi, donc le fichier est là ;
 * s'il ne l'est pas, quelque chose d'autre est cassé et le taire ferait chercher ailleurs.
 *
 * **Chemin absent** — une machine de développeur, où `../canterel` peut ne pas exister : la garde
 * se **déclare non exécutée** et rend `0`. C'est la forme que `check:roadmap` a déjà pour les
 * registres voisins : dire qu'on n'a pas lu, plutôt que laisser « ok » se lire « tout est
 * vérifié ». Un `1` ici rendrait `npm run check` impossible à passer sur une machine qui n'a qu'un
 * des quatre dépôts, et la garde serait retirée dans la semaine.
 *
 * La différence n'est donc pas la rigueur, c'est **ce qui est su** : là où le checkout est garanti,
 * son absence est une panne ; là où il ne l'est pas, son absence est une machine ordinaire.
 */

const root = fileURLToPath(new URL("../..", import.meta.url));
const donne = process.argv[2];
const worker = donne ?? join(root, "..", "canterel");

if (donne === undefined && !(await lisible(join(worker, WORKER_PIN)))) {
  process.stdout.write(
    `worker-pin: non exécutée — aucun dépôt worker sous « ${worker} », et aucun chemin donné\n`,
  );
  process.exit(0);
}

const reading = await readWorkerPin(worker);
if (reading.kind === "lu") {
  // Le commit épinglé, imprimé avant toute conclusion : c'est la seule façon de savoir **quelle**
  // révision a été comparée quand la garde rend `ok`, et un `ok` sans lui ne dit pas contre quoi.
  process.stdout.write(
    `worker-pin: « ${worker} » épingle ${reading.commit.slice(0, 12)}… et ${reading.entries.length} fichier(s)\n`,
  );
}

process.exitCode = report("worker-pin", await compareWithSource(reading, root, worker));

async function lisible(path: string): Promise<boolean> {
  return access(path).then(
    () => true,
    () => false,
  );
}
