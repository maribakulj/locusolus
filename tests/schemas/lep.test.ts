import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import test from "node:test";

import {
  compile,
  inspectSchemas,
  readRegistry,
  stripFixture,
} from "../../tooling/schemas/validate.ts";

const root = fileURLToPath(new URL("../..", import.meta.url));
const schemasDir = join(root, "schemas");
const registry = readRegistry(schemasDir);
const { ajv, findings: compileFindings } = compile(schemasDir, registry);

/** A mission the schema accepts, to be broken one field at a time. */
function mission(): Record<string, unknown> {
  const raw = JSON.parse(
    readFileSync(join(schemasDir, "examples/mission-envelope-nominal.json"), "utf8"),
  );
  return stripFixture(raw).body as Record<string, unknown>;
}

function capability(): Record<string, unknown> {
  const raw = JSON.parse(
    readFileSync(join(schemasDir, "examples/capability-manifest-vm-linux.json"), "utf8"),
  );
  return stripFixture(raw).body as Record<string, unknown>;
}

function accepts(id: string, document: unknown): boolean {
  const validate = ajv.getSchema(id);
  assert.ok(validate, `aucun schéma sous ${id}`);
  return validate(document) as boolean;
}

const MISSION = "urn:locus:schema:lep:1.0:mission-envelope";
const CAPABILITY = "urn:locus:schema:lep:1.0:capability-manifest";

test("every schema the registry names compiles", () => {
  assert.deepEqual(compileFindings, []);
});

test("the repository's own examples satisfy the contract", () => {
  assert.deepEqual(inspectSchemas(root), []);
});

test("the fixture block is metadata and never reaches the schema", () => {
  const raw = JSON.parse(
    readFileSync(join(schemasDir, "examples/mission-envelope-nominal.json"), "utf8"),
  );
  assert.ok("_fixture" in raw);
  const { body, expect } = stripFixture(raw);
  assert.equal(expect, "accepted");
  assert.ok(!("_fixture" in (body as Record<string, unknown>)));
  // And the schema would have rejected it: the fixture block is not a LEP field, so leaving it
  // in would quietly test a document nobody ever sends.
  assert.equal(accepts(MISSION, body), true);
});

//
// A schema that accepts everything passes every fixture. These are the tests that say no.
//

test("a mission missing any required field is refused", () => {
  for (const field of [
    "protocol",
    "task_id",
    "attempt_id",
    "branch_id",
    "objective",
    "context_view",
    "environment",
    "sandbox",
    "resources",
    "budget",
    "output_contract",
  ]) {
    const broken = mission();
    delete broken[field];
    assert.equal(accepts(MISSION, broken), false, `${field} devrait être obligatoire`);
  }
});

test("the protocol field accepts the 1.x line and refuses the next major", () => {
  for (const version of ["lep/1.0", "lep/1.1", "lep/1.12"]) {
    assert.equal(accepts(MISSION, { ...mission(), protocol: version }), true, version);
  }
  for (const version of ["lep/2.0", "lep/1", "1.0", "lep/1.0-beta"]) {
    assert.equal(accepts(MISSION, { ...mission(), protocol: version }), false, version);
  }
});

test("a context hash must be a whole digest, not a placeholder", () => {
  const bad = ["sha256:...", "sha256:abc", "b7a4c0ce", `sha512:${"a".repeat(64)}`];
  for (const hash of bad) {
    const broken = mission();
    broken.context_view = { id: "ctx-example", hash };
    assert.equal(accepts(MISSION, broken), false, hash);
  }
  const good = mission();
  good.context_view = { id: "ctx-example", hash: `sha512:${"a".repeat(128)}` };
  assert.equal(accepts(MISSION, good), true);
});

test("every budget bound is required, because one free bound hides the overrun", () => {
  for (const bound of ["max_model_calls", "max_input_tokens", "max_output_tokens"]) {
    const broken = mission();
    broken.budget = { ...(mission().budget as object) };
    delete (broken.budget as Record<string, unknown>)[bound];
    assert.equal(accepts(MISSION, broken), false, bound);
  }
});

test("a reservation of nothing is not a reservation", () => {
  for (const [field, value] of [
    ["cpu", 0],
    ["cpu", -1],
    ["memory_mb", 0],
    ["disk_mb", -100],
    ["wall_time_seconds", 0],
  ] as const) {
    const broken = mission();
    broken.resources = { ...(mission().resources as object), [field]: value };
    assert.equal(accepts(MISSION, broken), false, `${field}=${value}`);
  }
});

