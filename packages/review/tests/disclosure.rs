//! Test de sortie de `W26.c` — **`Disclosure` : motif, portée, échéance, journal.**
//!
//! Quatre clauses, celles de la roadmap :
//!
//! 1. les quatre sont **exigés par le type**, et il n'existe pas de constructeur qui en laisse un de
//!    côté ;
//! 2. l'énumération des motifs **commence vide** et reçoit ici son premier — l'objection non résolue
//!    après un nombre borné de tours —, **avec le mécanisme qui le déclenche, jamais sans** ;
//! 3. « toutes les traces de cette branche » n'est **pas** une portée constructible ;
//! 4. un dévoilement **expiré ne donne plus rien**, et un test le passe de part et d'autre de
//!    l'échéance.

use locus_domain::{ContentHash, RevisionId};
use locus_memory::{Disclosed, Reader, Reading, Refusal, Trace, read};
use locus_protocol::{Id, IdKind, Timestamp, id::Agent};
use locus_review::disclosure::{Contestation, Disclosure, DisclosureError, Reason, Scope};
use locus_review::rebuttal::Rebuttal;

use locus_artifacts::ProducedBy;

const ECHEANCE: &str = "2026-08-25T18:00:00.000Z";

fn instant(iso: &str) -> Timestamp {
    Timestamp::parse(iso).expect("instant bien formé")
}

fn octroi() -> Timestamp {
    instant("2026-08-25T12:00:00.000Z")
}

/// Les fixtures d'identifiant sont celles de `tests/rebuttal.rs` — même crate, même forme.
fn id_de<K: IdKind>(graine: u8) -> Id<K> {
    let mut entropy = [0_u8; 10];
    entropy[9] = graine;
    Id::from_parts(Timestamp::from_millis(1_700_000_000_000), entropy)
        .expect("l'instant de fixture tient sur 48 bits")
}

fn agent(graine: u8) -> Id<Agent> {
    id_de(graine)
}

/// Le **texte** de l'identifiant, tel que le port le compare.
///
/// `Id<Agent>` n'est pas une chaîne libre : un nom inventé comme « `agt_brahe` » n'est pas la forme
/// que `to_string` rend, et un test qui l'utiliserait vérifierait un refus dû au mauvais motif.
fn agent_texte(graine: u8) -> String {
    agent(graine).to_string()
}

fn constat() -> RevisionId {
    id_de::<locus_domain::ids::RevisionKind>(1)
}

fn trace() -> Trace {
    let mut produite = ProducedBy::new("tsk_catalyseur", 3);
    produite.agent_id = Some("agt_kepler".to_owned());
    Trace::declaring(
        "art_raisonnement",
        ContentHash::parse(&format!("sha256:{}", "ab".repeat(32))).expect("hash bien formé"),
        4_096,
        produite,
    )
    .expect("la déclaration est bien formée")
}

/// Une réponse qui **conteste et relance** — donc un tour.
fn tour() -> Rebuttal {
    Rebuttal::to_finding(constat(), agent(2), "la mesure ne tient pas")
        .expect("réponse non vide")
        .contesting("le protocole")
        .requesting_recheck()
}

