# ADR 0023 — Le raisonnement d'ontologie est un coprocesseur, pas un substrat

**Statut :** accepté. **N'amende** aucune section de `SPEC_V1.md`. S'appuie sur l'`ADR 0022`, dont il
réutilise le `Plan` et le reçu sans en créer de second. Ouvre `W18.h` et `W14.e`.

**Contexte.** Le projet a besoin de classification et de vérification de cohérence sur des ontologies
— CIDOC CRM, BIBFRAME, référentiels institutionnels, ontologies de domaine — et n'en a aucune. La
question posée était : faut-il déplacer le graphe épistémique vers RDF/OWL ?

---

## Décision 1 — RDF/OWL n'est jamais le substrat du graphe épistémique

**Et le motif n'est pas la représentation.** Une correction s'impose, parce qu'une formulation
antérieure était fausse : RDF représente parfaitement les relations n-aires. Le motif est officiel
depuis avril 2006 — *Defining N-ary Relations on the Semantic Web*, Note du W3C par Noy et Rector —
et consiste à créer un individu qui est l'instance de la relation, relié à chaque participant par une
propriété binaire. Une inférence à trois prémisses s'écrit sans perte : un individu, trois arêtes
`hasPremise`, une `hasConclusion`, une `usesRule`.

Ce qui est exact, dans les mots du W3C, est qu'« une propriété est une relation **binaire** ». Il en
découle deux limites réelles, et une raison décisive qui n'est ni l'une ni l'autre.

**Limite formelle d'OWL, pas de RDF.** Les rôles d'une logique de description sont binaires, et OWL
ne sait pas exprimer « la conclusion tient si et seulement si **toutes** les prémisses tiennent ». Il
faudrait fermer la propriété, ce que l'hypothèse de monde ouvert refuse. Les couches de règles le
peuvent — Datalog avec prédicats n-aires natifs, SWRL en fragment DL-safe, SHACL Rules — au prix de
la fermeture du monde.

**Le monde ouvert et les résultats négatifs.** RDF stocke sans difficulté « nous avons tenté trois
fois et échoué », qui est un fait sur un processus. Ce qu'OWL ne fait pas, c'est traiter l'absence
d'entailment comme une négation. Le raisonneur ne peut donc **jamais arbitrer un résultat négatif**,
et les résultats négatifs restent dans le journal — où §18.7 leur donne déjà un agrégat complet, avec
sa puissance statistique et ses exclusions.

**La raison décisive : où vit la garantie.** Dans `packages/graph`, « on ne peut pas aplatir une
hyperarête » est appliqué par l'**absence d'API** et vérifié par un test qui existe précisément parce
que « c'est la fonction de commodité que quelqu'un finira par vouloir écrire ». Dans un magasin RDF
utilisant le motif n-aire, la même structure est une **convention** : une requête `?inf :hasPremise
?p` rend un sac plat d'arêtes binaires, légalement et sans erreur. La garantie passerait de
« impossible » à « tout le monde interroge correctement », c'est-à-dire au mode de défaillance contre
lequel le crate a été conçu.

Le motif n-aire a d'ailleurs un coût mesuré : le range de la relation cesse d'être le type du
participant pour devenir celui du conteneur. Dans un système dont l'intérêt est le typage, ce n'est
pas neutre.

---

## Décision 2 — Un raisonneur entre comme capacité admise, jamais comme dépendance

Un raisonneur d'ontologie entre par un `Published` de `packages/environments`, qui est la seule porte
selon `W18.d` — *vérifié* : `Published` est bien construit par `BuildState::published(signature)` au
terme de la chaîne `Locked → Built → Inventoried → Scanned → Tested → Published`, et il est exporté
depuis `packages/environments/src/lib.rs`. Aucune dépendance de build, aucune ligne dans `packages/`
ni dans `canterel`, et il s'abandonne en retirant une entrée de configuration.

**Motif.** C'est ce que `W18.d` a construit et qui n'a encore éprouvé aucun artefact réel. Un
raisonneur MCP est le candidat naturel : il exerce le chemin de gouvernance de bout en bout, et ce
chemin ne demande pas d'hôte `S3`/`S4` — c'est l'**exercice** contre un hôte réel qui attend, et il
attend déjà sous `W18.f`.

---

## Décision 3 — Une sortie de raisonneur est un claim proposé, et `Undetermined` refuse la confiance

Une classification, une subsomption ou une vérification de cohérence entre comme **claim proposé**,
avec sa provenance — quel raisonneur, quelle version d'ontologie, quel profil de raisonnement — et
soumis au pipeline de validation normal de §8.1. Jamais comme fait.

Un verdict à trois valeurs est exigé : cohérent, rejeté, **indéterminé**. Un échec à dériver une
contradiction n'est pas une cohérence.

**Motif.** C'est la discipline de `W4.b` — « une sonde non exécutée est un troisième verdict », qui
**refuse** la confiance parce que « c'est la preuve qui manque ». Un raisonneur qui rendrait
« cohérent » faute d'avoir trouvé une contradiction convertirait une limite de calcul en affirmation.
C'est aussi celle de l'`ADR 0020` : `matches` rend trois réponses, et `None` pour un algorithme non
calculé n'est pas un échec de vérification mais une absence de vérification.

---

## Décision 4 — Les règles proposent, elles ne décident pas

Un moteur de règles — SHACL Rules, Datalog, SWRL — peut produire des propositions. Il n'entre dans
aucun chemin de décision.

