use glam::{Vec2, Vec3};
use observed_hex::{HexFace, PortClass, PortSignature, TILE_LEVEL_HEIGHT};
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

#[test]
fn every_generated_seed_tile_parses_and_snaps() {
    for (name, content) in tile_source::sources() {
        if name.ends_with(".ron") {
            continue;
        }
        let tile = parse_tile(&content)
            .unwrap_or_else(|error| panic!("{name} failed to parse: {error:?}"));
        assert!(!tile.hulls.is_empty(), "{name} has no geometry");
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
        signature(&[
            (HexFace::East, PortClass::Door),
            (HexFace::West, PortClass::Door),
        ]),
        signature(&[(HexFace::East, PortClass::Door)]),
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

    // A demand nothing covers is reported, not swallowed.
    let missing = signature(&[
        (HexFace::SouthEast, PortClass::Door),
        (HexFace::NorthWest, PortClass::Door),
    ]);
    assert_eq!(manifest.uncovered(&[missing]), vec![missing]);
}

#[test]
fn committed_assets_do_not_drift_from_the_typed_source() {
    for (name, content) in tile_source::sources() {
        let committed = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/tiles")
                .join(name),
        )
        .unwrap_or_else(|error| panic!("committed {name} missing: {error}"));
        assert_eq!(committed, content, "{name} drifted — rerun bake_tiles");
    }
}

/// THE PHASE 89 GATE: the shared production controller walks up the ramp
/// prefab and gains a full level without jumping. If this fails, the taller
/// tile / walkable ramp assumption of the whole arc is invalid.
#[test]
fn the_shared_controller_walks_the_ramp_up_a_full_level() {
    let ramp = parse_tile(&tile_source::ramp_e_map()).expect("ramp parses");
    let arena = ramp.arena_spec();
    arena.validate().expect("ramp arena is valid");
    let scene = RapierTraversalScene::from_arena_spec(&arena);
    let config = FpsConfig::default();

    // Feet on the West slab top (0.5 m), just inside the door, facing East.
    let start_feet = Vec3::new(-5.6, 0.5, 0.0);
    let mut body = FpsBody::spawned(
        start_feet + Vec3::Y * config.half_height,
        std::f32::consts::FRAC_PI_2,
    );
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
    assert!(!jumped, "ascent must be plain walking");
    let rise = max_feet - start_feet.y;
    assert!(
        rise >= TILE_LEVEL_HEIGHT - 0.6,
        "controller only climbed {rise:.2} m of the {TILE_LEVEL_HEIGHT} m level \
         (max feet {max_feet:.2}); the walkable-ramp assumption is broken"
    );
}
