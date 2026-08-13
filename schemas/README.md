# `schemas/`

JSON Schemas versionnés. Ce sont les contrats de fil : ce qui est ici prime sur toute représentation
en mémoire, dans n'importe quel langage.

`docs/SPEC_V1.md` §5 prévoit la partition `commands/`, `events/`, `lep/`, `artifacts/`,
`environments/`, `federation/`. Les répertoires apparaissent avec les schémas qu'ils portent (W0.5
et W0.6 de `docs/10_V1_ROADMAP.md`), pas avant.

## Pourquoi les schémas d'abord

Le langage d'implémentation de `locusd` reste une décision ouverte. Les schémas sont communs aux
deux options : ils fixent le protocole avant qu'un choix de langage puisse l'infléchir, et ils sont
la source depuis laquelle les SDK sont générés (W0.8). Une implémentation qui diverge du schéma a
tort.

## Règles

Un schéma publié ne change pas de sens sous le même identifiant. L'évolution suit `docs/06` : majeur
= rupture, mineur = champs optionnels compatibles. `lep/1.0` gèle à la fin de W0.

Les exemples et les fixtures ne sont pas de la documentation : chaque fichier de `schemas/examples/`
doit valider — ou invalider intentionnellement, selon son `expect` déclaré — contre le schéma qu'il
illustre.

## Ce qui est écrit (W0.5)

| Schéma                 | Emplacement                                          |
| ---------------------- | ---------------------------------------------------- |
| `CapabilityManifest`   | `lep/1.0/capability-manifest.schema.json`            |
| `MissionEnvelope`      | `lep/1.0/mission-envelope.schema.json`               |
| `ContextView`          | `lep/1.0/context-view.schema.json`                   |
| `SandboxSpec`          | `lep/1.0/sandbox-spec.schema.json`                   |
| `ResourceSpec`         | `lep/1.0/resource-spec.schema.json`                  |
| `EnvironmentBlueprint` | `environments/1.0/environment-blueprint.schema.json` |

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
