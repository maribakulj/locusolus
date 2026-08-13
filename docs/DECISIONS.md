# DECISIONS — décisions architecturales consolidées

- D001 : Locus Solus est le nom du laboratoire ; Canterel celui du runtime agentique.
- D002 : LEP est générique et n’est pas nommé d’après Canterel.
- D003 : **révisé par ADR 0009** — quatre repos (`locusolus`, `canterel`, `xiiif`, `emacs-config`). Le client Emacs vit dans `locusolus/apps/emacs`, pas dans un dépôt séparé.
- D004 : deployment-agnostic via ports/adapters.
- D005 : PostgreSQL/event store comme vérité de référence ; caches/index dérivés.
- D006 : WorkflowBackend abstrait ; Temporal backend de référence local/VM.
- D007 : Execution Fabric séparée du runtime agentique.
- D008 : `locus-execd` sépare les privilèges d’exécution de `locusd`.
- D009 : toolchains versionnées ; pas d’image universelle.
- D010 : OAuth personnel uniquement sur workers de confiance locaux.
- D011 : Emacs = cockpit ; Web = visualisation riche ; 3D web-native.
- D012 : xiiif = viewer humain IIIF, pas agent tool obligatoire.
- D013 : artifact-first ; viewers remplaçables.
- D014 : GPU/MPS/CUDA = capabilities de workers.
- D015 : local, Mac mini, VM, cloud et hybrid partagent le même domaine.
- D016 : **ADR 0011** — `locusd`, `locus-execd` et la CLI en Rust ; web, SDK client et worker en TypeScript. Amende §4.5.
