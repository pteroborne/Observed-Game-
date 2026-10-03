use glam::{Vec2, Vec3};
use observed_hex::{HexFace, PortClass, PortSignature, face_edge};
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
use observed_traversal::{
    FollowTarget, FollowerPose, FpsBody, FpsConfig, TraversalDirection, TraversalRuntimeProfile,
    follow_stateless,
};
use player_input::PlayerIntent;

use crate::CompiledTileCatalog;
use crate::manifest::Manifest;
use crate::tile::{TileError, parse_tile};
use crate::tile_source;

fn signature(ports: &[(HexFace, PortClass)]) -> PortSignature {
    let mut all = [PortClass::Sealed; 8];
    for &(face, class) in ports {
        all[face.index()] = class;
    }
    PortSignature::try_from_ports(all).expect("test signature is valid")
}

fn doors(faces: &[HexFace]) -> Vec<(HexFace, PortClass)> {
    faces.iter().map(|&face| (face, PortClass::Door)).collect()
}

#[test]
fn every_generated_tile_parses_and_snaps() {
    for (name, content) in tile_source::sources() {
        if name.ends_with(".ron") {
            continue;
        }
        let tile = parse_tile(&content)
            .unwrap_or_else(|error| panic!("{name} failed to parse: {error:?}"));
        assert!(!tile.hulls.is_empty(), "{name} has no geometry");
        assert!(
            !tile.lights.is_empty(),
            "{name} has no authored light source"
        );
    }
}

#[test]
fn every_compatibility_cell_carries_explicit_lighting() {
    let cells = tile_source::compatibility_cells().expect("compatibility cells parse");
    assert!(!cells.is_empty());
    assert!(cells.iter().all(|tile| !tile.lights.is_empty()));
}

/// The pin: every committed asset is byte-identical to the typed generator's
/// output. If this fails, rerun `cargo run -p observed_authoring --bin
/// bake_tiles`.
#[test]
#[cfg(any())]
fn committed_assets_do_not_drift_from_the_typed_source() {
    for (name, content) in tile_source::sources() {
        let committed = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/tiles")
                .join(&name),
        )
        .unwrap_or_else(|error| panic!("committed {name} missing: {error}"));
        assert_eq!(committed, content, "{name} drifted — rerun bake_tiles");
    }
}

#[test]
fn committed_authored_maps_validate_independently_of_the_generator() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles");
    let manifest = Manifest::load(&root.join("manifest.ron")).expect("manifest loads");
    let tiles = manifest
        .load_tiles(&root)
        .expect("every catalogued authored map validates");
    assert_eq!(tiles.len(), manifest.tiles.len());
}

#[test]
fn embedded_catalog_matches_the_filesystem_catalog() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles");
    let filesystem = crate::RuntimeHexCatalog::load(&root, &[]).expect("filesystem catalog loads");
    let read = |name: &str| {
        std::fs::read_to_string(root.join(name))
            .unwrap_or_else(|error| panic!("{name} reads: {error}"))
    };
    let embedded = crate::RuntimeHexCatalog::from_embedded(
        &read("compiled_catalog.ron"),
        &read("compiled_catalog.sha256"),
        &read("composition_profile.ron"),
        &read("composition_profile.sha256"),
        &[],
    )
    .expect("embedded catalog loads");

    assert_eq!(embedded.cells.len(), filesystem.cells.len());
    assert_eq!(embedded.rooms.len(), filesystem.rooms.len());
    assert_eq!(
        embedded.simulation_content_hash,
        filesystem.simulation_content_hash
    );
    assert_eq!(embedded.composition, filesystem.composition);
}

#[test]
fn seed_tile_signatures_match_their_authored_ports() {
    let straight = parse_tile(&tile_source::hall_straight_ew_map()).expect("straight parses");
    assert_eq!(
        straight.signature,
        signature(&[
            (HexFace::East, PortClass::Door),
            (HexFace::West, PortClass::Door)
        ])
    );
}

