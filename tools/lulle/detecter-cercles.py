"""Repérer les folios porteurs d'une grande figure circulaire.

# Pourquoi un détecteur avant la vision

Le corpus fait dix mille folios. Les passer tous à un modèle de vision coûterait
le budget entier pour découvrir que la plupart sont du texte courant. Une figure
lullienne est d'abord une **propriété géométrique** — de l'encre disposée en
cercle —, et la géométrie se mesure sans modèle.

Le détecteur ne dit pas « c'est une volvelle ». Il dit « il y a ici une
structure circulaire », ce qui est vérifiable, reproductible et gratuit. Le
jugement vient après, sur les candidats.

# La mesure, et la première qui ne marchait pas

Premier jet : sommer l'énergie de contour le long du cercle. Mesuré sur 721
folios, il a classé en tête l'*Ars moriendi* — un texte dense, sans figure. Le
défaut est net une fois vu : une page très encrée a un gradient fort **partout**,
donc sur n'importe quel cercle qu'on y trace. Le détecteur mesurait la densité
d'encre en croyant mesurer une forme.

Ce qui distingue vraiment un cercle est l'**orientation** du gradient. Sur un
trait circulaire, il pointe vers le centre — il est radial. Dans du texte, les
orientations sont quelconques et s'annulent. On mesure donc l'alignement moyen
entre le gradient et la direction radiale, ce qui est sans dimension et
indifférent à la quantité d'encre.

Le score reste normalisé par le nombre de points échantillonnés, sans quoi les
grands rayons gagneraient toujours — ce qui ferait préférer un cercle imaginaire
passant par les marges à une figure réelle plus petite.
"""
import json, sys, urllib.request, urllib.error, math
import numpy as np
from PIL import Image
import io, concurrent.futures as cf

TAILLE = 400          # côté de la vignette analysée
RAYONS = range(40, 170, 8)
PAS_CENTRE = 12       # grille des centres, en pixels

def charger(url, timeout=25):
    req = urllib.request.Request(url, headers={"User-Agent": "locus-solus/0.1"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            if r.status != 200:
                return None
            return Image.open(io.BytesIO(r.read())).convert("L")
    except Exception:
        return None

def score_circulaire(img):
    """Le meilleur score de cercle de l'image, et sa géométrie."""
    img = img.resize((TAILLE, TAILLE))
    a = np.asarray(img, dtype=np.float32) / 255.0
    # Gradient : l'encre contre le parchemin. Un simple Sobel suffit — on cherche
    # une disposition, pas une frontière fine.
    gx = np.zeros_like(a); gy = np.zeros_like(a)
    gx[:, 1:-1] = a[:, 2:] - a[:, :-2]
    gy[1:-1, :] = a[2:, :] - a[:-2, :]
    g = np.sqrt(gx * gx + gy * gy)
    g = g / (g.max() + 1e-6)

    # Gradient normalisé : seule l'orientation compte, pas l'intensité.
    norme = g + 1e-6
    ux, uy = gx / norme, gy / norme
    # Les pixels sans contour n'ont pas d'orientation ; les compter ajouterait du
    # bruit orienté au hasard. On les éteint plutôt que de les laisser voter.
    seuil = float(np.quantile(g, 0.80))
    actif = (g >= seuil).astype(np.float32)

    def soutien(cx, cy, r):
        """La fraction de la circonférence portée par un trait tangent."""
        n = max(48, int(2 * math.pi * r / 3))
        ang = np.linspace(0, 2 * math.pi, n, endpoint=False)
        rx, ry = np.cos(ang), np.sin(ang)
        ys = (cy + r * ry).astype(np.int32)
        xs = (cx + r * rx).astype(np.int32)
        if ys.min() < 0 or ys.max() >= TAILLE or xs.min() < 0 or xs.max() >= TAILLE:
            return None
        align = np.abs(ux[ys, xs] * rx + uy[ys, xs] * ry)
        return float(((align > 0.72) & (actif[ys, xs] > 0)).mean())

    meilleur = (0.0, 0, 0, 0)
    centres = range(TAILLE // 4, 3 * TAILLE // 4, PAS_CENTRE)
    for r in RAYONS:
        n = max(48, int(2 * math.pi * r / 3))
        ang = np.linspace(0, 2 * math.pi, n, endpoint=False)
        rx, ry = np.cos(ang), np.sin(ang)          # direction radiale au point
        dx = (r * rx).astype(np.int32)
        dy = (r * ry).astype(np.int32)
        for cy in centres:
            ys = cy + dy
            if ys.min() < 0 or ys.max() >= TAILLE:
                continue
            for cx in centres:
                xs = cx + dx
                if xs.min() < 0 or xs.max() >= TAILLE:
                    continue
                # |gradient · radiale| : 1 quand le trait est perpendiculaire au
                # rayon, c'est-à-dire tangent au cercle ; 0 quand il est quelconque.
                align = np.abs(ux[ys, xs] * rx + uy[ys, xs] * ry)
                # **La couverture, pas la moyenne.** Un cercle réel est un trait
                # *continu* : la quasi-totalité de sa circonférence porte de l'encre
                # orientée. Une page de texte peut faire monter une moyenne avec
                # quelques points bien orientés par hasard, sur un cercle qui
                # n'existe pas — mesuré : un folio de l'`Arbre de ciència` entièrement
                # typographique classé sixième.
                #
                # On compte donc la *fraction* de la circonférence qui est à la fois
                # encrée et tangente. Un seuil plutôt qu'une moyenne : ce qui compte
                # est qu'un point soutienne le cercle, pas de combien.
                ici = float(((align > 0.72) & (actif[ys, xs] > 0)).mean())
                if ici <= meilleur[0]:
                    continue
                # **Le contraste, pas le soutien seul.** Une page de texte gothique
                # dense soutient n'importe quel cercle : à quatre cents pixels,
                # la moitié d'une circonférence quelconque rencontre un trait
                # tangent par hasard. Mesuré — un folio entièrement typographique
                # de l'`Arbre de ciència` arrivait premier.
                #
                # Un cercle *dessiné* a de l'encre sur son tracé et pas à six
                # pixels de là. On retranche donc le meilleur soutien des cercles
                # voisins : une page uniforme s'annule, un trait réel ressort.
                voisins = [soutien(cx, cy, r + d) for d in (-7, 7)]
                voisins = [v for v in voisins if v is not None]
                s = ici - (max(voisins) if voisins else 0.0)
                if s > meilleur[0]:
                    meilleur = (s, cx, cy, r)
    return meilleur

def traiter(entree):
    img = charger(entree["url"])
    if img is None:
        return None
    s, cx, cy, r = score_circulaire(img)
    return {**entree, "score": round(s, 4), "centre": [cx, cy], "rayon": r}

if __name__ == "__main__":
    cibles = json.load(open(sys.argv[1]))
    with cf.ThreadPoolExecutor(max_workers=6) as ex:
        out = [x for x in ex.map(traiter, cibles) if x]
    out.sort(key=lambda e: -e["score"])
    json.dump(out, open(sys.argv[2], "w"), ensure_ascii=False, indent=1)
    print("analysés : %d" % len(out))
    for e in out[:15]:
        print("  %.4f r=%3d | %s | %s" % (e["score"], e["rayon"], str(e.get("label_ms"))[:34], str(e.get("label_canvas"))[:12]))
