# ADR 0022 — La mémoire a deux dimensions, et le retrieval est un plan

**Statut :** accepté. **Amende** `ADR 0016` décision 4 sur un point précis (décision 0 ci-dessous) et
`CLAUDE.md`, qui la cite. **Amende** `W17.a` et `W17.b`, dont les types gagnent une dimension.
**N'amende pas** `SPEC_V1.md` §16 : les sept niveaux de §16.1 et les dix signaux de §16.3 sont repris
tels quels, et un test tient leur nombre et leurs noms. Ouvre `W17.k` à `W17.n` et `W18.g`.

**Contexte.** `packages/memory` répond aujourd'hui à deux questions : **qui a le droit de voir**
(`Level`, les sept portées de §16.1) et **est-ce régénérable** (`Substance`). Il ne répond pas à la
troisième, qui est **par quelle autorité est-ce vrai**. Et `retrieve` applique les dix signaux de
§16.3 en une passe, sans intention de requête, sans ordre de canaux et sans critère d'arrêt.

Les deux manques ont le même effet : ils font perdre à la mémoire les distinctions que le projet
existe pour préserver.

**Ce que cet ADR corrige de sa propre proposition initiale.** Le document d'où viennent ces décisions
annonçait six points comme acquis qui ne le sont pas ; l'instruction dans le code les a démentis. Les
corrections sont **dans** les décisions, à leur place, et non reléguées en annexe — un ADR qui
renverrait ailleurs pour savoir ce qu'il décide serait le début de la dispersion qu'on veut éviter.
Chacune est signalée par **« vérifié »** et porte ce qui a été lu.

---

## Décision 0 — On ne livre jamais une promesse ; on livre toujours une capacité

> Une **promesse** est un type qui annonce un effet qui n'a pas lieu. Une arête `message` qu'aucun
> routeur n'honore ment à l'humain qui croit l'avoir posée : elle reste interdite.
>
> Une **capacité** est un sous-système complet et testé dont personne n'a encore eu besoin. Elle est
> finie. Une cuisine équipée dont personne n'a cuisiné est finie ; une cuisine avec une gazinière
> peinte sur le mur ne l'est pas.

Conséquence : « aucun appelant ne l'utilise encore » **n'est pas un motif de report**. Les deux seuls
motifs admis restent une **dépendance technique nommée** et un **hôte externe absent**, comme
`W18.f` le fait avec `attend:externe`.

**Ce que cela amende, et ce que cela n'amende pas.** `ADR 0016` décision 4 — « aucune sémantique
inerte » — **tient mot pour mot** pour ce qu'elle vise : l'énumération des sortes de relation de
coordination reste fermée, et une valeur n'y entre que lorsqu'un consommateur exécutable et testé
existe. Une sorte de relation sans consommateur est une promesse au sens ci-dessus, et le refus est
le même.

Ce qui est amendé est sa **généralisation** à des sous-systèmes entiers, faite par des sessions
ultérieures et jamais décidée : de « une valeur d'énumération sans consommateur ment » on avait tiré
« un module sans appelant est prématuré ». Les deux ne se ressemblent que de loin. Une valeur
d'énumération est une **affirmation** — elle dit qu'un effet existe ; un module est un **outil** — il
ne dit rien tant qu'on ne l'appelle pas.

**Ce que la correction rouvre, et qu'il faudra rouvrir sciemment.** Deux endroits de ce dépôt ont été
reportés sous la lecture large, et redeviennent recevables sans devenir obligatoires : les verbes
`message.read` / `message.acknowledged` / `message.expired` de l'`ADR 0019`, et les trois opérations
attributaires absentes de `version.rs` — `SET_VISIBILITY`, `SET_VALIDATOR`, `SET_EXECUTION_ORDER`.
Chacune reste soumise à son propre examen : `SET_VALIDATOR` attend qu'un validateur soit un nœud, ce
qui est une dépendance technique nommée et non une frilosité. Aucune ne s'ouvre par le seul effet de
cette décision.

---

## Décision 1 — Le genre est une dimension obligatoire, orthogonale à la portée

Dix genres, liste close, dans la forme exacte de `Level` : énumération fermée, `ALL`, `name`,
`parse`.

