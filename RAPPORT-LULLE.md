# Les volvelles de Lulle — ce qui a été construit, ce qui a été trouvé

*État au 11 septembre 2026. Tout ce qui est chiffré ici a été mesuré, pas estimé.*

---

## 1. Ce que le système fait, aujourd'hui

Une question de recherche entre par Emacs, traverse le plan de contrôle, est
exécutée par un agent confiné, et son résultat revient — puis nourrit l'étape
suivante, sans qu'on relance rien.

```
Emacs (plan)  →  locusd  →  placement  →  worker Canterel  →  modèle
     ↑                                                            │
     └──────────── résultat relu, étape suivante soumise ──────────┘
```

Ce qui tient, vérifié de bout en bout :

| capacité | état | preuve |
|---|---|---|
| Confinement attesté | ✅ | `S2` prouvé sous `bubblewrap+cgroup`, attestation liée à l'hôte et au worker |
| Placement | ✅ | `task.proposed → queued → leased → run.started → run.completed` |
| Budget par mission | ✅ | une mission arrêtée à `stopped_on_budget: true`, dépense rapportée |
| Budget de plan | ✅ | plafond cumulé, le plan s'arrête et les étapes restantes ne partent pas |
| Enchaînement | ✅ | étape 1 rend « 427 », étape 2 répond « départ=427 résultat=434 » |
| Relais de contenu | ✅ | `GET /tasks/{id}/result`, ajouté ; la sortie du modèle y voyage |
| Assignation par capacité | ✅ | `required_capabilities` → modèle, table locale à l'installation |
| Réseau par mission | ✅ | le mode déclaré atteint la sandbox, `curl` rend le vrai `info.json` |
| Cockpit | ✅ | missions repliées du journal, rafraîchissement automatique |
| Vision IIIF dans Emacs | ✅ | xiiif affiche les folios ; corpus ouvrable d'une touche |

---

## 2. Le corpus

**49 manuscrits lulliens de la Biblioteca de Catalunya, 10 126 folios**, tous
vérifiés par téléchargement de leur manifeste IIIF.

*Arbre de ciència* (584 f.), *Ars inventiva veritatis* (514), *Ars generalis
ultima* (456), *Llibre de Contemplació* (394), *Testamentum* (288), *Ars brevis*
(78), *Introductoria Artis demonstrativae* (46), *Lectura compendiosa*, *Recull
de textos lul·lians*, *Miscel·lània de textos lul·lians*…

