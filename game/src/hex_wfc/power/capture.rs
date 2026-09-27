//! Evidence for floor power: `OBSERVED2_CAPTURE_HEX_WFC_POWER=<dir>`.
//!
//! Launches a local Architect Ascent match as an Observer through the ordinary handoff,
//! then stages two scenes and plays each through the match's own input. The body is stood
//! a few metres from the ground floor's generator, facing it, and walks up; interact cuts
//! the floor's power and, a few seconds later, restores it. Then it is stood before the
//! floor's recharge station with its tool nearly empty, walks into the cradle and stands
//! there until the tool is full; last, the floor's power is staged off, to show a dead
//! station. The walk, the interact presses and the recharge are all the match's own.
//!
//! Every rendered frame is saved as `frames/frame_NNNNN.png`, one per 1/30 s of game time
//! (the capture advances time by exactly that much a frame, whatever the renderer
//! manages), for an MP4 at 30 fps. `stills.txt` names the frame that is each beat's still.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_match::ascent::economy::MAX_CHARGE;
use observed_match::ascent::facility::{Fixture, FixtureKind};
use player_input::PlayerIntent;

use crate::hex_wfc::sim::{HexWfcIntent, HexWfcRuntime};
use crate::hex_wfc::{HexWfcCapture, HexWfcCaptureMode};

/// Game time a rendered frame advances while capturing: the video's frame rate.
pub(in crate::hex_wfc) const FRAME_SECONDS: f32 = 1.0 / 30.0;
/// Give up after this many ticks rather than hang an unattended capture.
const GIVE_UP_TICKS: u64 = 6_000;
/// How close the walk to a fixture ends, metres across the floor.
const ARRIVE_GENERATOR: f32 = 2.05;
const ARRIVE_STATION: f32 = 1.6;
/// Charge the body starts the station scene with.
const NEARLY_EMPTY: u32 = 10;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Beat {
    #[default]
    Settle,
    ToGenerator,
    AtGenerator,
    Cut,
    Restored,
    ToStation,
    Recharging,
    Charged,
    Dead,
    Done,
}

/// Where the capture is, and what it asks of the next frame.
#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct PowerCapture {
    beat: Beat,
    /// The tick the beat began.
    since: u64,
    /// The fixture the body is walking to, and how close it stops.
    walking_to: Option<(Vec3, f32)>,
    /// Press interact on the next tick.
    press: bool,
    /// A named still for the next rendered frame.
    still: Option<&'static str>,
    /// Frames saved so far.
    frames: u32,
}

impl PowerCapture {
    fn go(&mut self, beat: Beat, tick: u64) {
        self.beat = beat;
        self.since = tick;
    }
}

fn fixture(runtime: &HexWfcRuntime, kind: FixtureKind) -> Option<Fixture> {
    let level = runtime.local().cell.level;
    runtime
        .ascent
        .as_ref()?
        .fixtures()
        .iter()
        .copied()
        .find(|fixture| fixture.kind == kind && fixture.cell.level == level)
}

/// Stand the local body a few metres from `fixture` with a clear walk to it, facing it.
fn stand_before(runtime: &mut HexWfcRuntime, fixture: Fixture) -> bool {
    let local = runtime.local_player;
    let at = fixture.floor + Vec3::Y * 0.9;
    for distance in [5.0, 4.0, 3.2] {
        for step in 0..16u8 {
            let angle = f32::from(step) * std::f32::consts::TAU / 16.0;
            let feet = fixture.floor + Vec3::new(angle.cos(), 0.0, angle.sin()) * distance;
            if runtime
                .match_state
                .stage_body_facing(local, fixture.cell, feet, at)
            {
                return true;
            }
        }
    }
    false
}

