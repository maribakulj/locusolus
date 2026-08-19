//! Le test de sortie de `W20.a` — `SPEC_V1.md` §22.2 et §22.5.

use locus_protocol::id::{Agent, Command, Workspace};
use locus_protocol::{Id, IdKind, Timestamp};
use locusd::{
    Accepted, CommandEnvelope, CommandError, Conflict, Draft, Family, Outcome, ResourceRef,
    Revision,
};

fn id<K: IdKind>(seed: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = seed;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn envelope() -> CommandEnvelope {
    CommandEnvelope::mutating(
        id::<Command>(1),
        "branch.fork",
        id::<Workspace>(2),
        id::<Agent>(3),
        "idem-1",
        Revision::new(18),
    )
    .expect("commande bien formée")
}

fn draft() -> Draft {
    Draft::new()
        .command_id(id::<Command>(1))
        .command_type("branch.fork")
        .workspace_id(id::<Workspace>(2))
        .actor_principal_id(id::<Agent>(3))
        .idempotency_key("idem-1")
}

// ---------------------------------------------------------------------------------------------
// 1. Une commande mutante sans `expected_revision`
// ---------------------------------------------------------------------------------------------

/// **La seconde porte : le refus nomme le champ.**
///
/// Un décodeur, une CLI, un client assemblent une commande morceau par morceau et ne peuvent pas
/// être tenus par une signature. Un refus générique les enverrait relire la documentation ; celui-ci
/// dit lequel manque.
#[test]
fn un_brouillon_mutant_sans_revision_est_refuse_par_le_nom_du_champ() {
    let erreur = draft().seal().expect_err("il manque la révision");
    assert_eq!(
        erreur,
        CommandError::Validation {
            field: "expected_revision".to_owned(),
            detail: "manquant".to_owned(),
        }
    );
    assert_eq!(erreur.family(), Family::Validation);
}

/// L'ordre des champs manquants est celui de §22.2, pas celui de la découverte.
///
/// Deux clients auxquels il manque les mêmes champs reçoivent le même message, et un message stable
/// se cite dans un rapport de bug. Un ordre qui dépendrait de la construction du brouillon rendrait
/// deux messages pour un seul défaut.
#[test]
fn le_champ_nomme_est_le_premier_de_la_spec() {
    let vide = Draft::new().seal().expect_err("tout manque");
    assert_eq!(
        vide,
        CommandError::Validation {
            field: "command_id".to_owned(),
            detail: "manquant".to_owned(),
        }
    );

    let sans_cle = Draft::new()
        .command_id(id::<Command>(1))
        .command_type("branch.fork")
        .workspace_id(id::<Workspace>(2))
        .actor_principal_id(id::<Agent>(3))
        .expected_revision(Revision::new(1))
        .seal()
        .expect_err("il manque la clé");
    assert_eq!(
        sans_cle,
        CommandError::Validation {
            field: "idempotency_key".to_owned(),
            detail: "manquant".to_owned(),
        }
    );
}

/// Un champ présent mais vide est refusé par son nom aussi.
#[test]
fn un_champ_blanc_est_refuse_par_son_nom() {
    for (field, brouillon) in [
        ("command_type", draft().command_type("  ")),
        ("idempotency_key", draft().idempotency_key("")),
    ] {
        let erreur = brouillon
            .expected_revision(Revision::new(1))
            .seal()
            .expect_err("un champ blanc n'est pas un champ");
        assert_eq!(
            erreur,
            CommandError::Validation {
                field: field.to_owned(),
                detail: "vide".to_owned(),
            }
        );
    }
}

/// La révision voyage jusqu'à l'enveloppe, et le brouillon complet passe.
#[test]
fn un_brouillon_complet_scelle_une_commande() {
    let commande = draft()
        .expected_revision(Revision::new(18))
        .seal()
        .expect("brouillon complet");
    assert_eq!(commande.expected_revision(), Revision::new(18));
    assert_eq!(commande.command_type(), "branch.fork");
    assert_eq!(commande.schema_version(), CommandEnvelope::SCHEMA_VERSION);
    assert_eq!(commande.delegation_id(), None);
    assert_eq!(commande.correlation_id(), None);
}

// ---------------------------------------------------------------------------------------------
// 2. Les huit familles, liste close
// ---------------------------------------------------------------------------------------------

/// **Huit, sous les noms de §22.5, et une neuvième n'existe pas.**
///
/// Le test échouera le jour où quelqu'un en ajoutera une, et c'est ce qu'on lui demande : une
/// famille de plus veut dire qu'un client doit apprendre une réaction nouvelle, ce qui est une
/// rupture de contrat et pas un détail d'implémentation.
#[test]
fn les_huit_familles_sont_une_liste_close() {
    assert_eq!(Family::NAMES.len(), 8);
    assert_eq!(Family::ALL.len(), 8);
    assert_eq!(
        Family::ALL.map(Family::name).to_vec(),
        Family::NAMES.to_vec(),
        "les deux listes disent la même chose, dans le même ordre"
    );
    assert_eq!(
        Family::NAMES.to_vec(),
        vec![
            "validation",
            "authorization",
            "conflict",
            "unavailable",
            "budget",
            "policy",
            "security",
            "internal"
        ],
        "les noms de §22.5, mot pour mot"
    );

    // Les catégories de `locus_protocol` sont un autre vocabulaire, pour un autre lecteur. Celles
    // qui n'ont rien à faire dans une réponse de commande sont nommées ici pour que la confusion
    // échoue si quelqu'un les fusionne.
    for etranger in ["sandbox", "model", "tool", "lease", "network", "protocol"] {
        assert!(
            !Family::NAMES.contains(&etranger),
            "« {etranger} » vient de la spec Canterel §26 : un client d'API n'a aucune réaction à lui opposer"
        );
    }
}

/// Le rang de chaque famille dans [`Family::ALL`], par un `match` **exhaustif**.
///
/// C'est lui qui attache l'énumération à `ALL`, et le détour n'est pas décoratif : `ALL` est une
/// liste écrite à la main, qu'une variante nouvelle laisse intacte. `ALL.len() == 8` resterait donc
/// vrai avec neuf familles, et l'assertion de la liste close serait verte à côté de la plaque —
/// c'est ce qu'un mutant a montré. Une neuvième variante rend ce `match` non exhaustif : le crate
/// de test cesse de compiler, et il faut passer ici pour la faire entrer.
fn rang(famille: Family) -> usize {
    match famille {
        Family::Validation => 0,
        Family::Authorization => 1,
        Family::Conflict => 2,
        Family::Unavailable => 3,
        Family::Budget => 4,
        Family::Policy => 5,
        Family::Security => 6,
        Family::Internal => 7,
    }
}

/// **Une neuvième famille ne peut pas entrer en silence**, par deux chemins distincts.
///
/// Ils n'attrapent pas la même chose, et aucun des deux ne suffit :
///
/// - une famille au **nom du vocabulaire Canterel** — `sandbox`, `lease` — est refusée par le
///   décodeur, pas seulement absente de `NAMES`. C'est la fusion que le module dit tentante, et un
///   test de non-appartenance à `NAMES` la manquerait si la variante existait vraiment ;
/// - une famille au **nom neuf** est attrapée par [`rang`], à la compilation.
#[test]
fn une_neuvieme_famille_ne_peut_pas_entrer_en_silence() {
    for (position, famille) in Family::ALL.iter().enumerate() {
        assert_eq!(rang(*famille), position, "{famille} n'est pas à son rang");
    }

    for nom in Family::NAMES {
        let relu: Family = serde_json::from_value(serde_json::Value::String(nom.to_owned()))
            .unwrap_or_else(|_| panic!("« {nom} » est une des huit et doit se relire"));
        assert_eq!(relu.name(), nom);
    }

    for etranger in ["sandbox", "model", "tool", "lease", "network", "protocol"] {
        let relu: Result<Family, _> =
            serde_json::from_value(serde_json::Value::String(etranger.to_owned()));
        assert!(
            relu.is_err(),
            "« {etranger} » vient de la spec Canterel §26 : le décodeur doit le refuser, pas le porter"
        );
    }
}

/// Chaque variante d'erreur rend sa famille, et deux variantes n'en partagent pas une.
#[test]
fn chaque_variante_rend_sa_famille() {
    let toutes = [
        CommandError::Validation {
            field: "f".to_owned(),
            detail: "d".to_owned(),
        },
        CommandError::Authorization {
            action: "branch.fork".to_owned(),
        },
        CommandError::Conflict(conflit()),
        CommandError::Unavailable {
            detail: "d".to_owned(),
        },
        CommandError::Budget {
            budget: "b".to_owned(),
            detail: "d".to_owned(),
        },
        CommandError::Policy {
            policy: "p".to_owned(),
            detail: "d".to_owned(),
        },
        CommandError::Security {
            detail: "d".to_owned(),
        },
        CommandError::Internal {
            detail: "d".to_owned(),
        },
    ];

    let familles: Vec<Family> = toutes.iter().map(CommandError::family).collect();
    assert_eq!(familles, Family::ALL.to_vec());

    let mut uniques = familles.clone();
    uniques.sort_unstable();
    uniques.dedup();
    assert_eq!(uniques.len(), 8, "deux variantes partagent une famille");
}

// ---------------------------------------------------------------------------------------------
// 3. Un conflit rend l'état courant, jamais un entier nu
// ---------------------------------------------------------------------------------------------

fn conflit() -> Conflict {
    Conflict {
        expected: Revision::new(18),
        current: Revision::new(21),
        resource: ResourceRef::new("/branches/br_01").expect("chemin non vide"),
    }
}

/// **Les deux révisions et la ressource**, parce qu'un client qui doit relire pour retenter a
/// besoin de savoir quoi relire.
///
/// Un `409` nu, ou même la seule révision courante, l'obligerait à deviner quelle collection
/// interroger — c'est-à-dire à reconstruire la table que le serveur possède déjà.
#[test]
fn un_conflit_dit_ce_qu_il_faut_relire() {
    let erreur = CommandError::Conflict(conflit());
    let CommandError::Conflict(porte) = &erreur else {
        panic!("la variante porte un conflit");
    };
    assert_eq!(porte.expected, Revision::new(18));
    assert_eq!(porte.current, Revision::new(21));
    assert_eq!(porte.resource.path(), "/branches/br_01");
    assert_ne!(
        porte.expected, porte.current,
        "sinon ce n'est pas un conflit"
    );

    // Le message porte les trois, **dans l'ordre où on les lit**. Vérifier seulement que les deux
    // nombres sont présents laisserait passer « attendu 21, courante 18 » — le message exactement
    // faux que le type `Revision` est censé rendre impossible. Un test de présence aurait été vert
    // sur cette inversion ; celui-ci la refuse par les positions.
    let rendu = erreur.to_string();
    let attendu = rendu
        .find("18")
        .expect("la révision attendue est dans le message");
    let courante = rendu
        .find("21")
        .expect("la révision courante est dans le message");
    assert!(
        attendu < courante,
        "« {rendu} » : la révision attendue se lit avant la courante, sinon le message dit l'inverse de ce qui est"
    );
    assert!(rendu.contains("/branches/br_01"));
}

/// Un conflit sans ressource n'a pas rendu l'état courant : il a seulement dit non.
#[test]
fn une_ressource_vide_est_refusee() {
    for blanc in ["", "   ", "\t"] {
        assert!(ResourceRef::new(blanc).is_err(), "{blanc:?}");
    }
}

/// Les deux révisions ne se confondent pas par leur position.
///
/// `Revision` est un type à part et non un `u64` nu : une inversion dans une signature à deux
/// entiers produirait un message exactement faux — « attendu 21, courant 18 » là où c'est
/// l'inverse — et rien ne la rattraperait.
#[test]
fn les_deux_revisions_sont_nommees_pas_positionnelles() {
    let conflit = Conflict {
        expected: Revision::new(1),
        current: Revision::new(2),
        resource: ResourceRef::new("/tasks/tsk_01").expect("chemin"),
    };
    assert_eq!(conflit.expected.get(), 1);
    assert_eq!(conflit.current.get(), 2);
}

// ---------------------------------------------------------------------------------------------
// 4. Aucun refus ne ressemble à un succès
// ---------------------------------------------------------------------------------------------

/// **Un refus interrogé sur son succès rend une absence, jamais une valeur.**
///
/// Trois façons de perdre la propriété, écartées : un `Default` qui produirait un succès sans
/// commande, une variante « accepté avec réserves » qu'un appelant lirait comme un accord, et un
/// accesseur qui inventerait une révision. Les trois sont testées ou inexprimables.
#[test]
fn un_refus_ne_ressemble_pas_a_un_succes() {
    let refus = Outcome::Refused(CommandError::Conflict(conflit()));
    assert_eq!(refus.accepted(), None, "aucune révision inventée");
    assert!(refus.refused().is_some());

    let succes = Outcome::Accepted(Accepted {
        revision: Revision::new(19),
    });
    assert_eq!(succes.refused(), None);
    assert_eq!(
        succes.accepted().map(|accepted| accepted.revision),
        Some(Revision::new(19))
    );
}

/// Le verdict porte son étiquette sur le fil, et les deux ne se ressemblent pas.
///
/// Un encodage qui rendrait `{"revision": 19}` pour un succès et `{"family": "conflict"}` pour un
/// refus laisserait un lecteur distraits confondre l'absence d'erreur avec un accord. L'étiquette
/// `outcome` est présente dans les deux cas.
#[test]
fn le_verdict_porte_son_etiquette_dans_les_deux_cas() {
    let succes = serde_json::to_value(Outcome::Accepted(Accepted {
        revision: Revision::new(19),
    }))
    .expect("encodage");
    let refus = serde_json::to_value(Outcome::Refused(CommandError::Security {
        detail: "clé révoquée".to_owned(),
    }))
    .expect("encodage");

    assert_eq!(succes["outcome"], "accepted");
    assert_eq!(refus["outcome"], "refused");
    assert_eq!(refus["family"], "security");
    assert_eq!(succes.get("family"), None);
}

/// `Outcome` n'a pas de valeur par défaut, et ne peut pas en avoir.
///
/// Vérifié par lecture du source, comme `W18.a` : un `impl Default for Outcome` produirait un
/// succès obtenu sans commande, et `#[derive(Default)]` sur l'énumération le ferait en une ligne.
#[test]
fn aucun_defaut_ne_produit_un_verdict() {
    let source = include_str!("../src/outcome.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for interdit in ["Default", "unwrap_or", "is_ok", "panic!"] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ouvrirait un chemin où un refus rendrait une valeur"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 5. La forme sur le fil
// ---------------------------------------------------------------------------------------------

/// L'enveloppe fait l'aller-retour, et les champs facultatifs absents ne reviennent pas en `null`.
#[test]
fn l_enveloppe_fait_l_aller_retour() {
    let commande = envelope();
    let encode = serde_json::to_value(&commande).expect("encodage");
    assert_eq!(encode["schema_version"], 1);
    assert_eq!(encode["expected_revision"], 18);
    assert_eq!(encode.get("delegation_id"), None, "absent, pas null");
    assert_eq!(encode.get("correlation_id"), None);

    let relu: CommandEnvelope = serde_json::from_value(encode.clone()).expect("décodage");
    assert_eq!(relu, commande);
    assert_eq!(serde_json::to_value(&relu).expect("ré-encodage"), encode);
}
