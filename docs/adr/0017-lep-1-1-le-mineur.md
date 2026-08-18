# ADR 0017 — `lep/1.1` : le mineur s'ouvre une fois, et aucun champ n'entre avant son lecteur

**Statut :** accepté. Ouvre le mineur `lep/1.1` que l'ADR 0016 annonçait comme ayant « son propre
ADR ». Débloque `W15.f` et `W16.d` de `docs/10`. Ne modifie **aucun** schéma : il décide ce que le
mineur a le droit de faire, dans quel ordre, et à quelle condition chaque champ entre. Ne change ni
`packages/protocol`, ni le SDK généré, ni aucune fixture.

**Contexte.** Quatre choses différentes attendent le même feu vert, et elles l'attendent depuis des
dates différentes.

L'ADR 0016 en a nommé deux, dans la section « Conséquences » : « la permission de fonctionnement hors
ligne, activable et désactivable, que la `MissionEnvelope` ne sait pas exprimer aujourd'hui, et les
codes de refus d'admission sur le fil ». Il a écrit que « ce mineur a son propre ADR ; W13 n'en dépend
pas et ne l'ouvre pas », et s'est arrêté là.

`W15.f` en a nommé une troisième en s'y cassant les dents. `SET_ROLE` demande que le rôle voyage
jusqu'au worker ; le lecteur est `selectOverlay` dans `canterel`, qui ne connaît d'une mission que ce
que la `MissionEnvelope` lui livre ; et l'enveloppe porte `review_policy` et `required_capabilities`,
**pas** de rôle. L'item est bloqué depuis, correctement.

`W16.d` en a nommé une quatrième : la visibilité institutionnelle facultative des sous-agents internes
du harnais — que `docs/10` désigne comme « le cas de W16 justifiant un mineur LEP, avec son ADR ».

Quatre besoins, un seul péage. Cet ADR décide de le payer une fois.

---

## Décision 1 — Le mineur s'ouvre, et il s'ouvre **une fois**

Les quatre ajouts sont arbitrés ensemble et portent un seul numéro de version : `lep/1.1`. Il n'y aura
pas de `1.2` pour le troisième d'entre eux.

**Motifs.** `CLAUDE.md` dit que « `packages/protocol` est le goulot du projet entier ». Le coût d'une
ouverture n'est pas la ligne ajoutée au schéma : c'est la régénération du SDK **dans les deux
langages** (`packages/lep/src/generated.ts` et `generated.rs`, tenus alignés par `check:generated`),
l'entrée de `registry.json`, les fixtures d'aller-retour, et la mise à jour du harnais de conformance.
Ce coût est presque entièrement **fixe** : il ne dépend pas du nombre de champs ajoutés. Quatre
ouvertures pour quatre champs, c'est payer quatre fois un péage qui se paie une fois.

Le motif inverse — arbitrer les quatre ensemble alors qu'ils n'ont pas la même maturité — est réel, et
c'est la décision 2 qui le traite.

---

## Décision 2 — Aucun champ n'entre dans un schéma avant son lecteur

Le **numéro** est décidé ici, une fois. Les **champs** entrent un par un, et chacun n'entre que le
jour où quelque chose d'exécutable et de testé le lit.

C'est la décision 4 de l'ADR 0016 — « aucune sémantique inerte » — appliquée au protocole plutôt qu'à
une énumération de relations. Le raisonnement y est déjà écrit et vaut mot pour mot : un champ que le
système saurait versionner, différencier, approuver et afficher, et que rien n'honorerait, est pire
qu'un champ absent. Absent, il se demande. Présent et inerte, il se croit tenu.

**Conséquence pratique, et elle n'est pas confortable :** cet ADR ne débloque pas les quatre items
d'un coup. Il débloque ceux dont le lecteur existe ou est écrivable dans le sprint qui ajoute le
champ. La décision 7 en donne l'ordre, et dit lesquels restent en attente de leur consommateur.

