"""Faire décrire chaque planche par un modèle de vision, une planche par appel.

# Pourquoi ce n'est pas une mission

Une mission passe par la boucle d'agent : elle planifie, appelle des outils,
compacte son contexte, se passe le relais à elle-même. C'est ce qu'il faut pour
un travail qui se découvre en le faisant. Décrire quarante planches n'est pas ce
travail-là : c'est quarante jugements **indépendants**, chacun tenant en un
regard.

Mesuré : confiée à la boucle, la tâche a lu les douze images avec succès — la
vision marche — puis a produit des plans et des passations au lieu de
descriptions, et a fini par affirmer avoir écrit un fichier qui n'existait pas.
Le harnais est un agent de code ; on lui demandait de la perception en série.

Un appel par image, un JSON par appel, rien à compacter. L'agent garde ce qui
demande de décider ; l'instrument fait ce qui demande de voir.

# Ce que le modèle a le droit de répondre

Une énumération fermée, et « aucune » en fait partie. Sans elle, un modèle
trouve toujours quelque chose — c'est le biais qui ferait classer les auréoles
d'un bois gravé parmi les rotae.
"""
import base64, json, os, sys, urllib.request, concurrent.futures as cf

INDEX = "/srv/locus/travail/w1/planches/index.json"
SORTIE = "/srv/locus/travail/w1/typologie.json"
MODELE = "mistral-large-latest"
CLE = os.environ.get("MISTRAL_API_KEY", "")

TYPES = ["rota", "cercles_concentriques", "triangle_lettre", "arbre",
         "table", "medaillons_lettres", "initiale_ornee", "bois_narratif",
         "cachet", "aucune"]

CONSIGNE = (
    "Tu regardes un folio de manuscrit ou d'imprimé ancien. Réponds UNIQUEMENT par un objet "
    "JSON, sans texte autour, avec les clés : type (une valeur parmi %s), figure (true si une "
    "figure géométrique construite est présente, false sinon), description (une phrase en "
    "français, factuelle, sur ce que tu vois), mots_lus (liste des mots latins ou catalans "
    "que tu arrives à lire sur ou autour de la figure ; liste vide si tu n'en lis aucun). "
    "N'invente aucun mot : ne rapporte que ce que tu lis réellement. Si la page ne porte que "
    "du texte, type vaut aucune et figure vaut false."
) % ", ".join(TYPES)

def regarder(entree):
    with open(entree["fichier"], "rb") as f:
        b64 = base64.b64encode(f.read()).decode()
    corps = json.dumps({
        "model": MODELE,
        "temperature": 0,
        "max_tokens": 500,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": CONSIGNE},
                {"type": "image_url", "image_url": "data:image/jpeg;base64," + b64},
            ],
        }],
    }).encode()
    req = urllib.request.Request(
        "https://api.mistral.ai/v1/chat/completions", data=corps,
        headers={"Authorization": "Bearer " + CLE, "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            rep = json.loads(r.read())
    except Exception as e:
        return {**entree, "erreur": str(e)[:120]}
    txt = rep["choices"][0]["message"]["content"].strip()
    # Le modèle encadre parfois son JSON ; on ne réécrit pas sa réponse, on la dégage.
    if txt.startswith("```"):
        txt = txt.strip("`")
        txt = txt[txt.find("{"):txt.rfind("}") + 1]
    try:
        verdict = json.loads(txt)
    except Exception:
        return {**entree, "erreur": "réponse non JSON", "brut": txt[:200]}
    u = rep.get("usage", {})
    return {**entree, **verdict,
            "jetons_entree": u.get("prompt_tokens"), "jetons_sortie": u.get("completion_tokens")}

if __name__ == "__main__":
    if not CLE:
        sys.exit("MISTRAL_API_KEY manquante")
    entrees = json.load(open(INDEX))
    n = int(sys.argv[1]) if len(sys.argv) > 1 else len(entrees)
    entrees = entrees[:n]
    with cf.ThreadPoolExecutor(max_workers=4) as ex:
        out = list(ex.map(regarder, entrees))
    json.dump(out, open(SORTIE, "w"), ensure_ascii=False, indent=1)

    from collections import Counter
    c = Counter(str(e.get("type", "erreur")) for e in out)
    je = sum(e.get("jetons_entree") or 0 for e in out)
    js = sum(e.get("jetons_sortie") or 0 for e in out)
    print("planches regardées : %d" % len(out))
    print("jetons : %d en entrée, %d en sortie" % (je, js))
    for k, v in c.most_common():
        print("  %2d  %s" % (v, k))
