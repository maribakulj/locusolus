import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import test from "node:test";

import {
  LEP_DOCUMENTS,
  LEP_FEATURES,
  LEP_MECHANISMS,
  featureSince,
  negotiate,
  type CapabilityManifest,
  type MissionEnvelope,
  type SandboxAttestation,
} from "../../packages/lep/src/index.ts";
import { buildModel } from "../../tooling/sdk/ir.ts";
import { stripFixture } from "../../tooling/schemas/validate.ts";

const root = fileURLToPath(new URL("../..", import.meta.url));
const examples = join(root, "schemas/examples");

function body(name: string): unknown {
  return stripFixture(JSON.parse(readFileSync(join(examples, name), "utf8"))).body;
}

test("le modèle se construit sans angle mort", () => {
  // Toute construction de schéma que le générateur ne modélise pas est un `finding`. Un
  // générateur qui saute ce qu'il ne comprend pas produit des types qui ont l'air complets.
  const { findings } = buildModel(join(root, "schemas"));
  assert.deepEqual(findings, []);
});

//
// Round-trip — le test de sortie de W0.8, côté TypeScript.
//

test("chaque fixture fait un aller-retour exact", () => {
  // En TypeScript les types sont effacés, donc l'aller-retour porte sur ce qui reste vrai à
  // l'exécution : la valeur décodée puis ré-encodée est la même valeur. Ce qui est prouvé côté
  // Rust — qu'aucun champ ne disparaît faute d'être modélisé — l'est par la compilation ici,
  // et par le test de couverture des champs plus bas.
  const fixtures = [
    "capability-manifest.json",
    "capability-manifest-vm-linux.json",
    "mission-envelope.json",
    "mission-envelope-nominal.json",
    "sandbox-attestation.json",
    "event-reconnection-1-started.json",
    "event-reconnection-4-replay.json",
    "attempt-late-result.json",
    "attempt-budget-exceeded.json",
    "lease-expired.json",
  ];
  for (const name of fixtures) {
    const original = body(name);
    assert.deepEqual(JSON.parse(JSON.stringify(original)), original, name);
  }
});

test("les types générés couvrent tous les champs des fixtures", () => {
  // Le vrai risque en TypeScript n'est pas la perte à l'exécution, c'est un type qui oublie un
  // champ : rien ne casserait, et le champ deviendrait invisible au premier consommateur typé.
  const { model } = buildModel(join(root, "schemas"));
  const byName = new Map(model.structs.map((struct) => [struct.name, struct]));
  const cases: [string, string][] = [
    ["capability-manifest-vm-linux.json", "CapabilityManifest"],
    ["mission-envelope-nominal.json", "MissionEnvelope"],
    ["sandbox-attestation.json", "SandboxAttestation"],
    ["attempt-late-result.json", "Attempt"],
    ["lease-expired.json", "Lease"],
    ["event-reconnection-2-progress.json", "Event"],
  ];
  for (const [fixture, typeName] of cases) {
    const struct = byName.get(typeName);
    assert.ok(struct, `${typeName} absent du modèle`);
    const known = new Set(struct.fields.map((field) => field.name));
    for (const key of Object.keys(body(fixture) as Record<string, unknown>)) {
      assert.ok(known.has(key), `${typeName} ne modélise pas \`${key}\` (vu dans ${fixture})`);
    }
  }
});

test("les documents du registre sont exposés par le SDK", () => {
  assert.ok(LEP_DOCUMENTS.length > 0);
  for (const name of ["MissionEnvelope", "CapabilityManifest", "Event"]) {
    assert.ok((LEP_DOCUMENTS as readonly string[]).includes(name), name);
  }
});

test("les types s'utilisent réellement sur une fixture", () => {
  // Une assertion de type qui ne compile pas ferait échouer `tsc`, donc ce test vaut autant à la
  // compilation qu'à l'exécution.
  const manifest = body("capability-manifest-vm-linux.json") as CapabilityManifest;
  assert.equal(manifest.platform.os, "linux");
  assert.ok(manifest.sandbox.levels.includes("S3"));

  const mission = body("mission-envelope-nominal.json") as MissionEnvelope;
  assert.equal(mission.sandbox.minimum_level, "S3");
  assert.equal(mission.budget.max_model_calls, 30);

  const attestation = body("sandbox-attestation.json") as SandboxAttestation;
  assert.equal(attestation.host_home_mounted, false);
  assert.equal(attestation.self_tests.read_host_home, "blocked");
});

