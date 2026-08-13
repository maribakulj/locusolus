# `schemas/`

JSON Schemas versionnés. Ce sont les contrats de fil : ce qui est ici prime sur toute représentation
en mémoire, dans n'importe quel langage.

`docs/SPEC_V1.md` §5 prévoit la partition `commands/`, `events/`, `lep/`, `artifacts/`,
`environments/`, `federation/`. Les répertoires apparaissent avec les schémas qu'ils portent (W0.5
et W0.6 de `docs/10_V1_ROADMAP.md`), pas avant.

## Pourquoi les schémas d'abord

Les schémas sont la source depuis laquelle les SDK sont générés (W0.8), et il y en aura deux :
TypeScript pour le worker, Rust pour le serveur (ADR 0011). Ils fixent le protocole avant qu'un
choix de langage puisse l'infléchir. Une implémentation qui diverge du schéma a tort.

## Dialecte : JSON Schema Draft 7

Contrainte d'outillage, pas de goût. `typify` — la voie de référence pour JSON Schema → Rust —
supporte réellement Draft 7 ; sur 2020-12 il fonctionne parfois et casse souvent, et sa refonte est
en cours. C'est la condition 1 d'ADR 0011.

Le dialecte est fixé ici parce que ce répertoire est vide : le choix ne coûte rien aujourd'hui et se
migre mal une fois `lep/1.0` gelé. Un prototype `typify` qui passerait sur 2020-12 lève la condition
— l'expérience se fait en W0.5, avant le premier schéma, pas après.

## Règles

Un schéma publié ne change pas de sens sous le même identifiant. L'évolution suit `docs/06` : majeur
= rupture, mineur = champs optionnels compatibles. `lep/1.0` gèle à la fin de W0.

Les exemples et les fixtures ne sont pas de la documentation : chaque fichier de `schemas/examples/`
doit valider — ou invalider intentionnellement, selon son `expect` déclaré — contre le schéma qu'il
illustre.

## Ce qui est écrit (W0.5 et W0.6)

| Schéma                 | Emplacement                                          |
| ---------------------- | ---------------------------------------------------- |
| `CapabilityManifest`   | `lep/1.0/capability-manifest.schema.json`            |
| `MissionEnvelope`      | `lep/1.0/mission-envelope.schema.json`               |
| `ContextView`          | `lep/1.0/context-view.schema.json`                   |
| `SandboxSpec`          | `lep/1.0/sandbox-spec.schema.json`                   |
| `ResourceSpec`         | `lep/1.0/resource-spec.schema.json`                  |
| `EnvironmentBlueprint` | `environments/1.0/environment-blueprint.schema.json` |
| `Lease`                | `lep/1.0/lease.schema.json`                          |
| `Attempt`              | `lep/1.0/attempt.schema.json`                        |
| Événement LEP          | `lep/1.0/event.schema.json`                          |
| `SandboxAttestation`   | `lep/1.0/sandbox-attestation.schema.json`            |
| `EpistemicCommit`      | `lep/1.0/epistemic-commit.schema.json`               |
| `ArtifactManifest`     | `artifacts/1.0/artifact-manifest.schema.json`        |
| `RunManifest`          | `artifacts/1.0/run-manifest.schema.json`             |

`lep/1.0/vocabulary.schema.json` porte les énumérations partagées — niveaux de sandbox, modes
réseau, classes de données, forme d'un hash. Elles vivent en un seul endroit parce que deux copies
d'une liste de niveaux de sandbox finissent par diverger sur celui qui compte.

`schemas/registry.json` dit quel exemple se valide contre quel schéma. C'est déclaratif et non
déduit d'un nom de fichier : un exemple qu'on ne valide plus ressemble en tout point à un exemple
qui passe, donc un exemple absent du registre est une violation, pas un silence.

## Décisions

**Draft 7.** Le brouillon le plus largement outillé hors de l'écosystème JavaScript — Rust, Go,
Python et Java le supportent tous, ce qui n'est pas vrai de 2020-12. Puisque le langage de `locusd`
n'est pas tranché, le choix qui ferme le moins de portes gagne.

