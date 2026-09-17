//! What the solved floor has to be true of before anybody plays on it.

use std::sync::{Arc, OnceLock};

use super::*;
use crate::site::{Site, load_content};
use observed_facility::hex_wfc::HexSpace;
use observed_hex::hex_origin;

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
fn the_floor_is_seven_solver_placed_cells_with_two_holes_in_it() {
    let site = site();
    assert_eq!(site.cells.len(), crate::site::CELL_COUNT);
    assert_eq!(site.voids.len(), 2);
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
fn retracting_a_tile_removes_that_tile_and_nothing_else() {
    let mut world = world(Mode::Practice);
    let cell = world.site.retracting;
    let before = world
        .site
        .colliders()
        .iter()
        .filter(|spec| world.physics.colliders[world.structural[&spec.id.0]].is_enabled())
        .count();
    let doomed: Vec<u32> = world.cell_colliders[&cell].clone();
    assert!(
        !doomed.is_empty(),
        "the retracting cell projected no geometry"
    );

    world.retract_warning = Some(1);
    world.step(idle());
    assert!(
        world
            .events
            .iter()
            .any(|event| matches!(event, Event::Retracted(at) if *at == cell)),
        "no retraction event: {:?}",
        world.events
    );
    assert!(!world.cell_present);

    let after = world
        .site
        .colliders()
        .iter()
        .filter(|spec| world.physics.colliders[world.structural[&spec.id.0]].is_enabled())
        .count();
    // Exactly the doomed tile's colliders went away, and they are all gone.
    let removed = before - after;
    let doomed_present: Vec<&u32> = doomed
        .iter()
        .filter(|id| world.structural.contains_key(id))
        .collect();
    assert_eq!(
        removed,
        doomed_present.len(),
        "retraction removed {removed} colliders but the tile owns {}",
        doomed_present.len()
    );
    for id in doomed_present {
        assert!(!world.physics.colliders[world.structural[id]].is_enabled());
    }
    // And the cell genuinely has no floor any more.
    let centre = Vec3::from_array(hex_origin(cell));
    assert!(
        world.support_height(centre + Vec3::Y * 0.2).is_none(),
        "the retracted tile still has a floor"
    );
}

#[test]
fn a_body_standing_on_the_retracted_tile_falls_out_of_the_facility() {
    let mut world = world(Mode::Encounter);
    let cell = world.site.retracting;
    let centre = Vec3::from_array(hex_origin(cell));
    let Some(feet) = world
        .site
        .nav
        .iter()
        .copied()
        .find(|point| point.with_y(centre.y).distance(centre) < 4.0)
    else {
        // Nothing stands on this tile on this seed; the retraction test above
        // still covers the geometry removal.
        return;
    };
    let id = world.spawn(Kind::Minor, feet + Vec3::Y * 0.6);
    for _ in 0..30 {
        world.step(idle());
    }
    assert!(
        world.actors[&id].alive,
        "the minor fell before the retraction"
    );
    world.retract_warning = Some(1);
    for _ in 0..600 {
        world.step(idle());
        if !world.actors[&id].alive {
            return;
        }
    }
    panic!("the minor survived the floor being deleted underneath it");
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
        assert_eq!(site.cells.len(), crate::site::CELL_COUNT);
        assert!(
            !site.thresholds.is_empty(),
            "seed {requested} has no doorway into its retracting tile, so the \
             Architect cannot make a ledge on it"
        );
        assert!(
            site.nav.len() >= crate::site::CELL_COUNT,
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

/// The lab's headline claim, and the one the corpus actually supports: the
/// solver seals every edge, so the only ledge on a solved floor is one the
/// Architect makes by retracting a tile — and the tool can then use it.
#[test]
fn a_retracted_tile_turns_its_doorway_into_a_drop_the_tool_can_use() {
    let site = site();
    let threshold = *site
        .thresholds
        .first()
        .expect("the doomed tile has a doorway into it");

    let outward = crate::demo::face_direction(threshold.face);
    let (stand, launch) = crate::demo::ledge_stage(&site, threshold);

    let mut world = WfcKineticWorld::new(Arc::clone(&site), Mode::Practice);
    let existing: Vec<ActorId> = world.actors.keys().copied().collect();
    for id in existing {
        let actor = world.actors.get_mut(&id).expect("listed");
        actor.alive = false;
        world.physics.bodies[actor.body].set_enabled(false);
        world.physics.colliders[actor.collider].set_enabled(false);
    }
    let id = world.spawn(Kind::Minor, launch + Vec3::Y * 0.6);
    crate::demo::aim_from(&mut world, stand, launch + Vec3::Y * 0.6);
    world.physics.step();
    for _ in 0..20 {
        world.step(idle());
    }

    // Before the retraction the threshold leads somewhere solid, so a shove
    // through it is survivable. That is the control for the claim below.
    let settled = world.pose(id).position;
    assert!(
        world.support_height(settled + outward * 7.0).is_some(),
        "the doorway already led nowhere before anything was retracted"
    );

    world.retract_warning = Some(1);
    world.step(idle());
    assert!(!world.cell_present, "the tile did not retract");
    assert!(
        world.support_height(settled + outward * 9.0).is_none(),
        "the retracted tile still catches a body"
    );

    let settled = world.pose(id).position;
    crate::demo::aim_from(&mut world, stand, settled + crate::demo::AIM_LIFT);
    let selected = world.target().expect("the minor is in the crosshair");
    assert_eq!(selected.id, id);
    world.step(Command {
        action: Action::Push,
        ..idle()
    });
    assert!(
        world
            .events
            .iter()
            .any(|event| matches!(event, Event::Fired(Action::Push, ..))),
        "the shove was refused: {:?}",
        world.events
    );

    let before = world.kills;
    for _ in 0..420 {
        world.step(idle());
        if !world.actors[&id].alive {
            assert_eq!(world.kills, before + 1);
            return;
        }
    }
    panic!(
        "shoved {outward:?} through a doorway onto a retracted tile and survived at {:?}",
        world.pose(id).position
    );
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

/// Retraction is a decision, not a light switch: the tile the panel removes is
/// chosen so that losing it actually severs the route the floor depends on.
#[test]
fn retraction_severs_the_crossing_it_was_chosen_to_sever() {
    let mut world = world(Mode::Practice);
    let (spawn, generator) = (world.site.spawn, world.site.generator);
    assert!(
        world.navigable(spawn, generator),
        "the floor did not connect the spawn room to the generator to begin with"
    );
    world.retract_warning = Some(1);
    world.step(idle());
    assert!(!world.cell_present);
    assert!(
        !world.navigable(spawn, generator),
        "the tile chosen as the floor's cut vertex was removed and the route survived"
    );
}
