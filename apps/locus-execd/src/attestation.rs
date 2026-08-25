//! Ce qu'une campagne de self-tests a conclu, et que `locus-execd` peut relire — `W5.t`, §12.2.
//!
//! # Le constat, et il était **écrit d'avance**
//!
//! `main.rs` câble `NothingProven`, et son commentaire dit exactement ce qui allait arriver :
//!
//! > « aucune campagne de self-tests n'est conservée par ce binaire, donc aucun worker n'a rien
//! > prouvé à ses yeux, donc il ne place rien au-dessus de `S0` […]. C'est exact — et c'est ce qui
//! > rend visible, **au premier placement réel**, qu'il manque la campagne, plutôt que de placer sur
//! > une déclaration. »
//!
//! Le premier placement réel a eu lieu, par le harnais de `W12.f`, et il a rendu ce que la phrase
//! annonçait : `level_not_attested (S1, proven: None)`. La quatrième occurrence de la même forme —
//! un port dont le défaut refuse, correct séparément, qu'aucun assemblage de production ne remplace,
//! après `NoIdentities`, `NoAdministrators` et `NoBlobs`.
//!
//! Ce qui est différent ici : la campagne, elle, **tourne déjà**. Le job `sandbox` de la CI exerce
//! seize sondes contre un conteneur rootless réel à `S2`, et elles tiennent. Ce qui manque n'est ni
//! l'exécution ni le verdict : c'est un endroit où le verdict **survit** au processus qui l'a rendu.
//!
//! # Une attestation est liée à l'hôte qui l'a subie
//!
//! C'est la décision qui compte dans ce module, et elle est facile à manquer.
//!
//! Un enregistrement qui dirait seulement « ce worker tient `S2` » serait rejouable **ailleurs** :
//! copié sur une autre machine, il ferait placer des missions `S2` sur un hôte où aucune sonde n'a
//! jamais tourné. Ce n'est plus une attestation, c'est une déclaration — précisément ce que le
//! défaut de `main.rs` refusait de croire.
//!
//! Chaque enregistrement porte donc l'**empreinte des capacités** contre lesquelles la campagne a
//! conclu, et [`RecordedProven`] n'honore un enregistrement que si l'hôte présenté aujourd'hui rend
//! la même empreinte. Un hôte qui a changé — un contrôleur cgroup retiré, un plafond qui baisse —
//! n'est plus l'hôte qui a prouvé, et son enregistrement cesse **tout seul** de compter.
//!
//! L'alternative aurait été une date de péremption. Elle se règle au jugé et ne dit rien : un hôte
//! peut se dégrader en une minute et rester identique un an.
//!
//! # Ce que l'empreinte suppose, et qui n'est vrai que du profil local
//!
//! Elle porte sur l'hôte de **`locus-execd`**, et [`Proven`] est indexé par **worker**. Les deux ne
//! coïncident que si le worker s'exécute là où `locus-execd` tourne — ce qui est le cas du profil
//! `personal-local` de §27.1, et de tout déploiement où la sandbox est celle que ce daemon pilote.
//!
//! C'est une supposition, et elle est écrite ici plutôt que tue. Le jour où un `locus-execd`
//! placerait sur des hôtes qu'il ne pilote pas, l'empreinte parlerait de la mauvaise machine — et
//! ce qu'il faudrait alors n'est pas un correctif de ce module mais une campagne par hôte, avec son
//! identité propre. La supposition tient tant que le broker et la sandbox sont sur la même machine.
//!
//! # Ce que le module ne fait pas
//!
//! Il ne **produit** pas la campagne — `packages/execution` la mène et rend un
//! [`locus_execution::selftest::Standing`]. Il ne décide pas non plus du placement :
//! `Candidate::proven_level` lit ce qu'il rend, comme il lisait déjà `NothingProven`.
//!
//! Et il ne conserve **pas** un `NotTrusted`. La raison est dans le contrat de [`Proven`] : vide
//! veut dire « aucune campagne n'a conclu », jamais « la campagne a conclu que non ». Les deux se
//! distinguent déjà par `Standing::NotTrusted`, que `proven_level` ignore — un enregistrement qui le
//! porterait ne changerait aucun placement, et laisserait croire qu'il le pourrait.

