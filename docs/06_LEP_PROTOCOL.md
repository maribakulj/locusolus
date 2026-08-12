# LEP v1 — Locus Execution Protocol

LEP relie le control plane aux executors. Il remplace toute appellation attachée à Canterel.

## Entités

- Worker
- CapabilityManifest
- Task
- Attempt
- Lease
- MissionEnvelope
- ContextView
- EnvironmentBlueprint
- SandboxSpec
- ResourceSpec
- ArtifactManifest
- RunManifest
- SandboxAttestation
- EpistemicCommit

## Sémantique

Le worker s’enregistre, annonce ses capabilities, reçoit une offre, accepte une lease, crée/exécute un attempt, heartbeats, produit artefacts et commit, puis Locus décide promotion/review/retry. Au moins once transport est acceptable à condition que les mutations soient idempotentes.

## Capabilities

Inclure toolchains, providers/models, auth mode, architecture, resources, accelerators, trust/isolation, network modes et classifications admissibles.

## Late results

Un result après lease expiration est stocké comme late candidate, jamais écrasé. Locus peut le comparer avec l’attempt officiel et ouvrir review/conflit.

## Human input

Un worker peut demander une entrée humaine structurée. Le workflow se suspend sans garder un processus coûteux vivant lorsque le backend le permet.

## Versioning

Major = rupture ; minor = champs optionnels compatibles ; feature negotiation au handshake ; schemas JSON versionnés et fixtures inter-SDK.
