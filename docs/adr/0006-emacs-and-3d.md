# ADR 0006 — Emacs et 3D

**Statut :** accepté.

**Contexte :** la tentation d'un rendu 3D dans Emacs existe et produirait un moteur graphique médiocre couplé à un éditeur.

**Décision :** Emacs est cockpit — il commande et inspecte. Les scènes 3D sont web-native (Three.js de référence), intégrables via WebView/xwidget quand le build le permet, avec fallback navigateur.

**Conséquences :** le service de visualisation produit une **projection**, jamais une copie mutable du graphe. Aucun frontend n'écrit directement dans le graphe. IDs stables des deux côtés ; toute mutation repasse par l'API de commandes.

**Rollback :** le fallback navigateur est le mode dégradé permanent ; il n'y a rien à annuler.
