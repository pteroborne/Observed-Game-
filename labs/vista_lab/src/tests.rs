use bevy::math::{Vec2, Vec3};
use observed_facility::hex_wfc::{HexCoord, HexFace, HexSpace, PortClass};
use player_input::PlayerIntent;

use crate::composition::{CONFIG, UNSAFE_FROM_LEVEL, Vista, c};
use crate::exposure::{Drop, Form, exposure, survey};
use crate::geometry::{Look, Shape, WALKWAY_WIDTH, build, face_mid, floor_point};
use crate::walk::Walker;

fn span_cells() -> [HexCoord; 8] {
    [
        c(7, 8, 4),
        c(8, 8, 4),
        c(11, 6, 5),
        c(12, 5, 5),
        c(9, 10, 4),
        c(9, 11, 4),
        c(9, 10, 1),
        c(10, 10, 1),
    ]
}

#[test]
fn every_door_in_the_vista_meets_a_matching_door() {
    let vista = Vista::authored();
    let grid = CONFIG.grid();
    for placement in vista.built() {
        for face in HexFace::LATERAL {
            if !placement.is_open(face) {
                continue;
            }
            let next = grid
                .neighbor(placement.coord, face)
                .and_then(|at| vista.world.placements.get(&at))
                .unwrap_or_else(|| panic!("{:?} opens off the lattice", placement.coord));
            assert!(
                next.space.built() && next.is_open(face.opposite()),
                "{:?} opens {face:?} onto {:?} {:?}",
                placement.coord,
                next.coord,
                next.space
            );
        }
        if placement.up == PortClass::RampOpen {
            let head = grid.neighbor(placement.coord, HexFace::Up).expect("head");
            assert_eq!(vista.world.placements[&head].down, PortClass::RampOpen);
        }
    }
}

#[test]
fn the_facilitys_own_pass_decides_what_is_sky() {
    let vista = Vista::authored();
    assert!(vista.air_cells > 2_000, "{}", vista.air_cells);
    let unbuilt: Vec<_> = vista
        .world
        .placements
        .values()
        .filter(|p| p.space.unbuilt())
        .collect();
    // Nothing in this vista seals a pocket, so every unbuilt cell is open to the sky.
    assert!(unbuilt.iter().all(|p| p.space == HexSpace::Air));
    assert_eq!(unbuilt.len(), vista.air_cells);
}

#[test]
fn a_face_against_rock_is_buried_not_sheer() {
    let mut vista = Vista::authored();
    let bastion = c(5, 8, 2);
    let before = exposure(&vista.world, bastion, UNSAFE_FROM_LEVEL).expect("built");
    assert!(before.is_sheer(HexFace::West));
    // Entomb the cell west of the Bastion: now it is rock, and that face is buried.
    let west = CONFIG
        .grid()
        .neighbor(bastion, HexFace::West)
        .expect("west");
    vista.world.placements.get_mut(&west).expect("cell").space = HexSpace::Void;
    let after = exposure(&vista.world, bastion, UNSAFE_FROM_LEVEL).expect("built");
    assert!(!after.is_sheer(HexFace::West));
    assert_eq!(after.sheer_count() + 1, before.sheer_count());
}

#[test]
fn every_walkway_is_a_span_and_the_flight_is_a_flight() {
    let vista = Vista::authored();
    for at in span_cells() {
        let e = exposure(&vista.world, at, UNSAFE_FROM_LEVEL).expect("built");
        assert!(matches!(e.form, Form::Span { .. }), "{at:?}: {:?}", e.form);
        assert_eq!(e.sheer_count(), 4, "{at:?} has air on both flanks");
    }
    let foot = exposure(&vista.world, c(10, 7, 4), UNSAFE_FROM_LEVEL).expect("foot");
    assert_eq!(
        foot.form,
        Form::Flight {
            entry: HexFace::SouthWest
        }
    );
    let head = exposure(&vista.world, c(10, 7, 5), UNSAFE_FROM_LEVEL).expect("head");
    assert_eq!(head.form, Form::Landing);
}

#[test]
fn the_bastion_west_face_is_a_forty_metre_cliff() {
    let vista = Vista::authored();
    for level in 0..=4 {
        let e = exposure(&vista.world, c(5, 8, level), UNSAFE_FROM_LEVEL).expect("built");
        assert!(e.is_sheer(HexFace::West), "level {level}");
        let expected = if level < 4 { Form::Storey } else { Form::Deck };
        assert_eq!(e.form, expected, "level {level}");
    }
}