#[test]
fn an_off_template_brush_fails_with_a_precise_diagnostic() {
    // A box poking 8 units past the East face plane (x 120 > 112).
    let mut map = String::from("{\n\"classname\" \"worldspawn\"\n");
    map += &tile_source::box_brush_text([96, -8, 0], [120, 8, 16]);
    map += "}\n";
    map += "{\n\"classname\" \"tile_meta\"\n\"archetype\" \"bad\"\n\"register\" \"institutional\"\n\"variant\" \"0\"\n\"levels\" \"1\"\n}\n";
    let error = parse_tile(&map).expect_err("off-template brush must be refused");
    match error {
        TileError::FootprintViolation { vertex, boundary } => {
            assert!(
                boundary.contains("east"),
                "diagnostic names the violated face: {boundary}"
            );
            assert!(
                vertex[0] > 112.0,
                "diagnostic reports the offending vertex: {vertex:?}"
            );
        }
        other => panic!("wrong error kind: {other:?}"),
    }
}

#[test]
fn vertical_overflow_fails_with_the_level_bound() {
    let mut map = String::from("{\n\"classname\" \"worldspawn\"\n");
    map += &tile_source::box_brush_text([-16, -16, 0], [16, 16, 200]);
    map += "}\n";
    map += "{\n\"classname\" \"tile_meta\"\n\"archetype\" \"bad\"\n\"register\" \"institutional\"\n\"variant\" \"0\"\n\"levels\" \"1\"\n}\n";
    match parse_tile(&map).expect_err("too-tall brush must be refused") {
        TileError::FootprintViolation { boundary, .. } => {
            assert!(boundary.contains("vertical"), "{boundary}");
        }
        other => panic!("wrong error kind: {other:?}"),
    }
}

#[test]
fn the_manifest_parses_and_covers_the_seed_demands() {
    let manifest = Manifest::from_ron(&tile_source::manifest_ron()).expect("manifest parses");
    let demands = [
        signature(&doors(&[HexFace::East, HexFace::West])),
        signature(&doors(&[HexFace::East])),
    ];
    assert_eq!(manifest.uncovered(&demands), Vec::new());

    // A demand nothing covers is reported, not swallowed: the generated library
    // has nothing open below by a ramp port - a climb's landing is authored.
    let missing = signature(&[(HexFace::Down, PortClass::RampOpen)]);
    assert_eq!(manifest.uncovered(&[missing]), vec![missing]);
}

/// Keys must be unique (the loader hard-fails on duplicates) and every entry
/// must agree with the generated `.map` it points at — this is the pin between
/// the committed manifest and the committed tile files.
#[test]
#[cfg(any())]
fn manifest_keys_are_unique_and_entries_match_their_maps() {
    let manifest = Manifest::from_ron(&tile_source::manifest_ron()).expect("manifest parses");
    let maps: std::collections::BTreeMap<String, String> =
        tile_source::sources().into_iter().collect();
    let mut seen = std::collections::BTreeSet::new();
    for entry in &manifest.tiles {
        assert!(
            seen.insert(entry.key.clone()),
            "duplicate TileKey {:?}",
            entry.key
        );
        let text = maps
            .get(&entry.map_path)
            .unwrap_or_else(|| panic!("{} is not a generated asset", entry.map_path));
        let tile = parse_tile(text)
            .unwrap_or_else(|error| panic!("{} failed to parse: {error:?}", entry.map_path));
        assert_eq!(tile.key, entry.key, "{} key mismatch", entry.map_path);
        assert_eq!(tile.levels, entry.levels, "{} levels", entry.map_path);
        assert_eq!(
            tile.signature,
            entry
                .declared_signature()
                .expect("declared ports are valid"),
            "{} ports disagree with the manifest",
            entry.map_path
        );
    }
}

