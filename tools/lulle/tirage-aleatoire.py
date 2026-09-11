"""Un échantillon tiré au sort, pour mesurer sans le biais du détecteur.

# Pourquoi ce tirage existe

Les quarante premières planches ont été choisies par un détecteur de traits
circulaires. Toute fréquence mesurée sur elles décrit donc le détecteur autant
que le fonds : si l'on y trouve beaucoup de lettres dans des cases rondes, c'est
d'abord parce qu'on a cherché des choses rondes.

Une observation tirée de cet échantillon ne peut pas être confirmée par lui. Il
faut un second échantillon dont la sélection **ignore** ce qu'on cherche — d'où
un tirage au sort, stratifié par manuscrit pour qu'un codex de six cents folios
n'écrase pas un de soixante.

La graine est fixe : le tirage doit pouvoir être rejoué à l'identique, sans quoi
la mesure n'est pas vérifiable.
"""
import json, os, random, urllib.request, concurrent.futures as cf

CORPUS = "/srv/locus/travail/w1/corpus.json"
DOSSIER = "/srv/locus/travail/w1/tirage"
INDEX = "/srv/locus/travail/w1/tirage/index.json"
PAR_MS = 2          # deux folios par manuscrit : la strate, pas le volume
GRAINE = 1232       # l'année de naissance de Llull, et surtout une constante

os.makedirs(DOSSIER, exist_ok=True)

def lire(u, t=40):
    req = urllib.request.Request(u, headers={"User-Agent": "locus-solus/0.1"})
    with urllib.request.urlopen(req, timeout=t) as r:
        return r.read()

def services(e):
    try:
        m = json.loads(lire(e["url"]))
    except Exception:
        return []
    cs = (m.get("sequences") or [{}])[0].get("canvases", [])
    out = []
    for i, c in enumerate(cs):
        try:
            out.append((i, str(c.get("label")), c["images"][0]["resource"]["service"]["@id"]))
        except Exception:
            pass
    return out

def tirer_ms(e):
    rng = random.Random("%s|%d" % (e["url"], GRAINE))
    svcs = services(e)
    if not svcs:
        return []
    # Les premières et dernières pages sont des gardes et des plats : elles
    # portent rarement du contenu, et les tirer fausserait la mesure vers `aucune`.
    coeur = svcs[2:-2] if len(svcs) > 8 else svcs
    choisis = rng.sample(coeur, min(PAR_MS, len(coeur)))
    out = []
    for idx, lab, svc in choisis:
        out.append({"label_ms": e.get("label"), "manifeste": e["url"], "index": idx,
                    "label_canvas": lab, "url_grande": svc + "/full/900,/0/default.jpg"})
    return out

def telecharger(arg):
    i, e = arg
    ms = "".join(c if c.isalnum() else "-" for c in str(e["label_ms"])[:26]).strip("-")
    chemin = os.path.join(DOSSIER, "t%02d_%s_%s.jpg" % (i, ms, e["index"]))
    if os.path.exists(chemin) and os.path.getsize(chemin) > 5000:
        return {**e, "fichier": chemin}
    try:
        octets = lire(e["url_grande"])
    except Exception:
        return None
    if len(octets) < 5000:
        return None
    open(chemin, "wb").write(octets)
    return {**e, "fichier": chemin}

corpus = json.load(open(CORPUS))
with cf.ThreadPoolExecutor(max_workers=6) as ex:
    cibles = [f for lot in ex.map(tirer_ms, corpus) for f in lot]
print("folios tirés : %d sur %d manuscrits" % (len(cibles), len(corpus)))
with cf.ThreadPoolExecutor(max_workers=4) as ex:
    tires = [x for x in ex.map(telecharger, enumerate(cibles, 1)) if x]
json.dump(tires, open(INDEX, "w"), ensure_ascii=False, indent=1)
print("téléchargés : %d" % len(tires))
