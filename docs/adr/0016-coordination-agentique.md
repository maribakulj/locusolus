# ADR 0016 — Coordination agentique : deux domaines, aucun vocabulaire parallèle, une cible mesurée

**Statut :** accepté. **Amende** `docs/SPEC_V1.md` §14 sur deux points, et la liste « Frontières
vérifiées par la CI » de `CLAUDE.md`, qui gagne une sixième règle. Ouvre W13 et inscrit W14 à W18.
Ne modifie ni §7.5, ni §7.6, ni `packages/graph`.

**Contexte.** `SPEC_V1.md` §7.1, §13, §16, §20 et §22 spécifient une société d'agents gouvernée, un
orchestrateur de portefeuille avec fonction de valeur et anti-gaming, sept niveaux de mémoire avec
retrieval hybride, un moteur de politique à cinq verbes, et une surface de commandes incluant
`team.modify` et un `CommandEnvelope` à `expected_revision`. **Aucun de ces objets n'existe en
code** : `grep -r 'AgentTemplate\|AgentInstance\|ApprovalRequest'` sur `packages/` ne rend rien, et
la seule trace de gouvernance est le champ `delegation_id` de l'enveloppe d'événement de §10.1. Et
aucun workstream de `docs/10` ne les assigne.

Le dépôt sait donc décrire ce qui est cru et pourquoi — quarante types épistémiques, vingt-huit
relations dirigées, des hyperarêtes d'inférence, quatre cibles d'objection, la propagation de
l'invalidation. Il ne sait rien dire de **qui travaille**.

Cet ADR décide ce que coûte de le lui apprendre, ce qu'il ne faut surtout pas inventer en même temps,
et où va le projet.

---

## Décision 1 — `packages/graph` reste strictement épistémique, et la frontière est vérifiée

Aucun objet de coordination, d'agent, d'outil ou de runtime n'entre dans `packages/graph`, **y compris
par ajout d'une variante à une énumération existante**. La frontière est portée par une sixième règle
de `boundaries.json`, dans les deux sens, et par un test.

**Motifs.** La garantie de `packages/graph` est une garantie **par absence d'API**. Son `lib.rs`
l'écrit : « pas de `flatten`, pas de `decompose`, pas d'`as_edges` — un test le vérifie par l'absence,
parce que c'est exactement la fonction de commodité que quelqu'un finira par vouloir écrire. » Un
graphe de coordination a légitimement besoin de projection, d'atteignabilité, de diff et
d'aplatissement. Faire cohabiter dans un même crate un objet qui exige ces opérations et un objet dont
la garantie est leur absence, c'est programmer la violation.

Le critère plus fondamental est causal, au sens de Models@run.time (Blair, Bencomo, France, 2009 —
l'une des cinq seules références relues et publiées du corpus). Un modèle au runtime n'est utile que
s'il est causalement connecté au système. Or les quatre graphes ne le sont pas de la même façon : une
version canonique de coordination est connectée **en écriture** — la modifier change les missions
émises ; un graphe réalisé est connecté **en lecture seule**, c'est un miroir ; une trace ne l'est
**dans aucun sens** ; le graphe épistémique est connecté à la **délibération**. Quatre régimes causaux
distincts ne partagent pas un crate.

La variante d'énumération mérite d'être nommée séparément parce que c'est le chemin le plus court et le
plus discret. `ObjectionTarget` de `inference.rs` a quatre variantes, et la tentation d'y ajouter
`Decision` pour rendre une décision de coordination contestable est réelle et bonne — voir décision 9.
Elle ferait entrer un identifiant du domaine de coordination dans le crate épistémique par une ligne
que personne ne relit comme un acte d'architecture.

Rainbow (Garlan et al., 2004) donne l'argument publié : le contrôleur d'adaptation ne se mélange pas
aux prompts métier, il constitue un plan de contrôle séparé, avec une infrastructure générique
distincte des connaissances propres au système. K-Component (Dowling & Cahill, 2001) ajoute la
séparation du code d'adaptation et du code fonctionnel. Et *Runtime Software Adaptation* (Oreizy,
Medvidovic, Taylor, 2008) donne la phrase qui résume cette décision : **« la créativité du contrôleur
ne peut pas être confondue avec la validité du système »**.

