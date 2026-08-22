use observed_authoring::TilePrototype;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{
    HexArchetype, HexPlacement, HexSpace, HexWfcConfig, HexWfcWorld, lateral_bit,
};
use observed_hex::PortClass;

use super::*;

fn tiles() -> Vec<TilePrototype> {
    crate::hex_wfc::test_tiles()
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
/// `"generic"`, since these two cells use the register-agnostic compatibility
/// hall kit), not `HexWfcWorld::architecture` directly. The role difference is
/// what this fixture can actually exercise.
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
        // A solved world always carries a revision per resolved cell, and the
        // projection relies on it: a tile that ships a spine or a deck records
        // a `ProjectedTraversalGuide`, which names the exact revision it was
        // built from. This fixture left the map empty and got away with it only
        // while neither of its two tiles carried an annotation. Cell `B` is a
        // ramp, and once the generated ramp gained a spine the omission became
        // a panic — in the projector, not here, which is the wrong place to
        // learn that a hand-built world is incomplete.
        cell_revisions: BTreeMap::from([(a, 1), (b, 1)]),
        last_attempts: 1,
        authored_pins: Default::default(),
        space_mix: observed_facility::hex_wfc::profile::SpaceMix::baseline(),
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
    let cells = summarize_cells(&snapshot.pieces, None);
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
    let cells = summarize_cells(&snapshot.pieces, None);
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

/// The scoped derivation is an optimisation, so it has to be indistinguishable from
/// filtering the full one. If these ever diverge, a relayout commit would respawn subtly
/// different trim than a fresh build of the same facility.
#[test]
fn scoped_trim_matches_the_owned_subset_of_the_full_derivation() {
    let world = showcase();
    let prototypes = tiles();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("projection");
    let full = derive_trim(&snapshot);
    assert!(!full.is_empty(), "fixture must produce trim to compare");

    let owners: Vec<observed_hex::HexCoord> = {
        let mut seen: Vec<observed_hex::HexCoord> = full
            .iter()
            .map(|piece| piece.cell)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        seen.truncate(24);
        seen
    };
    assert!(
        owners.len() > 4,
        "need several owners to be a real comparison"
    );

    // Each owner alone, then a batch: a commit invalidates an arbitrary set, and the
    // halo must be recomputed per call rather than accumulated across calls.
    for owner in &owners {
        let scoped = derive_trim_for(&snapshot, &BTreeSet::from([*owner]));
        let expected: Vec<_> = full.iter().filter(|p| p.cell == *owner).cloned().collect();
        assert_eq!(scoped, expected, "scoped trim diverged for {owner:?}");
    }

    let batch: BTreeSet<observed_hex::HexCoord> = owners.iter().copied().collect();
    let scoped = derive_trim_for(&snapshot, &batch);
    let expected: Vec<_> = full
        .iter()
        .filter(|piece| batch.contains(&piece.cell))
        .cloned()
        .collect();
    assert_eq!(
        scoped, expected,
        "scoped batch diverged from the full derivation"
    );

    // Cells with no projected pieces contribute nothing rather than panicking.
    let absent = observed_hex::HexCoord {
        q: u16::MAX - 1,
        r: u16::MAX - 1,
        level: 0,
    };
    assert!(derive_trim_for(&snapshot, &BTreeSet::from([absent])).is_empty());
}

/// Every lintel sits on a face the world actually calls a threshold, and there
/// is exactly one per named attachment.
///
/// `derive_trim` still emits none: the snapshot cannot tell a `Door` face from
/// any other shared face, and inventing one was the reason the rule waited.
#[test]
fn thresholds_put_a_lintel_on_every_named_room_port() {
    let world = showcase();
    let attachments = world.threshold_attachments();
    let lintels = derive_thresholds(&world);

    assert!(
        !attachments.is_empty(),
        "the showcase facility has rooms attached to halls"
    );
    assert_eq!(
        lintels.len(),
        attachments.len(),
        "one lintel per named threshold, no more and no fewer"
    );
    assert!(
        lintels
            .iter()
            .all(|piece| piece.kind == HexTrimKind::Lintel),
        "this pass emits lintels only"
    );

    // Each lintel is on the room's own cell and the port's own face.
    for (piece, attachment) in lintels.iter().zip(attachments.iter()) {
        assert_eq!(piece.cell, attachment.room_cell);
        assert_eq!(piece.face, attachment.face);
    }

    assert_eq!(lintels, derive_thresholds(&world), "pure over one world");

    let prototypes = tiles();
    let snapshot = HexWfcGeometrySnapshot::project(&world, &prototypes).expect("projection");
    assert!(
        !derive_trim(&snapshot)
            .iter()
            .any(|piece| piece.kind == HexTrimKind::Lintel),
        "the snapshot-fed pass must not guess at doorways"
    );
}

/// A lintel marks the boundary, so it must not move when the hall on the far
/// side reroutes. This is the geometric payoff of threshold-derived corridor
/// identity: both ends of an attachment are stable, so the doorway is too.
#[test]
fn a_lintel_stays_put_when_the_hall_beyond_it_reroutes() {
    use observed_core::PlayerId;
    use observed_facility::hex_wfc::HexObservationFrame;

    let mut moved = 0;
    for seed in 0xD00D_0100..0xD00D_0140u64 {
        let world = HexWfcWorld::generate(seed, showcase_config(4)).expect("world");
        let mut frame = HexObservationFrame::default();
        frame
            .occupied_cells
            .insert(PlayerId(0), world.config.spawn());
        frame.objective_cells.insert(world.config.spawn());

        let before = derive_thresholds(&world);
        let Ok(proposal) = world.propose_relayout(&frame) else {
            continue;
        };
        let mut committed = world.clone();
        if committed.commit_relayout(proposal, &frame).is_err() {
            continue;
        }

        // A room that survived keeps its ports, so any lintel whose room cell is
        // still a threshold must be in exactly the same place.
        let after = derive_thresholds(&committed);
        for piece in &before {
            if let Some(same) = after
                .iter()
                .find(|other| other.cell == piece.cell && other.face == piece.face)
                && (same.position != piece.position || same.rotation != piece.rotation)
            {
                moved += 1;
            }
        }
    }
    assert_eq!(moved, 0, "{moved} lintels shifted under a relayout");
}
