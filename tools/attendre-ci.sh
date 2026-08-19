#!/usr/bin/env bash
# Attendre qu'une CI se termine, et réveiller ce qui attend.
#
# # Pourquoi ce script existe
#
# Une session qui travaille la roadmap en boucle n'a, pendant qu'une CI tourne, aucun appel qui
# avance : il ne lui reste que le sondage. Un tour qui n'a plus d'appel à faire produit du texte, et
# produire du texte **est** l'arrêt. Le réveil est donc ce qui permet à la boucle de ne pas dépendre
# de la patience de qui la conduit — voir « Rythme de session » dans `CLAUDE.md`.
#
# Il se lance en arrière-plan et rend la main quand tous les jobs bloquants ont conclu.
#
# # La faute qu'il évite, et qui a déjà été commise
#
# La première version de cette attente comptait les jobs non terminés ainsi :
#
#     sum(1 for r in reponse.get("check_runs", []) if r["status"] != "completed")
#
# Sans jeton, l'API rend un `401`. Le corps d'un `401` n'a pas de clé `check_runs`. Le `.get` rendait
# donc une liste vide, la somme valait `0`, et « aucun job en cours » a été lu comme « tout est
# fini » : l'attente s'est terminée immédiatement, en annonçant un succès qu'elle n'avait pas
# constaté. C'est la faute que ce dépôt nomme partout — le silence lu comme un succès.
#
# Ce script distingue donc trois choses, et ne les confond jamais :
#
#   - la requête n'a pas abouti        → `exit 2`, bruyamment ;
#   - la réponse n'a aucun job          → `exit 2` aussi : une CI sans job n'est pas une CI verte ;
#   - la réponse a des jobs, tous finis → `exit 0`, et c'est le seul chemin qui rend zéro.
#
# Usage : tools/attendre-ci.sh <owner/repo> <sha> [secondes-entre-deux-lectures] [budget-secondes]

set -euo pipefail

repo="${1:?owner/repo attendu}"
sha="${2:?sha attendu}"
pause="${3:-30}"
budget="${4:-2400}"

if [ -z "${GITHUB_TOKEN:-}" ]; then
  echo "attendre-ci : GITHUB_TOKEN absent — une attente sans lecture ne conclut rien" >&2
  exit 2
fi

debut=$(date +%s)
while :; do
  corps=$(curl -sS --fail-with-body \
    -H "Authorization: Bearer $GITHUB_TOKEN" \
    -H "Accept: application/vnd.github+json" \
    "https://api.github.com/repos/$repo/commits/$sha/check-runs" 2>&1) || {
    echo "attendre-ci : la lecture a échoué — $corps" >&2
    exit 2
  }

  verdict=$(printf '%s' "$corps" | python3 -c '
import json, sys

reponse = json.load(sys.stdin)
if "check_runs" not in reponse:
    print("SANS_CLE")
    raise SystemExit
jobs = reponse["check_runs"]
if not jobs:
    print("SANS_JOB")
    raise SystemExit
restants = [j["name"] for j in jobs if j["status"] != "completed"]
if restants:
    print("EN_COURS " + " ".join(restants))
else:
    print("FINI " + " ".join(f"{j[\"name\"]}={j[\"conclusion\"]}" for j in jobs))
') || {
    echo "attendre-ci : réponse illisible" >&2
    exit 2
  }

  case "$verdict" in
    SANS_CLE)
      echo "attendre-ci : la réponse ne porte pas de « check_runs » — ce n'est pas zéro job, c'est zéro réponse" >&2
      exit 2
      ;;
    SANS_JOB)
      echo "attendre-ci : aucun job pour $sha — une CI sans job n'est pas une CI verte" >&2
      exit 2
      ;;
    FINI*)
      echo "attendre-ci : ${verdict#FINI }"
      exit 0
      ;;
    *)
      echo "attendre-ci : ${verdict#EN_COURS } — nouvelle lecture dans ${pause} s"
      ;;
  esac

  if [ $(($(date +%s) - debut)) -ge "$budget" ]; then
    echo "attendre-ci : budget de ${budget} s épuisé, jobs encore en cours" >&2
    exit 3
  fi
  sleep "$pause"
done
