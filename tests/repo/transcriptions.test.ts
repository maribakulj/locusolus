import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  type Transcription,
  fixtureKeys,
  inspectTranscriptions,
  rustFields,
} from "../../tooling/transcriptions/transcriptions.ts";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const scratch: string[] = [];

after(async () => {
  await Promise.all(scratch.map((path) => rm(path, { recursive: true, force: true })));
});

const TRANSCRIPTION: Transcription = {
  nom: "la fixture d'essai",
  rust: "src/type.rs",
  structure: "Corps",
  fixture: "tests/fixture.ts",
  fonction: "corps",
};

/** Un dépôt de fixture : une structure Rust, et la fonction qui la transcrit. */
async function fixture(champs: readonly string[], cles: readonly string[]): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "transcriptions-"));
  scratch.push(root);
  await mkdir(join(root, "src"), { recursive: true });
  await mkdir(join(root, "tests"), { recursive: true });
  await writeFile(
    join(root, "src", "type.rs"),
    ["pub struct Corps {", ...champs.map((champ) => `    pub ${champ}: String,`), "}", ""].join(
      "\n",
    ),
    "utf8",
  );
  await writeFile(
    join(root, "tests", "fixture.ts"),
    [
      "function corps() {",
      "  return {",
      ...cles.map((cle) => `    ${cle}: "x",`),
      "  };",
      "}",
      "",
    ].join("\n"),
    "utf8",
  );
  return root;
}

/**
 * **Le dépôt lui-même.**
 *
 * La garde est écrite pour un défaut réel, et daté : en livrant `W25.a`, le champ `cognition` est
 * entré dans `mission::Proposal` et pas dans la fixture e2e, `npm run check` est resté vert parce
 * qu'il ne joue pas l'e2e, et la CI a rendu le `400 sur un champ oublié` que l'en-tête de la fixture
 * annonçait mot pour mot.
 */
test("le dépôt lui-même a des transcriptions à jour", async () => {
  const { findings, fields, examined } = await inspectTranscriptions(repoRoot);
  assert.deepEqual(findings, []);
  assert.ok(examined.length > 0, "au moins une transcription confrontée");
  assert.ok(fields > 10, `${fields} champs confrontés : la garde a cessé de les voir`);
});

/**
 * **La violation délibérée qui démontre la règle** — le sens dans lequel elle vient de tomber.
 */
test("un champ du type absent de la fixture est refusé", async () => {
  const root = await fixture(["statement", "cognition"], ["statement"]);
  const { findings } = await inspectTranscriptions(root, [TRANSCRIPTION]);

  assert.equal(findings.length, 1);
  const [seule] = findings;
  assert.ok(seule);
  assert.equal(seule.rule, "champ-non-transcrit");
  assert.match(seule.message, /cognition/u);
});

/**
 * **L'autre sens, et il est plus discret.**
 *
 * `serde` ignorerait en silence une clé que le type ne porte plus : le corps partirait, le daemon
 * l'accepterait, et le test e2e continuerait de passer en exerçant autre chose que ce qu'il annonce.
 * Une garde qui ne dirait que le premier sens serait exacte et à moitié utile.
 */
test("une cle de la fixture absente du type est refusee", async () => {
  const root = await fixture(["statement"], ["statement", "un_champ_retire"]);
  const { findings } = await inspectTranscriptions(root, [TRANSCRIPTION]);

  assert.equal(findings.length, 1);
  const [seule] = findings;
  assert.ok(seule);
  assert.equal(seule.rule, "champ-inconnu");
  assert.match(seule.message, /un_champ_retire/u);
});

/**
 * **Une structure introuvable n'est pas une transcription juste.**
 *
 * Si l'extraction cessait de fonctionner — un renommage, un changement d'indentation — la garde
 * n'aurait plus rien à comparer et rendrait « ok ». C'est la règle 3 du rythme de session appliquée
 * à l'outillage : distinguer « la réponse est vide » de « il n'y a pas eu de réponse ».
 */
test("une structure introuvable est signalee comme telle", async () => {
  const root = await fixture(["statement"], ["statement"]);
  const { findings, examined } = await inspectTranscriptions(root, [
    { ...TRANSCRIPTION, structure: "AutreChose" },
  ]);

  assert.equal(findings.length, 1);
  const [seule] = findings;
  assert.ok(seule);
  assert.equal(seule.rule, "structure-illisible");
  assert.deepEqual(examined, [], "rien n'a été confronté, et le rapport le dit");
});

/** Le symétrique, côté fixture. */
test("une fonction de fixture introuvable est signalee comme telle", async () => {
  const root = await fixture(["statement"], ["statement"]);
  const { findings } = await inspectTranscriptions(root, [
    { ...TRANSCRIPTION, fonction: "autreChose" },
  ]);

  assert.equal(findings.length, 1);
  const [seule] = findings;
  assert.ok(seule);
  assert.equal(seule.rule, "fixture-illisible");
});

/**
 * Les extracteurs lisent le **premier niveau** et rien d'autre.
 *
 * Un champ imbriqué — `resources: { cpu: 2.0 }` — n'est pas une clé de la requête, et le compter
 * ferait rougir la garde sur une différence qui n'en est pas une. C'est le genre de faux positif qui
 * fait désactiver une garde, et cette session en a rencontré trois en une journée.
 */
test("les extracteurs ne lisent que le premier niveau", () => {
  const champs = rustFields(
    ["pub struct Corps {", "    pub resources: Resources,", "}", ""].join("\n"),
    "Corps",
  );
  assert.deepEqual(champs, ["resources"]);

  const cles = fixtureKeys(
    [
      "function corps() {",
      "  return {",
      "    resources: { cpu: 2.0, memory_mb: 4096 },",
      "    network: 'deny',",
      "  };",
      "}",
      "",
    ].join("\n"),
    "corps",
  );
  assert.deepEqual(cles, ["resources", "network"]);
});
