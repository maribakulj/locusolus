import { readFileSync, readdirSync } from "node:fs";
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

//
// W0.7 — le corpus. Les fixtures ne sont pas de la documentation : ce sont les cas que le
// harnais de conformance (W0.9) rejouera contre une implémentation tierce.
//

function fixtureOf(name: string): Record<string, unknown> {
  return JSON.parse(readFileSync(join(schemasDir, "examples", name), "utf8")) as Record<
    string,
    unknown
  >;
}

test("les cinq scénarios de W0.7 sont dans le corpus", () => {
  // La roadmap les nomme : nominal, refus d'admission, reconnexion, résultat tardif,
  // dépassement de budget. Les compter ici évite qu'un scénario disparaisse sans bruit.
  const scenarios = new Map<string, string[]>([
    ["nominal", ["capability-manifest-vm-linux.json", "mission-envelope-nominal.json"]],
    ["refus d'admission", ["capability-manifest.json", "mission-envelope.json"]],
    [
      "reconnexion",
      [
        "event-reconnection-1-started.json",
        "event-reconnection-2-progress.json",
        "event-reconnection-3-tool-completed.json",
        "event-reconnection-4-replay.json",
      ],
    ],
    ["résultat tardif", ["attempt-late-result.json", "lease-expired.json"]],
    ["dépassement de budget", ["attempt-budget-exceeded.json"]],
  ]);
  for (const [scenario, files] of scenarios) {
    for (const file of files) {
      assert.ok(fixtureOf(file), `${scenario} : ${file} manquant`);
    }
  }
});

test("chaque résultat déclaré existe dans le vocabulaire du registre", () => {
  // Le vocabulaire est une donnée, pas une constante du code : un résultat neuf doit venir
  // avec ce qu'il veut dire.
  const files = readdirSyncSorted();
  for (const file of files) {
    const raw = fixtureOf(file);
    const { expect } = stripFixture(raw);
    assert.ok(expect, `${file} sans \`expect\``);
    assert.ok(registry.expectations[expect], `${file} : \`${expect}\` absent du registre`);
    assert.ok(registry.expectations[expect].note.length > 0, `${file} : \`${expect}\` sans note`);
  }
});

test("le corpus exerce vraiment `expect: invalid`", () => {
  // Posé en W0.5 sans usage. Un chemin de code que rien n'emprunte n'est pas testé, et un
  // corpus qui ne contient que des documents valides ne teste que la moitié d'un schéma.
  const invalid = readdirSyncSorted().filter(
    (file) => stripFixture(fixtureOf(file)).expect === "invalid",
  );
  assert.ok(invalid.length >= 3, `attendu au moins 3 fixtures invalides, trouvé ${invalid.length}`);
});

test("le rejeu est le même document que l'original", () => {
  // C'est toute la propriété : si le rejeu différait, la déduplication par clé d'idempotence
  // ne prouverait rien.
  const original = stripFixture(fixtureOf("event-reconnection-3-tool-completed.json")).body;
  const replay = stripFixture(fixtureOf("event-reconnection-4-replay.json")).body;
  assert.deepEqual(replay, original);
});

test("le résultat tardif a bien dépassé sa lease", () => {
  // Une fixture qui affirmerait « tardif » sans que les dates le montrent ne serait pas une
  // fixture, seulement une étiquette.
  const attempt = stripFixture(fixtureOf("attempt-late-result.json")).body as Record<
    string,
    unknown
  >;
  const lease = stripFixture(fixtureOf("lease-expired.json")).body as Record<string, unknown>;
  assert.equal(attempt.late, true);
  assert.equal(attempt.lease_id, lease.lease_id);
  assert.ok(
    Date.parse(String(attempt.completed_at)) > Date.parse(String(lease.expires_at)),
    "la fixture tardive doit se terminer après l'expiration de sa lease",
  );
});

