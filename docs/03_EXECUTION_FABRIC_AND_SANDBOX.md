# Execution Fabric et sandbox

## Objectif

Permettre la même mission sur MacBook, Mac mini, VM Linux, cloud container ou GPU distant avec des garanties déclarées et vérifiables.

## Objets

### `SandboxSpec`

- isolation minimum ;
- réseau ;
- mounts ;
- secrets/capabilities ;
- readonly rootfs ;
- uid/gid ;
- timeouts ;
- règles de promotion des artefacts.

### `ResourceSpec`

- CPU minimum/préféré/max ;
- RAM ;
- disque ;
- PIDs ;
- wall time ;
- accelerator type/count/VRAM si pertinent.

### `SandboxAttestation`

Backend, image digest, rootless, network mode, mounts, capabilities dropped, limits appliquées, self-tests et timestamp.

## Local macOS

Profil recommandé : host macOS + VM Linux légère + containers rootless par mission. `locus-execd` parle au backend ; `locusd` ne reçoit jamais le socket runtime. Pour MPS/MLX, un worker macOS de confiance séparé annonce capability `mps` et reçoit uniquement les tâches compatibles.

## Linux VM

Rootless Podman/containerd ou micro-VM selon trust level. cgroups v2 pour CPU/RAM/PIDs, quota filesystem, namespaces, seccomp/AppArmor/SELinux lorsque disponibles.

## Cloud

Un backend cloud est conforme seulement s’il peut déclarer et faire respecter isolation, CPU/RAM/disque, lifecycle et egress. L’absence de GPU ou limite de taille devient une capability normale.

## Niveaux d’isolation

- S0 : process de confiance, aucun code non fiable ;
- S1 : permissions/logical sandbox ;
- S2 : OS sandbox ;
- S3 : rootless container dans boundary forte/VM ;
- S4 : micro-VM ou sandbox cloud par mission pour code hostile.

La politique choisit le niveau minimal par mission.

## Self-tests obligatoires

- écriture hors workspace ;
- lecture home ;
- accès socket runtime ;
- accès metadata cloud ;
- egress deny ;
- symlink escape ;
- fork bomb contrôlée ;
- dépassement mémoire ;
- quota disque ;
- tentative de lecture de secrets.

Un backend qui échoue à un test critique n’est pas `trusted`.

## Inspection utilisateur

CLI/Emacs/Web : list, inspect, logs, files, metrics, attestation, terminate. Shell humain optionnel et audité ; toute mutation manuelle marque le run.
