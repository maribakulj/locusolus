//! Test de sortie de `W5.ag` — ADR 0035 décision 3, ADR 0036 décision 1.
//!
//! **Une attestation ne vaut pour un worker que si le mécanisme est un des siens, et le refus dit
//! laquelle des deux raisons l'écarte.**
//!
//! # Ce que l'item a trouvé en se faisant
//!
//! `W5.ae` avait rendu `backend` obligatoire dans l'enregistrement, et la roadmap tenait `W5.ag`
//! pour un arbitrage de vocabulaire — « comparer deux `backend` demande qu'ils viennent du même
//! vocabulaire ». En mesurant, il manquait d'abord autre chose : `Proven::standing` rendait un
//! [`Standing`] **nu**, qui ne porte que le niveau. Le site de placement n'avait donc rien à
//! comparer, quelle qu'ait été la table qu'on lui aurait donnée. Le mécanisme voyage maintenant
//! avec le verdict, et `attestation.rs` a son propre test pour ça.
//!
//! # Les trois verdicts, et pourquoi trois
//!
//! C'est la forme de `locus_lep::negotiate`, et pour la même raison : fondre « ce n'est pas le même
//! mécanisme » et « je ne sais pas ce que ce nom désigne » rendrait un émetteur mal orthographié
//! indiscernable d'un émetteur légitime — et les deux se réparent différemment, l'un en lançant une
//! **autre** campagne, l'autre en nommant un mécanisme.

use locus_execd::{
    Attested, Candidate, Employment, HostCapabilities, Placement, RefusalReason, employment, place,
};
use locus_execution::{
    Mount, NetworkMode, ResourceSpec, SandboxLevel, SandboxProfile, SandboxSpec, Standing,
};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// Deux mécanismes **au registre**, et l'ADR 0035 les tient pour incomparables.
const BUBBLEWRAP: &str = "bubblewrap";
const PODMAN: &str = "podman-rootless";
/// Un nom qu'aucun émetteur n'écrit et que le registre ne connaît pas.
const INCONNU: &str = "mecanisme-que-personne-n-a-decrit";

fn hote(best: SandboxLevel, mecanisme: Option<&str>) -> HostCapabilities {
    let annonce = HostCapabilities::new(
        best,
        ResourceSpec::new(8_000, 32 << 30, 4_096, 1 << 40, 86_400).expect("quotas non nuls"),
        vec!["deny", "connector_only", "allowlist", "full"],
    );
    match mecanisme {
        Some(mecanisme) => annonce.employing(mecanisme),
        None => annonce,
    }
}

fn mission(level: SandboxLevel) -> SandboxSpec {
    SandboxSpec::new(
        level,
        SandboxProfile::UntrustedRepository,
        NetworkMode::Deny,
        Vec::<Mount>::new(),
        ResourceSpec::new(1_000, 1 << 30, 64, 0, 300).expect("quotas non nuls"),
    )
    .expect("spécification valide")
}

fn atteste(backend: &str, level: SandboxLevel) -> Attested {
    Attested {
        backend: backend.to_owned(),
        standing: Standing::Trusted { level },
    }
}

