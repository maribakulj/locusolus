# ADR 0037 — Ce qu'un mineur peut ajouter à une énumération, et ce qui le garantit

**Statut :** accepté. Tranche l'arbitrage que `W19.c` porte depuis `W12.d.4`. **Précise** l'interdit 3
de l'ADR 0017 décision 4 sans le lever, et corrige une description de schéma qui le contredisait.

**Contexte.** `runLoop` réclame une mission, l'admission dit non, et la boucle **rend la main sans
rien dire au plan de contrôle**. La mission reste sous bail jusqu'à expiration, et « le worker a
refusé » se confond avec « le worker est mort ». C'est la paire de silences que ce dépôt refuse
partout ailleurs, et `W12.d.4` l'a trouvée en exécutant la chaîne, pas en la lisant.

Trois voies existaient, et deux ont déjà été refusées avec leurs raisons : réutiliser
`attempt.failed` — non, `rejected` et `failed` n'envoient pas au même endroit, l'admission ayant dit
non **avant** toute exécution ; réutiliser `AdmissionRefusal` — non, sa liste `reason` porte les
motifs du **broker** et aucun code propre au worker. Reste un événement `task.refused`, et il bute
sur l'interdit 3 : « aucun membre nouveau sur une énumération existante ».

---

## Ce qui a été établi, et comment

Tout ce qui suit vient de la lecture du code et des schémas, pas de la documentation.

| Fait | Où il se vérifie |
| --- | --- |
| La boucle refuse et **n'émet rien** | `canterel/backend/cli/src/locus/worker-loop.ts`, la branche `if (!mapped.ok)` |
| Le port pour émettre existe **déjà**, et la boucle s'en sert deux fois | même fichier, `ports.emit` pour `attempt.started` puis `attempt.completed` |
| `event_type` est une énumération **inline**, écrite sur la propriété | `schemas/lep/1.0/event.schema.json` |
| Le générateur rend une énumération inline en **`String`** côté Rust | `tooling/sdk/emit-rust.ts` : « An inline enum has no name of its own ; the wire value is what matters » |
| Et en union de littéraux **fermée** côté TypeScript | `packages/lep/src/generated.ts`, le champ `event_type` de `Event` |
| Les huit énumérations que l'interdit 3 **nomme** sont toutes des `definitions` | `sandbox_level`, `network_mode`, `accelerator_type`, `os`, `arch`, `data_class` dans `vocabulary.schema.json` ; `containment_result` et `limit_result` dans `sandbox-attestation.schema.json` |
| Elles sont donc rendues en `enum` Rust **fermés** | `packages/lep/src/generated.rs` |
| La description de `event_type` **autorise** ce que l'interdit 3 refuse | « Un nouveau type est un ajout mineur qui met à jour cette liste » |

**Le quatrième fait est celui qui oblige à rouvrir le raisonnement.** L'interdit 3 justifie son
refus ainsi : « ajouter `"S6"` à `sandbox_level` ferait échouer la désérialisation chez tout
consommateur `1.0`, en silence pour l'émetteur ». C'est **exact** pour les huit qu'il nomme, et
**faux** pour `event_type`, que le générateur rend en `String`. Un `task.refused` qui arriverait chez
un consommateur Rust `1.0` ne casserait aucune désérialisation.

**Le huitième est celui qui rend la contradiction opposable.** Deux textes de ce dépôt disent le
contraire l'un de l'autre sur le même champ, et l'un des deux est un schéma — c'est-à-dire le
contrat.

---

## Décision 1 — L'interdit 3 reste, et sa vraie raison n'est pas celle qu'il donne

La raison qu'il donne — la désérialisation qui échoue — est un **mécanisme**, et un mécanisme se
mesure. Mesuré, il ne couvre pas `event_type`. Si c'était la seule raison, l'interdit ne s'y
appliquerait pas et il n'y aurait rien à trancher.

Ce n'est pas la seule raison, et la meilleure est écrite dans le schéma lui-même : « un type
d'événement inconnu n'est pas un champ qu'on peut ignorer : le consommateur ne saura ni quoi en faire
**ni s'il vient de rater quelque chose** ». C'est cela que l'interdit protège. Un consommateur qui
reçoit `task.refused` sans le connaître ne casse pas ; il **ne sait pas qu'il a manqué un refus**, et
la mission qu'il croit en cours ne l'est plus. La panne muette est pire que la panne bruyante, ce que
l'ADR 0036 a déjà établi à propos des bornes de ressources.

L'interdit 3 est donc maintenu **et** requalifié : ce qu'il interdit est de faire arriver chez un pair
une valeur dont il ne peut pas savoir qu'il l'a manquée.

---

## Décision 2 — Un membre entre dans une énumération fermée si et seulement si son émission est gardée par une feature négociée du même mineur

