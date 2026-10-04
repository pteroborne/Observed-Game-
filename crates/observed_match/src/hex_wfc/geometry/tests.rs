use observed_content::ArchitectureRegister;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{
    HexArchetype, HexMutationRegion, HexObservationFrame, HexRelayoutProgress, HexSpace,
    HexWfcConfig, HexWfcWorld,
};
use observed_hex::{HexFace, hex_origin};
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
use observed_traversal::{DeckPath, FpsBody, FpsConfig, StairSpine};
use player_input::PlayerIntent;

use super::*;

const SHOWCASE_SEED: u64 = 0xA11C_E3D0_0000_0008;

fn tiles() -> Vec<TilePrototype> {
    crate::hex_wfc::test_tiles()
}

fn showcase() -> HexWfcWorld {
    HexWfcWorld::generate(
        SHOWCASE_SEED,
        HexWfcConfig {
            cols: 12,
            rows: 9,
            levels: 4,
            min_rooms: 4,
            max_rooms: 8,
            retry_budget: 100,
            min_room_distance: 2,
        },
    )
    .expect("showcase world")
}

#[test]
fn identical_world_and_manifest_project_identically() {
    let world = showcase();
    let tiles = tiles();
    let a = HexWfcGeometrySnapshot::project(&world, &tiles).expect("projection");
    let b = HexWfcGeometrySnapshot::project(&world, &tiles).expect("projection");
    assert_eq!(a, b);
    a.arena.validate().expect("valid arena");
    assert!(
        !a.lights.is_empty(),
        "walkable prefabs project authored lights"
    );
    assert!(a.lights.iter().all(|light| {
        world.placements.contains_key(&light.source_cell) && light.position.is_finite()
    }));
}

#[test]
fn projected_guides_are_the_only_source_of_climb_and_deck_compatibility_maps() {
    let world = showcase();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projection");
    let (climbs, decks) = compatibility_guide_maps(&snapshot.guides);

    assert_eq!(snapshot.climbs, climbs);
    assert_eq!(snapshot.decks, decks);
    assert!(
        snapshot.guides.iter().all(|(coord, guide)| {
            *coord == guide.source_cell
                && guide.instance.source_cell == *coord
                && guide.revision
                    == HexModuleRevision::single(
                        *coord,
                        world
                            .cell_revision(*coord)
                            .expect("projected cell revision"),
                    )
                && guide.source_cells == [*coord]
                && guide.graph.is_none()
        }),
        "legacy guides carry exact instance-local identity without inventing a v4 graph"
    );
    // Since the climb compositions, no module ships a climb and a deck together;
    // the atomicity that pairing exercised is proved on a built guide below.
    assert!(
        snapshot.guides.values().any(|guide| guide.climb.is_some()),
        "the fixture must exercise climb guides"
    );
}

#[test]
fn guide_delta_replaces_and_removes_climb_and_deck_atomically() {
    let world = showcase();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projection");
    // A module carrying a climb and a deck at once, built from a real climb guide:
    // no tile in the corpus carries both since the climb compositions, and the
    // property is the delta's, not the corpus's.
    let (&coord, climbing) = snapshot
        .guides
        .iter()
        .find(|(_, guide)| guide.climb.is_some())
        .expect("fixture has a climb guide");
    let origin = Vec3::from_array(hex_origin(coord));
    let original = ProjectedTraversalGuide {
        deck: Some(DeckPath {
            nodes: [-3.0, 0.0, 3.0]
                .map(|x| origin + Vec3::new(x, observed_hex::FLOOR_SLAB_TOP, 2.0))
                .to_vec(),
        }),
        ..climbing.clone()
    };
    let mut snapshot = snapshot;
    snapshot.guides.insert(coord, original.clone());
    snapshot
        .decks
        .insert(coord, original.deck.clone().expect("just built"));
    let original = &original;
    let changed = BTreeSet::from([coord]);
    let replacement = ProjectedTraversalGuide {
        instance: original.instance,
        revision: original.revision.clone(),
        source_cells: original.source_cells.clone(),
        graph: None,
        source_cell: coord,
        climb: original.climb.clone(),
        deck: None,
    };
    let mut guides = snapshot.guides.clone();
    let mut climbs = snapshot.climbs.clone();
    let mut decks = snapshot.decks.clone();

    apply_guide_delta(
        &mut guides,
        &mut climbs,
        &mut decks,
        &changed,
        &BTreeMap::from([(coord, replacement.clone())]),
    );
    assert_eq!(guides.get(&coord), Some(&replacement));
    assert_eq!(climbs.get(&coord), replacement.climb.as_ref());
    assert!(
        !decks.contains_key(&coord),
        "the replaced module's old deck cannot survive its guide"
    );

    apply_guide_delta(
        &mut guides,
        &mut climbs,
        &mut decks,
        &changed,
        &BTreeMap::new(),
    );
    assert!(!guides.contains_key(&coord));
    assert!(!climbs.contains_key(&coord));
    assert!(!decks.contains_key(&coord));
}

#[test]
fn variation_modulo_keeps_the_full_portable_u64_key() {
    let key = u64::from(u32::MAX) + 17;
    assert_eq!(variation_index(key, 7), (key % 7) as usize);
}

