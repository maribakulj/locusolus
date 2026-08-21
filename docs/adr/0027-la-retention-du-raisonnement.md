# ADR 0027 — Le raisonnement est retenu ; ce qui est réglé est sa diffusion

**Statut :** accepté. N'amende aucune section de `SPEC_V1.md` — il en **relie** trois qui ne se
parlaient pas : §12.4, §16.1 et §16.6. Ouvre `W26`, et lève la moitié « décision » du blocage de
`W16.d`.

**Contexte.** La question a été posée par le propriétaire du produit, et elle porte : perdre le
raisonnement des agents est-il une erreur ? Peut-on le garder, n'en montrer que le résultat, le
cacher aux agents mais pas à l'humain, voire le rendre à des agents **selon une règle précise** — par
exemple lors d'un conflit prolongé ?

L'audit du terrain n'aide pas : les systèmes qui exhibent des populations d'agents ou bien
transmettent tout, ou bien ne gardent rien, et aucun ne distingue les deux gestes.

**Ce que le dépôt avait déjà, et ce qui manquait.** Trois pièces existent, écrites séparément et sans
lien :

- `memory::Level::AgentPrivate` — le niveau le plus étroit des sept de §16.1 — nomme une mémoire
  privée d'agent, et **aucun producteur ne l'alimente** ;
- `memory::Genre::MetaMemory` — dixième genre de l'ADR 0022 décision 1 — existe précisément pour que
  « l'utilité passée d'un document n'entre pas dans son score de vérité » ;
- `review::contamination::Contamination::GeneratorReasoningLeaked` — première des cinq formes de
  §16.6 — **détecte déjà** le partage du raisonnement du générateur avec un reviewer aveugle.

Autrement dit : le dépôt sait détecter la fuite, il a le rayonnage privé, il a le genre qui empêche
la contamination épistémique — et rien n'écrit le raisonnement nulle part. L'invariant 11 a été lu
comme un ordre de destruction alors qu'il énonce une borne de **lecture**.

---

## Décision 0 — Retenir et diffuser sont deux actes, et l'invariant 11 n'en gouverne qu'un

> L'invariant 11 dit : « Les reviewers indépendants ne reçoivent pas le raisonnement privé ou le
> contexte non autorisé du générateur. » Il gouverne un **ensemble de lecteurs**. Il n'a jamais dit
> que le raisonnement n'existe pas, ni qu'il faut le détruire.

Détruire est l'unique opération qu'aucun audit ne peut rattraper. Un dépôt dont la discipline est
qu'« une sonde non exécutée n'est pas un échec » et dont l'invariant 12 interdit d'effacer les
résultats négatifs ne peut pas, dans le même geste, jeter ce que ses agents ont pensé — et l'invariant
4 exige la provenance de tout résultat majeur, ce qu'un raisonnement absent rend incomplet.

**Corollaire, dans la forme de l'ADR 0025 :** ne pas garder une trace est une affirmation négative
sur l'état du système. Elle dit « cela n'a pas existé » là où la vérité est « nous avons choisi de ne
pas le regarder ».

---

## Décision 1 — Une trace de raisonnement est un artefact, et rien d'autre

La trace produite par une génération entre comme **artefact** au sens de §9.1 : déclarée avant
dépôt, hashée, portée par `packages/artifacts`, référencée par le journal via son condensat. Elle
est rangée en `Level::AgentPrivate` et en `Genre::MetaMemory`.

**Motifs.** Aucun mécanisme nouveau : c'est le chemin que `W2.14` a déjà construit pour tout contenu
volumineux, et celui que la décision 5 de l'ADR 0026 rappelle pour les messages. Un second chemin de
stockage serait le bus éphémère sous un autre nom.

**Aucun résumé n'est stocké à la place.** Faire condenser la trace par un modèle avant de l'écrire
produirait un document que personne ne peut confronter à ce qui a réellement été pensé, présenté
comme s'il l'était. Le résumé est une **lecture**, il se refait, il ne remplace pas.

---

## Décision 2 — Trois classes de lecteurs, et la lecture est elle-même un fait

