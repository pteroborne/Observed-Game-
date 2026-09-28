//! Evidence for the Rogue's sensors: `OBSERVED2_CAPTURE_HEX_WFC_SENSORS=<dir>`.
//!
//! Launches a local Architect Ascent match as an Observer through the ordinary handoff and
//! finds a cell near the local body with an open step to the next: a sensor is installed
//! in that next cell, as the Rogue installs one (staged into the rules:
//! `AscentRules::stage_sensor`), and the body is stood a few metres off with a clear line
//! to it, so it hangs in view and sees the body. The rest is the match's own: the body walks up
//! under it and takes it down with interact.
//!
//! Frames and stills as the doors capture's (`doors::capture`): one frame a fixed tick.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_hex::{HexCoord, HexFace};
use player_input::PlayerIntent;

use crate::hex_wfc::sim::{HexWfcIntent, HexWfcRuntime};
use crate::hex_wfc::{HexWfcCapture, HexWfcCaptureMode};

const GIVE_UP_TICKS: u64 = 3_000;
/// How close the walk ends, metres across the floor from under the sensor: within reach.
const UNDER: f32 = 1.6;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Beat {
    #[default]
    Settle,
    Staged,
    Watching,
    Walk,
    Under,
    TakenDown,
    Done,
}

#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct SensorsCapture {
    beat: Beat,
    since: u64,
    sensor: Option<HexCoord>,
    interact: bool,
    still: Option<&'static str>,
    frames: u32,
}

impl SensorsCapture {
    fn go(&mut self, beat: Beat, tick: u64) {
        self.beat = beat;
        self.since = tick;
    }
}

/// Stage the scene: a sensor on a cell one open step from a cell near the body, and the
/// body stood in that cell with a clear line to where the sensor hangs, a few metres off.
/// Where the sensor goes.
fn stage_scene(runtime: &mut HexWfcRuntime) -> Option<HexCoord> {
    let local = runtime.local_player;
    let home = runtime.local().cell;
    let (scenes, lobby) = {
        let rules = runtime.ascent.as_ref()?.rules();
        let grid = rules.world.config.grid();
        let mut cells: Vec<HexCoord> = rules
            .world
            .placements
            .keys()
            .copied()
            .filter(|cell| cell.level == home.level)
            .collect();
        cells.sort_by_key(|cell| (observed_hex::travel_distance(*cell, home), *cell));
        let scenes: Vec<(HexCoord, HexCoord)> = cells
            .into_iter()
            .take(60)
            .flat_map(|cell| {
                HexFace::LATERAL
                    .into_iter()
                    .filter_map(move |face| grid.neighbor(cell, face).map(|next| (cell, next)))
            })
            .filter(|&(cell, next)| rules.exits(cell).contains(&next))
            .collect();
        (scenes, rules.prison.cells.clone())
    };
    for (cell, next) in scenes {
        if lobby.contains(&cell) || lobby.contains(&next) {
            continue;
        }
        let game = &mut runtime.match_state;
        // Where it would hang: hung for a moment to find out; the rules' own set replaces
        // it on the next step.
        game.set_sensors([next]);
        let Some((_, at)) = game.sensors().next() else {
            continue;
        };
        let Some(floor) = game.standing_point(cell) else {
            continue;
        };
        let away = (floor - at).with_y(0.0).normalize_or_zero();
        let stood = [6.5_f32, 5.0, 8.0].into_iter().any(|metres| {
            let feet = at.with_y(floor.y) + away * metres;
            game.stage_body_facing(local, cell, feet, at)
        });
        if stood {
            runtime.ascent.as_mut()?.stage_sensor(next);
            return Some(next);
        }
    }
    runtime.match_state.set_sensors([]);
    None
}

/// Turn the body where it stands to face `at`.
fn face(runtime: &mut HexWfcRuntime, at: Vec3) {
    let local = runtime.local_player;
    let game = &mut runtime.match_state;
    let (cell, centre) = {
        let body = &game.players[&local];
        (body.cell, body.position)
    };
    if let Some(floor) = game.standing_point(cell) {
        game.stage_body_facing(local, cell, centre.with_y(floor.y), at);
    }
}

