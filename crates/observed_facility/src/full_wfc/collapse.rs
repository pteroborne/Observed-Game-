//! WFC domain construction, propagation, and collapse; layout validation
//! lives in [`super::validate`].

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_content::ArchitectureRegister;
use observed_core::SplitMix;

use super::config::AttemptRange;
use super::constraints::{all_coords, forced_route_edges};
use super::topology::{active_component, pinned_cells};
use super::validate::layout_valid;
use super::{
    CellCoord, FullWfcConfig, FullWfcError, ModuleArchetype, ModuleFace, ModuleInstanceId,
    ModulePlacement, ModuleSpace, ObservationFrame,
};

#[derive(Clone, Copy, Debug)]
struct Variant {
    space: ModuleSpace,
    archetype: ModuleArchetype,
    openings: u8,
    weight: u32,
}

fn catalogue() -> Vec<Variant> {
    let mut variants = vec![Variant {
        space: ModuleSpace::Void,
        archetype: ModuleArchetype::Void,
        openings: 0,
        weight: 4,
    }];
    for mask in 1u8..64 {
        let degree = mask.count_ones();
        let weight = match degree {
            1 => 4,
            2 => 3,
            3 => 2,
            _ => 1,
        };
        variants.push(Variant {
            space: ModuleSpace::Room,
            archetype: ModuleArchetype::Room,
            openings: mask,
            weight,
        });
        if degree == 2 {
            let archetype = hall_archetype(mask);
            variants.push(Variant {
                space: ModuleSpace::Hall,
                archetype,
                openings: mask,
                weight: 5,
            });
            if archetype == ModuleArchetype::Straight {
                variants.push(Variant {
                    space: ModuleSpace::Hall,
                    archetype: ModuleArchetype::Gantry,
                    openings: mask,
                    weight: 1,
                });
            }
        } else if (3..=4).contains(&degree) {
            // Multi-exit hall junctions are authored by the deterministic promotion
            // pass below. Keeping their variants in the domain lets an observed
            // junction survive relayout without making random branch networks.
            variants.push(Variant {
                space: ModuleSpace::Hall,
                archetype: ModuleArchetype::Junction,
                openings: mask,
                weight: 0,
            });
        }
    }
    variants
}

fn hall_archetype(mask: u8) -> ModuleArchetype {
    if mask.count_ones() >= 3 {
        return ModuleArchetype::Junction;
    }
    let vertical = mask & (ModuleFace::Up.bit() | ModuleFace::Down.bit());
    if vertical == ModuleFace::Up.bit() | ModuleFace::Down.bit() {
        ModuleArchetype::Shaft
    } else if vertical != 0 {
        ModuleArchetype::Climb
    } else if mask == ModuleFace::East.bit() | ModuleFace::West.bit()
        || mask == ModuleFace::North.bit() | ModuleFace::South.bit()
    {
        ModuleArchetype::Straight
    } else {
        ModuleArchetype::Corner
    }
}

pub(super) fn collapse(
    seed: u64,
    generation: u32,
    config: FullWfcConfig,
    previous: Option<&BTreeMap<CellCoord, ModulePlacement>>,
    observation: &ObservationFrame,
    reserved_exit_faces: &BTreeSet<ModuleFace>,
) -> Result<(BTreeMap<CellCoord, ModulePlacement>, u32), FullWfcError> {
    collapse_attempts(
        seed,
        generation,
        config,
        previous,
        observation,
        reserved_exit_faces,
        AttemptRange {
            start: 0,
            budget: config.retry_budget,
        },
    )
}

pub(super) fn collapse_attempts(
    seed: u64,
    generation: u32,
    config: FullWfcConfig,
    previous: Option<&BTreeMap<CellCoord, ModulePlacement>>,
    observation: &ObservationFrame,
    reserved_exit_faces: &BTreeSet<ModuleFace>,
    attempts: AttemptRange,
) -> Result<(BTreeMap<CellCoord, ModulePlacement>, u32), FullWfcError> {
    let variants = catalogue();
    let previous_pins = previous.map(|layout| pinned_cells(config, layout, observation));
    let attempt_end = attempts
        .start
        .saturating_add(attempts.budget)
        .min(config.retry_budget);

    for attempt in attempts.start..attempt_end {
        let attempt_seed = mixed(seed, generation, attempt, 0xF011_C011_A95E_0001);
        let mut rng = SplitMix::new(attempt_seed);
        let forced_edges = forced_route_edges(
            config,
            observation,
            previous,
            previous_pins.as_ref(),
            reserved_exit_faces,
            &mut rng,
        );
        let mut domains = initial_domains(
            config,
            &variants,
            previous,
            previous_pins.as_ref(),
            &forced_edges,
        );
        if !propagate_all(config, &variants, &mut domains) {
            continue;
        }
        if !collapse_domains(config, &variants, &mut domains, &mut rng) {
            continue;
        }
        let mut placements = materialize(
            seed,
            generation,
            config,
            &variants,
            &domains,
            previous,
            previous_pins.as_ref(),
        );
        prune_disconnected(config, &mut placements);
        if previous.is_none() {
            promote_hall_junction(seed, config, &mut placements);
        }

        let room_count = placements
            .values()
            .filter(|placement| placement.space == ModuleSpace::Room)
            .count();
        if !(config.min_rooms..=config.max_rooms).contains(&room_count)
            || !layout_valid(config, &placements, observation, reserved_exit_faces)
        {
            continue;
        }
        return Ok((placements, attempt + 1));
    }

    Err(FullWfcError::RetryBudgetExhausted {
        attempts: attempt_end,
    })
}

