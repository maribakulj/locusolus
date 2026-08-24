# ADR 0032 — Demander un placement, et ce qu'une annonce ne devient jamais

**Statut :** accepté. Ouvre `W20.q`.

**Contexte.** `W4.g` a livré `place` — choisir un hôte parmi des candidats sur ce qu'ils ont
**prouvé** — et il n'a jamais eu d'appelant. `W20.k` a livré la file de missions, dont
`MemoryQueue::take` reçoit le `worker_id` et **l'ignore délibérément** : sa propre documentation
l'écrit. La clause de `W12.d` « un worker s'enregistre et est placé sur ce qu'il a **prouvé** »
n'avait donc aucun sujet — la file servait la première offre à qui demandait, quel que soit son
manifeste, et un worker macOS pouvait recevoir une mission `S3`.

`W4.h` avait déjà posé le tube entre `locusd` et `locus-execd`, et son ADR (0028, décision 5)
annonçait explicitement la suite : « l'admission puis le cycle de vie des sandboxes s'ajouteront
comme des variantes de requête sur un tube qui marche ». C'est ce que cet item fait, et rien du
transport ne change.

---

## Décision 1 — Le manifeste voyage sur la **réclamation**, pas sur l'enrôlement

§15.3 fait annoncer le `CapabilityManifest` au handshake. Trois raisons font que ce n'est pas là
qu'il entre ici.

1. **Il n'y a pas de handshake côté serveur.** Le hello de `W2.7` porte le *hash* du manifeste, pas
   le manifeste, et `locusd` ne sert aucune route de handshake. Le faire entrer par l'enrôlement
   aurait demandé de changer la demande signée de `W2.4` — donc la charge signée, donc les deux
   moitiés de §7.2 — pour un document qui n'a rien à voir avec l'identité.
2. **Un inventaire vieillit.** Un disque se remplit, un accélérateur disparaît ; `capability-watch`
   (`W2.6`) existe parce que cela arrive. Un manifeste figé à l'enrôlement ferait placer une mission
   sur de l'espace disque qui n'existe plus, et le refus arriverait à l'exécution — c'est-à-dire
   après avoir commencé à construire une sandbox, ce que `HostCapabilities` dit explicitement qu'un
   broker ne doit pas faire.
3. **Le coût est d'un champ.** `canterel` tient déjà `ports.manifest()` dans le client de `W2.21`.

**Sans manifeste, la réclamation est refusée** — `validation`, en nommant le champ — et **même sur
une file vide**. Un worker mal configuré qui recevrait `204` lirait sa panne comme du calme et
attendrait indéfiniment ; le refus arrive donc au premier appel.

Le manifeste **ne dit pas qui parle**. La créance le dit, et un manifeste au nom d'un autre worker
est refusé plutôt qu'ignoré : le laisser passer ferait placer sur les capacités d'une machine et
exécuter sur une autre — un downgrade au sens de §21.6, obtenu sans jamais toucher au niveau demandé.

---

## Décision 2 — Une annonce n'est pas une preuve, et le port qui les sépare a un défaut qui refuse

`CapabilityManifestSandbox.attestation` vaut « ce worker **sait produire** une `SandboxAttestation` »
— c'est ce que le schéma de `W0.6` en dit. Le lire comme un [`Standing::Trusted`] serait exactement
la faute que `placement.rs` existe pour refuser : « la confiance ne se déclare pas, elle se prouve ».
Elle serait de surcroît **invisible** — le placement marcherait, sur un hôte dont personne n'a rien
vérifié.

Ce qu'un worker a prouvé vient donc d'un port, `Proven`, dont le défaut `NothingProven` ne connaît
personne. Conséquence assumée : **un broker sans campagne de self-tests ne place rien au-dessus de
`S0`**, et le refus le dit sous le nom `level_not_attested`, distinct de `level_unavailable`. C'est
exact, et c'est ce qui rend visible au premier placement réel qu'il manque la campagne. Ce qui
remplira ce port est nommé — `W12.e`, l'attestation — et non simulé.

---

## Décision 3 — Le vocabulaire de refus est celui de §10.2, pas un second

Un manque de placement s'écrit `locus_lep::Reason`, les sept motifs que `wire.rs` produit déjà pour
un `AdmissionRefusal`. Une seconde écriture aurait divergé au premier motif ajouté — et il s'en est
ajouté un : `disk_quota_not_enforceable`, né avec `W5.g` après l'ADR 0017 qui en nommait six.

Le refus porte **tous** ses motifs, comme `admit` le fait déjà : n'en transmettre qu'un ferait
corriger une condition, relancer, découvrir la suivante.

---