/// Drive the scene. Runs each fixed tick before the match steps.
pub(in crate::hex_wfc) fn drive(
    capture: Option<Res<HexWfcCapture>>,
    sensors: Option<ResMut<SensorsCapture>>,
    runtime: Option<ResMut<HexWfcRuntime>>,
    intent: Option<ResMut<HexWfcIntent>>,
) {
    let (Some(capture), Some(mut sensors), Some(mut runtime), Some(mut intent)) =
        (capture, sensors, runtime, intent)
    else {
        return;
    };
    if capture.mode != HexWfcCaptureMode::Sensors {
        return;
    }
    let tick = runtime.match_state.tick;
    let elapsed = tick.saturating_sub(sensors.since);
    intent.intent = PlayerIntent::default();
    intent.actions.interact = std::mem::take(&mut sensors.interact);
    let mount = sensors.sensor.and_then(|cell| {
        runtime
            .match_state
            .sensors()
            .find(|&(hung, _)| hung == cell)
            .map(|(_, at)| at)
    });
    let body = runtime.local().position;
    match sensors.beat {
        Beat::Settle if tick >= 150 => {
            let Some(cell) = stage_scene(&mut runtime) else {
                error!("sensors capture: nowhere to hang a sensor in view");
                sensors.go(Beat::Done, tick);
                return;
            };
            info!("sensors capture: a sensor at {cell:?}");
            sensors.sensor = Some(cell);
            sensors.go(Beat::Staged, tick);
        }
        // Hung by the step after it was installed, where the body is already looking.
        Beat::Staged if elapsed >= 2 => {
            if mount.is_none() {
                error!("sensors capture: the sensor never hung");
                sensors.go(Beat::Done, tick);
                return;
            }
            sensors.go(Beat::Watching, tick);
        }
        Beat::Watching if elapsed == 40 => {
            sensors.still = Some("sensors-1-watching-1280x800.png");
        }
        Beat::Watching if elapsed >= 70 => sensors.go(Beat::Walk, tick),
        Beat::Walk => {
            let at = mount.expect("hung");
            if (at - body).with_y(0.0).length() > UNDER {
                intent.intent.movement = Vec2::new(0.0, 0.5);
            } else {
                face(&mut runtime, at);
                sensors.go(Beat::Under, tick);
            }
            if elapsed > 400 {
                error!("sensors capture: never reached the sensor");
                sensors.go(Beat::Done, tick);
            }
        }
        Beat::Under if elapsed == 15 => sensors.still = Some("sensors-2-prompt-1280x800.png"),
        Beat::Under if elapsed >= 30 => {
            sensors.interact = true;
            sensors.go(Beat::TakenDown, tick);
        }
        Beat::TakenDown if elapsed == 20 => {
            // This one: the bot Rogue installs its own elsewhere.
            let gone = runtime.ascent.as_ref().is_some_and(|ascent| {
                sensors
                    .sensor
                    .is_some_and(|cell| !ascent.rules().sensors.contains_key(&cell))
            });
            if !gone {
                error!("sensors capture: interact did not take the sensor down");
            }
            sensors.still = Some("sensors-3-taken-down-1280x800.png");
        }
        Beat::TakenDown if elapsed >= 60 => sensors.go(Beat::Done, tick),
        _ => {}
    }
    if tick > GIVE_UP_TICKS && sensors.beat != Beat::Done {
        error!("sensors capture gave up at {:?}", sensors.beat);
        sensors.go(Beat::Done, tick);
    }
}

/// Save this frame, and note any still the drive asked for; leave once done.
pub(in crate::hex_wfc) fn advance(
    request: &mut HexWfcCapture,
    sensors: Option<&mut SensorsCapture>,
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
    let Some(sensors) = sensors else {
        commands.init_resource::<SensorsCapture>();
        return;
    };
    let dir = std::path::Path::new(&request.path);
    match sensors.beat {
        Beat::Settle => return,
        Beat::Done => {
            request.stills = request.stills.saturating_add(1);
            if request.stills > 20 {
                info!("sensors capture complete: {} frames", sensors.frames);
                exit.write(AppExit::Success);
            }
            return;
        }
        _ => {}
    }
    let name = format!("frame_{:05}.png", sensors.frames);
    sensors.frames += 1;
    if let Some(still) = sensors.still.take() {
        use std::io::Write as _;
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("stills.txt"))
            .and_then(|mut file| writeln!(file, "{name} {still}"));
        if let Err(error) = written {
            error!("sensors capture: could not note still {still}: {error}");
        }
    }
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(dir.join("frames").join(name)));
}
