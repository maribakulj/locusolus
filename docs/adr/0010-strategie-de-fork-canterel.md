# ADR 0010 — Stratégie de fork pour Canterel

**Statut :** accepté. **Amende** `repos/locusolus/SPEC_V1.md` §26.1 et `docs/12_CROSS_REPO_MIGRATION.md`.

**Contexte :** `maribakulj/canterel` n'est pas un codebase dont le projet hérite. C'est un fork **non divergé** de `synthetic-sciences/OpenScience` (Apache-2.0) : le seul commit local est le merge de synchronisation amont (`c3f734c`). Les packages publiés sont `@synsci/openscience`, `@synsci/workspace`, `@synsci/ui`, `@synsci/sdk`, `@synsci/plugin` ; le `NOTICE` est au nom de Synthetic Sciences ; 498 fichiers `.ts/.tsx/.json` portent la marque amont.

Le handoff demandait deux choses incompatibles : mettre à jour noms de packages et import paths (§26.1, `docs/12`), et préserver providers, agents, tools, skills, sessions, compaction, workspace, provenance, reviewer, sandbox et permissions (`repos/canterel/SPEC_V1.md` §30.1) — c'est-à-dire continuer à bénéficier d'un amont actif.

**Options.** *A — fork durci* : rebrand complet, plus de sync. La maintenance de 4 400 fichiers tiers passe au projet, pour la seule cohérence de nommage. *B — fork suiveur* : aucun rebrand ; le seul code local est `backend/cli/src/locus/**`.

**Décision : B.**

Trois raisons. Le nom ne porte pas l'architecture : LEP est explicitement générique (D002) et aucun invariant ne dépend du nom d'un package npm. `repos/canterel/SPEC_V1.md` §4 a déjà choisi le bon point d'insertion — `backend/cli/src/locus/` est un répertoire neuf, donc zéro conflit de merge quelle que soit l'évolution amont ; la spec avait implicitement adopté la logique du fork suiveur. Et l'amont est un actif : providers multi-fournisseurs, ~30 connecteurs scientifiques, skills, MCP, LSP, workspace SolidJS — exactement ce que §30.1 demande de préserver, et le meilleur moyen de le préserver est de continuer à le recevoir.

**Conséquences.**
- Le rename GitHub `openscienceDH` → `canterel` (déjà effectué) suffit. La mise à jour des noms de packages est retirée du périmètre.
- Un remote `upstream` est déclaré ; la politique de sync vit dans `docs/locus/upstream.md`.
- **Règle dure :** tout fichier modifié hors de `backend/cli/src/locus/**` est justifié dans l'`IMPLEMENTATION_LEDGER.md`, parce qu'il sera payé à chaque synchronisation.
- Une commande `canterel` est fournie comme alias de `openscience`, dans un fichier neuf.
- Le `NOTICE` d'origine est conservé intact ; toute addition locale substantielle est signalée en section séparée (Apache-2.0).
- Les tests LEP vivent sous `backend/cli/test/locus/`, jamais mêlés à la suite amont.

**Risque assumé :** l'amont peut refactorer `src/session/`, `src/provider/` ou `src/sandbox/` d'une manière qui casse `src/locus/**`. Le risque est borné à cinq modules d'adaptation — `session-map.ts`, `agent-overlay.ts`, `model-policy.ts`, `tool-policy.ts`, `sandbox-policy.ts`. Ils doivent rester minces ; c'est leur seule raison d'être.

**Réexamen :** si l'amont introduit son propre concept de worker distant, ou s'il devient inactif.

**Rollback :** trivial — il n'y a rien à défaire, puisque l'option B consiste à ne pas toucher au code amont.
