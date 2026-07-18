use std::path::{Path, PathBuf};

use observed_authoring::Manifest;
use observed_facility::hex_wfc::{
    HexArchetype, HexObservationFrame, HexRelayoutProgress, HexSpace, HexWfcConfig, HexWfcWorld,
};
use observed_hex::{HexFace, hex_origin};
use observed_traversal::rapier_controller::step_character;
use observed_traversal::{FpsBody, FpsConfig};
use player_input::PlayerIntent;

use super::*;

const SHOWCASE_SEED: u64 = 0xA11C_E3D0_0000_0008;

fn tile_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
}

fn tiles() -> Vec<TilePrototype> {
    let base = tile_dir();
    Manifest::load(&base.join("manifest.ron"))
        .expect("manifest")
        .load_tiles(&base)
        .expect("tiles")
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
            .is_some_and(|key| key.archetype == "room_single")
    }));
    assert!(
        snapshot
            .pieces
            .iter()
            .any(|piece| piece.role == HexStructureRole::Boundary)
    );
}

#[test]
fn pinned_prefab_pieces_remain_byte_identical_across_a_committed_generation() {
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
    world.commit_relayout(candidate, &frame).expect("commit");
    assert_eq!(world.generation, 1);
    let after = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("after");
    let after_by_id: BTreeMap<_, _> = after.pieces.iter().map(|piece| (piece.id, piece)).collect();
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
    let world = HexWfcWorld::generate(0xA11C_9300_0000_0001, HexWfcConfig::arc_default())
        .expect("arc default solves");
    let solve_time = started.elapsed();

    let prototypes = tiles();
    let started = std::time::Instant::now();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("projection");
    let projection_time = started.elapsed();
    let started = std::time::Instant::now();
    let scene = snapshot.rapier_scene();
    let scene_build_time = started.elapsed();

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
        "ARC_L_P92_BUDGET cells={} non_void={} colliders={} solve_ms={} projection_ms={} scene_build_ms={} characters={} batch_frame_us={} character_query_us={}",
        world.config.grid().cell_count(),
        non_void,
        snapshot.pieces.len(),
        solve_time.as_millis(),
        projection_time.as_millis(),
        scene_build_time.as_millis(),
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