use std::collections::BTreeMap;

use locus_execution::SandboxLevel;
use locus_execution::selftest::Standing;
use serde::{Deserialize, Serialize};

use crate::announced::Proven;
use crate::linux::HostFacts;

/// La variable qui dit où les attestations sont conservées.
pub const RECORD_ENV: &str = "LOCUS_EXECD_ATTESTATIONS";

/// Ce qu'une campagne a conclu, pour un worker et sur un hôte donné.
///
/// # Le niveau voyage en **code**, pas en type
///
/// `SandboxLevel` ne dérive ni `Serialize` ni `Deserialize`, et ce n'est pas un oubli à corriger
/// ici : `packages/execution` porte le domaine, et les formes de fil vivent dans `packages/lep`.
/// La forme sur disque est l'affaire de ce module, pas celle du domaine.
///
/// `code()` et `parse()` existent déjà et se répondent — la relecture d'un code inconnu rend
/// `None`, donc un fichier qui nommerait « S9 » est **refusé**, pas rangé dans une valeur voisine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    /// Le worker dont la campagne parle.
    pub worker_id: String,
    /// Le niveau qu'elle a tenu, sous le code de §21.6 — `"S2"`.
    pub level: String,
    /// L'empreinte des capacités contre lesquelles elle a conclu.
    ///
    /// Sans elle, l'enregistrement serait rejouable sur n'importe quelle machine. Voir le
    /// module.
    pub host: String,
    /// Quand elle a conclu, en millisecondes depuis l'époque.
    ///
    /// Lisible par un exploitant, et **jamais** consulté pour décider : la fraîcheur d'une
    /// attestation ne se juge pas au calendrier mais à l'identité de l'hôte, que `host` porte.
    /// Le champ existe pour qu'on puisse répondre « quand ? », pas pour périmer quoi que ce soit.
    pub concluded_at: i64,
}

/// Pourquoi un fichier d'attestations n'a pas pu être lu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRefusal {
    /// Le chemin fautif.
    pub path: String,
    /// Ce qui n'allait pas.
    pub reason: String,
}

impl std::fmt::Display for RecordRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "attestations — {} : {}", self.path, self.reason)
    }
}

/// L'empreinte d'un hôte, telle qu'une attestation s'y lie.
///
/// # Elle vient des faits **lus**, jamais de ce qu'un worker annonce
///
/// [`HostFacts`] est ce que `locus-execd` a constaté de sa propre machine ; `HostCapabilities`, lui,
/// dérive du **manifeste du worker**, c'est-à-dire de ce que le worker déclare. Lier une attestation
/// à la seconde serait circulaire : un worker qui façonnerait son manifeste pour coïncider avec un
/// enregistrement volé se ferait attester par sa propre déclaration. Toute la valeur de ce module
/// tient dans ce choix-là.
///
/// # Dérivée, jamais déclarée
///
/// Un champ que l'appelant remplirait serait une chose de plus à falsifier. Elle se calcule donc par
/// `Debug` sur les faits — grossier et exact : deux hôtes dont tous les faits coïncident sont
/// interchangeables pour ce que la campagne a éprouvé, et un fait qui change fait changer
/// l'empreinte sans que personne ait à s'en souvenir.
///
/// Le jour où [`HostFacts`] gagne un champ, l'empreinte le prend. C'est la propriété qu'une liste
/// écrite à la main perdrait au premier ajout.
#[must_use]
pub fn fingerprint(facts: &HostFacts) -> String {
    format!("{facts:?}")
}

/// Les attestations conservées, indexées par worker.
///
/// # Le défaut reste [`crate::announced::NothingProven`]
///
/// Comme les trois ports d'autorité de `locusd` : c'est le binaire qui câble la source réelle, et un
/// `locus-execd` assemblé sans elle doit continuer à **dire** qu'il ne place rien au-dessus de `S0`.
/// Un défaut qui honorerait un fichier absent en le traitant comme vide serait le même défaut sous
/// un nom plus rassurant.
#[derive(Debug, Default)]
pub struct RecordedProven {
    /// Ce que l'hôte courant rend comme empreinte, au démarrage.
    host: String,
    by_worker: BTreeMap<String, Vec<Attestation>>,
}

