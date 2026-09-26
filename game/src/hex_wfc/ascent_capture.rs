//! Evidence for the prison: `OBSERVED2_CAPTURE_HEX_WFC_PRISON=<dir>`.
//!
//! Launches a local Architect Ascent match through the ordinary handoff, then stages one
//! thing: the local body is jailed, as a Guardian's catch would jail it. (The Guardian
//! catches whoever it is hunting, which is not reliably the local player; the catch
//! itself is covered by `observed_match`'s prison tests.) The maze, the walk out and the
//! release into the lobby are the match's own, and the body walks out on the game's own
//! bot. Once jailed, it asks its Architect for rescue as a player would. Three stills:
//! inside the maze (with the ask and the Architect's answer), at its way out, and back in
//! the facility looking at the lobby's gate, for which the body is stood a few metres
//! off and turned to it.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_match::hex_wfc::HexBodyPlace;

use crate::hex_wfc::{
    HexWfcCapture, HexWfcCaptureMode,
    sim::{HexWfcIntent, HexWfcRuntime},
};

/// Give up after this many frames rather than hang an unattended capture.
const GIVE_UP_FRAMES: u16 = 12_000;

/// The body is walked out of its maze by the game's own bot.
pub(in crate::hex_wfc) fn drive(
    capture: Option<Res<HexWfcCapture>>,
    runtime: Option<Res<HexWfcRuntime>>,
    intent: Option<ResMut<HexWfcIntent>>,
) {
    let (Some(capture), Some(runtime), Some(mut intent)) = (capture, runtime, intent) else {
        return;
    };
    if capture.mode != HexWfcCaptureMode::Prison || !matches!(capture.stills, 2 | 3) {
        return;
    }
    let local = runtime.local_player;
    if runtime.local().place == HexBodyPlace::Prison {
        intent.intent = runtime.match_state.bot_player_command(local).intent;
    }
}

pub(in crate::hex_wfc) fn advance(
    request: &mut HexWfcCapture,
    runtime: Option<&mut HexWfcRuntime>,
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
    if request.frame > GIVE_UP_FRAMES {
        error!("prison capture gave up at stage {}", request.stills);
        exit.write(AppExit::error());
        return;
    }
    let Some(runtime) = runtime else {
        return;
    };
    let tick = runtime.match_state.tick;
    let place = runtime.local().place;
    let mut shoot = |name: &str| {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::Path::new(&request.path).join(name)));
    };
    match request.stills {
        // Settle, then jail the local body.
        0 if request.frame >= 60 => {
            let local = runtime.local_player;
            runtime.match_state.jail(local);
            request.stills = 1;
            request.last_shot_tick = tick;
        }
        // Once the rules know the body is jailed, it asks its Architect for rescue, as
        // its player would, so the maze still shows the ask and the answer.
        1 if (request.last_shot_tick + 10..request.last_shot_tick + 90).contains(&tick)
            && runtime.ascent.as_ref().is_some_and(|ascent| {
                !ascent
                    .session()
                    .requests
                    .contains_key(&runtime.local_player)
            }) =>
        {
            commands.queue(|world: &mut World| {
                if let Some(mut ask) = world.get_resource_mut::<super::ask::AskTheArchitect>() {
                    ask.pending = true;
                }
            });
        }
        1 if tick >= request.last_shot_tick + 90 => {
            shoot("prison-maze-1280x800.png");
            request.stills = 2;
        }
        // Two halls from the way out, with its gate in view.
        2 if place == HexBodyPlace::Prison && halls_to_go(runtime) == Some(2) => {
            shoot("prison-way-out-1280x800.png");
            request.stills = 3;
        }
        3 if place == HexBodyPlace::Facility => {
            let prison = runtime.match_state.prison.as_ref().expect("a prison match");
            let anchor = prison.lobby_anchor;
            let gate = Vec3::from_array(observed_hex::hex_origin(anchor));
            // Stand in the hall next door, through a doorway, and look in at the gate.
            let facility = &runtime.match_state.facility;
            let next_door = observed_hex::HexFace::LATERAL
                .into_iter()
                .filter(|&face| facility.placements[&anchor].is_open(face))
                .find_map(|face| facility.config.grid().neighbor(anchor, face))
                .unwrap_or(anchor);
            let stand = Vec3::from_array(observed_hex::hex_origin(next_door));
            let local = runtime.local_player;
            let body = runtime
                .match_state
                .players
                .get_mut(&local)
                .expect("local body");
            let toward = gate - stand;
            body.cell = next_door;
            body.position = stand + Vec3::Y * body.position.y;
            body.yaw = toward.x.atan2(-toward.z);
            body.pitch = -0.1;
            request.stills = 4;
            request.last_shot_tick = tick;
        }
        4 if tick >= request.last_shot_tick + 45 => {
            shoot("prison-lobby-1280x800.png");
            request.stills = 5;
            request.last_shot_tick = tick;
        }
        5 if tick >= request.last_shot_tick + 30 => {
            info!("prison capture complete");
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

/// How many halls the local body's walk out of its maze has left, counting the one it is in.
fn halls_to_go(runtime: &HexWfcRuntime) -> Option<usize> {
    let local = runtime.local();
    let maze = runtime
        .match_state
        .prison
        .as_ref()?
        .mazes
        .get(&local.team)?;
    maze.world
        .route_between(local.cell, maze.exit())
        .map(|route| route.len())
}
