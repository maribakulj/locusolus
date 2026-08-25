//! Ce qu'une campagne a conclu, et ce qui empêche de le rejouer ailleurs — `W5.t`, §12.2.
//!
//! # Ce que ces tests protègent
//!
//! Une attestation qui voyagerait ferait placer des missions confinées sur un hôte où aucune sonde
//! n'a jamais tourné. C'est la seule chose que ce module ajoute au système, et c'est donc la seule
//! qu'il faille tenir : le **lien à l'hôte**.
//!
//! S'y ajoutent les deux propriétés que les amorçages de `locusd` tiennent déjà, parce que c'est la
//! même forme de porte : le **défaut ne change rien**, et un fichier nommé qui ne se lit pas
//! **refuse** au lieu de démarrer sans.

use locus_execd::announced::{NothingProven, Proven};
use locus_execd::attestation::{
    Attestation, EMIT_ENV, RECORD_ENV, RecordedProven, annonce, emit, fingerprint, load, record,
};
use locus_execd::linux::HostFacts;
use locus_execution::SandboxLevel;
use locus_execution::selftest::Standing;

/// Les faits de l'hôte qui fait tourner ces tests.
///
/// Lus, et non fabriqués : l'empreinte porte sur ce que `locus-execd` constate de sa machine, donc
/// un test qui inventerait des faits n'éprouverait pas le lien qu'il prétend éprouver.
fn faits() -> HostFacts {
    HostFacts::read_host()
}

/// D'autres faits, forcément différents de ceux d'ici.
///
/// Obtenus en lisant un **autre** arbre — un répertoire vide, où aucun fichier de `/sys` n'existe.
/// C'est la façon la plus honnête de fabriquer « un autre hôte » : on ne bricole pas la structure,
/// on lit ailleurs.
fn faits_d_ailleurs() -> HostFacts {
    HostFacts::read(std::path::Path::new(
        "/var/empty/locus-attestation-inexistant",
    ))
}

fn attestation(worker: &str, level: &str, host: &str) -> Attestation {
    Attestation {
        worker_id: worker.to_owned(),
        level: level.to_owned(),
        host: host.to_owned(),
        concluded_at: 1_700_000_000_000,
    }
}

fn json(records: &[Attestation]) -> String {
    serde_json::to_string(records).expect("les attestations se sérialisent")
}

// ---------------------------------------------------------------------------------------------
// 1. Le lien à l'hôte — la propriété qui donne son sens au module.
// ---------------------------------------------------------------------------------------------

/// **Une attestation écrite ici compte ici.**
///
/// Le pendant positif de tout ce qui suit. Une garde qui écarterait tout serait exacte et inutile.
#[test]
fn une_attestation_de_cet_hote_est_retenue() {
    let facts = faits();
    let record = attestation("canterel-01", "S2", &fingerprint(&facts));

    let recorded = RecordedProven::read(&json(&[record]), "/attestations.json", &facts)
        .expect("le fichier se lit");

    assert_eq!(recorded.honoured("canterel-01").len(), 1);
    assert_eq!(recorded.standing("canterel-01").len(), 1);
}

/// **Une attestation écrite ailleurs ne compte pas — et elle est comptée comme écartée.**
///
/// C'est le cœur. Un enregistrement qui dirait seulement « ce worker tient `S2` » serait copiable
/// sur n'importe quelle machine, et ferait placer des missions confinées sur un hôte où aucune sonde
/// n'a jamais tourné. Ce ne serait plus une attestation mais une déclaration — exactement ce que le
/// défaut `NothingProven` refusait de croire.
///
/// L'écart est **compté** et non tu : une attestation ignorée en silence est indiscernable d'une
/// attestation absente, et les deux se réparent différemment — l'une en relançant la campagne ici,
/// l'autre en posant le fichier.
#[test]
fn une_attestation_d_un_autre_hote_est_ecartee_et_comptee() {
    let facts = faits();
    let record = attestation("canterel-01", "S2", "empreinte-d-une-autre-machine");

    let recorded = RecordedProven::read(&json(&[record]), "/attestations.json", &facts)
        .expect("le fichier se lit");

    assert!(recorded.honoured("canterel-01").is_empty());
    assert_eq!(recorded.foreign("canterel-01").len(), 1);
    assert!(
        recorded.standing("canterel-01").is_empty(),
        "un placement ne s'appuie pas sur une campagne menée ailleurs"
    );
}

