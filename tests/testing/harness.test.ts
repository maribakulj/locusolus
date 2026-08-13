import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import test from "node:test";

import type { CapabilityManifest, Event, Lease, MissionEnvelope } from "@locus/lep";
import {
  canonicalize,
  payloadHash,
  runConformance,
  VERIFICATIONS,
  type WorkerUnderTest,
} from "@locus/testing";
import { stripFixture } from "../../tooling/schemas/validate.ts";

const examples = join(fileURLToPath(new URL("../..", import.meta.url)), "schemas/examples");

function fixture<T>(name: string): T {
  return stripFixture(JSON.parse(readFileSync(join(examples, name), "utf8"))).body as T;
}

const MISSION = fixture<MissionEnvelope>("mission-envelope-nominal.json");
const CAPABLE = fixture<CapabilityManifest>("capability-manifest-vm-linux.json");
const UNDERPOWERED = fixture<CapabilityManifest>("capability-manifest.json");
const LEASE = fixture<Lease>("lease-expired.json");

function event(sequence: number, type: Event["event_type"], extra: Partial<Event> = {}): Event {
  return {
    protocol: "lep/1.0",
    event_type: type,
    sequence,
    occurred_at: "2026-08-13T09:30:00.000Z",
    idempotency_key: `idem-${sequence}`,
    task_id: MISSION.task_id,
    attempt: 1,
    ...extra,
  };
}

/** Un worker factice qui fait tout bien. Le point de comparaison de tous les autres. */
function conformant(events?: readonly Event[]): WorkerUnderTest {
  return {
    register: () => CAPABLE,
    offer: () => true,
    events: () =>
      events ?? [
        event(1, "attempt.started"),
        event(2, "heartbeat"),
        event(3, "progress"),
        event(4, "attempt.completed"),
      ],
  };
}

//
// JSON canonique — la dette de W0.8.
//

test("4 et 4.0 canonicalisent pareil", () => {
  // Toute la raison d'être du canonicaliseur : deux pairs conformes écrivent le même nombre
  // autrement, et un hash calculé sur leur sortie divergerait sur rien.
  assert.equal(canonicalize({ cpu: 4 }), canonicalize({ cpu: 4.0 }));
  assert.equal(canonicalize({ cpu: 4.0 }), '{"cpu":4}');
  assert.equal(payloadHash({ cpu: 4 }), payloadHash({ cpu: 4.0 }));
});

test("l'ordre des clés ne change pas le hash", () => {
  assert.equal(payloadHash({ b: 1, a: 2 }), payloadHash({ a: 2, b: 1 }));
  assert.equal(canonicalize({ b: 1, a: 2 }), '{"a":2,"b":1}');
});

test("le tri des clés est récursif et traverse les tableaux", () => {
  assert.equal(canonicalize({ z: [{ b: 1, a: 2 }] }), '{"z":[{"a":2,"b":1}]}');
});

test("une valeur non représentable s'arrête au lieu de rendre un hash faux", () => {
  // Un canonicaliseur qui rend quelque chose pour ce qu'il ne sait pas représenter produit un
  // hash, et un hash faux ressemble en tout point à un hash juste.
  assert.throws(() => canonicalize({ x: Number.NaN }), RangeError);
  assert.throws(() => canonicalize({ x: Number.POSITIVE_INFINITY }), RangeError);
  assert.throws(() => canonicalize({ x: 2 ** 53 }), RangeError);
});

test("undefined disparaît au lieu de produire du JSON invalide", () => {
  assert.equal(canonicalize({ a: 1, b: undefined }), '{"a":1}');
});

test("le hash porte son algorithme", () => {
  assert.match(payloadHash({}), /^sha256:[0-9a-f]{64}$/);
});

//
// Le harnais contre un worker factice — le test de sortie de W0.9.
//

test("un worker conforme ne produit aucun constat", async () => {
  const report = await runConformance(conformant(), MISSION, LEASE);
  assert.deepEqual(report.findings, []);
  // Et le rapport dit ce qui a tourné : « rien à signaler » et « rien vérifié » ne doivent pas
  // se ressembler.
  assert.equal(report.ran.length, VERIFICATIONS.length);
});

test("un worker qui accepte au-dessus de ses moyens est pris", async () => {
  // La faute exacte que la paire de refus du corpus existe pour attraper : mission S3, worker
  // macOS Seatbelt qui n'offre que S1/S2.
  const overreaching: WorkerUnderTest = { ...conformant(), register: () => UNDERPOWERED };
  const report = await runConformance(overreaching, MISSION, LEASE);
  assert.equal(report.findings.length, 1);
  assert.equal(report.findings[0]?.rule, "admission");
});

test("refuser une mission n'est pas une faute", async () => {
  // La politique locale d'un worker peut être plus restrictive que son manifeste (§10.2). Un
  // worker qui accepte tout est le vrai défaut.
  const prudent: WorkerUnderTest = {
    ...conformant(),
    register: () => UNDERPOWERED,
    offer: () => false,
  };
  const report = await runConformance(prudent, MISSION, LEASE);
  assert.deepEqual(report.findings, []);
});

test("un événement avant `attempt.started` est pris", async () => {
  const eager = conformant([event(1, "progress"), event(2, "attempt.started")]);
  const report = await runConformance(eager, MISSION, LEASE);
  assert.ok(report.findings.some((f) => f.rule === "sequence" && f.message.includes("progress")));
});

