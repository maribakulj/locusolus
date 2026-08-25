/**
 * Le pin du worker — `W12.g`, ADR 0033 décision 1.
 *
 * # Ce que ces tests protègent
 *
 * L'ADR décidait l'épinglage **deux fois** — titre de la décision 1, et conséquences — et le
 * workflow n'avait aucun `ref:`. Une décision écrite et non appliquée ne se voit pas : le
 * commentaire du workflow affirmait même « le worker réel, à une révision épinglée », juste
 * au-dessus de l'étape qui prenait `main`.
 *
 * Ce que ces tests tiennent est donc moins le pin lui-même que ce qui l'empêche de redevenir une
 * affirmation : le fichier existe, il porte un SHA **complet**, et son absence **refuse** au lieu
 * de retomber sur `main`.
 */

import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

import { HarnessFailure } from "./harness.ts";
import { PIN_FILE, workerPin } from "./pin.ts";

/** Un répertoire portant le pin qu'on lui donne. */
function avecPin(contenu: string): string {
  const racine = mkdtempSync(join(tmpdir(), "e2e-pin-"));
  writeFileSync(join(racine, PIN_FILE), contenu);
  return racine;
}

describe("le pin du worker — W12.g", () => {
  /**
   * **Le pin du dépôt est lisible, et porte un SHA complet.**
   *
   * Le fichier réel, pas une fixture : ce que ce test garantit est que *ce* dépôt-ci est épinglé,
   * et un test sur un contenu inventé ne dirait rien de cela.
   */
  it("le dépôt porte un pin, avec un SHA complet", () => {
    const pin = workerPin();

    assert.match(pin.commit, /^[0-9a-f]{40}$/);
    assert.match(pin.repo, /canterel/);
  });

  /**
   * **Un pin absent refuse, il ne retombe pas sur `main`.**
   *
   * C'est la propriété qui compte. Un repli silencieux rendrait vert un dossier monté contre une
   * révision que personne n'a choisie, et l'épinglage n'existerait plus que dans un commentaire —
   * précisément l'état d'où l'on vient.
   */
  it("un pin absent est une panne, jamais un repli", () => {
    const vide = mkdtempSync(join(tmpdir(), "e2e-sans-pin-"));

    assert.throws(
      () => workerPin(vide),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.equal(erreur.subject, PIN_FILE);
        assert.match(erreur.message, /dépendrait/);
        return true;
      },
    );
  });

  /**
   * **Une branche ou un tag ne sont pas un pin.**
   *
   * Les deux se **déplacent**, ce qui est exactement ce que l'épinglage existe pour empêcher.
   * Accepter `main` ici rendrait le fichier décoratif : il aurait l'air d'épingler et ne le ferait
   * pas, ce qui est pire que son absence.
   */
  it("une branche n'est pas une révision", () => {
    for (const mouvant of ["main", "v1.0.0", "HEAD"]) {
      assert.throws(
        () => workerPin(avecPin(`{"repo":"https://example/canterel","commit":"${mouvant}"}`)),
        (erreur: unknown) => erreur instanceof HarnessFailure && erreur.subject === PIN_FILE,
        `« ${mouvant} » se déplace, donc n'épingle rien`,
      );
    }
  });

  /**
   * **Un SHA court n'est pas un pin non plus.**
   *
   * Il peut devenir ambigu — deux objets partageant un préfixe —, et git résout alors autre chose
   * que ce qui avait été choisi. Séparé du test précédent parce que la raison diffère : une branche
   * bouge, un SHA court devient équivoque.
   */
  it("un SHA court n'épingle pas de façon univoque", () => {
    assert.throws(
      () => workerPin(avecPin('{"repo":"https://example/canterel","commit":"d9cc724"}')),
      (erreur: unknown) => erreur instanceof HarnessFailure,
    );
  });

  /**
   * **Un fichier illisible refuse, et le dit.**
   */
  it("un pin mal formé refuse", () => {
    assert.throws(
      () => workerPin(avecPin("ceci n'est pas du JSON")),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.match(erreur.message, /ne se relit pas/);
        return true;
      },
    );
  });

  /**
   * **Un pin sans dépôt refuse.**
   *
   * Le dépôt est en clair dans le fichier pour qu'un lecteur sache de quoi parle le SHA sans ouvrir
   * le workflow. Un pin qui l'omettrait laisserait quarante caractères hexadécimaux sans sujet.
   */
  it("un pin sans dépôt refuse", () => {
    assert.throws(
      () => workerPin(avecPin('{"commit":"d9cc7244189a9463931b6904e071bd7cdcb49ce1"}')),
      (erreur: unknown) => erreur instanceof HarnessFailure,
    );
  });
});
