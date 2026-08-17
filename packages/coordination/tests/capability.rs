//! Test de sortie de W13.c, première moitié — **la capacité effective est l'intersection des
//! quatre sources de §14.2, jamais leur union, et ignorer une source fait rougir.**
//!
//! §14.2 : « une instance n'hérite **jamais** tacitement des permissions du modèle ou du worker.
//! Les capacités effectives sont l'intersection de la mission, du template, de la politique locale
//! et de l'attestation du worker. »
//!
//! Sous l'union, une politique locale permissive suffirait à rendre un outil accessible à une
//! mission qui ne l'a jamais demandé, et l'attestation d'un worker deviendrait une source de
//! droits au lieu d'être une borne. C'est cette inversion que le test rend impossible.

use std::collections::BTreeSet;

use locus_coordination::{Capability, CapabilityError, Source, Sources, capabilities};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// L'univers des capacités du test. Petit et fixé : les 2⁴ = 16 combinaisons d'appartenance de
/// chacune sont énumérables **exhaustivement**, ce qui vaut mieux qu'un tirage aléatoire dont on
/// ne saurait pas s'il a couvert le cas qui compte.
const UNIVERSE: [&str; 4] = ["read_corpus", "run_python", "network_egress", "gpu"];

fn capability(name: &str) -> Capability {
    Capability::new(name).expect("nom non vide")
}

/// Les quatre sources décrites par un masque de bits par capacité.
///
/// `masks[i]` porte, pour la capacité `UNIVERSE[i]`, un bit par source dans l'ordre de
/// [`Source::ALL`]. Toutes les configurations possibles sont ainsi énumérables.
fn sources_from(masks: [u8; 4]) -> Sources {
    let mut sets: [BTreeSet<Capability>; 4] = Default::default();
    for (index, name) in UNIVERSE.iter().enumerate() {
        for (bit, set) in sets.iter_mut().enumerate() {
            if masks[index] & (1 << bit) != 0 {
                set.insert(capability(name));
            }
        }
    }
    let [mission, template, local, worker] = sets;
    Sources::new(mission, template, local, worker)
}

/// L'intersection, calculée indépendamment de l'implémentation : une capacité est effective quand
/// ses quatre bits sont posés.
fn expected(masks: [u8; 4]) -> BTreeSet<Capability> {
    UNIVERSE
        .iter()
        .enumerate()
        .filter(|(index, _)| masks[*index] == 0b1111)
        .map(|(_, name)| capability(name))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// L'intersection, sur toutes les configurations
// ---------------------------------------------------------------------------------------------

/// Les 16⁴ = 65 536 configurations des quatre capacités sur les quatre sources.
///
/// Ce n'est pas un échantillon : c'est **tout** l'espace. Une propriété vérifiée partout n'a pas
/// de cas restant où elle serait fausse, et c'est ce qui distingue ce test d'un tirage qui aurait
/// pu ne jamais produire la configuration intéressante.
#[test]
fn la_capacite_effective_est_l_intersection_sur_toutes_les_configurations() {
    for a in 0..16_u8 {
        for b in 0..16_u8 {
            for c in 0..16_u8 {
                for d in 0..16_u8 {
                    let masks = [a, b, c, d];
                    let sources = sources_from(masks);
                    assert_eq!(
                        sources.effective(),
                        expected(masks),
                        "configuration {masks:?}"
                    );
                }
            }
        }
    }
}

/// Et jamais l'union : dès qu'une source retient une capacité, elle n'est pas effective.
#[test]
fn une_seule_source_qui_retient_suffit_a_refuser() {
    for withholding in Source::ALL {
        let mut sets: [BTreeSet<Capability>; 4] = Default::default();
        for (index, source) in Source::ALL.into_iter().enumerate() {
            if source != withholding {
                sets[index].insert(capability("gpu"));
            }
        }
        let [mission, template, local, worker] = sets;
        let sources = Sources::new(mission, template, local, worker);

        assert!(
            sources.effective().is_empty(),
            "« {withholding} » retient `gpu` et pourtant elle passe : c'est l'union, pas \
             l'intersection"
        );
        assert_eq!(
            sources.withholding(&capability("gpu")),
            vec![withholding],
            "le refus doit nommer sa source : « le worker ne peut pas » n'appelle pas la même \
             suite que « la mission ne l'a pas demandée »"
        );
    }
}

/// La forme que prend « ignorer une source » : le calcul n'en regarde que trois. Ce test tient la
/// place de la mutation, dans le code du test lui-même — il montre qu'un calcul incomplet **rend
/// une réponse différente**, donc que la vérification a prise sur lui.
#[test]
fn un_calcul_qui_saute_une_source_donne_une_autre_reponse() {
    for skipped in Source::ALL {
        let mut sets: [BTreeSet<Capability>; 4] = Default::default();
        for (index, source) in Source::ALL.into_iter().enumerate() {
            if source != skipped {
                sets[index].insert(capability("run_python"));
            }
        }
        let [mission, template, local, worker] = sets;
        let sources = Sources::new(mission, template, local, worker);

        // Ce que rendrait un calcul qui oublie `skipped` : l'intersection des trois autres.
        let partial: BTreeSet<Capability> = Source::ALL
            .into_iter()
            .filter(|source| *source != skipped)
            .map(|source| sources.granted_by(source).clone())
            .reduce(|mut left, right| {
                left.retain(|capability| right.contains(capability));
                left
            })
            .unwrap_or_default();

        assert_eq!(partial, BTreeSet::from([capability("run_python")]));
        assert!(
            sources.effective().is_empty(),
            "sauter « {skipped} » accorderait `run_python` : le calcul complet doit le refuser"
        );
    }
}

#[test]
fn les_quatre_sources_de_14_2_sont_nommees_et_distinctes() {
    let slugs: Vec<&str> = Source::ALL.into_iter().map(Source::slug).collect();
    assert_eq!(
        slugs,
        vec!["mission", "template", "local_policy", "worker_attestation"],
        "§14.2 les nomme ; un cinquième nom ou un nom manquant change ce que « intersection » veut \
         dire"
    );
    assert_eq!(
        slugs.iter().collect::<BTreeSet<_>>().len(),
        4,
        "deux sources qui porteraient le même nom se confondraient dans un journal"
    );
}

// ---------------------------------------------------------------------------------------------
// Les cas dégénérés
// ---------------------------------------------------------------------------------------------

#[test]
fn une_source_vide_ne_laisse_rien_passer() {
    let full = capabilities(UNIVERSE).expect("noms non vides");
    let sources = Sources::new(full.clone(), full.clone(), full, BTreeSet::new());
    assert!(
        sources.effective().is_empty(),
        "un worker qui n'atteste rien ne rend rien possible, quoi que la mission demande"
    );
}

#[test]
fn tout_accorder_partout_rend_tout() {
    let full = capabilities(UNIVERSE).expect("noms non vides");
    let sources = Sources::new(full.clone(), full.clone(), full.clone(), full.clone());
    assert_eq!(sources.effective(), full);
}

#[test]
fn une_capacite_sans_nom_ne_s_accorde_ni_ne_se_refuse() {
    assert_eq!(Capability::new("  "), Err(CapabilityError::Empty));
    assert_eq!(capabilities(["ok", ""]), Err(CapabilityError::Empty));
}

#[test]
fn le_nom_d_une_capacite_est_normalise_a_la_construction() {
    // Sinon `" gpu"` et `"gpu"` seraient deux capacités, l'intersection les séparerait, et une
    // mission verrait sa capacité refusée par une espace.
    assert_eq!(capability(" gpu ").as_str(), "gpu");
}
