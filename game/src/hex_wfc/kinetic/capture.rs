//! Evidence for minors and the kinetic tool: `OBSERVED2_CAPTURE_HEX_WFC_MINORS=<dir>`.
//!
//! Launches a local Architect Ascent match as an Observer through the ordinary handoff,
//! finds a push that kills - the highest floor first, and proven by playing it on a copy
//! of the match (`HexWfcMatch::killing_push`) - and stages three minors there: one at the
//! edge the push sends it over, two more beside it. The local body is stood a few metres
//! behind the first, looking at it. Then everything is the match's own: the minors hunt
//! the body, the push is the body's own input, and the fall breaks the minor.
//!
//! Frames and stills as the power capture's (`power::capture`), but one frame a 1/60 s of
//! game time - a tick a frame, as in play, so presentation that reads a tick's events
//! (the notice a kill raises) sees every tick - for a 60 fps video.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_match::hex_wfc::{HexKillingPush, HexMatchEventKind};
use player_input::PlayerIntent;

use crate::hex_wfc::sim::{HexWfcIntent, HexWfcRuntime};
use crate::hex_wfc::{HexWfcCapture, HexWfcCaptureMode};

/// Game time a rendered frame advances while capturing: one fixed tick.
pub(in crate::hex_wfc) const FRAME_SECONDS: f32 = 1.0 / 60.0;
/// The minor the push is for, and the two beside it.
const FIRST: u16 = 900;
const OTHERS: [u16; 2] = [901, 902];
/// Replays `killing_push` may spend finding the edge.
const TRIES: usize = 60;
/// Give up after this many ticks rather than hang an unattended capture.
const GIVE_UP_TICKS: u64 = 3_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Beat {
    #[default]
    Settle,
    Staged,
    Pushed,
    Broken,
    Done,
}

#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct MinorsCapture {
    beat: Beat,
    since: u64,
    press: bool,
    still: Option<&'static str>,
    frames: u32,
}

impl MinorsCapture {
    fn go(&mut self, beat: Beat, tick: u64) {
        self.beat = beat;
        self.since = tick;
    }
}

/// Stand the local body behind `push`, looking at the minor it is for. Whether it could.
fn stage(runtime: &mut HexWfcRuntime, push: HexKillingPush) -> bool {
    let local = runtime.local_player;
    let game = &mut runtime.match_state;
    let aim = push.feet + Vec3::Y * 0.9;
    let placed = [4.2, 3.6, 3.0].into_iter().any(|back| {
        game.stage_body_facing(local, push.cell, push.feet - push.direction * back, aim)
    });
    // A little nearer the drop than the proven point: it walks at the body before the push.
    let edge_minor = [1.2, 0.6, 0.0]
        .into_iter()
        .map(|ahead| push.feet + push.direction * ahead)
        .find(|&feet| game.stands_clear(feet))
        .unwrap_or(push.feet);
    if !placed || !game.stage_minor(FIRST, push.cell, edge_minor) {
        return false;
    }
    // Two more further off on the same floor, in view ahead: coming, not yet arrived.
    let body = game.players[&local].position;
    let grid = game.facility.config.grid();
    let near: Vec<_> = std::iter::once(push.cell)
        .chain(
            observed_hex::HexFace::LATERAL
                .into_iter()
                .filter_map(|face| grid.neighbor(push.cell, face)),
        )
        .filter(|cell| {
            game.facility
                .placements
                .get(cell)
                .is_some_and(|p| p.space.built())
        })
        .collect();
    let mut ahead: Vec<(observed_hex::HexCoord, Vec3)> = near
        .into_iter()
        .flat_map(|cell| {
            game.standing_points(cell)
                .into_iter()
                .map(move |feet| (cell, feet))
        })
        .filter(|&(_, feet)| {
            let offset = (feet - body).with_y(0.0);
            (10.0..16.0).contains(&offset.length())
                && (feet.y - body.y).abs() < 2.5
                && offset.normalize().dot(push.direction) > 0.7
        })
        .collect();
    ahead.sort_by(|a, b| {
        a.1.distance(body)
            .total_cmp(&b.1.distance(body))
            .then(a.1.x.total_cmp(&b.1.x))
    });
    let mut placed_feet: Vec<Vec3> = Vec::new();
    for (cell, feet) in ahead {
        if placed_feet.len() == OTHERS.len() {
            break;
        }
        if placed_feet.iter().any(|other| other.distance(feet) < 3.0) {
            continue;
        }
        if game.stage_minor(OTHERS[placed_feet.len()], cell, feet) {
            placed_feet.push(feet);
        }
    }
    info!("minors capture: {} more minors ahead", placed_feet.len());
    true
}