| Genre | Contient | Ce qui fait autorité |
|---|---|---|
| `Episodic` | attempts, actions, échecs, décisions | l'histoire observée — le journal |
| `Semantic` | claims validés, concepts, relations | la validation épistémique §8.1 |
| `Formal` | lemmes vérifiés, termes de preuve, dépendances | un vérificateur, jamais un consensus |
| `Negative` | échecs, contre-exemples, routes impossibles | l'observation ou la vérification, invariant 12 |
| `Procedural` | skills, workflows, outils réutilisables | des tests exécutables |
| `Strategic` | tactiques, patterns de décomposition | l'utilité empirique mesurée |
| `Literature` | sources, citations, provenance bibliographique | la provenance de source |
| `Computational` | résultats de calcul, expériences numériques | la reproductibilité §19 |
| `Coordination` | qui sait quoi, qui a besoin de quoi | temporaire, jamais canonique |
| `MetaMemory` | fiabilité d'une source, utilité passée d'un retrieval | métadonnée apprise |

Genre **et** portée sont tous deux obligatoires : une mémoire dont le genre n'est pas nommé n'existe
pas, exactement comme `W17.a` l'a posé pour le niveau.

**Le type s'appelle `Genre`, pas `Kind`** — *vérifié* : `packages/memory/src/lib.rs` exporte déjà
`compaction::Kind`. Ce crate a d'ailleurs subi la même pression une fois, et l'a résolue dans le même
sens : `dedup::Candidate` est exporté comme `DuplicateCandidate` parce que `retrieval::Candidate`
tenait le nom. Deux `Kind` dans un `use` seraient renommés à l'import par chaque appelant, ce qui est
la duplication de vocabulaire sous une autre forme.

**Motifs.** La portée dit **qui peut voir** ; le genre dit **ce que le lecteur a le droit d'en
faire**. Ce sont deux propriétés différentes, et les aplatir coûte cher. Trois cas suffisent.

Sans genre `Formal`, un lemme vérifié par un checker se range par proximité d'embedding à côté d'une
conjecture : la machine cesse de distinguer « démontré » de « qui ressemble à ». Sans genre `Negative`
distinct, la `negative_result_policy` de §16.2 n'a aucun ensemble sur lequel réserver un budget. Sans
`MetaMemory` séparée, l'utilité passée d'un document finit par entrer dans son score de vérité — le
biais de citation reconstruit avec de l'apprentissage automatique.

---

## Décision 1 bis — Quand un genre recoupe un type existant, le désaccord est un refus

*Vérifié, et c'est la correction la plus lourde de cet ADR.* Quatre des dix genres recouvrent des
distinctions que le dépôt encode **déjà**, ailleurs :

| Genre | Ce qui existe déjà | Où |
|---|---|---|
| `Negative` | `CoreObjectType::NegativeResult`, l'un des quarante de §7.3 — **et** l'agrégat `NegativeResult` de §18.7, avec `Power`, `Exclusion`, `CONCLUSIVE_POWER` | `packages/domain` |
| `Formal` | `FormalizationStatus` | `packages/graph` |
| `Computational` | `reproducibility::{Assessment, Level, Missing}` | `packages/artifacts` |
| `Coordination` | l'agrégat de coordination en entier | `packages/coordination` |

Le retrieval en porte même une cinquième trace : `Candidate.is_negative: bool`. Inscrire
`Genre::Negative` sans rien décider en ferait la **quatrième** représentation de la même notion, et
ce dépôt vient de payer exactement cela — `ADR 0021` a retiré `proposal::Change` parce qu'il
redisait, dans un second vocabulaire, ce que `version::Operation` disait déjà.

**Décision.** Le genre reste **déclaré**, et non dérivé. Une `Entry` doit être auto-descriptive : la
faire dépendre d'une résolution de type obligerait `packages/memory` à connaître `graph`,
`artifacts` et `domain`, et un rangement échouerait parce qu'un résolveur est absent — ce qui est
absurde pour une mémoire.

Mais **là où un type est connu, le désaccord est un refus et jamais un silence.** La vérification
passe par un port que l'appelant fournit — la même forme que `EpistemicIndex` dans `proposal.rs`,
qui demande « cette révision existe-t-elle » et rien de plus. Un rangement qui déclare
`Genre::Semantic` sur une clé que le port résout en `NegativeResult` est **refusé en nommant les
deux**. Un rangement dont la clé n'est connue d'aucun port est **accepté** : l'ignorance n'est pas un
démenti, et c'est la règle que `xiiif` applique déjà en ne collapsant pas `unverified` sur `broken`.

**Motifs.** C'est la seule forme qui donne les deux propriétés qu'on veut ensemble. Le genre reste
lisible sans jointure, donc la mémoire fonctionne seule ; et deux sources ne peuvent pas diverger en
silence, puisque leur désaccord est une erreur. Un genre purement dérivé aurait la seconde propriété
et pas la première ; un genre purement déclaré, l'inverse.

---

## Décision 2 — L'autorité contraint le retrieval, et trois interdits sont nommés

