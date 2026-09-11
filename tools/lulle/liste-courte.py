"""La liste courte : ce que la vision aura à juger.

Le détecteur géométrique a réduit dix mille folios à un classement. Il ne dit
pas « volvelle » — il dit « il y a ici un trait circulaire qui ne ressemble pas
à son voisinage ». Le tri final demande un jugement, et c'est ce qu'on paie.
"""
import json
f = json.load(open("/srv/locus/travail/w1/figures.json"))
# Quarante : assez pour une typologie, assez peu pour que la vision reste
# anecdotique dans le budget. Le seuil se relèvera si la moisson est bonne.
courte = f[:40]
for e in courte:
    e["url_grande"] = e["url"].replace("/full/400,/", "/full/900,/")
json.dump(courte, open("/srv/locus/travail/w1/liste-courte.json", "w"), ensure_ascii=False, indent=1)
print("liste courte : %d folios" % len(courte))
ms = {}
for e in courte:
    ms[e["label_ms"]] = ms.get(e["label_ms"], 0) + 1
for k, v in sorted(ms.items(), key=lambda kv: -kv[1]):
    print("  %2d | %s" % (v, str(k)[:55]))