| Lecteur | Ce qu'il obtient | À quelle condition |
|---|---|---|
| Le générateur | sa propre trace | sans condition |
| L'institution — un humain, par le cockpit de §20 | toute trace | sans condition d'autorisation, **mais la lecture est journalisée** |
| Un pair — un autre agent | une trace nommée | **seulement** par un `Disclosure` valide, décision 3 |

**Motifs.** C'est exactement le partage que la question demandait : l'humain voit, les agents non,
sauf règle. Ce qui s'y ajoute est la journalisation de la lecture institutionnelle. Elle ne
restreint personne ; elle empêche qu'un accès sans trace devienne le chemin par lequel un contexte
non autorisé remonte, et elle coûte une ligne de journal.

**Il n'y a pas de quatrième classe.** Un « lecteur système » ou un « outil d'analyse » qui lirait
sans être ni le générateur, ni l'institution, ni un pair autorisé serait la porte dérobée de ce
mécanisme. Une classe nouvelle demande un amendement de cet ADR.

---

## Décision 3 — Un dévoilement porte un motif nommé, une portée et une échéance

Un `Disclosure` n'est pas constructible sans les quatre :

1. **un motif**, pris dans une énumération close ;
2. **une portée** — quelle trace, vers quel lecteur, et rien de plus large ;
3. **une échéance** — au-delà, la trace n'est plus lisible par ce lecteur ;
4. **la journalisation** — le dévoilement est un fait, comme la lecture.

**L'énumération des motifs commence vide.** C'est la règle du dépôt : « une sorte de relation
n'entre dans son énumération que lorsqu'un consommateur exécutable et testé existe. » Chaque motif
arrive avec le mécanisme qui le déclenche, et `W26.c` livre le premier — **l'objection non résolue
après un nombre borné de tours de contestation**, c'est-à-dire le conflit prolongé que la question
nommait. `packages/review` et `coordination::objection` en portent déjà la matière.

**Un dévoilement ne s'accorde jamais par défaut, et jamais globalement.** « Toutes les traces de
cette branche » n'est pas une portée : c'est une politique de diffusion déguisée en autorisation
ponctuelle.

---

## Décision 4 — Ce qui est dévoilé n'est jamais une prémisse

Une trace dévoilée entre dans le contexte du lecteur en `Genre::MetaMemory`, et **aucun chemin ne
mène d'elle vers un `Support` ou une prémisse d'`Inference`**. Elle peut changer ce que quelqu'un va
**chercher** ; elle ne peut jamais changer ce qui est **tenu pour vrai** sans être réétabli par les
voies ordinaires.

**Motifs.** C'est la réponse exacte au risque de contamination, et c'est la troisième fois que ce
dépôt tient la même forme : `W18.h` fait entrer la sortie du raisonneur d'ontologie comme claim
*proposé* avec sa provenance et jamais comme fait ; `W24.c` fera influencer le **rang** par la
fiabilité observée et jamais la **validité** ; l'ADR 0022 décision 2 sépare `MetaMemory` pour que
l'utilité n'entre pas dans la vérité. Un raisonnement d'autrui est de l'utilité, pas de la preuve.

Tenu par l'absence, comme les trois autres : aucune signature ne permet la conversion.

---

## Décision 5 — L'aveuglement du reviewer ne se dévoile pas pendant sa revue, et se paie après

Aucun motif ne peut viser un reviewer indépendant **tant que sa revue est ouverte**. L'invariant 11
n'est pas un défaut qu'un motif surclasse : c'est une borne sur le mécanisme lui-même, tenue par la
construction — un `Disclosure` vers un tel lecteur n'existe pas.

Une fois le verdict **enregistré**, la revue est un fait figé que rien ne peut contaminer
rétroactivement. Un dévoilement devient alors admissible, et il produit un **second verdict** qui
porte le dévoilement dans sa provenance. Les deux sont conservés : l'invariant 12 interdit de faire
disparaître le premier, et l'écart entre les deux est précisément l'information que le conflit
prolongé cherchait.