fn selection_digest(selections: &BTreeMap<HexCoord, TileKey>) -> u64 {
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |byte: u8| {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for (coord, key) in selections {
        for byte in coord.q.to_le_bytes() {
            mix(byte);
        }
        for byte in coord.r.to_le_bytes() {
            mix(byte);
        }
        mix(coord.level);
        for value in [&key.archetype, &key.register] {
            for &byte in value.as_bytes() {
                mix(byte);
            }
            mix(0xff);
        }
        for byte in key.variant.to_le_bytes() {
            mix(byte);
        }
    }
    digest
}

fn port_signature(ports: &[(HexFace, PortClass)]) -> PortSignature {
    let mut values = [PortClass::Sealed; 8];
    for &(face, class) in ports {
        values[face.index()] = class;
    }
    PortSignature::try_from_ports(values).expect("test signature is valid")
}

fn selected_tiles(snapshot: &HexWfcGeometrySnapshot) -> BTreeMap<HexCoord, TileKey> {
    let mut selections = BTreeMap::new();
    for piece in &snapshot.pieces {
        let Some(tile) = &piece.tile else {
            continue;
        };
        if let Some(previous) = selections.insert(piece.source_cell, tile.clone()) {
            assert_eq!(
                previous, *tile,
                "one resolved cell projected pieces from multiple tile keys"
            );
        }
    }
    selections
}

/// The real committed corpus must keep selecting the same concrete modules on
/// both the canonical spectator seed and the seed that exposed the perimeter
/// tower stall. This pins the pre-refactor bucket ordering before selection is
/// moved behind a catalog object and later changed deliberately in TR-9.
///
/// **Re-pinned when corner, junction and expanse gained interior readings.**
/// More candidates in a bucket move the modulo, so the hall digests had to
/// change; the point is *what did not*. Both seeds kept their cell count (384)
/// and — byte for byte — their tower count and tower digest, because stair
/// towers gained no readings and their selection is therefore untouched. A
/// change that had moved those would have been a different change than the one
/// intended.
///
/// | | before | after |
/// | --- | --- | --- |
/// | seed 1 | `0x53ff32531fe8f9f8` | `0xcdcbe8fbaacac084` |
/// | seed 10000031 | `0x68eb78fd363f2172` | `0xdb8c7c36035aec42` |
///
/// Moved again when Keystone, Monitor and Recovery each gained a second door.
/// Both seeds project more tiles than before (384 -> 387 and 400), which is the
/// expected direction: a room that opens a second face needs the cell beyond it
/// to be a hall rather than the fill it used to be.
///
/// | | before | after |
/// | --- | --- | --- |
/// | seed 1 | `0xcdcbe8fbaacac084` | `0x724700725be9a28d` |
/// | seed 10000031 | `0xdb8c7c36035aec42` | `0x1a2c49860d076a3a` |
///
/// Moved again when the collapse began drawing the space before the variant.
/// Both seeds project *fewer* tiles now - 387 to 300 and 400 to 267 - and that
/// direction is the whole point: the facility went from 1.7% void to 18.5%, so
/// there is simply less of it to build.
///
/// | | before | after |
/// | --- | --- | --- |
/// | seed 1 | `0x724700725be9a28d` | `0xda2b4f50572c48fb` |
/// | seed 10000031 | `0x1a2c49860d076a3a` | `0x5b7df763dd7bbf5d` |
///
/// Moved again by the branching stair landing. The alphabet grew from 404
/// variants to 509 - every three- and four-door shaft mask, against three
/// vertical connectivities - so the lottery moved for every cell, and the
/// corpus grew the 105 towers to serve them.
///
/// The tower columns are the ones to read. Seed 1 goes from 68 towers to 84 and
/// seed 10000031 from 45 to 64, against tile counts that barely move (300 -> 276
/// and 267 -> 281). That is the shaft family taking a larger share of the hall
/// alphabet - 20% to 31% by weight - and it is the number to watch if the
/// facility ever starts reading as stairs again, which is what backlog #13 was.
///
/// | | before | after |
/// | --- | --- | --- |
/// | seed 1 | `0xda2b4f50572c48fb` | `0xacfd4d912b5386e9` |
/// | seed 10000031 | `0x5b7df763dd7bbf5d` | `0x50068539c58d897f` |
///
/// Moved twice more in one packet, and the tower column is again the one to
/// read. T-4 resized the facility from 5,600 cells to 3,264, and the flat
/// alphabet was then doubled against the shaft family to undo the share the
/// branching landing had taken. Towers go from 84 to 45 on seed 1 and 64 to 29
/// on seed 10000031 - roughly halved on both, against tile counts that barely
/// move. That is backlog #13's number coming back down: stair towers were 29.7%
/// of placed geometry after the landing and are 18.9% now.
///
/// | | before | after |
/// | --- | --- | --- |
/// | seed 1 | `0xacfd4d912b5386e9` | `0x080fe5f632e2aae6` |
/// | seed 10000031 | `0x50068539c58d897f` | `0xca66060faa6ab9aa` |
///
/// Moved again by Arc T corpus authoring, and this time the *unmoved* columns
/// carry the argument. `hall_straight` gained a handed variant, `hall_ramp` a
/// second reading, and the expanse gained its district's ceiling - all hall
/// geometry. The tile counts are identical (293 and 238) and so are the tower
/// count and tower digest, bit for bit, because nothing vertical was touched.
/// A corpus change that had disturbed the towers would show here rather than
/// somewhere a playtest would find it.
///
/// | | before | after |
/// | --- | --- | --- |
/// | seed 1 | `0x080fe5f632e2aae6` | `0xa1b63d6430817c7b` |
/// | seed 10000031 | `0xca66060faa6ab9aa` | `0x67aba9266a6b3789` |
///
/// Moved again by the archetype sweep, and this time the move *is* the result.
/// Both counts are unchanged - 293 and 238 - and both tower digests are
/// identical bit for bit, because the sweep added no cells and touched nothing
/// vertical. What changed is which module the weighted draw returned in cells
/// that were already there, which is the first evidence that authored program
/// rooms and district dialects are being placed in a solved facility rather
/// than sitting in the catalogue unreachable.
///
/// | | before | after |
/// | --- | --- | --- |
/// | seed 1 | `0xa1b63d6430817c7b` | `0x598eeee703c6ca82` |
/// | seed 10000031 | `0x67aba9266a6b3789` | `0x9318b4ffadb83e31` |
///
/// Moved once more by the twenty open halls, and the shape of the move is the
/// same as last time: both counts unchanged at 293 and 238, both tower digests
/// identical bit for bit, only the module chosen in cells that were already
/// there. Four districts now route through rooms rather than channels.
///
/// | | before | after |
/// | --- | --- | --- |
/// | seed 1 | `0x598eeee703c6ca82` | `0x0ab420fab0783b4a` |
/// | seed 10000031 | `0x9318b4ffadb83e31` | `0x873fec8e95dc7f56` |
/// # What a solved facility actually contains, measured
///
/// Instrumented once, here, over these two seeds:
///
/// | | seed 1 | seed 10000031 |
/// | --- | --- | --- |
/// | cells | 293 | 238 |
/// | hulls | 6170 | 5002 |
/// | practical lights | 530 | 426 |
/// | cells drawn from the archetype sweep | 39 | 35 |
///
/// Twenty-one hulls a cell on average, and five hundred and thirty point
/// lights in one facility. Those are the numbers the render budget has to
/// answer for, and they are recorded here because the count is the cheap half
/// of the question and nobody had written it down.
///
/// The sweep column is the interesting one. Seed 1 places a canteen, a locker
/// corridor, two classrooms, two office floors and a plant room among its 293
/// cells, in six different districts. Program rooms carry 24 to 33 hulls
/// against that 21 average, so nine of them cost on the order of a hundred
/// hulls in six thousand: whatever the budget risk in a facility this size is,
/// **it is not the program layer**, and that is worth knowing before anyone
/// optimises the wrong thing.
#[test]
fn production_catalog_selection_is_pinned_for_spectator_seeds() {
    // Curation retires 96 alternatives from the production catalogue. Re-pin content;
    // placement counts and the climb compositions' own selection remain exact gates.
    let catalog = crate::hex_wfc::test_catalog();
    // Re-pinned when the committed profile moved to the open-air composition (void
    // share 300 -> 2,000): the same seeds build about half as many cells, because more
    // of the lattice is air. The table above records the facility before that.
    //
    // Re-pinned again when every floor became one district on the climb (143 -> 147
    // and 101 -> 151 cells), and again when each floor drew its own openness (-> 208
    // and 184): on these four floors the closed Backrooms and half-closed Lumen build
    // more than the open Monument and sky above them give back. The switchback ramp
    // moved the selection digests and nothing else: the generated wedge no longer
    // shares the ramp's keys, so every ramp is the authored one; and again when each
    // district's ramp got its own dressing.
    //
    // Re-pinned when the climb compositions replaced the ramps and stair towers (208 ->
    // 189 and 184 -> 147 cells, routed around the longer climbs); the gate that
    // followed the tower family follows the climb cells now: 32 and 24 of them, eight
    // and six compositions. Every district's own climb then moved both digests and
    // neither count: the same cells, drawn from each register's dressing.
    //
    // Re-pinned when the climbs gained turned mid cells and landings (189 -> 214 and
    // 147 -> 198 cells; 32 -> 16 and 24 -> 44 climb cells): a different solve, since
    // the alphabet grew and its weights were rescaled. Across twelve unprofiled solves
    // the climbs hold their share and the facility builds 8% more cells
    // (`variants::CLIMB_WEIGHT`); these two seeds swing either way.
    let cases = [
        (
            1u64,
            214usize,
            0x102b_9f57_5839_8d54u64,
            16usize,
            0x3a00_ae27_3908_e18eu64,
        ),
        (
            10_000_031u64,
            198usize,
            0x3d35_97f4_c0c2_4587u64,
            44usize,
            0x5f6f_8a03_3dd5_64b6u64,
        ),
    ];
    let mut actual = Vec::new();
    for (seed, _, _, _, _) in cases {
        let world = HexWfcWorld::generate_with_profile(
            seed,
            HexWfcConfig {
                levels: 4,
                ..HexWfcConfig::default()
            },
            None,
            &catalog.composition,
        )
        .expect("pinned spectator seed solves");
        let snapshot =
            HexWfcGeometrySnapshot::project_with_rooms(&world, &catalog.cells, &catalog.rooms)
                .expect("production corpus projects");
        let selections = selected_tiles(&snapshot);
        let digest = selection_digest(&selections);
        let climbs = selections
            .iter()
            .filter(|(_, tile)| tile.archetype.starts_with("climb_"))
            .map(|(coord, tile)| (*coord, tile.clone()))
            .collect::<BTreeMap<_, _>>();
        let climb_digest = selection_digest(&climbs);
        eprintln!(
            "seed={seed} count={} digest={digest:#018x} climb_count={} climb_digest={climb_digest:#018x}",
            selections.len(),
            climbs.len()
        );
        actual.push((seed, selections.len(), digest, climbs.len(), climb_digest));
    }
    // Report both seeds together when an intentional content addition moves
    // the selection pin; every count and tower digest remains part of it.
    assert_eq!(actual.as_slice(), &cases);
}

/// Compatibility content still has no family, and this is what that costs.
///
/// Selection over uncontracted prototypes is local to one
/// `(archetype, register, signature)` bucket, so the same variation key cannot
/// promise that two signatures choose members of one implicit family: the
/// buckets can contain different members. That is not a defect in the selector
/// — it is the reason a family has to be *declared*, and it is precisely the
/// gap `two_declared_families_never_mix_inside_one_column` closes for
/// contracted content. It stays pinned until TR-10 migrates the corpus.
#[test]
fn identical_variation_keys_do_not_guarantee_family_coherence() {
    let mut first_a = tiles().into_iter().next().expect("fixture tile");
    first_a.key.archetype = "family_probe".to_string();
    first_a.key.register = "generic".to_string();
    first_a.key.variant = 10;
    first_a.signature = port_signature(&[]);
    let mut second_a = first_a.clone();
    second_a.key.variant = 20;

    let mut first_b = first_a.clone();
    first_b.key.variant = 20;
    first_b.signature = port_signature(&[(HexFace::Up, PortClass::ShaftOpen)]);
    let mut second_b = first_b.clone();
    second_b.key.variant = 30;

    let prototypes = [first_a, second_a, first_b, second_b];
    let catalogue = HexTileCatalogue::new(&prototypes);
    let variation = 0;
    let lower = catalogue
        .select(
            "family_probe",
            "monolith",
            port_signature(&[]),
            None,
            variation,
            variation,
        )
        .expect("no family is involved")
        .expect("generic fallback answers the first signature");
    let upper = catalogue
        .select(
            "family_probe",
            "monolith",
            port_signature(&[(HexFace::Up, PortClass::ShaftOpen)]),
            None,
            variation,
            variation,
        )
        .expect("no family is involved")
        .expect("generic fallback answers the second signature");

    assert_eq!(lower.key.variant, 10, "generic fallback keeps bucket order");
    assert_eq!(upper.key.variant, 20, "same key is applied independently");
    assert_ne!(
        lower.key.variant, upper.key.variant,
        "an identical variation key is not an assembly-family contract"
    );
}

#[test]
fn oversized_grid_reports_collider_id_capacity_before_projection() {
    let config = HexWfcConfig {
        cols: u16::MAX,
        rows: u16::MAX,
        levels: u8::MAX,
        min_rooms: 2,
        max_rooms: 2,
        retry_budget: 1,
        min_room_distance: 1,
    };
    let world = HexWfcWorld {
        seed: 1,
        generation: 0,
        config,
        placements: BTreeMap::new(),
        blueprints: Vec::new(),
        architecture: BTreeMap::new(),
        cell_revisions: BTreeMap::new(),
        last_attempts: 1,
        authored_pins: Default::default(),
        space_mix: observed_facility::hex_wfc::profile::SpaceMix::baseline(),
        route_corridors: false,
        carve_unrouted: false,
        open_air: false,
        sealed: false,
    };
    assert!(matches!(
        HexWfcGeometrySnapshot::project(&world, &[]),
        Err(HexGeometryError::ColliderIdCapacity { .. })
    ));
}

#[test]
fn every_non_void_cell_is_covered_by_a_prefab_instance() {
    let world = showcase();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projection");
    let covered: BTreeSet<_> = snapshot
        .pieces
        .iter()
        .filter(|piece| piece.tile.is_some())
        .map(|piece| piece.source_cell)
        .collect();
    for placement in world.placements.values() {
        if placement.space.unbuilt() {
            continue;
        }
        // Every climb cell, the landing included, is a tile of its own: nothing in a
        // climb composition is the geometry-free half of another cell's prefab.
        assert!(covered.contains(&placement.coord), "missing {placement:?}");
    }
    assert_eq!(snapshot.blueprint_instances, world.blueprints.len());
    assert!(
        world
            .placements
            .values()
            .any(|placement| matches!(placement.archetype, HexArchetype::Climb { .. })),
        "showcase includes climb compositions"
    );
}

#[test]
fn stable_ids_are_unique_and_partitioned_by_source_cell() {
    let world = showcase();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projection");
    let mut ids = BTreeSet::new();
    for piece in &snapshot.pieces {
        assert!(ids.insert(piece.id), "duplicate {:?}", piece.id);
        if piece.tile.is_some() {
            let base = world.config.grid().index(piece.source_cell) * COLLIDER_STRIDE + 1;
            assert!((base..base + COLLIDER_STRIDE).contains(&(piece.id.0 as usize)));
        }
    }
}

#[test]
fn nothing_walls_the_lattice_and_its_rim_halls_open_onto_the_outside() {
    let world = showcase();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projection");
    assert!(
        snapshot
            .pieces
            .iter()
            .all(|piece| piece.role != HexStructureRole::Boundary),
        "the arena shell is gone: the edge of the lattice is open air"
    );
    let grid = world.config.grid();
    let rim_halls = world
        .placements
        .values()
        .filter(|placement| {
            open_edge::can_open(placement)
                && HexFace::LATERAL.into_iter().any(|face| {
                    !placement.is_open(face) && grid.neighbor(placement.coord, face).is_none()
                })
        })
        .collect::<Vec<_>>();
    assert!(!rim_halls.is_empty());
    for placement in rim_halls {
        assert!(
            snapshot.pieces.iter().any(
                |piece| piece.source_cell == placement.coord && piece.part == HexPiecePart::Lip
            ),
            "rim hall {:?} has no lip",
            placement.coord
        );
    }
}

#[test]
fn boundary_start_uses_its_authored_blueprint_signature() {
    let world = showcase();
    let start = world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.anchor == world.config.spawn())
        .expect("start blueprint");
    let authored = blueprint_for_role(start.role).cell_signature((0, 0, 0));
    let solved = world.placements[&start.anchor].ports();
    assert_ne!(authored, solved, "boundary solve seals out-of-grid faces");

    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projection");
    let start_pieces: Vec<_> = snapshot
        .pieces
        .iter()
        .filter(|piece| piece.anchor == start.anchor && piece.tile.is_some())
        .collect();
    assert!(!start_pieces.is_empty());
    // `room_single`, not `sanctuary`. Every room cell of every role used to ask
    // for the same single-hex shape (bug backlog #15); a Start room is a
    // single-hex room, so this is the one case where the answer looks the same
    // and the reason is different.
    let expected = blueprint_cell_archetype(start.role, 0).expect("start has a cell archetype");
    assert!(start_pieces.iter().all(|piece| {
        piece
            .tile
            .as_ref()
            .is_some_and(|key| key.archetype == expected)
    }));
}

