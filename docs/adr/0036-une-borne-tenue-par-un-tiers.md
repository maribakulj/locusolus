# ADR 0036 — Une borne tenue par un tiers n'est pas tenue par le mécanisme attesté

**Statut :** accepté. Tranche la question que `W5.af.3` a fait apparaître par la mesure, et que
`W5.ai` porte.

**Contexte.** La campagne de `W5.af.3` exerce les seize sondes contre le vrai `bwrap`, et rend :

```
NotTrusted { level: S2, blocking: [exceed_cpu_quota, exceed_memory_quota, exceed_pid_quota] }
```

Les trois sondes sont `NotRun` — « ce qu'elle devait lire n'était pas là » — et ce sont exactement
les trois contrôleurs que `bubblewrap::unenforced` déclarait d'avance : `cpu.max`, `memory.max`,
`pids.max`. Deux choses écrites séparément disent la même chose, et un test l'exige par égalité
d'ensembles.

Ce n'est pas un défaut à réparer dans le mécanisme. `bubblewrap` compose des namespaces et des
montages ; la comptabilité des ressources appartient à qui l'appelle. Un `Proven` pour un worker sous
ce mécanisme demande donc que ces bornes soient posées **autour** de `bwrap` — et c'est cette
possibilité-là qui pose une question à l'ADR 0035.

---

## Ce qui a été établi, et comment

Tout ce qui suit vient de mesures, pas de lectures de documentation.

| Fait | Où il se vérifie |
| --- | --- |
| `bubblewrap` n'écrit aucun cgroup : ni mémoire, ni CPU, ni PID | `W5.af.1`, et la campagne de `W5.af.3` le confirme indépendamment |
| Les trois sondes de quota rendent `NotRun`, pas `Blocked` — elles n'ont **rien lu** | table de `W5.af.3` |
| L'ensemble bloquant de la campagne **égale** l'ensemble déclaré par `unenforced` | test d'égalité d'ensembles, `W5.af.3` |
| `HostFacts` lit déjà les contrôleurs **délégués au cgroup de ce processus**, et non ceux de la racine | `apps/locus-execd/src/linux/probe.rs`, `fn cgroup` |
| Sur le runner de CI : `cgroup_v2=available controllers=cpu,cpuset,io,memory,pids` | attestation déposée par le job `sandbox` |
| Sur le conteneur de développement : `cgroup_v2` **indisponible**, contrôleurs `{}` | `HostFacts::read_host()`, exécuté ; hiérarchies v1, unifiée à `/sys/fs/cgroup/unified` ne portant que `hugetlb` |

**Le cinquième fait est celui qui rend la question réelle** : la capacité de poser un cgroup autour
d'un enfant existe là où la campagne tourne. Ce qui manque n'est pas l'hôte.

**Le sixième est celui qui interdit d'en conclure trop vite** : elle n'existe pas partout, et un
mécanisme qui la supposerait échouerait sur une machine que ce dépôt utilise tous les jours.

---

## Décision 1 — Une borne tenue par un tiers n'est pas tenue par le mécanisme attesté

L'ADR 0035 décision 1 refuse que `podman-rootless` et `bubblewrap` partagent un nom, et donne le
critère : « ils ne sont pas deux façons d'écrire la même garantie. Ils échouent différemment, ils
s'installent différemment. »

Le même critère, appliqué à `bubblewrap` seul et à `bubblewrap` **dans un cgroup posé par le
broker** :

- **ils échouent différemment.** Le second peut échouer d'au moins trois façons que le premier ne
  connaît pas : aucun cgroup délégué, contrôleur non activé pour les enfants, processus non déplacé
  dans le cgroup avant `exec`. Aucune de ces trois ne ressemble à un défaut de `bwrap`, et deux
  d'entre elles sont **silencieuses** — la sandbox tourne, simplement sans borne ;
- **ils s'installent différemment.** Le second exige une délégation que l'hôte accorde ou non, et la
  mesure ci-dessus montre les deux cas sur deux machines de ce chantier.

Ils ne partagent donc pas de nom. Un enregistrement qui porterait `backend: bubblewrap` en attestant
un niveau dont les bornes sont tenues ailleurs affirmerait de `bubblewrap` une propriété qu'il n'a
pas — la faute exacte que l'ADR 0035 a été écrit pour interdire, dans un costume neuf.

---

## Décision 2 — Le nom du mécanisme composé entre **avec** son implémentation, jamais avant

`CLAUDE.md` : « une sorte de relation […] n'entre dans son énumération que lorsqu'un consommateur
exécutable et testé existe ». L'ADR 0022 décision 0 dit la même chose autrement : on ne livre jamais
une promesse.

