"""Bâtir la liste des folios à analyser, depuis les manuscrits de l'Ars."""
import json, urllib.request, concurrent.futures as cf

CORPUS = "/srv/locus/travail/w1/corpus.json"
SORTIE = "/srv/locus/travail/w1/cibles.json"
# Les œuvres où les figures vivent — l'Ars et ses dérivés, pas la prose.
MOTS = ("Ars ", "Arbre de ci", "Ars generalis", "Ars inventiva", "Testamentum",
        "Introductoria", "Lectura compendiosa", "Arte breve", "lul·lians")
PAR_MS = 40   # folios échantillonnés par manuscrit

def lire(u, t=30):
    req = urllib.request.Request(u, headers={"User-Agent": "locus-solus/0.1"})
    with urllib.request.urlopen(req, timeout=t) as r:
        return json.loads(r.read())

def folios(e):
    try:
        m = lire(e["url"])
    except Exception:
        return []
    cs = (m.get("sequences") or [{}])[0].get("canvases", [])
    out = []
    for i, c in enumerate(cs):
        try:
            svc = c["images"][0]["resource"]["service"]["@id"]
        except Exception:
            continue
        out.append({"label_ms": e.get("label"), "manifeste": e["url"], "index": i,
                    "label_canvas": str(c.get("label")), "url": svc + "/full/400,/0/default.jpg"})
    if len(out) <= PAR_MS:
        return out
    pas = len(out) / PAR_MS
    return [out[int(i * pas)] for i in range(PAR_MS)]

corpus = [e for e in json.load(open(CORPUS)) if any(m.lower() in str(e.get("label","")).lower() for m in MOTS)]
print("manuscrits retenus : %d" % len(corpus))
for e in corpus:
    print("  %5s | %s" % (e.get("canvases"), str(e.get("label"))[:55]))
with cf.ThreadPoolExecutor(max_workers=6) as ex:
    tout = [f for lot in ex.map(folios, corpus) for f in lot]
json.dump(tout, open(SORTIE, "w"), ensure_ascii=False)
print("folios à analyser : %d" % len(tout))