#[test]
fn matching_whole_room_module_takes_precedence_over_cell_fallbacks() {
    let world = showcase();
    let start = world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.anchor == world.config.spawn())
        .expect("start blueprint");
    let register = world.architecture[&start.anchor].slug().to_string();
    let fallback_hulls = tiles()
        .into_iter()
        .find(|tile| {
            tile.key.archetype
                == blueprint_cell_archetype(start.role, 0).expect("start cell archetype")
                && (tile.key.register == register || tile.key.register == "generic")
                && tile.signature == blueprint_for_role(start.role).cell_signature((0, 0, 0))
        })
        .expect("start fallback")
        .hulls;
    let ports = [(HexFace::West, "entrance"), (HexFace::East, "exit")]
        .into_iter()
        .map(|(face, name)| observed_authoring::RoomPrototypePort {
            cell: ModuleCellRef {
                q: 0,
                r: 0,
                level: 0,
            },
            face,
            class: PortClass::Door,
            name: name.to_string(),
        })
        .collect();
    let room = RoomPrototype {
        id: "test/whole-start".to_string(),
        room_role: "start".to_string(),
        key: TileKey {
            archetype: "whole_start".to_string(),
            register,
            variant: 60_000,
        },
        weight: 1,
        footprint: vec![ModuleCellRef {
            q: 0,
            r: 0,
            level: 0,
        }],
        ports,
        sockets: vec![observed_authoring::RoomPrototypeSocket {
            id: "test_socket".to_string(),
            kind: observed_authoring::RoomSocketKind::Monitor,
            cell: ModuleCellRef {
                q: 0,
                r: 0,
                level: 0,
            },
            position: glam::Vec3::new(1.0, 1.5, -2.0),
            yaw_degrees: 30.0,
        }],
        hulls: fallback_hulls,
        lights: Vec::new(),
        contract: None,
        assembly: None,
    };
    let snapshot = HexWfcGeometrySnapshot::project_with_rooms(&world, &tiles(), &[room])
        .expect("whole-room projection");
    let start_pieces = snapshot
        .pieces
        .iter()
        .filter(|piece| piece.role == HexStructureRole::Room && piece.anchor == start.anchor)
        .collect::<Vec<_>>();
    assert!(!start_pieces.is_empty());
    assert!(start_pieces.iter().all(|piece| {
        piece
            .tile
            .as_ref()
            .is_some_and(|key| key.archetype == "whole_start")
    }));
    assert_eq!(snapshot.sockets.len(), 1);
    assert_eq!(snapshot.sockets[0].id, "test_socket");
    assert_eq!(snapshot.sockets[0].cell, start.anchor);
    assert_eq!(
        snapshot.sockets[0].kind,
        observed_authoring::RoomSocketKind::Monitor
    );
}