/// Drive the local body: walk to the fixture, press interact when asked. Runs each fixed
/// tick before the match steps.
pub(in crate::hex_wfc) fn drive(
    capture: Option<Res<HexWfcCapture>>,
    power: Option<ResMut<PowerCapture>>,
    runtime: Option<ResMut<HexWfcRuntime>>,
    intent: Option<ResMut<HexWfcIntent>>,
) {
    let (Some(capture), Some(mut power), Some(mut runtime), Some(mut intent)) =
        (capture, power, runtime, intent)
    else {
        return;
    };
    if capture.mode != HexWfcCaptureMode::Power {
        return;
    }
    let tick = runtime.match_state.tick;
    let elapsed = tick.saturating_sub(power.since);
    intent.intent = PlayerIntent::default();
    if std::mem::take(&mut power.press) {
        intent.actions.interact = true;
    }
    if let Some((target, arrive)) = power.walking_to {
        let body = runtime.local().position;
        let across = (target - body).with_y(0.0);
        if across.length() > arrive {
            intent.intent.movement = Vec2::new(0.0, 0.6);
        } else {
            power.walking_to = None;
        }
    }
    let walking = power.walking_to.is_some();
    let charge = || {
        let ascent = runtime.ascent.as_ref()?;
        let observer = ascent.observer_for(runtime.local_player)?;
        Some(ascent.rules().economy.charge(observer))
    };
    match power.beat {
        Beat::Settle if tick >= 150 => {
            let Some(generator) = fixture(&runtime, FixtureKind::Generator) else {
                error!("power capture: no generator on the spawn floor");
                power.go(Beat::Done, tick);
                return;
            };
            if !stand_before(&mut runtime, generator) {
                error!("power capture: nowhere to stand before the generator");
                power.go(Beat::Done, tick);
                return;
            }
            power.walking_to = Some((generator.floor, ARRIVE_GENERATOR));
            power.still = Some("power-1-generator-1280x800.png");
            power.go(Beat::ToGenerator, tick);
        }
        Beat::ToGenerator if elapsed > 30 && !walking => power.go(Beat::AtGenerator, tick),
        Beat::AtGenerator if elapsed == 50 => {
            power.still = Some("power-2-generator-prompt-1280x800.png");
        }
        Beat::AtGenerator if elapsed >= 80 => {
            power.press = true;
            power.go(Beat::Cut, tick);
        }
        Beat::Cut if elapsed == 90 => {
            power.still = Some("power-3-generator-cut-1280x800.png");
        }
        Beat::Cut if elapsed >= 240 => {
            power.press = true;
            power.go(Beat::Restored, tick);
        }
        Beat::Restored if elapsed == 60 => {
            power.still = Some("power-4-generator-restored-1280x800.png");
        }
        Beat::Restored if elapsed >= 120 => {
            let Some(station) = fixture(&runtime, FixtureKind::Station) else {
                error!("power capture: no station on the spawn floor");
                power.go(Beat::Done, tick);
                return;
            };
            if !stand_before(&mut runtime, station) {
                error!("power capture: nowhere to stand before the station");
                power.go(Beat::Done, tick);
                return;
            }
            let local = runtime.local_player;
            if let Some(ascent) = runtime.ascent.as_mut() {
                ascent.stage_charge(local, NEARLY_EMPTY);
            }
            power.walking_to = Some((station.floor, ARRIVE_STATION));
            power.still = Some("power-5-station-1280x800.png");
            power.go(Beat::ToStation, tick);
        }
        Beat::ToStation if elapsed > 30 && !walking => power.go(Beat::Recharging, tick),
        Beat::Recharging if elapsed == 100 => {
            power.still = Some("power-6-station-recharging-1280x800.png");
        }
        Beat::Recharging if charge() == Some(MAX_CHARGE) => power.go(Beat::Charged, tick),
        Beat::Charged if elapsed == 30 => {
            power.still = Some("power-7-station-charged-1280x800.png");
        }
        Beat::Charged if elapsed >= 90 => {
            let level = runtime.local().cell.level;
            if let Some(ascent) = runtime.ascent.as_mut() {
                ascent.stage_power(level, false);
            }
            power.go(Beat::Dead, tick);
        }
        Beat::Dead if elapsed == 90 => {
            power.still = Some("power-8-station-dead-1280x800.png");
        }
        Beat::Dead if elapsed >= 150 => power.go(Beat::Done, tick),
        _ => {}
    }
    if tick > GIVE_UP_TICKS && power.beat != Beat::Done {
        error!("power capture gave up at {:?}", power.beat);
        power.go(Beat::Done, tick);
    }
}

/// Save this frame, and any still the drive asked for; leave once done.
pub(in crate::hex_wfc) fn advance(
    request: &mut HexWfcCapture,
    power: Option<&mut PowerCapture>,
    commands: &mut Commands,
    exit: &mut MessageWriter<AppExit>,
) {
    if request.frame == 12 {
        commands.queue(|world: &mut World| {
            let mut windows = world.query::<&mut Window>();
            for mut window in windows.iter_mut(world) {
                window.resolution.set(1280.0, 800.0);
            }
        });
    }
    let Some(power) = power else {
        commands.init_resource::<PowerCapture>();
        return;
    };
    let dir = std::path::Path::new(&request.path);
    if power.beat == Beat::Done {
        // A few frames for the last saves to land.
        request.stills = request.stills.saturating_add(1);
        if request.stills > 20 {
            info!("power capture complete: {} frames", power.frames);
            exit.write(AppExit::Success);
        }
        return;
    }
    if power.beat == Beat::Settle {
        return;
    }
    // One screenshot entity a frame, so the saves never race on the window. A named still
    // is this frame: `stills.txt` names which, for copying out once the run is done.
    let name = format!("frame_{:05}.png", power.frames);
    power.frames += 1;
    if let Some(still) = power.still.take() {
        use std::io::Write as _;
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("stills.txt"))
            .and_then(|mut file| writeln!(file, "{name} {still}"));
        if let Err(error) = written {
            error!("power capture: could not note still {still}: {error}");
        }
    }
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(dir.join("frames").join(name)));
}