test("un dépassement de budget n'est pas réessayable", () => {
  // Réessayer ne rendrait pas le budget. Une erreur `retryable` ici enverrait le worker
  // reconsommer ce qu'il vient d'épuiser.
  const attempt = stripFixture(fixtureOf("attempt-budget-exceeded.json")).body as Record<
    string,
    unknown
  >;
  assert.equal(attempt.state, "failed");
  assert.equal((attempt.error as Record<string, unknown>).retryable, false);
});

test("la paire de refus n'est pas lisible comme un cas nominal", () => {
  // L'ambiguïté du paquet d'origine, verrouillée : la mission exige S3, le worker apparié
  // n'offre que S1/S2. Un worker qui l'accepte est en faute.
  const mission = stripFixture(fixtureOf("mission-envelope.json")).body as Record<string, unknown>;
  const worker = stripFixture(fixtureOf("capability-manifest.json")).body as Record<
    string,
    unknown
  >;
  const required = (mission.sandbox as Record<string, unknown>).minimum_level as string;
  const offered = (worker.sandbox as Record<string, unknown>).levels as string[];
  assert.ok(!offered.includes(required), `${required} ne doit pas figurer dans ${offered.join()}`);
});

function readdirSyncSorted(): string[] {
  return readdirSync(join(schemasDir, "examples"))
    .filter((name) => name.endsWith(".json"))
    .sort();
}

//
// W19.a — le refus d'admission sur le fil, tranche 2 du mineur `lep/1.1`.
//

const REFUSAL = "urn:locus:schema:lep:1.0:admission-refusal";

/** Les neuf motifs, sous les codes qui voyagent. */
const REFUSAL_CODES = [
  "level_unavailable",
  "capacity_exceeded",
  "accelerator_unavailable",
  "disk_quota_not_enforceable",
  "network_mode_unsupported",
  "level_not_attested",
  "accelerator_outside_sandbox",
  "mechanism_not_employed",
  "mechanism_unresolved",
];

test("un refus porte au moins un motif", () => {
  // Un refus qui ne dit pas ce qui manque ne se corrige pas : il envoie relancer à l'identique.
  // `minItems: 1` le rend inexprimable plutôt que de compter sur la bonne tenue de l'émetteur.
  const refusal = {
    protocol: "lep/1.1",
    task_id: "t",
    attempt_id: "a",
    reasons: [{ code: "capacity_exceeded" }],
  };
  assert.equal(accepts(REFUSAL, refusal), true);
  assert.equal(accepts(REFUSAL, { ...refusal, reasons: [] }), false);
});

test("un motif ne porte pas les champs d'un autre", () => {
  // `additionalProperties: false` sur chaque variante empêche un émetteur de composer un motif
  // hybride que le lecteur interpréterait à moitié.
  const hybride = {
    protocol: "lep/1.1",
    task_id: "t",
    attempt_id: "a",
    reasons: [{ code: "level_unavailable", required: "S4", best: "S2", kind: "cuda" }],
  };
  assert.equal(accepts(REFUSAL, hybride), false);
});

test("`proven` est facultatif, et c'est une ignorance nommée", () => {
  // « Aucune campagne n'a conclu » et « la campagne a conclu plus bas » envoient chercher deux
  // choses différentes ; le schéma laisse la première s'écrire par l'absence.
  const base = { protocol: "lep/1.1", task_id: "t", attempt_id: "a" };
  assert.equal(
    accepts(REFUSAL, { ...base, reasons: [{ code: "level_not_attested", required: "S3" }] }),
    true,
  );
  assert.equal(
    accepts(REFUSAL, {
      ...base,
      reasons: [{ code: "level_not_attested", required: "S3", proven: "S1" }],
    }),
    true,
  );
});

/**
 * **Aucune énumération existante ne gagne un membre.**
 *
 * C'est l'interdit 3 d'ADR 0017, et la raison pour laquelle les motifs de refus sont un **document
 * nouveau** plutôt que des valeurs ajoutées quelque part. Le SDK Rust émet des `enum` fermés : un
 * membre de plus sur une énumération ancienne ferait échouer la désérialisation chez tout
 * consommateur `1.0`, **en silence pour l'émetteur**.
 *
 * Le test lit les schémas plutôt que de faire confiance à la relecture du diff : c'est la seule
 * façon de tenir la propriété au prochain mineur aussi.
 */
