import { fileURLToPath } from "node:url";

import { report } from "../lib/findings.ts";
import { readReconciliation, reconcile } from "./roadmap.ts";

const root = process.argv[2] ?? fileURLToPath(new URL("../..", import.meta.url));
const state = await readReconciliation(root);

/**
 * Combien de lignes d'item le motif a reconnues, **avant** toute conclusion.
 *
 * Sans cette ligne, une cécité du motif se lit « rien à signaler » : les lignes qu'il ne voit pas
 * n'entrent dans aucun ensemble, donc aucune règle ne parle d'elles, donc aucun décompte ne baisse.
 * C'est ainsi que huit lignes livrées sont restées sans marqueur pendant que la garde imprimait
 * « frontière vide — toute ligne du plan est faite ».
 *
 * Le total est **dérivé**, jamais écrit en dur : `recognised + unrecognised.length` est le nombre de
 * lignes qui portent un type `[R]` ou `[M]`, et ce dénombrement-là ne passe pas par le motif qu'il
 * sert à vérifier. Un nombre attendu écrit en constante aurait demandé d'être relevé à chaque ligne
 * ajoutée, et une constante qu'on relève sans y penser ne vérifie plus rien.
 */
process.stdout.write(
  `roadmap: ${state.recognised} ligne(s) d'item reconnue(s) sur ${state.recognised + state.unrecognised.length}\n`,
);

/**
 * Ce qui n'a pas été lu se dit, sinon « ok » se lirait « tout est vérifié ».
 *
 * La CI de ce dépôt ne voit qu'un checkout sur quatre : la règle « marqué sans entrée » y est
 * suspendue, et un « ok » muet ferait passer cette suspension pour une vérification.
 */
if (state.unread.length > 0) {
  process.stdout.write(
    `roadmap: registres non lus (${state.unread.join(", ")}) — « marqué sans entrée » suspendue\n`,
  );
}

/**
 * La frontière, calculée plutôt que lue à l'œil.
 *
 * « Le premier item non terminé dont les dépendances sont satisfaites » se lisait en parcourant le
 * tableau, et une lecture à l'œil a déjà confondu `bloqué` avec « à faire ». Les lignes qui ne
 * déclarent rien sont les seules candidates ; les nommer ici évite de refaire le tri à chaque tour.
 */
process.stdout.write(
  state.frontier.length > 0
    ? `roadmap: frontière — ${state.frontier.join(", ")}\n`
    : "roadmap: frontière vide — toute ligne du plan est faite, bloquée ou reportée\n",
);

process.exitCode = report("roadmap", reconcile(state));