## Décision 4 — Trois grandeurs que §15.3 n'annonce pas ne décident de rien, et un test le tient

Le manifeste n'inventorie ni quota **PID**, ni **horizon** de temps, ni applicabilité d'un **quota
disque**. Les inventer donnerait à un refus une cause que personne n'a déclarée.

- Le PID est neutralisé par la même valeur des deux côtés — la demande tient toujours dans l'offre.
- L'horizon n'est pas comparé par `quotas_fit_within`, et un test épingle cette indistinction : le
  jour où il pèsera, il rougira et dira à qui l'a fait peser que §15.3 ne l'annonce pas.
- L'applicabilité d'un quota disque est un fait **lu sur la machine** (`/proc/mounts`, `W5.g`), pas
  une chose qu'on déclare. Ne pas refuser sur ce motif est donc exact ; la poser à `NotEnforceable`
  refuserait tous les workers pour une raison qu'on n'a pas constatée.

Même règle pour `AcceleratorReach::NativeOnly` : elle porte un `native_level` que §15.3 n'a pas, et
le déduire de la plateforme (« macOS, donc MPS est natif, donc `S1` ») écrirait une politique de
sécurité dans une traduction. La variante garde son consommateur — les capacités **lues** sur l'hôte
local la posent.

Le **profil** d'une mission suit la même logique par l'autre bout : §21.6 dit qu'il « nomme une
intention ; c'est `minimum_level` qui engage », et une mission qui n'en nomme aucun s'en voit prêter
un. Le prêt va au plus confiné, pour une seule raison — si un profil se met un jour à engager quelque
chose, mieux vaut avoir été trop sévère que pas assez —, et un test vérifie que les **sept** rendent
le même verdict. Un profil que §21.6 ne nomme pas est refusé plutôt qu'ignoré : un nom que personne
n'a défini serait quand même écrit dans le journal de ce qui a été appliqué.

---

## Décision 5 — Une mission qu'on ne confie pas **retourne dans la file**

`MissionQueue::take` retire. Si le placement échoue ensuite, la mission serait perdue au profit de
personne : un worker macOS qui sonde une file portant une mission `S3` la ferait disparaître, et le
worker Linux qui pouvait la porter ne la verrait jamais. Aucun journal ne montrerait cette perte —
il ne se passe rien.

Elle est donc remise dans **tous** les cas où elle n'est pas confiée : refus de placement, broker
injoignable, broker qui refuse de parler. Une seule offre est examinée par réclamation ; parcourir la
file jusqu'à en trouver une qui convienne serait de l'ordonnancement, donc `W23.c`, et l'écrire ici
le rendrait invisible.

---

## Décision 6 — « Je n'ai pas pu demander » n'est pas « rien pour toi », jusque dans le code de statut

ADR 0028 décision 4, tenue sur la seconde question du tube :

| Ce qui s'est passé | Ce que `/lep/v1/claim` rend |
| --- | --- |
| Le broker place | `200` avec l'offre, et `task.leased` au journal |
| Le broker dit non | `204` — il y a du travail, mais pas pour cet hôte |
| Le broker est injoignable, illisible, ou refuse | `503` — la famille `unavailable` de §22.5 |

Un worker qui recevrait `204` sur un lien coupé attendrait en silence un ordonnanceur qui, lui, avait
du travail, et personne ne saurait pourquoi rien n'avance. Le client de `W2.21` tient déjà cette
séparation de son côté — un `204` y devient un tour `idle`, un `503` y lève — et il n'a rien eu à
changer pour cela ; c'est le point.

---

## Décision 7 — Cinq verdicts dans une énumération, et un hors-sujet se dit

Il n'y a qu'une `Response` et qu'un `answer`, donc les verdicts des deux questions vivent dans le
même type. Ils ne répondent pas à la même question, et ce n'est **pas** au lecteur de s'en arranger :
`BrokerPort::place` refuse un verdict de disponibilité en `Malformed`, et `Standing::probe` refuse
symétriquement un verdict de placement. Les deux le disent plutôt que de l'interpréter — c'est la
règle d'`answer` pour un désaccord de version, appliquée à un désaccord de question. Sans cela, la
fusion se paierait le jour où deux binaires de versions différentes se parlent, et se paierait en
placements silencieusement faux.

---

## Ce que cet item ne fait pas

Il ne conserve aucune attestation (`W12.e`), n'ordonnance rien (`W23.c`), et ne fait pas respecter
l'expiration d'une créance à la réclamation — `Credential.expires_at` vaut toujours `null`, comme
l'ADR 0031 décision 5 l'a nommé. Aucune de ces trois absences n'est comblée par un défaut permissif :
chacune produit un refus qui la nomme.
