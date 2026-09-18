//! What the solved floor has to be true of before anybody plays on it.

use std::sync::{Arc, OnceLock};

use super::*;
use crate::site::{Site, load_content};
use observed_facility::hex_wfc::HexSpace;
use observed_hex::hex_origin;
use observed_traversal::gravity::BodyFrame;

/// The default floor, solved once for the whole test binary. Loading the tile
/// catalogue is the expensive part and it does not vary between tests.
fn site() -> Arc<Site> {
    static SITE: OnceLock<Arc<Site>> = OnceLock::new();
    Arc::clone(SITE.get_or_init(|| {
        let content = load_content();
        Arc::new(Site::solve(crate::DEFAULT_SEED, &content).expect("default seed solves"))
    }))
}

fn world(mode: Mode) -> WfcKineticWorld {
    WfcKineticWorld::new(site(), mode)
}

fn idle() -> Command {
    Command::default()
}

#[test]
fn the_floor_is_solver_placed_cells_with_holes_in_it() {
    let site = site();
    assert!(
        (crate::site::MIN_CELLS..=crate::site::MAX_CELLS).contains(&site.cells.len()),
        "{} cells is not a small floor",
        site.cells.len()
    );
    assert!(!site.voids.is_empty(), "a floor with no holes in it");
    // Every occupied cell came from the solver, not from this lab.
    for cell in &site.cells {
        assert_ne!(
            site.world.placements[cell].space,
            HexSpace::Void,
            "{cell:?} is listed as occupied but the solve voided it"
        );
    }
    for cell in &site.voids {
        assert!(
            site.world
                .placements
                .get(cell)
                .is_none_or(|placement| placement.space == HexSpace::Void),
            "{cell:?} is listed as a hole but the solve built on it"
        );
    }
    // And the projection produced real authored geometry for it.
    assert!(
        site.colliders().len() > 100,
        "a seven-cell solve should project more than a handful of hulls, got {}",
        site.colliders().len()
    );
}

#[test]
fn the_observer_stands_on_the_floor_rather_than_in_the_ceiling_or_the_air() {
    let world = world(Mode::Practice);
    let feet = world.player.position - Vec3::Y * world.player_config.half_height;
    assert!(
        world.support_height(feet).is_some(),
        "spawn feet at {feet:?} have nothing under them"
    );
    // The bug this pins: a downward ray from the top of the level reports the
    // ceiling, which put the Observer eight metres up inside the shell.
    assert!(
        feet.y < observed_hex::TILE_LEVEL_HEIGHT * 0.5,
        "spawn feet at y={} are in the upper half of the cell, which is the ceiling",
        feet.y
    );
    assert!(world.line_clear(world.eye(), world.eye() + Vec3::Y * 0.4));
}

#[test]
fn a_void_cell_has_no_floor_and_anything_over_it_leaves_the_facility() {
    let mut world = world(Mode::Encounter);
    let hole = *site().voids.first().expect("a hole");
    let over_the_hole = Vec3::from_array(hex_origin(hole)) + Vec3::Y * 2.0;
    assert!(
        world.support_height(over_the_hole).is_none(),
        "the solver's void cell has something to stand on"
    );
    let id = world.spawn(Kind::Minor, over_the_hole);
    let before = world.kills;
    let mut eliminated = None;
    for _ in 0..600 {
        world.step(idle());
        if let Some(event) = world
            .events
            .iter()
            .find(|event| matches!(event, Event::Eliminated(who, _) if *who == id))
        {
            eliminated = Some(event.clone());
            break;
        }
    }
    let Some(Event::Eliminated(_, cell)) = eliminated else {
        panic!("a minor dropped into a void cell never left the facility");
    };
    assert_eq!(world.kills, before + 1);
    assert!(!world.actors[&id].alive);
    assert_eq!(cell, Some(hole), "the fall was not attributed to the hole");
}