L'autorité d'un genre n'est pas une étiquette : elle est appliquée.

**Un objet `Formal` ne se classe pas par similarité vectorielle.** Son autorité est un vérificateur ;
un score de proximité n'a aucune relation avec elle. Le couple `(Genre::Formal, Signal::Vector)` est
refusé, et il n'existe pas de chemin pour l'écrire.

*Vérifié — et le refus coûte plus cher que prévu.* `Ranking::of(&[(Signal, f64)])` ne connaît pas le
candidat, donc le refus ne peut pas s'y poser ; c'était l'analyse d'origine et elle est juste. Mais
`Candidate` **n'a pas de constructeur** : ses quatre champs sont `pub` et elle se construit par
littéral. Poser le refus « à la construction du candidat » exige donc de **privatiser les champs et
d'ajouter un constructeur faillible** — une rupture d'API sur du code livré, à chiffrer comme telle
dans `W17.k`.

Le choix reste le bon malgré le prix : un `Ranking` valide qui deviendrait invalide en étant attaché
à un candidat serait un état intermédiaire invalide représentable, ce que ce dépôt évite partout
ailleurs.

**Un objet `MetaMemory` influence le rang, jamais la validité.** Il peut contribuer à un `Ranking` ;
il n'entre jamais dans une `Support` de `packages/graph` ni dans une prémisse d'`Inference`. Tenu par
l'absence de conversion.

**Un objet `Negative` a un budget réservé — et la réserve appartient au `Plan`.**

*Vérifié, et cette correction sauve la promesse d'additivité.* `retrieve` trie par score puis coupe
au rang : `position >= budget` produit `BeyondBudget`, et `is_negative` **n'est jamais lu dans ce
chemin**. Aujourd'hui les négatifs sont favorisés au **classement**, par `Signal::NegativeResults`,
et nullement protégés au **budget**.

Faire de la réserve une propriété du genre changerait donc silencieusement le comportement de code
livré, et rendrait faux l'engagement qu'un `Plan::default()` reproduit `retrieve` d'aujourd'hui. La
réserve est donc un champ du plan, sa valeur vient de la `negative_result_policy` de §16.2 — qui
existe dans la spec et n'est pas inventée ici —, et un plan sans politique attachée n'en a pas.

**Le défaut est écrit, pas tu.** Le reçu de la décision 6 porte la réserve, y compris quand elle vaut
zéro. Une garantie absente et une garantie nulle ne se lisent pas pareil, et c'est la même discipline
que `Option<Coverage>` plus bas.

---

## Décision 3 — Aucune conversion entre genres ; la promotion change le niveau

Une promotion — d'épisodique de branche vers sémantique de programme, par exemple — change le
**niveau** et laisse le **genre**. Aucune fonction ne transforme un genre en un autre.

**Motifs.** `packages/memory/src/separated.rs` a déjà établi cette discipline pour les deux
retrievals : « aucune conversion n'est écrivable, parce que le préfixe fait partie de l'identité ».
La même raison vaut ici, et elle est plus forte : une conversion de genre serait une **conversion
d'autorité**, c'est-à-dire l'affirmation qu'un objet est vrai pour une raison qui ne l'a jamais
établi. Un objet formel ne devient pas sémantique parce qu'il est beaucoup cité ; une stratégie qui a
souvent marché ne devient pas une preuve.

Refuser la factorisation par le haut est également décidé ici : un trait générique « ce qui peut être
rangé, retrouvé, promu » reconstruirait la conversion. La duplication est le choix correct et porte
sa justification dans le code.

---

## Décision 4 — Le retrieval est un plan : intention, canaux ordonnés, escalade enregistrée

`retrieve` gagne un `Plan`, composé d'une `Intent`, d'une suite ordonnée de `Channel`, d'un budget,
d'une réserve de négatifs, d'un critère d'arrêt et de l'identité de la fonction de classement
(décision 6).

Une `Intent` est ce que la question **cherche** : `Explanatory` — pourquoi X contredit-il H ;
`Episodic` — avons-nous déjà essayé ceci ; `Formal` — ce lemme est-il démontré ; `Bibliographic` —
d'où vient cette affirmation ; `Structural` — quelles conclusions reposent sur ce type d'argument ;
`Global` — que dit l'ensemble du dossier. Six, liste close.

Une **escalade** — expansion de graphe plus profonde, appel à un coprocesseur, élargissement de
périmètre — est **enregistrée**, et un résultat obtenu après escalade se distingue d'un résultat
obtenu directement. `Escalation` est un type, jamais un booléen : `DeeperGraph { from_depth,
to_depth }`, `BroaderScope { requested, granted_by }`, `Coprocessor { capability_id }`.