Disponibilité réelle des images, mesurée sur 288 canvas : **283 servis par IIIF,
4 par une voie de secours** (l'API directe de ContentDM sert là où IIIF échoue),
**1 perdu**. 47 manuscrits sur 49 sont entièrement disponibles.

---

## 3. Les figures trouvées

Un détecteur géométrique — gratuit, sans modèle — a réduit 10 126 folios à 40
candidats, puis un modèle de vision les a décrits. Dix figures construites :

| planche | type | ce qu'on y lit |
|---|---|---|
| *Ars brevis* f. 1v | **rota** centrée sur **A** | *Bonitas, Magnitudo, Duratio, Potestas* — les dignités |
| *Lectura compendiosa* f. 8r | **rota** | *Deus, Angelus, Homo, animalia, plantes, ignis, aer, aqua, terra* |
| *Lectura compendiosa* f. 7r | **triangle lettré** | 4 éléments, 4 qualités, 4 humeurs |
| *Testamentum* p. 270 | **cercles concentriques** | *Quinta Essentia*, *Rota digestionum* |
| *Testamentum* p. 266 | **triangles lettrés** | *Figura compositionis Albi sulphuris* |
| *Ars oratoria* f. 60r, 58v | **médaillons à lettres** | art de la mémoire |
| *Ars oratoria* f. 2v | **arbre** | *Prudentia, Temperantia, Fortitudo, Iustitia* |
| *Recull* f. 34r | **cercles concentriques** | lettres rouges |

Onze planches sont des initiales ornées ou des bois narratifs — les faux
positifs du détecteur, reconnus comme tels. Dix-sept ne portent rien.

Un cas gardé tel quel : une **autorisation d'exportation de la Biblioteca
Nazionale de Rome**, tamponnée dans la *Miscel·lània*, classée `table` et
décrite exactement. Feuille moderne reliée dans un manuscrit ancien.

---

## 4. L'observation, et sa correction

### L'hypothèse posée

Dans ce seul fonds, un même dispositif — **une lettre isolée dans une case
circulaire** — traverse trois genres sans rapport : théologie (*Ars brevis*,
lettres = dignités divines), rhétorique (*Ars oratoria*, lettres = lieux de
mémoire), alchimie (*Testamentum*, lettres = substances). Le signe est le même,
sa sémantique change entièrement.

### L'objection

Le système l'a éprouvée plutôt qu'approuvée, et a opposé deux choses justes :
**3 folios sur 40**, c'est rare ; et l'échantillon ayant été choisi par un
détecteur de cercles, il ne pouvait pas confirmer une observation sur les
cercles.

### Le tirage au sort

91 folios tirés au hasard, deux par manuscrit, graine fixe.

Première mesure, question posée directement : **53 %** des folios porteraient le
motif. Deux positifs vérifiés à l'œil : **deux hallucinations**. Un verso vierge
où le modèle voyait « une lettre A dans un triangle rouge » ; une page de texte
où il lisait « I, H, S dans un compartiment allongé ».

Le défaut n'est pas la vision — le même modèle a lu correctement *Rota
digestionum* et *Quinta Essentia*. C'est la **forme de la question** : un
oui/non sur un détail subtil, posé d'une page ordinaire, obtient oui.

Avec une porte — demander d'abord ce que la page *est*, ne poser la seconde
question que si elle porte quelque chose — **53 % → 3,6 %**, cohérent avec
elle-même. Soit de l'ordre de **366 folios porteurs** sur 10 126.

### Ce que le tirage a trouvé

***Recull miscel·lani de textos pseudo-lul·lians*, f. 105** : sept **tables
combinatoires**, chaque case portant un triplet de lettres isolées — `b d c`,
`b d f`, `b d g`, `g t d c`, `g t d f` — et chaque table introduite par une
rubrique opératoire :

> *Tabula prima dat modum qualiter extrahatur … de vino rubeo et albo*
> *Sequens tabula dat modum faciendi elixir ex saturno et iove ut fiat ex eis sol et luna*
> *Sequens tabula docet qualiter ex saturno et marte fiat elixir*

C'est la combinatoire de l'Ars — l'alphabet de principes, combiné en cases —
devenue **procédure d'atelier**. Et ce n'est pas une roue : c'est un tableau.

### L'hypothèse corrigée

L'élément portable n'est pas « la lettre dans une case circulaire ». C'est **la
lettre comme jeton combinable dans une case**, et le contenant migre : roue,
table, triangle, médaillon. La roue n'est qu'un emballage parmi d'autres.

En retirant le biais du cercle, le tirage a montré ce que l'échantillon biaisé
ne pouvait pas voir.

### Ce qu'il faudrait faire ensuite

- Étendre le tirage : 2 folios par manuscrit ne donnent que 3 folios porteurs.
  Un tirage à 10 par manuscrit donnerait un ordre de grandeur fiable et coûterait
  environ 1,50 $.
- Chercher le motif dans les contenants **non circulaires** — le détecteur ne
  les voit pas, et c'est là que le tableau de f. 105 se cachait.
- Dater et localiser : le motif apparaît-il à une période, dans un atelier ?

---

## 5. Ce que ça coûte

**Environ 1,30 $ sur 10 €** pour tout ce qui précède.

Deux chiffres qui décident du reste :

**Le plancher par appel est d'environ 20 000 jetons d'entrée** — prompt système
et contexte, avant même la question.

**Un modèle faible coûte plus cher qu'un modèle fort.** La même reconnaissance
IIIF, deux fois :

| modèle | appels | coût | résultat |
|---|---|---|---|
| `pixtral-12b` | 53 | 0,30 $ | échec — a mal lu son propre `jq`, conclu à une panne DNS inexistante |
| `mistral-large` | 12 | **0,042 $** | 81 manifestes vérifiés |

Il paie en tâtonnements ce qu'il n'a pas en jugement.

---

## 6. Les cinq leçons qui ont coûté quelque chose

**Un agent coûte des jetons, une boucle `for` n'en coûte pas.** La mesure de
disponibilité des images, confiée à un agent, a rendu un fichier de 49 entrées
toutes à zéro — la structure était là, la mesure n'avait pas tourné, et rien ne
le disait. Refaite en Python : le vrai chiffre en une minute, zéro centime.

**La perception n'est pas une mission.** Décrire 40 planches, confié à la boucle
d'agent : elle a lu les 12 premières images avec succès, puis a produit des
plans et des passations au lieu de descriptions, et a fini par affirmer avoir
écrit un fichier inexistant. Un appel par image, rien à compacter, et c'est
réglé.

**Un agent regarde des fichiers, pas des URLs.** L'outil de lecture attache une
image en base64 ; donné 40 URLs, l'agent a entrepris de réimplémenter un
détecteur OpenCV. Ce n'était pas de la désobéissance : un agent de code répond
par du code.

**Une absence n'est pas une mesure.** 29 appels sur 40 ont échoué en DNS. Sans
reprise, un défaut de transport devient un trou dans les données, et un trou se
lit comme une planche sans figure.

**Une question mal posée obtient la réponse qu'elle appelle.** 53 % contre 3,6 %
selon qu'on demande « vois-tu ceci ? » ou « qu'est-ce que c'est ? ».

---

## 7. Ce qui reste ouvert

- **`allowlist` n'est pas applicable** : le worker ne sait annoncer que `deny` ou
  `full`. Une mission à liste blanche d'hôtes n'est pas tenable aujourd'hui.
- **Sept tests rouges dans xiiif**, antérieurs à ce chantier, non examinés.
- **L'interface** : le cockpit Emacs montre du texte. Les vignettes, les
  colonnes, les barres de progression et les graphes n'y sont pas — voir la
  note qui suit ce rapport.