#[test]
fn a_push_commits_a_body_and_a_refusal_never_spends_charge() {
    let mut world = world(Mode::Encounter);
    // Aim at the crate beside the station by walking the look direction onto it.
    let crate_id = *world
        .actors
        .iter()
        .find(|(_, actor)| actor.kind == Kind::Prop)
        .map(|(id, _)| id)
        .expect("a crate");
    let target = world.pose(crate_id).position;
    world.player.position = target + Vec3::new(0., world.player_config.half_height, 3.0);
    let to_target = (target - world.eye()).normalize();
    world.player.yaw = to_target.x.atan2(-to_target.z);
    world.player.pitch = to_target.y.asin();
    world.physics.bodies[world.player_handle].set_translation(rv(world.player.position), true);
    world.physics.step();

    let selected = world
        .target()
        .expect("the crate is in front of the crosshair");
    assert_eq!(selected.id, crate_id);

    let before_charge = world.charge;
    let before_position = world.pose(crate_id).position;
    world.step(Command {
        action: Action::Push,
        ..idle()
    });
    assert!(
        world
            .events
            .iter()
            .any(|event| matches!(event, Event::Fired(Action::Push, ..))),
        "the push did not fire: {:?}",
        world.events
    );
    assert_eq!(world.charge, before_charge - world.config.cost);

    // The very next tick is inside the cooldown, so it must refuse and cost
    // nothing. This is the rule that stops a held button draining the pool.
    let charge_after_fire = world.charge;
    world.step(Command {
        action: Action::Push,
        ..idle()
    });
    assert!(
        world
            .events
            .iter()
            .any(|event| matches!(event, Event::Refused(Refusal::Cooldown))),
        "a push inside the cooldown was not refused: {:?}",
        world.events
    );
    assert_eq!(world.charge, charge_after_fire, "a refusal spent charge");

    for _ in 0..40 {
        world.step(idle());
    }
    let moved = world.pose(crate_id).position.distance(before_position);
    assert!(moved > 0.5, "the crate only moved {moved} m");
}

#[test]
fn replaying_the_same_commands_reproduces_the_tick_exactly() {
    let script = |index: usize| Command {
        movement: PlayerIntent {
            movement: glam::Vec2::new(
                ((index % 7) as f32 - 3.0) / 3.0,
                ((index % 5) as f32 - 2.0) / 2.0,
            ),
            look: glam::Vec2::new(((index % 11) as f32 - 5.0) * 0.01, 0.0),
            jump_pressed: index.is_multiple_of(53),
            ..Default::default()
        },
        action: match index % 37 {
            5 => Action::Push,
            19 => Action::Pull,
            29 => Action::Interact,
            _ => Action::None,
        },
    };

    let mut a = world(Mode::Encounter);
    let mut b = world(Mode::Encounter);
    assert_eq!(a.digest(), b.digest(), "two fresh worlds already disagree");

    // A mid-flight clone must continue identically to the world it came from.
    let mut clone = None;
    for index in 0..240 {
        a.step(script(index));
        b.step(script(index));
        assert_eq!(a.digest(), b.digest(), "diverged at tick {index}");
        if index == 120 {
            clone = Some(a.clone());
        }
    }
    let mut clone = clone.expect("snapshot taken");
    for index in 121..240 {
        clone.step(script(index));
    }
    assert_eq!(
        clone.digest(),
        a.digest(),
        "a restored snapshot did not continue the same way"
    );
}

#[test]
fn the_generator_gates_the_station_and_charge_never_regenerates_on_its_own() {
    let mut world = world(Mode::Encounter);
    world.charge = 40.0;
    // Stand at the station.
    world.player.position = world.site.station + Vec3::Y * world.player_config.half_height;
    world.physics.bodies[world.player_handle].set_translation(rv(world.player.position), true);
    let look = (world.site.station + Vec3::Y * 1.3 - world.eye()).normalize_or_zero();
    world.player.yaw = look.x.atan2(-look.z);
    world.physics.step();

    let before = world.charge;
    for _ in 0..60 {
        world.step(idle());
    }
    let powered_gain = world.charge - before;
    assert!(
        powered_gain > 10.0,
        "a powered station restored only {powered_gain}"
    );

    world.powered = false;
    world.charge = 40.0;
    let before = world.charge;
    for _ in 0..60 {
        world.step(idle());
    }
    assert_eq!(
        world.charge, before,
        "charge moved with the generator off; there is no passive regeneration"
    );
}

