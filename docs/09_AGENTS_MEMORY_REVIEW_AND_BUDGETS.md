# Agents, mémoire, review et tokens

## Société d’agents

Locus instancie des rôles : governance, exploration, specialization, adversarial, memory. Un agent template n’est pas une instance ; une équipe est un objet distinct.

## Spawn

Déclenché par événements et policy : domain gap, disagreement, barrier, counterexample need, stagnation. Score multiobjectif impact/information gain/diversity/cost/redundancy ; budgets d’exploration réservés.

## ContextView

Chaque mission reçoit un snapshot minimal, hashé et filtré. Les reviewers aveugles ne reçoivent ni transcript ni raisonnement privé du générateur.

## Tokens

Les tokens sont une ressource budgétaire distincte du CPU/GPU. Exécuter un script ne consomme pas de tokens tant que ses données ne sont pas renvoyées au modèle. Les agents doivent résumer/filtrer les sorties et conserver les gros résultats comme artefacts.

Budget par mission : input/output tokens, model calls, currency ceiling, wall time, CPU/GPU seconds. Prompt caching et modèles moins coûteux peuvent être policies, pas des hypothèses du domaine.

## Review

Structural, bibliographic, computational falsification, logical, reproduction, formalization et meta-review selon type de claim. Findings, rebuttal, recheck et désaccords persistent.
