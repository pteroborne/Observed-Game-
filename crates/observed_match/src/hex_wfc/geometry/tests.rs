use observed_content::ArchitectureRegister;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{
    HexArchetype, HexMutationRegion, HexObservationFrame, HexRelayoutProgress, HexSpace,
    HexWfcConfig, HexWfcWorld,
};
use observed_hex::{HexFace, hex_origin};
use observed_traversal::rapier_controller::step_character;
use observed_traversal::{FpsBody, FpsConfig};
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
    assert!(
        snapshot
            .guides
            .values()
            .any(|guide| guide.climb.is_some() && guide.deck.is_some()),
        "the fixture must exercise one atomic climb-plus-deck module guide"
    );
}

#[test]
fn guide_delta_replaces_and_removes_climb_and_deck_atomically() {
    let world = showcase();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projection");
    let (&coord, original) = snapshot
        .guides
        .iter()
        .find(|(_, guide)| guide.climb.is_some() && guide.deck.is_some())
        .expect("fixture has a climb-plus-deck guide");
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
#[test]
fn production_catalog_selection_is_pinned_for_spectator_seeds() {
    let catalog = crate::hex_wfc::test_catalog();
    let cases = [
        (
            1u64,
            293usize,
            0x080f_e5f6_32e2_aae6u64,
            45usize,
            0x95e0_1b87_e452_104cu64,
        ),
        (
            10_000_031u64,
            238usize,
            0xca66_060f_aa6a_b9aau64,
            29usize,
            0xe5db_a473_4a53_b1a3u64,
        ),
    ];
    for (seed, expected_count, expected_digest, expected_tower_count, expected_tower_digest) in
        cases
    {
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
        let towers = selections
            .iter()
            .filter(|(_, tile)| tile.archetype == "stair_tower")
            .map(|(coord, tile)| (*coord, tile.clone()))
            .collect::<BTreeMap<_, _>>();
        let tower_digest = selection_digest(&towers);
        eprintln!(
            "seed={seed} count={} digest={digest:#018x} tower_count={} tower_digest={tower_digest:#018x}",
            selections.len(),
            towers.len()
        );
        assert_eq!(selections.len(), expected_count);
        assert_eq!(digest, expected_digest);
        assert_eq!(towers.len(), expected_tower_count);
        assert_eq!(tower_digest, expected_tower_digest);
    }
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
        if placement.space == HexSpace::Void {
            continue;
        }
        if placement.archetype == HexArchetype::RampHead {
            assert!(
                world
                    .config
                    .grid()
                    .neighbor(placement.coord, HexFace::Down)
                    .is_some()
            );
        } else {
            assert!(covered.contains(&placement.coord), "missing {placement:?}");
        }
    }
    assert_eq!(snapshot.blueprint_instances, world.blueprints.len());
    assert!(snapshot.ramp_heads > 0, "showcase includes paired ramps");
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
fn projected_ramp_pair_is_walkable_in_the_continuous_scene() {
    let world = showcase();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projection");
    let scene = snapshot.rapier_scene();
    let ramp = world
        .placements
        .values()
        .find(|placement| placement.archetype == HexArchetype::RampUp)
        .expect("showcase ramp");
    let entrance = HexFace::LATERAL
        .into_iter()
        .find(|&face| ramp.is_open(face))
        .expect("ramp entrance");
    let [a, b] = observed_hex::face_edge(entrance);
    let outward = Vec2::new((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5).normalize();
    let origin = Vec3::from_array(hex_origin(ramp.coord));
    let config = FpsConfig::default();
    let start_feet = origin + Vec3::new(outward.x * 6.3, 0.95, outward.y * 6.3);
    let facing = -outward;
    let mut body = FpsBody::spawned(
        start_feet + Vec3::Y * config.half_height,
        facing.x.atan2(-facing.y),
    );
    let intent = PlayerIntent {
        movement: Vec2::Y,
        ..PlayerIntent::default()
    };
    let mut max_feet = start_feet.y;
    for _ in 0..240 {
        step_character(&scene, &mut body, intent, &config, 1.0 / 60.0);
        max_feet = max_feet.max(body.position.y - config.half_height);
    }
    assert!(
        max_feet - start_feet.y >= TILE_LEVEL_HEIGHT - 0.6,
        "placed ramp rises one full level: start={} max={max_feet}",
        start_feet.y
    );
}

#[test]
fn boundary_shell_traces_the_rhombic_domain_outline() {
    let world = showcase();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projection");
    let outline = rhombus_outline(&world);
    let boundaries: Vec<_> = snapshot
        .pieces
        .iter()
        .filter(|piece| piece.role == HexStructureRole::Boundary)
        .collect();
    assert_eq!(boundaries.len(), outline.len());
    assert!(
        outline.len() >= 6,
        "quantized rhombus has a faceted outline"
    );
}

#[test]
fn boundary_start_uses_its_authored_blueprint_signature_and_the_shell_closes_it() {
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
    assert!(
        snapshot
            .pieces
            .iter()
            .any(|piece| piece.role == HexStructureRole::Boundary)
    );
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
    if let Some(ramp) = world
        .placements
        .values()
        .find(|placement| placement.archetype == HexArchetype::RampUp)
    {
        frame.visible_cells.insert(ramp.coord);
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
            .all(|piece| logical.changed_cells.contains(&piece.source_cell))
    );
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
    assert_eq!(incremental.ramp_heads, after.ramp_heads);
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
        .filter(|placement| placement.space != HexSpace::Void)
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

/// A shaft column must use one tower shape all the way up.
///
/// A tower's stairwell opening is the hole the flight below arrives through, so
/// two shapes in one column leave the lower flight topping out under the upper
/// cell's solid deck. The surfaces union, so nothing reads as broken — a body
/// simply climbs into the underside of the floor above and stops. Districts
/// drift between levels, so a column crossing a district boundary is routine
/// rather than rare, and choosing the tower per cell made this the common case:
/// measured as a soak stall the moment a second tower shape shipped.
#[test]
fn a_shaft_column_uses_one_tower_shape() {
    let world = HexWfcWorld::generate(SHOWCASE_SEED, HexWfcConfig::arc_default()).expect("solves");
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projects");
    let mut per_column: BTreeMap<(u16, u16), BTreeSet<String>> = BTreeMap::new();
    for piece in &snapshot.pieces {
        let Some(tile) = piece.tile.as_ref() else {
            continue;
        };
        if tile.archetype != "stair_tower" {
            continue;
        }
        per_column
            .entry((piece.source_cell.q, piece.source_cell.r))
            .or_default()
            .insert(tile.register.clone());
    }
    assert!(
        !per_column.is_empty(),
        "the pinned seed should place stair towers"
    );
    for (column, registers) in &per_column {
        assert_eq!(
            registers.len(),
            1,
            "column {column:?} mixes tower shapes: {registers:?}"
        );
    }

    // And the handed districts really are reaching their own towers, or the
    // check above would pass on a facility that still has only one shape.
    let shapes: BTreeSet<_> = per_column.values().flatten().cloned().collect();
    assert!(
        shapes.len() > 1,
        "every column drew the same tower, so vertical circulation is still a \
         monoculture: {shapes:?}"
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
                    .select(archetype, foreign, tile.signature, key, key)
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

/// Two declared tower families, distinguishable by their climbs.
const TOWER_FAMILIES: [(&str, f32); 2] = [("test/tower-a", 7.75), ("test/tower-b", 7.50)];
/// Two turns each. A turn belongs to the assembly variant, so a column drawing
/// one turn at one level and another at the next is exactly the fault under
/// test, not a cosmetic difference.
const TOWER_ROTATIONS: [u8; 2] = [0, 3];

/// Decode which assembly variant a projected tower came from. The variant
/// number carries it because `TileKey` is what a projected piece reports.
fn tower_variant(variant: u16) -> (usize, u8) {
    let packed = variant / 256;
    (usize::from(packed / 6), (packed % 6) as u8)
}

/// The runtime shape a second complete tower family expands into.
///
/// `observed_authoring`'s `two_complete_tower_families_expand_into_runtime_prototypes`
/// proves the same thing one layer down, through the real contract compiler.
/// Here the point is what selection does with the result, so the kit is built
/// directly against every `stair_tower` signature the solver can demand: each
/// family answers all of them, at every turn it accepts, which is what makes it
/// *complete*. Both families are `generic`, so every district reaches both and
/// register fallback cannot hide a mix.
///
/// The match layer reads only `TilePrototype::assembly`. No bot or match
/// conditional knows any of these names, and adding one would mean the
/// selection design had failed.
fn two_family_tower_kit() -> Vec<TilePrototype> {
    let template = tiles()
        .into_iter()
        .find(|tile| tile.key.archetype == "stair_tower")
        .expect("the committed corpus ships towers to model");
    let signatures = observed_facility::hex_wfc::geometry_demands()
        .into_iter()
        .filter(|demand| demand.archetype == "stair_tower")
        .map(|demand| demand.signature)
        .collect::<Vec<_>>();
    assert!(
        signatures.len() > 8,
        "the tower demand set should be substantial: {}",
        signatures.len()
    );

    let mut kit = Vec::new();
    for (family_index, (family, climb_top)) in TOWER_FAMILIES.into_iter().enumerate() {
        for (rotation_index, rotation) in TOWER_ROTATIONS.into_iter().enumerate() {
            for (signature_index, &signature) in signatures.iter().enumerate() {
                let packed = family_index * 6 + rotation_index;
                let mut tile = template.clone();
                tile.key.register = "generic".to_string();
                tile.key.variant = u16::try_from(packed * 256 + signature_index)
                    .expect("the synthetic kit fits a variant");
                tile.signature = signature;
                tile.weight = 1;
                // The climbs really do reach different heights, so mixing two
                // families in one column would leave a flight short of the deck
                // above it rather than merely looking different.
                tile.spine = StairSpine {
                    nodes: vec![
                        Vec3::new(0.0, 0.5, 0.0),
                        Vec3::new(0.0, climb_top - f32::from(rotation) * 0.05, 0.0),
                    ],
                };
                tile.contract = None;
                tile.assembly = Some(observed_authoring::RuntimeAssembly {
                    variant: observed_authoring::AssemblyVariantId {
                        family: ModuleFamilyId(family.to_string()),
                        rotation,
                    },
                    scope: AssemblyScope::VerticalColumn,
                    family_weight: 1,
                });
                kit.push(tile);
            }
        }
    }
    kit
}

/// Every prototype except the committed towers, which the two declared families
/// replace wholesale.
fn tiles_with_two_tower_families() -> Vec<TilePrototype> {
    let mut prototypes = tiles()
        .into_iter()
        .filter(|tile| tile.key.archetype != "stair_tower")
        .collect::<Vec<_>>();
    prototypes.extend(two_family_tower_kit());
    prototypes
}

/// Which assembly variant each column drew, keyed by its plan cell.
fn tower_variants_by_column(
    snapshot: &HexWfcGeometrySnapshot,
) -> BTreeMap<(u16, u16), BTreeSet<(usize, u8)>> {
    let mut per_column: BTreeMap<(u16, u16), BTreeSet<(usize, u8)>> = BTreeMap::new();
    for piece in &snapshot.pieces {
        let Some(tile) = piece.tile.as_ref() else {
            continue;
        };
        if tile.archetype != "stair_tower" {
            continue;
        }
        per_column
            .entry((piece.source_cell.q, piece.source_cell.r))
            .or_default()
            .insert(tower_variant(tile.variant));
    }
    per_column
}

/// **The TR-9 exit criterion.** Two complete families never mix within a
/// column, across seeds, door signatures, end caps, and registers.
///
/// This is the fault that forced an entire authored stair family to be replaced
/// atomically. A column drew turn 1 at one level and turn 4 at the next, and the
/// lower flight topped out under the upper cell's solid deck: the surfaces
/// union, so nothing reads as broken, and a body simply climbs into the
/// underside of the floor above and stops. `AssemblyVariantId` exists so that
/// cannot recur, and this asserts it on real solved facilities rather than on a
/// unit fixture.
#[test]
fn two_declared_families_never_mix_inside_one_column() {
    let prototypes = tiles_with_two_tower_families();
    let mut drawn = BTreeSet::new();
    let mut columns = 0usize;
    let mut signature_variety = 0usize;

    for seed in [
        1u64,
        10_000_031,
        SHOWCASE_SEED,
        0x0BAD_C0DE_0000_0001,
        0x5EED_5EED_5EED_5EED,
    ] {
        let world = HexWfcWorld::generate(
            seed,
            HexWfcConfig {
                levels: 4,
                ..HexWfcConfig::default()
            },
        )
        .expect("seed solves");
        let snapshot =
            HexWfcGeometrySnapshot::project(&world, &prototypes).expect("two families project");

        for (column, variants) in tower_variants_by_column(&snapshot) {
            assert_eq!(
                variants.len(),
                1,
                "seed {seed:#x} column {column:?} mixes assembly variants: {variants:?}"
            );
            drawn.extend(variants);
            columns += 1;
        }

        // The columns really are being asked different questions at different
        // levels: end caps, through cells, and varying door counts. Without this
        // the assertion above could pass on a facility whose towers all
        // presented one signature.
        let mut per_column_signatures: BTreeMap<(u16, u16), BTreeSet<u16>> = BTreeMap::new();
        for piece in &snapshot.pieces {
            if let Some(tile) = piece.tile.as_ref()
                && tile.archetype == "stair_tower"
            {
                per_column_signatures
                    .entry((piece.source_cell.q, piece.source_cell.r))
                    .or_default()
                    .insert(tile.variant % 256);
            }
        }
        signature_variety += per_column_signatures
            .values()
            .filter(|signatures| signatures.len() > 1)
            .count();
    }

    assert!(columns > 50, "unexpectedly small sample: {columns} columns");
    assert!(
        signature_variety > 0,
        "no column drew two different tower signatures, so signature-invariance \
         of the family choice was never actually exercised"
    );
    assert_eq!(
        drawn.len(),
        TOWER_FAMILIES.len() * TOWER_ROTATIONS.len(),
        "every declared family and turn should be reachable, or the facility \
         only ever proved one of them coherent: {drawn:?}"
    );
}

/// A bounded relayout keeps the column's assembly identity.
///
/// Family and turn are drawn from the column's base cell, and a relayout does
/// not move that cell's variation key, so replacing part of a column reinstalls
/// the same family. Losing this would reintroduce the fault mid-match, where it
/// is hardest to see.
#[test]
fn a_relayout_reinstalls_the_same_assembly_variant() {
    let prototypes = tiles_with_two_tower_families();
    let mut world = showcase();
    world.config.retry_budget = 1;
    let before = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("before");
    let before_columns = tower_variants_by_column(&before);
    assert!(!before_columns.is_empty(), "the seed should place towers");

    let mut frame = HexObservationFrame::default();
    let room = world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.anchor != world.config.spawn())
        .expect("non-start room");
    frame.visible_cells.insert(room.cells[0]);
    if let Some(shaft) = world
        .placements
        .values()
        .find(|placement| placement.archetype == HexArchetype::Shaft)
    {
        frame.visible_cells.insert(shaft.coord);
    }
    frame.objective_cells.insert(world.config.spawn());
    let work = world.begin_relayout(&frame);
    let candidate = match world.advance_relayout(work).expect("advance") {
        HexRelayoutProgress::Ready(candidate) => candidate,
        HexRelayoutProgress::Pending(_) => panic!("retry budget one must finish"),
    };
    world
        .commit_relayout_delta(candidate, &frame)
        .expect("commit");

    let after = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("after");
    for (column, variants) in tower_variants_by_column(&after) {
        assert_eq!(
            variants.len(),
            1,
            "column {column:?} mixes assembly variants after relayout: {variants:?}"
        );
        if let Some(previous) = before_columns.get(&column) {
            assert_eq!(
                *previous, variants,
                "column {column:?} changed assembly variant across a relayout"
            );
        }
    }
}

/// A hole in one assembly variant is reported as one, not filled from a sibling
/// family and not blamed on a missing tile.
///
/// Borrowing a member from the other family is the single escape hatch that
/// would make the exit criterion above unprovable, so the selector has to refuse
/// it out loud even though a perfectly good member exists one family over.
#[test]
fn a_variant_with_a_hole_is_named_rather_than_filled_from_a_sibling() {
    let complete = two_family_tower_kit();
    let hole = complete[0].signature;
    let prototypes = tiles()
        .into_iter()
        .filter(|tile| tile.key.archetype != "stair_tower")
        .chain(complete.into_iter().filter(|tile| {
            // `test/tower-a` at turn 0 loses exactly one signature. Every other
            // family and turn still answers it.
            tower_variant(tile.key.variant) != (0, 0) || tile.signature != hole
        }))
        .collect::<Vec<_>>();

    let catalogue = HexTileCatalogue::new(&prototypes);
    assert_eq!(
        catalogue.supply("stair_tower", "monolith", hole),
        HexTileSupply::Missing,
        "coverage has to see the hole the projector would fall into"
    );

    // Probe the selector directly: whether one particular seed happens to demand
    // the missing signature from that exact variant is not the point being
    // pinned. Some assembly draw lands on it, and that draw must refuse.
    let key = (0..64u64)
        .map(|probe| probe.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .find(|&key| {
            catalogue
                .select("stair_tower", "monolith", hole, key, 0)
                .is_err()
        })
        .expect("some assembly draw must land on the variant with the hole");
    let error = catalogue
        .select("stair_tower", "monolith", hole, key, 0)
        .expect_err("the incomplete variant must refuse");
    assert_eq!(error.family, ModuleFamilyId("test/tower-a".to_string()));
    assert_eq!(error.rotation, 0);
    assert_eq!(error.register, "generic");

    // And a sibling family answers that very signature, so the refusal is a
    // deliberate choice rather than an absence of geometry.
    let sibling = (0..64u64)
        .map(|probe| probe.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .filter_map(|key| {
            catalogue
                .select("stair_tower", "monolith", hole, key, 0)
                .ok()
                .flatten()
        })
        .next()
        .expect("another family answers the signature that was refused");
    assert_ne!(tower_variant(sibling.key.variant), (0, 0));
}

/// The column rule survived losing its name.
///
/// `"stair_tower"` no longer appears anywhere in selection; assembly width is
/// now declared, or for compatibility content read from the geometry. This
/// asserts the *answer*, not the mechanism: a tower cell whose own district
/// differs from its column base's still resolves against the base, and an
/// ordinary hall still answers for itself. The crossing counter matters — on a
/// facility where no column ever left its district the two rules would be
/// indistinguishable and this would pass on nothing.
#[test]
fn a_towers_register_still_comes_from_its_column_base() {
    let tiles = tiles();
    let catalogue = HexTileCatalogue::new(&tiles);
    let mut crossings = 0usize;
    let mut halls = 0usize;

    for seed in [1u64, 10_000_031, SHOWCASE_SEED] {
        let world = HexWfcWorld::generate(
            seed,
            HexWfcConfig {
                levels: 4,
                ..HexWfcConfig::default()
            },
        )
        .expect("seed solves");
        for (coord, placement) in &world.placements {
            let Some(archetype) = observed_facility::hex_wfc::placement_tile_archetype(placement)
            else {
                continue;
            };
            let Some(resolved) = catalogue.assembly_register(&world, *coord, archetype) else {
                continue;
            };
            let own = world
                .architecture
                .get(coord)
                .map(|register| register.slug().to_string());
            if placement.archetype == HexArchetype::Shaft {
                let base = world
                    .architecture
                    .get(&HexCoord { level: 0, ..*coord })
                    .map(|register| register.slug().to_string());
                assert_eq!(Some(resolved), base, "a tower follows its column");
                if own != base {
                    crossings += 1;
                }
            } else {
                assert_eq!(Some(resolved), own, "an ordinary cell answers for itself");
                halls += 1;
            }
        }
    }

    assert!(halls > 100, "unexpectedly small hall sample: {halls}");
    assert!(
        crossings > 0,
        "no tower column crossed a district boundary, so resolving at the column \
         base was never distinguishable from resolving per cell"
    );
}

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
    const BOUNDARY_EPSILON: f32 = 0.05;

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