/// Drive the scene: stage it, press push, and mark the stills. Runs each fixed tick
/// before the match steps.
pub(in crate::hex_wfc) fn drive(
    capture: Option<Res<HexWfcCapture>>,
    minors: Option<ResMut<MinorsCapture>>,
    runtime: Option<ResMut<HexWfcRuntime>>,
    intent: Option<ResMut<HexWfcIntent>>,
) {
    let (Some(capture), Some(mut minors), Some(mut runtime), Some(mut intent)) =
        (capture, minors, runtime, intent)
    else {
        return;
    };
    if capture.mode != HexWfcCaptureMode::Minors {
        return;
    }
    let tick = runtime.match_state.tick;
    let elapsed = tick.saturating_sub(minors.since);
    intent.intent = PlayerIntent::default();
    if std::mem::take(&mut minors.press) {
        intent.actions.kinetic_push = true;
    }
    let broke = runtime
        .match_state
        .recent_events
        .iter()
        .any(|event| event.kind == HexMatchEventKind::GuardianLost)
        && !runtime.match_state.released.contains_key(&FIRST);
    match minors.beat {
        Beat::Settle if tick >= 150 => {
            let Some(push) = runtime.match_state.killing_push(TRIES) else {
                error!("minors capture: no killing push within {TRIES} replays");
                minors.go(Beat::Done, tick);
                return;
            };
            info!("minors capture: pushing over the edge at {:?}", push.cell);
            if !stage(&mut runtime, push) {
                error!(
                    "minors capture: could not stage the scene at {:?}",
                    push.cell
                );
                minors.go(Beat::Done, tick);
                return;
            }
            minors.go(Beat::Staged, tick);
        }
        Beat::Staged if elapsed == 6 => minors.still = Some("minors-1-closing-1280x800.png"),
        Beat::Staged if elapsed >= 10 => {
            // It has come a step at the body: turn back onto it where it stands now.
            let local = runtime.local_player;
            let game = &mut runtime.match_state;
            let at = game.released.get(&FIRST).map(|minor| minor.position());
            let (cell, feet) = {
                let body = &game.players[&local];
                (body.cell, body.position)
            };
            // Its feet on the floor it stands on: the cell's floor height, where it is.
            let floor = game.standing_point(cell).map(|point| feet.with_y(point.y));
            if let (Some(at), Some(floor)) = (at, floor) {
                game.stage_body_facing(local, cell, floor, at);
            }
            minors.press = true;
            minors.go(Beat::Pushed, tick);
        }
        Beat::Pushed if elapsed == 14 => minors.still = Some("minors-2-pushed-1280x800.png"),
        Beat::Pushed if broke => minors.go(Beat::Broken, tick),
        Beat::Pushed if elapsed > 240 => {
            error!("minors capture: the pushed minor never broke");
            minors.go(Beat::Done, tick);
        }
        Beat::Broken if elapsed == 12 => minors.still = Some("minors-3-broken-1280x800.png"),
        Beat::Broken if elapsed >= 60 => minors.go(Beat::Done, tick),
        _ => {}
    }
    if tick > GIVE_UP_TICKS && minors.beat != Beat::Done {
        error!("minors capture gave up at {:?}", minors.beat);
        minors.go(Beat::Done, tick);
    }
}

/// Save this frame, and note any still the drive asked for; leave once done.
pub(in crate::hex_wfc) fn advance(
    request: &mut HexWfcCapture,
    minors: Option<&mut MinorsCapture>,
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
    let Some(minors) = minors else {
        commands.init_resource::<MinorsCapture>();
        return;
    };
    let dir = std::path::Path::new(&request.path);
    match minors.beat {
        Beat::Settle => return,
        Beat::Done => {
            request.stills = request.stills.saturating_add(1);
            if request.stills > 20 {
                info!("minors capture complete: {} frames", minors.frames);
                exit.write(AppExit::Success);
            }
            return;
        }
        _ => {}
    }
    let name = format!("frame_{:05}.png", minors.frames);
    minors.frames += 1;
    if let Some(still) = minors.still.take() {
        use std::io::Write as _;
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("stills.txt"))
            .and_then(|mut file| writeln!(file, "{name} {still}"));
        if let Err(error) = written {
            error!("minors capture: could not note still {still}: {error}");
        }
    }
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(dir.join("frames").join(name)));
}
