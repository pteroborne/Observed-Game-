//! Is the plumb feasible at all? These are that question, phase by phase.

use super::*;
use crate::script::{self, Phase};

fn run_phase(world: &mut PlumbWorld, phase: Phase) -> script::Outcome {
    script::run(world, phase, |_, _| {})
}

#[test]
fn a_plumb_puts_the_subject_on_every_surface_of_the_room() {
    let mut world = PlumbWorld::new();
    for phase in script::phases() {
        let outcome = run_phase(&mut world, phase);
        assert!(
            outcome.ended_planted,
            "{}: the subject never came to rest (surface {:?})",
            phase.caption, outcome.surface
        );
        assert_eq!(
            outcome.surface,
            Some(phase.expect),
            "{}: came to rest on the wrong surface",
            phase.caption
        );
        let settled = outcome
            .settled
            .unwrap_or_else(|| panic!("{}: never planted", phase.caption));
        assert!(
            settled < script::PHASE_TICKS / 2,
            "{}: took {settled} ticks to land",
            phase.caption
        );
    }
}

#[test]
fn the_subject_walks_along_whatever_it_is_standing_on() {
    let mut world = PlumbWorld::new();
    for phase in script::phases() {
        let outcome = run_phase(&mut world, phase);
        // It walks out and back, so the distance covered is what matters
        // rather than the displacement. A body that merely slid under gravity
        // would not accumulate this, because only tangential motion counts.
        assert!(
            outcome.walked > 2.0,
            "{}: walked only {:.2} m along {:?}",
            phase.caption,
            outcome.walked,
            outcome.surface
        );
    }
}

#[test]
fn the_subject_hangs_from_the_ceiling_rather_than_falling_off_it() {
    let mut world = PlumbWorld::new();
    // Get it onto the ceiling.
    let ceiling = script::phases()
        .into_iter()
        .find(|phase| phase.caption.starts_with("UP /"))
        .expect("the script tries the ceiling");
    run_phase(&mut world, ceiling);
    assert_eq!(world.contact_surface, Some(crate::room::Surface::Ceiling));
    let height = world.position().y;
    // Hold it there well past the phase: an upward plumb has to keep holding,
    // not merely throw the body at the ceiling once.
    // Duration outlives the loop, so this measures holding rather than expiry.
    world.apply(Plumb::new(Vec3::Y, GRAVITY, 700));
    for _ in 0..600 {
        world.step(true);
    }
    assert_eq!(world.footing, Footing::Planted);
    assert!(
        (world.position().y - height).abs() < 0.5,
        "the subject drifted {:.2} m off the ceiling",
        world.position().y - height
    );
}

#[test]
fn releasing_a_plumb_returns_the_subject_to_the_world() {
    let mut world = PlumbWorld::new();
    world.apply(Plumb::new(Vec3::Y, GRAVITY, 200));
    for _ in 0..200 {
        world.step(false);
    }
    assert!(
        world.position().y > room::HEIGHT * 0.5,
        "never reached the ceiling"
    );
    // The plumb expires on its own; nothing external releases it.
    assert!(world.plumb.is_none(), "the plumb outlived its duration");
    for _ in 0..300 {
        world.step(false);
    }
    assert_eq!(world.contact_surface, Some(crate::room::Surface::Floor));
    assert!(world.position().y < 2.0, "did not come back down");
}

#[test]
fn the_same_script_reproduces_tick_for_tick() {
    let script = script::phases();
    let mut a = PlumbWorld::new();
    let mut b = PlumbWorld::new();
    assert_eq!(a.digest(), b.digest());
    for phase in script {
        run_phase(&mut a, phase);
        run_phase(&mut b, phase);
        assert_eq!(a.digest(), b.digest(), "{} diverged", phase.caption);
    }
    // And a mid-flight clone continues the same way.
    let mut clone = a.clone();
    for _ in 0..120 {
        a.step(true);
        clone.step(true);
    }
    assert_eq!(a.digest(), clone.digest());
}
