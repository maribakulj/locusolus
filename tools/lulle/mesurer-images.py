"""Mesurer ce que le serveur d'images sert réellement, manuscrit par manuscrit.

Pur I/O : aucun modèle, aucun coût. Ce qui demande du jugement viendra après ;
compter ce qui répond n'en demande pas, et le faire faire à un agent revient à
payer des jetons pour une boucle `for`.
"""
import json, urllib.request, urllib.error, concurrent.futures as cf

CORPUS = "/srv/locus/travail/w1/corpus.json"
SORTIE = "/srv/locus/travail/w1/images.json"
ECHANTILLON = 6          # canvas par manuscrit, répartis régulièrement
DELAI = 20

def lire(url, timeout=DELAI):
    req = urllib.request.Request(url, headers={"User-Agent": "locus-solus/0.1"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, b""
    except Exception:
        return 0, b""

def services(manifeste):
    code, corps = lire(manifeste, 30)
    if code != 200:
        return []
    try:
        m = json.loads(corps)
    except Exception:
        return []
    cs = (m.get("sequences") or [{}])[0].get("canvases", [])
    out = []
    for c in cs:
        try:
            out.append(c["images"][0]["resource"]["service"]["@id"])
        except Exception:
            pass
    return out

def echantillon(liste, n):
    if len(liste) <= n:
        return liste
    pas = len(liste) / n
    return [liste[int(i * pas)] for i in range(n)]

def mesurer(entree):
    svcs = echantillon(services(entree["url"]), ECHANTILLON)
    ok_iiif = ok_sec = ko = 0
    for s in svcs:
        code, _ = lire(s + "/full/500,/0/default.jpg")
        if code == 200:
            ok_iiif += 1
            continue
        # Voie de secours : l'API directe de ContentDM, qui sert là où IIIF casse.
        try:
            coll, item = s.rsplit("/", 1)[-1].split(":")
        except ValueError:
            ko += 1
            continue
        code2, _ = lire(f"https://mdc.csuc.cat/digital/api/singleitem/image/{coll}/{item}/default.jpg")
        if code2 == 200:
            ok_sec += 1
        else:
            ko += 1
    return {
        "url_manifeste": entree["url"],
        "label": entree.get("label"),
        "canvas_total": entree.get("canvases"),
        "canvas_testes": len(svcs),
        "ok_iiif": ok_iiif,
        "ok_secours": ok_sec,
        "echecs": ko,
    }

corpus = json.load(open(CORPUS))
with cf.ThreadPoolExecutor(max_workers=8) as ex:
    resultats = list(ex.map(mesurer, corpus))
json.dump(resultats, open(SORTIE, "w"), ensure_ascii=False, indent=1)

ti = sum(r["ok_iiif"] for r in resultats)
ts = sum(r["ok_secours"] for r in resultats)
tk = sum(r["echecs"] for r in resultats)
print("manuscrits : %d" % len(resultats))
print("canvas testés : %d" % sum(r["canvas_testes"] for r in resultats))
print("servis par IIIF : %d | par secours : %d | perdus : %d" % (ti, ts, tk))
utilisables = [r for r in resultats if r["ok_iiif"] + r["ok_secours"] >= r["canvas_testes"] and r["canvas_testes"] > 0]
print("manuscrits entièrement servis : %d" % len(utilisables))
for r in sorted(utilisables, key=lambda r: -(r["canvas_total"] or 0))[:12]:
    print("  %5s folios | %s" % (r["canvas_total"], str(r["label"])[:55]))
