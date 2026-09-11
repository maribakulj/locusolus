"""Télécharger les folios candidats sur disque, pour que l'agent puisse les **voir**.

# Pourquoi le téléchargement est ici et pas dans une mission

L'outil de lecture du harnais attache un fichier image au modèle en base64 : il
faut donc un fichier local, pas une URL. Un agent à qui l'on donne des URLs ne
peut pas regarder — et, mesuré, il fait autre chose : confronté à quarante URLs,
il a entrepris de réimplémenter un détecteur de cercles avec OpenCV, épuisé
1,5 million de jetons et rendu zéro planche classée.

Ce n'était pas une désobéissance. Un agent de code répond à une tâche par du
code ; c'est à l'appel de lui donner de quoi regarder plutôt que de quoi coder.
"""
import json, os, urllib.request, concurrent.futures as cf

SOURCE = "/srv/locus/travail/w1/liste-courte.json"
DOSSIER = "/srv/locus/travail/w1/planches"
INDEX = "/srv/locus/travail/w1/planches/index.json"

os.makedirs(DOSSIER, exist_ok=True)

def nom(i, e):
    ms = "".join(c if c.isalnum() else "-" for c in str(e.get("label_ms", ""))[:28]).strip("-")
    fol = "".join(c if c.isalnum() else "-" for c in str(e.get("label_canvas", ""))[:12]).strip("-")
    return "%02d_%s_%s.jpg" % (i, ms, fol)

def tirer(arg):
    i, e = arg
    chemin = os.path.join(DOSSIER, nom(i, e))
    if os.path.exists(chemin) and os.path.getsize(chemin) > 5000:
        return {**e, "fichier": chemin}
    req = urllib.request.Request(e["url_grande"], headers={"User-Agent": "locus-solus/0.1"})
    try:
        with urllib.request.urlopen(req, timeout=40) as r:
            if r.status != 200:
                return None
            octets = r.read()
    except Exception:
        return None
    with open(chemin, "wb") as f:
        f.write(octets)
    return {**e, "fichier": chemin, "octets": len(octets)}

entrees = list(enumerate(json.load(open(SOURCE)), start=1))
with cf.ThreadPoolExecutor(max_workers=6) as ex:
    tires = [x for x in ex.map(tirer, entrees) if x]
json.dump(tires, open(INDEX, "w"), ensure_ascii=False, indent=1)
print("planches téléchargées : %d sur %d" % (len(tires), len(entrees)))
total = sum(t.get("octets", 0) for t in tires)
print("poids : %.1f Mo" % (total / 1e6))
for t in tires[:5]:
    print("  %s" % os.path.basename(t["fichier"]))