#[test]
fn floating_structures_hang_over_true_void() {
    let vista = Vista::authored();
    for at in [
        c(9, 8, 3),
        c(10, 8, 4),
        c(13, 4, 1),
        c(7, 10, 1),
        c(9, 12, 2),
    ] {
        let e = exposure(&vista.world, at, UNSAFE_FROM_LEVEL).expect("built");
        assert!(
            matches!(e.drop, Drop::Hanging { onto: None, .. }),
            "{at:?}: {:?}",
            e.drop
        );
    }
    // The Gallery span's drop is caught: the understory is twenty-four metres down.
    let over = exposure(&vista.world, c(9, 10, 4), UNSAFE_FROM_LEVEL).expect("built");
    assert_eq!(
        over.drop,
        Drop::Hanging {
            levels: 2,
            onto: Some(c(9, 10, 1))
        }
    );
    // Floor to floor: level four down to level one is three storeys, not two.
    assert_eq!(over.drop.metres(), Some(24.0));
}

#[test]
fn railings_stop_where_the_architecture_turns_unsafe() {
    let vista = Vista::authored();
    let railed = |at| {
        exposure(&vista.world, at, UNSAFE_FROM_LEVEL)
            .expect("built")
            .railed
    };
    assert!(railed(c(7, 8, 4)));
    assert!(railed(c(9, 10, 1)));
    assert!(!railed(c(11, 6, 5)));
    // The flight arrives at level five, so it is judged there.
    assert!(!railed(c(10, 7, 4)));
    assert!(!railed(c(13, 4, 5)));
}

#[test]
fn the_facilitys_router_takes_the_same_tour() {
    let vista = Vista::authored();
    let route = vista
        .world
        .route_between_cells(vista.tour[0], *vista.tour.last().expect("tour"))
        .expect("the Needle is reachable from the Bastion");
    assert_eq!(route.cells, vista.tour);
}

#[test]
fn the_far_shore_is_one_cell_of_air_away_and_unreachable() {
    let vista = Vista::authored();
    assert_eq!(vista.world.placements[&c(11, 12, 4)].space, HexSpace::Air);
    assert!(
        vista
            .world
            .route_between_cells(c(10, 12, 4), c(12, 12, 4))
            .is_none()
    );
}

#[test]
fn walkways_are_narrow_and_decks_are_whole() {
    let vista = Vista::authored();
    let pieces = build(&vista, &survey(&vista.world, UNSAFE_FROM_LEVEL)).pieces;
    for at in span_cells() {
        let decks: Vec<_> = pieces
            .iter()
            .filter(|p| p.cell == at && p.look == Look::WalkwayDeck)
            .collect();
        assert_eq!(decks.len(), 1, "{at:?}");
        let Shape::Block { half, .. } = decks[0].shape else {
            panic!("a walkway deck is a block");
        };
        assert!((half.z * 2.0 - WALKWAY_WIDTH).abs() < 1e-4);
        assert!(
            half.x * 2.0 > 13.5,
            "the deck runs the cell: {}",
            half.x * 2.0
        );
    }
}

#[test]
fn each_storey_face_is_either_cliff_or_wall_never_both() {
    let vista = Vista::authored();
    let exposures = survey(&vista.world, UNSAFE_FROM_LEVEL);
    let pieces = build(&vista, &exposures).pieces;
    for e in exposures.iter().filter(|e| e.form == Form::Storey) {
        let solid = |look: fn(&Look) -> bool| {
            pieces
                .iter()
                .filter(|p| p.cell == e.coord && p.collides && look(&p.look))
                .count()
        };
        let cliffs = solid(|l| *l == Look::SheerFace);
        let walls = solid(|l| matches!(l, Look::Wall(_)));
        assert_eq!(cliffs as u32, e.sheer_count(), "{:?}", e.coord);
        assert_eq!(cliffs + walls, 6, "{:?}", e.coord);
    }
}

