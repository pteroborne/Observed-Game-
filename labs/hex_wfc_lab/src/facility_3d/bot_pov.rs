//! Bot-POV walkthrough capture for the hex match layer (Arc L Phase 94).
//!
//! Drives the deterministic objective bot of an `observed_match` hex match on
//! the pinned Phase 94 gate seed — whose solved route crosses two ramp levels
//! and two shafts — and renders the run from the bot's eye, saving a numbered
//! frame sequence the CLAUDE.md FFmpeg recipe compiles into a loopable GIF.
//! Enabled with `OBSERVED2_HEX_BOT_POV=<dir>`.

use std::collections::BTreeMap;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_core::PlayerId;
use observed_facility::hex_wfc::HexWfcConfig;
use observed_match::hex_wfc::{
    HEX_INPUT_VERSION, HexInputFrame, HexMatchConfig, HexMatchStatus, HexWfcMatch,
};

use super::{CameraMode, FacilityCamera, FacilityState, FacilityStatus, LabViewMode};

/// The pinned gate seed from the Phase 94 headless gate: a 12×9×5 showcase whose
/// solved spawn→exit route crosses two ramp levels and two shafts.
const GATE_SEED: u64 = 0xa11c_0000_0000_0000;
/// Simulation steps advanced per rendered frame, keeping the GIF a readable
/// length (the full run is a few thousand 60 Hz ticks).
const STEPS_PER_FRAME: u64 = 10;
/// Frames rendered before stepping begins, letting the pipeline and camera warm.
const WARMUP_FRAMES: u32 = 8;
/// Hard cap on captured frames so an unexpected stall still terminates.
const MAX_FRAMES: u32 = 320;

pub(crate) fn gate_config() -> HexWfcConfig {
    HexWfcConfig {
        cols: 12,
        rows: 9,
        levels: 5,
        min_rooms: 4,
        max_rooms: 8,
        retry_budget: 100,
        min_room_distance: 2,
    }
}

#[derive(Resource)]
pub(crate) struct BotPovRun {
    dir: String,
    game: HexWfcMatch,
    frame: u32,
    shot: u32,
    warmed: bool,
    stepped_ticks: u64,
}

/// Build the run and re-point the rendered facility at the gate world so the bot
/// walks the exact geometry on screen.
pub(crate) fn setup(mut commands: Commands, mut facility: ResMut<FacilityState>) {
    let Ok(dir) = std::env::var("OBSERVED2_HEX_BOT_POV") else {
        return;
    };
    let game = HexWfcMatch::new(
        GATE_SEED,
        HexMatchConfig {
            teams: 1,
            members_per_team: 1,
            wfc: gate_config(),
        },
        &facility.prototypes,
    )
    .expect("Phase 94 gate match must build");
    facility.rebuild(&game.facility);
    facility.camera_mode = CameraMode::FreeFly;
    facility.overlay = false;
    place_camera_on_bot(&mut facility, &game);
    commands.insert_resource(BotPovRun {
        dir,
        game,
        frame: 0,
        shot: 0,
        warmed: false,
        stepped_ticks: 0,
    });
}

/// Advance the bot, follow it with the walk camera, and save a frame each render
/// tick until the bot escapes (or the cap trips).
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    run: Option<ResMut<BotPovRun>>,
    mut facility: ResMut<FacilityState>,
    mut mode: ResMut<LabViewMode>,
    mut plan_cameras: Query<&mut Camera, (With<Camera2d>, Without<FacilityCamera>)>,
    mut facility_cameras: Query<&mut Camera, With<FacilityCamera>>,
    mut status: Query<&mut Visibility, With<FacilityStatus>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut run) = run else {
        return;
    };
    if run.frame == 0 {
        std::fs::create_dir_all(&run.dir).expect("bot-POV capture dir must be creatable");
        // Switch the render to the 3D facility view from the bot's eye.
        *mode = LabViewMode::Facility3d;
        for mut camera in &mut plan_cameras {
            camera.is_active = false;
        }
        for mut camera in &mut facility_cameras {
            camera.is_active = true;
        }
        for mut visibility in &mut status {
            *visibility = Visibility::Hidden;
        }
    }
    run.frame += 1;
    if !run.warmed {
        if run.frame < WARMUP_FRAMES {
            return;
        }
        run.warmed = true;
    }

    // Save the frame the camera reached last tick, then advance the sim.
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "{}/bot_pov_{:03}.png",
            run.dir, run.shot
        )));
    run.shot += 1;

    let finished = run.game.status == HexMatchStatus::Finished;
    if finished || run.shot >= MAX_FRAMES {
        write_manifest(&run);
        exit.write(AppExit::Success);
        return;
    }

    for _ in 0..STEPS_PER_FRAME {
        if run.game.status == HexMatchStatus::Finished {
            break;
        }
        let command = run.game.bot_command(PlayerId(0));
        let tick = run.game.tick;
        let commands = BTreeMap::from([(
            PlayerId(0),
            observed_match::hex_wfc::HexPlayerCommand {
                intent: command,
                actions: Default::default(),
            },
        )]);
        run.game.step(&HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick,
            commands,
        });
        run.stepped_ticks = run.game.tick;
    }
    place_camera_on_bot(&mut facility, &run.game);
}

/// Point the camera at the bot from just over its shoulder and slightly above,
/// looking down its heading. Untextured flat-shaded geometry reads poorly from a
/// dead-ahead first-person eye (a wall or a ramp slab fills the frame as flat
/// colour); this chase framing keeps the bot's surroundings — hall pillars, the
/// ramp slope, the shaft's orange well and ledges — legible throughout the run.
fn place_camera_on_bot(facility: &mut FacilityState, game: &HexWfcMatch) {
    if let Some(player) = game.players.get(&PlayerId(0)) {
        let yaw = player.yaw;
        let forward = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
        let eye = player.position + Vec3::Y * 0.7;
        facility.camera_mode = CameraMode::FreeFly;
        facility.fly_position = eye + Vec3::Y * 1.6 - forward * 2.6;
        facility.fly_yaw = yaw;
        facility.fly_pitch = -0.26;
    }
}

fn write_manifest(run: &BotPovRun) {
    let manifest = serde_json::json!({
        "lab": "hex_wfc_lab",
        "capture": "bot_pov",
        "phase": 94,
        "seed": format!("{:#018x}", GATE_SEED),
        "grid_3d": [12, 9, 5],
        "frames": run.shot,
        "steps_per_frame": STEPS_PER_FRAME,
        "final_tick": run.stepped_ticks,
        "escaped": run.game.status == HexMatchStatus::Finished,
        "note": "objective bot spawn->exit; route crosses two ramp levels and two shafts",
    });
    std::fs::write(
        format!("{}/manifest.json", run.dir),
        serde_json::to_string_pretty(&manifest).expect("manifest serializes"),
    )
    .expect("bot-POV manifest writes");
}