**Motifs.** Trois questions différentes ne se paient pas au prix de toutes les routes, et le travail
publié le mesure — `arXiv:2603.15658`, atelier ICLR 2026, formalise le retrieval comme un problème de
routage entre magasins et montre qu'un routeur oracle atteint une meilleure exactitude avec
substantiellement moins de tokens.

L'escalade doit être visible parce qu'elle **change la nature de la preuve** : un résultat trouvé
après élargissement du périmètre de branche n'a pas été obtenu sous les mêmes contraintes
d'isolation, et §12.4 dépend de cette distinction.

---

## Décision 5 — `Channel` produit, `Signal` classe, et les dix de §16.3 ne sont pas touchés

Les dix `Signal` restent la liste de §16.3, inchangée — *vérifié* : `Signal::ALL` est bien
`[Self; 10]`. Un `Channel` est une **route qui produit des candidats** ; un `Signal` est un facteur
qui les **classe**. Quatre canaux nouveaux :

`Formal` — récupération de lemmes par similarité d'état de preuve. `Structural` — récupération des
inférences de **même forme de prémisses**. `Regional` — régions IIIF, zones ALTO, régions de figure :
le graphe tient l'identité et la boîte, l'artefact tient les octets, le canal rend des identifiants
et **jamais** d'octets, tenu par l'absence de type. `Community` — résumés de communautés, pour
l'intention `Global` uniquement.

**Motifs.** Les dix signaux de §16.3 mélangent des routes (`GraphTraversal`, `Lexical`, `Vector`,
`ExactIdentifiers`), des filtres (`ValidationLevel`, `BranchAndConfidentiality`, `ContextBudget`) et
des objectifs (`SourceDiversity`, `NegativeResults`). C'est fidèle à la spec et il ne faut pas y
toucher — mais ajouter les canaux à cette énumération perpétuerait la confusion **et** modifierait
une liste normative. Deux axes, deux types.

**Le canal `Structural` a besoin d'un oracle de types que rien ne fournit aujourd'hui.** *Vérifié* :
`Graph` ne contient que `relations` et `inferences`, et `minimal_premise_sets` rend des `RevisionId`.
La forme d'une inférence est le multiensemble des **types** de ses prémisses, pas leurs identités ;
il faut donc une résolution `RevisionId → ObjectType` qu'aucun crate ne détient. Elle entre comme
port fourni par l'appelant, au même titre que celui de la décision 1 bis — nommée plutôt que
supposée.

C'est un canal propre à ce projet, et il mérite d'être dit : il faut des hyperarêtes pour le poser.
« Quelles autres conclusions reposent sur exactement ce type d'argument » est une question qu'aucun
index vectoriel ni aucun magasin de triplets ne sait formuler.

**`Community` n'est jamais un défaut.** Le survey `arXiv:2506.05690` montre que l'approche par résumé
global n'est pas universellement meilleure que des baselines simples. Canal sélectionné par
l'intention, jamais appliqué d'office.

---

## Décision 6 — Le reçu de retrieval est un fait durable et contestable

Chaque construction de `ContextView` produit un `RetrievalReceipt` écrit comme fait : l'intention, la
version de la politique de retrieval, **l'identité de la fonction de classement**, le watermark de
source, les canaux interrogés, les escalades, la réserve de négatifs, le nombre de candidats, les
identifiants retenus, les **exclusions par motif**, la couverture en preuve et en contre-preuve, et
les lacunes connues.

**Pourquoi l'identité de la fonction de classement en fait partie.** *Vérifié.* `Ranking::of` reçoit
des `(Signal, f64)` **calculés par l'appelant** ; ce crate ne produit aucun score. Un reçu qui
n'enregistrerait pas comment les scores ont été produits promettrait un rejeu déterministe sur une
entrée qu'il ne connaît pas : rejoué sous une autre fonction de classement, il rendrait d'autres
inclusions et un autre condensat. Le test passerait quand même, parce qu'une fixture est déterministe
par construction — c'est-à-dire qu'il ne testerait rien. Toute entrée de classement qui n'est pas
fonction du journal entre dans le reçu par son identité.

**La couverture se rend même à zéro.** `Option<Coverage>` où `None` veut dire « non mesurée » et
`Some(0.0)` « mesurée et nulle ». Ne pas les confondre est le point, et c'est la règle du dépôt :
`unverified` n'est pas un `broken` atténué, et `matches` rend trois réponses.