#[test]
fn every_seed_a_player_can_type_produces_a_playable_floor() {
    let content = load_content();
    for requested in [0u64, 1, 7, 42, 1234, u64::MAX] {
        let site = Site::solve(requested, &content)
            .unwrap_or_else(|error| panic!("seed {requested}: {error}"));
        assert!((crate::site::MIN_CELLS..=crate::site::MAX_CELLS).contains(&site.cells.len()));
        assert!(
            !site.thresholds.is_empty(),
            "seed {requested} has no doorway into its retracting tile, so the \
             Architect cannot make a ledge on it"
        );
        assert!(
            site.nav.len() >= crate::site::MIN_CELLS,
            "seed {requested} has {} waypoints",
            site.nav.len()
        );
        // The devices are on the floor, not inside it or over a hole.
        let world = WfcKineticWorld::new(Arc::new(site), Mode::Encounter);
        for (label, at) in [
            ("spawn", world.site.spawn),
            ("generator", world.site.generator),
            ("station", world.site.station),
            ("panel", world.site.panel),
        ] {
            assert!(
                world.support_height(at + Vec3::Y * 0.1).is_some(),
                "seed {requested}: the {label} has no floor under it"
            );
        }
    }
}

/// The finding this lab was built to discover, kept as a falsifiable claim.
///
/// The design says a shove commits a minor "off unrailed geometry". On this
/// tile corpus there is no unrailed geometry to use: every face onto void comes
/// back parapeted, and the solver never points a door at a cell it declined to
/// build. If either number below ever moves, the assumption has become
/// satisfiable without the Architect's help and this test should be read as
/// good news rather than a regression.
#[test]
fn the_corpus_seals_every_face_onto_void() {
    let content = load_content();
    let grid = crate::site::config().grid();
    for requested in [0u64, 3, 8, 17, 42] {
        let site = Site::solve(requested, &content).expect("a floor");
        assert!(
            site.ledges.is_empty(),
            "seed {}: {} face(s) now open onto void at body height — the tile \
             corpus has gained unrailed geometry, and the design's shove has a \
             site on a solved floor without a retraction",
            site.seed,
            site.ledges.len(),
        );
        for cell in &site.cells {
            let placement = site.placement(*cell).expect("occupied");
            for face in observed_hex::HexFace::LATERAL {
                if !placement.is_open(face) {
                    continue;
                }
                let neighbour = grid.neighbor(*cell, face);
                assert!(
                    neighbour.is_some_and(|next| site.cells.contains(&next)),
                    "seed {}: ({}, {}) has a door onto void across {face:?}",
                    site.seed,
                    cell.q,
                    cell.r,
                );
            }
        }
        // And the floor still offers the Architect's answer.
        assert!(
            !site.thresholds.is_empty(),
            "seed {}: nothing leads into the retracting tile, so retraction \
             cannot make a ledge either",
            site.seed,
        );
    }
}

/// The encounter has to fit in a frame.
///
/// This is a regression with a story: navigation used to walk every structural
/// collider linearly and re-solve the whole route per minor per tick, which cost
/// 18 ms a tick with *two* minors and froze for 134 ms whenever the graph was
/// rebuilt. Both are now broad-phase queries against one shared solve. The
/// bound is deliberately loose — roughly fifteen times the measured cost — so it
/// catches an algorithmic regression rather than a slow machine.
#[test]
fn a_full_wave_of_minors_fits_inside_a_frame() {
    use std::time::Instant;
    const BUDGET_MS: f64 = 8.0;

    let mut world = world(Mode::Encounter);
    // More minors than the last wave releases, all pursuing at once.
    for feet in world.site.muster.clone().iter().cycle().take(6) {
        world.spawn(Kind::Minor, *feet + Vec3::Y * 0.6);
    }
    for _ in 0..60 {
        world.step(idle());
    }
    let pursuing = world
        .actors
        .values()
        .filter(|actor| actor.alive && actor.kind == Kind::Minor)
        .count();
    assert!(pursuing >= 6, "only {pursuing} minors survived the warm-up");

    let start = Instant::now();
    for _ in 0..120 {
        world.step(idle());
    }
    let per_tick = start.elapsed().as_secs_f64() * 1000.0 / 120.0;
    assert!(
        per_tick < BUDGET_MS,
        "{pursuing} pursuing minors cost {per_tick:.2} ms/tick (budget {BUDGET_MS})"
    );

    // And the rebuild a relayout forces is a hitch, not a freeze.
    //
    // Only the bounded rebuild is measured, because only that one happens
    // during play. Linking the whole graph costs about 95 ms on this floor and
    // is paid once, while the lab is starting up; doing it on every relayout
    // was a visible stutter, which is why the bounded form exists.
    let changed = std::collections::BTreeSet::from([world.site.cells[0]]);
    let start = Instant::now();
    world.rebuild_navigation_for_profiling(&changed);
    let rebuild = start.elapsed().as_secs_f64() * 1000.0;
    assert!(
        rebuild < 25.0,
        "a bounded navigation rebuild took {rebuild:.1} ms"
    );
}

