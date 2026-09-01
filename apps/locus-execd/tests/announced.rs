//! Le test de sortie de `W20.q` — un worker est placé sur ce qu'il a **prouvé**.
//!
//! # Ce que ces tests éprouvent
//!
//! `W4.g` a livré `place` sans appelant, et la file de `W20.k` servait la première offre à qui la
//! demandait, quel que soit son manifeste — sa propre documentation le disait. Ici la question
//! traverse le couloir de l'ADR 0028 : le vrai client de `locusd` envoie un `CapabilityManifest`,
//! le vrai serveur de `locus-execd` le relit, appelle le vrai `place`, et rend un verdict.
//!
//! # Les documents viennent de `W0.7`, aucun n'est bricolé ici
//!
//! Le worker macOS est `capability-manifest.json` — celui qui annonce `S1`/`S2` « et jamais
//! `S3`/`S4` », parce que Seatbelt. La mission est `mission-envelope-nominal.json`, qui exige `S3`
//! et qui s'apparie avec `capability-manifest-vm-linux.json`. Et le refus attendu est
//! `admission-refusal-four-reasons.json`, qui s'apparie explicitement avec le manifeste macOS.
//!
//! Un manifeste écrit pour l'occasion aurait éprouvé le code contre lui-même. Les fixtures de `W0.7`
//! existent précisément pour que les deux moitiés du fil soient jugées sur les mêmes documents.

use std::path::PathBuf;
use std::thread;

use locus_broker::port::{BrokerPort, Placement};
use locus_broker::unix::{UnixSocketBroker, listen};
use locus_execd::announced::{Attested, NothingProven, Proven, placement, requirement};
use locus_execd::link::serve;
use locus_execd::linux::HostFacts;
use locus_execution::{SandboxLevel as Niveau, Standing};
use locus_lep::{CapabilityManifest, MissionEnvelope, Reason, SandboxLevel};

const MACOS: &str = "canterel-macbook-01";
const LINUX: &str = "canterel-vm-linux-01";

/// Les mécanismes que les deux manifestes de fixture **annoncent** — ADR 0035 décision 3.
///
/// Repris du corpus, pas choisis ici : une campagne qui attesterait sous un autre nom serait écartée
/// par le rapprochement, et chaque cas de placement éprouverait le rapprochement au lieu de ce qu'il
/// annonce éprouver. Un test le vérifie plutôt que de le supposer.
const MECANISME_MACOS: &str = "seatbelt";
const MECANISME_LINUX: &str = "rootless-oci";

fn fixture<T: serde::de::DeserializeOwned>(nom: &str) -> T {
    let chemin = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/examples")
        .join(nom);
    let brut = std::fs::read_to_string(&chemin).expect("fixture lisible");
    let mut valeur: serde_json::Value =
        serde_json::from_str(&brut).expect("fixture en JSON valide");
    valeur
        .as_object_mut()
        .expect("une fixture est un objet")
        .remove("_fixture");
    serde_json::from_value(valeur).expect("la fixture se décode dans le type généré")
}

fn mission() -> MissionEnvelope {
    let mission: MissionEnvelope = fixture("mission-envelope-nominal.json");
    assert_eq!(
        mission.sandbox.minimum_level,
        SandboxLevel::S3,
        "la mission nominale de W0.7 exige S3 ; si elle change, ce n'est plus elle qu'on éprouve"
    );
    mission
}

fn manifeste(nom: &str) -> CapabilityManifest {
    fixture(nom)
}

/// Une campagne de self-tests qui a conclu, pour ce worker et à ce niveau.
///
/// C'est ce que `W4.d.3` produit. Aucun code de ce dépôt n'en conserve encore les verdicts — c'est
/// `W12.e` —, et ce port est là pour que l'absence se voie plutôt que de se combler par un défaut
/// permissif.
struct Campagne {
    worker: String,
    /// Le mécanisme sous lequel elle a conclu — ADR 0035 décision 3.
    ///
    /// Le double par défaut atteste sous le mécanisme que les manifestes de fixture annoncent : sans
    /// cela, chaque cas de placement se ferait écarter par le rapprochement de mécanismes plutôt que
    /// d'éprouver ce qu'il annonce éprouver.
    backend: String,
    verdict: Standing,
}

