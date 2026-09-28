//! Evidence for doors: `OBSERVED2_CAPTURE_HEX_WFC_DOORS=<dir>`.
//!
//! Launches a local Architect Ascent match as an Observer through the ordinary handoff and
//! finds a doorway near the local body. The body is stood a few metres before it, facing
//! it, and a door is deployed there closed, as a door card deploys one (staged into the
//! rules: `AscentRules::stage_door`). The body walks up to it, and a minor is stood just
//! beyond it, waiting. The rest is the match's own: the body opens the door with interact,
//! the minor comes through for it, the body pushes it back through the doorway with the
//! kinetic tool, and closes the door on it again.
//!
//! Frames and stills as the minors capture's (`kinetic::capture`): one frame a fixed tick.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_match::hex_wfc::door_pose;
use player_input::PlayerIntent;

use crate::hex_wfc::sim::{HexWfcIntent, HexWfcRuntime};
use crate::hex_wfc::{HexWfcCapture, HexWfcCaptureMode};

const MINOR: u16 = 910;
const GIVE_UP_TICKS: u64 = 4_000;
/// How close the walk to the door ends, metres from its middle: within reach.
const AT_DOOR: f32 = 1.9;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Beat {
    #[default]
    Settle,
    Staged,
    Holding,
    ToDoor,
    AtDoor,
    Opened,
    Pushed,
    Closed,
    Done,
}

#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct DoorsCapture {
    beat: Beat,
    since: u64,
    door: Option<(HexCoord, HexFace)>,
    /// Where the minor waits beyond the door.
    minor: Option<(HexCoord, Vec3)>,
    interact: bool,
    push: bool,
    still: Option<&'static str>,
    frames: u32,
}

impl DoorsCapture {
    fn go(&mut self, beat: Beat, tick: u64) {
        self.beat = beat;
        self.since = tick;
    }
}

/// A doorway a door can stand in, and where the body and the minor stand either side of it.
struct Doorway {
    key: (HexCoord, HexFace),
    body: Vec3,
    minor_cell: HexCoord,
    minor: Vec3,
}