#[test]
fn every_drop_edge_is_lit() {
    let vista = Vista::authored();
    let exposures = survey(&vista.world, UNSAFE_FROM_LEVEL);
    let pieces = build(&vista, &exposures).pieces;
    for e in exposures.iter().filter(|e| e.form == Form::Deck) {
        let lips = pieces
            .iter()
            .filter(|p| p.cell == e.coord && p.look == Look::FallEdge)
            .count();
        assert!(lips as u32 >= e.sheer_count(), "{:?}", e.coord);
    }
    for at in span_cells() {
        let lips = pieces
            .iter()
            .filter(|p| p.cell == at && p.look == Look::FallEdge)
            .count();
        assert_eq!(lips, 2, "{at:?}: both flanks");
    }
}

#[test]
fn keels_never_reach_what_they_hang_over() {
    let vista = Vista::authored();
    let exposures = survey(&vista.world, UNSAFE_FROM_LEVEL);
    let pieces = build(&vista, &exposures).pieces;
    let keels = pieces.iter().filter(|p| p.look == Look::Underside).count();
    assert!(keels > 20, "{keels}");
    for e in &exposures {
        let Drop::Hanging {
            onto: Some(onto), ..
        } = e.drop
        else {
            continue;
        };
        let catch = floor_point(onto).y;
        for piece in pieces
            .iter()
            .filter(|p| p.cell == e.coord && p.look == Look::Underside)
        {
            let Shape::Hull(points) = &piece.shape else {
                panic!("a keel is a hull");
            };
            let lowest = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
            assert!(lowest > catch + 1.0, "{:?} keel reaches {onto:?}", e.coord);
        }
    }
}

#[test]
fn the_build_is_deterministic() {
    let a = Vista::authored();
    let b = Vista::authored();
    let first = build(&a, &survey(&a.world, UNSAFE_FROM_LEVEL));
    assert_eq!(first, build(&b, &survey(&b.world, UNSAFE_FROM_LEVEL)));
    assert_eq!(
        first.collider_specs().len(),
        first.pieces.iter().filter(|p| p.collides).count()
    );
}

fn built() -> (Vista, crate::geometry::Build) {
    let vista = Vista::authored();
    let pieces = build(&vista, &survey(&vista.world, UNSAFE_FROM_LEVEL));
    (vista, pieces)
}

#[test]
fn the_production_controller_walks_the_whole_tour() {
    let (vista, pieces) = built();
    let mut walker = Walker::touring(&vista, &pieces);
    while !walker.finished() && walker.elapsed < 90.0 {
        walker.step_tour();
    }
    let needle = floor_point(c(13, 4, 5)).y;
    assert!(
        walker.finished(),
        "stalled at waypoint {} after {:.1}s, feet {:?}",
        walker.next,
        walker.elapsed,
        walker.feet()
    );
    assert!(!walker.recovered, "fell into the void on the way");
    assert!(
        walker.lowest_feet > floor_point(c(5, 8, 4)).y - 0.3,
        "dipped to {}",
        walker.lowest_feet
    );
    assert!(
        (walker.feet().y - needle).abs() < 0.3,
        "{:?}",
        walker.feet()
    );
}

/// Stand mid-span and walk straight off the side for four seconds.
fn walk_off_the_side(at: HexCoord) -> (f32, Walker) {
    let (vista, pieces) = built();
    let e = exposure(&vista.world, at, UNSAFE_FROM_LEVEL).expect("span");
    let Form::Span { axis } = e.form else {
        panic!("{at:?} is a span");
    };
    let along = face_mid(axis).normalize();
    let across = Vec2::new(-along.z, along.x);
    let floor = floor_point(at).y;
    let mut walker = Walker::standing(&pieces, floor_point(at), across);
    for _ in 0..240 {
        walker.step_with(PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..PlayerIntent::default()
        });
    }
    (floor, walker)
}

#[test]
fn an_unrailed_span_lets_you_walk_off_it() {
    let (floor, walker) = walk_off_the_side(c(11, 6, 5));
    assert!(
        walker.lowest_feet < floor - 8.0,
        "still at {} after walking off",
        walker.lowest_feet
    );
}

#[test]
fn a_railed_span_holds_you() {
    let (floor, walker) = walk_off_the_side(c(7, 8, 4));
    let start = floor_point(c(7, 8, 4));
    let strayed = Vec3::new(walker.feet().x - start.x, 0.0, walker.feet().z - start.z).length();
    assert!((walker.feet().y - floor).abs() < 0.3, "{:?}", walker.feet());
    assert!(strayed < WALKWAY_WIDTH * 0.5, "{strayed}");
}
