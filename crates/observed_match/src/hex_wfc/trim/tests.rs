use observed_authoring::TilePrototype;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{
    HexArchetype, HexPlacement, HexSpace, HexWfcConfig, HexWfcWorld, lateral_bit,
};
use observed_hex::PortClass;

use super::*;

fn tiles() -> Vec<TilePrototype> {
    observed_authoring::tile_source::compatibility_cells().expect("compatibility tiles")
}

/// A hand-built two-cell world: `A` at `(5, 5, 0)` is a plain E/W hall, `B`
/// immediately east of it is a ramp cell (different [`HexStructureRole`]).
/// No solver run, no other occupied cells — every other lateral neighbor of
/// `A` and `B` is void by omission. This gives a fully known expected trim
/// set:
/// - a single `Buttress` on `A`'s `East` face (the only occupied neighbor,
///   with a differing role: Hall vs. Ramp),
/// - a `Railing` on every other lateral face of both `A` and `B` (five each,
///   all bordering nothing).
///
/// Both cells are put on different [`ArchitectureRegister`]s too, but that
/// alone would not be visible to `derive_trim`: [`HexStructurePiece::tile`]
/// carries the *selected geometry-catalogue* register (here always
/// `"generic"`, since [`tiles`] only supplies the register-agnostic
/// compatibility kit), not `HexWfcWorld::architecture` directly. The role
/// difference is what a compatibility-only catalogue can actually exercise.
fn two_cell_world() -> HexWfcWorld {
    let a = observed_hex::HexCoord {
        q: 5,
        r: 5,
        level: 0,
    };
    let b = observed_hex::HexCoord {
        q: 6,
        r: 5,
        level: 0,
    };
    let mut placements = BTreeMap::new();
    placements.insert(
        a,
        HexPlacement {
            coord: a,
            space: HexSpace::Hall,
            archetype: HexArchetype::Straight,
            doors: lateral_bit(HexFace::East) | lateral_bit(HexFace::West),
            up: PortClass::Sealed,
            down: PortClass::Sealed,
        },
    );
    placements.insert(
        b,
        HexPlacement {
            coord: b,
            space: HexSpace::Hall,
            archetype: HexArchetype::RampUp,
            doors: lateral_bit(HexFace::West),
            up: PortClass::RampOpen,
            down: PortClass::Sealed,
        },
    );
    let mut architecture = BTreeMap::new();
    architecture.insert(a, ArchitectureRegister::Institutional);
    architecture.insert(b, ArchitectureRegister::Megastructure);
    HexWfcWorld {
        seed: 1,
        generation: 0,
        config: HexWfcConfig {
            cols: 10,
            rows: 10,
            levels: 1,
            min_rooms: 0,
            max_rooms: 0,
            retry_budget: 1,
            min_room_distance: 1,
        },
        placements,
        blueprints: Vec::new(),
        architecture,
        cell_revisions: BTreeMap::new(),
        last_attempts: 1,
    }
}

#[test]
fn two_cell_world_yields_one_buttress_and_ten_railings_at_the_expected_faces() {
    let world = two_cell_world();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("tiny projection");
    let trim = derive_trim(&snapshot);

    let a = observed_hex::HexCoord {
        q: 5,
        r: 5,
        level: 0,
    };
    let b = observed_hex::HexCoord {
        q: 6,
        r: 5,
        level: 0,
    };

    let buttresses: Vec<_> = trim
        .iter()
        .filter(|piece| piece.kind == HexTrimKind::Buttress)
        .collect();
    assert_eq!(buttresses.len(), 1, "exactly one shared occupied seam");
    assert_eq!(buttresses[0].cell, a, "owned by the lower-ordered cell");
    assert_eq!(buttresses[0].face, HexFace::East);

    let railings: Vec<_> = trim
        .iter()
        .filter(|piece| piece.kind == HexTrimKind::Railing)
        .collect();
    assert_eq!(railings.len(), 10, "five open ledges on each of A and B");
    for face in HexFace::LATERAL {
        if face == HexFace::East {
            assert!(
                !railings
                    .iter()
                    .any(|piece| piece.cell == a && piece.face == face),
                "A's East face borders B, not void"
            );
        } else {
            assert!(
                railings
                    .iter()
                    .any(|piece| piece.cell == a && piece.face == face),
                "missing railing on A's {face:?} face"
            );
        }
    }
    for face in HexFace::LATERAL {
        if face == HexFace::West {
            assert!(
                !railings
                    .iter()
                    .any(|piece| piece.cell == b && piece.face == face),
                "B's West face borders A, not void"
            );
        } else {
            assert!(
                railings
                    .iter()
                    .any(|piece| piece.cell == b && piece.face == face),
                "missing railing on B's {face:?} face"
            );
        }
    }

    // No lintel rule is implemented yet (see module docs): the derivation
    // must never fabricate one from role/register adjacency alone.
    assert!(
        trim.iter().all(|piece| piece.kind != HexTrimKind::Lintel),
        "lintel derivation is out of scope until the snapshot carries port classes"
    );
}