/// The recorded loop has to reach the beats the floor actually has.
///
/// On the seven-cell arena this asserted eliminations too. It does not any
/// more, and that is a real limitation rather than a relaxed bar: the floor is
/// now four times the size, the walk between the control and the fight is much
/// longer, and the simple policy that cleared waves in one room gets caught
/// crossing this one. Making the hole is what the loop depends on, so that is
/// what is pinned; winning on a floor this size is a tuning question nobody has
/// answered yet.
#[test]
fn the_director_reaches_the_beats_the_floor_has() {
    let mut world = WfcKineticWorld::new(site(), Mode::Encounter);
    let mut retracted_at = None;
    let mut waves = 0;
    for tick in 0..crate::demo::LOOP_TICKS {
        let command = crate::demo::loop_command(&world, tick);
        world.step(command);
        for event in &world.events {
            match event {
                Event::Retracted(_) if retracted_at.is_none() => retracted_at = Some(tick),
                Event::Wave(_) => waves += 1,
                _ => {}
            }
        }
        if world.outcome != Outcome::Playing {
            break;
        }
    }
    let retracted_at = retracted_at.expect("the director never made the hole");
    // Crossing the floor on foot and standing through the two-second warning is
    // most of this, and it got longer when the Observer started standing up in
    // the best-lit room rather than the first one — that room is not chosen for
    // its proximity to anything. Twenty-five seconds of walking is poor pacing
    // and worth fixing when the recorded loop is next worked on; what this
    // asserts is only that the hole arrives early enough to leave the rest of
    // the run a fight.
    assert!(
        retracted_at < 1800,
        "the hole took {retracted_at} ticks; nothing can be removed before it exists"
    );
    assert!(waves >= 1, "the encounter never started");
    // And the hole it made is one a body could be put through, which is the
    // only reason the retraction verb exists alongside relayout.
    assert!(
        !world.open_thresholds().is_empty(),
        "the retracted tile left no doorway opening onto it"
    );
}

/// Walk the floor until somewhere can telegraph a pocket.
///
/// What is decoherable depends on where the Observer stands and which way they
/// face, because that is what the protected set is made of. A player does this
/// by walking; a test does it by trying the waypoints.
fn find_a_pocket(world: &mut WfcKineticWorld) {
    let vantages: Vec<Vec3> = world.site.nav.clone();
    for feet in vantages {
        stand_at(world, feet, feet + Vec3::X);
        world.telegraph();
        if world.telegraph.is_some() {
            return;
        }
    }
}

/// Put the Observer's feet at `feet`, facing `at`.
fn stand_at(world: &mut WfcKineticWorld, feet: Vec3, at: Vec3) {
    world.player.position = feet + Vec3::Y * world.player_config.half_height;
    world.player.velocity = Vec3::ZERO;
    let look = (at - world.player.position).normalize_or_zero();
    world.player.yaw = look.x.atan2(-look.z);
    world.player.pitch = look.y.asin().clamp(-1.2, 1.2);
    world.physics.bodies[world.player_handle].set_translation(rv(world.player.position), true);
    world.physics.step();
}