### Forme de la frontière : les agrégats vivent dans un crate séparé

L'arbitrage laissé ouvert par le handoff est tranché ici, parce qu'il engage la structure du workspace
et qu'il n'a qu'une réponse mécanique. `tooling/boundaries/rules.ts` n'accepte que deux `kind` :
`"imports"` et `"emacs-isolation"`. Il n'existe pas de garde par absence d'identifiant, et une garde
d'import ne sait pas interdire à un crate de nommer *certains types* d'un crate dont il dépend déjà.
Or `packages/graph/Cargo.toml` déclare `locus-domain`. Loger les agrégats de coordination dans
`packages/domain` rendrait donc la décision 1 **inapplicable par la CI** : `packages/graph` pourrait
nommer `AgentInstance` sans qu'aucune règle exprimable ait quoi que ce soit à dire.

Les agrégats de §7.1 vont donc dans `packages/coordination`, crate distinct, et la frontière est une
règle d'import triviale. C'est le raisonnement d'ADR 0013 pour `packages/projections` : ce qui justifie
le crate est la **garantie testée** qu'il porte, ici la frontière elle-même. Le coût assumé est que le
modèle de domaine canonique de §7 se lit désormais dans deux crates.

Le crate n'est **pas créé par cet ADR** : la convention du dépôt est qu'un répertoire apparaît quand il
porte une garantie testée, et un crate vide n'en porte aucune. Il apparaît en W13.c. D'ici là la règle
examine zéro fichier, ce que `check:boundaries` **imprime** au lieu de le taire — comme la règle 4
avant qu'`apps/locusd` existe — et ce qu'elle garantit est vérifié sur fixtures.

---

## Décision 2 — Aucun vocabulaire parallèle : §7.1, §13, §20 et §22 sont le vocabulaire

Les objets de gouvernance sont ceux du texte normatif, sous leur nom : `AgentTemplate`,
`AgentInstance`, `Team`, `Decision`, `ApprovalRequest` (§7.1, schémas complets) ; les actions du
portefeuille (§13.5) ; `Policy` et les cinq verbes (§20.2) ; `Delegation` (§20.4) ; les commandes
`team.modify`, `agent.spawn`, `decision.propose`, `approval.respond` et le `CommandEnvelope` à
`expected_revision` (§22.2, §22.3).

Il n'est créé **ni** `MutationPolicy`, **ni** `MutationGrant`, **ni** `AgentAuthorityMode`, **ni**
`TopologyPatch` comme commande, **ni** échelle d'autorité à cinq barreaux, **ni** contrôleur de
mutation.

**Motifs.** Deux vocabulaires de gouvernance pour la même chose, c'est deux constitutions pour une
république : le conflit de normes est une question de temps, et il se découvre au moment d'un
arbitrage. Un dossier de recherche externe a proposé l'ensemble de ce vocabulaire parallèle ; il ne
citait jamais §7.1, §13, §16 ni §22, et c'est la cause racine de la totalité de ses doublons.

Une échelle ordonnée à cinq barreaux serait en outre une erreur de catégorie : une échelle invite à
monter d'un cran, or l'extension de capacité n'est pas un degré supérieur de la modification de
coordination, c'est une capacité **disjointe** (décision 8).

Et §14.2 donne une règle plus forte que l'état de l'art : « les capacités effectives sont
l'**intersection** de la mission, du template, de la politique locale et de l'attestation du worker ».
C'est la *capability monotonicity* d'ATM en plus fort — une intersection sur quatre sources interdit
structurellement le gain de capacité, là où une comparaison avant/après le détecte après coup.

---

## Décision 3 — §14 est amendé sur deux points, et deux seulement

