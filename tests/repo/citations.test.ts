import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import { inspectCitations, sections } from "../../tooling/citations/citations.ts";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const scratch: string[] = [];

after(async () => {
  await Promise.all(scratch.map((path) => rm(path, { recursive: true, force: true })));
});

/** Un dépôt de fixture : un spec, et les documents qu'on veut lui confronter. */
async function fixture(spec: string, docs: Record<string, string>): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "citations-"));
  scratch.push(root);
  await mkdir(join(root, "docs", "adr"), { recursive: true });
  await writeFile(join(root, "docs", "SPEC_V1.md"), spec, "utf8");
  await writeFile(join(root, "CLAUDE.md"), "", "utf8");
  for (const [path, body] of Object.entries(docs)) {
    await writeFile(join(root, path), body, "utf8");
  }
  return root;
}

const SPEC = [
  "## 12. Workers",
  "### 12.4 Backpressure",
  "## 16. Mémoire",
  "### 16.6 Contamination",
].join("\n");

/**
 * **Le dépôt lui-même.**
 *
 * La garde est écrite pour un défaut réel, trouvé dans ces documents-ci : `docs/10_V1_ROADMAP.md` et
 * l'ADR 0026 citaient « §12.4 (isolation de branche) », alors que le §12.4 de ce dépôt s'appelle
 * *Backpressure* et que la section visée est celle du spec du **worker**.
 */
test("le dépôt lui-même n'a aucune citation nue vers une section absente", async () => {
  const { findings, citations } = await inspectCitations(repoRoot);
  assert.deepEqual(findings, []);
  assert.ok(citations > 100, `${citations} citations confrontées : la garde a cessé de les voir`);
});

/**
 * **La violation délibérée qui démontre la règle.**
 *
 * Sans elle, une garde qui ne reconnaîtrait plus rien rendrait « ok » exactement comme une garde qui
 * a tout vérifié — c'est ce que `W22.a` a payé, et ce que le décompte imprimé sert à distinguer.
 */
test("une citation nue vers une section absente est refusée", async () => {
  const root = await fixture(SPEC, {
    "docs/note.md": "Le mécanisme casse §12.9 et §16.6.\n",
  });
  const { findings } = await inspectCitations(root);
  assert.equal(findings.length, 1);
  const [seule] = findings;
  assert.ok(seule);
  assert.equal(seule.rule, "citation-sans-section");
  assert.equal(seule.where, "docs/note.md:1");
  assert.match(seule.message, /§12\.9/u);
});

/**
 * Une citation **qualifiée** ne rougit pas, quel que soit le numéro.
 *
 * C'est la moitié de la règle qui la rend vivable : elle n'exige pas que toute citation vise le spec
 * local, elle exige qu'une citation dise **où chercher**. Les deux formes en usage dans ce dépôt sont
 * couvertes — un renvoi inter-dépôts qui nomme son fichier, et un ADR qui cite ses propres sections.
 */
test("une citation qui nomme son document est admise", async () => {
  const root = await fixture(SPEC, {
    "docs/note.md": [
      "L'isolation vit dans `repos/canterel/SPEC_V1.md` §12.4, pas ici.",
      "La tranche 1 (ADR 0017 §5.1) est livrée.",
    ].join("\n"),
  });
  assert.deepEqual((await inspectCitations(root)).findings, []);
});

/**
 * **Un spec illisible n'est pas un spec sans faute.**
 *
 * Si la lecture des titres cessait de fonctionner, `sections` rendrait une carte vide, *toutes* les
 * citations deviendraient fautives, et le rapport se lirait comme une avalanche de fautes plutôt que
 * comme « je n'ai pas su lire le spec ». La garde préfère le dire une fois, sous son propre nom.
 *
 * C'est la règle 3 du rythme de session appliquée à l'outillage : une garde bâtie sur une lecture
 * distingue « la réponse est zéro » de « il n'y a pas eu de réponse ».
 */
test("un spec dont aucune section ne se lit est signalé comme tel", async () => {
  const root = await fixture("du texte sans aucun titre numéroté\n", {
    "docs/note.md": "voir §12.4\n",
  });
  const { findings, citations } = await inspectCitations(root);
  assert.equal(findings.length, 1);
  const [seule] = findings;
  assert.ok(seule);
  assert.equal(seule.rule, "spec-illisible");
  assert.equal(citations, 0, "rien n'a été confronté, et le compteur le dit");
});

/**
 * Les titres se lisent aux trois profondeurs, avec ou sans point après le numéro.
 *
 * Les deux specs du chantier emploient les deux formes, et une lecture qui n'en verrait qu'une
 * déclarerait absentes des sections qui existent — c'est-à-dire produirait exactement le faux positif
 * qui fait désactiver une garde.
 */
test("les titres numérotés se lisent dans leurs formes réelles", () => {
  const lues = sections(
    ["## 7. Domaine", "### 7.1 Agrégats", "#### 7.1.2 Détail", "### 12.4 Backpressure"].join("\n"),
  );
  assert.deepEqual([...lues.keys()], ["7", "7.1", "7.1.2", "12.4"]);
  assert.equal(lues.get("12.4"), "Backpressure");
});
