//! L'état d'un artefact : quarantaine, puis promotion — ADR 0005, `docs/SPEC_V1.md` §19.2.

use std::fmt;

/// Où en est un artefact.
///
/// # Pourquoi c'est un champ et non une suite de types
///
/// W5.b a porté la chaîne de construction par une suite de types, parce qu'un build est un
/// **processus** : il se déroule une fois, au même endroit, et l'ordre de ses étapes est celui des
/// appels. Un état d'artefact est un **fait** : il voyage dans un manifeste, se sérialise, se relit
/// six mois plus tard, se compare entre pairs fédérés. Un typestate le rendrait inexprimable en
/// JSON, et `artifact-manifest.schema.json` le déclare bel et bien comme une énumération.
///
/// Ce qui reste à tenir est donc la légalité des **transitions**, et c'est la forme de
/// `TaskState::transition` du domaine, pour la même raison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactState {
    /// Le hash est annoncé, le contenu n'est pas encore arrivé.
    Declared,
    /// Le contenu est arrivé, et son hash correspond à ce qui avait été annoncé.
    Uploaded,
    /// Le contenu est retenu : il vient d'une source non fiable, ou un scan l'a signalé.
    Quarantined,
    /// Le contenu a été vérifié.
    Verified,
    /// L'artefact est promu : il peut être cité, servi, dérivé.
    Promoted,
    /// L'artefact est refusé.
    Rejected,
}

impl ArtifactState {
    /// Les six, dans l'ordre du schéma.
    pub const ALL: [Self; 6] = [
        Self::Declared,
        Self::Uploaded,
        Self::Quarantined,
        Self::Verified,
        Self::Promoted,
        Self::Rejected,
    ];

    /// Le nom que `artifact-manifest.schema.json` emploie.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Uploaded => "uploaded",
            Self::Quarantined => "quarantined",
            Self::Verified => "verified",
            Self::Promoted => "promoted",
            Self::Rejected => "rejected",
        }
    }

    /// Relire un nom d'état.
    ///
    /// `None` plutôt qu'un défaut : un état inconnu traité comme `declared` ferait réuploader un
    /// artefact promu, et traité comme `promoted` servirait un contenu que personne n'a vérifié.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.slug() == value)
    }

    /// Les états atteignables depuis celui-ci.
    ///
    /// # Ce que la table refuse, et pourquoi
    ///
    /// `declared → promoted` n'existe pas : ADR 0005 dit « hash déclaré avant upload, **quarantaine
    /// puis promotion** », et sauter d'un bout à l'autre servirait un contenu que personne n'a vu.
    /// `uploaded → promoted` non plus : un contenu arrivé n'est pas un contenu vérifié.
    ///
    /// `Promoted` est terminal. Retirer un artefact promu n'est pas une transition d'état — ce
    /// serait effacer qu'il a été cité — mais un acte de revue, qui viendra avec W7 et qui laissera
    /// sa propre trace. L'invariant 12 vaut ici comme ailleurs : rien ne disparaît pour faire propre.
    #[must_use]
    pub fn allowed(self) -> &'static [Self] {
        match self {
            Self::Declared => &[Self::Uploaded, Self::Rejected],
            Self::Uploaded => &[Self::Quarantined, Self::Verified, Self::Rejected],
            Self::Quarantined => &[Self::Verified, Self::Rejected],
            Self::Verified => &[Self::Promoted, Self::Rejected],
            Self::Promoted | Self::Rejected => &[],
        }
    }

    /// Vrai quand plus rien ne suit.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        self.allowed().is_empty()
    }

    /// Vrai quand cet état autorise à servir le contenu.
    ///
    /// Un seul le fait. Le dire par une fonction plutôt que par une comparaison éparpillée évite
    /// que quelqu'un écrive un jour `state != Rejected` en croyant dire la même chose.
    #[must_use]
    pub const fn is_servable(self) -> bool {
        matches!(self, Self::Promoted)
    }
}

impl fmt::Display for ArtifactState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.slug())
    }
}

/// Une transition que la table refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbiddenTransition {
    /// D'où.
    pub from: ArtifactState,
    /// Vers où.
    pub to: ArtifactState,
    /// Ce qui était possible.
    pub allowed: Vec<ArtifactState>,
}

impl fmt::Display for ForbiddenTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.allowed.is_empty() {
            return write!(
                formatter,
                "« {} » est terminal : rien ne mène à « {} »",
                self.from, self.to
            );
        }
        let allowed: Vec<&str> = self.allowed.iter().map(|state| state.slug()).collect();
        write!(
            formatter,
            "« {} » ne mène pas à « {} » — seulement à {}",
            self.from,
            self.to,
            allowed.join(", ")
        )
    }
}

impl std::error::Error for ForbiddenTransition {}

/// Franchir une transition.
///
/// # Errors
///
/// [`ForbiddenTransition`] quand la table ne l'autorise pas, en nommant ce qui l'était.
pub fn transition(
    from: ArtifactState,
    to: ArtifactState,
) -> Result<ArtifactState, ForbiddenTransition> {
    if from.allowed().contains(&to) {
        return Ok(to);
    }
    Err(ForbiddenTransition {
        from,
        to,
        allowed: from.allowed().to_vec(),
    })
}
