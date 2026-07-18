use glam::{Vec2, Vec3};
use observed_facility::hex_wfc::geometry_demands;
use observed_hex::{HexFace, PortClass, PortSignature, TILE_LEVEL_HEIGHT, face_edge};
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
use observed_traversal::{FpsBody, FpsConfig};
use player_input::PlayerIntent;

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
    }
}

/// The pin: every committed asset is byte-identical to the typed generator's
/// output. If this fails, rerun `cargo run -p observed_authoring --bin
/// bake_tiles`.
#[test]
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
    let shaft = parse_tile(&tile_source::shaft_map()).expect("shaft parses");
    assert_eq!(
        shaft.signature,
        signature(&[
            (HexFace::Up, PortClass::ShaftOpen),
            (HexFace::Down, PortClass::ShaftOpen)
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
        signature(&[
            (HexFace::Up, PortClass::ShaftOpen),
            (HexFace::Down, PortClass::ShaftOpen),
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

/// Every register covers the production solver's exact geometry-emitter
/// contract. Matching includes semantic archetype as well as signature: a hall
/// with coincidentally equal ports cannot stand in for an authored room wing or
/// shaft landing.
#[test]
fn every_register_covers_the_production_geometry_demands() {
    let manifest = Manifest::from_ron(&tile_source::manifest_ron()).expect("manifest parses");
    let demands = geometry_demands();
    assert_eq!(demands.len(), 134, "geometry-demand alphabet drifted");
    let mut missing: Vec<String> = Vec::new();
    for &reg in tile_source::REGISTERS {
        for demand in &demands {
            if !manifest.tiles.iter().any(|tile| {
                tile.key.archetype == demand.archetype
                    && tile.key.register == reg
                    && tile.declared_signature().ok() == Some(demand.signature)
            }) {
                missing.push(format!("{}/{reg}/{:?}", demand.archetype, demand.signature));
            }
        }
        let shaft_landings = manifest
            .tiles
            .iter()
            .filter(|tile| tile.key.archetype == "shaft_landing" && tile.key.register == reg)
            .count();
        assert_eq!(shaft_landings, 63, "{reg} shaft signature coverage");

        // Straight halls still carry all three interior readings per axis even
        // though geometry coverage needs only one tile per exact signature.
        let straights = manifest
            .tiles
            .iter()
            .filter(|t| t.key.archetype == "hall_straight" && t.key.register == reg)
            .count();
        assert_eq!(straights, 9, "{reg} straight interiors");
    }
    assert!(missing.is_empty(), "missing tiles: {missing:#?}");
    assert_eq!(manifest.tiles.len(), 1_332, "library-size drift");
}

/// The room blueprint cells match the Phase 90 alignment note
/// (`docs/arc_l/phase_90_91_alignment.md`): internal faces sealed, every
/// exterior face a door, and the two-level atrium pair joined by
/// `shaft_open` verticals.
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
        let mut lower = doors(&HexFace::LATERAL);
        lower.push((HexFace::Up, PortClass::ShaftOpen));
        require("room_atrium_lower", reg, signature(&lower));
        let mut upper = doors(&HexFace::LATERAL);
        upper.push((HexFace::Down, PortClass::ShaftOpen));
        require("room_atrium_upper", reg, signature(&upper));
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
