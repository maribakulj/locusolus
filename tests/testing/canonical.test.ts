import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import test from "node:test";

import { canonicalize, payloadHash } from "@locus/testing";

/**
 * L'autre bord du corpus partagé — §7.7, `W20.ac`.
 *
 * `packages/lep/tests/canonical_corpus.rs` vérifie que la moitié **Rust** produit ce que le corpus
 * dit ; ce fichier-ci vérifie que la moitié **TypeScript** le produit encore. Les valeurs attendues
 * du corpus viennent d'elle : sans cette seconde garde, elle pourrait dériver et le corpus dériver
 * avec elle, en emmenant Rust — trois choses d'accord et toutes les trois fausses.
 *
 * C'est la même dissymétrie que pour le SDK vendoré : le producteur des attendus est le seul qui ne
 * peut pas se vérifier lui-même, donc c'est lui qu'on gèle.
 */

const corpus = JSON.parse(
  readFileSync(
    join(fileURLToPath(new URL("../..", import.meta.url)), "tests/fixtures/canonical.json"),
    "utf8",
  ),
) as { cas: { nom: string; valeur: unknown; canonique: string; empreinte: string }[] };

test("le corpus n'est pas vide", () => {
  // Un compteur qui n'a rien lu ne vaut pas zéro : un corpus vidé passerait chaque comparaison de
  // la boucle suivante sans en faire aucune.
  assert.ok(corpus.cas.length >= 8, `${corpus.cas.length} cas lus`);
});

for (const cas of corpus.cas) {
  test(`la forme canonique de « ${cas.nom} » n'a pas bougé`, () => {
    assert.equal(canonicalize(cas.valeur), cas.canonique);
    assert.equal(payloadHash(cas.valeur), cas.empreinte);
  });
}