#[test]
fn bounded_delta_matches_full_projection_and_preserves_pinned_pieces() {
    let prototypes = tiles();
    let mut world = showcase();
    world.config.retry_budget = 1;
    let mut frame = HexObservationFrame::default();
    let room = world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.anchor != world.config.spawn())
        .expect("non-start room");
    frame.visible_cells.insert(room.cells[0]);
    if let Some(straight) = world
        .placements
        .values()
        .find(|placement| placement.archetype == HexArchetype::Straight)
    {
        frame.visible_cells.insert(straight.coord);
    }
    if let Some(climb) = world
        .placements
        .values()
        .find(|placement| matches!(placement.archetype, HexArchetype::Climb { .. }))
    {
        frame.visible_cells.insert(climb.coord);
    }
    frame.objective_cells.insert(world.config.spawn());

    let work = world.begin_relayout(&frame);
    let pinned = work.pinned_cells().clone();
    let before = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("before");
    let before_pinned: BTreeMap<_, _> = before
        .pieces
        .iter()
        .filter(|piece| pinned.contains(&piece.source_cell))
        .map(|piece| (piece.id, piece.clone()))
        .collect();
    assert!(!before_pinned.is_empty());

    let candidate = match world.advance_relayout(work).expect("advance") {
        HexRelayoutProgress::Ready(candidate) => candidate,
        HexRelayoutProgress::Pending(_) => panic!("retry budget one must finish"),
    };
    let logical = world
        .commit_relayout_delta(candidate, &frame)
        .expect("commit");
    assert_eq!(world.generation, 1);
    let delta = before
        .project_delta(&world, &logical, &prototypes)
        .expect("delta projection");
    assert!(
        delta
            .upserted_pieces
            .iter()
            .all(|piece| delta.changed_cells.contains(&piece.source_cell))
    );
    // The only cells re-projected beyond the logical change are halls and rooms beside
    // it, whose open edges and windows may have moved with it.
    let grid = world.config.grid();
    for cell in delta.changed_cells.difference(&logical.changed_cells) {
        assert!(
            observed_facility::hex_wfc::exposure::follows_neighbours(&world.placements[cell]),
            "{cell:?}"
        );
        assert!(
            HexFace::LATERAL.into_iter().any(|face| grid
                .neighbor(*cell, face)
                .is_some_and(|next| logical.changed_cells.contains(&next))),
            "{cell:?} re-projected without touching the change"
        );
    }
    let mut incremental = before.clone();
    let mut scene = before.rapier_scene();
    scene
        .apply_collider_delta(&delta.colliders)
        .expect("live collider update");
    incremental.apply_delta(&delta).expect("snapshot update");
    let after = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("after");
    let after_by_id: BTreeMap<_, _> = after.pieces.iter().map(|piece| (piece.id, piece)).collect();
    let incremental_by_id: BTreeMap<_, _> = incremental
        .pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect();
    assert_eq!(incremental_by_id, after_by_id);
    let incremental_colliders = incremental
        .arena
        .colliders
        .iter()
        .map(|collider| (collider.id, collider))
        .collect::<BTreeMap<_, _>>();
    let after_colliders = after
        .arena
        .colliders
        .iter()
        .map(|collider| (collider.id, collider))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(incremental_colliders, after_colliders);
    assert_eq!(
        incremental.guides, after.guides,
        "bounded relayout must replace complete module guides"
    );
    assert_eq!(incremental.climbs, after.climbs);
    assert_eq!(incremental.decks, after.decks);
    assert_eq!(incremental.blueprint_instances, after.blueprint_instances);
    assert_eq!(scene.collider_count(), after.arena.colliders.len());
    for (id, before_piece) in before_pinned {
        assert_eq!(
            after_by_id.get(&id).copied(),
            Some(&before_piece),
            "pinned collider {id:?} drifted"
        );
    }
}

/// Manual risk measurement for the full production-shaped grid.
/// Ignored in the ordinary suite because the large WFC solve is intentionally expensive.
#[test]
#[ignore = "manual production-scale collider budget measurement"]
fn report_arc_default_collider_build_and_step_budget() {
    let started = std::time::Instant::now();
    let mut world = HexWfcWorld::generate(0xA11C_9300_0000_0001, HexWfcConfig::arc_default())
        .expect("arc default solves");
    let solve_time = started.elapsed();

    let prototypes = tiles();
    let started = std::time::Instant::now();
    let mut snapshot = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("projection");
    let projection_time = started.elapsed();
    let started = std::time::Instant::now();
    let mut scene = snapshot.rapier_scene();
    let scene_build_time = started.elapsed();

    let mut observation = HexObservationFrame::default();
    for raw in 0..4 {
        observation
            .occupied_cells
            .insert(PlayerId(raw), world.config.spawn());
    }
    let started = std::time::Instant::now();
    let mut work = world.begin_relayout(&observation);
    let candidate = loop {
        match world.advance_relayout(work).expect("local solve") {
            HexRelayoutProgress::Pending(next) => work = next,
            HexRelayoutProgress::Ready(candidate) => break candidate,
        }
    };
    let pocket_solve_time = started.elapsed();
    let started = std::time::Instant::now();
    let logical = world
        .commit_relayout_delta(candidate, &observation)
        .expect("local commit");
    let logical_commit_time = started.elapsed();
    let started = std::time::Instant::now();
    let geometry_delta = snapshot
        .project_delta(&world, &logical, &prototypes)
        .expect("delta projection");
    let delta_projection_time = started.elapsed();
    let collider_ops =
        geometry_delta.colliders.removed.len() + geometry_delta.colliders.upserted.len();
    let started = std::time::Instant::now();
    scene
        .apply_collider_delta(&geometry_delta.colliders)
        .expect("incremental Rapier update");
    let physics_delta_time = started.elapsed();
    let started = std::time::Instant::now();
    snapshot
        .apply_delta(&geometry_delta)
        .expect("snapshot delta");
    let snapshot_delta_time = started.elapsed();

    let config = FpsConfig::deliberate_rapier();
    let spawn =
        Vec3::from_array(hex_origin(world.config.spawn())) + Vec3::Y * (config.half_height + 0.5);
    let characters = 8u32;
    let mut bodies: Vec<FpsBody> = (0..characters)
        .map(|index| {
            let angle = index as f32 * std::f32::consts::TAU / characters as f32;
            let offset = Vec3::new(angle.cos() * 0.8, 0.0, angle.sin() * 0.8);
            FpsBody::spawned(spawn + offset, angle)
        })
        .collect();
    let intent = PlayerIntent {
        movement: Vec2::new(0.35, 1.0),
        look: Vec2::new(0.02, 0.0),
        sprint_held: true,
        ..PlayerIntent::default()
    };
    let frames = 600u32;
    let started = std::time::Instant::now();
    for _ in 0..frames {
        for body in &mut bodies {
            step_character(&scene, body, intent, &config, 1.0 / 60.0);
        }
    }
    let step_time = started.elapsed();
    let batch_frame_micros = step_time.as_micros() / u128::from(frames);
    let character_query_micros = step_time.as_micros() / u128::from(frames * characters);
    let non_void = world
        .placements
        .values()
        .filter(|placement| placement.space.built())
        .count();
    eprintln!(
        "ARC_M_MUTATION_BUDGET cells={} non_void={} colliders={} solve_ms={} projection_ms={} scene_build_ms={} pocket_cells={} changed_cells={} collider_ops={} pocket_solve_us={} logical_commit_us={} delta_projection_us={} physics_delta_us={} snapshot_delta_us={} characters={} batch_frame_us={} character_query_us={}",
        world.config.grid().cell_count(),
        non_void,
        snapshot.pieces.len(),
        solve_time.as_millis(),
        projection_time.as_millis(),
        scene_build_time.as_millis(),
        logical.region.cells.len(),
        logical.changed_cells.len(),
        collider_ops,
        pocket_solve_time.as_micros(),
        logical_commit_time.as_micros(),
        delta_projection_time.as_micros(),
        physics_delta_time.as_micros(),
        snapshot_delta_time.as_micros(),
        characters,
        batch_frame_micros,
        character_query_micros,
    );
    assert_eq!(scene.collider_count(), snapshot.pieces.len());
    assert!(
        batch_frame_micros < 16_667,
        "eight moving characters must step inside 60 Hz"
    );
}

// ---------------------------------------------------------------------------
// Multi-cell whole-room projection (Stream B).
//
// The single existing whole-room test above
// (`matching_whole_room_module_takes_precedence_over_cell_fallbacks`) only
// exercises a one-cell `Start` blueprint. Every test below hand-builds a
// genuinely multi-cell `HexWfcWorld` (two- and three-cell footprints) so the
// real fan-out in `project_blueprint`/`push_room`/`project_delta_with_rooms`
// gets covered instead of just its single-cell degenerate case.

/// A small, definitely-non-degenerate convex hull (Rapier's `convex_hull`
/// builder rejects anything with fewer than 4 non-coplanar points).
fn tiny_tetrahedron(offset: f32) -> Vec<Vec3> {
    vec![
        Vec3::new(offset, 0.0, 0.0),
        Vec3::new(offset + 1.0, 0.0, 0.0),
        Vec3::new(offset, 1.0, 0.0),
        Vec3::new(offset, 0.0, 1.0),
    ]
}

/// A contract-valid whole-room prototype for `role`: footprint mirrors
/// `blueprint_for_role(role).cells` exactly and only its named exterior
/// thresholds are authored as ports. Internal sibling faces are continuous
/// geometry and therefore do not become module boundary ports.
fn multi_cell_room_prototype(role: RoomRole, archetype: &str, variant: u16) -> RoomPrototype {
    let blueprint = blueprint_for_role(role);
    let footprint = blueprint
        .cells
        .iter()
        .map(|&offset| cell_ref(offset).expect("small test offsets fit ModuleCellRef"))
        .collect();
    let ports = blueprint
        .named_ports
        .iter()
        .map(
            |&(name, offset, face)| observed_authoring::RoomPrototypePort {
                cell: cell_ref(offset).expect("small test offsets fit ModuleCellRef"),
                face,
                class: PortClass::Door,
                name: name.to_string(),
            },
        )
        .collect();
    RoomPrototype {
        id: format!("test/{archetype}"),
        room_role: blueprint.name.to_string(),
        key: TileKey {
            archetype: archetype.to_string(),
            register: "generic".to_string(),
            variant,
        },
        weight: 1,
        footprint,
        ports,
        sockets: Vec::new(),
        hulls: vec![tiny_tetrahedron(0.0)],
        lights: Vec::new(),
        contract: None,
        assembly: None,
    }
}