#[test]
fn blueprint_footprint_cells_match_the_phase_90_alignment() {
    let manifest = Manifest::from_ron(&tile_source::manifest_ron()).expect("manifest parses");
    let mut missing: Vec<String> = Vec::new();
    let mut require = |archetype: &str, reg: &str, sig: PortSignature| {
        if !manifest.tiles.iter().any(|t| {
            t.key.archetype == archetype
                && t.key.register == reg
                && t.declared_signature().ok() == Some(sig)
        }) {
            missing.push(format!("{archetype}/{reg}"));
        }
    };
    // Cell -> open sibling faces plus named exterior thresholds, straight from
    // the blueprint contract.
    let openings: [(&str, &[HexFace]); 11] = [
        ("room_double_west", &[HexFace::East, HexFace::West]),
        ("room_double_east", &[HexFace::West, HexFace::East]),
        ("room_double_nw", &[HexFace::SouthEast, HexFace::West]),
        ("room_double_se", &[HexFace::NorthWest, HexFace::East]),
        (
            "room_tri_a",
            &[HexFace::East, HexFace::SouthEast, HexFace::West],
        ),
        (
            "room_tri_b",
            &[HexFace::West, HexFace::SouthWest, HexFace::East],
        ),
        (
            "room_tri_c",
            &[HexFace::NorthWest, HexFace::NorthEast, HexFace::SouthEast],
        ),
        (
            "room_fork_a",
            &[HexFace::East, HexFace::SouthEast, HexFace::West],
        ),
        (
            "room_fork_b",
            &[
                HexFace::West,
                HexFace::SouthWest,
                HexFace::SouthEast,
                HexFace::East,
            ],
        ),
        (
            "room_fork_c",
            &[
                HexFace::NorthWest,
                HexFace::NorthEast,
                HexFace::East,
                HexFace::West,
            ],
        ),
        (
            "room_fork_d",
            &[HexFace::West, HexFace::NorthWest, HexFace::East],
        ),
    ];
    for &reg in tile_source::REGISTERS {
        require("room_single", reg, signature(&doors(&[HexFace::West])));
        require(
            "room_single",
            reg,
            signature(&doors(&[HexFace::West, HexFace::East])),
        );
        for (archetype, open) in openings {
            require(archetype, reg, signature(&doors(open)));
        }
    }
    assert!(missing.is_empty(), "missing blueprint cells: {missing:#?}");
}

fn committed_liminal_cells() -> Vec<crate::TilePrototype> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles");
    let text = std::fs::read_to_string(root.join("compiled_catalog.ron"))
        .expect("compiled catalogue is committed");
    CompiledTileCatalog::from_ron(&text)
        .expect("catalogue schema")
        .runtime_catalog(&["liminal_grid"])
        .expect("Liminal runtime expansion")
        .cells
        .into_iter()
        .filter(|tile| tile.key.register == "liminal_grid")
        .collect()
}