impl RecordedProven {
    /// Relire un fichier d'attestations pour l'hôte donné.
    ///
    /// # Errors
    ///
    /// [`RecordRefusal`] quand le fichier est nommé et ne se lit pas. Un fichier **nommé et
    /// illisible** refuse : un exploitant qui a posé la variable veut que ses attestations comptent,
    /// et démarrer sans elles le laisserait chercher pendant que ses missions reçoivent
    /// `level_not_attested`.
    pub fn read(contents: &str, path: &str, facts: &HostFacts) -> Result<Self, RecordRefusal> {
        let records: Vec<Attestation> =
            serde_json::from_str(contents).map_err(|erreur| RecordRefusal {
                path: path.to_owned(),
                reason: format!("ne se relit pas : {erreur}"),
            })?;

        let host = fingerprint(facts);
        let mut by_worker: BTreeMap<String, Vec<Attestation>> = BTreeMap::new();
        for record in records {
            // Un code inconnu **refuse le fichier entier**, plutôt que d'écarter la ligne. Écarter
            // ferait démarrer un daemon qui honore trois attestations sur quatre sans le dire, et
            // l'exploitant lirait `level_not_attested` sur la quatrième sans savoir qu'elle a été
            // jetée à la lecture.
            if SandboxLevel::parse(&record.level).is_none() {
                return Err(RecordRefusal {
                    path: path.to_owned(),
                    reason: format!(
                        "« {} » n'est pas un niveau de §21.6, pour le worker « {} »",
                        record.level, record.worker_id
                    ),
                });
            }
            by_worker
                .entry(record.worker_id.clone())
                .or_default()
                .push(record);
        }
        Ok(Self { host, by_worker })
    }