#[test]
fn railing_sits_at_the_face_edge_midpoint_one_meter_above_the_floor() {
    let world = two_cell_world();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &tiles()).expect("tiny projection");
    let trim = derive_trim(&snapshot);
    let a = observed_hex::HexCoord {
        q: 5,
        r: 5,
        level: 0,
    };
    let west = trim
        .iter()
        .find(|piece| {
            piece.kind == HexTrimKind::Railing && piece.cell == a && piece.face == HexFace::West
        })
        .expect("A has a west railing");

    let origin = Vec3::from_array(hex_origin(a));
    let [edge_a, edge_b] = face_edge(HexFace::West);
    let expected_xz = Vec2::new(
        (edge_a.0 + edge_b.0) as f32 * 0.5,
        (edge_a.1 + edge_b.1) as f32 * 0.5,
    );
    assert_eq!(west.position.x, origin.x + expected_xz.x);
    assert_eq!(west.position.z, origin.z + expected_xz.y);
    assert_eq!(west.position.y, origin.y + RAILING_HEIGHT);
}

fn showcase_config(levels: u8) -> HexWfcConfig {
    HexWfcConfig {
        cols: 12,
        rows: 9,
        levels,
        min_rooms: 4,
        max_rooms: 8,
        retry_budget: 100,
        min_room_distance: 2,
    }
}

fn showcase() -> HexWfcWorld {
    HexWfcWorld::generate(0xA11C_E3D0_0000_0008, showcase_config(4)).expect("showcase world")
}

#[test]
fn derive_trim_is_pure_and_deterministic_over_a_solved_facility() {
    let world = showcase();
    let prototypes = tiles();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("projection");
    let first = derive_trim(&snapshot);
    let second = derive_trim(&snapshot);
    assert_eq!(first, second);
    assert!(!first.is_empty(), "a bounded facility has open ledges");
    assert!(
        first.iter().any(|piece| piece.kind == HexTrimKind::Railing),
        "a bounded facility has at least one open-ledge railing"
    );
    assert!(
        first.iter().all(|piece| piece.kind != HexTrimKind::Lintel),
        "lintel derivation is out of scope until the snapshot carries port classes"
    );
}

#[test]
fn buttress_pieces_mark_a_real_role_or_register_seam_between_two_occupied_cells() {
    let world = showcase();
    let prototypes = tiles();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("projection");
    let cells = summarize_cells(&snapshot.pieces);
    for piece in derive_trim(&snapshot)
        .into_iter()
        .filter(|piece| piece.kind == HexTrimKind::Buttress)
    {
        let neighbor_coord = step(piece.cell, piece.face).expect("buttress faces a real neighbor");
        let here = cells.get(&piece.cell).expect("buttress owner is occupied");
        let neighbor = cells
            .get(&neighbor_coord)
            .expect("buttress neighbor is occupied");
        assert!(
            piece.cell < neighbor_coord,
            "owned by the lower-ordered cell"
        );
        assert!(
            here.role != neighbor.role || here.register != neighbor.register,
            "buttress must mark an actual role or register difference"
        );
    }
}

#[test]
fn railing_pieces_mark_a_real_open_ledge_on_a_walkable_cell() {
    let world = showcase();
    let prototypes = tiles();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("projection");
    let cells = summarize_cells(&snapshot.pieces);
    for piece in derive_trim(&snapshot)
        .into_iter()
        .filter(|piece| piece.kind == HexTrimKind::Railing)
    {
        assert!(cells.contains_key(&piece.cell), "railing owner is occupied");
        let neighbor_absent = step(piece.cell, piece.face)
            .map(|neighbor| !cells.contains_key(&neighbor))
            .unwrap_or(true);
        assert!(neighbor_absent, "railing must border an unoccupied cell");
    }
}