---

## Décision 3 — `1.1` ne crée pas de répertoire : la ligne `1.x` est **déjà** ouverte

Aucun `schemas/lep/1.1/`. Les fichiers de `schemas/lep/1.0/` valident déjà toute la ligne `1.x`, et
c'est `W0.5` qui l'a décidé — avant que quiconque ait besoin d'un mineur.

**Ce n'est pas une interprétation, c'est écrit et vérifiable :**

- `vocabulary.schema.json` définit `protocol_version` par le motif `^lep/1\.[0-9]+$`, avec en
  commentaire : « docs/06 fait du mineur un ajout de champs optionnels compatibles, donc un
  consommateur 1.0 doit accepter un document 1.1 et ignorer ce qu'il ne connaît pas. Un `const` ici
  transformerait chaque ajout mineur en rupture. »
- `schemas/README.md` : « Les documents restent ouverts. Aucun `additionalProperties: false` au niveau
  document. […] Fermer les documents transformerait chaque ajout mineur en rupture. »
- Et le fait : `grep -rl additionalProperties schemas/lep/1.0/` ne rend **rien**. Zéro fichier sur
  douze. La règle n'est pas seulement énoncée, elle est tenue.

**Motifs du refus de dupliquer.** Un `schemas/lep/1.1/` complet serait douze fichiers recopiés dont
huit ne changeraient pas d'une ligne, et chaque correction ultérieure devrait être appliquée deux
fois. C'est exactement la « duplication cross-repo des contrats » que `CLAUDE.md` interdit, appliquée
au protocole contre lui-même — et sa dérive serait silencieuse, puisque rien ne compare deux
répertoires.

Une variante par `allOf` — un `1.1` qui ne porterait que le delta et référencerait son ancêtre — évite
la duplication mais demande au générateur (`tooling/sdk/ir.ts`) de savoir fusionner des schémas
composés, ce qu'il ne sait pas faire. On paierait en machinerie de génération un problème que la
ligne ouverte n'a pas.

**Le nom du répertoire reste `1.0`.** Il nomme la version **fondatrice de la ligne**, pas une
exclusivité. Le renommer en `1.x` ne changerait rien de ce qu'un pair voit — les `$id` sont des URN,
indépendantes du chemin — tout en touchant chaque chemin de la chaîne d'outils. Le prix serait payé
pour un gain nul.

**Ce que le mineur écrit malgré tout dans les fichiers :** chaque propriété ajoutée porte
`"x-since": "1.1"`, et chaque feature nouvelle une entrée `since: "1.1"` dans `features.json` — champ
qui existe déjà et dont le commentaire dit à quoi il sert : « refuser proprement une feature venue
d'un mineur que le pair ne connaît pas ». Un lecteur doit pouvoir dire d'un champ quand il est apparu
sans lire un journal ailleurs.

---

## Décision 4 — Un mineur ajoute des **champs**, jamais des **valeurs**

Quatre interdits, dont le troisième est le seul qui ne va pas de soi.

1. **Aucun champ existant ne devient obligatoire.** Un `required` ajouté rend invalide un document
   `1.0` parfaitement légitime : c'est une rupture, quel que soit le numéro qu'on lui donne.
2. **Aucun champ existant ne change de type ni de sens sous la même URN.** C'est déjà la règle des
   identifiants de `schemas/README.md` — « un schéma publié ne change pas de sens sous le même
   identifiant » — rappelée ici parce qu'un mineur est précisément l'occasion de l'oublier.
