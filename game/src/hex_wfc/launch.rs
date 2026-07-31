//! Prepared-launch boundary for the canonical hex facility.
//!
//! Callers finalize seed policy and [`HexMatchConfig`] before crossing this
//! boundary. That keeps match construction independent from Bevy resources and
//! makes the same operation safe to move onto a loading worker later.

use std::sync::OnceLock;

use observed_authoring::{RoomPrototype, RuntimeHexCatalog, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_match::hex_wfc::{HexMatchConfig, HexMatchError, HexWfcMatch};

const NEARBY_SEED_ATTEMPTS: u64 = 64;

/// How strictly match construction must honor the requested seed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HexSeedPolicy {
    /// Try the requested seed, then the next 63 seeds after solve contradictions.
    Nearby,
    /// Use only the requested seed and require the authoritative content hash.
    Exact { expected_content_hash: [u8; 32] },
}

/// Complete, immutable input to canonical hex-match construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HexLaunchSpec {
    pub requested_seed: u64,
    pub config: HexMatchConfig,
    pub seed_policy: HexSeedPolicy,
}

/// Fully constructed simulation plus the metadata needed by the loading/runtime seam.
#[derive(Debug)]
pub(crate) struct PreparedHexLaunch {
    pub match_state: HexWfcMatch,
    pub requested_seed: u64,
    pub selected_seed: u64,
    pub seed_offset: u64,
    pub simulation_content_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HexLaunchError {
    CatalogLoad(String),
    ContentHashMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    ExactSeedRejected {
        seed: u64,
        error: HexMatchError,
    },
    NearbySeedsExhausted {
        requested_seed: u64,
        attempts: u64,
        last_error: Option<HexMatchError>,
    },
}

impl std::fmt::Display for HexLaunchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CatalogLoad(error) => write!(formatter, "load authored hex corpus: {error}"),
            Self::ContentHashMismatch { .. } => {
                formatter.write_str("server simulation content does not match this client")
            }
            Self::ExactSeedRejected { seed, error } => {
                write!(
                    formatter,
                    "construct exact hex match for seed {seed}: {error:?}"
                )
            }
            Self::NearbySeedsExhausted {
                requested_seed,
                attempts,
                last_error,
            } => write!(
                formatter,
                "no solvable hex match in {attempts} seeds from {requested_seed}; last error: {last_error:?}"
            ),
        }
    }
}

impl std::error::Error for HexLaunchError {}

#[derive(Clone)]
pub(super) struct HexAuthoringCorpus {
    pub(super) cells: Vec<TilePrototype>,
    pub(super) rooms: Vec<RoomPrototype>,
    pub(super) simulation_content_hash: [u8; 32],
}

/// Load the current committed corpus and construct the exact simulation described by
/// `spec`. This function reads no settings or other mutable application resources.
pub(crate) fn prepare(spec: HexLaunchSpec) -> Result<PreparedHexLaunch, HexLaunchError> {
    let corpus = load_current_corpus()?;
    prepare_with_corpus(spec, &corpus)
}

pub(super) fn tile_dir() -> std::path::PathBuf {
    let installed = observed_assets::assets_root().join("tiles");
    if installed.exists() {
        installed
    } else {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/tiles")
    }
}

pub(super) fn load_current_corpus() -> Result<HexAuthoringCorpus, HexLaunchError> {
    static CORPUS: OnceLock<Result<HexAuthoringCorpus, String>> = OnceLock::new();
    CORPUS
        .get_or_init(|| {
            let register_slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
            RuntimeHexCatalog::load(&tile_dir(), &register_slugs).map(|loaded| HexAuthoringCorpus {
                cells: loaded.cells,
                rooms: loaded.rooms,
                simulation_content_hash: loaded.simulation_content_hash,
            })
        })
        .clone()
        .map_err(HexLaunchError::CatalogLoad)
}