**Identifiants en URN.** `urn:locus:schema:lep:1.0:mission-envelope` plutôt qu'une URL. Un
identifiant de schéma doit être stable et porter sa version ; une URL promet en plus d'être
récupérable, ce que nous ne tenons pas. La règle « un schéma publié ne change pas de sens sous le
même identifiant » se lit directement dans l'URN.

**Les documents restent ouverts.** Aucun `additionalProperties: false` au niveau document. `docs/06`
fait du mineur un ajout de champs optionnels compatibles : un consommateur `lep/1.0` qui rencontre
un document `1.1` doit ignorer ce qu'il ne connaît pas, pas rejeter le message. Fermer les documents
transformerait chaque ajout mineur en rupture. Le prix est qu'une faute de frappe dans un nom de
champ passe la validation — c'est le corpus de fixtures (W0.7), qui teste des issues sémantiques,
qui l'attrape.

Pour la même raison, `protocol` accepte toute la ligne `1.x` par motif plutôt qu'un `const`.

**Une demande n'est pas une offre.** `MissionEnvelope.resources` et `CapabilityManifest.resources`
ne partagent pas leur forme : l'un réserve (`cpu`, `disk_mb`, `wall_time_seconds`), l'autre
inventorie (`cpu_cores`, `disk_free_mb`). Même chose pour `SandboxSpec.minimum_level` — un plancher
— face à `CapabilityManifest.sandbox.levels` — une liste. Deux noms différents pour deux grandeurs
différentes valent mieux qu'un nom commun qui invite à les soustraire sans y penser, et c'est
exactement l'erreur qui accorde S1 à une mission qui demandait S3.

**`connector-only`, pas `connector_only`.** `SPEC_V1.md` §21.7 écrit `connector_only` ; les fixtures
reçues écrivent `connector-only`, comme toutes les autres valeurs d'énumération du protocole
(`oauth-local`, `rootless-oci`, `service-credential`, `python-science`). Les noms de champs sont en
`snake_case`, les valeurs en kebab-case : le texte de la spec est l'exception, pas la règle. La
graphie retenue est donc `connector-only`, et un test la fixe explicitement pour que la renverser
soit un changement visible.

**Les hashs sont vérifiés par longueur d'algorithme.** `sha256:` exige 64 hexadécimaux, `sha512:` en
exige 128. Un digest tronqué est la forme que prend une intégrité cassée, et un motif permissif
laisserait passer le placeholder `sha256:...` que portaient deux fixtures — remplacé ici par un
digest réel, `sha256("ctx-example")`.

## Deux frontières que ces schémas ne franchissent pas

**L'enveloppe du journal institutionnel (§10.1) n'est pas ici.** Elle vit sous `schemas/events/` et
appartient à W1. Un worker ne modifie jamais directement la base canonique (invariant 3) : ce qui
traverse le fil et ce qui est écrit dans l'event store ne peuvent pas être le même objet, et les
confondre ferait du worker un écrivain de l'histoire.

**Ce qu'un schéma ne peut pas dire.** Draft 7 n'exprime pas de relation arithmétique entre deux
champs, donc « le heartbeat est inférieur au tiers du TTL » (§12.3) n'est pas dans `Lease`. La règle
est vérifiée par le harnais de conformance (W0.9). C'est écrit dans le schéma plutôt que passé sous
silence : croire qu'une garantie existe est pire que savoir qu'elle manque.

## Ce qu'une attestation a le droit de dire

`SandboxAttestation` accepte `host_home_mounted: true`. C'est délibéré. Refuser au niveau du schéma
ne rendrait pas le montage impossible — ça rendrait le worker non conforme **incapable de
l'avouer**, et un worker qui ment par construction est pire qu'un worker qui déclare une mauvaise
isolation. Le refus appartient à l'admission, qui compare l'attestation au plancher de la
`SandboxSpec`.

En revanche, une attestation **muette** est invalide : `host_home_mounted` est obligatoire même
quand la réponse est `false`, parce qu'un champ absent se lit « je n'ai pas regardé » aussi bien que
« non », et qu'un seul des deux est une attestation.