3. **Aucun membre nouveau sur une énumération existante.** `docs/06` écrit « minor = champs optionnels
   compatibles ». Le mot est **champs**, et il se lit strictement : une valeur nouvelle sur une
   énumération ancienne est une rupture pour un consommateur `1.0` qui filtre exhaustivement — et le
   nôtre le fait. `packages/lep/src/generated.rs` émet des `enum` Rust **fermés, sans variante
   fourre-tout** : `SandboxLevel`, `NetworkMode`, `AcceleratorType`, `Os`, `Arch`, `DataClass`,
   `ContainmentResult`, `LimitResult`. Ajouter `"S6"` à `sandbox_level` ferait échouer la
   désérialisation chez tout consommateur `1.0`, en silence pour l'émetteur. Si un mineur a besoin
   d'une alternative sur une dimension existante, elle passe par un **champ nouveau**, pas par un
   membre de plus.
4. **Aucune feature n'est présumée.** Une capacité négociée absente du handshake est absente, jamais
   activée par défaut parce que « le pair est sûrement récent ».

L'interdit 3 contraint la forme des quatre ajouts, et la décision 5 montre qu'aucun n'en souffre.

---

## Décision 5 — Les quatre ajouts, sous leur nom

### 5.1 — `role`, dans la `MissionEnvelope` — pour `W15.f`

Un champ optionnel qui nomme le rôle de l'instance d'agent, au sens de `SPEC_V1.md` §20 (« `- role:
logical-reviewer` ») et §7.1, où `role` est un champ d'`AgentTemplate`. Champ nouveau : l'interdit 3
ne mord pas.

**Lecteur :** `selectOverlay`, dans `canterel/backend/cli/src/locus/agent-overlay.ts`. Il existe,
il est testé, et il choisit aujourd'hui par politique de revue puis par capacité.

**Contrainte que le sprint d'implémentation ne peut pas contourner :** le rôle ne prend jamais le pas
sur l'invariant 11. `selectOverlay` envoie déjà toute revue `independent` ou `independent-blind` vers
`reviewer`, « quelles que soient les capacités demandées : c'est l'invariant 11 qui décide, pas le
domaine scientifique ». Un `role` qui pourrait renvoyer une revue indépendante vers le profil du
générateur reconstruirait exactement le trou que ce test bouche. L'ordre est donc : politique de
revue, **puis** rôle, **puis** capacités.

### 5.2 — Les codes de refus d'admission, sur le fil

Aujourd'hui les six motifs de refus existent — `LevelUnavailable`, `CapacityExceeded`,
`AcceleratorUnavailable`, `NetworkModeUnsupported`, `LevelNotAttested`, `AcceleratorOutsideSandbox` —
et ils existent **en Rust seulement**, dans `apps/locus-execd/src/admission.rs`. Aucun document de
`schemas/lep/1.0/` ne les porte. Le fil sait dire « non » ; il ne sait pas dire pourquoi.