impl Proven for Campagne {
    fn standing(&self, worker_id: &str) -> Vec<Attested> {
        if worker_id == self.worker {
            vec![Attested {
                backend: self.backend.clone(),
                standing: self.verdict.clone(),
            }]
        } else {
            Vec::new()
        }
    }
}

fn codes(reasons: &[Reason]) -> Vec<&'static str> {
    reasons
        .iter()
        .map(|reason| match reason {
            Reason::LevelUnavailable { .. } => "level_unavailable",
            Reason::CapacityExceeded => "capacity_exceeded",
            Reason::AcceleratorUnavailable { .. } => "accelerator_unavailable",
            Reason::DiskQuotaNotEnforceable { .. } => "disk_quota_not_enforceable",
            Reason::NetworkModeUnsupported { .. } => "network_mode_unsupported",
            Reason::LevelNotAttested { .. } => "level_not_attested",
            Reason::AcceleratorOutsideSandbox { .. } => "accelerator_outside_sandbox",
            Reason::MechanismNotEmployed { .. } => "mechanism_not_employed",
            Reason::MechanismUnresolved { .. } => "mechanism_unresolved",
        })
        .collect()
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("locus-execd-place-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("répertoire de travail");
        Self(path)
    }

    fn socket(&self) -> PathBuf {
        self.0.join("broker.sock")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Servir sur un vrai tube, avec la campagne donnée, et poser **une** question de placement.
fn demander(nom: &str, manifeste: &CapabilityManifest, proven: Campagne) -> Placement {
    let mission = mission();
    match sur_le_tube(nom, manifeste, &mission.sandbox, &mission.resources, proven) {
        Ok(placement) => placement,
        Err(verdict) => panic!("le broker devait répondre sur le placement : {verdict:?}"),
    }
}

/// Le même, en rendant le verdict brut quand ce n'est **pas** une réponse de placement.
///
/// `Err` porte donc « le broker a parlé, mais d'autre chose » — ce qui est le cas d'une demande
/// illisible, et ce que le port refuse de lire comme un refus de placement.
fn sur_le_tube(
    nom: &str,
    manifeste: &CapabilityManifest,
    sandbox: &locus_lep::SandboxSpec,
    resources: &locus_lep::ResourceSpec,
    proven: Campagne,
) -> Result<Placement, locus_broker::port::BrokerError> {
    let scratch = Scratch::new(nom);
    let path = scratch.socket();
    let listener = listen(&path).expect("écoute");
    let service = thread::spawn(move || {
        let facts = HostFacts::read_host();
        serve(&listener, &facts, &proven, |_| {});
    });
    drop(service);

    UnixSocketBroker::at(&path).place(manifeste, sandbox, resources)
}

/// La campagne qui a conclu `S3` pour le worker Linux — le cas nominal de `W0.7`.
fn campagne_linux() -> Campagne {
    Campagne {
        worker: LINUX.to_owned(),
        backend: MECANISME_LINUX.to_owned(),
        verdict: Standing::Trusted { level: Niveau::S3 },
    }
}

// ---------------------------------------------------------------------------------------------
// 1. La paire de refus de `W0.7` : un worker macOS ne reçoit pas une mission `S3`.
// ---------------------------------------------------------------------------------------------

/// **Les mécanismes que ces tests attestent sont ceux que les fixtures annoncent.**
///
/// `MECANISME_MACOS` et `MECANISME_LINUX` sont des constantes de ce fichier, donc des affirmations
/// sur le corpus. Si une fixture change de `backend` sans que la constante suive, chaque campagne
/// serait écartée par le rapprochement de l'ADR 0035 décision 3 — et tous les cas de placement
/// tomberaient d'un coup, en accusant le placement plutôt que la constante. Le lire ici le dit une
/// fois, à l'endroit où l'on peut le corriger.
#[test]
fn les_constantes_de_mecanisme_sont_celles_du_corpus() {
    assert_eq!(
        manifeste("capability-manifest.json")
            .sandbox
            .backend
            .as_deref(),
        Some(MECANISME_MACOS)
    );
    assert_eq!(
        manifeste("capability-manifest-vm-linux.json")
            .sandbox
            .backend
            .as_deref(),
        Some(MECANISME_LINUX)
    );
}

/// **Un worker macOS n'est pas placé sur une mission qui exige `S3`.**
///
/// C'est la clause de `W12.d` « placé sur ce qu'il a prouvé », prise par son cas le plus net.
/// `capability-manifest.json` annonce `S1`/`S2` — « Seatbelt = S1/S2, jamais S3/S4 » —, donc l'hôte
/// ne sait pas faire, et il n'a rien prouvé non plus : **deux** motifs, et le refus les porte tous
/// les deux. Un refus qui n'en dirait qu'un ferait corriger une chose, relancer, découvrir l'autre.
#[test]
fn un_worker_macos_ne_recoit_pas_une_mission_qui_exige_s3() {
    let macos = manifeste("capability-manifest.json");
    assert_eq!(macos.worker_id, MACOS);

    let verdict = demander(
        "macos-refuse",
        &macos,
        Campagne {
            worker: MACOS.to_owned(),
            backend: MECANISME_MACOS.to_owned(),
            // La campagne a conclu, et elle a conclu **bas**. C'est le cas qui compte : « il a
            // prouvé S2 » n'est pas « on ne sait pas », et le refus doit le dire.
            verdict: Standing::Trusted { level: Niveau::S2 },
        },
    );

    let Placement::NotPlaced { shortfalls } = verdict else {
        panic!("un hôte qui plafonne à S2 ne porte pas une mission S3 : {verdict:?}");
    };
    assert_eq!(shortfalls.len(), 1, "un manque par worker soumis");
    assert_eq!(shortfalls[0].worker, MACOS);

    let produits = codes(&shortfalls[0].reasons);
    assert!(
        produits.contains(&"level_unavailable"),
        "l'hôte ne sait pas confiner aussi fort, et le refus le dit : {produits:?}"
    );
    assert!(
        produits.contains(&"level_not_attested"),
        "et il n'a pas prouvé S3 non plus — les deux ne se fondent pas : {produits:?}"
    );

    // Le vocabulaire est celui du refus de `W0.7` qui s'apparie avec ce manifeste-là. La fixture
    // n'est pas décorative : elle dit quels codes un refus macOS porte, et ce test refuse d'en
    // inventer un autre.
    let attendu: locus_lep::AdmissionRefusal = fixture("admission-refusal-four-reasons.json");
    let vocabulaire = codes(&attendu.reasons);
    for code in &produits {
        assert!(
            vocabulaire.contains(code),
            "« {code} » ne figure pas dans le refus apparié de W0.7 ({vocabulaire:?}) : un motif \
             que le corpus ne connaît pas est un motif que personne n'a défini"
        );
    }
}

/// **Le worker Linux qui a prouvé `S3` reçoit la mission, au niveau qu'elle exige.**
///
/// Le pendant indispensable du test précédent : sans lui, un broker qui refuserait tout le monde
/// passerait le premier. `capability-manifest-vm-linux.json` s'apparie avec cette mission — « S3
/// demandé, S3 offert, ressources dans l'enveloppe » — et la campagne a conclu.
///
/// Le niveau rendu est celui que la **mission exige**, pas le plafond de l'hôte : accorder un `S3`
/// à qui demandait `S1` gaspillerait un hôte rare, et accorder le plafond ferait varier le
/// confinement appliqué selon la machine qui a répondu.
#[test]
fn un_worker_linux_qui_a_prouve_s3_est_place_au_niveau_exige() {
    let linux = manifeste("capability-manifest-vm-linux.json");
    assert_eq!(linux.worker_id, LINUX);

    let verdict = demander(
        "linux-place",
        &linux,
        Campagne {
            worker: LINUX.to_owned(),
            backend: MECANISME_LINUX.to_owned(),
            verdict: Standing::Trusted { level: Niveau::S3 },
        },
    );

    assert_eq!(
        verdict,
        Placement::Placed {
            worker: LINUX.to_owned(),
            level: SandboxLevel::S3,
        },
        "l'hôte apparié à cette mission la porte, au niveau qu'elle exige"
    );
}

/// **Le même worker Linux, sans campagne, n'est pas placé — et le motif est l'attestation.**
///
/// C'est la propriété que `placement.rs` existe pour tenir : « la confiance ne se déclare pas, elle
/// se prouve ». Le manifeste annonce `S3` et `attestation: true` ; ce champ dit que le worker
/// **sait produire** une attestation, pas qu'il en a produit une. Le lire comme une preuve ferait
/// marcher le placement sur un hôte dont personne n'a rien vérifié, et rien ne le signalerait.
///
/// Le refus ne porte **qu'un** motif : l'hôte annonce bien tout le reste. Un refus qui dirait aussi
/// `level_unavailable` enverrait changer de machine pour un problème de campagne.
#[test]
fn un_worker_qui_annonce_s3_sans_l_avoir_prouve_n_est_pas_place() {
    let linux = manifeste("capability-manifest-vm-linux.json");
    assert_eq!(
        linux.sandbox.attestation,
        Some(true),
        "la fixture annonce savoir attester : c'est précisément ce qu'on refuse de prendre pour \
         une preuve"
    );

    let verdict = demander(
        "linux-sans-preuve",
        &linux,
        Campagne {
            // Une campagne qui a conclu pour **quelqu'un d'autre** : le broker ne doit pas la lui
            // prêter. Un port qui rendrait le même verdict à tout le monde passerait les deux
            // autres tests.
            worker: MACOS.to_owned(),
            backend: MECANISME_MACOS.to_owned(),
            verdict: Standing::Trusted { level: Niveau::S3 },
        },
    );

    let Placement::NotPlaced { shortfalls } = verdict else {
        panic!("une annonce n'est pas une preuve : {verdict:?}");
    };
    assert_eq!(
        codes(&shortfalls[0].reasons),
        vec!["level_not_attested"],
        "un seul motif : tout le reste de l'annonce convient, et seule la preuve manque"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Ce que §15.3 n'annonce pas ne décide de rien — et un test le tient.
// ---------------------------------------------------------------------------------------------

/// **L'horizon d'une mission ne change aucun placement.**
///
/// Un manifeste n'inventorie pas de budget de temps : `quotas_fit_within` regarde le CPU, la
/// mémoire, les PID et le disque, et pas l'horizon. La valeur prêtée à l'offre est donc arbitraire
/// et **inerte**.
///
/// Ce test épingle cette inertie. Le jour où l'horizon se mettra à peser dans la comparaison, il
/// rougira, et il dira à qui l'a fait peser que §15.3 ne l'annonce pas — donc qu'un worker se
/// verrait refuser une mission sur une grandeur qu'il n'a jamais déclarée.
#[test]
fn l_horizon_d_une_mission_ne_decide_d_aucun_placement() {
    let linux = manifeste("capability-manifest-vm-linux.json");
    let mission = mission();
    let campagne = Campagne {
        worker: LINUX.to_owned(),
        backend: MECANISME_LINUX.to_owned(),
        verdict: Standing::Trusted { level: Niveau::S3 },
    };

    let mut long = mission.resources.clone();
    long.wall_time_seconds = 86_400 * 365;
    let mut court = mission.resources.clone();
    court.wall_time_seconds = 1;

    let avec_long = placement(&linux, &mission.sandbox, &long, &campagne).expect("lisible");
    let avec_court = placement(&linux, &mission.sandbox, &court, &campagne).expect("lisible");

    assert_eq!(
        avec_long, avec_court,
        "l'horizon n'est pas un critère de placement, faute d'être annoncé"
    );
}

/// **Le profil d'une mission ne change aucun placement non plus.**
///
/// §21.6 : un profil « nomme une intention ; c'est `minimum_level` qui engage », et `level.rs` note
/// que les profils ne portent pas de niveau. Une mission qui n'en nomme aucun s'en voit donc prêter
/// un, et ce prêt doit rester sans conséquence.
///
/// Les sept sont éprouvés, pas un échantillon : c'est le seul moyen de dire que la valeur prêtée
/// n'est pas un privilège déguisé.
#[test]
fn le_profil_prete_a_une_mission_qui_n_en_nomme_aucun_est_inerte() {
    let linux = manifeste("capability-manifest-vm-linux.json");
    let mission = mission();
    let campagne = Campagne {
        worker: LINUX.to_owned(),
        backend: MECANISME_LINUX.to_owned(),
        verdict: Standing::Trusted { level: Niveau::S3 },
    };
    assert!(
        mission.sandbox.profile.is_none(),
        "la mission nominale n'en nomme aucun : c'est le cas où un profil est prêté"
    );

    let sans = placement(&linux, &mission.sandbox, &mission.resources, &campagne).expect("lisible");

    for profil in locus_execution::SandboxProfile::ALL {
        let mut nomme = mission.sandbox.clone();
        nomme.profile = Some(profil.slug().to_owned());
        let avec =
            placement(&linux, &nomme, &mission.resources, &campagne).expect("profil de §21.6");
        assert_eq!(
            avec, sans,
            "le profil « {profil} » ne doit rien changer au placement"
        );
    }
}

/// **Un profil que §21.6 ne nomme pas est refusé, pas ignoré.**
///
/// Un nom que personne n'a défini serait quand même écrit dans le journal de ce qui a été appliqué,
/// et se lirait comme une intention honorée. C'est la règle que `xiiif` tient pour un cinquième
/// verdict de revue, et elle vaut ici.
///
/// La demande est **illisible**, donc ce n'est pas un refus de placement : le broker rend
/// `Verdict::Refused`, qui envoie corriger une requête et non changer de machine.
#[test]
fn un_profil_hors_de_21_6_rend_la_demande_illisible() {
    let mission = mission();
    let mut invente = mission.sandbox.clone();
    invente.profile = Some("profil-maison".to_owned());

    let erreur = requirement(&invente, &mission.resources)
        .expect_err("un profil inventé ne se lit pas comme une exigence");

    assert!(
        erreur.to_string().contains("profil-maison"),
        "le refus cite ce qui a été annoncé : {erreur}"
    );
    assert!(
        erreur.to_string().contains("interactive-local"),
        "et il énumère les sept de §21.6, pour qu'on n'ait pas à les chercher : {erreur}"
    );
}

/// **Un manifeste qui n'annonce aucun niveau ne se lit pas — et ne vaut pas `S0`.**
///
/// Un worker qui n'annonce rien n'a pas dit qu'il exécute à nu : il n'a rien dit. Les confondre
/// placerait une mission `S0` sur un silence, ce qui est la forme la plus discrète d'une exécution
/// non confinée.
#[test]
fn un_manifeste_sans_niveau_ne_se_lit_pas() {
    let mut muet = manifeste("capability-manifest-vm-linux.json");
    muet.sandbox.levels.clear();
    let mission = mission();

    let erreur = placement(&muet, &mission.sandbox, &mission.resources, &NothingProven)
        .expect_err("un manifeste muet ne se lit pas");

    assert!(
        erreur.to_string().contains("aucun niveau"),
        "le refus dit ce qui manque : {erreur}"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Ce qu'une passe de mutation a trouvé, et que rien ne tenait.
// ---------------------------------------------------------------------------------------------

/// **L'accélérateur retenu est celui que la mission demande, pas le premier annoncé.**
///
/// Un manifeste inventorie **plusieurs** accélérateurs ; une `ResourceSpec` n'en porte qu'un, parce
/// que c'est une réservation. En élire un sans savoir ce qui est demandé refuserait une mission
/// `mps` sur un hôte offrant `cuda` **et** `mps`, au motif que `cuda` était écrit en premier — et le
/// refus dirait `accelerator_unavailable`, c'est-à-dire enverrait acheter du matériel qui est là.
///
/// La documentation de `capabilities` portait cet argument ; **rien ne l'éprouvait**. Une passe de
/// mutation a remplacé la recherche par genre par « le premier » et a survécu.
#[test]
fn l_accelerateur_retenu_est_celui_que_la_mission_demande() {
    let mut hote = manifeste("capability-manifest-vm-linux.json");
    hote.accelerators = Some(vec![
        locus_lep::CapabilityManifestAcceleratorsItem {
            r#type: locus_lep::AcceleratorType::Cuda,
            count: 1,
            memory_mb: Some(8192),
        },
        locus_lep::CapabilityManifestAcceleratorsItem {
            r#type: locus_lep::AcceleratorType::Mps,
            count: 1,
            memory_mb: Some(8192),
        },
    ]);

    let mission = mission();
    let mut demande = mission.resources.clone();
    // Le second de la liste, délibérément : le premier passerait aussi avec « prends le premier ».
    demande.accelerator = Some(locus_lep::ResourceSpecAccelerator {
        r#type: locus_lep::AcceleratorType::Mps,
        count: 1,
        memory_mb: Some(4096),
    });

    let verdict =
        placement(&hote, &mission.sandbox, &demande, &campagne_linux()).expect("demande lisible");

    assert_eq!(
        verdict,
        locus_execd::placement::Placement::Placed {
            worker: LINUX.to_owned(),
            level: Niveau::S3,
        },
        "l'hôte offre bien « mps » : le refuser parce que « cuda » vient en premier enverrait \
         acheter du matériel qui est là"
    );
}

/// **`connector-only` du fil et `connector_only` du domaine désignent le même mode.**
///
/// `locus_execution::NetworkMode::ConnectorOnly.slug()` s'écrit avec un **tiret bas**, et LEP écrit
/// un **tiret**. `admit` compare le slug de la mission aux modes annoncés par l'hôte : comparer
/// l'une à l'autre ferait qu'un hôte annonçant `connector-only` ne satisferait jamais une mission
/// qui l'exige — un refus permanent, silencieux, sur une capacité que l'hôte a bel et bien.
///
/// La traduction portait ce commentaire ; **rien ne l'éprouvait**. Une passe de mutation a remis
/// l'orthographe du fil à la place de celle du domaine et a survécu.
#[test]
fn les_deux_orthographes_de_connector_only_designent_le_meme_mode() {
    let hote = manifeste("capability-manifest-vm-linux.json");
    assert!(
        hote.sandbox
            .network_modes
            .contains(&locus_lep::NetworkMode::ConnectorOnly),
        "la fixture Linux annonce ce mode ; sans cela le test ne prouverait rien"
    );

    let mission = mission();
    let mut exige = mission.sandbox.clone();
    exige.network = locus_lep::NetworkMode::ConnectorOnly;

    let verdict =
        placement(&hote, &exige, &mission.resources, &campagne_linux()).expect("demande lisible");

    assert_eq!(
        verdict,
        locus_execd::placement::Placement::Placed {
            worker: LINUX.to_owned(),
            level: Niveau::S3,
        },
        "l'hôte annonce « connector-only » et la mission l'exige : une différence d'orthographe \
         entre le fil et le domaine ne doit pas produire un refus"
    );
}

/// **Une demande illisible traverse le fil comme un refus, pas comme un placement manqué.**
///
/// `un_profil_hors_de_21_6_rend_la_demande_illisible` éprouve la lecture ; celui-ci éprouve ce
/// qu'elle **devient sur le fil**. Un `NotPlaced` vide enverrait chercher une machine plus grosse à
/// qui a envoyé un document incomplet, et un refus sans motif est précisément le refus muet que
/// l'ADR 0028 décision 2 interdit.
///
/// Une passe de mutation a remplacé le `Refused` par un `NotPlaced` vide et a survécu : rien ne
/// traversait cette branche-là.
#[test]
fn une_demande_illisible_devient_un_refus_sur_le_fil_et_non_un_placement_manque() {
    let hote = manifeste("capability-manifest-vm-linux.json");
    let mission = mission();
    let mut invente = mission.sandbox.clone();
    invente.profile = Some("profil-maison".to_owned());

    let verdict = sur_le_tube(
        "illisible",
        &hote,
        &invente,
        &mission.resources,
        campagne_linux(),
    )
    .expect("le broker répond");

    let Placement::Refused { why } = verdict else {
        panic!("une demande illisible se refuse, elle ne se place pas mal : {verdict:?}");
    };
    assert!(
        why.contains("profil-maison"),
        "le refus dit ce qui n'a pas été compris : {why}"
    );
}
