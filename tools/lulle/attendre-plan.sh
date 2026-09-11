#!/bin/bash
# Attendre qu'un plan d'orchestration se termine, puis rendre la main.
#
# C'est le mécanisme de boucle : lancé en arrière-plan, ce script se termine
# quand le plan est fini, ce qui réveille la session pour l'étape suivante.
# Sans lui, chaque étape demande qu'on revienne la demander.
export PATH="/opt/homebrew/bin:$PATH"
limite=${1:-180}   # tours de 20 s, soit une heure par défaut
for i in $(seq 1 "$limite"); do
  etat=$(emacsclient --eval '(locus-orchestre-etat)' 2>/dev/null | tr -d '"')
  case "$etat" in
    *"aucun plan"*) echo "PLAN TERMINÉ après $((i*20))s"; exit 0;;
  esac
  sleep 20
done
echo "ATTENTE ÉCOULÉE — le plan tourne encore : $etat"
exit 0
