//! Domain construction, AC-3 propagation, and min-entropy collapse.

use std::collections::{BTreeMap, VecDeque};

use observed_core::SplitMix;
use observed_hex::{HexCoord, HexFace};

use super::constraints::{all_coords, forced_route_edges, room_sites};
use super::trace::SolveStep;
use super::validate::layout_failure;
use super::variants::{HexVariant, catalogue, variants_compatible};
use super::{HexArchetype, HexPlacement, HexSpace, HexWfcConfig, HexWfcError};

pub(super) fn collapse(
    seed: u64,
    generation: u32,
    config: HexWfcConfig,
    mut trace: Option<&mut Vec<SolveStep>>,
) -> Result<(BTreeMap<HexCoord, HexPlacement>, u32), HexWfcError> {
    let variants = catalogue();
    let mut last_failure: Option<&'static str> = None;
    for attempt in 0..config.retry_budget {
        emit(&mut trace, SolveStep::AttemptStart { attempt });
        let mut rng = SplitMix::new(mixed(seed, generation, attempt, 0x4E8C_0FFE_D011_88AA));
        let sites = room_sites(config, &mut rng);
        let forced = forced_route_edges(config, &mut rng);
        if let Some(steps) = trace.as_deref_mut() {
            for &coord in &sites {
                steps.push(SolveStep::RoomSite { coord });
            }
            for (&coord, &required) in &forced {
                steps.push(SolveStep::ForcedCell { coord, required });
            }
        }
        let mut domains = initial_domains(config, &variants, &forced, &sites);
        if !propagate_all(config, &variants, &mut domains, &mut trace) {
            last_failure = Some("propagation contradiction");
            continue;
        }
        if !collapse_domains(config, &variants, &mut domains, &mut rng, &mut trace) {
            last_failure = Some("collapse contradiction");
            continue;
        }
        let mut placements = materialize(config, &variants, &domains);
        prune_disconnected(config, &mut placements);
        promote_hall_junction(seed, generation, config, &mut placements);

        let rooms = placements
            .values()
            .filter(|placement| placement.space == HexSpace::Room)
            .count();
        if !(config.min_rooms..=config.max_rooms).contains(&rooms) {
            last_failure = Some("room count outside configured range");
            continue;
        }
        if let Some(reason) = layout_failure(config, &placements) {
            last_failure = Some(reason);
            continue;
        }
        let halls = placements
            .values()
            .filter(|placement| placement.space == HexSpace::Hall)
            .count();
        emit(
            &mut trace,
            SolveStep::Completed {
                rooms: rooms as u16,
                halls: halls as u16,
            },
        );
        return Ok((placements, attempt + 1));
    }
    Err(HexWfcError::RetryBudgetExhausted {
        attempts: config.retry_budget,
        last_failure,
    })
}

fn emit(trace: &mut Option<&mut Vec<SolveStep>>, step: SolveStep) {
    if let Some(steps) = trace.as_deref_mut() {
        steps.push(step);
    }
}

fn initial_domains(
    config: HexWfcConfig,
    variants: &[HexVariant],
    forced: &BTreeMap<HexCoord, u8>,
    sites: &std::collections::BTreeSet<HexCoord>,
) -> Vec<Vec<usize>> {
    let grid = config.grid();
    all_coords(config)
        .map(|coord| {
            let required = forced.get(&coord).copied().unwrap_or(0);
            let is_site = sites.contains(&coord);
            variants
                .iter()
                .enumerate()
                .filter(|(_, variant)| {
                    // No door may face the lattice boundary.
                    if HexFace::LATERAL.iter().any(|&face| {
                        grid.neighbor(coord, face).is_none()
                            && variant.doors & super::lateral_bit(face) != 0
                    }) {
                        return false;
                    }
                    if variant.doors & required != required {
                        return false;
                    }
                    // Rooms exist only at seeded sites (which stay rooms);
                    // everything else is connective fabric. This makes room
                    // separation structural rather than a retry lottery.
                    if is_site {
                        return variant.space == HexSpace::Room;
                    }
                    if variant.space == HexSpace::Room {
                        return false;
                    }
                    true
                })
                .map(|(index, _)| index)
                .collect()
        })
        .collect()
}

/// AC-3 style arc consistency: retain only variants with a compatible
/// neighbor variant across every lateral face. Returns false on an emptied
/// domain (contradiction).
fn propagate_all(
    config: HexWfcConfig,
    variants: &[HexVariant],
    domains: &mut [Vec<usize>],
    trace: &mut Option<&mut Vec<SolveStep>>,
) -> bool {
    let grid = config.grid();
    let mut queue: VecDeque<usize> = (0..domains.len()).collect();
    while let Some(index) = queue.pop_front() {
        let coord = grid.coord(index);
        for face in HexFace::LATERAL {
            let Some(neighbor) = grid.neighbor(coord, face) else {
                continue;
            };
            let neighbor_index = grid.index(neighbor);
            let before = domains[neighbor_index].len();
            let source = domains[index].clone();
            domains[neighbor_index].retain(|&candidate| {
                source.iter().any(|&variant| {
                    variants_compatible(variants[variant], variants[candidate], face)
                })
            });
            let after = domains[neighbor_index].len();
            if after == 0 {
                emit(trace, SolveStep::Contradiction { coord: neighbor });
                return false;
            }
            if after < before {
                emit_narrowing(trace, variants, domains, neighbor, neighbor_index);
                queue.push_back(neighbor_index);
            }
        }
    }
    true
}