fn initial_domains(
    config: FullWfcConfig,
    variants: &[Variant],
    previous: Option<&BTreeMap<CellCoord, ModulePlacement>>,
    pins: Option<&BTreeSet<CellCoord>>,
    forced: &BTreeMap<CellCoord, u8>,
) -> Vec<Vec<usize>> {
    all_coords(config)
        .map(|coord| {
            let required = forced.get(&coord).copied().unwrap_or(0);
            let previous_placement = previous.and_then(|layout| layout.get(&coord));
            let pinned = pins.is_some_and(|pins| pins.contains(&coord));
            variants
                .iter()
                .enumerate()
                .filter(|(_, variant)| {
                    if ModuleFace::ALL.iter().any(|&face| {
                        config.neighbor(coord, face).is_none() && variant.openings & face.bit() != 0
                    }) {
                        return false;
                    }
                    if variant.openings & required != required {
                        return false;
                    }
                    if coord == config.spawn() || coord == config.exit() {
                        return variant.space == ModuleSpace::Room;
                    }
                    if required.count_ones() > 2 && variant.space != ModuleSpace::Room {
                        return false;
                    }
                    if variant.space == ModuleSpace::Hall
                        && variant.openings.count_ones() > 2
                        && !(pinned
                            && previous_placement.is_some_and(|previous| {
                                previous.space == ModuleSpace::Hall
                                    && previous.archetype == ModuleArchetype::Junction
                            }))
                    {
                        return false;
                    }
                    if !pinned {
                        return true;
                    }
                    let Some(previous) = previous_placement else {
                        return true;
                    };
                    match previous.space {
                        ModuleSpace::Room => variant.space == ModuleSpace::Room,
                        ModuleSpace::Hall => {
                            variant.space == ModuleSpace::Hall
                                && variant.openings == previous.openings
                                && variant.archetype == previous.archetype
                        }
                        ModuleSpace::Void => variant.space == ModuleSpace::Void,
                    }
                })
                .map(|(index, _)| index)
                .collect()
        })
        .collect()
}

fn propagate_all(config: FullWfcConfig, variants: &[Variant], domains: &mut [Vec<usize>]) -> bool {
    let mut queue: VecDeque<usize> = (0..domains.len()).collect();
    while let Some(index) = queue.pop_front() {
        let coord = config.coord(index);
        for face in ModuleFace::ALL {
            let Some(neighbor) = config.neighbor(coord, face) else {
                continue;
            };
            let neighbor_index = config.index(neighbor);
            let before = domains[neighbor_index].len();
            let source = domains[index].clone();
            domains[neighbor_index].retain(|&candidate| {
                source
                    .iter()
                    .any(|&other| variants_compatible(variants[other], variants[candidate], face))
            });
            if domains[neighbor_index].is_empty() {
                return false;
            }
            if domains[neighbor_index].len() != before {
                queue.push_back(neighbor_index);
            }
        }
    }
    true
}

fn variants_compatible(a: Variant, b: Variant, face: ModuleFace) -> bool {
    let a_open = a.openings & face.bit() != 0;
    let b_open = b.openings & face.opposite().bit() != 0;
    if a_open != b_open {
        return false;
    }
    if !a_open {
        return true;
    }
    !(a.space == ModuleSpace::Room && b.space == ModuleSpace::Room)
        && a.space != ModuleSpace::Void
        && b.space != ModuleSpace::Void
}