/// Les motifs qu'un placement refusé porte pour son unique candidat.
fn motifs(spec: &SandboxSpec, candidat: Candidate) -> Vec<RefusalReason> {
    match place(spec, &[candidat]) {
        Placement::Refused { shortfalls } => {
            assert_eq!(shortfalls.len(), 1, "un manque par candidat soumis");
            shortfalls
                .into_iter()
                .next()
                .expect("le manque du seul candidat")
                .1
        }
        Placement::Placed { worker, level } => {
            panic!("ce candidat ne devait pas être placé : « {worker} » en {level:?}")
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Les trois verdicts du rapprochement
// ---------------------------------------------------------------------------------------------

/// **Le même mécanisme des deux côtés se rapproche.**
///
/// Le pendant positif : un rapprochement qui refuserait tout serait exact et inutile.
#[test]
fn le_meme_mecanisme_des_deux_cotes_se_rapproche() {
    assert_eq!(
        employment(BUBBLEWRAP, Some(BUBBLEWRAP)),
        Employment::Employed
    );
}

/// **Deux mécanismes connus et différents ne se rapprochent pas, et c'est `Foreign`.**
///
/// L'ADR 0035 les tient pour incomparables faute de les avoir mesurés l'un contre l'autre, et sa
/// question ouverte le dit : décider qu'un mécanisme en « couvre » un autre demanderait de mesurer
/// les deux contre les seize sondes, ce qui n'a pas été fait.
#[test]
fn deux_mecanismes_connus_et_differents_sont_etrangers() {
    assert_eq!(employment(PODMAN, Some(BUBBLEWRAP)), Employment::Foreign);
    assert_eq!(employment(BUBBLEWRAP, Some(PODMAN)), Employment::Foreign);
}

/// **Un nom hors registre ne rend pas `Foreign` : il rend `Unresolved`, et il se nomme.**
///
/// C'est toute la différence que le registre achète. Sans lui, l'égalité de chaînes dirait « pas le
/// même mécanisme » d'un nom mal orthographié comme d'un mécanisme réellement différent, et
/// l'exploitant relancerait des campagnes pour un problème de registre.
#[test]
fn un_nom_hors_registre_ne_se_rapproche_pas_et_se_nomme() {
    assert_eq!(
        employment(INCONNU, Some(BUBBLEWRAP)),
        Employment::Unresolved {
            unregistered: vec![INCONNU.to_owned()]
        }
    );
    assert_eq!(
        employment(BUBBLEWRAP, Some(INCONNU)),
        Employment::Unresolved {
            unregistered: vec![INCONNU.to_owned()]
        }
    );
}

/// **Deux noms hors registre mais égaux se rapprochent quand même.**
///
/// Le registre sert à distinguer les deux façons de dire non, pas à autoriser le oui. Refuser un
/// mécanisme attesté à un worker qui annonce **le même nom** obligerait chaque déploiement tiers à
/// modifier un fichier de ce dépôt pour placer quoi que ce soit, et ne protégerait de rien : ce que
/// l'ADR 0035 interdit est de rapprocher deux mécanismes **distincts**, pas un nom de lui-même.
#[test]
fn deux_noms_hors_registre_mais_egaux_se_rapprochent() {
    assert_eq!(employment(INCONNU, Some(INCONNU)), Employment::Employed);
}

/// **Un manifeste qui ne nomme aucun mécanisme ne rapproche rien, et la liste reste vide.**
///
/// `backend` est facultatif dans `CapabilityManifestSandbox` et obligatoire dans
/// `SandboxAttestation` — une asymétrie des schémas gelés, trouvée en les lisant. Le troisième terme
/// de la décision 3 ne se vérifie donc pas, et `unregistered` reste **vide** : le nom manque au
/// manifeste, et l'ajouter au registre ne réparerait rien.
#[test]
fn un_manifeste_sans_mecanisme_ne_rapproche_rien() {
    assert_eq!(
        employment(BUBBLEWRAP, None),
        Employment::Unresolved {
            unregistered: Vec::new()
        }
    );
    assert_eq!(
        employment(INCONNU, None),
        Employment::Unresolved {
            unregistered: Vec::new()
        }
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Ce que le placement en fait
// ---------------------------------------------------------------------------------------------

/// **Une preuve sous le mécanisme employé place, comme avant.**
#[test]
fn une_preuve_sous_le_mecanisme_employe_place() {
    let candidat = Candidate::new("worker-bwrap", hote(SandboxLevel::S3, Some(BUBBLEWRAP)))
        .attested(atteste(BUBBLEWRAP, SandboxLevel::S2));

    assert_eq!(
        place(&mission(SandboxLevel::S2), &[candidat]),
        Placement::Placed {
            worker: "worker-bwrap".to_owned(),
            level: SandboxLevel::S2,
        }
    );
}

/// **Une preuve sous un autre mécanisme n'est pas une preuve pour ce worker — et le refus le dit
/// sous son propre motif.**
///
/// Le test de sortie que `W5.ae` annonçait. C'est le cas mesuré par l'ADR 0035 : sur un runner de
/// CI, `locus-execd` prouve `S2` sous podman pendant que le worker, faute de `bwrap`, n'annonce que
/// `bubblewrap`. Sans ce motif, le refus dirait « aucune campagne n'a conclu » d'une campagne qui a
/// conclu — et enverrait la relancer indéfiniment au lieu d'en lancer une **autre**.
#[test]
fn une_preuve_sous_un_autre_mecanisme_est_ecartee_sous_son_propre_motif() {
    let candidat = Candidate::new("worker-bwrap", hote(SandboxLevel::S3, Some(BUBBLEWRAP)))
        .attested(atteste(PODMAN, SandboxLevel::S3));

    let motifs = motifs(&mission(SandboxLevel::S2), candidat);

    assert_eq!(
        motifs,
        vec![RefusalReason::MechanismNotEmployed {
            required: SandboxLevel::S2,
            employs: BUBBLEWRAP.to_owned(),
            attested: vec![PODMAN.to_owned()],
        }],
        "un seul motif : le niveau **a** été prouvé, sous un mécanisme qui n'est pas le sien"
    );
}

/// **`level_not_attested` ne s'ajoute pas quand la preuve écartée atteignait le niveau.**
///
/// La propriété que le test précédent tient par son `assert_eq!` complet, isolée ici parce qu'elle
/// est facile à casser sans s'en apercevoir : « l'hôte annonce ce niveau et ne l'a jamais prouvé »
/// serait **faux**, et les deux phrases côte à côte enverraient l'exploitant à deux endroits dont un
/// seul est le bon.
#[test]
fn un_mecanisme_etranger_ne_dit_pas_aussi_que_rien_n_a_ete_prouve() {
    let candidat = Candidate::new("worker-bwrap", hote(SandboxLevel::S3, Some(BUBBLEWRAP)))
        .attested(atteste(PODMAN, SandboxLevel::S3));

    assert!(
        !motifs(&mission(SandboxLevel::S2), candidat)
            .iter()
            .any(|motif| matches!(motif, RefusalReason::LevelNotAttested { .. }))
    );
}

/// **Quand la preuve écartée n'atteignait pas non plus le niveau, les deux motifs sont là.**
///
/// Le symétrique du précédent, et il compte autant : `admission-refusal.schema.json` demande
/// **toutes** les conditions manquantes, jamais la première seule. Ici il manque deux choses — le
/// mécanisme ne correspond pas, *et* aucune campagne employée n'atteint le niveau — et n'en dire
/// qu'une ferait corriger l'une pour retomber aussitôt sur l'autre.
#[test]
fn un_mecanisme_etranger_et_une_preuve_trop_basse_donnent_les_deux_motifs() {
    let candidat = Candidate::new("worker-bwrap", hote(SandboxLevel::S3, Some(BUBBLEWRAP)))
        .attested(atteste(BUBBLEWRAP, SandboxLevel::S1))
        .attested(atteste(PODMAN, SandboxLevel::S2));

    let motifs = motifs(&mission(SandboxLevel::S3), candidat);

    assert!(motifs.contains(&RefusalReason::MechanismNotEmployed {
        required: SandboxLevel::S3,
        employs: BUBBLEWRAP.to_owned(),
        attested: vec![PODMAN.to_owned()],
    }));
    assert!(
        motifs.contains(&RefusalReason::LevelNotAttested {
            required: SandboxLevel::S3,
            proven: Some(SandboxLevel::S1),
        }),
        "le niveau prouvé rendu est celui du mécanisme **employé**, pas le meilleur de tous : \
         {motifs:?}"
    );
}

/// **Un nom hors registre refuse sous `mechanism_unresolved`, en nommant le nom fautif.**
#[test]
fn un_nom_hors_registre_refuse_en_nommant_le_nom() {
    let candidat = Candidate::new("worker-obscur", hote(SandboxLevel::S3, Some(INCONNU)))
        .attested(atteste(BUBBLEWRAP, SandboxLevel::S3));

    let motifs = motifs(&mission(SandboxLevel::S2), candidat);

    assert_eq!(
        motifs,
        vec![RefusalReason::MechanismUnresolved {
            required: SandboxLevel::S2,
            employs: Some(INCONNU.to_owned()),
            unregistered: vec![INCONNU.to_owned()],
        }]
    );
}

/// **Un manifeste sans mécanisme refuse aussi, et `employs` absent est l'information.**
///
/// Conséquence assumée, trouvée en livrant : un worker qui n'annonce pas son `backend` ne peut plus
/// tirer d'aucune attestation. C'est strictement la décision 3 — le troisième terme ne se vérifie
/// pas — et le refus dit quoi faire, ce que le silence d'avant ne faisait pas.
#[test]
fn un_manifeste_sans_mecanisme_refuse_et_le_dit() {
    let candidat = Candidate::new("worker-muet", hote(SandboxLevel::S3, None))
        .attested(atteste(BUBBLEWRAP, SandboxLevel::S3));

    let motifs = motifs(&mission(SandboxLevel::S2), candidat);

    assert_eq!(
        motifs,
        vec![RefusalReason::MechanismUnresolved {
            required: SandboxLevel::S2,
            employs: None,
            unregistered: Vec::new(),
        }]
    );
}

/// **Sous `S0`, aucun rapprochement n'est demandé.**
///
/// L'attestation n'entre au placement qu'au-dessus de `S0`, et le rapprochement n'y entre pas non
/// plus : un worker qui n'annonce pas son mécanisme continue de recevoir ce qui n'exige rien.
#[test]
fn sous_s0_le_rapprochement_ne_refuse_rien() {
    let candidat = Candidate::new("worker-muet", hote(SandboxLevel::S3, None))
        .attested(atteste(PODMAN, SandboxLevel::S3));

    assert_eq!(
        place(&mission(SandboxLevel::S0), &[candidat]),
        Placement::Placed {
            worker: "worker-muet".to_owned(),
            level: SandboxLevel::S0,
        }
    );
}

/// **Un `NotTrusted` sous un mécanisme étranger ne se lit pas « prouvé ailleurs ».**
///
/// Une campagne qui a conclu que non ne prouve rien sous aucun mécanisme. La compter parmi les
/// preuves écartées ferait dire « prouvé, mais ailleurs » d'un échec, et enverrait lancer une
/// seconde campagne pour un backend qui a déjà échoué.
#[test]
fn un_verdict_negatif_sous_un_autre_mecanisme_ne_compte_pas_comme_preuve() {
    let candidat = Candidate::new("worker-bwrap", hote(SandboxLevel::S3, Some(BUBBLEWRAP)))
        .attested(Attested {
            backend: PODMAN.to_owned(),
            standing: Standing::NotTrusted {
                level: SandboxLevel::S3,
                blocking: Vec::new(),
            },
        });

    let motifs = motifs(&mission(SandboxLevel::S2), candidat);

    assert_eq!(
        motifs,
        vec![RefusalReason::LevelNotAttested {
            required: SandboxLevel::S2,
            proven: None,
        }],
        "aucun motif de mécanisme : il n'y avait pas de preuve à écarter"
    );
}