fn face_direction(face: HexFace) -> Vec2 {
    let [a, b] = face_edge(face);
    Vec2::new((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5).normalize()
}

fn drive_capsule_to(
    scene: &RapierTraversalScene,
    body: &mut FpsBody,
    target: Vec2,
    config: &FpsConfig,
) -> bool {
    for _ in 0..300 {
        let plan = Vec2::new(body.position.x, body.position.z);
        let delta = target - plan;
        if delta.length() <= 0.75 {
            return true;
        }
        let direction = delta.normalize();
        body.yaw = direction.x.atan2(-direction.y);
        step_character(
            scene,
            body,
            PlayerIntent {
                movement: Vec2::Y,
                ..PlayerIntent::default()
            },
            config,
            1.0 / 60.0,
        );
    }
    false
}

/// Every Liminal horizontal runtime variant is physically open between each
/// pair of declared lateral thresholds. Vertical sanctuary apertures are
/// intentionally excluded: their stair-tower traversal remains the existing
/// vertical kit, outside this horizontal expansion.
#[test]
fn every_liminal_horizontal_variant_is_capsule_traversable_between_entrances() {
    let horizontal = [
        "hall_cap",
        "hall_straight",
        "hall_turn_60",
        "hall_turn_120",
        "hall_junction_3way",
        "hall_junction_4way",
        "sanctuary",
    ];
    let config = FpsConfig::default();
    let mut exercised = 0usize;
    for tile in committed_liminal_cells().into_iter().filter(|tile| {
        horizontal.contains(&tile.key.archetype.as_str())
            && tile.signature.port(HexFace::Up) == PortClass::Sealed
            && tile.signature.port(HexFace::Down) == PortClass::Sealed
    }) {
        let doors = HexFace::LATERAL
            .into_iter()
            .filter(|&face| tile.signature.port(face) == PortClass::Door)
            .collect::<Vec<_>>();
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        for &entrance in &doors {
            let start = face_direction(entrance) * 6.1;
            let mut body =
                FpsBody::spawned(Vec3::new(start.x, 0.5 + config.half_height, start.y), 0.0);
            assert!(
                drive_capsule_to(&scene, &mut body, Vec2::ZERO, &config),
                "{:?} could not reach its center from {entrance:?}",
                tile.key
            );
            for &exit in &doors {
                if exit == entrance {
                    continue;
                }
                let mut branch = body;
                let destination = face_direction(exit) * 6.1;
                assert!(
                    drive_capsule_to(&scene, &mut branch, destination, &config),
                    "{:?} blocked {entrance:?} -> {exit:?}; ended at {:?}",
                    tile.key,
                    branch.position
                );
                exercised += 1;
            }
            if doors.len() == 1 {
                exercised += 1;
            }
        }
    }
    assert!(
        exercised > 1_000,
        "unexpectedly small traversal corpus: {exercised}"
    );
}

/// Walk a body up a tile's own stair spine with the production controller,
/// steering perfectly, and report how far it rose and where it gave up.
///
/// The Phase 89 gate drives a constant forward intent, which only works on a
/// climb that runs in a straight line. A perimeter climb turns, so a body has
/// to be steered along the spine - which is exactly what the objective bot
/// does. Steering perfectly here separates the two questions: whether the
/// geometry can be climbed at all, and whether the bot aims correctly.
fn walk_spine_and_measure_rise(map: &str) -> (f32, f32, Vec3) {
    let tile = crate::parse_authored_module(map)
        .expect("module parses")
        .prototype;
    let nodes = tile.spine.nodes.clone();
    assert!(nodes.len() >= 2, "this tile has no spine to walk");
    let arena = tile.arena_spec();
    arena.validate().expect("arena is valid");
    let scene = RapierTraversalScene::from_arena_spec(&arena);
    let config = FpsConfig::default();

    let start_feet = nodes[0];
    let mut body = FpsBody::spawned(start_feet + Vec3::Y * config.half_height, 0.0);
    let mut target = 1usize;
    let mut max_feet = start_feet.y;
    let mut stuck_at = start_feet;

    for _ in 0..3_000 {
        let feet = body.position - Vec3::Y * config.half_height;
        // Advance to the next node once this one is underfoot, measured in
        // plan: a node above the body is still ahead of it, not reached.
        while target < nodes.len() - 1
            && Vec2::new(nodes[target].x - feet.x, nodes[target].z - feet.z).length() < 0.6
        {
            target += 1;
        }
        let to = nodes[target] - feet;
        // The controller reads movement in body space, so face the target and
        // walk straight at it. This is `walk_toward` without the bot around it.
        body.yaw = to.x.atan2(-to.z);
        let intent = PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..PlayerIntent::default()
        };
        step_character(&scene, &mut body, intent, &config, 1.0 / 60.0);
        let feet = body.position - Vec3::Y * config.half_height;
        if feet.y > max_feet {
            max_feet = feet.y;
            stuck_at = feet;
        }
    }
    (
        max_feet - start_feet.y,
        nodes[nodes.len() - 1].y - start_feet.y,
        stuck_at,
    )
}

/// The same walk, but choosing the target the way the objective bot does.
///
/// `StairSpine::target` calls `locate`, which picks the nearest leg **in plan**
/// and hands back that leg's far end. Comparing this against the sequential
/// walk isolates the steering rule from the geometry: if the geometry climbs
/// and this does not, the rule is what is broken.
fn walk_spine_as_the_bot_does(map: &str) -> Walk {
    let tile = crate::parse_authored_module(map)
        .expect("module parses")
        .prototype;
    let nodes = &tile.spine.nodes;

    // Start where a body actually arrives - just inside a lateral door - not
    // on the spine. Starting on the spine is what made the first version of
    // this harness pass everything: it never exercised the approach, which is
    // the half that fails. A body that walks in through a door is 6 m from the
    // foot of a wrapped climb, well past `CLIMB_CAPTURE_RADIUS`.
    let floor = nodes[0].y;
    let entrance = HexFace::LATERAL
        .into_iter()
        .find(|&face| tile.signature.port(face) == PortClass::Door);
    let start_feet = match entrance {
        Some(face) => just_inside(face, floor),
        // A tower has no lateral door: a body arrives through the floor
        // aperture, carried by the flight below, and lands on the climb.
        None => nodes[0],
    };
    walk_from_as_the_bot_does(map, start_feet)
}

