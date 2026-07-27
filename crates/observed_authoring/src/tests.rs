use glam::{Vec2, Vec3};
use observed_hex::{
    FLOOR_SLAB_TOP, HexFace, PortClass, PortSignature, TILE_LEVEL_HEIGHT, face_edge,
};
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
use observed_traversal::{FpsBody, FpsConfig};
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
fn seed_tile_signatures_match_their_authored_ports() {
    let straight = parse_tile(&tile_source::hall_straight_ew_map()).expect("straight parses");
    assert_eq!(
        straight.signature,
        signature(&[
            (HexFace::East, PortClass::Door),
            (HexFace::West, PortClass::Door)
        ])
    );
    let ramp = parse_tile(&tile_source::ramp_e_map()).expect("ramp parses");
    assert_eq!(ramp.levels, 2);
    assert_eq!(
        ramp.signature,
        signature(&[
            (HexFace::West, PortClass::Door),
            (HexFace::Up, PortClass::RampOpen)
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
        signature(&[
            (HexFace::West, PortClass::Door),
            (HexFace::Up, PortClass::RampOpen),
        ]),
    ];
    assert_eq!(manifest.uncovered(&demands), Vec::new());

    // A demand nothing covers is reported, not swallowed: no tile is a ramp
    // head (`down: ramp_open`) — the two-level ramp prefab bakes its head in.
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
    // Cell -> internally-sealed faces, straight from the alignment note.
    let sealed: [(&str, &[HexFace]); 11] = [
        ("room_double_west", &[HexFace::East]),
        ("room_double_east", &[HexFace::West]),
        ("room_double_nw", &[HexFace::SouthEast]),
        ("room_double_se", &[HexFace::NorthWest]),
        ("room_tri_a", &[HexFace::East, HexFace::SouthEast]),
        ("room_tri_b", &[HexFace::West, HexFace::SouthWest]),
        ("room_tri_c", &[HexFace::NorthWest, HexFace::NorthEast]),
        ("room_fork_a", &[HexFace::East, HexFace::SouthEast]),
        (
            "room_fork_b",
            &[HexFace::West, HexFace::SouthWest, HexFace::SouthEast],
        ),
        (
            "room_fork_c",
            &[HexFace::NorthWest, HexFace::NorthEast, HexFace::East],
        ),
        ("room_fork_d", &[HexFace::West, HexFace::NorthWest]),
    ];
    for &reg in tile_source::REGISTERS {
        require("room_single", reg, signature(&doors(&HexFace::LATERAL)));
        for (archetype, internal) in sealed {
            let exterior: Vec<HexFace> = HexFace::LATERAL
                .into_iter()
                .filter(|face| !internal.contains(face))
                .collect();
            require(archetype, reg, signature(&doors(&exterior)));
        }
    }
    assert!(missing.is_empty(), "missing blueprint cells: {missing:#?}");
}

fn walk_ramp_and_measure_rise(map: &str, entrance: HexFace) -> (f32, bool) {
    let ramp = parse_tile(map).expect("ramp parses");
    let arena = ramp.arena_spec();
    arena.validate().expect("ramp arena is valid");
    let scene = RapierTraversalScene::from_arena_spec(&arena);
    let config = FpsConfig::default();
    // Feet just inside the entrance door, facing the exit across the cell.
    let [a, b] = face_edge(entrance);
    let mid = Vec2::new((a.0 + b.0) as f32 * 0.5, (a.1 + b.1) as f32 * 0.5);
    let dir = mid.normalize();
    let start_feet = Vec3::new(dir.x * 6.3, 0.95, dir.y * 6.3);
    let facing = -dir;
    let yaw = facing.x.atan2(-facing.y);
    let mut body = FpsBody::spawned(start_feet + Vec3::Y * config.half_height, yaw);
    let intent = PlayerIntent {
        movement: Vec2::new(0.0, 1.0),
        ..PlayerIntent::default()
    };
    let mut max_feet = f32::MIN;
    let mut jumped = false;
    for _ in 0..600 {
        let report = step_character(&scene, &mut body, intent, &config, 1.0 / 60.0);
        jumped |= report.jumped;
        max_feet = max_feet.max(body.position.y - config.half_height);
    }
    (max_feet - start_feet.y, jumped)
}

/// THE PHASE 89 GATE: the shared production controller walks up the ramp
/// prefab and gains a full level without jumping. If this fails, the taller
/// tile / walkable ramp assumption of the whole arc is invalid.
#[test]
fn the_shared_controller_walks_the_ramp_up_a_full_level() {
    let (rise, jumped) = walk_ramp_and_measure_rise(&tile_source::ramp_e_map(), HexFace::West);
    assert!(!jumped, "ascent must be plain walking");
    assert!(
        rise >= TILE_LEVEL_HEIGHT - 0.6,
        "controller only climbed {rise:.2} m of the {TILE_LEVEL_HEIGHT} m level; \
         the walkable-ramp assumption is broken"
    );
}

/// Phase 91: every authored direction is walkable with the shared production
/// controller. This is deliberately a six-direction corpus rather than an
/// assumption that rotated brush geometry remains equivalent.
#[test]
fn the_shared_controller_walks_all_six_ramp_directions() {
    for exit in HexFace::LATERAL {
        let map = tile_source::ramp_map("megastructure", exit);
        let (rise, jumped) = walk_ramp_and_measure_rise(&map, exit.opposite());
        assert!(!jumped, "{exit:?} ascent must be plain walking");
        assert!(
            rise >= TILE_LEVEL_HEIGHT - 0.6,
            "controller only climbed {rise:.2} m on the {exit:?} ramp"
        );
    }
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

/// The switchback stair is what every `Shaft` cell renders (`stair_tower`), so
/// it is the most-walked vertical element in the facility - and it had no
/// geometric coverage at all. These are the contracts whose violation a player
/// sees as "the pieces don't smoothly connect".
#[test]
fn the_switchback_stair_lands_flush_on_the_deck_above() {
    let tile = parse_tile(&tile_source::stair_segment_map("megastructure")).expect("stair parses");
    let climb_top = tile
        .hulls
        .iter()
        .flatten()
        .fold(f32::MIN, |top, point| top.max(point.y));
    // The flight is the tallest thing in the cell; it must finish exactly on the
    // deck of the cell above, never proud of it. It used to top out at 9.00 m -
    // 0.50 m above that deck - which reads as a lip at every level junction and,
    // since autostep only lifts `FpsConfig::step_height` (0.45 m), physically
    // stopped a body stepping from the upper deck back onto the flight.
    let deck_above = TILE_LEVEL_HEIGHT + 0.5;
    assert!(
        (climb_top - deck_above).abs() <= 0.02,
        "switchback tops out at {climb_top:.2} m but the deck above is at {deck_above:.2} m; \
         an overshoot beyond {:.2} m cannot be stepped back onto",
        FpsConfig::default().step_height
    );
}

/// Every span carried by a support must actually rest on it. The pier heights
/// were once hand-tuned constants that drifted out of step with the flight above
/// them and stopped 0.23 m to 0.40 m short, leaving the staircase visibly
/// floating. They are now derived from the flight, and this pins that.
#[test]
fn every_switchback_support_meets_the_span_it_carries() {
    let tile = parse_tile(&tile_source::stair_segment_map("megastructure")).expect("stair parses");
    let bbox = |hull: &Vec<glam::Vec3>| {
        hull.iter()
            .fold(([f32::MAX; 3], [f32::MIN; 3]), |(min, max), p| {
                (
                    [min[0].min(p.x), min[1].min(p.y), min[2].min(p.z)],
                    [max[0].max(p.x), max[1].max(p.y), max[2].max(p.z)],
                )
            })
    };
    // The lower flight: the one broad sloped deck rising off the cell floor.
    let flight = tile
        .hulls
        .iter()
        .find(|hull| {
            let (min, max) = bbox(hull);
            min[1] <= 0.01 && max[0] - min[0] >= 7.0 && max[1] - min[1] >= 3.0
        })
        .expect("lower flight");
    let (fmin, fmax) = bbox(flight);
    // Its underside is planar and varies only along x, so sample the lowest
    // vertex at each end and interpolate. A bounding box will not do: the box's
    // floor is the flight's low end, not its height above any given pier.
    let lowest_near = |x: f32| {
        flight
            .iter()
            .filter(|p| (p.x - x).abs() <= 0.05)
            .fold(f32::MAX, |low, p| low.min(p.y))
    };
    let (west, east) = (lowest_near(fmin[0]), lowest_near(fmax[0]));
    let underside_at = |x: f32| west + (x - fmin[0]) / (fmax[0] - fmin[0]) * (east - west);

    let mut checked = 0;
    for hull in &tile.hulls {
        let (min, max) = bbox(hull);
        let footprint = (max[0] - min[0]) * (max[2] - min[2]);
        // A pier: a slim column standing on the cell floor under the flight.
        if min[1] > 0.01 || footprint > 1.5 || max[1] <= 0.6 || max[1] >= fmax[1] {
            continue;
        }
        let centre = (min[0] + max[0]) * 0.5;
        if centre < fmin[0] || centre > fmax[0] {
            continue;
        }
        let expected = underside_at(centre);
        assert!(
            (max[1] - expected).abs() <= 0.06,
            "pier at x={centre:.2} tops at {:.2} m but the flight's underside there \
             is {expected:.2} m: a {:.2} m discrepancy leaves it visibly unsupported",
            max[1],
            (max[1] - expected).abs()
        );
        checked += 1;
    }
    assert!(checked >= 3, "expected the flight's piers, found {checked}");
}

/// The spine is the contract that lets vertical circulation be more than one
/// shape. Every stair tower has to ship a line a body can walk, whatever its
/// interior looks like, or authoring a second tower produces geometry the
/// objective bot cannot follow — which is precisely what blocked backlog #13
/// for an arc.
#[test]
fn every_generated_stair_tower_ships_a_followable_spine() {
    let cells = tile_source::compatibility_cells().expect("generated kit parses");
    let towers = cells
        .iter()
        .filter(|tile| tile.key.archetype.starts_with("stair_"))
        .collect::<Vec<_>>();
    assert!(!towers.is_empty(), "the generated kit has stair towers");
    for tile in towers {
        let spine = &tile.spine;
        assert!(
            !spine.is_empty(),
            "{} {} ships no climb spine",
            tile.key.archetype,
            tile.key.register
        );
        assert_eq!(
            spine.self_crossing(),
            None,
            "{} {} doubles back within a body's width of itself, so a follower \
             cannot tell which stretch it is on",
            tile.key.archetype,
            tile.key.register
        );

        // Both ends stand on flat deck: the bottom on this cell's floor slab and
        // the top on the deck of the cell above. A spine that starts partway up
        // a flight strands a body that walks in through a lateral door.
        let first = spine.nodes.first().expect("checked non-empty");
        let last = spine.nodes.last().expect("checked non-empty");
        assert!(
            (first.y - FLOOR_SLAB_TOP).abs() <= 0.05,
            "{} {} starts at {:.2} m, off this cell's deck at {FLOOR_SLAB_TOP:.2} m",
            tile.key.archetype,
            tile.key.register,
            first.y
        );
        assert!(
            (last.y - (TILE_LEVEL_HEIGHT + FLOOR_SLAB_TOP)).abs() <= 0.05,
            "{} {} ends at {:.2} m, off the deck above at {:.2} m",
            tile.key.archetype,
            tile.key.register,
            last.y,
            TILE_LEVEL_HEIGHT + FLOOR_SLAB_TOP
        );
    }
}

/// A follower walking the spine must never be sent backwards. This is the
/// property the old hardcoded steering lacked: it chose its target by proximity
/// to a waypoint, which flips as you walk away from one, so the bot span on the
/// spot just past the turn and burnt ~31,000 ticks on a single storey.
#[test]
fn walking_the_spine_never_sends_a_follower_backwards() {
    let tile = parse_tile(&tile_source::stair_segment_map("wellshaft")).expect("stair parses");
    let spine = &tile.spine;
    let mut highest = 0;
    // Sample densely along the spine itself, which is the path a body on the
    // stair actually traces.
    for segment in 0..spine.nodes.len() - 1 {
        for step in 0..=40_u32 {
            let point = spine.nodes[segment].lerp(spine.nodes[segment + 1], step as f32 / 40.0);
            let (index, _) = spine.locate(point).expect("a spine with nodes locates");
            assert!(
                index >= highest,
                "walking segment {segment} sent the follower back from {highest} to {index}"
            );
            highest = index;
        }
    }
    assert_eq!(
        highest,
        spine.nodes.len() - 2,
        "the walk should finish on the last segment"
    );
}

/// The floor path is the other half of the tower contract, and the one that
/// closes bug backlog #19. A tower's deck has a hole in it; without a declared
/// route around that hole a body crossing to a lateral door walks into the
/// stairwell or into a pier, which is exactly how the objective bot wedged.
#[test]
fn every_generated_stair_tower_ships_a_walkable_deck() {
    let cells = tile_source::compatibility_cells().expect("generated kit parses");
    for tile in cells
        .iter()
        .filter(|tile| tile.key.archetype.starts_with("stair_"))
    {
        let deck = &tile.deck;
        assert!(
            !deck.is_empty(),
            "{} {} ships no deck path",
            tile.key.archetype,
            tile.key.register
        );
        for node in &deck.nodes {
            assert!(
                (node.y - FLOOR_SLAB_TOP).abs() <= 0.05,
                "{} {} has a deck node at {:.2} m, off the floor slab at {FLOOR_SLAB_TOP:.2} m",
                tile.key.archetype,
                tile.key.register,
                node.y
            );
        }

        // The climb has to be reachable from the floor, or the tower is a
        // staircase nobody can get to the bottom of.
        let foot = *tile.spine.nodes.first().expect("towers ship a spine");
        let approach = deck
            .nodes
            .iter()
            .map(|node| (node.x - foot.x).hypot(node.z - foot.z))
            .fold(f32::MAX, f32::min);
        assert!(
            approach <= 2.0,
            "{} {}: the foot of the climb is {approach:.2} m from the nearest deck node, so a \
             body crossing the floor has no declared way onto it",
            tile.key.archetype,
            tile.key.register
        );
    }
}