test("sandbox levels come from the spec's ladder, spelled its way", () => {
  for (const level of ["S0", "S3", "S5"]) {
    const ok = mission();
    ok.sandbox = { minimum_level: level, network: "deny" };
    assert.equal(accepts(MISSION, ok), true, level);
  }
  for (const level of ["S6", "s3", "3", "high"]) {
    const broken = mission();
    broken.sandbox = { minimum_level: level, network: "deny" };
    assert.equal(accepts(MISSION, broken), false, level);
  }
});

test("an allowlist without a list is refused, and a deny with one too", () => {
  const missing = mission();
  missing.sandbox = { minimum_level: "S3", network: "allowlist" };
  assert.equal(accepts(MISSION, missing), false);

  const listed = mission();
  listed.sandbox = {
    minimum_level: "S3",
    network: "allowlist",
    network_allowlist: ["arxiv.org"],
  };
  assert.equal(accepts(MISSION, listed), true);

  // A deny that carries an allowlist is a contradiction, and reading it either way is worse
  // than refusing it.
  const contradictory = mission();
  contradictory.sandbox = {
    minimum_level: "S3",
    network: "deny",
    network_allowlist: ["arxiv.org"],
  };
  assert.equal(accepts(MISSION, contradictory), false);

  const empty = mission();
  empty.sandbox = { minimum_level: "S3", network: "allowlist", network_allowlist: [] };
  assert.equal(accepts(MISSION, empty), false);
});

test("the network mode is kebab-case, as every other multiword value on the wire", () => {
  const kebab = mission();
  kebab.sandbox = { minimum_level: "S3", network: "connector-only" };
  assert.equal(accepts(MISSION, kebab), true);

  // SPEC_V1 §21.7 writes `connector_only`; the received fixtures and every other enum value in
  // the protocol use kebab-case. See schemas/README.md — this test is where that arbitration
  // lives, so overturning it is a visible change and not a silent one.
  const snake = mission();
  snake.sandbox = { minimum_level: "S3", network: "connector_only" };
  assert.equal(accepts(MISSION, snake), false);
});

test("a worker announces the levels it applies, and cannot invent one", () => {
  const broken = capability();
  broken.sandbox = { levels: ["S3", "S9"], network_modes: ["deny"] };
  assert.equal(accepts(CAPABILITY, broken), false);

  const duplicated = capability();
  duplicated.sandbox = { levels: ["S1", "S1"], network_modes: ["deny"] };
  assert.equal(accepts(CAPABILITY, duplicated), false, "uniqueItems");

  const none = capability();
  none.sandbox = { levels: [], network_modes: ["deny"] };
  assert.equal(accepts(CAPABILITY, none), false, "un worker sans niveau n'offre rien");
});

test("an offer and a reservation keep different field names", () => {
  // Feeding a ResourceSpec where a capability inventory belongs must not quietly pass: the two
  // shapes exist precisely so a scheduler cannot compare them by accident.
  const swapped = capability();
  swapped.resources = { cpu: 4, memory_mb: 8192, disk_mb: 12000, wall_time_seconds: 60 };
  assert.equal(accepts(CAPABILITY, swapped), false);
});

test("a data class outside the ladder is refused", () => {
  const broken = capability();
  broken.data_classes = ["public", "secret-ish"];
  assert.equal(accepts(CAPABILITY, broken), false);
});

test("une date mal formée est refusée", () => {
  // `format` is annotation-only in draft-07 unless a validator asserts it, so this test is what
  // proves ajv-formats is actually wired in rather than imported and forgotten. No fixture
  // carries a `deadline`, which is exactly why the guard needed its own test.
  const ok = mission();
  ok.deadline = "2026-08-13T09:00:00Z";
  assert.equal(accepts(MISSION, ok), true);

  for (const bad of ["hier", "2026-13-45T00:00:00Z", "13/08/2026"]) {
    const broken = mission();
    broken.deadline = bad;
    assert.equal(accepts(MISSION, broken), false, bad);
  }
});

test("unknown fields are tolerated, which is what makes a minor version compatible", () => {
  // docs/06: minor = compatible optional fields. A 1.0 consumer meeting a 1.1 document must
  // ignore what it does not know rather than reject the message.
  const forward = mission();
  forward.some_field_added_in_1_1 = { anything: true };
  assert.equal(accepts(MISSION, forward), true);
});

//
// W0.6 — la seconde moitié. Mêmes règles : ce qui est testé, c'est ce que les schémas refusent.
//

const ATTESTATION = "urn:locus:schema:lep:1.0:sandbox-attestation";
const COMMIT = "urn:locus:schema:lep:1.0:epistemic-commit";
const EVENT = "urn:locus:schema:lep:1.0:event";
const LEASE = "urn:locus:schema:lep:1.0:lease";
const ATTEMPT = "urn:locus:schema:lep:1.0:attempt";