//
// Négociation de features au handshake.
//

test("une feature tenue des deux côtés est accordée", () => {
  const agreed = negotiate(["late-results", "human-input"], ["late-results", "pull-queue"]);
  assert.deepEqual(agreed.features, ["late-results"]);
  assert.deepEqual(agreed.declined, ["human-input"]);
  assert.deepEqual(agreed.unknown, []);
});

test("refusée et inconnue sont deux signaux différents", () => {
  // Les fondre en un seul « non » rendrait un pair venu d'un mineur ultérieur indiscernable
  // d'un pair qui a mal orthographié son besoin. Le premier appelle un repli, le second un
  // rapport d'erreur.
  const agreed = negotiate(["human-input", "telepathy"], ["late-results"]);
  assert.deepEqual(agreed.declined, ["human-input"]);
  assert.deepEqual(agreed.unknown, ["telepathy"]);
});

test("la négociation est stable et sans doublon", () => {
  const agreed = negotiate(
    ["pull-queue", "late-results", "pull-queue"],
    ["late-results", "pull-queue"],
  );
  assert.deepEqual(agreed.features, ["late-results", "pull-queue"]);
});

test("aucune feature accordée quand le pair n'en annonce aucune", () => {
  const agreed = negotiate(Object.keys(LEP_FEATURES), []);
  assert.deepEqual(agreed.features, []);
  assert.equal(agreed.declined.length, Object.keys(LEP_FEATURES).length);
});

test("chaque feature déclare le mineur qui l'introduit", () => {
  for (const name of Object.keys(LEP_FEATURES)) {
    assert.match(featureSince(name) ?? "", /^1\.\d+$/, name);
  }
  assert.equal(featureSince("telepathy"), undefined);
});

// ---------------------------------------------------------------------------------------------
// Le registre des mécanismes — `W5.ag`, ADR 0035 décision 3.
// ---------------------------------------------------------------------------------------------

test("le registre des mécanismes ne porte ni doublon ni nom vide", () => {
  // Un doublon ne casserait rien et ne ferait rien : le nom serait connu deux fois, et la seule
  // trace en serait une ligne de plus dans un fichier que personne ne relit. Un nom vide serait
  // pire — `backend` est `minLength: 1` dans les deux schémas, donc il ne se rapprocherait d'aucun
  // document valide tout en donnant au registre l'air de le connaître.
  assert.equal(new Set(LEP_MECHANISMS).size, LEP_MECHANISMS.length);
  for (const name of LEP_MECHANISMS) assert.ok(name.length > 0, JSON.stringify(name));
});

test("le registre nomme le mécanisme que chaque manifeste du corpus annonce", () => {
  // Le registre est la moitié qui rend le rapprochement décidable ; le corpus est ce sur quoi la
  // chaîne s'exécute. Un manifeste dont le mécanisme sort du registre n'est pas invalide — c'est
  // délibéré, `lep/1.0` laisse `backend` libre — mais il ne peut plus tirer d'aucune attestation,
  // et le savoir ici vaut mieux que de le découvrir sur un refus de placement.
  const inconnus = [];
  for (const nom of ["capability-manifest.json", "capability-manifest-vm-linux.json"]) {
    const manifeste = stripFixture(JSON.parse(readFileSync(join(examples, nom), "utf8")))
      .body as CapabilityManifest;
    const backend = manifeste.sandbox.backend;
    if (backend !== undefined && !(LEP_MECHANISMS as readonly string[]).includes(backend)) {
      inconnus.push(`${nom} → « ${backend} »`);
    }
  }
  assert.deepEqual(inconnus, ["capability-manifest-vm-linux.json → « rootless-oci »"]);
});