Le reçu est une cible de contestation, avec la famille d'objection que `W15` a construite pour les
décisions de coordination. La `ContextView` reste immuable et hashée (§16.2) : **la contestation vise
le reçu, jamais la vue.**

**Motifs.** Deux propriétés le rendent structurel plutôt que décoratif. Il rend le déclencheur
`DomainGapDetected` **auditable** : une lacune cesse d'être affirmée par un agent pour devenir
lisible dans un reçu — et la décision 1 de l'`ADR 0023` en dépend. Et il rend le retrieval
**réfutable** : on peut objecter à l'exclusion d'un résultat négatif comme on objecte à une prémisse.
Les systèmes de mémoire de 2026 rendent le retrieval auditable — `arXiv:2605.30771` le fait
déterministe et enregistré ; aucun ne le rend contestable.

---

## Décision 6 bis — La jonction n'existe pas, et c'est elle le travail

*Vérifié, et c'est la seconde correction lourde.* La décision 6 dit « chaque construction de
`ContextView` produit un reçu ». Trois faits l'en empêchent aujourd'hui :

1. `ContextView` vit dans **`packages/review`**, pas dans `packages/memory` ;
2. les deux `Cargo.toml` le confirment : **les deux crates ne se connaissent pas** ;
3. `ContextView::build` prend `&[(ContextItem, u64)]` et n'appelle jamais `retrieve` — qui rend des
   `Candidate` clés par `String`, quand la vue consomme des `RevisionId`.

Il n'y a donc **aucun chemin** entre retrieval et vue de contexte. Le reçu n'est pas un type à
ajouter : il est ce qui **relie** deux sous-systèmes construits séparément, et `W17.n` est cette
jonction.

**Décision.** Le reçu vit dans `packages/memory`, et `packages/review` gagne une dépendance sur
`packages/memory`. La direction se justifie : une vue de contexte se construit **depuis** un
retrieval, jamais l'inverse. Un troisième crate pour un seul type a été écarté — c'est la dispersion
appliquée au code.

---

## Décision 7 — Ce qui est refusé, et pourquoi

**Un magasin vectoriel ou un magasin de graphes comme source de vérité.** §9.1 en fait des
projections reconstructibles ; le graphe, les événements et les artefacts sont canoniques.

**Un propriétaire unique de la mémoire — « agent mémoire », « Memory Governor ».** La possession est
distribuée sur sept niveaux, et un propriétaire unique serait un second chemin d'écriture. La
décision appartient au moteur de politique §20 et au portefeuille §13, comme `ADR 0016` décision 6
l'a tranché pour la coordination.

**La fusion automatique de quasi-duplicats.** §16.4 l'interdit et `W17.d` l'applique.

**Un orchestrateur de retrieval appris qui agit.** Il peut **proposer** un plan ; le plan est
déclaratif, inscrit dans le reçu, et contestable. Un orchestrateur qui choisit sans laisser de plan
lisible est une décision sans provenance.

---

## Conséquences

`packages/memory` gagne `Genre`, `Plan`, `Intent`, `Channel`, `Escalation` et `RetrievalReceipt`.
`Entry` et `Shelf::store` gagnent un paramètre obligatoire, et **`Candidate` perd ses champs publics
au profit d'un constructeur faillible** : deux changements de signature sur du code livré, `[R]`,
sans migration de données puisque l'event store est en mémoire.

`packages/review` gagne une dépendance sur `packages/memory` (décision 6 bis). `packages/graph` n'est
pas touché. Aucune modification de `canterel`. Aucune extension de protocole.

Deux ports nouveaux, tous deux fournis par l'appelant : la résolution de type pour la cohérence de
genre (décision 1 bis) et pour la forme des prémisses (décision 5).

Les dix `Signal` de §16.3 sont **inchangés**, et un test le tient : leur nombre et leurs noms.

## Plan de rollback

Les décisions 1 à 3 se retirent par suppression de la dimension, tant qu'aucun consommateur externe
ne lit un genre. Après `W18.g`, le producteur d'observations lit des genres : revenir coûte alors le
producteur aussi.

Les décisions 4 à 6 sont additives — un `Plan::default()` reproduit exactement le comportement de
`retrieve` d'aujourd'hui, et c'est un test. La décision 2 est ce qui rend cet engagement tenable : la
réserve de négatifs étant dans le plan et non dans le genre, le défaut ne change rien.

La décision 6 bis se retire en retirant la dépendance, ce qui suppose de retirer aussi le reçu — les
deux sont le même changement, et le rollback le dit plutôt que de laisser croire à deux gestes.

Aucune donnée n'est en jeu : l'event store est en mémoire et les projections sont reconstructibles.
