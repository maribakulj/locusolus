/**
 * La révision de `canterel` que le harnais monte — `W12.g`, ADR 0033 décision 1.
 *
 * # Ce que l'ADR décidait, et que le workflow ne faisait pas
 *
 * ADR 0033, deux fois : le titre de la décision 1 dit « `canterel` y est cloné à une **révision
 * épinglée** », et ses conséquences répètent « à une révision épinglée, **comme le SDK** ».
 *
 * `.github/workflows/ci.yml`, étape « Le dépôt worker », n'avait **aucun `ref:`**. Le checkout
 * prenait le `HEAD` de `main`. La décision était écrite et non appliquée — et son commentaire, juste
 * au-dessus de l'étape, affirmait pourtant « le worker réel, à une révision épinglée ».
 *
 * # Pourquoi ce n'est pas cosmétique
 *
 * Le verdict e2e de `locusolus` dépendait silencieusement de ce que `canterel/main` se trouvait
 * être. Un merge dans l'autre dépôt pouvait rendre cette CI rouge **sans aucun changement ici** —
 * exactement le « verdict qui peut rougir pour une raison étrangère cesse d'être lu » que la
 * décision 3 du même ADR appelle décisive, appliqué dans l'autre sens.
 *
 * # L'objection, et ce qui y répond
 *
 * Épingler fait cesser d'exercer le worker **courant**. C'est une objection réelle : c'est
 * l'exécution du worker courant qui a trouvé cinq défauts en une seule session — un `locus.identity`
 * périmé, un `lepCall` muet, un `project_id` exigé à tort, une administration non câblée, une
 * entropie absente. Un pin qui ne bougerait jamais rendrait la chaîne décorative.
 *
 * La réponse est dans le mot que l'ADR emploie : **comme le SDK**. `canterel` épingle le SDK par
 * commit dans `PINNED.json`, et le **bump délibérément**. Un pin n'est pas « ne jamais mettre à
 * jour » : c'est « mettre à jour est un acte qui se voit dans un diff ». Les deux propriétés
 * tiennent alors ensemble — le verdict ne bouge pas sous les pieds, et le bump ré-exerce le worker
 * courant sous revue.
 *
 * # Le pin absent **refuse**, il ne retombe pas sur `main`
 *
 * C'est la même règle que `workerRepo` applique déjà à `LOCUS_E2E_WORKER` : un repli silencieux sur
 * `main` rendrait vert un dossier monté contre une révision que personne n'a choisie, et
 * l'épinglage n'existerait plus que dans un commentaire — ce qui est précisément l'état d'où l'on
 * vient.
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { HarnessFailure } from "./harness.ts";

/** Le fichier qui porte la révision. */
export const PIN_FILE = "WORKER-PINNED.json";

/** Ce que le pin dit. */
export type Pin = {
  /** Le dépôt, en clair — pour qu'un lecteur sache de quoi on parle sans ouvrir le workflow. */
  readonly repo: string;
  /** La révision, en SHA complet. */
  readonly commit: string;
};

/** Un SHA de commit git, complet. Un `ref` court laisserait deux révisions porter le même nom. */
const SHA = /^[0-9a-f]{40}$/;

/**
 * Lire le pin, ou échouer en disant ce qui manque.
 *
 * @throws {HarnessFailure} quand le fichier est absent, illisible, ou ne porte pas un SHA complet.
 * Jamais de repli sur `main` : voir le module.
 */
export function workerPin(directory = dirname(fileURLToPath(import.meta.url))): Pin {
  const path = join(directory, PIN_FILE);

  let brut: string;
  try {
    brut = readFileSync(path, "utf8");
  } catch (erreur) {
    throw new HarnessFailure(
      PIN_FILE,
      `introuvable à « ${path} » : ${erreur instanceof Error ? erreur.message : String(erreur)}. ` +
        "Sans lui, le harnais monterait le `main` du worker, et le verdict de cette CI dépendrait " +
        "d'un dépôt que personne n'a fait bouger ici",
    );
  }

  let lu: unknown;
  try {
    lu = JSON.parse(brut);
  } catch (erreur) {
    throw new HarnessFailure(
      PIN_FILE,
      `ne se relit pas : ${erreur instanceof Error ? erreur.message : String(erreur)}`,
    );
  }

  if (typeof lu !== "object" || lu === null) {
    throw new HarnessFailure(PIN_FILE, "n'est pas un objet");
  }
  const { repo, commit } = lu as { repo?: unknown; commit?: unknown };

  if (typeof repo !== "string" || repo.trim() === "") {
    throw new HarnessFailure(PIN_FILE, "ne nomme pas de dépôt");
  }
  // Le SHA complet, et pas un `ref` quelconque : une branche ou un tag se déplacent, ce qui est
  // exactement ce que l'épinglage existe pour empêcher. Un SHA court, lui, peut devenir ambigu.
  if (typeof commit !== "string" || !SHA.test(commit)) {
    throw new HarnessFailure(
      PIN_FILE,
      `« ${String(commit)} » n'est pas un SHA de commit complet. Une branche ou un tag se ` +
        "déplacent, et un SHA court peut devenir ambigu — les trois rendraient l'épinglage " +
        "illusoire",
    );
  }

  return { repo, commit };
}
