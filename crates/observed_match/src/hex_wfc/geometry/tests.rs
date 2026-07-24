use observed_core::PlayerId;
use observed_facility::hex_wfc::{
    HexArchetype, HexObservationFrame, HexRelayoutProgress, HexSpace, HexWfcConfig, HexWfcWorld,
};
use observed_hex::{HexFace, hex_origin};
use observed_traversal::rapier_controller::step_character;
use observed_traversal::{FpsBody, FpsConfig};
use player_input::PlayerIntent;

use super::*;

const SHOWCASE_SEED: u64 = 0xA11C_E3D0_0000_0008;

fn tiles() -> Vec<TilePrototype> {
    observed_authoring::tile_source::compatibility_cells().expect("compatibility tiles")
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
    assert!(start_pieces.iter().all(|piece| {
        piece
            .tile
            .as_ref()
            .is_some_and(|key| key.archetype == "sanctuary")
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
            tile.key.archetype == "sanctuary"
                && (tile.key.register == register || tile.key.register == "generic")
                && tile.signature == blueprint_for_role(start.role).cell_signature((0, 0, 0))
        })
        .expect("start fallback")
        .hulls;
    let ports = HexFace::LATERAL
        .into_iter()
        .map(|face| observed_authoring::RoomPrototypePort {
            cell: ModuleCellRef {
                q: 0,
                r: 0,
                level: 0,
            },
            face,
            class: PortClass::Door,
            name: match face {
                HexFace::West => "entrance".to_string(),
                HexFace::East => "exit".to_string(),
                _ => format!("side_{face:?}"),
            },
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
