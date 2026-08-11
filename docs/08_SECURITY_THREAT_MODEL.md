# Sécurité et threat model synthétique

## Principaux adversaires

- code généré ou téléchargé ;
- prompt injection dans pages/documents ;
- package malveillant ;
- worker compromis ;
- artefact actif hostile ;
- fuite de token OAuth/API ;
- SSRF et metadata cloud ;
- confusion de contexte entre branches/reviewers.

## Défenses

- least privilege ;
- sandbox réelle ;
- réseau deny-by-default ;
- secret broker et credentials courts ;
- séparation OAuth local / service credentials distants ;
- ContextView immuable ;
- viewers actifs isolés ;
- content addressing ;
- SBOM/signature/scans ;
- audit event log ;
- data classification et egress policy.

## Données

Classification minimale : public, internal, confidential, restricted. Un worker annonce les classes acceptées. Une mission restricted ne doit pas être routée vers un cloud public sans policy explicite.

## OAuth

Un token personnel utilisé par Canterel reste sur un worker de confiance local. Il n’est jamais copié dans une sandbox de code ni transmis à un worker opportuniste.