**Motif.** §20.2 exige que le moteur de politique soit **déterministe à entrées identiques**. La
spécification de SHACL 1.2 Rules, encore en Working Draft, reconnaît que la négation par l'échec
« pourrait conduire à des graphes inférés différents selon l'ordre dans lequel les règles sont
exécutées ». Incompatible avec une décision, acceptable pour une proposition.

---

## Décision 5 — Chaque couche d'ontologie a son régime, et le pont porte le couplage

| Couche | Ce qu'elle décrit | Régime |
|---|---|---|
| Domaine | ce que les choses sont | OWL/RDFS : subsomption, cohérence |
| Mémoire | genre, portée, substance | énumérations closes, aucune inférence |
| Épistémique | claim, evidence, objection, statut | `packages/graph`, jamais OWL |
| Provenance | agents, activités, dérivations | l'enveloppe d'événement §10.1 |
| Coordination | agents, équipes, relations, décisions | `packages/coordination` |
| Récupération | index, embeddings, régions | identités, aucune inférence |

**Le principe de pont.** La couche mémoire ne sait pas ce qu'est un `E22_Man-Made_Object` ni une
protéine kinase. Elle sait `Claim about Entity`. Les ontologies de domaine portent la sémantique de
domaine, et c'est ce qui permet à plusieurs référentiels de coexister sans aligner tout sur tout.

**L'objet de composition — quels modules d'ontologie, quels graphes nommés, quel profil de
raisonnement, quels canaux, quel budget — est le `Plan` de l'`ADR 0022` décision 4.** Une face regarde
les ontologies, l'autre les index ; c'est le même objet, il produit le même reçu, et l'escalade vers
un raisonneur y est déjà prévue sous `Escalation::Coprocessor { capability_id }`. Il n'en est pas
créé un second.

---

## Décision 6 — Un alignement est une proposition, jamais une inférence

Une équivalence entre deux ontologies — `owl:equivalentClass`, `skos:exactMatch`, `owl:sameAs` — ne
s'infère pas et ne s'écrit pas directement. Elle est une `Proposal` soumise à politique et
approbation, comme toute modification structurelle — donc, depuis l'`ADR 0021`, elle porte un **diff
d'opérations** qui se rejoue, et non une déclaration que rien n'applique.

**Motif, et il est empirique.** Le dépôt d'alignement examiné publie une ablation qui répond
directement à la question « qu'est-ce qui porte le résultat » : la **contrainte structurelle**, et
non la pondération des similarités. Retirer l'appariement un-à-un comme seule variable fait chuter le
F1 de 0,829 à 0,728, tandis que cinq configurations de pondération s'écartent de 0,0033. Le même
dépôt rapporte, sur la piste de référence, un rang de 9 sur 13 et un gain de +0,063 F1 sur une
baseline d'égalité de chaînes, avec un échec en **rappel** et non en précision.

Autrement dit : l'identité entre régimes descriptifs est structurelle, difficile, et non résolue par
la similarité. Un matcher propose ; il ne décide jamais que deux choses sont la même.

**Ce qu'il faut lire avant d'écrire `W14.e`.** §18 n'a pas été instruit pour cet ADR, et c'est la
section la plus susceptible de le contredire : une fusion de branches interagit avec la mémoire de
branche de §16.1 et avec les propositions d'alignement. §18.7 est déjà connu — il spécifie
`NegativeResult` en entier — mais §18.3 à §18.6 ne le sont pas.

---

## Décision 7 — Une capacité se résout par identité, pas par nom

Le registre des capacités résout par identifiant, et l'ordre de résolution est une propriété testée.

**Motif.** Un harnais tiers documente la raison, et elle est bonne : ses providers de mémoire sont
découverts dans l'ordre « la source la plus précoce l'emporte », l'inverse de son système de plugins
général, parce qu'un provider activé **par nom** qu'on masque « redirigerait silencieusement la
mémoire de l'agent au lieu de simplement remplacer un outil ». Une substitution de source de
connaissance ne produit pas d'erreur : elle produit des réponses plausibles fondées sur autre chose.

---

## Conditions, sans lesquelles la décision 2 est mauvaise

Le raisonneur candidat — un serveur MCP en Rust sous licence MIT adossé à Oxigraph, dont le
vérificateur de claims porte déjà le verdict à trois valeurs avec la bonne discipline — porte trois
réserves qui doivent être écrites avant de l'admettre, pas après :

1. son magasin est un `Mutex<Store>` unique, ce qui disqualifie l'usage « mémoire partagée » et **pas**
   l'usage « un agent pose une question de classification » ;
2. son registre n'a qu'un emplacement d'ontologie actif, suffisant pour cet usage et pour lui seul ;
3. ses résultats d'alignement sont publiés comme inférieurs aux baselines — ce qui est une raison de
   plus pour la décision 6, et non une objection à la décision 2.

---

## Conséquences

`packages/environments` gagne un type d'entrée de capacité de raisonnement ; `packages/adaptation`
gagne le chemin d'admission correspondant ; `packages/policy` gagne la catégorie d'alignement. Aucun
crate n'acquiert de dépendance RDF. `packages/graph` n'est pas touché.

## Plan de rollback

Entièrement additif. Une capacité de raisonnement retirée du registre laisse le dépôt dans son état
antérieur, et aucun type de domaine ne dépend d'elle. La décision 6 se retire en supprimant la
catégorie de politique.