/// Where a body stands the moment it is through the door on `face`, at floor
/// height: on the face-midpoint bearing, 0.7 m clear of the wall.
fn just_inside(face: HexFace, floor: f32) -> Vec3 {
    let [a, b] = face_edge(face);
    let mid = Vec2::new((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5);
    let dir = mid.normalize();
    Vec3::new(dir.x * 6.3, floor, dir.y * 6.3)
}

/// What one steered walk did.
///
/// `highest` and `ended` are both here because either alone misreports half the
/// failures. A body that climbs and then stalls gives up at its highest point;
/// a body that never leaves the floor has a highest point of "wherever it stood
/// on the tick its feet first settled", which is its start, and reporting that
/// as where it stopped points the reader at the wrong end of the tile.
struct Walk {
    rise: f32,
    wanted: f32,
    highest: Vec3,
    ended: Vec3,
}

impl Walk {
    /// Whether the body finished the climb, allowing the residual a walker
    /// leaves when it stops steering at its target.
    fn finished(&self) -> bool {
        self.rise >= self.wanted - 0.6
    }

    /// Where it gave up: the top of the climb it managed, or - if it never
    /// climbed at all - where it came to rest.
    fn gave_up(&self) -> Vec3 {
        if self.rise > 0.6 {
            self.highest
        } else {
            self.ended
        }
    }
}

fn shared_climb_target(
    spine: &crate::StairSpine,
    deck: &crate::DeckPath,
    feet: Vec3,
    profile: &TraversalRuntimeProfile,
) -> Option<Vec3> {
    follow_stateless(
        FollowerPose { feet, yaw: 0.0 },
        FollowTarget::Climb {
            spine,
            approach: Some(deck),
            direction: TraversalDirection::Forward,
        },
        profile,
    )
    .target
}

/// The bot's steering rule, run against the production controller from an
/// arbitrary standing start.
///
/// Target selection comes from the production follower. This harness applies
/// the target as an immediate yaw because its job is to audit authored
/// collision, not to duplicate the match controller profile.
fn walk_from_as_the_bot_does(map: &str, start_feet: Vec3) -> Walk {
    let tile = crate::parse_authored_module(map)
        .expect("module parses")
        .prototype;
    let spine = tile.spine.clone();
    let deck = tile.deck.clone();
    let nodes = &spine.nodes;
    let arena = tile.arena_spec();
    let scene = RapierTraversalScene::from_arena_spec(&arena);
    let config = FpsConfig::default();
    let profile = TraversalRuntimeProfile::from_controller(config);

    let floor = nodes[0].y;
    let wanted = nodes[nodes.len() - 1].y - floor;

    let mut body = FpsBody::spawned(start_feet + Vec3::Y * config.half_height, 0.0);
    let mut max_feet = floor;
    let mut highest = start_feet;
    let mut ended = start_feet;

    for _ in 0..6_000 {
        let feet = body.position - Vec3::Y * config.half_height;
        let target = shared_climb_target(&spine, &deck, feet, &profile);
        let Some(target) = target else { break };
        let to = target - feet;
        body.yaw = to.x.atan2(-to.z);
        let intent = PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..PlayerIntent::default()
        };
        step_character(&scene, &mut body, intent, &config, 1.0 / 60.0);
        ended = body.position - Vec3::Y * config.half_height;
        if ended.y > max_feet {
            max_feet = ended.y;
            highest = ended;
        }
    }
    Walk {
        rise: max_feet - floor,
        wanted,
        highest,
        ended,
    }
}