Ce document fixe donc la **règle**, pas le nom. Tant que le placement du cgroup n'est pas écrit et
éprouvé, aucune valeur nouvelle n'entre : un nom de mécanisme qui existerait sans mécanisme serait
une valeur d'énumération affirmant un effet qui n'a pas lieu.

Conséquence immédiate et assumée : jusque-là, un worker sous `bubblewrap` **ne peut pas** obtenir un
`Proven` à un niveau qui promet des bornes de ressources. C'est le constat exact, et la campagne de
`W5.af.3` le rend déjà lisible plutôt que de le taire.

---

## Décision 3 — C'est `locus-execd` qui pose le cgroup, et il **refuse** quand rien ne lui est délégué

L'ADR 0004 pose la division : `locus-execd` est le service privilégié, `locusd` ne détient jamais de
socket de runtime. Poser un cgroup autour d'un processus enfant est un acte privilégié sur l'hôte
d'exécution ; il appartient au broker, et à personne d'autre dans ce dépôt.

Le déploiement, lui, doit **déléguer** un cgroup au broker — on ne crée pas un cgroup dont on n'a pas
le parent. Ce que le déploiement fait pour cela (scope systemd, cgroup parent, autre) est hors de ce
document : ce qui compte ici est que le broker sache dire si la délégation a eu lieu.

Quand elle n'a pas eu lieu, le broker **refuse d'attester le niveau qui en dépend**. Il ne pose pas
silencieusement une sandbox sans borne : une ignorance ne se range pas du bon côté par défaut, et
deux des trois modes d'échec de la décision 1 sont précisément silencieux. `HostFacts` porte déjà la
lecture qu'il faut — les contrôleurs délégués **à ce processus**, et non ceux de la racine —, et le
conteneur de développement de ce chantier est le cas où elle rend l'ensemble vide.

---

## Décision 4 — La campagne doit exercer le **composé**, pas ses moitiés

L'ADR 0035 décision 4 : « `Proven` ne peut être rempli pour un worker réel que par une campagne
exerçant le mécanisme que ce worker emploie ».

Si le mécanisme est `bwrap` **dans** un cgroup posé par le broker, alors une campagne qui exercerait
`bwrap` seul mesurerait autre chose — et elle le mesurerait *bien*, ce qui est le piège : ses neuf
sondes de confinement passeraient, ses trois sondes de quota rendraient toujours `NotRun`, et
personne ne verrait qu'on a éprouvé la mauvaise chose. C'est le défaut `canterel-local` sous un
troisième costume.

La campagne du composé ouvre donc la sandbox **par le chemin qui pose le cgroup**, et ses trois
sondes de quota doivent alors conclure — `Blocked` ou `Succeeded`, mais plus `NotRun`. Ce passage de
`NotRun` à un verdict est le test de sortie de l'item qui implémentera la décision 3 : tant que les
trois n'ont rien lu, rien n'a été mesuré.

---

## Ce que cet ADR ne décide pas

- **Le nom du mécanisme composé.** Décision 2 : il entre avec son implémentation. Le choisir ici
  reviendrait à livrer une valeur d'énumération avant l'effet qu'elle annonce.
- **Comment le déploiement délègue le cgroup.** Scope systemd, cgroup parent, ou autre : c'est une
  propriété de l'hôte d'exécution, et ce dépôt n'en vise aucun en particulier. Ce qui est décidé est
  que le broker **lit** si la délégation a eu lieu et refuse sinon.
- **Si le `S2` du composé est comparable au `S2` de `podman-rootless`.** L'ADR 0035 traite les
  mécanismes comme incomparables faute de les avoir mesurés l'un contre l'autre ; ce document hérite
  de ce refus sans le rouvrir.
- **Ce que devient `unenforced` une fois le cgroup posé.** La liste cesserait d'être vide pour les
  mêmes contrôleurs, ce qui est cohérent, mais la forme exacte du témoignage — qui dit *qui* tient la
  borne — appartient à l'item qui l'écrira, avec un test qui la confronte à la campagne comme
  `W5.af.3` l'a fait.

## Ce que cet ADR débloque et ce qu'il ferme

Il ferme la question que `W5.ai` portait — « que dit l'attestation quand la borne est tenue par
quelqu'un d'autre que le mécanisme attesté » — et la ferme **par une application du critère déjà posé
en 0035**, non par une décision neuve.

Il laisse `W5.ai` entièrement du travail : lire la délégation, poser le cgroup, refuser sans elle,
nommer le composé, et faire passer les trois sondes de quota de `NotRun` à un verdict. Aucune de ces
cinq choses ne demande d'arbitrage supplémentaire.