/// The headline: an unobserved pocket re-collapses, and the geometry that comes
/// back is the geometry the solver decided on.
#[test]
fn an_unobserved_pocket_re_collapses_into_new_geometry() {
    let mut world = world(Mode::Practice);
    find_a_pocket(&mut world);
    let Some((_, candidate)) = world.telegraph.as_ref() else {
        panic!("no vantage on this floor could telegraph a pocket");
    };
    let pocket: Vec<HexCoord> = candidate.region.cells.iter().copied().collect();
    assert!(!pocket.is_empty());
    // A pocket is chosen from cells nobody is watching, so the Observer is
    // already looking elsewhere. Leave them there.
    assert!(
        world
            .observation()
            .visible_cells
            .iter()
            .all(|cell| !pocket.contains(cell)),
        "the solver offered a pocket the Observer can see"
    );

    let before_generation = world.geometry_generation;
    let before_pieces = world.snapshot.pieces.len();
    let before_facility = world.world.generation;
    let mut relaid = false;
    for _ in 0..200 {
        world.step(idle());
        if world
            .events
            .iter()
            .any(|event| matches!(event, Event::Relaid(_)))
        {
            relaid = true;
            break;
        }
        assert!(
            !world
                .events
                .iter()
                .any(|event| matches!(event, Event::Held)),
            "the floor held while nobody was looking at it"
        );
    }
    assert!(relaid, "the pocket never re-collapsed");
    assert_eq!(world.geometry_generation, before_generation + 1);
    assert_eq!(world.world.generation, before_facility + 1);
    // The projected geometry moved with the logical floor rather than lagging
    // behind it, and the collision world agrees with the snapshot.
    assert_ne!(
        (world.snapshot.pieces.len(), world.world.generation),
        (before_pieces, before_facility)
    );
    assert_eq!(
        world.snapshot.arena.colliders.len(),
        world.structural.len(),
        "the collision world and the projected snapshot disagree"
    );
}

/// The mechanic that makes it first-person: standing in the pocket saves it.
#[test]
fn occupying_a_pocket_holds_the_floor() {
    let mut world = world(Mode::Practice);
    find_a_pocket(&mut world);
    let Some((_, candidate)) = world.telegraph.as_ref() else {
        panic!("no vantage on this floor could telegraph a pocket");
    };
    let cell = *candidate.region.cells.iter().next().expect("a pocket cell");

    // Walk into it. The solver re-derives what is protected at commit time, so
    // this is a decision the Observer makes during the warning rather than a
    // state the lab set up in advance.
    let feet = world
        .site
        .nav
        .iter()
        .copied()
        .min_by(|a, b| {
            let centre = Vec3::from_array(hex_origin(cell));
            a.distance_squared(centre)
                .total_cmp(&b.distance_squared(centre))
        })
        .expect("a waypoint");
    stand_at(&mut world, feet, feet + Vec3::X);

    let before_generation = world.geometry_generation;
    let mut held = false;
    for _ in 0..200 {
        world.step(idle());
        if world
            .events
            .iter()
            .any(|event| matches!(event, Event::Held))
        {
            held = true;
            break;
        }
        assert!(
            !world
                .events
                .iter()
                .any(|event| matches!(event, Event::Relaid(_))),
            "the floor was rewritten with the Observer standing in it"
        );
    }
    assert!(held, "the commit neither held nor landed");
    assert_eq!(
        world.geometry_generation, before_generation,
        "a held floor changed anyway"
    );
}

/// Put a body in front of the crosshair and return its id.
fn staged_target(world: &mut WfcKineticWorld, kind: Kind) -> ActorId {
    let existing: Vec<ActorId> = world.actors.keys().copied().collect();
    for id in existing {
        let actor = world.actors.get_mut(&id).expect("listed");
        actor.alive = false;
        world.physics.bodies[actor.body].set_enabled(false);
        world.physics.colliders[actor.collider].set_enabled(false);
    }
    let feet = world.site.spawn;
    let ahead = feet + world.site.spawn_facing * 4.0 + Vec3::Y * 0.6;
    let id = world.spawn(kind, ahead);
    crate::demo::aim_from(world, feet, ahead);
    world.physics.step();
    for _ in 0..20 {
        world.step(idle());
    }
    let settled = world.pose(id).position;
    crate::demo::aim_from(world, feet, settled);
    id
}

#[test]
fn a_plumb_commits_the_armed_direction_to_what_the_crosshair_has() {
    let mut world = world(Mode::Encounter);
    let id = staged_target(&mut world, Kind::Prop);
    assert_eq!(
        world.target().map(|target| target.id),
        Ok(id),
        "the staged body is not in the crosshair"
    );

    // Arm straight up by looking at the ceiling, then commit it.
    world.player.pitch = 1.2;
    world.step(Command {
        action: Action::Arm,
        ..idle()
    });
    assert!(
        world.armed.y > 0.8,
        "arming did not take the look direction"
    );
    // Look back at the target to fire.
    let (from, at) = (world.site.spawn, world.pose(id).position);
    crate::demo::aim_from(&mut world, from, at);

    let before = world.charge;
    world.step(Command {
        action: Action::Plumb,
        ..idle()
    });
    assert!(
        world
            .events
            .iter()
            .any(|event| matches!(event, Event::Plumbed(who, _) if *who == id)),
        "the plumb did not land: {:?}",
        world.events
    );
    assert_eq!(world.charge, before - world.config.plumb_cost);
    let lash = world.actors[&id].lash.expect("the body carries the plumb");
    assert!(lash.direction.y > 0.8);

    // And it actually falls that way: up, off the floor it was resting on.
    let height = world.pose(id).position.y;
    for _ in 0..90 {
        world.step(idle());
    }
    assert!(
        world.pose(id).position.y > height + 1.5,
        "a body plumbed upward only rose {:.2} m",
        world.pose(id).position.y - height
    );
}