/// A minimal solved world whose only content is one multi-cell blueprint
/// stamped at `anchor`, matching `blueprint_for_role(role)` exactly. Small
/// enough to stay independent of the real WFC solver and the production
/// catalog entirely.
fn multi_cell_world(role: RoomRole, anchor: HexCoord) -> HexWfcWorld {
    let blueprint = blueprint_for_role(role);
    let cells: Vec<HexCoord> = blueprint
        .cells
        .iter()
        .map(|&(dq, dr, dl)| HexCoord {
            q: (i32::from(anchor.q) + dq) as u16,
            r: (i32::from(anchor.r) + dr) as u16,
            level: (i32::from(anchor.level) + dl) as u8,
        })
        .collect();
    let mut placements = BTreeMap::new();
    let mut architecture = BTreeMap::new();
    let mut cell_revisions = BTreeMap::new();
    for (&coord, &offset) in cells.iter().zip(&blueprint.cells) {
        let signature = blueprint.cell_signature(offset);
        let doors = HexFace::LATERAL
            .into_iter()
            .filter(|&face| signature.port(face) == PortClass::Door)
            .fold(0, |mask, face| mask | (1 << face.index()));
        placements.insert(
            coord,
            HexPlacement {
                coord,
                space: HexSpace::Room,
                archetype: HexArchetype::Room,
                doors,
                up: signature.port(HexFace::Up),
                down: signature.port(HexFace::Down),
            },
        );
        architecture.insert(coord, ArchitectureRegister::Institutional);
        cell_revisions.insert(coord, 0);
    }
    HexWfcWorld {
        seed: 0xD00D_0000_0000_0001,
        generation: 0,
        config: HexWfcConfig {
            cols: 6,
            rows: 6,
            levels: 1,
            min_rooms: 2,
            max_rooms: 2,
            retry_budget: 1,
            min_room_distance: 1,
        },
        placements,
        blueprints: vec![StampedBlueprint {
            id: 0,
            role,
            anchor,
            cells,
        }],
        architecture,
        cell_revisions,
        last_attempts: 1,
        authored_pins: Default::default(),
        space_mix: observed_facility::hex_wfc::profile::SpaceMix::baseline(),
        route_corridors: false,
        carve_unrouted: false,
        open_air: false,
        sealed: false,
    }
}

#[test]
fn multi_cell_room_internal_seams_are_traversable() {
    let anchor = HexCoord {
        q: 0,
        r: 0,
        level: 0,
    };
    let world = multi_cell_world(RoomRole::DualStation, anchor);
    let sibling = HexCoord {
        q: 1,
        r: 0,
        level: 0,
    };

    assert!(
        world.route_between(anchor, sibling).is_some(),
        "one blueprint footprint must route as one continuous room"
    );
}

/// Pins the multi-cell selection contract: a valid two-cell `DualStation`
/// module wins over the per-cell fallback, and — because `push_room` always
/// anchors every hull it emits at `stamped.anchor` — the *entire* footprint
/// collapses into one piece set rather than leaving a leftover per-cell
/// fallback piece at the second cell.
#[test]
fn matching_two_cell_room_module_consumes_the_entire_footprint_as_one_piece_set() {
    let anchor = HexCoord {
        q: 0,
        r: 0,
        level: 0,
    };
    let world = multi_cell_world(RoomRole::DualStation, anchor);
    let second_cell = HexCoord {
        q: 1,
        r: 0,
        level: 0,
    };
    let room = multi_cell_room_prototype(RoomRole::DualStation, "whole_dual_station", 1);
    let snapshot = HexWfcGeometrySnapshot::project_with_rooms(&world, &[], &[room])
        .expect("contract-valid two-cell module must project");

    let room_pieces: Vec<_> = snapshot
        .pieces
        .iter()
        .filter(|piece| piece.role == HexStructureRole::Room)
        .collect();
    assert!(!room_pieces.is_empty());
    assert!(room_pieces.iter().all(|piece| {
        piece.anchor == anchor
            && piece.source_cell == anchor
            && piece
                .tile
                .as_ref()
                .is_some_and(|key| key.archetype == "whole_dual_station")
    }));
    // No leftover per-cell fallback piece at the non-anchor footprint cell.
    assert!(
        !room_pieces
            .iter()
            .any(|piece| piece.source_cell == second_cell),
        "whole-room pieces must not be split back out per footprint cell"
    );
    assert_eq!(snapshot.blueprint_instances, 1);
}

/// Delta-path counterpart: touching just one non-anchor cell of a matching
/// three-cell `Decision` module must still (a) recognize the room match and
/// (b) expand `changed_cells`/`upserted_pieces` to the room's whole
/// footprint, not just the literally-touched cell. This path was completely
/// unexercised before this test.
#[test]
fn multi_cell_room_delta_projection_expands_a_partial_touch_to_the_full_footprint() {
    let anchor = HexCoord {
        q: 0,
        r: 0,
        level: 0,
    };
    let touched_cell = HexCoord {
        q: 1,
        r: 0,
        level: 0,
    };
    let before_world = multi_cell_world(RoomRole::Decision, anchor);
    let prototypes = tiles();
    // Project without the room prototype first: every footprint cell gets its
    // own per-cell fallback piece, exactly like a plain hall relayout would
    // have produced before the room module existed.
    let before = HexWfcGeometrySnapshot::project(&before_world, &prototypes)
        .expect("per-cell fallback projects the Decision footprint");

    let mut after_world = before_world.clone();
    after_world.generation = 1;
    let room = multi_cell_room_prototype(RoomRole::Decision, "whole_decision", 1);

    let mut initial_changed_cells = BTreeSet::new();
    initial_changed_cells.insert(touched_cell);
    let logical = HexRelayoutDelta {
        previous_generation: 0,
        generation: 1,
        previous_attempts: before_world.last_attempts,
        region: HexMutationRegion {
            cells: BTreeSet::new(),
            boundary_cells: BTreeSet::new(),
            protected_cells: BTreeSet::new(),
        },
        changed_cells: initial_changed_cells.clone(),
        placements: BTreeMap::new(),
        architecture: BTreeMap::new(),
        cell_revisions: BTreeMap::new(),
        previous_placements: BTreeMap::new(),
        previous_architecture: BTreeMap::new(),
        previous_cell_revisions: BTreeMap::new(),
        previous_blueprints: Vec::new(),
        removed_blueprints: Vec::new(),
        upserted_blueprints: Vec::new(),
    };

    let delta = before
        .project_delta_with_rooms(&after_world, &logical, &prototypes, &[room])
        .expect("matching multi-cell room delta must project");

    let footprint: BTreeSet<HexCoord> = after_world.blueprints[0].cells.iter().copied().collect();
    assert_eq!(footprint.len(), 3, "Decision is a three-cell blueprint");
    assert!(
        initial_changed_cells.len() < delta.changed_cells.len(),
        "touching one footprint cell must expand to the whole room"
    );
    assert_eq!(
        delta.changed_cells, footprint,
        "delta must cover exactly the room's footprint, no more, no less"
    );
    assert!(!delta.upserted_pieces.is_empty());
    assert!(delta.upserted_pieces.iter().all(|piece| {
        piece.source_cell == anchor
            && piece
                .tile
                .as_ref()
                .is_some_and(|key| key.archetype == "whole_decision")
    }));
    // The three old per-cell fallback pieces (one id range per footprint
    // cell) must be retired now that one room piece set replaces them.
    assert!(!delta.removed_piece_ids.is_empty());
}

/// Every way a candidate room can fail `room_contract_matches` must fall back
/// to per-cell tiles cleanly — never panic, never silently emit a
/// half-matched room.
#[test]
fn mismatched_room_contracts_fall_back_to_per_cell_tiles_without_panicking() {
    let anchor = HexCoord {
        q: 0,
        r: 0,
        level: 0,
    };
    let second_cell = HexCoord {
        q: 1,
        r: 0,
        level: 0,
    };
    let world = multi_cell_world(RoomRole::DualStation, anchor);
    let prototypes = tiles();
    let base = multi_cell_room_prototype(RoomRole::DualStation, "whole_dual_station_reject", 1);

    let assert_falls_back = |room: RoomPrototype, case: &str| {
        let snapshot = HexWfcGeometrySnapshot::project_with_rooms(&world, &prototypes, &[room])
            .unwrap_or_else(|error| panic!("{case} must fall back, not error: {error:?}"));
        let room_pieces: Vec<_> = snapshot
            .pieces
            .iter()
            .filter(|piece| piece.role == HexStructureRole::Room)
            .collect();
        assert!(
            !room_pieces.is_empty(),
            "{case}: fallback must still project"
        );
        // The per-cell fallback for a two-hex room is now the pair of wing
        // shapes that actually open toward each other, not one shape twice
        // (bug backlog #15).
        let wings: Vec<&str> = (0..2)
            .map(|index| {
                blueprint_cell_archetype(RoomRole::DualStation, index)
                    .expect("dual station cell archetype")
            })
            .collect();
        assert!(
            room_pieces.iter().all(|piece| piece
                .tile
                .as_ref()
                .is_some_and(|key| wings.contains(&key.archetype.as_str()))),
            "{case}: rejected candidate must not win, per-cell fallback must be used instead"
        );
        // Fallback covers both footprint cells individually (unlike the
        // whole-room path, whose pieces all anchor at the anchor cell).
        assert!(
            room_pieces
                .iter()
                .any(|piece| piece.source_cell == second_cell),
            "{case}: fallback must still cover the second footprint cell"
        );
    };

    // Clause: footprint must be set-equal to the stamped blueprint cells —
    // missing a cell.
    let mut missing_cell = base.clone();
    missing_cell.footprint.pop();
    assert_falls_back(missing_cell, "footprint missing a cell");

    // Clause: footprint must be set-equal — an extra, unexpected cell.
    let mut extra_cell = base.clone();
    extra_cell.footprint.push(ModuleCellRef {
        q: 5,
        r: 5,
        level: 0,
    });
    assert_falls_back(extra_cell, "footprint has an extra cell");

    // Clause: every unnamed exterior face is sealed. Adding a port to one
    // reintroduces the tile-grid perimeter that the room contract forbids.
    let mut unexpected_face = base.clone();
    unexpected_face
        .ports
        .push(observed_authoring::RoomPrototypePort {
            cell: ModuleCellRef {
                q: 1,
                r: 0,
                level: 0,
            },
            face: HexFace::SouthEast,
            class: PortClass::Door,
            name: "not_a_threshold".to_string(),
        });
    assert_falls_back(unexpected_face, "unnamed exterior face opened as Door");

    // Clause: a named threshold cannot be omitted.
    let mut missing_named_port = base.clone();
    missing_named_port
        .ports
        .retain(|port| port.name != "port_a");
    assert_falls_back(missing_named_port, "named threshold missing/Sealed");

    // Clause: every blueprint `named_port` needs a matching authored port
    // with class `Door` and the same `normalized_role(name)` — renaming the
    // authored port leaves the face itself still `Door` (so the exterior
    // face-signature loop still passes) but breaks the distinct named-port
    // identity check.
    let mut renamed_named_port = base.clone();
    let named_port = renamed_named_port
        .ports
        .iter_mut()
        .find(|port| port.name == "port_a")
        .expect("port_a is authored on the base prototype");
    named_port.name = "not_port_a".to_string();
    assert_falls_back(renamed_named_port, "named port present but misnamed");
}

