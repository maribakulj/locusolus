# Toolchain Registry et environnements

## Principe

Canterel ne doit pas deviner si Lean, PyTorch ou SageMath existent sur l’hôte. Locus Solus résout un `EnvironmentBlueprint` vers une image/environnement attesté.

## Profils V1

### `base`
Git, curl/wget, jq, ripgrep, bash, build-essential/clang selon OS, cmake, pkg-config, ffmpeg, pandoc.

### `python-science`
Python, uv, NumPy, SciPy, pandas, Polars, DuckDB, PyArrow, SymPy, scikit-learn, statsmodels, matplotlib, Jupyter, networkx.

### `ml-cpu`
PyTorch CPU, torchvision, transformers, datasets, tokenizers, sentence-transformers, ONNX Runtime, safetensors, llama.cpp.

### `ml-mps`
Worker macOS natif : PyTorch MPS, MLX/mlx-lm, llama.cpp Metal. Ce profil ne doit pas être prétendu portable dans une image Linux.

### `ml-cuda`
PyTorch CUDA, CUDA/cuDNN compatibles, transformers, vLLM/ONNX GPU selon besoin. Version de driver/runtime vérifiée par health check.

### `math-formal`
Lean 4 via elan, lake, mathlib, Z3, cvc5, éventuellement E/Vampire. Les projets Lean conservent `lean-toolchain` et `lake-manifest.json`.

### `math-compute`
SageMath, GAP, PARI/GP, Singular, Maxima et autres outils explicitement versionnés.

### `browser`
Chromium/Firefox, Playwright, PDF render/screenshot.

### `dh`
IIIF clients, ALTO/PageXML/TEI, XML sûr, RDF/SPARQL, Tesseract/OpenCV, GIS selon sous-profil.

### autres
R/renv, Julia/Manifest, GIS GDAL/PROJ, security Syft/Trivy/Cosign/Gitleaks.

## Build

Une demande de dépendance nouvelle déclenche `EnvironmentBuildWorkflow` : environnement réseau séparé → lockfile → build OCI → SBOM → scan → health checks → signature → digest. La mission scientifique redémarre ensuite avec l’environnement immuable.

## Interdictions

Pas de gigantesque image universelle. Pas de `sudo` dans une mission. Pas de package flottant sans version dans un environnement promu.