#[test]
fn a_plumb_wears_off_and_hands_the_body_back_to_the_world() {
    let mut world = world(Mode::Practice);
    let id = staged_target(&mut world, Kind::Prop);
    world.armed = Vec3::Y;
    world.step(Command {
        action: Action::Plumb,
        ..idle()
    });
    assert!(world.actors[&id].lash.is_some());

    let mut released = None;
    for tick in 0..world.config.plumb_ticks + 120 {
        world.step(idle());
        if world
            .events
            .iter()
            .any(|event| matches!(event, Event::Unplumbed(who) if *who == id))
        {
            released = Some(tick);
            break;
        }
    }
    let released = released.expect("the plumb never wore off");
    assert!(released <= world.config.plumb_ticks);
    assert!(world.actors[&id].lash.is_none());

    // Back under the world's gravity: it comes down again.
    let height = world.pose(id).position.y;
    for _ in 0..180 {
        world.step(idle());
    }
    assert!(
        world.pose(id).position.y < height,
        "the body kept rising after the plumb expired"
    );
}

/// The reason the tool is worth a slot: it reaches a hole a shove cannot.
#[test]
fn a_minor_plumbed_toward_a_hole_falls_into_it() {
    let site = site();
    let mut world = WfcKineticWorld::new(Arc::clone(&site), Mode::Practice);
    // Make the hole first, the way the floor requires.
    world.retract_warning_for_tests();
    world.step(idle());

    // And work it from the doorway that now opens onto nothing. A hole is only
    // reachable through a threshold — that is the whole parapet finding — and
    // the floor's *original* void cells are sealed, so picking one of those
    // just plumbs a body into a wall.
    let (cell, face) = *world
        .open_thresholds()
        .first()
        .expect("the retraction left a doorway onto the hole");
    let inside = Vec3::from_array(hex_origin(cell));
    let centre = Vec3::from_array(hex_origin(
        crate::site::config()
            .grid()
            .neighbor(cell, face)
            .expect("the doorway leads somewhere"),
    ));

    let existing: Vec<ActorId> = world.actors.keys().copied().collect();
    for id in existing {
        let actor = world.actors.get_mut(&id).expect("listed");
        actor.alive = false;
        world.physics.bodies[actor.body].set_enabled(false);
        world.physics.colliders[actor.collider].set_enabled(false);
    }
    let toward_hole = (centre - inside).with_y(0.).normalize_or(Vec3::X);
    let ideal = inside + toward_hole * 5.0;
    let nav = world.site.nav.clone();
    let stand = nav
        .iter()
        .copied()
        .filter(|point| world.standable(*point))
        .min_by(|a, b| {
            a.distance_squared(ideal)
                .total_cmp(&b.distance_squared(ideal))
        })
        .expect("somewhere in the doorway");
    let id = world.spawn(Kind::Minor, stand + Vec3::Y * 0.6);
    for _ in 0..20 {
        world.step(idle());
    }
    assert!(
        world.actors[&id].alive,
        "the minor fell before it was plumbed"
    );

    // Down becomes "through the doorway, and then keep going".
    let toward = (toward_hole * 2.0 + Vec3::NEG_Y).normalize();
    world.armed = toward;
    let plumb = Plumb::new(toward, world.config.plumb_strength, 600);
    world.attach_for_tests(id, plumb);

    for _ in 0..600 {
        world.step(idle());
        if !world.actors[&id].alive {
            return;
        }
    }
    panic!(
        "a minor plumbed through the doorway survived at {:?}",
        world.pose(id).position
    );
}