/// Emit the right trace event for a narrowed domain: `Collapsed` when it hit
/// a single variant, `DomainPruned` otherwise.
fn emit_narrowing(
    trace: &mut Option<&mut Vec<SolveStep>>,
    variants: &[HexVariant],
    domains: &[Vec<usize>],
    coord: HexCoord,
    index: usize,
) {
    let remaining = domains[index].len();
    if remaining == 1 {
        let variant = variants[domains[index][0]];
        emit(
            trace,
            SolveStep::Collapsed {
                coord,
                space: variant.space,
                archetype: variant.archetype,
                doors: variant.doors,
            },
        );
    } else {
        emit(
            trace,
            SolveStep::DomainPruned {
                coord,
                remaining: remaining as u16,
            },
        );
    }
}

/// Min-entropy observe/propagate loop. Returns false on contradiction.
fn collapse_domains(
    config: HexWfcConfig,
    variants: &[HexVariant],
    domains: &mut [Vec<usize>],
    rng: &mut SplitMix,
    trace: &mut Option<&mut Vec<SolveStep>>,
) -> bool {
    let grid = config.grid();
    loop {
        let min_size = domains.iter().map(Vec::len).filter(|&len| len > 1).min();
        let Some(min_size) = min_size else {
            return true; // fully collapsed
        };
        let candidates: Vec<usize> = (0..domains.len())
            .filter(|&index| domains[index].len() == min_size)
            .collect();
        let cell = candidates[(rng.next_u64() % candidates.len() as u64) as usize];

        let total: u64 = domains[cell]
            .iter()
            .map(|&variant| u64::from(variants[variant].weight))
            .sum();
        let picked = if total == 0 {
            domains[cell][0]
        } else {
            let mut roll = rng.next_u64() % total;
            let mut chosen = domains[cell][0];
            for &variant in &domains[cell] {
                let weight = u64::from(variants[variant].weight);
                if roll < weight {
                    chosen = variant;
                    break;
                }
                roll -= weight;
            }
            chosen
        };
        domains[cell] = vec![picked];
        let variant = variants[picked];
        emit(
            trace,
            SolveStep::Collapsed {
                coord: grid.coord(cell),
                space: variant.space,
                archetype: variant.archetype,
                doors: variant.doors,
            },
        );
        if !propagate_all(config, variants, domains, trace) {
            return false;
        }
    }
}

fn materialize(
    config: HexWfcConfig,
    variants: &[HexVariant],
    domains: &[Vec<usize>],
) -> BTreeMap<HexCoord, HexPlacement> {
    let grid = config.grid();
    domains
        .iter()
        .enumerate()
        .map(|(index, domain)| {
            debug_assert_eq!(domain.len(), 1);
            let variant = variants[domain[0]];
            let coord = grid.coord(index);
            (
                coord,
                HexPlacement {
                    coord,
                    space: variant.space,
                    archetype: variant.archetype,
                    doors: variant.doors,
                },
            )
        })
        .collect()
}

/// Void every cell unreachable from spawn (their doors pointed only at each
/// other, so edge consistency survives).
fn prune_disconnected(config: HexWfcConfig, placements: &mut BTreeMap<HexCoord, HexPlacement>) {
    let keep = super::topology::active_component(config, placements, config.spawn());
    for (coord, placement) in placements.iter_mut() {
        if placement.space != HexSpace::Void && !keep.contains(coord) {
            placement.space = HexSpace::Void;
            placement.archetype = HexArchetype::Void;
            placement.doors = 0;
        }
    }
}

/// Deterministically promote one 3-4 door room (never spawn/exit) into a
/// junction hall so multi-exit halls exist in fresh solves.
fn promote_hall_junction(
    seed: u64,
    generation: u32,
    config: HexWfcConfig,
    placements: &mut BTreeMap<HexCoord, HexPlacement>,
) {
    let rooms = placements
        .values()
        .filter(|placement| placement.space == HexSpace::Room)
        .count();
    if rooms <= config.min_rooms {
        return;
    }
    let candidate = placements
        .values()
        .filter(|placement| {
            placement.space == HexSpace::Room
                && placement.coord != config.spawn()
                && placement.coord != config.exit()
                && (3..=4).contains(&placement.doors.count_ones())
        })
        .min_by_key(|placement| mixed(seed, generation, 0, coord_key(placement.coord)));
    let Some(coord) = candidate.map(|placement| placement.coord) else {
        return;
    };
    let placement = placements.get_mut(&coord).expect("candidate exists");
    placement.space = HexSpace::Hall;
    placement.archetype = HexArchetype::Junction;
}

fn coord_key(coord: HexCoord) -> u64 {
    u64::from(coord.q) | (u64::from(coord.r) << 16) | (u64::from(coord.level) << 32)
}

fn mixed(seed: u64, generation: u32, attempt: u32, domain: u64) -> u64 {
    let mut rng = SplitMix::new(
        seed ^ u64::from(generation).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(attempt).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ domain.wrapping_mul(0x94D0_49BB_1331_11EB),
    );
    rng.next_u64()
}
