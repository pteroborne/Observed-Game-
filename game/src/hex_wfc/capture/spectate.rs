//! A spectated Ascent match as video: the Observer seat, every body a bot, watched
//! one of two ways ([`Watch`]).
//!
//! **The overview** (`view::spectate`) follows one body, and a body chosen at random
//! spends most of a match on the floor it started on. So it directs: whenever
//! another body stands on a higher storey than the one being followed, the view
//! cuts to it - the same cut the spectator's own focus key makes - and the video is
//! the climb, floor by floor and district by district, as far as any body gets.
//!
//! **Through the eyes** is the opposite choice. A cut in first person throws the
//! viewer into another body mid-stride, and cutting at every new storey would cut
//! exactly as the climb it was waiting for finished. So the eyes stay with one
//! body, its climbs and its catch, and change only when that body leaves the
//! facility.
//!
//! Every rendered frame advances a fixed slice of game time ([`Watch::frame_seconds`])
//! and is one 30 fps video frame, saved as `frames/frame_NNNNN.png`. Encode with:
//!
//! ```text
//! ffmpeg -framerate 30 -i <dir>/frames/frame_%05d.png -c:v libx264 -pix_fmt yuv420p \
//!     -crf 20 -movflags +faststart spectate.mp4
//! ```

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use super::{HexWfcCapture, sim};

/// How the match is watched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::hex_wfc) enum Watch {
    /// The isometric cutaway, following the highest body.
    Overview,
    /// First person, through one body's eyes until it is caught.
    Eyes,
}

impl Watch {
    /// Game time per video frame. The overview is a 6x time-lapse, since a climb
    /// takes minutes; Bevy clamps a frame's virtual time at 250 ms, so 8x would
    /// quietly play at 7.5x. First person is 2x: faster is unwatchable from inside
    /// a head, and a whole match in real time is more frames than the scratch disk.
    pub(in crate::hex_wfc) fn frame_seconds(self) -> f32 {
        match self {
            Self::Overview => 6.0 / 30.0,
            Self::Eyes => 2.0 / 30.0,
        }
    }

    /// The most video frames recorded. A match usually ends sooner, and the
    /// recording with it: about 25 s of overview, 75 s of eyes.
    fn frames(self) -> u16 {
        match self {
            Self::Overview => 1_800,
            Self::Eyes => 3_600,
        }
    }
}

/// Frames to let the facility load and the view settle before recording.
const WARMUP: u16 = 30;

/// Frames the clock may stand still before the match is taken to be over: a beat
/// held on the last picture, and out before the results screen takes the window.
const HOLD: u16 = 20;

/// The capture's running state, since `HexWfcCapture` is shared by every mode.
#[derive(Default)]
pub(super) struct Clock {
    last_tick: u64,
    still_for: u16,
}

pub(super) fn advance(
    watch: Watch,
    request: &mut HexWfcCapture,
    clock: &mut Clock,
    runtime: Option<&mut sim::HexWfcRuntime>,
    commands: &mut Commands,
    exit: &mut MessageWriter<AppExit>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    match watch {
        Watch::Overview => follow_the_highest(runtime),
        Watch::Eyes => follow_until_caught(runtime),
    }
    let Some(index) = request.frame.checked_sub(WARMUP) else {
        return;
    };
    // A finished match stops its clock and then leaves this state for the results,
    // where this system no longer runs - so the end is heard here, while it can be.
    let tick = runtime.match_state.tick;
    clock.still_for = if tick == clock.last_tick {
        clock.still_for + 1
    } else {
        0
    };
    clock.last_tick = tick;
    if index >= watch.frames() || clock.still_for >= HOLD {
        info!("spectate capture: {index} frames, ending at tick {tick}");
        exit.write(AppExit::Success);
        return;
    }
    if index % 30 == 0 {
        let levels: Vec<_> = runtime
            .match_state
            .players
            .values()
            .map(|player| player.cell.level)
            .collect();
        info!(
            "spectate capture: tick {} following {:?} on level {}, every body {levels:?}",
            runtime.match_state.tick,
            runtime.local_player,
            runtime.local().cell.level,
        );
    }
    let path = std::path::Path::new(&request.path)
        .join("frames")
        .join(format!("frame_{index:05}.png"));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

/// Cut to a body standing higher than the one being followed.
///
/// Strictly higher, so two bodies on one floor never trade the view back and forth;
/// and only bodies still in the facility, so the view never cuts to a cell.
fn follow_the_highest(runtime: &mut sim::HexWfcRuntime) {
    let here = runtime.local();
    let held = here.in_facility().then_some(here.cell.level);
    let higher = runtime
        .match_state
        .players
        .values()
        .filter(|player| player.in_facility())
        .filter(|player| held.is_none_or(|level| player.cell.level > level))
        .max_by_key(|player| player.cell.level)
        .map(|player| player.id);
    if let Some(id) = higher {
        runtime.local_player = id;
    }
}

/// Stay with the body being followed until it leaves the facility, then take the
/// highest one still in it.
fn follow_until_caught(runtime: &mut sim::HexWfcRuntime) {
    if !runtime.local().in_facility() {
        follow_the_highest(runtime);
    }
}