**Motifs.** C'est le seul agencement qui rende la demande satisfiable sans affaiblir l'invariant.
Dévoiler pendant la revue le casse ; ne jamais dévoiler laisse le conflit sans issue autre que
l'autorité. Séparer les deux verdicts fait payer le dévoilement en **traçabilité** plutôt qu'en
crédibilité.

---

## Décision 6 — La détection de fuite doit distinguer un dévoilement d'une fuite, et le défaut reste « fuite »

`contamination::inspect` signale aujourd'hui tout raisonnement de générateur trouvé dans le contexte
d'un destinataire. Une fois `Disclosure` construit, il doit apprendre la différence — **et le sens du
défaut est décidé ici** : un élément de contexte qui ne porte pas de dévoilement valide reste une
`GeneratorReasoningLeaked`.

**Motifs.** L'inverse — présumer régulier ce qui n'est pas prouvé irrégulier — ferait de l'oubli
d'attacher le dévoilement un silence. Et la leçon de `W22.d` vaut dans l'autre sens : une garde qui
crie sur ce qui est juste se fait désactiver. Les deux exigences se tiennent ensemble et la seule
issue est que le dévoilement **voyage avec l'élément**, plutôt que d'être cherché ailleurs par la
garde.

---

## Décision 7 — `W16.d` cesse d'attendre une décision ; il attend un lecteur

`W16.d` — la visibilité institutionnelle facultative des sous-agents internes du harnais, tranche 4
du mineur `lep/1.1` — était bloqué `attend:externe`, sa ligne disant que « ce que l'institution voit
d'un sous-agent reste à trancher ». C'est tranché ici :

> L'institution voit **qu'un sous-agent a existé**, sa classe de cognition, son coût et son résultat.
> Elle ne voit son contexte et son raisonnement que par les décisions 1 à 5, comme pour n'importe
> quel agent.

Ce que la ligne disait déjà — « voir qu'un sous-agent existe et voir son contexte sont deux choses »
— devient la décision au lieu d'être la question. Il reste à `W16.d` un lecteur, que `W26.b` fournit :
son blocage devient `attend:W26.b`, vérifiable et périssable, au lieu d'`attend:externe`, qui ne se
périme jamais.

---

## Décision 8 — Ce qui n'est pas construit, et pourquoi

**Aucune variante « expurgée » d'une trace.** Une rédaction que personne ne peut vérifier est une
promesse au sens de l'ADR 0022 décision 0. `ContextView::Redaction` existe et opère sur des révisions
nommées ; il n'y a rien à généraliser à du texte libre.

**Aucune durée de rétention en V1.** Effacer est l'acte irréversible, et rien ne l'exige encore.
Quand une politique de rétention deviendra nécessaire, elle sera une **valeur de politique**,
versionnée et visible — jamais une constante de code, exactement comme les seuils que l'ADR 0024
décision 9 a tenus hors du Rust.

**Aucun score de qualité du raisonnement.** Noter une trace la ferait entrer par la porte de la
décision 4 : un rang calculé sur un raisonnement est une utilité qui finit par se lire comme une
vérité.

---

## Conséquences

`W26` s'ouvre avec quatre items et n'est bloqué par rien : la matière est déjà là — artefacts,
niveaux de mémoire, genres, détection de contamination, revue. `W16.d` change de raison de blocage et
devient périssable. §12.4, §16.1 et §16.6 sont désormais reliés par un mécanisme nommé plutôt que par
une intention.

Et une chose cesse d'être vraie : le dépôt ne jette plus ce que ses agents ont pensé.

## Plan de rollback

Les décisions 0, 2, 7 et 8 sont documentaires et se retirent par un diff. La décision 1 n'ajoute
aucun stockage — elle range dans un chemin existant — et se retire en cessant d'écrire. Les décisions
3, 4, 5 et 6 introduisent un type et une borne sur une garde existante : les retirer rend
`contamination::inspect` à son comportement actuel, qui est le plus strict des deux, donc le rollback
ne peut pas ouvrir de fuite. `W16.d` reprendrait son `attend:externe`.