    /// Les attestations retenues pour ce worker — celles que **cet** hôte a produites.
    ///
    /// Lecture, pour les diagnostics : un exploitant qui voit `level_not_attested` alors qu'il a
    /// posé un fichier veut savoir si l'enregistrement a été écarté, et lequel.
    #[must_use]
    pub fn honoured(&self, worker_id: &str) -> Vec<&Attestation> {
        self.by_worker
            .get(worker_id)
            .map(|records| {
                records
                    .iter()
                    .filter(|record| record.host == self.host)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Combien d'attestations cet hôte retient, tous workers confondus.
    #[must_use]
    pub fn total_honoured(&self) -> usize {
        self.by_worker
            .values()
            .flatten()
            .filter(|record| record.host == self.host)
            .count()
    }

    /// Combien parlent d'un **autre** hôte — comptées, jamais tues.
    #[must_use]
    pub fn total_foreign(&self) -> usize {
        self.by_worker
            .values()
            .flatten()
            .filter(|record| record.host != self.host)
            .count()
    }

    /// Les attestations **écartées** parce qu'elles parlent d'un autre hôte.
    ///
    /// Rendues à part plutôt que tues : une attestation ignorée en silence est indiscernable d'une
    /// attestation absente, et les deux se réparent différemment — l'une en relançant la campagne
    /// ici, l'autre en posant le fichier.
    #[must_use]
    pub fn foreign(&self, worker_id: &str) -> Vec<&Attestation> {
        self.by_worker
            .get(worker_id)
            .map(|records| {
                records
                    .iter()
                    .filter(|record| record.host != self.host)
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Proven for RecordedProven {
    fn standing(&self, worker_id: &str) -> Vec<Standing> {
        self.honoured(worker_id)
            .into_iter()
            // `parse` ne peut pas échouer ici : `read` refuse le fichier entier sur un code
            // inconnu, donc tout ce qui est stocké se relit. `filter_map` plutôt qu'`expect` quand
            // même — une invariante tenue ailleurs se maintient mal par une panique.
            .filter_map(|record| {
                SandboxLevel::parse(&record.level).map(|level| Standing::Trusted { level })
            })
            .collect()
    }
}

/// Lire le fichier que l'environnement nomme, s'il en nomme un.
///
/// Trois issues, et elles ne se confondent pas :
///
/// - `Ok(None)` — rien n'est demandé. Le daemon démarre et ne place rien au-dessus de `S0`, comme
///   avant.
/// - `Ok(Some(recorded))` — un fichier est demandé, et il se lit.
/// - `Err(refusal)` — un fichier est demandé et il ne se lit pas. Le daemon ne démarre pas.
///
/// # Errors
///
/// [`RecordRefusal`] quand le fichier est nommé et introuvable, illisible, ou mal formé.
pub fn load(
    lookup: impl Fn(&str) -> Option<String>,
    facts: &HostFacts,
) -> Result<Option<RecordedProven>, RecordRefusal> {
    let Some(path) = lookup(RECORD_ENV).filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let path = path.trim();
    // Un fichier **absent** refuse aussi. C'est la différence avec une variable absente : poser le
    // chemin est une intention, et démarrer sans les attestations laisserait l'exploitant lire
    // `level_not_attested` en cherchant pourquoi son fichier n'a rien fait.
    let contents = std::fs::read_to_string(path).map_err(|erreur| RecordRefusal {
        path: path.to_owned(),
        reason: format!("ne s'ouvre pas : {erreur}"),
    })?;
    RecordedProven::read(&contents, path, facts).map(Some)
}

/// Ce que `main` a le droit d'imprimer d'un fichier lu.
///
/// Le compte des attestations honorées **et** celui des écartées. Taire les secondes ferait lire
/// « 0 attestation » à un exploitant dont le fichier en contient trois, écrites sur une autre
/// machine — et il chercherait un fichier vide au lieu d'un hôte qui a changé.
#[must_use]
pub fn annonce(recorded: &RecordedProven) -> String {
    let honorees = recorded.total_honoured();
    let etrangeres = recorded.total_foreign();
    if etrangeres == 0 {
        return format!("attestations : {honorees} retenue(s) pour cet hôte");
    }
    format!(
        "attestations : {honorees} retenue(s) pour cet hôte, {etrangeres} écartée(s) — \
         elles parlent d'un hôte différent de celui-ci"
    )
}

/// La variable qui dit où une campagne dépose ce qu'elle a conclu.
///
/// Distincte de [`RECORD_ENV`], qui dit où `locus-execd` **lit**. Les confondre ferait qu'une
/// campagne écrase le fichier qu'un daemon est en train de lire, et qu'un exploitant ne puisse plus
/// distinguer « ce que j'ai posé » de « ce que la dernière campagne a produit ».
pub const EMIT_ENV: &str = "LOCUS_EXECD_ATTESTATION_OUT";

/// Ce qu'une campagne a conclu, sous la forme qui se conserve — ou `None`.
///
/// # `NotTrusted` ne s'enregistre pas
///
/// La règle est celle du module : `proven_level` ignore un `NotTrusted`, donc l'écrire ne
/// changerait aucun placement — et laisserait croire qu'il le pourrait. L'absence d'enregistrement
/// dit déjà « rien n'est prouvé », ce qui est exactement ce qu'une campagne en échec établit.
///
/// Ce n'est pas une perte d'information : ce qu'une campagne en échec a trouvé est dans son rapport,
/// que la CI publie, et un fichier d'attestations n'est pas un journal de campagne.
#[must_use]
pub fn record(
    worker_id: &str,
    standing: &Standing,
    facts: &HostFacts,
    concluded_at: i64,
) -> Option<Attestation> {
    match standing {
        Standing::NotTrusted { .. } => None,
        Standing::Trusted { level } => Some(Attestation {
            worker_id: worker_id.to_owned(),
            level: level.code().to_owned(),
            host: fingerprint(facts),
            concluded_at,
        }),
    }
}

/// Le fichier qu'une campagne dépose, à partir de ce qu'elle a conclu.
///
/// # Errors
///
/// [`RecordRefusal`] quand la sérialisation échoue — ce qui ne devrait pas arriver sur des champs
/// de chaînes et d'entiers, et qui est rendu plutôt que masqué par un `unwrap` au motif que ça
/// n'arrive pas.
pub fn emit(records: &[Attestation], path: &str) -> Result<String, RecordRefusal> {
    serde_json::to_string_pretty(records).map_err(|erreur| RecordRefusal {
        path: path.to_owned(),
        reason: format!("ne se sérialise pas : {erreur}"),
    })
}