/// Regression-risk capture: `push_room` (geometry.rs) checks
/// `room.hulls.len() > COLLIDER_STRIDE` exactly **once**, for the whole
/// multi-cell footprint. The per-cell fallback (`push_tile`) checks the same
/// bound **once per cell** instead. So a two-cell `DualStation` fallback kit
/// can carry up to `COLLIDER_STRIDE * 2` hulls total, but promoting the same
/// room to a whole-room module caps it at `COLLIDER_STRIDE` — half the
/// budget, and proportionally worse for larger footprints (Decision's
/// three-cell fallback allows `COLLIDER_STRIDE * 3`). A sufficiently detailed
/// authored room module can therefore regress from "renders fine per-cell"
/// to `HexGeometryError::TooManyHulls` purely by being promoted to a
/// whole-room module, with no change to its actual geometric complexity.
#[test]
fn whole_room_hull_budget_is_shared_across_the_entire_footprint_not_per_cell() {
    let anchor = HexCoord {
        q: 0,
        r: 0,
        level: 0,
    };
    let world = multi_cell_world(RoomRole::DualStation, anchor);
    let mut room =
        multi_cell_room_prototype(RoomRole::DualStation, "whole_dual_station_oversized", 1);
    let oversized = COLLIDER_STRIDE + 1;
    // Hull point data is irrelevant here: `push_room`'s length check runs
    // before any per-hull validation, so a single placeholder point per hull
    // is enough to exercise the budget check without touching arena
    // validation at all.
    room.hulls = (0..oversized).map(|_| vec![Vec3::ZERO]).collect();

    let error = HexWfcGeometrySnapshot::project_with_rooms(&world, &[], &[room])
        .expect_err("a room over the shared hull budget must be rejected");
    assert_eq!(
        error,
        HexGeometryError::TooManyHulls {
            coord: anchor,
            hulls: oversized,
        }
    );
}

/// A district-exclusive tile must be unreachable from a foreign district.
///
/// Exclusivity has to be a property of the selector, not a convention about how
/// tiles are keyed. `HexTileCatalogue::select` tries the exact `(archetype,
/// register, signature)` first and falls back to `generic` — so a tile keyed to Liminal
/// Grid can only ever be reached by asking for Liminal Grid, and a widened
/// fallback (or a stray `generic` relabel) would be the way that breaks. This
/// pins it by asking every other register for every exclusive tile's signature
/// and checking none of them hands it back.
#[test]
fn a_district_exclusive_tile_never_answers_for_another_district() {
    let tiles = tiles();
    let catalogue = HexTileCatalogue::new(&tiles);
    let registers: Vec<&str> = ArchitectureRegister::ALL
        .iter()
        .map(|register| register.slug())
        .collect();

    let exclusive: Vec<&TilePrototype> = tiles
        .iter()
        .filter(|tile| tile.key.register == ArchitectureRegister::LiminalGrid.slug())
        .collect();
    assert!(
        !exclusive.is_empty(),
        "Liminal Grid should have tiles of its own to be exclusive about"
    );

    let mut checked = 0usize;
    for tile in exclusive {
        // Only archetypes the solver actually asks for can leak through
        // selection; `hall_cap` and friends exist in the kit but are never
        // demanded, so there is nothing to probe.
        let Some(archetype) = observed_facility::hex_wfc::geometry_demands()
            .into_iter()
            .map(|demand| demand.archetype)
            .find(|candidate| *candidate == tile.key.archetype)
        else {
            continue;
        };
        for foreign in &registers {
            if *foreign == tile.key.register {
                continue;
            }
            // Every variation key, not one: selection is weighted, so a single
            // probe could miss a leak that only shows on some rolls.
            for variation in 0..16u64 {
                let key = variation.wrapping_mul(0x9E37_79B9_7F4A_7C15);
                let picked = catalogue
                    .select(archetype, foreign, tile.signature, None, key, key)
                    .unwrap_or(None);
                if let Some(picked) = picked {
                    assert_ne!(
                        picked.key.register, tile.key.register,
                        "{foreign} was handed a {} tile ({archetype}, {:?})",
                        tile.key.register, tile.signature
                    );
                }
            }
            checked += 1;
        }
    }
    assert!(
        checked > 100,
        "unexpectedly small exclusivity probe: {checked}"
    );
}

/// End to end: no cell in a solved facility is built out of another district's
/// geometry.
///
/// The catalog-side gate proves every demand has an exact tile for every
/// register. This proves the selector actually reaches them on a real solve,
/// which is the claim a player can see. Before Phase 110 the generated kit was
/// one institutional library relabelled `generic`, so nine of the ten districts
/// were built almost entirely from a tenth district's geometry however they were
/// lit or composed.
#[test]
fn every_placed_cell_is_built_from_its_own_district() {
    let prototypes = tiles();
    // Four seeds rather than one. This gate ran on `SHOWCASE_SEED` alone and
    // passed for years on a rule that was wrong - measured, the old rule reports
    // foreign geometry on three of eight seeds at the lattice size before T-4
    // and five of eight after it, and the pinned seed happened to be a clean one
    // both times until the size moved. A property this cheap to break should not
    // rest on one draw.
    for offset in 0..4u64 {
        let seed = SHOWCASE_SEED ^ offset.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        check_one_facility_is_built_from_its_own_districts(seed, &prototypes);
    }
}

fn check_one_facility_is_built_from_its_own_districts(seed: u64, prototypes: &[TilePrototype]) {
    let world = HexWfcWorld::generate(seed, HexWfcConfig::arc_default()).expect("world solves");
    let snapshot = HexWfcGeometrySnapshot::project(&world, prototypes).expect("projects");
    // Against the register that governs each piece's **assembly**, not the one
    // under its own feet.
    //
    // This used to read `world.architecture` per cell and exempt `stair_tower`
    // by name, because a tower is chosen for the whole column from its base
    // cell (Phase 109) and so need not match the cell it stands in. The
    // exemption was a name where the selector had already moved to a property:
    // `compatibility_scope` gives `VerticalColumn` to *any* archetype presenting
    // a `ShaftOpen` face, and its own comment says so - "unlike the string it
    // stays true for any tower an author draws next".
    //
    // The two-level Guardian Control atrium is exactly that next thing. It opens
    // `up: ShaftOpen`, so it is column-scoped by the same rule, and this gate
    // called it foreign geometry whenever its column crossed a district
    // boundary. Measured: three of eight seeds at the old lattice size and five
    // of eight at the new one - so the gate was passing on a lucky pinned seed
    // rather than on the property, which is precisely the failure Arc T's own
    // plan warns about.
    //
    // Asking `assembly_register` removes the exemption rather than widening it.
    // A tower and an atrium are now both checked, against the cell that actually
    // decides them.
    let catalogue = HexTileCatalogue::new(prototypes);
    let mut foreign: BTreeMap<String, usize> = BTreeMap::new();
    let mut own = 0usize;
    for piece in &snapshot.pieces {
        let Some(tile) = piece.tile.as_ref() else {
            continue;
        };
        let Some(register) =
            catalogue.assembly_register(&world, piece.source_cell, &tile.archetype)
        else {
            continue;
        };
        if tile.register == register {
            own += 1;
        } else {
            *foreign.entry(tile.register.clone()).or_default() += 1;
        }
    }
    assert!(
        own > 1_000,
        "seed {seed:#x}: unexpectedly small sample: {own}"
    );
    assert!(
        foreign.is_empty(),
        "seed {seed:#x}: {} colliders are drawn from another district's kit: {foreign:?}",
        foreign.values().sum::<usize>()
    );
}