**Une relation de coordination entre deux `AgentInstance` nommées.** §14.3 et `Team.coordination_mode`
ne donnent que des modes *globaux* : un mode s'applique à une équipe entière. Aucun objet ne dit « cet
agent-ci révise cet agent-là ». C'est l'asymétrie que la relation comble, et c'est un ajout à `Team`,
non un objet parallèle.

**Une proposition de modification structurelle, symétrique de la proposition de spawn de §14.5.**
§14.5 donne l'ajout d'un nœud sans la modification d'une relation ; §13.5 dit ce que le portefeuille
peut faire sans dire comment une action est proposée, versionnée et annulée. La proposition est le
payload de `team.modify` (§22.3), et elle conserve du schéma d'un dossier externe ce qui est bon :
`trigger`, `rationale`, `evidence_refs`, et un `proposer.kind` à quatre valeurs — `human`, `agent`,
`telemetry`, `policy` — la quatrième couvrant le cas où §13.5 agit.

Rien d'autre n'est amendé. Les deux manques auraient été rencontrés de toute façon à W7, dont la revue
indépendante a besoin d'instances distinctes et de savoir qui relit qui.

---

## Décision 4 — Aucune sémantique inerte

L'énumération des sortes de relation est **fermée**, et une valeur n'y entre que lorsqu'un
consommateur exécutable et testé existe dans le dépôt. W13.e livre **une seule** valeur : `review`.

**Motifs.** `review` a **trois** points d'application vérifiés, dans trois couches, et c'est ce qui la
distingue d'un champ de configuration versionné : `review_policy` de la `MissionEnvelope`, consommé par
`selectOverlay` du worker, décide **vers qui** la mission part ; `WorkflowKind::Review` de
`packages/workflow`, implémenté et testé sur le backend déterministe de W3.b, décide **ce qui est
orchestré** ; et la classe de tâche `review-isolated` de §12.1 — « contexte aveugle et lecture
seule » — décide **comment le scheduler place**. Une relation qui contraint le routage, l'orchestration
et le placement n'est pas un champ déguisé.

`dependency` **ne qualifie pas encore**, et la vérification est consignée ici pour qu'elle ne soit pas
refaite : `packages/workflow/src/definition.rs` porte `steps: Vec<Step>`, une séquence ordonnée **à
l'intérieur** d'un workflow, et §11.2 nomme onze workflows métier, pas un exécuteur de graphe
générique. Rien n'ordonne des attempts **entre instances d'agent**. Le scheduler de §12 concerne le
placement, pas l'ordre : deux gates distinctes, à ne pas confondre.

`message`, `data`, `control` et `state-route` n'ont aucun consommateur : il n'existe ni bus de messages
inter-agents ni canal de données entre nœuds. Les inclure produirait des relations que le système sait
versionner, différencier, approuver et afficher, et que **rien n'honore** — un graphe décoratif, pire
qu'un graphe absent parce qu'un humain croira l'avoir modifié.

`role` et `visibility` ont chacune un consommateur candidat identifié — les `extraInstructions`
additives de `agent-overlay.ts` pour la première, la construction de `ContextView` pour la seconde — et
sont les deux prochaines à instruire.