/// A doorway near the local body, with floor either side for the scene.
fn find_doorway(runtime: &HexWfcRuntime) -> Option<Doorway> {
    let game = &runtime.match_state;
    let home = runtime.local().cell;
    let grid = game.facility.config.grid();
    let mut cells: Vec<HexCoord> = game
        .facility
        .placements
        .keys()
        .copied()
        .filter(|cell| cell.level == home.level)
        .collect();
    cells.sort_by_key(|cell| (observed_hex::travel_distance(*cell, home), *cell));
    let same_room = |a: HexCoord, b: HexCoord| {
        game.facility
            .blueprints
            .iter()
            .any(|room| room.cells.contains(&a) && room.cells.contains(&b))
    };
    let lobby = game
        .prison
        .as_ref()
        .map(|prison| prison.lobby.clone())
        .unwrap_or_default();
    // Away from every other body, so the minor beyond the door has only this one to hunt.
    let others: Vec<HexCoord> = game
        .players
        .values()
        .filter(|player| player.id != runtime.local_player)
        .map(|player| player.cell)
        .collect();
    let alone = |cell: HexCoord| {
        others.iter().all(|&other| {
            other.level != cell.level || observed_hex::travel_distance(cell, other) >= 4
        })
    };
    for cell in cells.into_iter().take(160) {
        let placement = game.facility.placements[&cell];
        if !placement.space.built() || lobby.contains(&cell) || !alone(cell) {
            continue;
        }
        for face in HexFace::LATERAL {
            let Some(next) = grid.neighbor(cell, face) else {
                continue;
            };
            let opens = placement.is_open(face)
                && game
                    .facility
                    .placements
                    .get(&next)
                    .is_some_and(|other| other.space.built() && other.is_open(face.opposite()));
            if !opens || same_room(cell, next) || lobby.contains(&next) || !alone(next) {
                continue;
            }
            let (floor, _) = door_pose(cell, face);
            let across = (Vec3::from_array(hex_origin(next)) - Vec3::from_array(hex_origin(cell)))
                .with_y(0.0)
                .normalize();
            let body = floor - across * 4.5;
            let minor = floor + across * 2.5;
            let walk_clear =
                (1..=8).all(|step| game.stands_clear(floor - across * (0.5 * step as f32)));
            // A body fits in the doorway itself: not an opening split by a column.
            let through = game.stands_clear(floor)
                && game.stands_clear(floor + across * 1.0)
                && game.stands_clear(floor - across * 1.0);
            if walk_clear && through && game.stands_clear(body) && game.stands_clear(minor) {
                return Some(Doorway {
                    key: (cell, face),
                    body,
                    minor_cell: next,
                    minor,
                });
            }
        }
    }
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
    doors: Option<ResMut<DoorsCapture>>,
    runtime: Option<ResMut<HexWfcRuntime>>,
    intent: Option<ResMut<HexWfcIntent>>,
) {
    let (Some(capture), Some(mut doors), Some(mut runtime), Some(mut intent)) =
        (capture, doors, runtime, intent)
    else {
        return;
    };
    if capture.mode != HexWfcCaptureMode::Doors {
        return;
    }
    let tick = runtime.match_state.tick;
    let elapsed = tick.saturating_sub(doors.since);
    intent.intent = PlayerIntent::default();
    intent.actions.interact = std::mem::take(&mut doors.interact);
    intent.actions.kinetic_push = std::mem::take(&mut doors.push);
    let local = runtime.local_player;
    let door_floor = doors.door.map(|(cell, face)| door_pose(cell, face).0);
    let minor_at = runtime
        .match_state
        .released
        .get(&MINOR)
        .map(|minor| minor.position());
    let body = runtime.local().position;
    match doors.beat {
        Beat::Settle if tick >= 150 => {
            let Some(Doorway {
                key,
                body: stand,
                minor_cell,
                minor: minor_feet,
            }) = find_doorway(&runtime)
            else {
                error!("doors capture: no doorway near the body");
                doors.go(Beat::Done, tick);
                return;
            };
            let (floor, _) = door_pose(key.0, key.1);
            let game = &mut runtime.match_state;
            if !game.stage_body_facing(local, key.0, stand, floor + Vec3::Y * 1.5) {
                error!("doors capture: could not stage the scene at {key:?}");
                doors.go(Beat::Done, tick);
                return;
            }
            info!("doors capture: a door at {key:?}");
            doors.door = Some(key);
            doors.minor = Some((minor_cell, minor_feet));
            doors.go(Beat::Staged, tick);
        }
        Beat::Staged if elapsed == 10 => {
            let (cell, face) = doors.door.expect("staged");
            if let Some(ascent) = runtime.ascent.as_mut() {
                ascent.stage_door(cell, face, true);
            }
            doors.go(Beat::Holding, tick);
        }
        Beat::Holding if elapsed == 40 => doors.still = Some("doors-1-deployed-1280x800.png"),
        Beat::Holding if elapsed >= 70 => doors.go(Beat::ToDoor, tick),
        Beat::ToDoor => {
            let floor = door_floor.expect("staged");
            if (floor - body).with_y(0.0).length() > AT_DOOR {
                intent.intent.movement = Vec2::new(0.0, 0.5);
            } else {
                doors.go(Beat::AtDoor, tick);
            }
            if elapsed > 300 {
                error!("doors capture: never reached the door");
                doors.go(Beat::Done, tick);
            }
        }
        Beat::AtDoor if elapsed == 15 => doors.still = Some("doors-2-prompt-1280x800.png"),
        Beat::AtDoor if elapsed == 20 => {
            let (cell, feet) = doors.minor.expect("staged");
            if !runtime.match_state.stage_minor(MINOR, cell, feet) {
                error!("doors capture: could not stand the minor beyond the door");
                doors.go(Beat::Done, tick);
            }
        }
        Beat::AtDoor if elapsed >= 30 => {
            doors.interact = true;
            doors.go(Beat::Opened, tick);
        }
        Beat::Opened => {
            if elapsed == 18 {
                doors.still = Some("doors-3-opened-1280x800.png");
            }
            // It comes through: turn onto it and push it back.
            if let Some(at) = minor_at {
                if elapsed >= 10 {
                    face(&mut runtime, at);
                }
                if elapsed >= 22 && (at - body).with_y(0.0).length() < 3.5 {
                    doors.push = true;
                    doors.go(Beat::Pushed, tick);
                }
            }
            if elapsed > 240 {
                error!("doors capture: the minor never came through");
                doors.go(Beat::Done, tick);
            }
        }
        Beat::Pushed if elapsed == 8 => doors.still = Some("doors-4-pushed-1280x800.png"),
        Beat::Pushed if elapsed >= 22 => {
            face(&mut runtime, door_floor.expect("staged") + Vec3::Y * 1.5);
            doors.interact = true;
            doors.go(Beat::Closed, tick);
        }
        Beat::Closed if elapsed == 40 => doors.still = Some("doors-5-closed-1280x800.png"),
        Beat::Closed if elapsed >= 90 => doors.go(Beat::Done, tick),
        _ => {}
    }
    if tick > GIVE_UP_TICKS && doors.beat != Beat::Done {
        error!("doors capture gave up at {:?}", doors.beat);
        doors.go(Beat::Done, tick);
    }
}

/// Save this frame, and note any still the drive asked for; leave once done.
pub(in crate::hex_wfc) fn advance(
    request: &mut HexWfcCapture,
    doors: Option<&mut DoorsCapture>,
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
    let Some(doors) = doors else {
        commands.init_resource::<DoorsCapture>();
        return;
    };
    let dir = std::path::Path::new(&request.path);
    match doors.beat {
        Beat::Settle => return,
        Beat::Done => {
            request.stills = request.stills.saturating_add(1);
            if request.stills > 20 {
                info!("doors capture complete: {} frames", doors.frames);
                exit.write(AppExit::Success);
            }
            return;
        }
        _ => {}
    }
    let name = format!("frame_{:05}.png", doors.frames);
    doors.frames += 1;
    if let Some(still) = doors.still.take() {
        use std::io::Write as _;
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("stills.txt"))
            .and_then(|mut file| writeln!(file, "{name} {still}"));
        if let Err(error) = written {
            error!("doors capture: could not note still {still}: {error}");
        }
    }
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(dir.join("frames").join(name)));
}
