//! `OBSERVED2_CAPTURE=<dir>`: every candidate in every state, the line-up, the ranks,
//! and two short films of each form: hunting until it is seen, and hunting until it
//! catches someone.
//!
//! The plan is a flat list of frames, each naming what to show and where to write it.
//! Poses are pure functions of state and time, so every frame is exactly what the plan
//! says, however fast the machine renders.
use crate::form::{Form, State};
use crate::view::{Framing, Layout};

/// One frame of the plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub layout: Layout,
    pub framing: Framing,
    pub state: State,
    /// Seconds in `state`.
    pub t: f32,
    pub clock: f32,
    /// Where to write this frame, relative to the capture directory.
    pub output: Option<String>,
}

/// Frames a new layout is held before its first still, while it spawns and settles.
const SETTLE: usize = 8;
/// Frames held before anything is written at all, while the renderer compiles its
/// pipelines: the first still of a run came out empty with only `SETTLE`.
const WARM_UP: usize = 90;
/// Film frame rate.
pub const FPS: u16 = 30;

/// When a still of each state is taken: long enough in to be settled, and a catch
/// caught mid-flare.
fn still_time(state: State) -> f32 {
    match state {
        State::Hunting => 2.3,
        State::FrozenBySight | State::FrozenByAnchor => 1.2,
        State::Catch => 0.55,
    }
}

fn hold(frames: &mut Vec<Frame>, frame: &Frame, output: String) {
    for _ in 0..SETTLE {
        frames.push(Frame {
            output: None,
            ..frame.clone()
        });
    }
    frames.push(Frame {
        output: Some(output),
        ..frame.clone()
    });
}

/// A film's timeline, `(state, from, until)` in seconds. Each continues its hunting
/// pose unbroken: a Guardian goes from hunting to frozen, or from hunting to a catch,
/// never from frozen back to hunting in one shot, which would jump its mechanism.
pub type Timeline = [(State, f32, f32); 2];

/// Hunting, and then seen.
pub const SEEN: Timeline = [(State::Hunting, 0.0, 3.0), (State::FrozenBySight, 3.0, 5.5)];
/// Hunting, unseen, and then a catch.
pub const CATCH: Timeline = [(State::Hunting, 0.0, 1.5), (State::Catch, 1.5, 3.3)];

#[must_use]
pub fn plan() -> Vec<Frame> {
    let mut frames = Vec::new();
    for _ in 0..WARM_UP {
        frames.push(Frame {
            layout: Layout::Single(Form::MAJORS[0]),
            framing: Framing::ThreeQuarter,
            state: State::Hunting,
            t: 0.0,
            clock: 0.0,
            output: None,
        });
    }
    for (n, form) in Form::MAJORS.into_iter().enumerate() {
        let name = form.name();
        for (i, state) in State::ALL.into_iter().enumerate() {
            let t = still_time(state);
            let frame = Frame {
                layout: Layout::Single(form),
                framing: Framing::ThreeQuarter,
                state,
                t,
                clock: if state == State::Hunting { t } else { 3.0 + t },
                output: None,
            };
            hold(
                &mut frames,
                &frame,
                format!(
                    "{}{}_{name}_{state}.png",
                    n + 1,
                    i + 1,
                    state = state.name()
                ),
            );
        }
        let frame = Frame {
            layout: Layout::Single(form),
            framing: Framing::Encounter,
            state: State::FrozenBySight,
            t: 1.2,
            clock: 4.2,
            output: None,
        };
        hold(
            &mut frames,
            &frame,
            format!("{}5_{name}_encounter.png", n + 1),
        );
    }
    for (state, label) in [
        (State::Hunting, "hunting"),
        (State::FrozenBySight, "frozen"),
    ] {
        let t = still_time(state);
        let frame = Frame {
            layout: Layout::Lineup,
            framing: Framing::Lineup,
            state,
            t,
            clock: if state == State::Hunting { t } else { 3.0 + t },
            output: None,
        };
        hold(&mut frames, &frame, format!("40_lineup_{label}.png"));
    }
    let frame = Frame {
        layout: Layout::Ranks,
        framing: Framing::Ranks,
        state: State::Hunting,
        t: 2.3,
        clock: 2.3,
        output: None,
    };
    hold(&mut frames, &frame, "41_tumbler_ranks.png".into());
    for form in Form::MAJORS {
        for (film, timeline) in [("seen", SEEN), ("catch", CATCH)] {
            film_frames(
                &mut frames,
                form,
                &format!("film_{}_{film}", form.name()),
                timeline,
            );
        }
    }
    frames
}

fn film_frames(frames: &mut Vec<Frame>, form: Form, dir: &str, timeline: Timeline) {
    let first = Frame {
        layout: Layout::Single(form),
        framing: Framing::ThreeQuarter,
        state: timeline[0].0,
        t: 0.0,
        clock: 0.0,
        output: None,
    };
    for _ in 0..SETTLE {
        frames.push(first.clone());
    }
    let end = timeline[timeline.len() - 1].2;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = (end * f32::from(FPS)).round() as usize;
    for k in 0..count {
        #[allow(clippy::cast_precision_loss)]
        let clock = k as f32 / f32::from(FPS);
        let (state, from, _) = timeline
            .into_iter()
            .find(|&(_, from, until)| clock >= from && clock < until)
            .unwrap_or(timeline[timeline.len() - 1]);
        frames.push(Frame {
            layout: Layout::Single(form),
            framing: Framing::ThreeQuarter,
            state,
            t: clock - from,
            clock,
            output: Some(format!("{dir}/f_{k:04}.png")),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{CATCH, SEEN, plan};

    #[test]
    fn the_plan_writes_every_file_once() {
        let outputs: Vec<String> = plan().into_iter().filter_map(|f| f.output).collect();
        let mut unique = outputs.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), outputs.len());
        // Three forms, four states and an encounter each, two line-ups, the ranks.
        let stills = outputs.iter().filter(|o| !o.starts_with("film_")).count();
        assert_eq!(stills, 3 * 5 + 3);
    }

    #[test]
    fn each_film_is_one_unbroken_timeline() {
        for film in [SEEN, CATCH] {
            assert!((film[0].2 - film[1].1).abs() < 1e-6);
        }
    }
}