// ------------------------------------------------- TR-9 acceptance: two families

/// The corpus must be able to build the solver's **whole** vocabulary at
/// production scale, not merely the part a given profile happens to reach.
///
/// This exists because it did not, and nothing noticed. `HexArchetype::Expanse`
/// is a third of an unprofiled production layout, and the studio's 12x9 working
/// lattice is too narrow to place a single one — so every instrument pointed at
/// this corpus reported full coverage while the largest archetype went
/// unexercised. An unprofiled solve is the useful thing to project for exactly
/// that reason: it asks what the corpus *could* be asked for, rather than what
/// today's composition happens to ask.
#[test]
fn the_corpus_builds_the_solvers_whole_vocabulary_at_production_scale() {
    let catalog = crate::hex_wfc::test_catalog();
    let world = HexWfcWorld::generate(0xa11c_0000_0000_0000, HexWfcConfig::arc_default())
        .expect("production dimensions solve");

    let mut placed: std::collections::BTreeMap<HexArchetype, usize> =
        std::collections::BTreeMap::new();
    for placement in world.placements.values() {
        if placement.archetype != HexArchetype::Void {
            *placed.entry(placement.archetype).or_default() += 1;
        }
    }
    assert!(
        placed.contains_key(&HexArchetype::Expanse),
        "this seed is meant to exercise Expanse; without it the test proves nothing"
    );

    HexWfcGeometrySnapshot::project_with_rooms(&world, &catalog.cells, &catalog.rooms)
        .unwrap_or_else(|error| {
            panic!(
                "the corpus cannot build a production layout: {error:?}\n\
                 placed archetypes: {placed:?}"
            )
        });
}

/// Sample what geometry a cell actually presents at one of its lateral faces:
/// the lowest and highest Y of any hull vertex sitting on that face's boundary
/// plane.
///
/// The projected twin of `seam_auditor::sample_face_signature`, which reads
/// module-local `.map` geometry. Both answer the same question — *what floor
/// and headroom does this boundary really offer* — but only this one can be
/// asked of a **projected** facility, which is the only place the generated
/// compatibility library's geometry exists.
///
/// Piece points are **cell-local**: `observed_cutaway` adds `hex_origin` when
/// it batches them, so they arrive here unoffset. Two lateral neighbours share
/// a level, so their local Y values are directly comparable and the face edge
/// is read in each cell's own frame.
fn projected_face_extent(
    snapshot: &HexWfcGeometrySnapshot,
    coord: HexCoord,
    face: HexFace,
) -> Option<(f32, f32)> {
    // The nominal hex is quantized to x=7, z=4/8. A 60-degree turn of
    // its corner differs from the nominal corner by about 0.0718 m. The
    // source audit sees unrotated geometry; this projection audit must admit
    // that known plan-space rounding or it misses a rotated floor entirely
    // and reports the next lintel as the floor. Height tolerance stays 0.05 m.
    const BOUNDARY_EPSILON: f32 = 0.08;

    let [(ax, az), (bx, bz)] = observed_hex::metrics::face_edge(face);
    #[allow(clippy::cast_precision_loss)]
    let a = (ax as f32, az as f32);
    #[allow(clippy::cast_precision_loss)]
    let b = (bx as f32, bz as f32);

    let on_edge = |x: f32, z: f32| {
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        let length_sq = dx * dx + dz * dz;
        if length_sq <= f32::EPSILON {
            return false;
        }
        let t = (((x - a.0) * dx + (z - a.1) * dz) / length_sq).clamp(0.0, 1.0);
        let (px, pz) = (a.0 + dx * t, a.1 + dz * t);
        (x - px).hypot(z - pz) <= BOUNDARY_EPSILON
    };

    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    for piece in &snapshot.pieces {
        if piece.source_cell != coord {
            continue;
        }
        let observed_traversal::ColliderShape::ConvexHull { points } = &piece.shape else {
            continue;
        };
        for point in points {
            if on_edge(point.x, point.z) {
                low = low.min(point.y);
                high = high.max(point.y);
            }
        }
    }
    // Clamp the span to one level, for the reason `sample_face_signature`
    // gives: on a two-level tile the wall mass above the lintel runs the tile's
    // full height, so an unclamped sample reports 16 m of "headroom" for an
    // ordinary door and falsely mismatches its 8 m neighbour.
    (low.is_finite() && high.is_finite())
        .then(|| (low, (high - low).min(observed_hex::TILE_LEVEL_HEIGHT)))
}

/// Every open seam in a projected production facility must actually meet.
///
/// `tilec audit-seams` checks the 125 committed `.map` sources. The **generated**
/// compatibility library is most of what a player walks through and has never
/// been audited at all — it is built in code at load time, so it is in no
/// directory the auditor scans. This closes that gap where it matters most: on
/// real placements, in a real projection, at production dimensions.
#[test]
fn every_open_seam_in_a_projected_facility_actually_meets() {
    let catalog = crate::hex_wfc::test_catalog();
    let world = HexWfcWorld::generate(0xa11c_0000_0000_0000, HexWfcConfig::arc_default())
        .expect("production dimensions solve");
    let snapshot =
        HexWfcGeometrySnapshot::project_with_rooms(&world, &catalog.cells, &catalog.rooms)
            .expect("production layout projects");

    let grid = world.config.grid();
    let mut checked = 0usize;
    let mut floor_mismatches: Vec<String> = Vec::new();
    let mut headroom_mismatches: Vec<String> = Vec::new();
    let mut headroom_shapes: Vec<(f32, f32)> = Vec::new();

    for (&coord, placement) in &world.placements {
        for face in HexFace::LATERAL {
            if !placement.is_open(face) {
                continue;
            }
            // Each seam once: only walk it from the lower-sorting side.
            let Some(neighbour) = grid.neighbor(coord, face) else {
                continue;
            };
            if neighbour < coord || !world.placements.contains_key(&neighbour) {
                continue;
            }
            let (Some(near), Some(far)) = (
                projected_face_extent(&snapshot, coord, face),
                projected_face_extent(&snapshot, neighbour, face.opposite()),
            ) else {
                continue;
            };
            checked += 1;
            let entry = format!(
                "{coord:?} {face:?} floor {:.3}/{:.3} headroom {:.3}/{:.3}",
                near.0, far.0, near.1, far.1
            );
            if (near.0 - far.0).abs() > 0.05 {
                floor_mismatches.push(entry.clone());
            }
            if (near.1 - far.1).abs() > 0.05 {
                headroom_shapes.push((near.1, far.1));
                headroom_mismatches.push(entry);
            }
        }
    }

    // A floor on the sample, not on the facility. It was 5,000 when a
    // production lattice was 5,600 cells; T-4 took the lattice to 3,264 and the
    // seam count fell with it, to about 3,050. The gate is unchanged in what it
    // proves - every open seam in a whole projected facility meets - and this
    // number only exists so a harness that silently stopped projecting cannot
    // pass by checking nothing.
    assert!(
        checked > 2_500,
        "only {checked} seams sampled; this gate proves nothing that small"
    );

    // Floors must agree everywhere. There is no benign reason for two cells to
    // offer different floor heights at a seam the solver says you can walk.
    assert!(
        floor_mismatches.is_empty(),
        "{} of {checked} open seams disagree about floor height:\n{}",
        floor_mismatches.len(),
        sample(&floor_mismatches)
    );

    // Headroom carries one **pre-existing** disagreement, present before this
    // gate was written and not introduced by it: a face whose geometry touches
    // the boundary plane only up to the lintel reads 4.5 m (`DOOR_TOP`), while
    // one whose wall mass above the lintel reaches the plane reads the full
    // 8 m level. It is a modelling difference between the generated library and
    // the authored corpus, not a hole — both sides still present a floor and a
    // doorway at the same heights.
    //
    // So this pins the shape rather than demanding zero: any *other* headroom
    // pair is a new defect and fails, and the count may not grow. That is what
    // makes this gate useful to the work it was built for — rewriting all of
    // the generated geometry must not make seams worse.
    const KNOWN_HEADROOM_DISAGREEMENTS: usize = 259;
    for (a, b) in &headroom_shapes {
        let pair = (a.min(*b), a.max(*b));
        assert!(
            (pair.0 - 4.5).abs() < 0.01 && (pair.1 - 8.0).abs() < 0.01,
            "unfamiliar headroom disagreement {pair:?}; the known one is 4.5 vs 8.0"
        );
    }
    assert!(
        headroom_mismatches.len() <= KNOWN_HEADROOM_DISAGREEMENTS,
        "headroom disagreements rose from {KNOWN_HEADROOM_DISAGREEMENTS} to {} of {checked}:\n{}",
        headroom_mismatches.len(),
        sample(&headroom_mismatches)
    );
}