function commit(): Record<string, unknown> {
  return {
    protocol: "lep/1.0",
    task_id: "task-nominal",
    attempt: 1,
    status: "staged",
    produced_at: "2026-08-13T09:00:00.000Z",
  };
}

function event(type: string): Record<string, unknown> {
  return {
    protocol: "lep/1.0",
    event_type: type,
    sequence: 7,
    occurred_at: "2026-08-13T09:00:00.000Z",
    idempotency_key: "idem-7",
    task_id: "task-nominal",
    attempt: 1,
  };
}

test("un commit ne peut pas se valider lui-même", () => {
  // §2.3 : jamais au-delà de `staged`. C'est la garantie la plus forte du lot, et la seule
  // dont la violation rend le document littéralement invalide plutôt que seulement suspect.
  for (const status of ["draft", "staged"]) {
    assert.equal(accepts(COMMIT, { ...commit(), status }), true, status);
  }
  for (const status of ["validated", "under_review", "accepted", "contested"]) {
    assert.equal(accepts(COMMIT, { ...commit(), status }), false, status);
  }
});

test("une claim sans confiance déclarée se lirait comme certaine", () => {
  const withConfidence = commit();
  withConfidence.claims = [{ statement: "C-184 est réfutable", confidence: 0.6 }];
  assert.equal(accepts(COMMIT, withConfidence), true);

  const bare = commit();
  bare.claims = [{ statement: "C-184 est réfutable" }];
  assert.equal(accepts(COMMIT, bare), false);

  for (const confidence of [-0.1, 1.5]) {
    const broken = commit();
    broken.claims = [{ statement: "x", confidence }];
    assert.equal(accepts(COMMIT, broken), false, String(confidence));
  }
});

test("un type d'événement inconnu est refusé, contrairement à un champ inconnu", () => {
  assert.equal(accepts(EVENT, event("heartbeat")), true);
  assert.equal(accepts(EVENT, event("attempt.exploded")), false);
  // Un champ inconnu reste toléré : c'est la compatibilité mineure de docs/06.
  assert.equal(accepts(EVENT, { ...event("heartbeat"), nouveau_champ: 1 }), true);
});

test("un événement d'attempt dit toujours de quel attempt il parle", () => {
  const orphan = event("heartbeat");
  delete orphan.task_id;
  assert.equal(accepts(EVENT, orphan), false);

  const noAttempt = event("progress");
  delete noAttempt.attempt;
  assert.equal(accepts(EVENT, noAttempt), false);

  // `worker.registered` précède toute tâche : lui exiger un task_id serait une faute.
  const registration = event("worker.registered");
  delete registration.task_id;
  delete registration.attempt;
  assert.equal(accepts(EVENT, registration), true);
});

test("une reprise de stream a besoin de sa séquence et de sa clé d'idempotence", () => {
  for (const field of ["sequence", "idempotency_key"]) {
    const broken = event("progress");
    delete broken[field];
    assert.equal(accepts(EVENT, broken), false, field);
  }
});

test("une attestation peut décrire une mauvaise sandbox", () => {
  // Le point de conception : refuser au niveau du schéma rendrait le worker non conforme
  // incapable de l'avouer. Le refus appartient à l'admission.
  const base = {
    sandbox_id: "sbx-1",
    backend: "lima-podman",
    isolation_level: "S3",
    network_mode: "deny",
    host_home_mounted: true,
    runtime_socket_exposed: true,
    limits: { cpu: 4, memory_mb: 8192, pids: 512, disk_mb: 12000 },
    self_tests: {
      write_outside_workspace: "allowed",
      read_host_home: "allowed",
      network_egress: "allowed",
      memory_limit: "unenforced",
    },
  };
  assert.equal(accepts(ATTESTATION, base), true);

  // En revanche une attestation MUETTE est invalide : un champ absent se lit « je n'ai pas
  // regardé » aussi bien que « non ».
  for (const field of ["host_home_mounted", "runtime_socket_exposed", "limits", "self_tests"]) {
    const silent: Record<string, unknown> = { ...base };
    delete silent[field];
    assert.equal(accepts(ATTESTATION, silent), false, field);
  }
});

