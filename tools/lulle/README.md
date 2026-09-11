# Les outils du corpus lullien

Ce que ces scripts font, et **pourquoi ils ne sont pas des missions**.

Un agent coûte des jetons ; une boucle `for` n'en coûte pas. Tout ce qui est
déterministe — compter ce qui répond, télécharger, mesurer une géométrie — se
fait ici, gratuitement et de façon reproductible. Ce qui demande du jugement —
« est-ce une rota ou un arbre ? » — passe par une mission.

La première version faisait mesurer la disponibilité des images par un agent.
Il a écrit un fichier de quarante-neuf entrées toutes à zéro : la structure
était là, la mesure n'avait pas tourné, et rien ne le disait. Refaite ici, la
même mesure a rendu le vrai chiffre en une minute.

## L'ordre

1. `mesurer-images.py` — ce que le serveur sert réellement, manuscrit par
   manuscrit, IIIF puis voie de secours ContentDM. Mesuré : 283 canvas sur 288
   par IIIF, 4 par secours, 1 perdu ; 47 manuscrits sur 49 entièrement servis.
2. `cibles.py` — les folios à analyser, échantillonnés dans les œuvres de l'Ars.
3. `detecter-cercles.py` — les folios porteurs d'un trait circulaire. Aucun
   modèle : c'est de la géométrie, et la géométrie se mesure.
4. `liste-courte.py` — les quarante candidats que la vision aura à juger.
5. `attendre-plan.sh` — attend la fin d'un plan et rend la main. Lancé en
   arrière-plan, il réveille la session pour l'étape suivante : c'est ce qui
   fait qu'une boucle est une boucle et non une suite de relances.

## Ce que le détecteur a appris en trois rédactions

**Somme de l'énergie de contour** : classait en tête l'`Ars moriendi`, un texte
dense sans figure. Une page très encrée a un gradient fort partout, donc sur
n'importe quel cercle qu'on y trace. Il mesurait la densité d'encre en croyant
mesurer une forme.

**Alignement radial du gradient** : mieux, mais une page gothique dense soutient
encore n'importe quel cercle — à quatre cents pixels, la moitié d'une
circonférence quelconque rencontre un trait tangent par hasard.

**Contraste avec les cercles voisins** : un cercle *dessiné* a de l'encre sur son
tracé et pas à sept pixels de là. Une page uniforme s'annule. Le folio 270 du
`Testamentum` — quatre rotae, dont une `Rota digestionum` nommée sur la page —
est alors arrivé premier, et le folio 266 du même manuscrit troisième.

Il reste des faux positifs : les initiales rubriquées et les cachets de
bibliothèque sont ronds. C'est voulu — le détecteur trie, il ne juge pas.