fn sample(entries: &[String]) -> String {
    entries
        .iter()
        .take(10)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Walk `spine` up from its first node to its last (or down, from the last to the
/// first) on the production controller, steering each tick toward the point the spine
/// says to walk to next - the follower's own rule. The ticks it took, or where it
/// stalled.
fn walk_spine(scene: &RapierTraversalScene, spine: &StairSpine, up: bool) -> Result<u32, Vec3> {
    let config = FpsConfig::default();
    let start = if up {
        spine.nodes[0]
    } else {
        *spine.nodes.last().expect("a spine")
    };
    let mut body = FpsBody::spawned(start + Vec3::Y * config.half_height, 0.0);
    for ticks in 0..1_200 {
        let feet = body.position - Vec3::Y * config.half_height;
        let done = if up {
            spine.has_arrived(feet)
        } else {
            spine.has_descended(feet)
        };
        if done {
            return Ok(ticks);
        }
        let target = spine.target(feet, up).expect("a target on the spine");
        let toward = (target - feet).with_y(0.0).normalize_or_zero();
        body.yaw = toward.x.atan2(-toward.z);
        let intent = PlayerIntent {
            movement: Vec2::Y,
            ..PlayerIntent::default()
        };
        step_character(scene, &mut body, intent, &config, 1.0 / 60.0);
    }
    Err(body.position - Vec3::Y * config.half_height)
}

/// Every climb composition in a production facility climbs and descends end to end.
///
/// The four cells' spines, chained in climbing order, are one flight from the foot's
/// door to the landing's: a body stood at the foot steers along it on the production
/// controller and must reach the landing, and stood at the landing must come back
/// down. The proof that the cells meet - floor heights at every span, the flight
/// under the ceilings, the landing's lip - with nothing to step over or catch on.
#[test]
fn every_production_climb_composition_climbs_and_descends_end_to_end() {
    use observed_facility::hex_wfc::ClimbPart;

    let catalog = crate::hex_wfc::test_catalog();
    let mut climbs = 0usize;
    let mut slowest = (0u32, 0u32);
    let mut stalls = Vec::new();
    for seed in [1u64, 2, 3] {
        let world = HexWfcWorld::generate_with_profile(
            seed,
            HexWfcConfig::arc_default(),
            None,
            &catalog.composition,
        )
        .expect("production seed solves");
        let snapshot =
            HexWfcGeometrySnapshot::project_with_rooms(&world, &catalog.cells, &catalog.rooms)
                .expect("production corpus projects");
        let scene = snapshot.rapier_scene();
        let grid = world.config.grid();
        for placement in world.placements.values() {
            let HexArchetype::Climb {
                part: ClimbPart::Foot,
                heading,
            } = placement.archetype
            else {
                continue;
            };
            let cells = observed_facility::hex_wfc::composition_in(
                grid,
                &world.placements,
                placement.coord,
            )
            .expect("a whole composition");
            let shape = (
                world.placements[&cells[1]].archetype,
                world.placements[&cells[3]].archetype,
            );
            let mut nodes: Vec<Vec3> = Vec::new();
            for cell in cells {
                for &node in &snapshot.climbs[&cell].nodes {
                    if nodes.last().is_none_or(|last| last.distance(node) > 0.05) {
                        nodes.push(node);
                    }
                }
            }
            let spine = StairSpine { nodes };
            climbs += 1;
            for up in [true, false] {
                match walk_spine(&scene, &spine, up) {
                    Ok(ticks) if up => slowest.0 = slowest.0.max(ticks),
                    Ok(ticks) => slowest.1 = slowest.1.max(ticks),
                    Err(at) => stalls.push((seed, placement.coord, heading, shape, up, at)),
                }
            }
        }
    }
    eprintln!(
        "climbs={climbs} slowest climb={} descent={} ticks stalls={}",
        slowest.0,
        slowest.1,
        stalls.len()
    );
    assert!(climbs >= 10, "production facilities place climbs: {climbs}");
    assert!(stalls.is_empty(), "climbs stalled: {stalls:?}");
}

/// The controller must cross every causeway and gallery in both directions.
/// The reservoir's internal joins must also be open above the causeways.
#[test]
fn cistern_crossings_and_galleries_are_physical_for_every_rotation() {
    let mut world = showcase();
    world.blueprints.clear();
    for p in world.placements.values_mut() {
        p.space = HexSpace::Void;
        p.archetype = HexArchetype::Void;
        p.doors = 0;
        p.up = PortClass::Sealed;
        p.down = PortClass::Sealed;
    }
    let tiles = tiles();
    let anchor = HexCoord {
        q: 4,
        r: 4,
        level: 0,
    };
    for rotation in 0..6 {
        let sectors =
            observed_facility::hex_wfc::authored_cistern_room(world.config, anchor, rotation)
                .unwrap();
        let mut fixture = world.clone();
        for sector in sectors {
            fixture.placements.insert(sector.coord, sector);
        }
        let geometry =
            HexWfcGeometrySnapshot::project(&fixture, &tiles).expect("bespoke reservoir projects");
        let scene = RapierTraversalScene::from_arena_spec(&geometry.arena);
        let centers = sectors.map(|p| Vec3::from_array(hex_origin(p.coord)) + Vec3::Y * 0.75);
        let room_center = (centers[0] + centers[1] + centers[2]) / 3.0;
        for reverse in [false, true] {
            let mut route = centers.to_vec();
            route.push(centers[0]);
            if reverse {
                route.reverse();
            }
            walk_cistern_route(&scene, &route)
                .unwrap_or_else(|feet| panic!("rotation {rotation} crossing stalled at {feet:?}"));
        }
        for sector in sectors {
            let HexArchetype::Cistern { heading } = sector.archetype else {
                unreachable!()
            };
            let turn = glam::Quat::from_rotation_y(
                -(heading.index() as f32) * std::f32::consts::TAU / 6.0,
            );
            let origin = Vec3::from_array(hex_origin(sector.coord));
            let gallery: Vec<_> = [2, 3, 4, 5, 0]
                .into_iter()
                .map(|corner| {
                    let (x, z) = observed_hex::CORNERS[corner];
                    origin + turn * Vec3::new(x as f32 * 0.8, 0.75, z as f32 * 0.8)
                })
                .collect();
            for reverse in [false, true] {
                let mut route = gallery.clone();
                if reverse {
                    route.reverse();
                }
                walk_cistern_route(&scene, &route).unwrap_or_else(|feet| {
                    panic!("rotation {rotation}, sector {heading:?} gallery stalled at {feet:?}")
                });
            }
            for face in HexFace::LATERAL {
                if sector.ports().port(face) != PortClass::Span {
                    continue;
                }
                let [a, b] = observed_hex::face_edge(face);
                let a = Vec3::new(a.0 as f32, 0.0, a.1 as f32);
                let b = Vec3::new(b.0 as f32, 0.0, b.1 as f32);
                let midpoint = Vec3::from_array(hex_origin(sector.coord)) + (a + b) * 0.5;
                let tangent = (b - a).normalize();
                let normal = ((a + b) * 0.5).normalize();
                // Where an open span ends at the exterior, adjoining wall
                // corners must seal. Exact rotations of a quantized hex left
                // a narrow full-height slit here, visible in the first capture.
                let origin = Vec3::from_array(hex_origin(sector.coord));
                let corner = [origin + a, origin + b]
                    .into_iter()
                    .max_by(|a, b| {
                        a.distance_squared(room_center)
                            .total_cmp(&b.distance_squared(room_center))
                    })
                    .unwrap();
                let outward = (corner - room_center).with_y(0.0).normalize();
                let across = Vec3::new(-outward.z, 0.0, outward.x);
                for offset in [-0.02, 0.0, 0.02] {
                    let from = corner - outward * 0.2 + across * offset + Vec3::Y * 2.0;
                    assert!(
                        scene.ray_distance(from, outward, 0.6).is_some(),
                        "exterior slit at rotation {rotation}, seam {face:?}, offset {offset}"
                    );
                }
                for along in [-3.0, 0.0, 3.0] {
                    for height in [1.8, 5.5] {
                        let from = midpoint + tangent * along + Vec3::Y * height - normal * 0.4;
                        assert_eq!(
                            scene.ray_distance(from, normal, 0.8),
                            None,
                            "rotation {rotation}, seam {face:?}, height {height}, offset {along}"
                        );
                    }
                }
            }
        }
    }
}

/// Walk real collision geometry, including the 25 cm gallery/deck steps.
fn walk_cistern_route(scene: &RapierTraversalScene, route: &[Vec3]) -> Result<(), Vec3> {
    let config = FpsConfig::default();
    let mut body = FpsBody::spawned(route[0] + Vec3::Y * config.half_height, 0.0);
    for target in &route[1..] {
        let mut arrived = false;
        for _ in 0..600 {
            let feet = body.position - Vec3::Y * config.half_height;
            let toward = (*target - feet).with_y(0.0);
            if toward.length() < 0.25 {
                arrived = true;
                break;
            }
            body.yaw = toward.x.atan2(-toward.z);
            step_character(
                scene,
                &mut body,
                PlayerIntent {
                    movement: Vec2::Y,
                    ..PlayerIntent::default()
                },
                &config,
                1.0 / 60.0,
            );
        }
        if !arrived {
            return Err(body.position - Vec3::Y * config.half_height);
        }
    }
    Ok(())
}
