//! Initial physical hall choices over an already solved, connected facility.
//! The planner changes no topology, protection, revision or team knowledge.
use crate::{
    TilePrototype,
    forge::initial_halls::{InitialHallKind, kind_for_key},
};
use observed_facility::hex_wfc::{
    HexArchetype, HexCompositionProfile, HexSpace, HexWfcWorld, placement_tile_archetype,
};
use observed_hex::{HexCoord, HexFace, lateral_distance};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialHallComposition {
    pub register: &'static str,
    pub kind: InitialHallKind,
    /// A connected, ordered three-cell approach/reveal, all ordinary mutable halls.
    pub cells: Vec<HexCoord>,
}

fn neighbors(world: &HexWfcWorld, cell: HexCoord) -> impl Iterator<Item = HexCoord> + '_ {
    HexFace::LATERAL.into_iter().filter_map(move |face| {
        let next = world.config.grid().neighbor(cell, face)?;
        (world.placements[&cell].is_open(face) && world.placements[&next].is_open(face.opposite()))
            .then_some(next)
    })
}

fn distances(world: &HexWfcWorld, source: HexCoord) -> BTreeMap<HexCoord, usize> {
    let mut out = BTreeMap::from([(source, 0)]);
    let mut queue = VecDeque::from([source]);
    while let Some(cell) = queue.pop_front() {
        let distance = out[&cell] + 1;
        for next in neighbors(world, cell) {
            if let std::collections::btree_map::Entry::Vacant(entry) = out.entry(next) {
                entry.insert(distance);
                queue.push_back(next);
            }
        }
    }
    out
}

fn path(
    world: &HexWfcWorld,
    anchor: HexCoord,
    eligible: &BTreeMap<HexCoord, u16>,
    used: &BTreeSet<HexCoord>,
) -> Option<Vec<HexCoord>> {
    let mut next = neighbors(world, anchor)
        .filter(|cell| eligible.contains_key(cell) && !used.contains(cell))
        .collect::<Vec<_>>();
    next.sort_by_key(|&cell| (world.tile_variation_key(cell), cell));
    next.into_iter().find_map(|middle| {
        let mut ends = neighbors(world, middle)
            .filter(|cell| *cell != anchor && eligible.contains_key(cell) && !used.contains(cell))
            .collect::<Vec<_>>();
        ends.sort_by_key(|&cell| (world.tile_variation_key(cell), cell));
        ends.first().map(|&end| vec![anchor, middle, end])
    })
}