Forme : un **document nouveau**, pas un membre de plus sur une énumération existante — l'interdit 3
est respecté, et c'est ce que la forme demandait de toute façon, puisqu'un refus porte des données
(le niveau exigé, le meilleur niveau prouvé, le genre d'accélérateur) et pas seulement un code.

**Deux propriétés à ne pas perdre en traduisant,** parce qu'elles sont la raison d'être des six :

- **Jamais un seul motif à la fois.** `admit` accumule les raisons et rend `Refused { reasons }` au
  pluriel. Un fil qui ne transmettrait que la première ferait corriger une condition pour retomber
  aussitôt sur la suivante, autant de fois qu'il en manque.
- **`LevelNotAttested` n'est pas `LevelUnavailable`.** « L'hôte ne sait pas faire » et « l'hôte
  l'annonce sans l'avoir prouvé » envoient chercher deux choses différentes. Les fondre en un
  « niveau indisponible » ferait acheter du matériel pour un problème d'attestation. Même règle pour
  `AcceleratorOutsideSandbox` face à `AcceleratorUnavailable`, dont le code dit déjà pourquoi.

**Lecteurs :** les deux extrémités existent. `apps/locus-execd` produit, et la construction du refus
côté institution a déjà son vocabulaire.

### 5.3 — La permission de fonctionnement hors ligne, activable et désactivable

`SPEC_V1.md` §1.2, dernier invariant : « une installation locale complète doit rester utilisable hors
ligne, à l'exception des outils ou modèles explicitement distants. » Ce n'est pas un confort de
déploiement, c'est un invariant non négociable de la spécification, et il n'a aujourd'hui aucune
expression sur le fil.

L'enveloppe sait dire `network_mode: deny` ; elle ne
sait pas dire « cette mission a le **droit** de se passer de réseau » — ce qui n'est pas la même
chose, parce que le premier est une contrainte imposée au worker et le second une permission qui le
dispense d'échouer quand le réseau manque.

Champ nouveau, optionnel, distinct de `sandbox.network_mode` — les fondre ferait d'un confinement une
permission, et l'ADR 0004 sépare ces deux-là partout ailleurs.

**Lecteur : à écrire.** C'est celui des quatre dont le consommateur est le moins avancé. Par la
décision 2, il attend donc son sprint, et ce sprint commence par le lecteur.

### 5.4 — La visibilité des sous-agents internes du harnais — pour `W16.d`

**Facultative** est le mot de `docs/10`, et il se traduit en `feature` négociée au handshake, pas en
champ que tout worker devrait remplir. Un harnais qui ne subdivise pas n'a rien à déclarer, et
l'obliger à déclarer « aucun » ferait payer la fonctionnalité à ceux qui ne l'utilisent pas.

Entrée dans `features.json` avec `since: "1.1"`, et le champ correspondant marqué `x-since`.

**Ce que le sprint devra trancher, et que cet ADR laisse ouvert délibérément :** ce que
l'institution voit d'un sous-agent. Le voir « exister » et voir son contexte sont deux choses, et la
seconde traverse l'invariant 11 — un sous-agent reviewer interne au harnais ne doit pas devenir un
chemin par lequel le raisonnement privé du générateur remonte. Trancher cela ici, sans consommateur
sous les yeux, serait de la spéculation ; le sprint le tranchera avec son test de sortie.

---

## Décision 6 — Le mineur ne se constate pas, il se teste

Deux tests **définissent** ce que « mineur » veut dire ici. Ils sont le test de sortie de la première
tranche, et le second est le plus important des deux.

**Test 1 — un document `1.1` est accepté par un consommateur `1.0`, qui ignore ce qu'il ne connaît
pas.** C'est la moitié facile : les schémas sont ouverts et le motif de `protocol_version` couvre la
ligne. Le test existe pour que refermer un document devienne un échec bruyant plutôt qu'une décision
prise en passant.

**Test 2 — un document `1.0` reçu par un consommateur `1.1` laisse le champ nouveau ABSENT.**
Jamais rempli par un défaut. Un `role` qui vaudrait `research` faute de mieux rendrait « l'institution
n'a pas dit » indiscernable de « l'institution a dit `research` », et c'est le second qui se croit
tenu. C'est la règle que le dépôt applique déjà partout : `SandboxLevel::parse` rend `None` plutôt
qu'un niveau par défaut, « un niveau inconnu traité comme `S0` ouvrirait la sandbox, et traité comme
`S5` masquerait une configuration fausse en la rendant inoffensive. Les deux sont pires que l'aveu. »
Ici l'aveu s'appelle l'absence.

---

## Décision 7 — L'ordre des tranches, et ce qui reste en attente

Un numéro, quatre tranches, chacune avec son lecteur :

| Tranche | Ajout | Lecteur | État du lecteur |
| --- | --- | --- | --- |
| 1 | `role` (§5.1) | `selectOverlay` dans `canterel` | **existe et est testé** |
| 2 | codes de refus (§5.2) | `apps/locus-execd/src/admission.rs` | **existent, des deux côtés** |
| 3 | permission hors ligne (§5.3) | à écrire | à écrire dans son sprint |
| 4 | visibilité des sous-agents (§5.4) | à écrire, et à border sur l'invariant 11 | à écrire dans son sprint |

**Tranche 1 d'abord**, parce que son lecteur est le seul qui existe déjà en entier et que `W15.f` est
bloqué depuis le plus longtemps. Les deux tests de la décision 6 s'écrivent avec elle : ils ont besoin
d'un champ pour être écrits, et un seul suffit.

**Ce que cet ADR débloque donc réellement, tout de suite :** `W15.f` (tranche 1). `W16.d` cesse
d'attendre un ADR et attend désormais un consommateur, ce qui est un blocage d'une autre nature et se
lève par du travail plutôt que par une décision.

---

## Clause de falsification

Cet ADR affirme que le coût d'un mineur est **fixe** — que c'est le péage, pas le champ, qui coûte.
La tranche 2 est le test : elle ajoute un document entier là où la tranche 1 ajoute une propriété.

- Si la tranche 2 coûte, hors rédaction du document lui-même, à peu près ce qu'a coûté la tranche 1 —
  une régénération, une entrée de registre, des fixtures — l'affirmation tient, et la décision 1 était
  la bonne.
- Si elle coûte substantiellement plus **par nature** — si un document nouveau demande au générateur
  ou au harnais un travail qu'une propriété ne demande pas — alors le péage n'était pas fixe, et
  grouper quatre ajouts hétérogènes sous un numéro aura mélangé deux choses de coûts différents. Le
  constat est écrit au ledger, et la décision 1 est rouverte pour les mineurs suivants.

Dans un sens ou dans l'autre, le constat s'écrit. Une clause dont on ne consigne que l'issue favorable
ne falsifie rien.

---

## Conséquences

`schemas/lep/1.0/` gagne des propriétés marquées `x-since: "1.1"` et `features.json` des entrées
`since: "1.1"`, une tranche à la fois. Aucun fichier n'est déplacé, aucun `$id` ne change.

`packages/lep/src/generated.ts` et `generated.rs` sont régénérés à chaque tranche, et
`check:generated` garantit que les deux le sont ensemble.

`canterel` est touché à la tranche 1 — `selectOverlay` et son test — et la modification reste sous
`backend/cli/src/locus/**`, donc sans conflit de merge amont, conformément à l'ADR 0010.

`apps/locus-execd` est touché à la tranche 2 : les six motifs de refus se sérialisent au lieu de
rester internes.

**Un coût assumé :** le mot « 1.0 » dans un chemin de répertoire et dans une URN désigne désormais une
ligne, pas une version. C'est déjà ce que le motif de `protocol_version` dit depuis `W0.5`, mais un
lecteur qui ouvre `schemas/lep/1.0/` sans avoir lu cet ADR peut s'y tromper. Le commentaire `x-since`
sur chaque champ ajouté est ce qui le rattrape à l'endroit exact où la confusion se produirait.

**Ce que cet ADR n'ouvre pas :** aucun majeur, aucune rupture, aucun retrait de champ. `W16.e` —
epochs, messages tardifs, transfert d'état — n'est pas concerné : il attend une messagerie
inter-agents, pas un mineur de protocole.

---

## Plan de rollback

Cet ADR seul est documentaire. Avant la tranche 1, revenir coûte son annulation et le retour de
`W15.f` et `W16.d` à leur statut bloqué.

Après une tranche, revenir coûte le retrait de ses propriétés des schémas, une régénération et
l'annulation de son lecteur. Aucune donnée n'est en jeu : les champs sont optionnels, un document qui
en portait un reste valide sans lui, et un consommateur qui ne les lit plus les ignore — c'est
précisément la propriété que la ligne ouverte garantit, et elle joue dans les deux sens.

Le rollback qui coûterait cher n'est pas là. C'est celui de la décision 2 : ajouter un champ sans son
lecteur ne casserait aucun test, et le retirer plus tard demanderait de prouver que personne ne s'est
mis à l'écrire entre-temps. C'est le seul de cet ADR qui coûte une garantie plutôt qu'un diff, et
c'est pourquoi la décision est prise avant la première tranche plutôt qu'après.