test("les quatre self-tests d'ADR 0004 sont tous obligatoires", () => {
  const raw = JSON.parse(
    readFileSync(join(schemasDir, "examples/sandbox-attestation.json"), "utf8"),
  );
  const attestation = stripFixture(raw).body as Record<string, unknown>;
  assert.equal(accepts(ATTESTATION, attestation), true);

  for (const probe of [
    "write_outside_workspace",
    "read_host_home",
    "network_egress",
    "memory_limit",
  ]) {
    const partial = JSON.parse(JSON.stringify(attestation)) as Record<string, unknown>;
    delete (partial.self_tests as Record<string, unknown>)[probe];
    assert.equal(accepts(ATTESTATION, partial), false, probe);
  }
});

test("une lease sans borne n'est pas une lease", () => {
  const lease = {
    protocol: "lep/1.0",
    lease_id: "lease-1",
    task_id: "task-nominal",
    attempt: 1,
    worker_id: "w-1",
    issued_at: "2026-08-13T09:00:00.000Z",
    expires_at: "2026-08-13T09:05:00.000Z",
    ttl_seconds: 300,
    heartbeat_interval_seconds: 60,
  };
  assert.equal(accepts(LEASE, lease), true);

  for (const field of ["expires_at", "ttl_seconds", "heartbeat_interval_seconds"]) {
    const broken: Record<string, unknown> = { ...lease };
    delete broken[field];
    assert.equal(accepts(LEASE, broken), false, field);
  }
  // Le rang d'attempt commence à 1 : « attempt 0 » se confondrait avec « pas encore tenté ».
  assert.equal(accepts(LEASE, { ...lease, attempt: 0 }), false);
});

test("un attempt échoué porte son erreur", () => {
  const base = {
    protocol: "lep/1.0",
    task_id: "task-nominal",
    attempt: 1,
    worker_id: "w-1",
    state: "running",
    started_at: "2026-08-13T09:00:00.000Z",
  };
  assert.equal(accepts(ATTEMPT, base), true);

  const failedBare = { ...base, state: "failed" };
  assert.equal(accepts(ATTEMPT, failedBare), false);

  const failed = { ...base, state: "failed", error: { category: "tool", message: "boom" } };
  assert.equal(accepts(ATTEMPT, failed), true);

  // Les verdicts de l'institution ne sont pas des états que le worker s'attribue.
  for (const state of ["accepted", "rejected", "superseded", "validated"]) {
    assert.equal(accepts(ATTEMPT, { ...base, state }), false, state);
  }
});

test("un run consigne ce qu'il a réservé, et une commande n'est pas une ligne de shell", () => {
  const RUN = "urn:locus:schema:artifacts:1.0:run-manifest";
  const run = {
    run_id: "run-1",
    task_id: "task-nominal",
    attempt: 1,
    environment: {
      environment_id: "math-formal-v1",
      image_digest: `sha256:${"a".repeat(64)}`,
      toolchains: ["math-formal"],
    },
    inputs: [{ content_hash: `sha256:${"b".repeat(64)}` }],
    commands: [{ argv: ["lake", "build"] }],
    resources: { reserved: { cpu: 4, memory_mb: 8192, disk_mb: 12000, wall_time_seconds: 3600 } },
    started_at: "2026-08-13T09:00:00.000Z",
  };
  assert.equal(accepts(RUN, run), true);

  // Une image par tag plutôt que par digest : §21.8 l'interdit, le motif le refuse.
  const tagged = JSON.parse(JSON.stringify(run)) as typeof run;
  (tagged.environment as Record<string, unknown>).image_digest = "python:3.12";
  assert.equal(accepts(RUN, tagged), false);

  // argv vide, ou une chaîne au lieu d'un tableau.
  const shellish = JSON.parse(JSON.stringify(run)) as Record<string, unknown>;
  shellish.commands = [{ argv: "lake build && rm -rf /" }];
  assert.equal(accepts(RUN, shellish), false);
});

test("un artefact sans provenance n'est pas un artefact", () => {
  const ARTIFACT = "urn:locus:schema:artifacts:1.0:artifact-manifest";
  const artifact = {
    artifact_id: "art-1",
    content_hash: `sha256:${"c".repeat(64)}`,
    media_type: "application/pdf",
    size_bytes: 1024,
    produced_by: { task_id: "task-nominal", attempt: 1 },
    classification: "internal",
    state: "declared",
  };
  assert.equal(accepts(ARTIFACT, artifact), true);

  for (const field of ["content_hash", "media_type", "size_bytes", "produced_by", "state"]) {
    const broken: Record<string, unknown> = { ...artifact };
    delete broken[field];
    assert.equal(accepts(ARTIFACT, broken), false, field);
  }
  // Un état de promotion automatique n'existe pas : la quarantaine se lève par une revue.
  assert.equal(accepts(ARTIFACT, { ...artifact, state: "auto-promoted" }), false);
});
