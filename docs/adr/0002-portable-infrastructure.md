# ADR 0002 — Infrastructure portable

**Statut :** accepté.

**Contexte :** la même mission doit tourner sur MacBook, Mac mini, VM Linux, container cloud ou GPU distant, avec des garanties déclarées.

**Décision :** toutes les dépendances d'infrastructure passent par des ports.

**Alternative écartée :** coder d'abord le profil local et abstraire plus tard. Rejetée parce que l'abstraction rétroactive épouse toujours la forme du premier backend.

**Conséquences :** mêmes objets métier partout. `packages/domain` n'importe aucun package d'infrastructure, et la CI le vérifie.

**Rollback :** aucun — c'est une contrainte de structure, pas un composant.
