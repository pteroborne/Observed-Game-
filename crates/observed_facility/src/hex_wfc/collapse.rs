//! Domain construction, AC-3 propagation, and min-entropy collapse.

use std::collections::{BTreeMap, VecDeque};

use observed_core::SplitMix;
use observed_hex::{HexCoord, HexFace, PortClass, PortSignature};

use super::blueprint::StampedBlueprint;
use super::constraints::{all_coords, forced_route_edges, stamp_blueprints, stamped_signatures};
use super::trace::SolveStep;
use super::validate::layout_failure;
use super::variants::{HexVariant, catalogue, variants_compatible};
use super::{HexArchetype, HexPlacement, HexSpace, HexWfcConfig, HexWfcError};

type CollapseOutput = (BTreeMap<HexCoord, HexPlacement>, Vec<StampedBlueprint>, u32);

pub(super) fn collapse(
    seed: u64,
    generation: u32,
    config: HexWfcConfig,
    mut trace: Option<&mut Vec<SolveStep>>,
) -> Result<CollapseOutput, HexWfcError> {
    let variants = catalogue();
    let mut last_failure: Option<&'static str> = None;
    for attempt in 0..config.retry_budget {
        emit(&mut trace, SolveStep::AttemptStart { attempt });
        let mut rng = SplitMix::new(mixed(seed, generation, attempt, 0x4E8C_0FFE_D011_88AA));

        let blueprints = stamp_blueprints(config, &mut rng);
        let signatures = stamped_signatures(config, &blueprints);
        let Some((forced_doors, forced_up, forced_down)) =
            forced_route_edges(config, &blueprints, &signatures, &mut rng)
        else {
            last_failure = Some("blueprints blocked every forced route");
            continue;
        };

        if let Some(steps) = trace.as_deref_mut() {
            for stamped in &blueprints {
                for &coord in &stamped.cells {
                    steps.push(SolveStep::BlueprintCell {
                        coord,
                        role: stamped.role,
                    });
                }
            }
            for (&coord, &required) in &forced_doors {
                steps.push(SolveStep::ForcedCell { coord, required });
            }
        }

        let mut domains = initial_domains(
            config,
            &variants,
            &forced_doors,
            &forced_up,
            &forced_down,
            &signatures,
        );

        // Blueprint cells are pre-collapsed to their exact signature; replay
        // them as Collapsed so the lab shows the stamp before hall collapse.
        if let Some(steps) = trace.as_deref_mut() {
            let grid = config.grid();
            for &coord in signatures.keys() {
                let domain = &domains[grid.index(coord)];
                if let [only] = domain[..] {
                    let variant = variants[only];
                    steps.push(SolveStep::Collapsed {
                        coord,
                        space: variant.space,
                        archetype: variant.archetype,
                        doors: variant.doors,
                        up: variant.up,
                        down: variant.down,
                    });
                }
            }
        }

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

        if let Some(reason) = layout_failure(config, &placements, &blueprints) {
            last_failure = Some(reason);
            continue;
        }
        let rooms = placements
            .values()
            .filter(|placement| placement.space == HexSpace::Room)
            .count();
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
        return Ok((placements, blueprints, attempt + 1));
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

fn signature_doors(signature: PortSignature) -> u8 {
    let mut mask = 0u8;
    for face in HexFace::LATERAL {
        if signature.port(face) == PortClass::Door {
            mask |= super::lateral_bit(face);
        }
    }
    mask
}

/// The starting domain of every cell: boundary-safe variants that satisfy the
/// forced route, with blueprint cells pinned to their exact stamped signature
/// and rooms excluded everywhere else.
fn initial_domains(
    config: HexWfcConfig,
    variants: &[HexVariant],
    forced_doors: &BTreeMap<HexCoord, u8>,
    forced_up: &BTreeMap<HexCoord, PortClass>,
    forced_down: &BTreeMap<HexCoord, PortClass>,
    signatures: &BTreeMap<HexCoord, PortSignature>,
) -> Vec<Vec<usize>> {
    let grid = config.grid();
    all_coords(config)
        .map(|coord| {
            let required_doors = forced_doors.get(&coord).copied().unwrap_or(0);
            let required_up = forced_up.get(&coord).copied().unwrap_or(PortClass::Sealed);
            let required_down = forced_down
                .get(&coord)
                .copied()
                .unwrap_or(PortClass::Sealed);
            let blueprint_signature = signatures.get(&coord);

            variants
                .iter()
                .enumerate()
                .filter(|(_, variant)| {
                    // No opening may face the lattice boundary.
                    if HexFace::ALL.iter().any(|&face| {
                        grid.neighbor(coord, face).is_none()
                            && ((face.is_lateral()
                                && variant.doors & super::lateral_bit(face) != 0)
                                || (face == HexFace::Up && variant.up != PortClass::Sealed)
                                || (face == HexFace::Down && variant.down != PortClass::Sealed))
                    }) {
                        return false;
                    }
                    if variant.doors & required_doors != required_doors {
                        return false;
                    }
                    if required_up != PortClass::Sealed && variant.up != required_up {
                        return false;
                    }
                    if required_down != PortClass::Sealed && variant.down != required_down {
                        return false;
                    }
                    match blueprint_signature {
                        // Blueprint cells collapse to exactly the stamped
                        // exterior signature.
                        Some(&signature) => {
                            variant.space == HexSpace::Room
                                && variant.doors == signature_doors(signature)
                                && variant.up == signature.port(HexFace::Up)
                                && variant.down == signature.port(HexFace::Down)
                        }
                        // Rooms exist only inside blueprint footprints;
                        // everything else is connective fabric.
                        None => variant.space != HexSpace::Room,
                    }
                })
                .map(|(index, _)| index)
                .collect()
        })
        .collect()
}

/// AC-3 style arc consistency over all eight faces: retain only variants with
/// a compatible neighbor variant across every face. Returns false on an
/// emptied domain (contradiction).
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
        for face in HexFace::ALL {
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
                up: variant.up,
                down: variant.down,
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
                up: variant.up,
                down: variant.down,
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
                    up: variant.up,
                    down: variant.down,
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
            placement.up = PortClass::Sealed;
            placement.down = PortClass::Sealed;
        }
    }
}

fn mixed(seed: u64, generation: u32, attempt: u32, domain: u64) -> u64 {
    let mut rng = SplitMix::new(
        seed ^ u64::from(generation).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(attempt).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ domain.wrapping_mul(0x94D0_49BB_1331_11EB),
    );
    rng.next_u64()
}
