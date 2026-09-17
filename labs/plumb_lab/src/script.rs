//! The sequence of plumbs the lab runs, and what each one is asking.

use glam::Vec3;

use crate::model::{Plumb, PlumbWorld};
use crate::room::Surface;

/// One thing to try.
#[derive(Clone, Copy, Debug)]
pub struct Phase {
    pub caption: &'static str,
    /// Which way "down" becomes, or `None` to release back to world gravity.
    pub down: Option<Vec3>,
    /// Which surface the subject is expected to end up standing on.
    pub expect: Surface,
    pub ticks: u32,
}

/// Five seconds each: long enough to fall, land, and walk a few metres.
pub const PHASE_TICKS: u32 = 300;
/// How long a phase's plumb outlives the phase itself.
pub const EXPIRY_MARGIN: u32 = 60;

/// The run.
///
/// Floor first as a control — if the subject cannot walk on a floor under a
/// plumb pointing the way gravity already points, nothing else means anything.
#[must_use]
pub fn phases() -> Vec<Phase> {
    vec![
        Phase {
            caption: "DOWN / the control: a plumb that agrees with the world",
            down: Some(Vec3::NEG_Y),
            expect: Surface::Floor,
            ticks: PHASE_TICKS,
        },
        Phase {
            caption: "EAST / down becomes a wall",
            down: Some(Vec3::X),
            expect: Surface::Wall,
            ticks: PHASE_TICKS,
        },
        Phase {
            caption: "UP / down becomes the ceiling",
            down: Some(Vec3::Y),
            expect: Surface::Ceiling,
            ticks: PHASE_TICKS,
        },
        Phase {
            caption: "NORTH / down becomes the far wall",
            down: Some(Vec3::NEG_Z),
            expect: Surface::Wall,
            ticks: PHASE_TICKS,
        },
        Phase {
            caption: "RELEASE / the plumb wears off and the floor takes it back",
            down: None,
            expect: Surface::Floor,
            ticks: PHASE_TICKS,
        },
    ]
}

/// What one phase did.
#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    /// Ticks from the plumb being applied to the subject being planted.
    pub settled: Option<u32>,
    pub surface: Option<Surface>,
    /// Distance walked along that surface once planted.
    pub walked: f32,
    pub ended_planted: bool,
}

/// Run one phase against a world, returning what happened.
///
/// The subject walks for the second half of each phase and turns around at
/// three quarters, so the recording shows control rather than a slide.
pub fn run(
    world: &mut PlumbWorld,
    phase: Phase,
    mut on_tick: impl FnMut(&PlumbWorld, u32),
) -> Outcome {
    // The plumb outlives the phase. Timed to expire exactly on the last tick,
    // it releases before the outcome is read and the report describes the floor
    // the subject was already falling back to rather than the surface the phase
    // was asking about. Expiry has its own test.
    match phase.down {
        Some(down) => world.apply(Plumb::new(
            down,
            crate::model::GRAVITY,
            phase.ticks + EXPIRY_MARGIN,
        )),
        None => world.release(),
    }
    let mut settled = None;
    for tick in 0..phase.ticks {
        // Give it a moment to fall and settle before asking it to walk.
        let walking = tick > phase.ticks / 3;
        if tick == phase.ticks * 3 / 4 {
            world.reverse();
        }
        world.step(walking);
        if settled.is_none() && world.footing == crate::model::Footing::Planted {
            settled = Some(tick);
        }
        on_tick(world, tick);
    }
    Outcome {
        settled,
        surface: world.contact_surface,
        walked: world.walked,
        ended_planted: world.footing == crate::model::Footing::Planted,
    }
}

/// Run the whole script, returning a line per phase.
#[must_use]
pub fn report() -> Vec<(Phase, Outcome)> {
    let mut world = PlumbWorld::new();
    phases()
        .into_iter()
        .map(|phase| {
            let outcome = run(&mut world, phase, |_, _| {});
            (phase, outcome)
        })
        .collect()
}