/// **L'empreinte suit les faits, et deux hôtes différents n'en rendent pas la même.**
///
/// Tenu en **lisant** deux arbres, plutôt qu'en fabriquant deux structures : ce que le module
/// promet est que l'empreinte change quand la machine change, et une structure bricolée à la main
/// ne dirait rien de cette promesse-là.
#[test]
fn deux_hotes_ne_rendent_pas_la_meme_empreinte() {
    assert_ne!(fingerprint(&faits()), fingerprint(&faits_d_ailleurs()));
    // Et elle est stable : deux lectures du même hôte se répondent, sans quoi aucune attestation ne
    // survivrait à un redémarrage.
    assert_eq!(fingerprint(&faits()), fingerprint(&faits()));
}

// ---------------------------------------------------------------------------------------------
// 2. Le défaut ne change rien.
// ---------------------------------------------------------------------------------------------

/// **Sans variable, rien n'est conservé — donc rien au-dessus de `S0`.**
#[test]
fn sans_variable_rien_n_est_conserve() {
    let facts = faits();
    let charge = load(|_| None, &facts).expect("rien à lire n'est pas une faute");

    assert!(charge.is_none());
    assert!(NothingProven.standing("canterel-01").is_empty());
}

/// **Une variable vide vaut une variable absente.**
///
/// Un `LOCUS_EXECD_ATTESTATIONS=""` traîne dans tous les orchestrateurs qui posent leurs variables
/// sans les remplir. La lire comme un chemin ferait refuser le démarrage d'un daemon dont personne
/// n'a rien demandé.
#[test]
fn une_variable_vide_ne_demande_rien() {
    let facts = faits();
    for vide in ["", "   "] {
        assert!(
            load(|name| (name == RECORD_ENV).then(|| vide.to_owned()), &facts)
                .expect("vide n'est pas une faute")
                .is_none()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Un fichier nommé qui ne se lit pas refuse.
// ---------------------------------------------------------------------------------------------

/// **Un fichier nommé et absent refuse, en citant le chemin.**
///
/// C'est la différence avec une variable absente : poser le chemin est une **intention**. Démarrer
/// sans les attestations laisserait l'exploitant lire `level_not_attested` en cherchant pourquoi son
/// fichier n'a rien fait.
#[test]
fn un_fichier_nomme_et_absent_refuse() {
    let facts = faits();
    let refus = load(
        |name| (name == RECORD_ENV).then(|| "/var/empty/pas-de-fichier.json".to_owned()),
        &facts,
    )
    .expect_err("un chemin posé et introuvable est une intention non honorée");

    assert_eq!(refus.path, "/var/empty/pas-de-fichier.json");
    assert!(refus.reason.contains("ne s'ouvre pas"), "{}", refus.reason);
}

/// **Un fichier mal formé refuse, et le refus se lit en une ligne.**
#[test]
fn un_fichier_mal_forme_refuse() {
    let facts = faits();
    let refus = RecordedProven::read("ceci n'est pas du JSON", "/attestations.json", &facts)
        .expect_err("le contenu ne se relit pas");

    let phrase = refus.to_string();
    assert!(phrase.contains("attestations"), "{phrase}");
    assert!(phrase.contains("/attestations.json"), "{phrase}");
}

/// **Un niveau inconnu refuse le fichier entier, pas seulement sa ligne.**
///
/// Écarter la ligne ferait démarrer un daemon qui honore trois attestations sur quatre **sans le
/// dire**, et l'exploitant lirait `level_not_attested` sur la quatrième sans savoir qu'elle a été
/// jetée à la lecture. C'est la même règle que partout ici : une ignorance ne se range pas en
/// silence dans une absence.
#[test]
fn un_niveau_inconnu_refuse_le_fichier_entier() {
    let facts = faits();
    let empreinte = fingerprint(&facts);
    let records = [
        attestation("canterel-01", "S2", &empreinte),
        attestation("canterel-02", "S9", &empreinte),
    ];

    let refus = RecordedProven::read(&json(&records), "/attestations.json", &facts)
        .expect_err("« S9 » n'est pas un niveau de §21.6");

    assert!(refus.reason.contains("S9"), "{}", refus.reason);
    assert!(
        refus.reason.contains("canterel-02"),
        "le refus nomme la ligne fautive : {}",
        refus.reason
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Ce que le démarrage annonce.
// ---------------------------------------------------------------------------------------------

/// **L'annonce compte les retenues *et* les écartées.**
///
/// Taire les secondes ferait lire « 0 attestation » à un exploitant dont le fichier en contient
/// trois, écrites sur une autre machine — et il chercherait un fichier vide au lieu d'un hôte qui a
/// changé.
#[test]
fn l_annonce_compte_les_deux() {
    let facts = faits();
    let empreinte = fingerprint(&facts);
    let records = [
        attestation("canterel-01", "S2", &empreinte),
        attestation("canterel-02", "S2", "une-autre-machine"),
    ];

    let recorded = RecordedProven::read(&json(&records), "/attestations.json", &facts)
        .expect("le fichier se lit");

    let phrase = annonce(&recorded);
    assert!(phrase.contains('1'), "{phrase}");
    assert!(phrase.contains("écartée"), "{phrase}");

    // `W5.w` : et elle porte **l'empreinte de cet hôte**. Dire à un exploitant que son
    // enregistrement parle d'une autre machine sans lui dire laquelle est celle-ci nomme le
    // problème et cache le remède ; avec elle, la correction est un copier-coller.
    assert!(
        phrase.contains(&empreinte),
        "l'empreinte de cet hôte manque au refus : {phrase}"
    );

    // Et sans écartée, la phrase ne parle pas d'un écart qui n'a pas eu lieu.
    let seules = [attestation("canterel-01", "S2", &empreinte)];
    let propre = RecordedProven::read(&json(&seules), "/attestations.json", &facts)
        .expect("le fichier se lit");
    assert!(
        !annonce(&propre).contains("écartée"),
        "{}",
        annonce(&propre)
    );
    // Ni de l'empreinte : ce serait du bruit sur le cas nominal, et une ligne que personne ne lit
    // plus ne dit rien à personne le jour où elle compte.
    assert!(
        !annonce(&propre).contains(&empreinte),
        "le cas nominal n'a pas besoin de l'empreinte : {}",
        annonce(&propre)
    );
}

/// **L'empreinte que le registre expose est celle de l'hôte qu'on lui a donné.**
///
/// Un accesseur qui rendrait autre chose — une constante, l'empreinte d'un enregistrement — ferait
/// donner à l'exploitant une valeur qui ne correspond à rien, et sa correction serait écartée à son
/// tour. Le pendant du test précédent, au grain du registre.
#[test]
fn le_registre_expose_l_empreinte_de_son_hote() {
    let facts = faits();
    let recorded = RecordedProven::read("[]", "/attestations.json", &facts).expect("vide se lit");

    assert_eq!(recorded.host(), fingerprint(&facts));
}

// ---------------------------------------------------------------------------------------------
// 5. Ce qu'une campagne dépose — `W5.u`.
// ---------------------------------------------------------------------------------------------

/// **Une campagne qui tient dépose une attestation liée à cet hôte.**
///
/// Et le tour complet se referme : ce qui est déposé se relit, et se relit **honoré**. Deux tests
/// séparés vérifieraient chacun leur moitié sans jamais dire que les deux se répondent — or c'est la
/// seule chose qui compte pour un fichier qu'un daemon va croire.
#[test]
fn ce_qu_une_campagne_depose_se_relit_et_est_honore() {
    let facts = faits();
    let depose = record(
        "canterel-01",
        &Standing::Trusted {
            level: SandboxLevel::S2,
        },
        &facts,
        1_700_000_000_000,
    )
    .expect("une campagne qui tient a quelque chose à déposer");

    let contenu = emit(&[depose], "/attestations.json").expect("le dépôt se sérialise");
    let recorded = RecordedProven::read(&contenu, "/attestations.json", &facts)
        .expect("ce qui a été déposé se relit");

    assert_eq!(recorded.honoured("canterel-01").len(), 1);
    assert_eq!(
        recorded.standing("canterel-01"),
        vec![Standing::Trusted {
            level: SandboxLevel::S2
        }],
        "le niveau déposé est celui qui ressort"
    );
}

/// **Une campagne qui ne tient pas ne dépose rien.**
///
/// `proven_level` ignore un `NotTrusted` : l'écrire ne changerait aucun placement et laisserait
/// croire qu'il le pourrait. L'absence d'enregistrement dit déjà « rien n'est prouvé », ce qui est
/// exactement ce qu'une campagne en échec établit.
#[test]
fn une_campagne_qui_ne_tient_pas_ne_depose_rien() {
    let facts = faits();

    assert!(
        record(
            "canterel-01",
            &Standing::NotTrusted {
                level: SandboxLevel::S2,
                blocking: Vec::new(),
            },
            &facts,
            1_700_000_000_000,
        )
        .is_none()
    );
}

/// **Les deux variables ne se confondent pas.**
///
/// L'une dit où `locus-execd` **lit**, l'autre où une campagne **écrit**. Les confondre ferait
/// qu'une campagne écrase le fichier qu'un daemon est en train de lire, et qu'un exploitant ne
/// puisse plus distinguer « ce que j'ai posé » de « ce que la dernière campagne a produit ».
#[test]
fn lire_et_deposer_ne_partagent_pas_leur_variable() {
    assert_ne!(RECORD_ENV, EMIT_ENV);
}

// ---------------------------------------------------------------------------------------------
// 6. L'empreinte porte des verdicts, pas de la prose — `W5.v`.
// ---------------------------------------------------------------------------------------------

/// **Aucun motif de `Support` n'entre dans l'empreinte.**
///
/// C'est le test qui aurait attrapé `W5.t`, et il n'existait pas. Le premier dépôt réel en CI a
/// écrit ceci dans le champ `host` :
///
/// ```text
/// disk_quota: Undetermined { reason: "aucune racine de stockage n'a été déclarée : on ne sait
///   pas quel système de fichiers portera la couche inscriptible" }
/// ```
///
/// Une **phrase** dans l'identité d'un hôte. La reformuler — une virgule, une clarification —
/// invaliderait silencieusement toutes les attestations déjà déposées, sur toutes les machines,
/// sans qu'aucun hôte ait changé. Le refus dirait alors `level_not_attested`, qui envoie relancer
/// une campagne au lieu de relire un diff.
///
/// Tenu sur des faits **réels** et non fabriqués : ce que ce test refuse est que la prose des faits
/// de cette machine-ci se retrouve dans son empreinte.
#[test]
fn aucune_prose_n_entre_dans_l_empreinte() {
    let empreinte = fingerprint(&faits());

    // Les mots qui trahissent un motif recopié. Ils viennent des `reason` que `probe.rs` écrit ;
    // aucun n'a de raison d'apparaître dans une identité d'hôte.
    for prose in ["aucune racine", "on ne sait pas", "reason", "déclarée"] {
        assert!(
            !empreinte.contains(prose),
            "« {prose} » est de la prose, pas un verdict : {empreinte}"
        );
    }
}

/// **Les cinq faits qui décident sont dans l'empreinte, nommés.**
///
/// Le pendant du test précédent : retirer la prose ne doit pas retirer la substance. La liste est
/// écrite ici **en clair** pour que l'ajout d'un sixième fait à `HostFacts` se remarque — une
/// empreinte énumérée à la main se périme au premier champ ajouté, et c'est le coût assumé de ne
/// pas y mettre de prose.
#[test]
fn les_cinq_faits_qui_decident_sont_dans_l_empreinte() {
    let empreinte = fingerprint(&faits());

    for fait in [
        "cgroup_v2=",
        "controllers=",
        "userns=",
        "seccomp=",
        "disk_quota=",
    ] {
        assert!(empreinte.contains(fait), "{fait} manque : {empreinte}");
    }
}

/// Un noyau de fixture : chaque chemin rend ce qu'on lui dit, et rien d'autre.
///
/// Écrit parce qu'une passe de mutation a montré que les tests précédents ne valaient rien contre
/// les mutants qui comptent — voir [`chaque_fait_contribue_a_l_empreinte`].
struct Noyau {
    controleurs: Option<&'static str>,
    userns: Option<&'static str>,
    seccomp: Option<&'static str>,
    /// Le système de fichiers qui porte la racine, tel que `mountinfo` le décrirait.
    stockage: &'static str,
    /// Ses options de super-bloc — `prjquota` est ce qui rend un quota de projet **activé**.
    options: &'static str,
}

impl Noyau {
    /// Un hôte qui offre tout.
    const fn complet() -> Self {
        Self {
            controleurs: Some("cpu io memory pids"),
            userns: Some("15000"),
            seccomp: Some("act_allow act_errno act_kill_process act_trap"),
            stockage: "ext4",
            options: "rw",
        }
    }
}

impl locus_execd::linux::Reader for Noyau {
    fn read(&self, path: &str) -> Option<String> {
        // Le prober lit le `cgroup.controllers` de **son propre** répertoire, pas celui de la
        // racine : `/proc/self/cgroup` le désigne, et le chemin qui en découle doit répondre aussi.
        // Une fixture qui ne servirait que la racine ferait rendre « illisible » à un hôte complet,
        // et le test parlerait alors d'autre chose que de ce qu'il croit.
        if path.ends_with("/cgroup.controllers") {
            return self.controleurs.map(str::to_owned);
        }
        match path {
            "/proc/self/cgroup" => Some("0::/\n".to_owned()),
            "/proc/sys/user/max_user_namespaces" => self.userns.map(str::to_owned),
            "/proc/sys/kernel/seccomp/actions_avail" => self.seccomp.map(str::to_owned),
            "/proc/self/mountinfo" => Some(format!(
                "31 1 8:1 / / rw,relatime shared:1 - {} /dev/sda1 {}\n",
                self.stockage, self.options
            )),
            _ => None,
        }
    }
}

/// **Chaque fait qui décide fait bouger l'empreinte, seul.**
///
/// # Ce que ce test remplace, et pourquoi
///
/// La première rédaction vérifiait que les clés étaient **présentes** et que la prose était
/// **absente**. Une passe de mutation l'a démolie : vider les contrôleurs, faire lire `seccomp` à
/// la place de `disk_quota`, ou rendre `Undetermined` indiscernable d'`Available` — les trois
/// **survivaient**. Des assertions de forme, là où ce qui compte est la **contribution**.
///
/// C'est exactement le défaut que ce dépôt nomme partout ailleurs — « comparé champ pour champ, pas
/// par présence » —, et je l'avais écrit dans mes propres tests. Celui-ci fait varier un fait à la
/// fois, en pilotant le vrai prober avec un noyau de fixture, et exige que l'empreinte change.
#[test]
fn chaque_fait_contribue_a_l_empreinte() {
    let complet = fingerprint(&HostFacts::probe(&Noyau::complet()));

    // Un contrôleur en moins : l'hôte ne borne plus la même chose.
    let sans_pids = fingerprint(&HostFacts::probe(&Noyau {
        controleurs: Some("cpu io memory"),
        ..Noyau::complet()
    }));
    assert_ne!(
        complet, sans_pids,
        "l'ensemble des contrôleurs doit peser : sans lui, un hôte qui perd `pids` garderait son \
         attestation"
    );

    // `userns` illisible : « on n'a pas pu savoir » n'est pas « l'hôte l'offre ».
    let userns_inconnu = fingerprint(&HostFacts::probe(&Noyau {
        userns: None,
        ..Noyau::complet()
    }));
    assert_ne!(complet, userns_inconnu);

    // Et `seccomp` de même — les deux étant lus séparément, un seul accesseur recopié pour l'autre
    // rendrait ces deux empreintes identiques.
    let seccomp_inconnu = fingerprint(&HostFacts::probe(&Noyau {
        seccomp: None,
        ..Noyau::complet()
    }));
    assert_ne!(complet, seccomp_inconnu);
    assert_ne!(
        userns_inconnu, seccomp_inconnu,
        "deux capacités différentes ne rendent pas la même empreinte : un accesseur recopié le \
         ferait, et la mutation l'a montré"
    );
}

/// **Le quota disque pèse, et il pèse séparément.**
///
/// `with_storage` est la seconde étape du prober — le quota est une propriété du **chemin** où le
/// runtime écrira, pas de l'hôte en général. Une empreinte qui l'ignorerait ferait honorer, sur un
/// hôte où la borne n'est pas applicable, une attestation conclue là où elle l'était.
#[test]
fn le_quota_disque_pese_dans_l_empreinte() {
    let sur_ext4 = Noyau::complet();
    // `xfs` **et** `prjquota` : le premier seul ne suffit pas — un XFS monté sans l'option est un
    // hôte où `--storage-opt size=` échouera quand même, et `probe.rs` le range en `Unavailable`
    // comme `ext4`. Les deux sont alors le même verdict, et l'empreinte a raison de ne pas les
    // distinguer : pour ce que la campagne a éprouvé, ces hôtes sont interchangeables.
    //
    // Ma première rédaction opposait `ext4` à un `xfs` nu et attendait deux empreintes différentes.
    // C'était le test qui avait tort.
    let sur_xfs = Noyau {
        stockage: "xfs",
        options: "rw,prjquota",
        ..Noyau::complet()
    };

    // Sans racine déclarée, le quota est **indéterminé** — personne n'a dit où l'on écrira. Avec
    // une racine sur `ext4`, il devient **indisponible** : le fait est établi, et négativement.
    let indetermine = fingerprint(&HostFacts::probe(&sur_ext4));
    let sur_ext4_declare = fingerprint(&HostFacts::probe(&sur_ext4).with_storage(&sur_ext4, "/"));
    assert_ne!(
        indetermine, sur_ext4_declare,
        "« on ne sait pas où l'on écrira » et « on sait, et la borne n'y est pas applicable » sont \
         deux états : les fondre ferait honorer une attestation sur un hôte dont on ignore le \
         système de fichiers"
    );

    // Et deux systèmes de fichiers différents ne rendent pas la même empreinte : sans quoi un
    // accesseur recopié — `disk_quota` lisant `seccomp` — passerait inaperçu.
    assert_ne!(
        sur_ext4_declare,
        fingerprint(&HostFacts::probe(&sur_xfs).with_storage(&sur_xfs, "/")),
    );
}