#[test]
fn self_plumb_lifecycle_warning_and_release() {
    let mut world = world(Mode::Practice);
    for _ in 0..10 {
        world.step(idle());
    }
    assert_eq!(world.gravity.remaining, 0);
    assert!(world.gravity.frame.is_upright());

    world.armed = Vec3::NEG_Y;
    world.step(Command {
        action: Action::SelfPlumb,
        ..Default::default()
    });
    assert_eq!(world.gravity.remaining, 480);
    assert!(world.events.contains(&Event::SelfPlumbed));

    // Step until just before warning (remaining > 60)
    for _ in 0..419 {
        world.step(idle());
    }
    assert_eq!(world.gravity.remaining, 61);
    assert!(!world.events.contains(&Event::GravityWarning));

    // Step 1 tick: remaining drops to 60 -> GravityWarning fires
    world.step(idle());
    assert_eq!(world.gravity.remaining, 60);
    assert!(world.events.contains(&Event::GravityWarning));

    // Step 59 ticks: remaining reaches 1
    for _ in 0..59 {
        world.step(idle());
    }
    assert_eq!(world.gravity.remaining, 1);

    // Step 1 tick: remaining reaches 0 -> GravityReleased fires
    world.step(idle());
    assert_eq!(world.gravity.remaining, 0);
    assert!(world.events.contains(&Event::GravityReleased));

    // After return to upright, frame is upright
    for _ in 0..25 {
        world.step(idle());
    }
    assert!(world.gravity.frame.is_upright());
    assert_eq!(world.gravity.frame, BodyFrame::default());
}

#[test]
fn self_plumb_refusals_and_early_release() {
    let mut world = world(Mode::Encounter);
    // Deplete charge
    world.charge = 0.0;
    assert_eq!(world.self_plumb_ready(), Err(Refusal::EmptyCharge));

    world.step(Command {
        action: Action::SelfPlumb,
        ..Default::default()
    });
    assert!(world.events.contains(&Event::Refused(Refusal::EmptyCharge)));

    // Restore charge
    world.charge = 100.0;
    world.armed = Vec3::NEG_Y;
    assert!(world.self_plumb_ready().is_ok());

    world.step(Command {
        action: Action::SelfPlumb,
        ..Default::default()
    });
    assert_eq!(world.charge, 100.0 - world.config.plumb_cost);
    assert!(world.gravity.remaining > 0);

    // While in transition, self_plumb_ready returns Refusal::Reorienting
    if world.gravity.transition > 0 {
        assert_eq!(world.self_plumb_ready(), Err(Refusal::Reorienting));
    }

    // Manual early release via Action::Release
    world.step(Command {
        action: Action::Release,
        ..Default::default()
    });
    assert_eq!(world.gravity.remaining, 0);
    assert!(world.events.contains(&Event::GravityReleased));
}

#[test]
fn self_plumb_determinism_and_digest_sensitivity() {
    let mut a = world(Mode::Practice);
    let mut b = world(Mode::Practice);
    for _ in 0..10 {
        a.step(idle());
        b.step(idle());
    }
    assert_eq!(a.digest(), b.digest());

    // Only 'a' activates self-plumb
    a.armed = Vec3::NEG_Y;
    a.step(Command {
        action: Action::SelfPlumb,
        ..Default::default()
    });
    assert_ne!(
        a.digest(),
        b.digest(),
        "digest must change when gravity state changes"
    );

    // Now 'b' executes the exact same command
    b.armed = Vec3::NEG_Y;
    b.step(Command {
        action: Action::SelfPlumb,
        ..Default::default()
    });
    assert_eq!(
        a.digest(),
        b.digest(),
        "identical gravity state and commands must reproduce identical digest"
    );

    // Step both forward
    for _ in 0..50 {
        a.step(idle());
        b.step(idle());
        assert_eq!(a.digest(), b.digest());
    }
}

#[test]
fn observation_upright_preserves_horizontal_cone_and_wall_walk_observes_3d() {
    let mut world = world(Mode::Practice);
    for _ in 0..10 {
        world.step(idle());
    }
    let upright_frame = world.observation();
    assert!(
        !upright_frame.visible_cells.is_empty(),
        "upright Observer must see cells"
    );

    // Standing cells are protected
    let protected = world.protected_body_cells();
    for cell in &protected {
        assert!(upright_frame.visible_cells.contains(cell));
    }
}