C'est la même discipline que l'enum fermé de `event_type` dans `event.schema.json` (« un type
d'événement inconnu n'est pas un champ qu'on peut ignorer ») et que la règle « un répertoire apparaît
quand il porte une garantie testée ».

**Symétrique à tenir :** un type paramétré qui n'a jamais eu qu'une valeur n'a pas démontré qu'il en
supportait deux. D'où la décision 10.

---

## Décision 5 — Le versionnement est celui de la spec, et l'annulation est un commit inverse

La base de version d'une proposition **est** l'`expected_revision` du `CommandEnvelope` de §22.2, servi
par `Expected::Exact` de `packages/event-store`. Aucun compteur, aucun magasin, aucun bus n'est créé.

Une annulation est le **commit d'un changement inverse**, produisant une nouvelle version dont le
parent est la version courante et qui référence celle qu'elle annule. Aucune version antérieure n'est
supprimée.

**Motifs.** `store.rs` porte déjà tout : `Expected`, qui « n'a pas de variante “peu importe” — un
écrivain qui ne sait pas sur quelle révision il construit n'a rien vérifié » ; `stream_revision`
attribué par le journal ; l'idempotence par commande. Et §9.2 fixe la chaîne d'écriture avec atomicité
entre événements, révision d'agrégat et outbox.

Une annulation qui retirerait une version rendrait l'histoire fausse : on ne pourrait plus dire qu'une
mission a tourné sous une organisation qui, désormais, n'aurait jamais existé. C'est l'esprit de
l'invariant 12 et la forme du ledger. Corollaire : une modification non inversible ne peut être que
**compensée**, et elle le déclare à la proposition — comme ADR 0014 et la convention `[R]`/`[M]` le
font pour les migrations.

---

## Décision 6 — L'organe de décision est l'orchestrateur de portefeuille, pas un contrôleur neuf

§13.5 liste déjà « composer ou renforcer une équipe » et « créer un agent-pont » parmi les actions du
portefeuille, avec `V(b)` (§13.4), ses quinze indicateurs (§13.2), ses contraintes de qualité-diversité
(§13.3), et l'exigence : « les actions dépassant les seuils de coût, de confidentialité ou d'impact
exigent une `ApprovalRequest` ».

La décision d'une mutation de coordination appartient donc au moteur de politique (§20) et au
portefeuille (§13), à l'intérieur de `locusd`. Ce qui est neuf est l'objet de proposition, son
versionnement et sa contestabilité — pas le décideur.

**Conséquence de dépendance, qui corrige une erreur de rédaction antérieure :** le mode `bounded`
dépend de l'anti-gaming de §13.6, qui doit détecter « la production de tâches pour maximiser
l'activité » — exactement le comportement d'un agent proposant des recâblages pour paraître utile. Et
`docs/10` §W7 avertit déjà que « l'anti-gaming du portefeuille doit exister **avant** que la fonction
de valeur pilote des décisions automatiques ». `bounded` dépend donc de W14, pas seulement de la
sandbox.

---

## Décision 7 — Un agent est auteur de proposition dès W13 ; seul l'auto-commit est gaté

Une proposition écrite par un agent est **le même objet** qu'une proposition humaine et suit le même
chemin : validation, politique, décision, approbation, commit. W13.e livre l'autorité de proposition
agentique.

**Motifs.** §14.5 est écrit pour l'auteur agentique : ses onze déclencheurs —
`domain_gap_detected`, `review_disagreement`, `barrier_encountered`, `branch_stagnation`,
`formalization_blocked`, `counterexample_needed`, `new_method_found`, `bridge_candidate`,
`high_uncertainty`, `reproduction_failure`, `source_conflict` — sont des faits que l'agent observe, pas
l'humain. Et la même section conclut : « le moteur de politique peut accepter, refuser, modifier ou
soumettre à approbation. Aucun agent ne crée librement une flotte non bornée. » Le texte normatif
prévoit l'auteur **et** sa borne, dans la même phrase.

Confondre *qui écrit la proposition* avec *qui la commite* coûterait la partie la plus intéressante de
la première tranche. Le premier ne demande aucune machinerie de plus ; le second demande une classe de
risque dérivée, l'anti-gaming, et une ombre pour les opérations qui la justifient.

**La borne à ne pas relâcher :** le proposeur ne peut pas être l'approbateur de sa propre proposition.
§20.3 a déjà `forbid_self_approval`. C'est plus général que de détecter un conflit d'intérêt au cas par
cas, et c'est ce qui empêche un agent de contrôler les règles décidant de son propre remplacement.

---

## Décision 8 — Deux modes, dont le défaut interdit à l'agent de modifier le graphe

Il existe un mode dans lequel un agent **ne peut pas** proposer de modification de coordination, et un
mode dans lequel il peut. Le défaut est le premier. Les modes sont déclarés au niveau du workspace ou
du programme, et **le changement de mode est lui-même un acte gouverné et journalisé**.

| Mode | Ce que l'agent peut | Mécanisme | Disponible |
|---|---|---|---|
| `observed` | signaler un besoin, sans proposer | aucune `Delegation` ne couvre l'action | W13.e |
| `assisted` | proposer ; un humain approuve | `Delegation` + `require_approval` | W13.e |
| `bounded` | committer dans une région, sous plafond de budget et classe de risque dérivée | `allow` dans les bornes de `scope` et `budget_ceiling` | W14 + W16 |
| `operator` | opérations privilégiées, réparation, rollback forcé | humain nommé, jamais un agent | W14 |

**Motifs.** §33 fait de « rendre toute action autonome sans seuil humain » un **non-objectif explicite
de la V1** : le mode fermé n'est pas une précaution, c'est une exigence. Et le mode est la seule partie
du système qui pourrait échapper à la provenance, alors que c'est lui qui détermine ce que les agents
ont le droit de faire ; le journaliser répond à la question que la littérature nomme « sécurité des
agents auto-modifiants », dont la première réponse est qu'un agent ne contrôle pas l'espace des
opérations qui lui sont permises.

**L'extension de capacité est un axe orthogonal, pas un cinquième barreau.** Un déploiement peut être
en `bounded` sur la coordination et interdire toute capacité nouvelle, ou l'inverse. Sa **forme** est
décidée ici : une capacité nouvelle arrive comme `EnvironmentBlueprint` construit, scanné, signé et
attesté (§19.3), consommé par une mission dont `locus-execd` juge l'admission — jamais comme du code
injecté dans un processus. Ce que la littérature et les harnais tiers appellent « système de plugins »
est ici une **admission de capacité**, dont Locusolus possède déjà le blueprint, l'artefact,
l'attestation et le refus nommant toutes ses conditions. Ce qui manque est la proposition, la politique
et l'approbation : du travail de gouvernance.

Trois bornes ne se relâchent dans aucun mode : `forbid_self_approval` ; la capacité effective reste
l'intersection des quatre sources de §14.2 ; aucun worker n'atteint la base canonique (invariant 3).

---

## Décision 9 — Un objet simulé n'existe pas comme type dans le domaine épistémique

La garantie est une **absence de type**, pas un champ de classification.

**Motifs.** Un champ `evidence_class` repose sur le fait que *chaque* consommateur le vérifie ; c'est
le genre d'invariant qui tient six mois. Pis, `packages/validation/src/propagation.rs` implémente la
propagation de l'invalidation sur les niveaux de §8.1 : ajouter un niveau `simulated` ferait circuler
la simulation dans cette machinerie. Un résultat de simulation vit donc dans le domaine de
coordination, comme propriété d'une *proposition*, jamais d'un *claim*, et aucun chemin de type ne l'y
convertit — vérifié par un test d'absence et par la règle de la décision 1.

**Corollaire pour la contestabilité, qui est la contribution originale du projet.** `ObjectionTarget`
distingue déjà l'objection à une prémisse, à la règle et au scope, parce que « sur trois arêtes
indépendantes, “la règle est fausse” n'a aucun endroit où s'accrocher ». La même finesse appliquée au
déclencheur, à la politique et au périmètre d'une décision de coordination donnerait un système où
l'histoire de l'organisation est réfutable comme un claim. Cela **ne se fait pas** en ajoutant une
variante à `ObjectionTarget`, mais par une famille parallèle dans le domaine de coordination,
partageant la forme sans partager le crate, avec un test vérifiant l'absence de conversion. Il faudra
aussi refuser la factorisation par le haut : un trait générique « ce qui peut être objecté, relu,
réfuté, remplacé » serait la conversion reconstruite. La duplication est ici le choix correct et porte
une ligne de justification dans le code, dans l'esprit de la double liste de coalescence du worker,
gardée redondante exprès pour qu'un test vérifie qu'elle ne se recoupe pas.

---

## Décision 10 — Le domaine n'est pas baptisé collectivement, et la décision est falsifiable

**Aucun nom collectif n'est fixé.** Ni « topologie », ni « graphe organisationnel », ni « graphe de
coordination » n'est retenu comme nom de domaine. `packages/coordination` nomme un **emplacement**
imposé par la forme de la garde (décision 1) ; ce n'est pas une thèse sur ce que le domaine est, et les
types y sont nommés individuellement, sous les noms de §7.1.

**Motifs.** Le risque qu'un nom collectif était censé écarter — un `TopologyNode` générique mélangeant
une instance d'agent et un runtime d'outil — est déjà écarté par la décision 2, puisque §7.1 fournit
l'identité de l'agent. Choisir un nom étroit garantit un rename le jour où un validateur ou un routeur
entre dans le domaine ; choisir un nom large fait promettre ce qui n'est pas tenu. Le nom est en aval
de la thèse, pas en amont.

**Clause de falsification.** Le deuxième membre de l'énumération de la décision 4 sera `role`, dont le
consommateur candidat est l'overlay additif du worker. Son ajout est le test de cet ADR :

- s'il se branche en modifiant l'énumération, la projection et un point d'application, l'abstraction
  tient et le nom collectif peut alors être choisi en connaissance de cause ;
- s'il exige de refondre la représentation parce que la première relation avait été taillée pour
  `review_policy`, **alors l'abstraction n'existait pas** : ce qui aura été construit est un champ de
  configuration versionné avec une machinerie de gouvernance autour — utile, mais qui ne doit pas
  s'appeler un graphe. Le constat est écrit au ledger et la décision 3 est rouverte.

**Amendement du 2026-08-18 : la sonde est `visibility`, pas `role`.** En instruisant W15.e, une
troisième issue est apparue, que la clause n'avait pas prévue — **la sonde était mal choisie**.
`role` n'est pas une relation : `SPEC_V1.md` §7.1 en fait un champ d'`AgentTemplate`, §20 une
classification dans une exigence de reviewers (`- role: logical-reviewer`), §6.3 un attribut
d'appartenance héritable ; nulle part il n'a la forme *A → B*, et `packages/coordination/src/agent.rs`
le portait déjà comme attribut avant que la question soit posée. W15.a l'avait classé indépendamment
parmi les opérations **attributaires** différées, et les deux analyses concordent.

Exécuter la clause telle qu'écrite aurait donné une réponse fausse dans les deux sens : forcer une
charge sur la sorte (`Role(String)`) pour condamner l'abstraction sur un test qu'on ne lui a jamais
fait passer, ou inventer une sémantique paire que la spec n'énonce pas pour la déclarer valide sans
l'avoir éprouvée.

La sonde devient donc `visibility`, qui est réellement de forme paire — « A voit ce que B a
produit » — et dont le consommateur, la construction de `ContextView` (décision 11), vit dans le
dépôt et s'éprouve de bout en bout. Les deux branches de la clause sont inchangées ; seule la valeur
qui l'exerce l'est. `role` reste dû, comme **attribut** : c'est `SET_ROLE`, dont le lecteur candidat
est l'overlay additif du worker, et il entre quand ce lecteur existe.

---

## Décision 11 — La mémoire est la substance de la coordination

§16 est normatif et suffisant sur trois points : sept niveaux de mémoire (§16.1) ; retrieval hybride
combinant traversée de graphe, lexical, vectoriel, identifiants exacts, temporalité, validation,
confidentialité, diversité et résultats négatifs, avec ranking dont les facteurs sont exposés et
embeddings ne contournant pas les ACL (§16.3) ; `ContextView` immuable et hashée (§16.2). §9.1 fait des
vecteurs et graph databases des projections reconstructibles ; §9.3 fait de l'index mémoire hybride une
projection obligatoire.

**Ce qui est décidé ici :** recâbler une relation change qui peut lire quoi, donc le graphe de
coordination **est** le graphe de circulation de la mémoire — ce que MANTA appelle visibilité de
l'information et chemins de validation. La sorte de relation `visibility` a donc pour consommateur la
construction de `ContextView`, et elle est la troisième à instruire.

**Trois ajouts.** La détection du **consensus circulaire** exigé par §16.6 : un cycle de `Cites` dont
aucun nœud n'a d'arête `AnchoredIn` vers une source externe — calculable sur les types existants de
`packages/graph`, non fait par aucun système recensé, publiable seul, et ne dépendant ni de W4 ni de
`locusd`. La **contamination vérifiée au recâblage** : une proposition créant une route qui donnerait
au reviewer aveugle accès au raisonnement du générateur est refusée avant validation sémantique. Et la
**séparation des deux retrievals**, épistémique et organisationnel, sans conversion.

**Trois refus.** Un vecteur ou graph store canonique (§9.1). Un agent propriétaire de la mémoire
(second chemin d'écriture). La fusion automatique de quasi-duplicats (§16.4 l'interdit).

---

## Décision 12 — La cible est de la V1, inscrite comme telle, et mesurée sur une échelle

`docs/10` ligne 3 : « Ce n'est pas une roadmap de MVP. Tous les workstreams appartiennent à la V1
finale ; l'ordre reflète les dépendances de construction. » Le moteur de politique, le portefeuille, le
cœur du graphe agentique, la reconfiguration vivante, le cockpit, l'orchestration de la mémoire et
l'adaptation automatique sont donc **des workstreams de la V1** — W14 à W18 — et non un programme
annexe. Chacun déclare le niveau de dynamisme qu'il fait franchir sur l'échelle à cinq niveaux, et un
workstream qui ne sait pas le dire n'a pas de critère de fin.

Le grain grossier de W14 à W18 n'est pas un renoncement : c'est la convention du fichier — « les
découper finement aujourd'hui produirait un plan faux ; chaque workstream est redécoupé au commit près
quand il devient le prochain ». `docs/13` porte l'état de l'art vérifié et la cible détaillée.

**Condition de réexamen propre à cette décision :** si W14 à W18 sont encore des blocs au grain
grossier six mois après la fin de W4, ce n'est plus un ordonnancement mais un renoncement non écrit. Le
constat se fait au ledger et oblige soit à découper, soit à retirer explicitement des items.

---

## Décision 13 — W13 ne prend jamais la priorité sur W4

W13 contient exclusivement : les agrégats de §7.1, la complétion de l'agrégat `Task`, la relation de
coordination, la proposition de modification, l'autorité de proposition agentique, et deux projections.
C'est du travail de repli quand une décision bloque ailleurs, comme `docs/10` §W10 le dit des items
xiiif. Les périmètres ne se recoupent pas : W4.d touche `apps/locus-execd` et ses drivers, W13 touche
`packages/` et `docs/`.

**Motif.** Le risque de ce chantier n'est pas conceptuel — la spec couvre l'essentiel — il est
attentionnel. Le dépôt en est à W4.c sur douze workstreams, sans client, sans daemon, sans persistance,
sans interface. La coordination gouvernée est le sujet le plus intéressant et le plus publiable, donc
le meilleur candidat pour absorber toute l'attention.

**La borne symétrique, à ne pas perdre :** une part de W13 est en **amont** de deux workstreams — la
revue indépendante de W7 a besoin d'instances distinctes, et le cockpit de W8/W9 a besoin d'une
identité unique traversant ses vues. « Repli » et « jamais prioritaire » ne peuvent donc pas signifier
« jamais planifié » : W13.c et W13.d sont inscrits comme **dépendances de W7** dans `docs/10`, et c'est
cette dépendance qui décide de leur moment, pas l'étiquette.

---

## Conséquences

`CLAUDE.md` gagne une sixième frontière et une règle. `boundaries.json` gagne deux catalogues et deux
règles, une par sens.

**Amendement du 2026-08-18, à la livraison de W15.d.** La décision 9 exigeait « un test vérifiant
l'absence de conversion » entre les deux familles d'objection, sans dire où il vivrait. La réponse
est qu'il ne peut vivre dans aucun des deux crates : la décision 1 leur interdit de se voir, donc
ni l'un ni l'autre ne peut nommer l'autre famille, fût-ce pour affirmer qu'il ne la convertit pas.
Une conversion ne peut naître que dans un **troisième** fichier qui importe les deux. `CLAUDE.md`
gagne donc une **septième** frontière — « aucun fichier ne voit les deux familles d'objection à la
fois » — et `boundaries.json` deux catalogues au grain du **symbole** et une règle d'une nature
nouvelle, `no-co-import`, qui interdit une conjonction plutôt qu'un import. Elle interdit du même
coup le trait générique que la décision 9 refusait : pour le déclarer sur les deux familles, il
faudrait les voir toutes les deux. `docs/10` gagne W13 découpé au commit près et W14 à W18 au grain grossier.
`docs/11` gagne les lignes de coordination, de modes et de métriques structurelles. `docs/13` entre
comme document subordonné.

`packages/protocol` gagne quatre natures d'identifiant — `Team`, `Task`, `Decision`, `Approval` — via
la macro existante. C'est additif, mais ce package est « le goulot du projet entier » et deux
consommateurs en dépendent : un test de round-trip accompagne l'ajout.

**`packages/event-store` ne change pas.** `EVENT_NAMESPACES` contient déjà `agent`, `team`, `decision`,
`approval` et `policy`, et seul le namespace est vérifié, pas le verbe. Une rédaction antérieure de cet
ADR annonçait un amendement de taxonomie : il n'est pas nécessaire.

**Aucun item de W13 ne touche `schemas/lep/1.0`.** Un recâblage change la valeur de `review_policy`
dans l'enveloppe du prochain attempt ; le champ existe. En revanche, **cet ADR n'affirme pas qu'aucun
mineur LEP ne s'ouvrira avant W16** : une rédaction antérieure le disait, et les arbitrages du
2026-08-17 l'ont démentie le jour même. Deux d'entre eux exigent un `lep/1.1` — la permission de
fonctionnement hors ligne, activable et désactivable, que la `MissionEnvelope` ne sait pas exprimer
aujourd'hui, et les codes de refus d'admission sur le fil. Ce mineur a son propre ADR ; W13 n'en
dépend pas et ne l'ouvre pas.

**Aucune modification de `canterel`**, à aucun item de W13.

Un coût assumé : deux familles d'objection qui se ressemblent sans se convertir, donc de la duplication
délibérée, avec sa justification écrite dans le code. Un second : le modèle de domaine canonique de §7
se lit désormais dans deux crates.

---

## Plan de rollback

La décision 1 est additive et se retire seule : deux règles de `boundaries.json`, trois fixtures et une
ligne de `CLAUDE.md` s'annulent par un diff, et rien n'en dépend au moment où elles sont posées — c'est
précisément pourquoi la frontière est posée maintenant, le seul moment où elle est gratuite, comme le
test de séparation d'Emacs en tête de W8.

Les décisions 2 à 4 et 8 à 12 sont documentaires. Avant W13.c, revenir coûte l'annulation de cet ADR et
le retrait du workstream.

Après W13.c, les agrégats sont dans `packages/coordination` et rien ne les consomme encore : revenir
coûte la suppression d'un crate. Après W13.e, un retour sur la décision 4 — ouvrir l'énumération à des
sortes sans consommateur — ne casserait aucun test, et c'est exactement le danger : c'est le seul
rollback qui coûte une garantie plutôt qu'un diff, et c'est pourquoi la décision est prise quand
l'énumération n'a qu'une valeur.

Aucune donnée n'est en jeu : l'event store est en mémoire, les projections sont reconstructibles par
construction, et le journal qui les alimente l'est aussi.
