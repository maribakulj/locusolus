//! Test de sortie de W13.c, seconde moitié — **les cinq agrégats de §7.1 existent sous leur nom,
//! et les quatre nouveaux identifiants font un aller-retour.**
//!
//! `CLAUDE.md` : « les objets d'organisation, de coordination et de gouvernance sont ceux de
//! `SPEC_V1.md` §7.1, §13, §16, §20 et §22, **sous leur nom**. Aucun vocabulaire parallèle. » Les
//! énumérations reprennent donc mot pour mot les listes de la spec, et ce test les épingle une par
//! une plutôt qu'en les comparant à elles-mêmes.

use std::collections::BTreeSet;

use locus_coordination::{
    AgentError, AgentInstance, AgentTemplate, ApprovalRequest, ApprovalState, CoordinationMode,
    Decision, DecisionError, DecisionState, InstanceState, Team, TeamError, TemplateStatus,
};
use locus_protocol::{
    Id, Timestamp,
    id::{
        Agent, Branch,
        provisional::{Approval, Decision as DecisionKind, Task, Team as TeamKind},
    },
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn id<K: locus_protocol::IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn template() -> AgentTemplate {
    AgentTemplate::new(
        id(1),
        "Reviewer logique",
        "LogicalReviewer",
        3,
        TemplateStatus::Active,
    )
    .expect("template valide")
}

// ---------------------------------------------------------------------------------------------
// Les quatre identifiants
// ---------------------------------------------------------------------------------------------

/// L'aller-retour que l'item demande. Chaque identifiant se réécrit à l'identique, et **son
/// préfixe le distingue des autres** : `team_01…` et `task_01…` peuvent porter le même corps sans
/// désigner la même chose, et c'est le type qui empêche de les confondre à la compilation.
#[test]
fn les_quatre_identifiants_font_un_aller_retour() {
    let task: Id<Task> = id(1);
    let team: Id<TeamKind> = id(1);
    let decision: Id<DecisionKind> = id(1);
    let approval: Id<Approval> = id(1);

    for (rendered, prefix) in [
        (task.to_string(), "task_"),
        (team.to_string(), "team_"),
        (decision.to_string(), "dec_"),
        (approval.to_string(), "apr_"),
    ] {
        assert!(
            rendered.starts_with(prefix),
            "« {rendered} » devrait commencer par « {prefix} »"
        );
    }

    assert_eq!(Id::<Task>::parse(&task.to_string()), Ok(task));
    assert_eq!(Id::<TeamKind>::parse(&team.to_string()), Ok(team));
    assert_eq!(
        Id::<DecisionKind>::parse(&decision.to_string()),
        Ok(decision)
    );
    assert_eq!(Id::<Approval>::parse(&approval.to_string()), Ok(approval));
}

#[test]
fn un_identifiant_ne_se_relit_pas_sous_une_autre_nature() {
    let team: Id<TeamKind> = id(7);
    assert!(
        Id::<Task>::parse(&team.to_string()).is_err(),
        "un identifiant d'équipe lu comme identifiant de tâche désignerait un objet qui n'existe pas"
    );
}

// ---------------------------------------------------------------------------------------------
// AgentTemplate et AgentInstance — §14.2
// ---------------------------------------------------------------------------------------------

/// §7.1 : « l'identité d'un agent comprend le template, **sa version**, le modèle exact… ». Une
/// instance qui ne garderait que `template_id` changerait d'identité rétroactivement à chaque
/// révision du template, et une revue d'il y a six mois cesserait de dire ce qu'elle disait.
#[test]
fn une_instance_fige_la_version_du_template() {
    let instance = AgentInstance::provision(id(2), &template()).expect("template actif");
    assert_eq!(instance.template_version(), 3);

    let revised = AgentTemplate::new(
        id(1),
        "Reviewer logique",
        "LogicalReviewer",
        4,
        TemplateStatus::Active,
    )
    .expect("template valide");
    assert_eq!(
        instance.template_version(),
        3,
        "réviser le template ne change pas ce qu'une instance déjà provisionnée a employé"
    );
    assert_eq!(revised.version(), 4);
}

#[test]
fn un_template_desactive_ne_s_instancie_plus_mais_un_deprecie_si() {
    let disabled = AgentTemplate::new(id(1), "Ancien", "RedTeam", 1, TemplateStatus::Disabled)
        .expect("valide");
    assert_eq!(
        AgentInstance::provision(id(2), &disabled),
        Err(AgentError::TemplateNotInstantiable {
            status: TemplateStatus::Disabled
        })
    );

    // `deprecated` reste instanciable : §7.1 le distingue de `disabled`, et confondre les deux
    // arrêterait des campagnes en cours au lieu d'en décourager de nouvelles.
    let deprecated = AgentTemplate::new(id(1), "Ancien", "RedTeam", 1, TemplateStatus::Deprecated)
        .expect("valide");
    assert!(AgentInstance::provision(id(2), &deprecated).is_ok());
}

#[test]
fn le_groupe_d_independance_descend_du_template_a_l_instance() {
    // §14.4 : deux relecteurs du même groupe ne comptent pas comme indépendants. Si l'instance ne
    // portait pas le groupe, la vérification devrait remonter au template — donc à sa version
    // courante, donc à une réponse qui change avec le temps.
    let grouped = template().in_independence_group("logique");
    let instance = AgentInstance::provision(id(2), &grouped).expect("template actif");
    assert_eq!(instance.independence_group(), Some("logique"));
}

#[test]
fn une_instance_terminee_ne_se_ranime_pas() {
    let instance = AgentInstance::provision(id(2), &template())
        .expect("template actif")
        .moved_to(InstanceState::Active)
        .expect("provisionnée puis active")
        .moved_to(InstanceState::Completed)
        .expect("terminée");

    assert_eq!(
        instance.moved_to(InstanceState::Active),
        Err(AgentError::TerminalState {
            state: InstanceState::Completed
        }),
        "§14.2 : une instance est temporaire, et la ranimer effacerait la trace de sa fin"
    );
}

#[test]
fn les_etats_d_instance_sont_ceux_de_7_1() {
    let slugs: Vec<&str> = InstanceState::ALL
        .into_iter()
        .map(InstanceState::slug)
        .collect();
    assert_eq!(
        slugs,
        vec![
            "provisioned",
            "active",
            "waiting",
            "completed",
            "failed",
            "terminated"
        ]
    );
    assert_eq!(
        InstanceState::parse("terminated"),
        Some(InstanceState::Terminated)
    );
    assert_eq!(InstanceState::parse("stopped"), None);
}

#[test]
fn un_template_sans_role_ou_sans_version_est_refuse() {
    assert_eq!(
        AgentTemplate::new(id(1), "Nom", "  ", 1, TemplateStatus::Active),
        Err(AgentError::EmptyField { field: "role" })
    );
    assert_eq!(
        AgentTemplate::new(id(1), "Nom", "RedTeam", 0, TemplateStatus::Active),
        Err(AgentError::ZeroVersion)
    );
}

// ---------------------------------------------------------------------------------------------
// Team — §14.3
// ---------------------------------------------------------------------------------------------

#[test]
fn les_cinq_modes_de_coordination_sont_ceux_de_14_3() {
    let slugs: Vec<&str> = CoordinationMode::ALL
        .into_iter()
        .map(CoordinationMode::slug)
        .collect();
    assert_eq!(
        slugs,
        vec![
            "coordinator",
            "blackboard",
            "debate",
            "independent_pool",
            "pipeline"
        ],
        "§14.3 les dit obligatoires, et « le mode est enregistré et peut être comparé »"
    );
    assert_eq!(
        CoordinationMode::parse("debate"),
        Some(CoordinationMode::Debate)
    );
    assert_eq!(
        CoordinationMode::parse("consensus"),
        None,
        "un mode inconnu rabattu sur un mode connu fausserait la comparaison de §14.3"
    );
}

/// L'invariant 11, lu dans le mode. `independent_pool` est le seul qui interdise tout partage
/// avant remise, et le dire une fois évite qu'on récrive la condition ailleurs à l'envers.
#[test]
fn un_seul_mode_retient_le_partage_avant_remise() {
    let withholding: Vec<&str> = CoordinationMode::ALL
        .into_iter()
        .filter(|mode| mode.withholds_sharing())
        .map(CoordinationMode::slug)
        .collect();
    assert_eq!(withholding, vec!["independent_pool"]);
}

#[test]
fn le_mode_coordinator_exige_un_coordinateur_membre() {
    let members: BTreeSet<Id<Agent>> = BTreeSet::from([id(1), id(2)]);

    assert_eq!(
        Team::new(
            id(9),
            id::<Branch>(1),
            "Revue",
            CoordinationMode::Coordinator,
            members.clone(),
            None
        ),
        Err(TeamError::CoordinatorRequired)
    );

    assert_eq!(
        Team::new(
            id(9),
            id::<Branch>(1),
            "Revue",
            CoordinationMode::Coordinator,
            members.clone(),
            Some(id(42))
        ),
        Err(TeamError::CoordinatorNotAMember)
    );

    let team = Team::new(
        id(9),
        id::<Branch>(1),
        "Revue",
        CoordinationMode::Coordinator,
        members,
        Some(id(1)),
    )
    .expect("équipe valide");
    assert_eq!(team.mode(), CoordinationMode::Coordinator);
    assert_eq!(team.members().len(), 2);
    assert!(team.shares_before_delivery());
}

#[test]
fn une_equipe_sans_membre_ne_coordonne_rien() {
    assert_eq!(
        Team::new(
            id(9),
            id::<Branch>(1),
            "Vide",
            CoordinationMode::Debate,
            BTreeSet::new(),
            None
        ),
        Err(TeamError::NoMembers)
    );
}

#[test]
fn un_pool_independant_ne_partage_pas_avant_remise() {
    let team = Team::new(
        id(9),
        id::<Branch>(1),
        "Relecture croisée",
        CoordinationMode::IndependentPool,
        BTreeSet::from([id(1), id(2), id(3)]),
        None,
    )
    .expect("équipe valide");
    assert!(!team.shares_before_delivery());
}

// ---------------------------------------------------------------------------------------------
// Decision et ApprovalRequest — §7.1, §20
// ---------------------------------------------------------------------------------------------

#[test]
fn une_decision_sans_justification_est_refusee() {
    // §20 fait de la décision l'objet que la gouvernance relit. Sans justification, elle consigne
    // qu'un choix a eu lieu, jamais pourquoi — c'est-à-dire exactement ce qui manque six mois plus
    // tard.
    assert_eq!(
        Decision::propose(id(4), "budget_increase", "   ", "governor-1"),
        Err(DecisionError::EmptyField { field: "rationale" })
    );
}

#[test]
fn une_decision_approuvee_se_revoque_et_une_rejetee_non() {
    let approved = Decision::propose(
        id(4),
        "budget_increase",
        "le portefeuille le soutient",
        "gov",
    )
    .expect("décision valide")
    .moved_to(DecisionState::Approved)
    .expect("approuvée");
    assert!(approved.clone().moved_to(DecisionState::Revoked).is_ok());

    let rejected = Decision::propose(id(4), "budget_increase", "trop coûteux", "gov")
        .expect("décision valide")
        .moved_to(DecisionState::Rejected)
        .expect("rejetée");
    assert_eq!(
        rejected.moved_to(DecisionState::Revoked),
        Err(DecisionError::Forbidden {
            from: DecisionState::Rejected,
            to: DecisionState::Revoked
        }),
        "il n'y a rien à défaire dans un rejet"
    );

    // Et une révocation ne ramène pas à `proposed` : la trace de l'approbation reste (invariant 12).
    let revoked = approved.moved_to(DecisionState::Revoked).expect("révoquée");
    assert_eq!(
        revoked.moved_to(DecisionState::Proposed),
        Err(DecisionError::Forbidden {
            from: DecisionState::Revoked,
            to: DecisionState::Proposed
        })
    );
}

#[test]
fn les_etats_de_decision_et_d_approbation_sont_ceux_de_7_1() {
    assert_eq!(
        DecisionState::ALL
            .into_iter()
            .map(DecisionState::slug)
            .collect::<Vec<_>>(),
        vec!["proposed", "approved", "rejected", "revoked"]
    );
    assert_eq!(
        ApprovalState::ALL
            .into_iter()
            .map(ApprovalState::slug)
            .collect::<Vec<_>>(),
        vec!["pending", "approved", "rejected", "expired", "cancelled"]
    );
}

/// « Suspendre **durablement** » (§7.1) suppose que quelqu'un puisse reprendre. Une demande que
/// personne n'est désigné pour trancher ne suspend pas, elle enterre.
#[test]
fn une_demande_que_personne_ne_peut_trancher_est_refusee() {
    assert_eq!(
        ApprovalRequest::request(
            id(5),
            "supprimer la branche",
            "irréversible",
            "agent-7",
            vec![]
        ),
        Err(DecisionError::NoRequiredRoles)
    );
    assert_eq!(
        ApprovalRequest::request(
            id(5),
            "supprimer la branche",
            "irréversible",
            "agent-7",
            vec!["  ".to_owned()]
        ),
        Err(DecisionError::NoRequiredRoles)
    );
}

#[test]
fn une_demande_deja_tranchee_ne_se_retranche_pas() {
    let answered = ApprovalRequest::request(
        id(5),
        "supprimer la branche",
        "irréversible",
        "agent-7",
        vec!["Governor".to_owned()],
    )
    .expect("demande valide")
    .answered(ApprovalState::Approved)
    .expect("approuvée");

    assert_eq!(
        answered.answered(ApprovalState::Rejected),
        Err(DecisionError::AlreadyAnswered {
            state: ApprovalState::Approved
        }),
        "la seconde réponse écraserait la première, et c'est la première qui a débloqué le workflow"
    );
}