La requalification donne la condition. Une valeur ne peut pas être manquée par un pair qui ne la
reçoit jamais ; un pair qui ne l'a pas négociée ne la reçoit jamais ; donc un membre dont l'émission
est **gardée** ne peut pas produire la faute que l'interdit 3 protège.

Ce qui rend la garde solide plutôt que pieuse est l'interdit **4** du même ADR : « aucune feature
n'est présumée. Une capacité négociée absente du handshake est absente, jamais activée par défaut
parce que le pair est sûrement récent. » Sans lui, la garde reposerait sur l'optimisme de chaque
émetteur ; avec lui, elle repose sur une règle que le dépôt tient déjà et qui a son propre code —
`negotiate` rend `features`, `declined` et `unknown`, et un pair qui n'annonce rien obtient une liste
vide.

La feature entre dans `features.json` avec le `since` du mineur qui l'introduit, comme
`subagent-visibility` l'a fait pour `1.1`. Une valeur gardée par une feature d'un mineur **antérieur**
serait un contournement : elle atteindrait un pair qui a négocié cette feature-là sans connaître la
valeur.

---

## Décision 3 — La garde est une propriété de l'émetteur, et elle se teste

Une règle qui ne vit que dans une prose d'ADR est une promesse. L'item qui ajoute un membre gardé
livre donc **un test qui exerce le pair sans la feature** et constate que la valeur n'est pas émise —
pas seulement le test qui constate qu'elle l'est quand la feature est accordée.

Les deux sens, comme partout ici : une garde qui ne dirait que « émis quand accordée » passerait
aussi sur un émetteur qui émet toujours.

---

## Décision 4 — La description de `event_type` est corrigée, parce qu'elle est fausse

Elle dit aujourd'hui : « Un nouveau type est un ajout mineur qui met à jour cette liste. » Sans
garde, c'est exactement ce que l'interdit 3 refuse, et le schéma étant le contrat, c'est lui qui a
tort. Elle porte désormais la règle de la décision 2.

C'est une correction de **description**, pas de forme : aucune valeur n'est retirée, aucun champ ne
change de type, et aucun document valide ne cesse de l'être.

---

## Ce que cet ADR ne permet pas

- **Rendre un champ obligatoire.** L'interdit 1 est intact ; une valeur gardée ne rend rien exigible.
- **Dégarder après coup.** Un membre entré sous une feature n'en sort pas sans un mineur nouveau :
  le dégarder ferait arriver la valeur chez un pair qui ne l'a pas demandée, ce qui est le cas
  d'origine.
- **Ajouter une valeur sans consommateur.** L'ADR 0022 décision 0 s'applique entière : une valeur
  d'énumération affirme qu'un effet existe, et « aucun appelant ne l'utilise encore » ne l'excuse
  pas. `task.refused` n'est acquis qu'avec l'émetteur qui le produit **et** le lecteur qui en tire
  une conséquence.

  Les deux ne peuvent pas tenir dans un même commit, et il faut le dire plutôt que de l'écrire comme
  une règle qu'on enfreindra à la ligne suivante : le protocole et le lecteur vivent ici, l'émetteur
  vit dans `canterel`, et l'ADR 0033 impose que le worker relise le contrat **avant** que le pin
  avance. L'ordre est donc protocole et lecteur, puis émetteur, puis pin. Ce que la règle interdit
  est de **déclarer l'item fait** entre les deux — pas de le livrer en deux temps.
- **Étendre la règle aux documents.** Ils restent ouverts, et aucun ne porte
  `additionalProperties: false` — ADR 0017 décision 3, vérifiée par le fait.

## Ce que cet ADR ne décide pas

- **Le nom de la feature, ni la forme du `payload` de `task.refused`.** Ce sont des choix d'item, pas
  d'architecture, et les figer ici les rendrait faux avant d'être écrits.
- **Ce que l'institution fait d'un refus reçu.** Libérer le bail, remettre en file, compter le refus
  contre le worker : la conséquence appartient à l'item, et elle est ce qui distingue une capacité
  d'une promesse.
- **Si les autres énumérations inline méritent le même traitement.** Aucune ne le demande
  aujourd'hui ; le décider d'avance serait généraliser depuis un seul cas.

## Ce que cet ADR débloque et ce qu'il ferme

Il ferme l'arbitrage de `W19.c`, qui attendait « une décision de protocole » depuis `W12.d.4`, et il
la ferme en **précisant** un interdit plutôt qu'en le levant — la distinction compte, parce qu'un
interdit levé se relève ailleurs.

Il laisse à `W19.c` du travail sans arbitrage : la feature, le membre, la forme du refus sur le fil,
l'émission gardée côté worker, la lecture côté institution, et les deux tests de la décision 3.
