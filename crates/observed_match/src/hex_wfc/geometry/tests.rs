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
fn variation_modulo_keeps_the_full_portable_u64_key() {
    let key = u64::from(u32::MAX) + 17;
    assert_eq!(variation_index(key, 7), (key % 7) as usize);
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

/// Manual risk measurement for the arc's full 28x20x10 production-shaped grid.
/// Ignored in the ordinary suite because the large WFC solve is intentionally expensive.
#[test]
#[ignore = "manual 28x20x10 collider budget measurement"]
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
/// tiles are keyed. `Catalogue::select` tries the exact `(archetype, register,
/// signature)` first and falls back to `generic` — so a tile keyed to Liminal
/// Grid can only ever be reached by asking for Liminal Grid, and a widened
/// fallback (or a stray `generic` relabel) would be the way that breaks. This
/// pins it by asking every other register for every exclusive tile's signature
/// and checking none of them hands it back.
#[test]
fn a_district_exclusive_tile_never_answers_for_another_district() {
    let tiles = tiles();
    let catalogue = Catalogue::new(&tiles);
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
                let picked = catalogue.select(
                    archetype,
                    foreign,
                    tile.signature,
                    variation.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                );
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
    let world =
        HexWfcWorld::generate(SHOWCASE_SEED, HexWfcConfig::arc_default()).expect("world solves");
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("projects");
    let mut foreign: BTreeMap<String, usize> = BTreeMap::new();
    let mut own = 0usize;
    for piece in &snapshot.pieces {
        let Some(tile) = piece.tile.as_ref() else {
            continue;
        };
        // A stair tower is deliberately chosen for the whole column from its
        // base cell's register (Phase 109), so it need not match the cell it
        // stands in — only the column, which `a_shaft_column_uses_one_tower_shape`
        // covers.
        if tile.archetype == "stair_tower" {
            continue;
        }
        let Some(register) = world.architecture.get(&piece.source_cell) else {
            continue;
        };
        if tile.register == register.slug() {
            own += 1;
        } else {
            *foreign.entry(tile.register.clone()).or_default() += 1;
        }
    }
    assert!(own > 1_000, "unexpectedly small sample: {own}");
    assert!(
        foreign.is_empty(),
        "{} colliders are drawn from another district's kit: {foreign:?}",
        foreign.values().sum::<usize>()
    );
}