fn collapse_domains(
    config: FullWfcConfig,
    variants: &[Variant],
    domains: &mut [Vec<usize>],
    rng: &mut SplitMix,
) -> bool {
    loop {
        let Some(minimum) = domains
            .iter()
            .filter(|domain| domain.len() > 1)
            .map(Vec::len)
            .min()
        else {
            return domains.iter().all(|domain| domain.len() == 1);
        };
        let choices: Vec<usize> = domains
            .iter()
            .enumerate()
            .filter_map(|(index, domain)| (domain.len() == minimum).then_some(index))
            .collect();
        let cell = choices[rng.below(choices.len())];
        let total: u64 = domains[cell]
            .iter()
            .map(|&variant| u64::from(variants[variant].weight))
            .sum();
        let mut roll = rng.next_u64() % total.max(1);
        let selected = domains[cell]
            .iter()
            .copied()
            .find(|&variant| {
                let weight = u64::from(variants[variant].weight);
                if roll < weight {
                    true
                } else {
                    roll -= weight;
                    false
                }
            })
            .unwrap_or(domains[cell][0]);
        domains[cell] = vec![selected];
        if !propagate_all(config, variants, domains) {
            return false;
        }
    }
}

fn materialize(
    seed: u64,
    generation: u32,
    config: FullWfcConfig,
    variants: &[Variant],
    domains: &[Vec<usize>],
    previous: Option<&BTreeMap<CellCoord, ModulePlacement>>,
    pins: Option<&BTreeSet<CellCoord>>,
) -> BTreeMap<CellCoord, ModulePlacement> {
    all_coords(config)
        .enumerate()
        .map(|(index, coord)| {
            let variant = variants[domains[index][0]];
            let prior = previous.and_then(|layout| layout.get(&coord));
            let pinned = pins.is_some_and(|pins| pins.contains(&coord));
            let retain_room = pinned
                && prior.is_some_and(|placement| {
                    placement.space == ModuleSpace::Room && variant.space == ModuleSpace::Room
                });
            let retain_exact = pinned
                && prior.is_some_and(|placement| {
                    placement.space == variant.space
                        && placement.archetype == variant.archetype
                        && (placement.space == ModuleSpace::Room
                            || placement.openings == variant.openings)
                });
            let architecture =
                if retain_room || retain_exact || coord == config.spawn() || coord == config.exit()
                {
                    prior.map_or_else(
                        || architecture_for(seed, coord, generation),
                        |placement| placement.architecture,
                    )
                } else {
                    architecture_for(seed, coord, generation)
                };
            let instance =
                if retain_room || retain_exact || coord == config.spawn() || coord == config.exit()
                {
                    prior.map_or_else(
                        || instance_for(seed, coord, generation),
                        |placement| placement.instance,
                    )
                } else {
                    instance_for(seed, coord, generation)
                };
            (
                coord,
                ModulePlacement {
                    coord,
                    space: variant.space,
                    archetype: variant.archetype,
                    openings: variant.openings,
                    architecture,
                    instance,
                },
            )
        })
        .collect()
}

fn architecture_for(seed: u64, coord: CellCoord, generation: u32) -> ArchitectureRegister {
    let key = mixed(
        seed,
        generation,
        u32::from(coord.x) | (u32::from(coord.z) << 16),
        u64::from(coord.level),
    );
    ArchitectureRegister::ALL[key as usize % ArchitectureRegister::ALL.len()]
}

fn instance_for(seed: u64, coord: CellCoord, generation: u32) -> ModuleInstanceId {
    ModuleInstanceId {
        generation,
        key: mixed(
            seed,
            generation,
            u32::from(coord.x) | (u32::from(coord.z) << 16),
            u64::from(coord.level) ^ 0x1D_51A7CE,
        ),
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

fn prune_disconnected(
    config: FullWfcConfig,
    placements: &mut BTreeMap<CellCoord, ModulePlacement>,
) {
    let keep = active_component(config, placements, config.spawn());
    for (coord, placement) in placements.iter_mut() {
        if placement.space != ModuleSpace::Void && !keep.contains(coord) {
            placement.space = ModuleSpace::Void;
            placement.archetype = ModuleArchetype::Void;
            placement.openings = 0;
        }
    }
}

fn promote_hall_junction(
    seed: u64,
    config: FullWfcConfig,
    placements: &mut BTreeMap<CellCoord, ModulePlacement>,
) {
    let room_count = placements
        .values()
        .filter(|placement| placement.space == ModuleSpace::Room)
        .count();
    if room_count <= config.min_rooms {
        return;
    }
    let candidate = placements
        .values()
        .filter(|placement| {
            placement.space == ModuleSpace::Room
                && placement.coord != config.spawn()
                && placement.coord != config.exit()
                && (3..=4).contains(&placement.openings.count_ones())
        })
        .min_by_key(|placement| mixed(seed, 0, 0, placement.instance.key));
    let Some(coord) = candidate.map(|placement| placement.coord) else {
        return;
    };
    let placement = placements.get_mut(&coord).expect("candidate exists");
    placement.space = ModuleSpace::Hall;
    placement.archetype = ModuleArchetype::Junction;
}