fn prepare_with_corpus(
    spec: HexLaunchSpec,
    corpus: &HexAuthoringCorpus,
) -> Result<PreparedHexLaunch, HexLaunchError> {
    match spec.seed_policy {
        HexSeedPolicy::Exact {
            expected_content_hash,
        } => {
            if corpus.simulation_content_hash != expected_content_hash {
                return Err(HexLaunchError::ContentHashMismatch {
                    expected: expected_content_hash,
                    actual: corpus.simulation_content_hash,
                });
            }
            let match_state = HexWfcMatch::new_with_rooms(
                spec.requested_seed,
                spec.config,
                &corpus.cells,
                &corpus.rooms,
            )
            .map_err(|error| HexLaunchError::ExactSeedRejected {
                seed: spec.requested_seed,
                error,
            })?;
            Ok(finish_preparation(spec, match_state, 0, corpus))
        }
        HexSeedPolicy::Nearby => {
            let mut last_error = None;
            for seed_offset in 0..NEARBY_SEED_ATTEMPTS {
                let selected_seed = spec.requested_seed.wrapping_add(seed_offset);
                match HexWfcMatch::new_with_rooms(
                    selected_seed,
                    spec.config,
                    &corpus.cells,
                    &corpus.rooms,
                ) {
                    Ok(match_state) => {
                        return Ok(finish_preparation(spec, match_state, seed_offset, corpus));
                    }
                    Err(error) => last_error = Some(error),
                }
            }
            Err(HexLaunchError::NearbySeedsExhausted {
                requested_seed: spec.requested_seed,
                attempts: NEARBY_SEED_ATTEMPTS,
                last_error,
            })
        }
    }
}

fn finish_preparation(
    spec: HexLaunchSpec,
    mut match_state: HexWfcMatch,
    seed_offset: u64,
    corpus: &HexAuthoringCorpus,
) -> PreparedHexLaunch {
    match_state.bind_simulation_content_hash(corpus.simulation_content_hash);
    PreparedHexLaunch {
        match_state,
        requested_seed: spec.requested_seed,
        selected_seed: spec.requested_seed.wrapping_add(seed_offset),
        seed_offset,
        simulation_content_hash: corpus.simulation_content_hash,
    }
}

#[cfg(test)]
mod tests {
    use observed_facility::hex_wfc::HexWfcConfig;

    use super::*;

    fn compact_config() -> HexMatchConfig {
        HexMatchConfig {
            teams: 2,
            members_per_team: 2,
            guardian: true,
            wfc: HexWfcConfig {
                cols: 12,
                rows: 9,
                levels: 1,
                min_rooms: 4,
                max_rooms: 8,
                retry_budget: 100,
                min_room_distance: 2,
            },
        }
    }

    #[test]
    fn exact_launch_rejects_a_different_simulation_content_hash() {
        let corpus = load_current_corpus().expect("committed authoring corpus loads");
        let mut incompatible_hash = corpus.simulation_content_hash;
        incompatible_hash[0] ^= 0xFF;
        let error = prepare(HexLaunchSpec {
            requested_seed: 44,
            config: compact_config(),
            seed_policy: HexSeedPolicy::Exact {
                expected_content_hash: incompatible_hash,
            },
        })
        .expect_err("an exact LAN launch must reject different simulation content");

        assert_eq!(
            error,
            HexLaunchError::ContentHashMismatch {
                expected: incompatible_hash,
                actual: corpus.simulation_content_hash,
            }
        );
    }

    #[test]
    fn nearby_launch_matches_the_former_selection_loop_and_snapshot() {
        let corpus = load_current_corpus().expect("committed authoring corpus loads");
        let spec = HexLaunchSpec {
            requested_seed: 0xF011_FAC1_1177,
            config: compact_config(),
            seed_policy: HexSeedPolicy::Nearby,
        };
        let (mut former_match, former_offset) = (0..NEARBY_SEED_ATTEMPTS)
            .find_map(|seed_offset| {
                HexWfcMatch::new_with_rooms(
                    spec.requested_seed.wrapping_add(seed_offset),
                    spec.config,
                    &corpus.cells,
                    &corpus.rooms,
                )
                .ok()
                .map(|match_state| (match_state, seed_offset))
            })
            .expect("compact corpus has a solvable nearby seed");
        former_match.bind_simulation_content_hash(corpus.simulation_content_hash);

        let first = prepare_with_corpus(spec, &corpus).expect("first prepared launch");
        let second = prepare_with_corpus(spec, &corpus).expect("second prepared launch");

        assert_eq!(first.requested_seed, spec.requested_seed);
        assert_eq!(first.seed_offset, former_offset);
        assert_eq!(
            first.selected_seed,
            spec.requested_seed.wrapping_add(former_offset)
        );
        assert_eq!(
            first.simulation_content_hash,
            corpus.simulation_content_hash
        );
        assert_eq!(first.match_state.snapshot(), former_match.snapshot());
        assert_eq!(first.match_state.snapshot(), second.match_state.snapshot());
    }
}