test("aucune énumération d'un schéma ancien ne porte un code de refus", () => {
  const codes = new Set(REFUSAL_CODES);
  const dir = join(root, "schemas");
  const registry = JSON.parse(readFileSync(join(dir, "registry.json"), "utf8")) as {
    shared: string[];
    documents: { schema: string }[];
  };
  const files = [...new Set([...registry.shared, ...registry.documents.map((e) => e.schema)])];

  for (const file of files) {
    if (file.endsWith("admission-refusal.schema.json")) continue;
    const text = readFileSync(join(dir, file), "utf8");
    for (const code of codes) {
      assert.equal(
        text.includes(`"${code}"`),
        false,
        `${file} nomme « ${code} » : un mineur ajoute des champs, jamais des valeurs`,
      );
    }
  }
});

//
// W19.b — la permission de fonctionnement hors ligne, tranche 3 du mineur `lep/1.1`.
//

/**
 * **La permission et le mode réseau sont indépendants, et le schéma le rend observable.**
 *
 * Les quatre combinaisons sont valides, et c'est le sens de « aucune fonction ne dérive l'une de
 * l'autre » : si le schéma en interdisait une, il aurait choisi une dérivation. `deny` sans
 * dispense décrit une mission qui n'a jamais eu de réseau à perdre ; `full` sans dispense décrit
 * une mission qui doit échouer si le réseau tombe. Les deux existent, et confondre le confinement
 * avec l'autorisation ferait disparaître la seconde.
 */
test("la permission hors ligne et le mode réseau ne se contraignent pas", () => {
  for (const network of ["deny", "full"]) {
    for (const offline of [true, false]) {
      const envelope = mission();
      (envelope["sandbox"] as Record<string, unknown>)["network"] = network;
      envelope["offline_allowed"] = offline;
      assert.equal(accepts(MISSION, envelope), true, `${network} / ${offline}`);
    }
  }
});

test("un budget hors ligne est un entier de millisecondes, strictement positif", () => {
  // Un budget nul ou négatif dirait « autorisé pendant zéro milliseconde », c'est-à-dire refusé —
  // une seconde façon d'écrire un refus, qui se lirait comme une permission.
  const envelope = mission();
  envelope["offline_allowed"] = true;
  for (const budget of [1, 60_000]) {
    assert.equal(accepts(MISSION, { ...envelope, offline_budget_ms: budget }), true, `${budget}`);
  }
  for (const budget of [0, -1, 1.5, "60000"]) {
    assert.equal(
      accepts(MISSION, { ...envelope, offline_budget_ms: budget }),
      false,
      String(budget),
    );
  }
});

test("la permission est un booléen, et rien d'autre", () => {
  // « oui », « 1 », « true » : trois écritures qu'un émetteur pourrait croire équivalentes, et que
  // le lecteur devrait alors interpréter. Une permission qui se lit de plusieurs façons est une
  // permission qu'on finit par accorder par erreur.
  for (const valeur of ["true", "oui", 1, null, {}]) {
    assert.equal(
      accepts(MISSION, { ...mission(), offline_allowed: valeur }),
      false,
      JSON.stringify(valeur),
    );
  }
  for (const valeur of [true, false]) {
    assert.equal(accepts(MISSION, { ...mission(), offline_allowed: valeur }), true, String(valeur));
  }
});

test("un document sans permission reste valide, et la permission reste absente", () => {
  // « Absente » et « refusée explicitement » sont deux faits différents pour un lecteur ; ce que le
  // schéma garantit est qu'un document `1.0` n'a pas à la porter. Ce que l'absence *vaut* est la
  // décision du lecteur, et `offlineVerdict` la prend deny-by-default.
  const ancien = mission();
  assert.equal(accepts(MISSION, ancien), true);
  assert.equal("offline_allowed" in ancien, false);
});
