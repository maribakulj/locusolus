//! Test de sortie de W13.e — **les quatre garanties de l'item.**
//!
//! 1. Deux propositions concurrentes sur la même base ne committent pas toutes deux, et le refus
//!    dit s'il faut rebaser.
//! 2. Une proposition sans justification citant un objet épistémique existant est refusée.
//! 3. Aucun chemin de code ne modifie une `MissionEnvelope` émise ni le hash de sa `ContextView`.
//! 4. Une proposition d'origine agentique suit le même chemin qu'une proposition humaine, et son
//!    proposeur ne peut pas l'approuver.

use std::collections::BTreeSet;

use locus_coordination::{
    Author, ContentDigest, CoordinationMode, Diff, DiffError, EpistemicIndex, Justification, Mode,
    Operation, Proposal, ProposalError, Relation, RelationKind, Version, approve, commit,
};
use locus_domain::RevisionId;
use locus_protocol::{
    Id, IdKind, Timestamp,
    id::{Agent, provisional::Approval, provisional::Decision as DecisionKind},
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn revision(seed: u8) -> RevisionId {
    id::<locus_domain::ids::RevisionKind>(seed)
}

/// Un index qui connaît quelques révisions, et pas les autres.
///
/// C'est le port de la sixième frontière : la proposition demande « cette révision existe-t-elle »
/// et rien de plus. Elle ne traverse aucun graphe, et ce crate n'importe pas `locus-graph`.
struct KnownRevisions(BTreeSet<String>);

impl KnownRevisions {
    fn with(revisions: &[RevisionId]) -> Self {
        Self(revisions.iter().map(ToString::to_string).collect())
    }
}

impl EpistemicIndex for KnownRevisions {
    fn contains(&self, revision: &RevisionId) -> bool {
        self.0.contains(&revision.to_string())
    }
}

fn justification() -> Justification {
    Justification::new("review_disagreement", revision(1)).expect("déclencheur non vide")
}

fn index() -> KnownRevisions {
    KnownRevisions::with(&[revision(1)])
}

fn reviews(from: u8, to: u8) -> Relation {
    Relation {
        from: id::<Agent>(from),
        to: id::<Agent>(to),
        kind: RelationKind::Review,
    }
}

/// L'organisation courante : deux agents, aucune relation.
///
/// `ContentDigest` plutôt qu'un condensat jouet, et c'est délibéré : ce fichier teste le **chemin de
/// commit**, pas la stabilité de la forme canonique — celle-ci a son propre test, avec sa fixture
/// figée. Employer ici l'implémentation de production vérifie en passant qu'elle se compose.
fn organisation() -> Version {
    Version::root(
        &[id::<Agent>(1), id::<Agent>(2)],
        &[],
        CoordinationMode::Blackboard,
        None,
        &ContentDigest,
    )
    .expect("fixture cohérente")
}

/// Le diff que la proposition porte : une relation de revue entre les deux membres.
fn diff() -> Diff {
    Diff::declaring(
        &organisation(),
        vec![Operation::AddEdge(reviews(1, 2))],
        &ContentDigest,
    )
    .expect("l'opération s'applique")
}

fn human_proposal(base: u64) -> Proposal {
    Proposal::write(
        id::<DecisionKind>(1),
        Author::Human("usr-marie".to_owned()),
        Mode::Observed,
        base,
        diff(),
        justification(),
        &index(),
    )
    .expect("un humain propose sous tout mode")
}

// ---------------------------------------------------------------------------------------------
// 1 — Deux propositions concurrentes
// ---------------------------------------------------------------------------------------------

/// Le CAS de §22.2. La seconde proposition a été écrite contre un monde qui n'existe plus, et le
/// refus **dit quoi faire** : un « conflit » sans consigne laisse l'appelant réessayer à
/// l'identique jusqu'à ce que quelqu'un lise le code.
#[test]
fn deux_propositions_sur_la_meme_base_ne_committent_pas_toutes_deux() {
    let current = 18;

    let first = approve(
        human_proposal(current),
        Author::Human("usr-gov".to_owned()),
        id::<Approval>(1),
    )
    .expect("approbateur distinct");
    let second = approve(
        human_proposal(current),
        Author::Human("usr-gov".to_owned()),
        id::<Approval>(2),
    )
    .expect("approbateur distinct");

    let committed =
        commit(first, current, &organisation(), &ContentDigest).expect("la première commite");
    assert_eq!(committed.revision, current + 1);

    let refused = commit(second, committed.revision, &organisation(), &ContentDigest)
        .expect_err("la seconde est périmée");
    assert_eq!(
        refused,
        ProposalError::Stale {
            expected: current,
            actual: committed.revision
        }
    );
    assert!(
        refused.needs_rebase(),
        "le refus doit dire s'il faut rebaser"
    );
    assert!(
        refused.to_string().contains("rebaser"),
        "et le dire en toutes lettres : {refused}"
    );
}

#[test]
fn une_proposition_a_jour_commite() {
    let approved = approve(
        human_proposal(18),
        Author::Human("usr-gov".to_owned()),
        id::<Approval>(1),
    )
    .expect("approbateur distinct");
    let committed = commit(approved, 18, &organisation(), &ContentDigest).expect("base à jour");
    assert_eq!(committed.revision, 19);
    assert_eq!(committed.proposal.diff(), &diff());

    // **Le commit produit une version**, et c'est ce que l'ADR 0021 a rebranché : avant, il rendait
    // un compteur et le diff n'était appliqué à rien.
    assert_eq!(committed.version.parent(), Some(organisation().id()));
    assert!(committed.version.relations().contains(&reviews(1, 2)));
    assert_ne!(
        committed.version.content_hash(),
        organisation().content_hash()
    );
}

/// **Deux vérifications, et aucune ne couvre l'autre.**
///
/// La révision dit que personne n'a écrit entre-temps ; le rejeu dit que ce qu'on applique est bien
/// ce qui a été approuvé. Ici la révision **correspond** et le diff ne s'applique pourtant pas : il
/// a été écrit sur une autre lignée. Un système qui n'aurait que le CAS aurait commité en croyant
/// avoir vérifié.
#[test]
fn une_revision_a_jour_ne_dispense_pas_de_rejouer_le_diff() {
    let approved = approve(
        human_proposal(18),
        Author::Human("usr-gov".to_owned()),
        id::<Approval>(1),
    )
    .expect("approbateur distinct");

    // Même contenu, autre lignée : un troisième agent est entré puis sorti, donc l'identité diffère.
    let autre = organisation()
        .apply(&Operation::AddNode(id::<Agent>(3)), &ContentDigest)
        .expect("un nœud entre")
        .apply(&Operation::RemoveNode(id::<Agent>(3)), &ContentDigest)
        .expect("et ressort");
    assert_eq!(
        autre.content_hash(),
        organisation().content_hash(),
        "le contenu est revenu"
    );
    assert_ne!(autre.id(), organisation().id(), "l'histoire non");

    let refused = commit(approved, 18, &autre, &ContentDigest).expect_err("autre lignée");
    let ProposalError::Inapplicable { detail, rebase } = &refused else {
        panic!("le refus doit distinguer le rejeu du CAS : {refused:?}");
    };
    assert!(*rebase, "le diff a dit qu'il fallait rebaser");
    assert!(detail.contains("rebaser"), "{detail}");
}

/// Une annulation est le commit d'un diff **inverse**, pas la suppression d'une version.
///
/// Retirer une version rendrait l'histoire fausse : on ne pourrait plus dire qu'une mission a
/// tourné sous une organisation qui, désormais, n'aurait jamais existé.
#[test]
fn une_annulation_est_un_commit_inverse_qui_ne_supprime_rien() {
    let original = human_proposal(18);
    let committed = commit(
        approve(
            original.clone(),
            Author::Human("usr-gov".to_owned()),
            id::<Approval>(1),
        )
        .expect("approbateur distinct"),
        18,
        &organisation(),
        &ContentDigest,
    )
    .expect("commitée");

    // L'inverse s'écrit sur la version que le commit a **produite**, pas sur celle d'avant : c'est
    // là qu'il doit s'appliquer.
    let inverse = original
        .diff()
        .inverse(&committed.version, &ContentDigest)
        .expect("ajouter une arête s'annule exactement");

    let cancelling = Proposal::write(
        id::<DecisionKind>(2),
        Author::Human("usr-marie".to_owned()),
        Mode::Observed,
        committed.revision,
        inverse,
        justification(),
        &index(),
    )
    .expect("proposition valide")
    .cancelling(original.id());

    assert_eq!(cancelling.cancels(), Some(original.id()));
    assert_eq!(
        cancelling.diff().operations(),
        &[Operation::RemoveEdge(reviews(1, 2))]
    );

    let undone = commit(
        approve(
            cancelling,
            Author::Human("usr-gov".to_owned()),
            id::<Approval>(2),
        )
        .expect("approbateur distinct"),
        committed.revision,
        &committed.version,
        &ContentDigest,
    )
    .expect("commitée à son tour");

    assert_eq!(
        undone.revision,
        committed.revision + 1,
        "l'annulation produit une version de plus, jamais une de moins"
    );

    // **Le contenu revient, l'histoire non** — la propriété que `version.rs` tient depuis W15.a, et
    // que le chemin de proposition doit préserver de bout en bout.
    assert_eq!(
        undone.version.content_hash(),
        organisation().content_hash(),
        "l'état revient"
    );
    assert_ne!(
        undone.version.id(),
        organisation().id(),
        "et il y est revenu par un commit de plus, pas par un retrait"
    );
    assert_eq!(undone.version.parent(), Some(committed.version.id()));
}

/// **Un diff se défait dans l'ordre inverse**, et l'ordre n'est pas un détail.
///
/// Retirer une arête puis son nœud se défait en remettant le nœud **puis** l'arête. L'ordre naïf —
/// inverser chaque opération en place — produirait une arête pendante, que `Version::apply` refuse.
/// Le test le vérifie sur une suite où les deux ordres ne sont pas interchangeables.
#[test]
fn un_diff_se_defait_dans_l_ordre_inverse() {
    let base = Version::root(
        &[id::<Agent>(1), id::<Agent>(2)],
        &[reviews(1, 2)],
        CoordinationMode::Blackboard,
        None,
        &ContentDigest,
    )
    .expect("fixture cohérente");

    // Retirer l'arête, puis le nœud qu'elle tenait : l'ordre imposé par le refus de la cascade.
    let forward = Diff::declaring(
        &base,
        vec![
            Operation::RemoveEdge(reviews(1, 2)),
            Operation::RemoveNode(id::<Agent>(1)),
        ],
        &ContentDigest,
    )
    .expect("les deux s'appliquent");
    let after = forward.replay(&base, &ContentDigest).expect("rejeu");

    let backward = forward
        .inverse(&after, &ContentDigest)
        .expect("les deux s'inversent");
    assert_eq!(
        backward.operations(),
        &[
            Operation::AddNode(id::<Agent>(1)),
            Operation::AddEdge(reviews(1, 2)),
        ],
        "le nœud revient avant l'arête, sans quoi l'arête pendrait"
    );

    let restored = backward.replay(&after, &ContentDigest).expect("rejeu");
    assert_eq!(restored.content_hash(), base.content_hash());
}

/// **Ce qui ne s'inverse pas le déclare** — ADR 0016 décision 5.
///
/// La fusion perd la partition : deux arêtes `X → premier` et `X → second` deviennent une seule, et
/// aucune scission ne saurait dire laquelle était laquelle. Rendre ici une scission plausible ferait
/// passer une **compensation** pour une **annulation**, et l'approbateur signerait sur un document
/// qui prétend rendre l'état d'avant. Le refus nomme l'opération, et l'appelant écrit lui-même ce
/// qu'il compense — lui seul sait ce qu'il veut compenser.
#[test]
fn une_fusion_refuse_de_s_inverser_en_se_nommant() {
    let base = Version::root(
        &[id::<Agent>(1), id::<Agent>(2), id::<Agent>(3)],
        &[],
        CoordinationMode::Blackboard,
        None,
        &ContentDigest,
    )
    .expect("fixture cohérente");

    let fusion = Operation::MergeNodes {
        first: id::<Agent>(1),
        second: id::<Agent>(2),
        into: id::<Agent>(9),
    };
    let forward = Diff::declaring(&base, vec![fusion.clone()], &ContentDigest).expect("s'applique");
    let after = forward.replay(&base, &ContentDigest).expect("rejeu");

    let refused = forward
        .inverse(&after, &ContentDigest)
        .expect_err("une fusion ne s'annule pas");
    assert_eq!(
        refused,
        DiffError::NotInvertible {
            position: 0,
            operation: fusion.canonical(),
        }
    );
    assert!(
        refused.to_string().contains("compensée"),
        "le refus doit dire ce qui reste possible : {refused}"
    );
    assert!(
        !refused.needs_rebase(),
        "rebaser n'y changerait rien : ce n'est pas un conflit de base"
    );
}

// ---------------------------------------------------------------------------------------------
// 2 — La justification cite un objet existant
// ---------------------------------------------------------------------------------------------

#[test]
fn une_justification_qui_cite_une_revision_inconnue_est_refusee() {
    let refused = Proposal::write(
        id::<DecisionKind>(1),
        Author::Human("usr-marie".to_owned()),
        Mode::Observed,
        18,
        diff(),
        Justification::new("barrier_encountered", revision(99)).expect("déclencheur non vide"),
        &index(),
    )
    .expect_err("la révision 99 n'existe pas");

    assert!(matches!(
        refused,
        ProposalError::UncitedJustification { .. }
    ));
    assert!(!refused.needs_rebase(), "rebaser n'y changerait rien");
}

#[test]
fn un_declencheur_vide_ne_justifie_rien() {
    assert_eq!(
        Justification::new("   ", revision(1)),
        Err(ProposalError::EmptyTrigger)
    );
}

/// Par **révision**, jamais par concept : §7.7 fait de `revision_id` l'identité d'une version
/// immuable, et citer un `stable_id` désignerait « la dernière version, quelle qu'elle soit » —
/// donc une justification qui change après coup.
#[test]
fn la_citation_designe_une_version_immuable() {
    let cited = justification();
    assert_eq!(cited.cites(), &revision(1));
    assert_ne!(revision(1), revision(2));
}

// ---------------------------------------------------------------------------------------------
// 3 — Rien ici ne touche à une mission émise
// ---------------------------------------------------------------------------------------------

/// La garantie se prouve **par absence**, et à deux niveaux.
///
/// D'abord la dépendance : ce crate ne connaît ni `locus-lep` ni `locus-graph`, donc il n'a aucun
/// type de mission sous la main — une `MissionEnvelope` ne peut pas être construite ici, encore
/// moins modifiée. Ensuite le texte : aucun fichier source ne nomme la mission ni la vue de
/// contexte, ce qui ferme la porte à une manipulation par chaîne de caractères.
///
/// Le test lit `Cargo.toml` et les sources, pas une liste recopiée : c'est ce qui le rend capable
/// de voir arriver la dépendance qu'il interdit.
#[test]
fn aucun_chemin_de_code_ne_touche_a_une_mission_emise() {
    use std::{fs, path::PathBuf};

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("le manifeste est lisible");
    for forbidden in ["locus-lep", "locus-graph"] {
        assert!(
            !manifest.contains(forbidden),
            "`{forbidden}` en dépendance donnerait à ce crate de quoi toucher à une mission"
        );
    }

    for entry in fs::read_dir(root.join("src")).expect("le répertoire des sources existe") {
        let path = entry.expect("entrée lisible").path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("source lisible");
        for forbidden in ["MissionEnvelope", "mission_envelope", "context_view"] {
            assert!(
                !source.contains(forbidden),
                "{} nomme « {forbidden} » : une modification de mission émise passerait par là",
                path.display()
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 4 — Le même chemin, et pas d'auto-approbation
// ---------------------------------------------------------------------------------------------

/// Décision 7 : « une proposition écrite par un agent est **le même objet** qu'une proposition
/// humaine et suit le même chemin ». Le test le vérifie en faisant parcourir aux deux la même
/// suite d'appels, et en comparant ce qui en sort.
#[test]
fn une_proposition_agentique_suit_le_meme_chemin_qu_une_humaine() {
    let from_agent = Proposal::write(
        id::<DecisionKind>(1),
        Author::Agent(id::<Agent>(7)),
        Mode::Assisted,
        18,
        diff(),
        justification(),
        &index(),
    )
    .expect("en mode assisted, un agent propose");

    let from_human = human_proposal(18);

    let agent_committed = commit(
        approve(
            from_agent.clone(),
            Author::Human("usr-gov".to_owned()),
            id::<Approval>(1),
        )
        .expect("un humain approuve"),
        18,
        &organisation(),
        &ContentDigest,
    )
    .expect("commitée");
    let human_committed = commit(
        approve(
            from_human,
            Author::Human("usr-gov".to_owned()),
            id::<Approval>(2),
        )
        .expect("approbateur distinct"),
        18,
        &organisation(),
        &ContentDigest,
    )
    .expect("commitée");

    assert_eq!(
        agent_committed.revision, human_committed.revision,
        "le même chemin produit le même effet : l'auteur ne change pas la mécanique"
    );
    assert_eq!(
        agent_committed.proposal.diff(),
        human_committed.proposal.diff()
    );
    assert_eq!(
        agent_committed.version.id(),
        human_committed.version.id(),
        "et la même version : le chemin est le même jusqu'au contenu produit"
    );
    assert_eq!(from_agent.author(), &Author::Agent(id::<Agent>(7)));
}

/// La borne qui ne se relâche dans aucun mode. §20.3 porte déjà `forbid_self_approval`, et c'est
/// ce qui empêche un agent de contrôler les règles décidant de son propre remplacement.
#[test]
fn un_proposeur_ne_peut_pas_approuver_sa_propre_proposition() {
    let agent = Author::Agent(id::<Agent>(7));
    let proposal = Proposal::write(
        id::<DecisionKind>(1),
        agent.clone(),
        Mode::Assisted,
        18,
        diff(),
        justification(),
        &index(),
    )
    .expect("mode assisted");

    assert_eq!(
        approve(proposal, agent, id::<Approval>(1)),
        Err(ProposalError::SelfApproval {
            author: format!("agent {}", id::<Agent>(7))
        })
    );

    // Et la borne vaut aussi pour un humain : ce n'est pas une méfiance envers les agents.
    let human = Author::Human("usr-marie".to_owned());
    assert!(matches!(
        approve(human_proposal(18), human, id::<Approval>(2)),
        Err(ProposalError::SelfApproval { .. })
    ));
}

/// Le défaut est `observed`, et c'est une exigence de §33 — « rendre toute action autonome sans
/// seuil humain » est un non-objectif explicite de la V1, pas une précaution.
#[test]
fn le_mode_par_defaut_interdit_a_un_agent_de_proposer() {
    assert_eq!(Mode::default(), Mode::Observed);

    let refused = Proposal::write(
        id::<DecisionKind>(1),
        Author::Agent(id::<Agent>(7)),
        Mode::Observed,
        18,
        diff(),
        justification(),
        &index(),
    )
    .expect_err("en observed, un agent signale mais ne propose pas");
    assert!(matches!(
        refused,
        ProposalError::NotAllowedToPropose {
            mode: Mode::Observed,
            ..
        }
    ));

    // Un humain propose sous tout mode : le mode borne ce que les agents peuvent faire, pas ce que
    // l'institution peut décider d'elle-même.
    assert!(Mode::Observed.allows(&Author::Human("usr-marie".to_owned())));
    assert!(Mode::Assisted.allows(&Author::Agent(id::<Agent>(7))));
}

/// Le refus de mode arrive **avant** la vérification de citation : un agent en `observed` ne doit
/// pas apprendre, par la nature du refus, quelles révisions existent.
#[test]
fn le_refus_de_mode_ne_revele_pas_ce_que_l_index_contient() {
    let refused = Proposal::write(
        id::<DecisionKind>(1),
        Author::Agent(id::<Agent>(7)),
        Mode::Observed,
        18,
        diff(),
        Justification::new("high_uncertainty", revision(99)).expect("déclencheur non vide"),
        &index(),
    )
    .expect_err("refusée");

    assert!(
        matches!(refused, ProposalError::NotAllowedToPropose { .. }),
        "la révision 99 est inconnue, et pourtant ce n'est pas ce que le refus dit : {refused}"
    );
}

// ---------------------------------------------------------------------------------------------
// L'énumération des sortes n'accueille que ce qu'un consommateur honore
// ---------------------------------------------------------------------------------------------

/// ADR 0016, décision 4 : « aucune sémantique inerte ». Une sorte de relation n'entre dans
/// l'énumération que lorsqu'un consommateur exécutable et testé existe.
///
/// **Ce test a fait son travail à l'arrivée de `visibility` (W15.e) : il a échoué.** Il liste donc
/// une valeur de plus, et chacune des deux se lit à côté du consommateur qui l'honore — `review`
/// pour l'indépendance de §14.4 (`region.rs`), `visibility` pour la construction de `ContextView`
/// (`visibility.rs`, éprouvé de bout en bout dans `tests/visibility.rs`). Élargir cette liste sans
/// écrire le consommateur à côté est exactement ce que la décision 4 interdit.
///
/// `role` n'y figure pas et n'y figurera pas : ce n'est pas une relation mais un champ
/// d'`AgentTemplate` (§7.1), et il revient comme l'opération attributaire `SET_ROLE`.
#[test]
fn une_sorte_de_relation_a_toujours_son_consommateur() {
    let declared: Vec<(&str, &str)> = RelationKind::ALL
        .into_iter()
        .map(|kind| {
            (
                kind.slug(),
                match kind {
                    RelationKind::Review => "region.rs — l'acyclicité de revue",
                    RelationKind::Visibility => "visibility.rs — la construction de ContextView",
                },
            )
        })
        .collect();
    assert_eq!(
        declared,
        vec![
            ("review", "region.rs — l'acyclicité de revue"),
            (
                "visibility",
                "visibility.rs — la construction de ContextView"
            ),
        ],
        "chaque sorte se lit à côté du consommateur qui l'honore"
    );
    for invented in [
        "mentors",
        "delegates_to",
        "supervises",
        "reports_to",
        "role",
    ] {
        assert_eq!(
            RelationKind::parse(invented),
            None,
            "« {invented} » n'a aucun consommateur exécutable"
        );
    }
}
