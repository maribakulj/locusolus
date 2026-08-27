import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { compareWithSource, readWorkerPin, WORKER_PIN } from "../../tooling/repo/worker-pin.ts";

const run = promisify(execFile);
const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const checker = join(repoRoot, "tooling/repo/check-worker-pin.ts");
const SDK = "packages/lep/src/generated.ts";
const VENDU = "backend/cli/src/locus/lep/generated.ts";
const scratch: string[] = [];

after(async () => {
  await Promise.all(scratch.map((path) => rm(path, { recursive: true, force: true })));
});

/** Un faux dépôt worker dont le pin dit ce que le test veut lui faire dire. */
async function fauxWorker(pin: unknown): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "locus-worker-pin-"));
  scratch.push(root);
  await mkdir(join(root, "backend/cli/src/locus/lep"), { recursive: true });
  await writeFile(
    join(root, WORKER_PIN),
    typeof pin === "string" ? pin : `${JSON.stringify(pin, null, 2)}\n`,
  );
  return root;
}

/** L'empreinte du SDK **tel qu'il est**, lue et non recopiée : c'est elle qui fait foi. */
async function empreinteDuSdk(): Promise<string> {
  const source = await readFile(join(repoRoot, SDK), "utf8");
  return createHash("sha256").update(source, "utf8").digest("hex");
}

function pinAvec(sha: string, source = SDK): Record<string, unknown> {
  return {
    repo: "https://github.com/maribakulj/locusolus",
    commit: "0".repeat(40),
    files: { [VENDU]: { source, sha256_source: sha, sha256_vendored: sha } },
  };
}

/**
 * **La moitié qui compte : un pin périmé rougit, et le message dit où aller.**
 *
 * C'est le défaut qui a vécu — `W16.d` a changé le SDK ici, `canterel` ne l'a pas suivi, et le
 * worker a ignoré trois choses que ce plan de contrôle émettait. La garde n'existe que pour ce cas,
 * et un test qui ne l'exercerait pas la laisserait verte sur un dépôt sans consommateur.
 */
test("une empreinte périmée est une lecture périmée, et le message nomme le fichier", async () => {
  const worker = await fauxWorker(pinAvec("f".repeat(64)));
  const findings = await compareWithSource(await readWorkerPin(worker), repoRoot, worker);

  assert.equal(findings.length, 1);
  assert.equal(findings[0]?.rule, "lecture-perimee");
  assert.equal(findings[0]?.where, SDK);
  // Le remède, pas seulement le constat : « pin périmé » nu enverrait relire quatre dépôts.
  assert.match(findings[0]?.message ?? "", /re-vendorer/i);
  assert.match(findings[0]?.message ?? "", /WORKER-PINNED\.json/);
});

/** Et la moitié qui empêche de tout rejeter : à jour, la garde ne dit rien. */
test("une empreinte à jour ne produit aucun constat", async () => {
  const worker = await fauxWorker(pinAvec(await empreinteDuSdk()));
  assert.deepEqual(await compareWithSource(await readWorkerPin(worker), repoRoot, worker), []);
});

/**
 * **Une source disparue n'est pas une empreinte différente.**
 *
 * Les deux se réparent ailleurs : l'une en re-vendorant, l'autre en corrigeant la table de
 * réécritures du consommateur. Les fondre en un seul motif enverrait la moitié des cas au mauvais
 * endroit — c'est la règle que `unplaced_note` applique déjà aux sept motifs de §10.2.
 */
test("une source qui n'existe plus ici porte son propre motif", async () => {
  const worker = await fauxWorker(pinAvec("0".repeat(64), "packages/lep/src/disparu.ts"));
  const findings = await compareWithSource(await readWorkerPin(worker), repoRoot, worker);

  assert.equal(findings.length, 1);
  assert.equal(findings[0]?.rule, "source-absente");
  assert.equal(findings[0]?.where, "packages/lep/src/disparu.ts");
});

/**
 * **Ce qui n'a pas été lu ne vaut pas zéro.**
 *
 * Six façons de ne rien lire, et toutes rendent `pin-illisible` plutôt qu'un tableau vide. La
 * dernière est la plus insidieuse : un pin syntaxiquement parfait qui n'épingle **aucun** fichier
 * passerait toutes les comparaisons — zéro fichier, zéro écart —, et la garde rendrait `ok` sur un
 * consommateur qu'elle n'a pas vérifié.
 */
test("un pin absent, malformé, incomplet ou vide se dit, et ne passe pas", async () => {
  const vide = await mkdtemp(join(tmpdir(), "locus-worker-pin-"));
  scratch.push(vide);

  const cas: readonly (readonly [string, string])[] = [
    ["absent", vide],
    ["pas du JSON", await fauxWorker("{ ceci n'est pas du JSON")],
    ["sans commit", await fauxWorker({ files: { [VENDU]: { source: SDK, sha256_source: "x" } } })],
    ["sans table files", await fauxWorker({ commit: "0".repeat(40) })],
    ["entrée incomplète", await fauxWorker({ commit: "0".repeat(40), files: { [VENDU]: {} } })],
    ["table vide", await fauxWorker({ commit: "0".repeat(40), files: {} })],
  ];

  for (const [quoi, worker] of cas) {
    const reading = await readWorkerPin(worker);
    assert.equal(reading.kind, "illisible", `« ${quoi} » aurait dû être illisible`);
    const findings = await compareWithSource(reading, repoRoot, worker);
    assert.equal(findings.length, 1, `« ${quoi} » n'a produit aucun constat`);
    assert.equal(findings[0]?.rule, "pin-illisible");
  }
});

/**
 * **Les deux modes de la commande, et pourquoi l'un rend `0` sur une absence.**
 *
 * Chemin donné : le checkout est garanti par le job qui appelle, donc son absence est une panne.
 * Chemin absent : une machine qui n'a qu'un des quatre dépôts est ordinaire, et rendre `1` ferait
 * retirer la garde dans la semaine. La distinction n'est pas la rigueur, c'est ce qui est su — et
 * le mode dégradé le **dit**, au lieu de laisser « ok » se lire « tout est vérifié ».
 */
test("chemin donné : strict ; chemin absent : non exécutée, et dit comme tel", async () => {
  const vide = await mkdtemp(join(tmpdir(), "locus-worker-pin-"));
  scratch.push(vide);

  const strict = await run("node", [checker, vide]).catch(
    (error: { code?: number; stderr?: string }) => error,
  );
  assert.equal((strict as { code?: number }).code, 1, "un chemin donné sans pin doit échouer");
  assert.match((strict as { stderr?: string }).stderr ?? "", /pin-illisible/);

  // Le mode dégradé s'observe en déplaçant la racine : la commande cherche `../canterel` **à côté
  // du dépôt**, et un répertoire temporaire n'en a pas.
  const seul = await run("node", [checker], { cwd: vide });
  assert.match(seul.stdout, /non exécutée|épingle/);
});
