"""Mesurer la fréquence du motif « lettre isolée dans une case », sans biais.

# Ce que cette mesure décide

Une observation tirée d'un échantillon choisi par un détecteur de cercles ne
peut pas être confirmée par cet échantillon. Ce script pose la même question à
un tirage au sort, et la question **ne nomme pas le cercle** : on demande si une
lettre est isolée dans une case fermée, quelle qu'en soit la forme.

Deux fréquences comparables en sortent — l'une sur l'échantillon dirigé, l'autre
sur le tirage — et leur écart est ce qui dira si le motif est un fait du fonds
ou un reflet de l'instrument.
"""
import base64, json, os, sys, time, urllib.request, concurrent.futures as cf

MODELE = "mistral-large-latest"
CLE = os.environ.get("MISTRAL_API_KEY", "")

# # Une porte avant la question, parce qu'un modèle interrogé trouve
#
# Premier jet : on demandait directement « une lettre isolée est-elle enfermée
# dans une case ? ». Réponse sur un tirage au sort : **53 %** des folios. Deux
# positifs vérifiés à l'œil, deux hallucinations — un verso vierge où le modèle
# a vu « une lettre A dans un triangle rouge », une page de texte où il a lu
# « I, H, S dans un compartiment allongé ». Ni l'un ni l'autre n'existaient.
#
# Le défaut n'est pas la vision : sur les planches à figure saillante, ce même
# modèle a lu correctement *Rota digestionum* et *Quinta Essentia*. Il est dans
# la **forme de la question**. Un oui/non sur un détail subtil, posé d'une page
# ordinaire, obtient oui : le modèle comble.
#
# La porte inverse la charge. On demande d'abord ce que la page **est** — vierge,
# texte seul, texte avec initiales, texte avec schéma, page de figure — et la
# question sur les lettres n'est posée que si la page porte quelque chose. Une
# page de texte ne peut plus produire un motif qu'elle n'a pas.
PORTE = (
    "Tu regardes un folio de manuscrit ou d'imprimé ancien. Réponds UNIQUEMENT par un objet JSON "
    "avec les clés : nature (une seule valeur parmi : vierge, texte_seul, texte_avec_initiales, "
    "texte_avec_schema, page_de_figure, illustration_narrative, document_moderne), "
    "certitude (haute, moyenne, basse), description (une phrase factuelle). "
    "vierge : la page ne porte rien, ou seulement un cachet, un numéro, une transparence de l'autre "
    "côté. texte_seul : uniquement du texte courant. texte_avec_initiales : du texte et des lettres "
    "ornées qui commencent des paragraphes. texte_avec_schema : du texte ET un dessin géométrique. "
    "page_de_figure : la page est occupée par une ou plusieurs figures construites. N'invente rien."
)

CONSIGNE = (
    "Tu regardes un folio de manuscrit ou d'imprimé ancien. Réponds UNIQUEMENT par un objet JSON, "
    "sans texte autour, avec les clés : "
    "lettre_en_case (true si une ou plusieurs LETTRES ISOLÉES — une seule lettre, détachée de tout "
    "mot — sont enfermées dans une case fermée d'une forme quelconque : cercle, triangle, carré, "
    "médaillon, compartiment ; false sinon), "
    "forme (la forme de la case : cercle, triangle, carre, medaillon, autre, ou aucune), "
    "combien (nombre de lettres ainsi isolées, 0 si aucune), "
    "lettres (la liste des lettres que tu lis ainsi isolées, liste vide si aucune), "
    "figure_quelconque (true s'il y a un schéma, diagramme, arbre ou table sur la page, false si la "
    "page ne porte que du texte courant ou une illustration narrative), "
    "description (une phrase factuelle). "
    "Une initiale ornée au début d'un paragraphe n'est PAS une lettre isolée en case : elle est "
    "attachée au texte qu'elle commence. N'invente rien."
)

