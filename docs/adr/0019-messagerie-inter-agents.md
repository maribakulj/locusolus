# ADR 0019 — La messagerie inter-agents comme usage du journal

**Statut :** accepté. Débloque `W16.e`. Amende la liste de `EVENT_NAMESPACES` (§10.3) d'un nom, et
la clause de gel de `docs/13` sur « epochs et transfert d'état ».

**Contexte.** `docs/13` gèle `W16.e` avec une raison technique, pas administrative : « epochs et
transfert d'état — aucune messagerie inter-agents n'existe ; **l'unité de concurrence est l'attempt**,
déjà versionné, leasé, idempotent et acquitté par séquence ». La position tenait : en V1 les agents ne
se parlent pas, ils se coordonnent par le journal et les leases, et une messagerie construite pour
faire passer un test aurait été une fonctionnalité inventée pour justifier une vérification.

**Le propriétaire du produit a énoncé que le besoin est réel.** La condition du gel disparaît donc, et
la question n'est plus *s'il faut* une messagerie mais **quelle forme** elle prend. Cet ADR ne
tranche que cela.

**La faute à éviter, nommée avant de décider.** Un courtier de messages est un stockage durable. Le
journal en est un aussi. `W20.f` a rencontré exactement ce choix pour le fil client et a conclu :
« deux stockages durables du même fait sont **deux vérités**, qui divergent le jour où l'une est
purgée ». Le worker a un spool parce qu'il **produit** des faits que rien d'autre ne détient ; un
agent qui parle à un autre agent, à l'intérieur de l'institution, n'est pas dans ce cas. L'invariant 2
le dit d'ailleurs déjà : « PostgreSQL/event store et graphe Locus sont la vérité institutionnelle,
**pas les transcripts** ». Un message échangé hors du journal est un transcript.

**Décision.**

1. **Un message est un événement.** La messagerie inter-agents n'est pas un transport parallèle :
   c'est un **usage** du journal existant. Émettre, c'est écrire un fait ; recevoir, c'est lire par
   cursor — le mécanisme que `W20.e` et `W20.f` ont déjà livré et éprouvé.
2. **Un `epoch` est une révision de configuration**, pas un compteur neuf. Ce qui change la
   configuration d'un ensemble d'agents produit déjà une `Version` (ADR 0016) ; c'est elle l'epoch.
   Un message porte l'epoch **sous lequel son émetteur a agi**.
3. **Un message tardif est rapporté, jamais appliqué en silence ni jeté en silence.** Deny-by-default :
   à epoch inconnu ou antérieur, le destinataire ne devine pas.
4. **Le namespace `message` entre dans `EVENT_NAMESPACES`**, signalé comme ajout local au même titre
   que `projection` et `migration` — §10.3 ne le liste pas, et le fondre sans le dire ferait passer un
   ajout pour une lecture de la spec.

**Motifs.**

Le journal donne gratuitement ce qu'une messagerie doit péniblement garantir : l'ordre total par
stream (§10.2), la durabilité, l'idempotence par commande, et la reprise par cursor sans trou ni
doublon — propriété que `W20.e` a vérifiée mutant par mutant. Un courtier devrait les réimplémenter,
et les réimplémenter moins bien, puisqu'elles seraient testées une seconde fois plutôt qu'une seule.

L'epoch comme `Version` évite un second vocabulaire de version. `CLAUDE.md` interdit le vocabulaire
parallèle, et « epoch » et « version de configuration » désigneraient sinon la même chose sous deux
noms — dont l'un dériverait de l'autre au premier oubli.

La visibilité institutionnelle vient en prime, et ce n'est pas un effet secondaire : un message qui
est un événement est **auditable**, versionné, et rejouable. Un message qui passe par un socket est
invisible à l'institution, donc absent du dossier quand on demandera qui a dit quoi à qui.

**Conditions, sans lesquelles la décision est mauvaise.**

1. **Aucun second stockage durable.** Pas de file, pas de courtier, pas de table de messages à part.
   Si un besoin de latence l'exige un jour, la réponse est un **cache** de ce que le journal contient
   déjà — pas une source concurrente. Un test d'absence tient la règle, comme `W20.b` le fait pour les
   écritures.
2. **Un message tardif ne se jette pas plus qu'il ne s'applique.** Les deux fautes sont symétriques et
   la seconde est la plus discrète : un message silencieusement ignoré rend un système qui « marche »
   et qui a perdu une information. Le destinataire rend un verdict — `Delivered`, `Late`, `Unknown` —
   et ce verdict est lisible.
3. **La messagerie n'est pas une porte dérobée pour la migration d'état.** La règle V1 reste celle de
   `docs/13` : « nouvel attempt, nouvelle vue, nouveau hash ». Le `content_hash` de `ContextView` est
   obligatoire et l'enveloppe immuable ; un message qui transporterait un contexte de mission
   contournerait cette immuabilité sans la nommer. Ce que `W16.e` appelle « transfert d'état » est le
   **passage de témoin** d'un `drain` — ce que le nœud sortant tenait et que le nœud entrant doit
   savoir — et non une copie de contexte.
4. **Le domaine ne devient pas asynchrone.** Comme en ADR 0018 condition 3 : écrire et lire un message
   sont des opérations synchrones sur le journal. Le bord asynchrone reste le bord.

**Conséquences.**

`W16.e` est débloqué et reçoit son test de sortie. Le namespace `message` entre dans la liste, avec le
commentaire qui le signale comme ajout local ; le test qui vérifie la liste devra être mis à jour, et
c'est voulu — une liste normative ne s'allonge pas sans que quelqu'un le lise.

`docs/13` voit sa clause de gel amendée sur un point et un seul : « aucune messagerie inter-agents
n'existe » devient « la messagerie inter-agents est un usage du journal, ADR 0019 ». Le reste de la
clause — l'attempt comme unité de concurrence — **reste vrai** : un message ne remplace ni un lease ni
un attempt, il les accompagne.

**Alternative écartée : un courtier dédié.** Elle donnerait la latence et le découplage qu'un système
distribué ordinaire recherche. Écartée pour la raison de `W20.f` : deux stockages durables du même
fait sont deux vérités. Et l'argument de latence ne porte pas ici — les agents de ce système
coordonnent des missions qui durent des minutes, pas des paquets qui durent des microsecondes.

**Alternative écartée : un canal direct entre agents.** La plus simple à écrire, et celle qui détruit
le plus. Un message qui ne passe pas par le journal est un transcript, et l'invariant 2 dit que les
transcripts ne sont pas la vérité institutionnelle. Le jour où l'on demandera pourquoi deux agents ont
divergé, la réponse serait dans un tampon que personne n'a gardé.

**Condition de réexamen.** Si des agents **persistants** apparaissent — `docs/13` le prévoit
explicitement pour la migration d'état — la condition 3 devra être rouverte, parce qu'un agent qui
survit à sa mission a un état qui ne se re-matérialise pas. Cet ADR ne préjuge pas de ce
qu'on décidera alors ; il note que c'est là que la question reviendra.

**Rollback.** Additif. La messagerie est un module et un namespace ; l'annuler coûte la suppression du
module, le retrait d'une entrée de `EVENT_NAMESPACES` et la remise de `W16.e` à l'état reporté. Aucun
événement déjà écrit ne devient illisible — il resterait un fait sous un namespace retiré de la liste,
ce que le journal supporte par construction, l'immuabilité logique de §10.2 interdisant de le réécrire
de toute façon.