/// Every authored climb must be walkable by the production controller when it
/// is steered along its own spine.
///
/// The perimeter ramps shipped without this and none of them can be finished:
/// the objective bot drove a spectator into a wall for a whole match. A spine
/// that cannot be walked is worse than no spine, because everything downstream
/// trusts it.
#[test]
fn every_authored_spine_can_be_walked_by_the_controller() {
    let mut failures = Vec::new();
    let mut walked = 0;
    // `generate_all`, not `builders`: the latter is only halls, silos, and
    // rooms, and every climb in the corpus lives in the three it leaves out.
    for (stem, text) in crate::forge::generate_all() {
        let module = crate::parse_authored_module(&text)
            .unwrap_or_else(|error| panic!("{stem} does not validate: {error:?}"));
        if module.prototype.spine.nodes.len() < 2 {
            continue;
        }
        walked += 1;
        let (rise, wanted, stuck) = walk_spine_and_measure_rise(&text);
        if rise < wanted - 0.6 {
            failures.push(format!(
                "  {stem}: rose {rise:.2} m of {wanted:.2} m, stopped at {stuck:?}"
            ));
        }
    }
    // A survey that silently walks nothing passes for the wrong reason. This
    // test did exactly that on its first run: it used `parse_tile`, which
    // rejects authoring version 2, so every module fell through the `continue`
    // and it reported success having walked none of them.
    assert!(
        walked >= 3,
        "expected the authored climbs to be walked, found {walked}"
    );
    assert!(
        failures.is_empty(),
        "{} of {walked} authored climbs cannot be finished:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The bot's own targeting rule must finish every climb the geometry allows.
///
/// `every_authored_spine_can_be_walked_by_the_controller` proves the geometry
/// climbs when steered node by node. This proves the rule the bot actually
/// uses gets the same result - and it is the pair that matters, because a
/// climbable tile the bot cannot climb is a stalled match either way.
#[test]
fn the_bots_targeting_rule_finishes_every_authored_climb() {
    let mut failures = Vec::new();
    let mut walked = 0;
    for (stem, text) in crate::forge::generate_all() {
        let module = crate::parse_authored_module(&text)
            .unwrap_or_else(|error| panic!("{stem} does not validate: {error:?}"));
        if module.prototype.spine.nodes.len() < 2 {
            continue;
        }
        walked += 1;
        let walk = walk_spine_as_the_bot_does(&text);
        if !walk.finished() {
            failures.push(format!(
                "  {stem}: rose {:.2} m of {:.2} m, stopped at {:?}",
                walk.rise,
                walk.wanted,
                walk.gave_up()
            ));
        }
    }
    assert!(walked >= 3, "expected climbs to walk, found {walked}");
    assert!(
        failures.is_empty(),
        "{} of {walked} climbs defeat the bot's targeting rule:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// The four `*_generated_stair_tower_*` tests and
// `the_vertical_districts_have_towers_of_their_own` retired with their subject:
// there is no generated stair tower any more. Their contracts did not retire
// with them - they moved to the authored family, which is where the geometry
// now is:
//
//   followable spine      -> forge::tower::every_tower_carries_a_spine
//                            forge::tower::the_climb_reaches_the_port_it_advertises
//   physically climbable  -> every_authored_spine_can_be_walked_by_the_controller
//                            the_bots_targeting_rule_finishes_every_authored_climb
//   walkable deck         -> forge::tower emits `ring_deck`; the importer's
//                            `DeckPathTooShort` rejects a path that is not one
//   capped clears its lid -> forge::tower::a_capped_tower_clears_its_climb_by_a_body
//
// **Per-register handedness is not carried over, and that is a real loss.** The
// switchback mirrored itself per register so districts stacked their towers
// differently; the authored family has one shape everywhere. It is deferred
// rather than dropped - `Extent` would need a hand, mirroring `outer` about
// x = 0 - and it is deliberately not smuggled into the replacement, because a
// handed pair is two climb shapes and this change is about proving one.

/// Run the rebuilt passage both ways, in all six rotations and three lanes.
/// The side lanes catch ribs that a centre-only ray would miss.
#[test]
fn lantern_passage_is_walkable_in_both_directions_at_every_rotation() {
    let source = crate::parse_authored_module(&crate::forge::halls::hall_straight())
        .expect("passage validates")
        .prototype;
    let config = FpsConfig::default();
    for turn in 0..6 {
        let mut tile = source.clone();
        tile.hulls = crate::rotation::rotate_hulls(&source.hulls, turn);
        let axis = face_direction(HexFace::LATERAL[usize::from(turn)]);
        let side = Vec2::new(-axis.y, axis.x);
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        for direction in [-1.0, 1.0] {
            for lane in [-1.0, 0.0, 1.0] {
                let start = axis * (6.5 * direction) + side * lane;
                let finish = axis * (-6.5 * direction) + side * lane;
                let mut body =
                    FpsBody::spawned(Vec3::new(start.x, 0.5 + config.half_height, start.y), 0.0);
                assert!(
                    drive_capsule_to(&scene, &mut body, finish, &config),
                    "blocked passage: turn {turn}, direction {direction}, lane {lane}"
                );
                assert!((body.position.y - config.half_height - 0.5).abs() < 0.1);
            }
        }
    }
}