def appeler(b64, consigne):
    corps = json.dumps({
        "model": MODELE, "temperature": 0, "max_tokens": 400,
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": consigne},
            {"type": "image_url", "image_url": "data:image/jpeg;base64," + b64}]}],
    }).encode()
    req = urllib.request.Request("https://api.mistral.ai/v1/chat/completions", data=corps,
        headers={"Authorization": "Bearer " + CLE, "Content-Type": "application/json"})
    rep, dernier = None, ""
    for essai in range(4):
        try:
            with urllib.request.urlopen(req, timeout=120) as r:
                rep = json.loads(r.read())
            break
        except Exception as e:
            dernier = str(e)[:110]
            time.sleep(2 * (essai + 1))
    if rep is None:
        return None, {"erreur": dernier}, 0, 0
    txt = rep["choices"][0]["message"]["content"].strip()
    if txt.startswith("```"):
        txt = txt.strip("`")
        txt = txt[txt.find("{"):txt.rfind("}") + 1]
    u = rep.get("usage", {})
    try:
        return json.loads(txt), None, u.get("prompt_tokens") or 0, u.get("completion_tokens") or 0
    except Exception:
        return None, {"erreur": "réponse non JSON"}, u.get("prompt_tokens") or 0, u.get("completion_tokens") or 0


PORTEUSES = ("texte_avec_schema", "page_de_figure")


def regarder(entree):
    with open(entree["fichier"], "rb") as f:
        b64 = base64.b64encode(f.read()).decode()
    porte, err, je, js = appeler(b64, PORTE)
    if err:
        return {**entree, **err}
    if porte.get("nature") not in PORTEUSES:
        # La porte referme : pas de seconde question, donc pas de motif inventé.
        return {**entree, **porte, "lettre_en_case": False, "combien": 0, "lettres": [],
                "figure_quelconque": False, "jetons_entree": je, "jetons_sortie": js}
    v, err2, je2, js2 = appeler(b64, CONSIGNE)
    if err2:
        return {**entree, **porte, **err2}
    return {**entree, **porte, **v, "figure_quelconque": True,
            "jetons_entree": je + je2, "jetons_sortie": js + js2}

if __name__ == "__main__":
    index, sortie = sys.argv[1], sys.argv[2]
    entrees = json.load(open(index))
    acquis = {}
    if os.path.exists(sortie):
        try:
            for e in json.load(open(sortie)):
                if not e.get("erreur"):
                    acquis[e.get("fichier")] = e
        except Exception:
            pass
    a_faire = [e for e in entrees if e["fichier"] not in acquis]
    print("acquises : %d — à regarder : %d" % (len(acquis), len(a_faire)))
    with cf.ThreadPoolExecutor(max_workers=2) as ex:
        neuves = list(ex.map(regarder, a_faire))
    par_fichier = {**acquis, **{n["fichier"]: n for n in neuves}}
    out = [par_fichier[e["fichier"]] for e in entrees]
    json.dump(out, open(sortie, "w"), ensure_ascii=False, indent=1)

    bons = [e for e in out if not e.get("erreur")]
    avec = [e for e in bons if e.get("lettre_en_case")]
    fig = [e for e in bons if e.get("figure_quelconque")]
    je = sum(e.get("jetons_entree") or 0 for e in bons)
    js = sum(e.get("jetons_sortie") or 0 for e in bons)
    print("folios lus : %d (erreurs : %d)" % (len(bons), len(out) - len(bons)))
    print("lettre isolée en case : %d  (%.1f %%)" % (len(avec), 100.0 * len(avec) / max(1, len(bons))))
    print("figure quelconque      : %d  (%.1f %%)" % (len(fig), 100.0 * len(fig) / max(1, len(bons))))
    from collections import Counter
    print("formes :", Counter(str(e.get("forme")) for e in avec).most_common())
    print("coût estimé : %.3f $" % (je / 1e6 * 2 + js / 1e6 * 6))