test("une séquence qui recule est prise, un rejeu ne l'est pas", async () => {
  const backwards = conformant([
    event(1, "attempt.started"),
    event(3, "heartbeat"),
    event(2, "progress"),
    event(4, "attempt.completed"),
  ]);
  assert.ok(
    (await runConformance(backwards, MISSION, LEASE)).findings.some((f) => f.rule === "sequence"),
  );

  // Le rejeu de W0.7 : même séquence, même clé. Le harnais doit le laisser passer, sinon il
  // interdirait la reprise de stream que §12.4 exige.
  const replayed = conformant([
    event(1, "attempt.started"),
    event(2, "heartbeat"),
    event(3, "progress"),
    event(3, "progress"),
    event(4, "attempt.completed"),
  ]);
  assert.deepEqual((await runConformance(replayed, MISSION, LEASE)).findings, []);
});

test("une séquence réutilisée avec une autre clé est prise", async () => {
  const colliding = conformant([
    event(1, "attempt.started"),
    event(2, "heartbeat"),
    event(3, "progress"),
    event(3, "progress", { idempotency_key: "autre" }),
    event(4, "attempt.completed"),
  ]);
  const report = await runConformance(colliding, MISSION, LEASE);
  assert.ok(report.findings.some((f) => f.message.includes("autre clé d'idempotence")));
});

test("la règle heartbeat < TTL/3 est vérifiée là où le schéma ne pouvait pas", async () => {
  // La dette héritée de W0.6, honorée ici.
  const tooSlow: Lease = { ...LEASE, ttl_seconds: 300, heartbeat_interval_seconds: 200 };
  const report = await runConformance(conformant(), MISSION, tooSlow);
  assert.ok(report.findings.some((f) => f.rule === "heartbeat"));

  const exact: Lease = { ...LEASE, ttl_seconds: 300, heartbeat_interval_seconds: 100 };
  assert.ok(
    (await runConformance(conformant(), MISSION, exact)).findings.some(
      (f) => f.rule === "heartbeat",
    ),
    "un tiers pile est déjà trop lent : §12.3 dit strictement inférieur",
  );

  const fine: Lease = { ...LEASE, ttl_seconds: 300, heartbeat_interval_seconds: 60 };
  assert.deepEqual((await runConformance(conformant(), MISSION, fine)).findings, []);
});

test("un attempt sans aucun heartbeat est pris", async () => {
  const silent = conformant([event(1, "attempt.started"), event(2, "attempt.completed")]);
  const report = await runConformance(silent, MISSION, LEASE);
  assert.ok(report.findings.some((f) => f.rule === "heartbeat"));
});

test("un payload_hash non canonique est pris", async () => {
  const payload = { cpu: 4, tool: "lake" };
  const honest = conformant([
    event(1, "attempt.started"),
    event(2, "heartbeat"),
    event(3, "tool.completed", { payload, payload_hash: payloadHash(payload) }),
    event(4, "attempt.completed"),
  ]);
  assert.deepEqual((await runConformance(honest, MISSION, LEASE)).findings, []);

  const sloppy = conformant([
    event(1, "attempt.started"),
    event(2, "heartbeat"),
    event(3, "tool.completed", { payload, payload_hash: `sha256:${"0".repeat(64)}` }),
    event(4, "attempt.completed"),
  ]);
  const report = await runConformance(sloppy, MISSION, LEASE);
  assert.ok(report.findings.some((f) => f.rule === "payload-hash"));
});

test("un attempt qui se termine deux fois, ou qui parle après sa fin, est pris", async () => {
  const twice = conformant([
    event(1, "attempt.started"),
    event(2, "heartbeat"),
    event(3, "attempt.completed"),
    event(4, "attempt.failed"),
  ]);
  const report = await runConformance(twice, MISSION, LEASE);
  assert.ok(report.findings.some((f) => f.rule === "lifecycle"));
});

test("un résultat tardif qui ne se déclare pas est pris", async () => {
  // LEASE expire à 10:00:00 ; l'attempt rend à 10:07:00 sans le dire.
  const late = conformant([
    event(1, "attempt.started"),
    event(2, "heartbeat"),
    event(3, "attempt.completed", { occurred_at: "2026-08-13T10:07:00.000Z" }),
  ]);
  const report = await runConformance(late, MISSION, LEASE);
  assert.ok(report.findings.some((f) => f.rule === "late-result"));

  const declared = conformant([
    event(1, "attempt.started"),
    event(2, "heartbeat"),
    event(3, "attempt.completed", {
      occurred_at: "2026-08-13T10:07:00.000Z",
      payload: { late: true },
    }),
  ]);
  assert.deepEqual((await runConformance(declared, MISSION, LEASE)).findings, []);
});

test("chaque vérification porte un identifiant et un énoncé", async () => {
  // Un harnais dont on ne peut pas citer les règles ne se conteste pas.
  for (const verification of VERIFICATIONS) {
    assert.match(verification.id, /^[a-z0-9-]+$/);
    assert.ok(verification.statement.length > 20, verification.id);
  }
  assert.equal(new Set(VERIFICATIONS.map((v) => v.id)).size, VERIFICATIONS.length);
});
