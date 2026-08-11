# Profil VM Linux

Services : locusd, PostgreSQL, Temporal backend de référence, object store, locus-execd et workers CPU. Containers rootless/cgroups v2 appliquent ressources. Le daemon et la DB restent hors des sandboxes de mission.

Exigences : backups, firewall, OIDC/SSH d’administration, volumes persistants, logs, monitoring disque/mémoire, aucun runtime socket exposé aux agents.