fn source(fichier: &str) -> String {
    let brut = std::fs::read_to_string(format!("{}/src/{fichier}", env!("CARGO_MANIFEST_DIR")))
        .expect("le module de production est lisible depuis son propre crate");
    brut.lines()
        .filter(|ligne| !ligne.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Le mécanisme mené jusqu'au motif, pour les tests qui ont besoin d'un dévoilement.
fn motif() -> locus_review::disclosure::Motive {
    let mut contestation = Contestation::on(constat());
    for _ in 0..3 {
        contestation = contestation.then(&tour());
    }
    contestation
        .unresolved_after(2)
        .expect("trois tours dépassent une borne de deux")
}

fn devoilement() -> Disclosure {
    Disclosure::granting(
        motif(),
        Scope::one("art_raisonnement", agent(2)),
        octroi(),
        instant(ECHEANCE),
    )
    .expect("l'échéance suit l'octroi")
    .0
}

// ---------------------------------------------------------------------------------------------
// 1. Les quatre sont exigés par le type
// ---------------------------------------------------------------------------------------------

/// **Aucun constructeur n'en laisse un de côté.**
///
/// `Disclosure::granting` prend le motif, la portée et les deux instants, et rend le fait. Tenu par
/// l'absence : pas de `new`, pas de `with_deadline` qui l'ajouterait après coup — un dévoilement sans
/// échéance aurait existé, ne serait-ce qu'un instant, et c'est un instant de trop pour une valeur
/// qui se clone.
#[test]
fn aucun_constructeur_ne_laisse_un_des_quatre_de_cote() {
    let code = source("disclosure.rs");
    for interdit in [
        "fn new(",
        "fn with_deadline",
        "fn with_scope",
        "fn with_motive",
        "fn without_deadline",
        "Default for Disclosure",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » permettrait un dévoilement incomplet"
        );
    }

    // Et le seul chemin est bien celui-là.
    let constructeurs = code.matches("    pub fn granting(").count();
    assert_eq!(constructeurs, 1, "un seul chemin vers un dévoilement");
}

/// Le quatrième — **la journalisation** — est rendu avec le dévoilement, pas à côté.
#[test]
fn le_devoilement_ecrit_un_fait_a_sa_construction() {
    let (devoile, fait) = Disclosure::granting(
        motif(),
        Scope::one("art_raisonnement", agent(2)),
        octroi(),
        instant(ECHEANCE),
    )
    .expect("l'échéance suit l'octroi");

    assert_eq!(fait.artifact_id(), "art_raisonnement");
    assert_eq!(fait.reader(), &agent(2));
    assert_eq!(fait.reason(), Reason::UnresolvedObjection);
    assert_eq!(fait.granted_at(), octroi());
    assert_eq!(fait.until(), instant(ECHEANCE));
    assert_eq!(
        devoile.until(),
        fait.until(),
        "le fait dit ce qui a été accordé"
    );
}

/// Un dévoilement **déjà expiré à sa naissance** est refusé.
///
/// Ce ne serait pas une autorisation prudente : ce serait une ligne de journal disant qu'on a
/// autorisé, sans que rien ne l'ait jamais été.
#[test]
fn un_devoilement_expire_a_sa_naissance_est_refuse() {
    for echeance in [octroi(), instant("2026-08-25T11:00:00.000Z")] {
        let refus = Disclosure::granting(
            motif(),
            Scope::one("art_raisonnement", agent(2)),
            octroi(),
            echeance,
        );
        assert!(matches!(
            refus,
            Err(DisclosureError::DeadlineNotAfterGrant { .. })
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Un motif ne s'écrit pas : il se constate
// ---------------------------------------------------------------------------------------------

/// **L'énumération a un barreau, et il a son mécanisme.**
///
/// Le décompte est ce qui porte la clause : autant de chemins produisant un motif que de barreaux.
/// Un barreau de plus sans son mécanisme ferait rougir ici, ce qui est exactement la règle du dépôt
/// — « une sorte n'entre dans son énumération que lorsqu'un consommateur exécutable et testé
/// existe ».
#[test]
fn chaque_motif_arrive_avec_son_mecanisme() {
    assert_eq!(Reason::ALL.len(), 1);
    assert_eq!(Reason::ALL[0], Reason::UnresolvedObjection);

    let code = source("disclosure.rs");
    let producteurs = code.matches("-> Option<Motive>").count() + code.matches("-> Motive").count();
    assert_eq!(
        producteurs,
        Reason::ALL.len(),
        "autant de mécanismes que de motifs : un motif sans déclencheur est une autorisation qu'on \
         peut écrire sans qu'il se soit rien passé"
    );
}

/// **Le fait lit le motif ; il ne le nomme pas.**
///
/// Trouvé par un mutant survivant : dans `granting`, `reason: motive.reason` remplacé par
/// `reason: Reason::UnresolvedObjection` ne faisait rougir aucun test. C'est un mutant **équivalent
/// aujourd'hui**, et démontrablement : `Reason::ALL` a un barreau, `Motive` n'a qu'un producteur, et
/// ce producteur pose ce barreau-là. Les deux écritures rendent la même valeur, toujours.
///
/// Le laisser ainsi aurait été exact et imprudent. Le jour où un second motif arrive — et
/// `chaque_motif_arrive_avec_son_mecanisme` force alors un second producteur —, la constante en dur
/// étiquetterait son fait avec le motif du premier : un journal qui dit qu'on a dévoilé pour une
/// raison, quand c'en était une autre. L'erreur ne se verrait nulle part.
///
/// Ce test convertit donc « équivalent aujourd'hui » en « ne peut pas régresser demain » : le corps
/// de `granting` **ne nomme aucun barreau**, il lit celui du motif qu'on lui donne.
#[test]
fn le_fait_lit_le_motif_et_ne_le_nomme_pas() {
    let code = source("disclosure.rs");
    let debut = code
        .find("    pub fn granting(")
        .expect("le constructeur existe");
    let fin = code[debut..].find("\n    }").expect("il se ferme") + debut;
    let corps = &code[debut..fin];
    assert!(
        corps.contains("motive.reason"),
        "le fait lit le motif : {corps}"
    );

    for barreau in Reason::ALL {
        let nomme = format!("Reason::{barreau:?}");
        assert!(
            !corps.contains(&nomme),
            "« {nomme} » en dur dans `granting` : le fait étiquetterait tout dévoilement du motif \
             écrit ici, quel que soit celui qu'on lui passe"
        );
    }
}

/// **`Motive` n'a aucun constructeur public.**
///
/// Un `Motive::new` aurait permis d'affirmer un conflit prolongé sans qu'aucun tour n'ait eu lieu —
/// c'est-à-dire de fabriquer le motif d'un dévoilement qu'on souhaitait accorder.
#[test]
fn un_motif_ne_s_ecrit_pas() {
    let code = source("disclosure.rs");
    let debut = code.find("impl Motive {").expect("le bloc existe");
    let fin = code[debut..].find("\n}").expect("il se ferme") + debut;
    let corps = &code[debut..fin];
    assert!(
        corps.len() > 80,
        "extraction vide : voir la règle 3 du rythme de session"
    );

    for interdit in [
        "pub fn new",
        "pub const fn new",
        "pub fn of",
        "pub fn for_reason",
    ] {
        assert!(
            !corps.contains(interdit),
            "« {interdit} » : un motif se constate, il ne s'écrit pas"
        );
    }
}

/// **Le mécanisme compte de vrais tours**, et pas des réponses.
///
/// Un tour est une réponse qui **conteste** et **demande un recheck**. Compter toutes les réponses
/// aurait fait du dialogue ordinaire un conflit prolongé, et un dévoilement se serait déclenché sur
/// une revue qui se passait bien.
#[test]
fn seule_une_reponse_qui_conteste_et_relance_ouvre_un_tour() {
    let simple =
        Rebuttal::to_finding(constat(), agent(2), "je prends note").expect("réponse non vide");
    let conteste_sans_relancer = Rebuttal::to_finding(constat(), agent(2), "je conteste")
        .expect("réponse non vide")
        .contesting("le protocole");
    let relance_sans_contester = Rebuttal::to_finding(constat(), agent(2), "peux-tu revoir ?")
        .expect("réponse non vide")
        .requesting_recheck();

    let contestation = Contestation::on(constat())
        .then(&simple)
        .then(&conteste_sans_relancer)
        .then(&relance_sans_contester);
    assert_eq!(
        contestation.rounds(),
        0,
        "trois réponses, aucun tour : aucune ne conteste et ne relance à la fois"
    );

    assert_eq!(Contestation::on(constat()).then(&tour()).rounds(), 1);
}

/// **Strictement au-delà de la borne**, et pas « à partir de ».
///
/// `bound` est le nombre de tours qu'on accepte **sans** dévoiler. Le test le passe de part et
/// d'autre : à la borne exacte, rien ; au tour suivant, le motif.
#[test]
fn le_motif_n_apparait_qu_au_dela_de_la_borne() {
    let mut contestation = Contestation::on(constat());
    assert_eq!(contestation.rounds(), 0, "zéro tour, et zéro est un fait");
    assert!(contestation.unresolved_after(2).is_none());

    contestation = contestation.then(&tour()).then(&tour());
    assert_eq!(contestation.rounds(), 2);
    assert!(
        contestation.unresolved_after(2).is_none(),
        "à la borne exacte, la contestation est encore dans ce qui était prévu"
    );

    contestation = contestation.then(&tour());
    let motif = contestation
        .unresolved_after(2)
        .expect("le tour suivant la fait sortir");
    assert_eq!(motif.reason(), Reason::UnresolvedObjection);
    assert_eq!(motif.rounds(), 3);
    assert_eq!(motif.finding(), &constat());
}

// ---------------------------------------------------------------------------------------------
// 3. « Toutes les traces de cette branche » n'est pas une portée
// ---------------------------------------------------------------------------------------------

/// **Une trace, un lecteur, et rien de plus large.**
///
/// Ce n'est pas un filtre qu'on aurait restreint : il n'y a pas de forme plus large à écrire. Le test
/// d'absence refuse le vocabulaire qui en ouvrirait une — « une politique de diffusion déguisée en
/// autorisation ponctuelle », dit l'ADR.
#[test]
fn aucune_portee_plus_large_qu_une_trace_et_un_lecteur() {
    let code = source("disclosure.rs");
    let debut = code
        .find("pub struct Scope {")
        .expect("la structure existe");
    let fin = code[debut..].find("\n}").expect("elle se ferme") + debut;
    let champs = &code[debut..fin];
    assert!(champs.len() > 40, "extraction vide : voir la règle 3");

    for interdit in [
        "Vec<", "branch", "Branch", "prefix", "pattern", "glob", "all", "All",
    ] {
        assert!(
            !champs.contains(interdit),
            "« {interdit} » dans `Scope` : une portée de branche est une politique de diffusion"
        );
    }

    for interdit in [
        "fn all(",
        "fn every(",
        "fn branch(",
        "fn any(",
        "fn wildcard(",
    ] {
        assert!(
            !code.contains(interdit),
            "« {interdit} » ouvrirait une portée plus large"
        );
    }
}

/// Une portée ne couvre **pas** une autre trace, ni un autre lecteur.
///
/// Le pendant exécutable : les deux bouts de la portée, chacun refusé séparément.
#[test]
fn une_portee_ne_couvre_ni_une_autre_trace_ni_un_autre_lecteur() {
    let devoile = devoilement();

    assert!(devoile.covers("art_raisonnement", &agent_texte(2), octroi()));
    assert!(
        !devoile.covers("art_autre", &agent_texte(2), octroi()),
        "une autre trace n'est pas couverte"
    );
    assert!(
        !devoile.covers("art_raisonnement", &agent_texte(3), octroi()),
        "un autre lecteur n'est pas couvert"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Un dévoilement expiré ne donne plus rien
// ---------------------------------------------------------------------------------------------

/// **De part et d'autre de l'échéance**, jusque dans la lecture.
///
/// Le test ne s'arrête pas à `covers` : il passe par `memory::read`, qui est là où l'expiration
/// compte. Vérifier seulement l'accesseur laisserait ouverte la possibilité que la lecture ne le
/// consulte pas.
#[test]
fn un_devoilement_expire_ne_donne_plus_rien() {
    let devoile = devoilement();
    let trace = trace();
    let pair = Reader::Peer {
        agent_id: agent_texte(2),
    };

    let avant = read(&pair, &trace, octroi(), Some(&devoile));
    assert!(
        matches!(avant, Reading::Disclosed(_)),
        "avant l'échéance, le pair lit"
    );

    let pile = read(&pair, &trace, instant(ECHEANCE), Some(&devoile));
    assert!(
        matches!(pile, Reading::Disclosed(_)),
        "à l'échéance exacte, il vaut encore"
    );

    let apres = read(
        &pair,
        &trace,
        instant("2026-08-25T18:00:00.001Z"),
        Some(&devoile),
    );
    assert_eq!(
        apres,
        Reading::Refused(Refusal::NeedsDisclosure {
            asked_by: agent_texte(2),
        }),
        "après, le pair retombe sur le refus ordinaire — et pas sur un refus d'un autre genre"
    );
}

/// Un dévoilement qui vise **une autre** trace ne débloque pas celle-ci.
///
/// Le refus est le **même** que sans dévoilement du tout, et c'est voulu : présenter un dévoilement
/// qui ne couvre pas n'est pas plus proche d'être autorisé que de n'en présenter aucun.
#[test]
fn un_devoilement_qui_ne_couvre_pas_vaut_pas_de_devoilement() {
    let ailleurs = Disclosure::granting(
        motif(),
        Scope::one("art_ailleurs", agent(2)),
        octroi(),
        instant(ECHEANCE),
    )
    .expect("l'échéance suit l'octroi")
    .0;

    let pair = Reader::Peer {
        agent_id: agent_texte(2),
    };
    let trace = trace();

    assert_eq!(
        read(&pair, &trace, octroi(), Some(&ailleurs)),
        read(&pair, &trace, octroi(), None),
        "un dévoilement qui ne couvre pas ne vaut pas mieux que rien"
    );
}

// ---------------------------------------------------------------------------------------------
// Le port n'a qu'un implémenteur, et c'est une garde et non une habitude
// ---------------------------------------------------------------------------------------------

/// **`Disclosed` n'est implémenté qu'ici, dans tout le workspace.**
///
/// L'en-tête de `memory::readers` dit la faiblesse plutôt que de la cacher : n'importe quel crate
/// peut implémenter le port et rendre `true`. Aucune signature ne l'empêche.
///
/// Ce qui la tient est ce test. Il parcourt les sources de tous les crates et exige **exactement un**
/// implémenteur — « personne d'autre ne l'implémente » devient une propriété vérifiée au lieu d'une
/// habitude. Il échoue bruyamment s'il ne trouve pas la racine du workspace : un balayage qui n'a
/// rien lu ne vaut pas zéro.
#[test]
fn le_port_n_a_qu_un_implementeur_dans_tout_le_workspace() {
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("le crate vit sous packages/, donc la racine est deux crans au-dessus")
        .to_path_buf();
    assert!(
        racine.join("Cargo.toml").is_file(),
        "racine du workspace introuvable : le balayage ne dirait rien"
    );

    let mut fichiers = 0_usize;
    let mut implementeurs = Vec::new();
    let mut a_visiter = vec![racine.clone()];
    while let Some(repertoire) = a_visiter.pop() {
        let Ok(entrees) = std::fs::read_dir(&repertoire) else {
            continue;
        };
        for entree in entrees.flatten() {
            let chemin = entree.path();
            let nom = chemin.file_name().unwrap_or_default().to_string_lossy();
            if chemin.is_dir() {
                if nom != "target" && nom != "node_modules" && !nom.starts_with('.') {
                    a_visiter.push(chemin);
                }
            } else if chemin
                .extension()
                .is_some_and(|extension| extension == "rs")
            {
                fichiers += 1;
                let Ok(contenu) = std::fs::read_to_string(&chemin) else {
                    continue;
                };
                // En **début de ligne** : la chaîne qui sert d'aiguille, elle, est toujours
                // indentée dans une expression. Sans cette précision le balayage se trouvait
                // lui-même — quatrième faux positif de l'idiome de scan de source, resserré et non
                // relâché, comme les trois autres.
                if contenu
                    .lines()
                    .any(|ligne| ligne.starts_with("impl Disclosed for"))
                {
                    implementeurs.push(
                        chemin
                            .strip_prefix(&racine)
                            .unwrap_or(&chemin)
                            .to_path_buf(),
                    );
                }
            }
        }
    }

    assert!(
        fichiers > 100,
        "{fichiers} fichiers lus : le balayage n'a pas vu le workspace, et son verdict ne vaut rien"
    );
    assert_eq!(
        implementeurs.len(),
        1,
        "un seul implémenteur du port, sinon « un pair ne lit que par un dévoilement » cesse d'être \
         vrai : {implementeurs:?}"
    );
    assert!(
        implementeurs[0].ends_with("packages/review/src/disclosure.rs"),
        "et c'est celui-ci : {implementeurs:?}"
    );
}
