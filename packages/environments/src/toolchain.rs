//! Les profils de toolchain — `docs/SPEC_V1.md` §19.4.

use std::fmt;

/// Un profil du Toolchain Registry.
///
/// # Pourquoi l'énumération est fermée
///
/// §19.4 : « les versions sont verrouillées ; on n'installe pas tout dans une image universelle ».
/// Un profil est un contrat sur ce que l'image contient, et un profil inconnu est un contrat que
/// personne n'a écrit. Le laisser passer sous forme de chaîne libre ferait qu'un
/// `pyhton-science` mal orthographié produirait une image sans Python, et le blueprint dirait
/// pourtant qu'elle en a un.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolchainProfile {
    /// `git`, `curl`, `jq`, `ripgrep`, build tools, `ffmpeg`, `pandoc`.
    Base,
    /// Python/`uv`, `NumPy`, `SciPy`, pandas/Polars, `DuckDB`, `PyArrow`, `SymPy`, scikit-learn, Jupyter.
    PythonScience,
    /// `PyTorch` CPU, `torchvision`, `transformers`, `datasets`, ONNX Runtime, `llama.cpp`.
    MlCpu,
    /// Capability macOS native : `PyTorch` MPS/MLX/Metal. **Non portable en image Linux.**
    MlMps,
    /// `PyTorch` CUDA, CUDA/`cuDNN`, `vLLM`/ONNX GPU selon image.
    MlCuda,
    /// Lean 4 via `elan`, `lake`, `mathlib`, Z3, `cvc5`.
    MathFormal,
    /// `SageMath`, GAP, PARI/GP, `Singular`/`Macaulay2` selon disponibilité.
    MathCompute,
    /// Chromium/Firefox, `Playwright`, outillage PDF et capture d'écran.
    Browser,
    /// Clients IIIF, ALTO/`PageXML`/TEI, XML sûr, RDF/SPARQL, `Tesseract`/`OpenCV`.
    Dh,
    /// Profil complémentaire R.
    R,
    /// Profil complémentaire Julia.
    Julia,
    /// Profil complémentaire SIG.
    Gis,
    /// Profil complémentaire outillage de sécurité.
    Security,
}

impl ToolchainProfile {
    /// Les treize, dans l'ordre de §19.4.
    pub const ALL: [Self; 13] = [
        Self::Base,
        Self::PythonScience,
        Self::MlCpu,
        Self::MlMps,
        Self::MlCuda,
        Self::MathFormal,
        Self::MathCompute,
        Self::Browser,
        Self::Dh,
        Self::R,
        Self::Julia,
        Self::Gis,
        Self::Security,
    ];

    /// Le nom que §19.4 lui donne, et que les templates emploient.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::PythonScience => "python-science",
            Self::MlCpu => "ml-cpu",
            Self::MlMps => "ml-mps",
            Self::MlCuda => "ml-cuda",
            Self::MathFormal => "math-formal",
            Self::MathCompute => "math-compute",
            Self::Browser => "browser",
            Self::Dh => "dh",
            Self::R => "r",
            Self::Julia => "julia",
            Self::Gis => "gis",
            Self::Security => "security",
        }
    }

    /// Relire un nom de profil.
    ///
    /// `None` plutôt qu'un profil par défaut : un nom inconnu est une erreur de configuration, et
    /// lui substituer `base` produirait une image sans ce que la mission croyait avoir demandé.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.slug() == value)
    }

    /// Vrai quand ce profil n'existe pas en image Linux portable.
    ///
    /// §19.4 le dit du seul `ml-mps` : « capability macOS native, **non image Linux portable** ».
    /// C'est le pendant, côté environnement, de la portée d'accélérateur de W4.f : un profil natif
    /// ne s'emporte pas dans un conteneur, et un blueprint qui prétendrait le faire promettrait une
    /// image qui n'existe pas.
    #[must_use]
    pub const fn is_native_only(self) -> bool {
        matches!(self, Self::MlMps)
    }
}

impl fmt::Display for ToolchainProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}