/// Select two distinct connected beats on each Library/Lumen floor. Module
/// choices are immutable initialization metadata, not pins; cell revisions
/// relinquish them individually after card plays or relayouts.
pub fn seed_initial_halls(
    world: &mut HexWfcWorld,
    prototypes: &[TilePrototype],
    profile: &HexCompositionProfile,
) -> Vec<InitialHallComposition> {
    if !profile.initial_hall_compositions
        || world.config.cols < 24
        || world.config.rows < 17
        || world.config.levels < 8
    {
        return Vec::new();
    }
    assert!(
        world.initial_modules.is_empty(),
        "initial composition is selected only once"
    );
    let route = world
        .route_between(world.config.spawn(), world.config.exit())
        .unwrap_or_default();
    let mut plans = Vec::new();
    let mut used = BTreeSet::new();
    for level in 0..world.config.levels {
        let register = world
            .architecture
            .iter()
            .find_map(|(cell, register)| (cell.level == level).then_some(register.slug()))
            .unwrap_or("");
        if !matches!(register, "infinite_gallery" | "overlit_grid") {
            continue;
        }
        let floor_route = route
            .iter()
            .copied()
            .filter(|cell| cell.level == level)
            .collect::<Vec<_>>();
        let Some(&entry) = floor_route.first() else {
            continue;
        };
        let departure = *floor_route.last().expect("nonempty route");
        for kind in [InitialHallKind::Gallery, InitialHallKind::Court] {
            let mut eligible = BTreeMap::new();
            for (&cell, placement) in &world.placements {
                if cell.level != level
                    || placement.space != HexSpace::Hall
                    || !matches!(
                        placement.archetype,
                        HexArchetype::Straight | HexArchetype::Corner | HexArchetype::Junction
                    )
                {
                    continue;
                }
                // A straight with four unbuilt flanks becomes a narrow span
                // in physical projection, which replaces its floor/dressing.
                // Other halls retain their authored deck, even over open air.
                if !world.sealed
                    && placement.archetype == HexArchetype::Straight
                    && HexFace::LATERAL
                        .into_iter()
                        .filter(|&face| !placement.is_open(face))
                        .all(|face| {
                            world
                                .config
                                .grid()
                                .neighbor(cell, face)
                                .and_then(|next| world.placements.get(&next))
                                .is_none_or(|next| next.space.unbuilt())
                        })
                {
                    continue;
                }
                let archetype = placement_tile_archetype(placement).expect("flat hall");
                let mut variants = prototypes
                    .iter()
                    .filter(|tile| {
                        tile.assembly.is_none()
                            && tile.key.register == register
                            && tile.key.archetype == archetype
                            && tile.signature == placement.ports()
                            && kind_for_key(&tile.key) == Some(kind)
                    })
                    .map(|tile| tile.key.variant)
                    .collect::<Vec<_>>();
                variants.sort_unstable();
                if !variants.is_empty() {
                    eligible.insert(
                        cell,
                        variants[(world.tile_variation_key(cell) % variants.len() as u64) as usize],
                    );
                }
            }
            let distance = distances(
                world,
                if kind == InitialHallKind::Gallery {
                    entry
                } else {
                    departure
                },
            );
            let gallery = plans
                .iter()
                .find(|plan: &&InitialHallComposition| {
                    plan.register == register
                        && plan.kind == InitialHallKind::Gallery
                        && plan.cells.first().is_some_and(|cell| cell.level == level)
                })
                .and_then(|plan| plan.cells.first())
                .copied();
            let mut anchors = eligible
                .keys()
                .copied()
                .filter(|cell| {
                    !used.contains(cell)
                        && (kind == InitialHallKind::Gallery
                            || (world.placements[cell].doors.count_ones() >= 3
                                && gallery
                                    .is_none_or(|gallery| lateral_distance(*cell, gallery) >= 6)))
                })
                .collect::<Vec<_>>();
            anchors.sort_by_key(|&cell| {
                (
                    distance.get(&cell).copied().unwrap_or(usize::MAX),
                    world.tile_variation_key(cell),
                    cell,
                )
            });
            if let Some(cells) = anchors
                .into_iter()
                .find_map(|anchor| path(world, anchor, &eligible, &used))
            {
                for &cell in &cells {
                    world.initial_modules.insert(cell, eligible[&cell]);
                    used.insert(cell);
                }
                plans.push(InitialHallComposition {
                    register,
                    kind,
                    cells,
                });
            }
        }
    }
    plans
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_facility::hex_wfc::HexWfcConfig;

    fn catalog() -> crate::RuntimeHexCatalog {
        crate::RuntimeHexCatalog::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles"),
            crate::tile_source::REGISTERS,
        )
        .expect("production content")
    }

    #[test]
    fn twenty_four_seeds_get_two_distinct_connected_mutable_beats_per_target_floor() {
        let catalog = catalog();
        for index in 0..24u64 {
            let seed = if index == 0 {
                1
            } else {
                0xA11C_E3D0_0000_0000 ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            };
            let mut world = HexWfcWorld::generate_with_profile(
                seed,
                HexWfcConfig::arc_default(),
                None,
                &catalog.composition,
            )
            .expect("survey seed solves");
            let before = world.clone();
            let plans = seed_initial_halls(&mut world, &catalog.cells, &catalog.composition);
            assert_eq!(plans.len(), 4, "seed {seed}");
            assert_eq!(world.initial_modules.len(), 12);
            assert_eq!(world.placements, before.placements);
            assert_eq!(world.blueprints, before.blueprints);
            assert_eq!(world.architecture, before.architecture);
            assert_eq!(world.cell_revisions, before.cell_revisions);
            assert_eq!(world.authored_pins, before.authored_pins);
            for plan in &plans {
                assert_eq!(plan.cells.len(), 3);
                for pair in plan.cells.windows(2) {
                    assert!(neighbors(&world, pair[0]).any(|next| next == pair[1]));
                }
                for &cell in &plan.cells {
                    let placement = world.placements[&cell];
                    assert_eq!(placement.space, HexSpace::Hall);
                    assert!(
                        !before
                            .blueprints
                            .iter()
                            .any(|room| room.cells.contains(&cell))
                    );
                    assert_eq!(placement.up, observed_hex::PortClass::Sealed);
                    assert_eq!(placement.down, observed_hex::PortClass::Sealed);
                    assert!(world.route_between(world.config.spawn(), cell).is_some());
                }
            }
            let mut repeated = before;
            assert_eq!(
                seed_initial_halls(&mut repeated, &catalog.cells, &catalog.composition),
                plans
            );
            assert_eq!(repeated.initial_modules, world.initial_modules);
        }
    }

    #[test]
    fn revisions_relinquish_only_the_rewritten_module_and_preserve_the_others() {
        let catalog = catalog();
        let mut world = HexWfcWorld::generate_with_profile(
            1,
            HexWfcConfig::arc_default(),
            None,
            &catalog.composition,
        )
        .unwrap();
        seed_initial_halls(&mut world, &catalog.cells, &catalog.composition);
        let cell = *world.initial_modules.keys().next().unwrap();
        let before = world
            .initial_modules
            .iter()
            .map(|(&cell, _)| (cell, world.geometry_identity(cell)))
            .collect::<BTreeMap<_, _>>();
        world.cell_revisions.insert(cell, 1);
        assert_eq!(world.initial_module_variant(cell), None);
        for (&other, identity) in &before {
            if other != cell {
                assert_eq!(world.geometry_identity(other), *identity);
            }
        }
        world.cell_revisions.insert(cell, 0);
        assert_eq!(world.geometry_identity(cell), before[&cell]);
    }

    #[test]
    fn opting_out_preserves_the_ordinary_initial_world() {
        let catalog = catalog();
        let mut profile = catalog.composition.clone();
        profile.initial_hall_compositions = false;
        let mut world =
            HexWfcWorld::generate_with_profile(1, HexWfcConfig::arc_default(), None, &profile)
                .unwrap();
        let before = world.clone();
        assert!(seed_initial_halls(&mut world, &catalog.cells, &profile).is_empty());
        assert_eq!(world, before);
    }
}
