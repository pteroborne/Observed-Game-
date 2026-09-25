//! Opt-in evidence staging for readable first-person interaction prompts.
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_authoring::RoomSocketKind;

use crate::hex_wfc::{
    HexWfcCapture, HexWfcCaptureMode,
    sim::{HexWfcIntent, HexWfcRuntime},
};

pub(in crate::hex_wfc) fn drive(
    capture: Option<Res<HexWfcCapture>>,
    intent: Option<ResMut<HexWfcIntent>>,
) {
    let (Some(capture), Some(mut intent)) = (capture, intent) else {
        return;
    };
    if capture.mode != HexWfcCaptureMode::Hud {
        return;
    }
    if (80..95).contains(&capture.frame) || capture.frame >= 110 {
        intent.actions.interact = true;
        intent.intent.interact_held = true;
    }
}

pub(in crate::hex_wfc) fn advance(
    request: &mut HexWfcCapture,
    mut runtime: Option<&mut HexWfcRuntime>,
    commands: &mut Commands,
    exit: &mut MessageWriter<AppExit>,
) {
    let frame = request.frame;
    if frame == 12 {
        commands.queue(|world: &mut World| {
            let mut windows = world.query::<&mut Window>();
            for mut window in windows.iter_mut(world) {
                window.resolution.set(1280.0, 800.0);
            }
        });
    }
    if matches!(frame, 20 | 100)
        && let Some(runtime) = runtime.as_deref_mut()
    {
        if frame == 100 {
            request.last_shot_tick = runtime.match_state.tick;
        }
        let kind = if frame == 20 {
            RoomSocketKind::Keystone
        } else {
            RoomSocketKind::StationA
        };
        if let Some(socket) = runtime
            .match_state
            .geometry
            .sockets
            .iter()
            .find(|s| s.kind == kind)
            .cloned()
        {
            let id = runtime.local_player;
            if let Some(player) = runtime.match_state.players.get_mut(&id) {
                player.cell = socket.cell;
                player.position = socket.position + Vec3::new(0.0, 0.9, 1.8);
                player.yaw = 0.0;
                player.pitch = -0.2;
            }
        }
    }
    if frame == 100 {
        commands.queue(|world: &mut World| {
            world
                .resource_mut::<crate::settings::Settings>()
                .gameplay_text_scale = 1.25;
        });
    }
    let progress_ready = frame > 100
        && runtime
            .as_ref()
            .is_some_and(|runtime| runtime.match_state.tick >= request.last_shot_tick + 60);
    if frame == 75 || (progress_ready && request.stills == 0) {
        let name = if frame == 75 {
            "interaction-1280x800.png"
        } else {
            "station-large-text-1280x800.png"
        };
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::Path::new(&request.path).join(name)));
        if progress_ready {
            request.stills = 1;
        }
    }
    if request.stills == 1
        && runtime
            .as_ref()
            .is_some_and(|runtime| runtime.match_state.tick >= request.last_shot_tick + 75)
    {
        exit.write(AppExit::Success);
    }
}
