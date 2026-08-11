# Artefacts, visualisation et Emacs 3D

## Artifact-first

Les viewers ne sont jamais sources de vérité. Une figure, scène 3D, Content State, notebook ou proof doit exister comme artefact avec hash et provenance.

## Viewer registry

- Markdown/Org → Emacs/native ;
- PNG/JPEG/SVG/PDF → native/external ;
- HTML → WebView/browser ;
- IIIF → xiiif/Mirador/OpenSeadragon ;
- graph large → web graph viewer ;
- glTF/GLB → Three.js ;
- point cloud → Potree ;
- molecule → Mol* ;
- scientific volume → vtk.js ;
- notebook → Jupyter/rendered HTML.

## 3D

Le service de visualisation Locus génère une projection, jamais une copie mutable du graphe. Application web Three.js de référence. Emacs peut l’intégrer via WebKit/xwidget si disponible ou ouvrir le navigateur.

Vues V1 : espace épistémique, paysage de branches, temps/provenance, société d’agents, artefacts 3D.

## Interaction

IDs stables. Emacs peut envoyer `focus`, `filter`, `select`; le viewer renvoie `node_selected`, `artifact_opened`, etc. Toute mutation passe ensuite par command API et confirmation appropriée.

## IIIF

Agents : APIs/headless. Humain : xiiif/Mirador/OpenSeadragon. Le Content State est un excellent artefact portable mais xiiif n’est pas requis à sa création.
