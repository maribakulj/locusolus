/**
 * Le test de sortie de `W12.f` — le harnais échoue **bruyamment**, il ne se saute pas.
 *
 * # Ce que ce fichier tient, et ce qu'il ne tient pas
 *
 * Il tient la moitié du test de sortie qui est vérifiable **sans les trois processus** : un
 * prérequis absent produit une panne qui le nomme, jamais un saut. C'est la moitié qui compte le
 * plus, parce que c'est celle qui pourrit en silence — un harnais qui se saute rend vert un dossier
 * que personne n'a exercé, et `W20.i` a montré ce que ça coûte.
 *
 * L'autre moitié — les trois processus démarrent et s'arrêtent pour de vrai — est exercée par le job
 * `e2e` de la CI, qui a `podman`, les deux dépôts et les binaires construits. Elle **n'est pas**
 * simulée ici : un test qui monterait de faux processus prouverait que `spawn` fonctionne.
 *
 * Cette séparation est écrite plutôt que subie. `npm test` ne démarre rien ; il vérifie que le
 * harnais refuse correctement, ce qui est une propriété entière et testable seule.
 */

import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

import { HarnessFailure, WORKER_REPO_ENV, builtBinary, workerRepo } from "./harness.ts";

/** Un dépôt worker crédible : ce que `workerRepo` va chercher, et rien de plus. */
function depotWorker(): string {
  const racine = mkdtempSync(join(tmpdir(), "e2e-repo-"));
  mkdirSync(join(racine, "backend", "cli", "src"), { recursive: true });
  writeFileSync(join(racine, "backend", "cli", "src", "index.ts"), "// worker\n");
  return racine;
}

describe("le harnais e2e refuse plutôt que de se sauter — W12.f", () => {
  it("sans LOCUS_E2E_WORKER, il échoue en nommant la variable", () => {
    assert.throws(
      () => workerRepo({}),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.equal(erreur.subject, WORKER_REPO_ENV);
        // La formule qui compte : le message dit **pourquoi** se sauter serait pire.
        assert.match(erreur.message, /rendrait vert un dossier que personne n'a exercé/);
        return true;
      },
    );
  });

  it("une variable vide vaut une variable absente", () => {
    // Un `LOCUS_E2E_WORKER=""` traîne dans tous les environnements de CI mal remplis. Le lire comme
    // « renseignée » ferait chercher un dépôt à la racine du système, et l'erreur parlerait d'un
    // `index.ts` introuvable plutôt que de la variable — un diagnostic à deux étages pour une
    // cause à un seul.
    for (const valeur of ["", "   "]) {
      assert.throws(
        () => workerRepo({ [WORKER_REPO_ENV]: valeur }),
        (erreur: unknown) => erreur instanceof HarnessFailure && erreur.subject === WORKER_REPO_ENV,
      );
    }
  });

  it("un répertoire qui n'est pas un dépôt canterel est refusé, en disant ce qui manque", () => {
    const vide = mkdtempSync(join(tmpdir(), "e2e-vide-"));

    assert.throws(
      () => workerRepo({ [WORKER_REPO_ENV]: vide }),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.match(erreur.message, /backend\/cli\/src\/index\.ts/);
        return true;
      },
    );
  });

  it("un dépôt worker complet est accepté", () => {
    const racine = depotWorker();

    // Le pendant du test précédent. Une garde qui crierait aussi sur ce qui est juste se ferait
    // désactiver, et c'est le seul moyen de savoir qu'elle regarde la bonne chose.
    assert.equal(workerRepo({ [WORKER_REPO_ENV]: racine }), racine);
  });

  it("un binaire non construit est une panne qui dit comment le construire", () => {
    const racine = mkdtempSync(join(tmpdir(), "e2e-cible-"));

    assert.throws(
      () => builtBinary(racine, "locusd"),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.equal(erreur.subject, "locusd");
        // Un message qui dirait seulement « absent » laisserait chercher la commande.
        assert.match(erreur.message, /cargo build --bin locusd/);
        return true;
      },
    );
  });

  it("un binaire construit est trouvé", () => {
    const racine = mkdtempSync(join(tmpdir(), "e2e-cible-"));
    mkdirSync(join(racine, "target", "debug"), { recursive: true });
    writeFileSync(join(racine, "target", "debug", "locusd"), "");

    assert.equal(builtBinary(racine, "locusd"), join(racine, "target", "debug", "locusd"));
  });

  it("l'échec porte la sortie du processus, pas seulement son nom", () => {
    const erreur = new HarnessFailure(
      "locusd",
      "mort au démarrage (code 1)",
      "adresse déjà utilisée",
    );

    // Sans la sortie, un timeout de démarrage se lit « locusd n'a pas démarré » et se diagnostique
    // en relançant à la main. Avec elle, la cause est dans le message.
    assert.match(erreur.message, /adresse déjà utilisée/);
    assert.match(erreur.message, /--- sortie ---/);
  });

  it("une sortie vide n'ajoute pas de section vide au message", () => {
    const erreur = new HarnessFailure("locusd", "binaire absent", "   ");

    assert.doesNotMatch(erreur.message, /--- sortie ---/);
  });
});
