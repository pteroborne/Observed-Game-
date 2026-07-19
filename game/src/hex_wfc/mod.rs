//! Canonical hex-facility match adapter (Arc L Phase 95). Pure authoritative simulation
//! lives in `observed_match::hex_wfc`; this module owns only Bevy input, presentation,
//! feedback, replay, and screen lifecycle integration. It parallels `full_wfc/` in
//! structure and fidelity, driven by [`observed_match::hex_wfc::HexWfcMatch`].

mod audio;
mod cues;
mod entities;
mod feedback;
mod hud;
mod input;
mod lantern;
mod perf;
pub mod sim;
mod tacmap;
mod view;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::GameState;

pub(crate) struct HexWfcPlugin;

impl Plugin for HexWfcPlugin {
    fn build(&self, app: &mut App) {
        perf::configure(app);
        app.add_systems(
            OnEnter(GameState::HexWfc),
            (
                sim::setup_runtime,
                view::setup_view,
                hud::setup,
                tacmap::setup,
                feedback::setup,
                audio::setup,
                entities::setup,
                lantern::setup,
                input::grab_cursor,
            )
                .chain(),
        )
        .add_systems(
            FixedUpdate,
            (
                perf::begin_fixed,
                drive_traversal_capture,
                sim::step_runtime,
                perf::end_fixed,
            )
                .chain()
                .run_if(in_state(GameState::HexWfc)),
        )
        .add_systems(
            Update,
            (
                input::map_input,
                input::mode_hotkeys,
                view::sync_changed_geometry,
                view::sync_streamed_cells,
                view::sync_camera,
                view::sync_lighting_and_atmosphere,
                hud::sync,
                tacmap::sync,
                feedback::sync,
                audio::sync,
                entities::sync,
                lantern::sync_projection,
                lantern::sync_dynamic,
                sim::finish_runtime,
            )
                .chain()
                .run_if(in_state(GameState::HexWfc)),
        )
        .add_systems(
            OnExit(GameState::HexWfc),
            (
                input::release_cursor,
                view::clear_view,
                tacmap::cleanup,
                feedback::cleanup,
                audio::cleanup,
                entities::cleanup,
                lantern::cleanup,
                sim::cleanup_runtime,
            )
                .chain(),
        );
        let capture = std::env::var("OBSERVED2_CAPTURE_HEX_WFC_STYLE")
            .map(|path| (path, HexWfcCaptureMode::Style))
            .or_else(|_| {
                std::env::var("OBSERVED2_CAPTURE_HEX_WFC_RELAYOUT")
                    .map(|path| (path, HexWfcCaptureMode::Relayout))
            })
            .or_else(|_| {
                std::env::var("OBSERVED2_CAPTURE_HEX_WFC_TRAVERSAL")
                    .map(|path| (path, HexWfcCaptureMode::Traversal))
            })
            .or_else(|_| {
                std::env::var("OBSERVED2_CAPTURE_HEX_WFC")
                    .map(|path| (path, HexWfcCaptureMode::Gameplay))
            })
            .or_else(|_| {
                std::env::var("OBSERVED2_CAPTURE_HEX_WFC_MAP")
                    .map(|path| (path, HexWfcCaptureMode::Map))
            });
        if let Ok((path, mode)) = capture {
            if matches!(
                mode,
                HexWfcCaptureMode::Style
                    | HexWfcCaptureMode::Relayout
                    | HexWfcCaptureMode::Traversal
            ) {
                std::fs::create_dir_all(&path)
                    .expect("hex-WFC directory-style capture directory must be creatable");
            }
            app.insert_resource(HexWfcCapture {
                path,
                frame: 0,
                mode,
                last_shot_tick: 0,
                stills: 0,
            })
            .add_systems(Startup, autostart_capture)
            .add_systems(
                Update,
                capture_progress
                    .after(view::sync_lighting_and_atmosphere)
                    .after(sync_traversal_capture_camera)
                    .run_if(in_state(GameState::HexWfc)),
            )
            .add_systems(
                Update,
                sync_traversal_capture_camera
                    .after(view::sync_camera)
                    .run_if(in_state(GameState::HexWfc)),
            );
        }
    }
}

#[derive(Resource)]
pub(super) struct HexWfcCapture {
    path: String,
    frame: u16,
    mode: HexWfcCaptureMode,
    /// Relayout mode only: the last simulation tick at which a GIF frame was captured.
    last_shot_tick: u64,
    /// Relayout mode only: bitset of which labeled stills (before / warning / after)
    /// have been taken.
    stills: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HexWfcCaptureMode {
    Gameplay,
    Map,
    Style,
    /// The arc headline: a mid-match observation-safe relayout captured before, during
    /// the warning window, and after the committed re-collapse. Forces the pinned seed
    /// and the showcase (12×9×4) config so the deterministic warning@575 / commit@695
    /// timeline (proven by `observed_relayout_commits_mid_match_deterministically`)
    /// reproduces on the capture path.
    Relayout,
    /// Pinned showcase route with two ramp rises plus a staged, real shaft descent.
    /// This is evidence-only automation: every frame still advances the authoritative
    /// match through `PlayerIntent` and the production controller.
    Traversal,
}

/// How many screenshots the style montage takes before exiting; matched to a spectated
/// walkthrough long enough to descend several levels.
const STYLE_SHOTS: u16 = 8;
const STYLE_STRIDE: u16 = 45;

/// The pinned seed whose showcase-config relayout commits deterministically at tick 695
/// (generation 0 → 1). Shared with the match-layer determinism test.
const RELAYOUT_CAPTURE_SEED: u64 = 0xA11C_9500_0000_0000;
/// Phase 94's route gate: two ramp transitions and two shaft transitions on a compact
/// five-level facility. Reused here so the integration evidence is reproducible.
const TRAVERSAL_CAPTURE_SEED: u64 = 0xa11c_0000_0000_0000;

fn autostart_capture(
    mut commands: Commands,
    capture: Res<HexWfcCapture>,
    mut next: ResMut<NextState<GameState>>,
) {
    // The relayout capture must reproduce the pinned deterministic timeline, so it pins
    // the seed; `sim::runtime_config` pairs it with the showcase config when this env is
    // set. Other modes keep the default production seed + 28×20×10 facility.
    match capture.mode {
        HexWfcCaptureMode::Relayout => {
            commands.insert_resource(crate::flow::ActiveMatchSeed(RELAYOUT_CAPTURE_SEED));
        }
        HexWfcCaptureMode::Traversal => {
            commands.insert_resource(crate::flow::ActiveMatchSeed(TRAVERSAL_CAPTURE_SEED));
        }
        _ => {}
    }
    // Capture always runs autonomously: the objective bot drives the runner so evidence
    // shows real traversal (ramps, shafts, the exit) rather than a frozen spawn.
    if capture.mode != HexWfcCaptureMode::Traversal {
        commands.insert_resource(crate::sim::state::SpectatorBot::for_seed(
            crate::flow::MATCH_SEED,
        ));
    }
    next.set(GameState::HexWfc);
}

fn capture_progress(
    mut request: ResMut<HexWfcCapture>,
    mut runtime: Option<ResMut<sim::HexWfcRuntime>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    request.frame = request.frame.saturating_add(1);
    match request.mode {
        HexWfcCaptureMode::Map => {
            if request.frame == 900
                && let Some(runtime) = runtime.as_deref_mut()
            {
                runtime.map_open = true;
            }
            if request.frame == 950 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(request.path.clone()));
            } else if request.frame == 1_020 {
                exit.write(AppExit::Success);
            }
        }
        HexWfcCaptureMode::Gameplay => {
            if request.frame == 60 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(request.path.clone()));
            } else if request.frame == 120 {
                exit.write(AppExit::Success);
            }
        }
        HexWfcCaptureMode::Style => {
            let shot = request.frame / STYLE_STRIDE;
            if request.frame % STYLE_STRIDE == STYLE_STRIDE - 10 && shot < STYLE_SHOTS {
                let path =
                    std::path::Path::new(&request.path).join(format!("hex_wfc_{shot:03}.png"));
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
            } else if request.frame >= STYLE_SHOTS * STYLE_STRIDE + 20 {
                exit.write(AppExit::Success);
            }
        }
        HexWfcCaptureMode::Relayout => {
            // Keyed on the simulation tick, never the frame counter: the warning fires at
            // 575 and the commit at 695, and several FixedUpdate steps can share one
            // render frame. At most one screenshot entity is spawned per frame so the
            // async saves never race on the same window.
            let Some(runtime) = runtime.as_deref() else {
                return;
            };
            let tick = runtime.match_state.tick;
            // The three headline stills: quiescent, mid-warning banner, post-commit.
            const STILLS: [(u64, u8, &str); 3] = [
                (555, 0b001, "relayout_1_before"),
                (635, 0b010, "relayout_2_warning"),
                (705, 0b100, "relayout_3_after"),
            ];
            for (at, bit, name) in STILLS {
                if tick >= at && request.stills & bit == 0 {
                    request.stills |= bit;
                    let path = std::path::Path::new(&request.path).join(format!("{name}.png"));
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(path));
                    return;
                }
            }
            // Dense GIF frames spanning the whole warning→commit window.
            const GIF_START: u64 = 540;
            const GIF_END: u64 = 735;
            if (GIF_START..=GIF_END).contains(&tick)
                && tick.saturating_sub(request.last_shot_tick) >= 4
            {
                let index = ((tick - GIF_START) / 4) as u16;
                request.last_shot_tick = tick;
                let path =
                    std::path::Path::new(&request.path).join(format!("frame_{index:03}.png"));
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
            } else if tick > GIF_END + 5 {
                exit.write(AppExit::Success);
            }
        }
        HexWfcCaptureMode::Traversal => {
            let Some(runtime) = runtime.as_deref() else {
                return;
            };
            let tick = runtime.match_state.tick;
            const STILLS: [(u64, &str); 7] = [
                (325, "ramp_1_approach"),
                (350, "ramp_2_ascent"),
                (430, "ramp_3_upper_chain"),
                (490, "ramp_4_upper_ascent"),
                (655, "wellshaft_1_upper"),
                (745, "wellshaft_2_descent"),
                (850, "wellshaft_3_lower"),
            ];
            for (at, name) in STILLS {
                let bit = 1u8 << STILLS.iter().position(|entry| entry.0 == at).unwrap_or(0);
                if tick >= at && request.stills & bit == 0 {
                    request.stills |= bit;
                    let path = std::path::Path::new(&request.path).join(format!("{name}.png"));
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(path));
                    return;
                }
            }
            if tick > 875 {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Evidence-only deterministic driver for the pinned traversal montage. Normal play and
/// every other capture mode return immediately. The first leg follows the real objective
/// bot through the lower ramp, the second is staged onto the route's upper ramp, and the
/// final leg teleports to a genuine two-level shaft bond before holding the ordinary
/// descend intent. No body or geometry is animated by presentation code.
fn drive_traversal_capture(
    capture: Option<Res<HexWfcCapture>>,
    mut runtime: Option<ResMut<sim::HexWfcRuntime>>,
    mut intent: Option<ResMut<sim::HexWfcIntent>>,
) {
    let (Some(capture), Some(runtime), Some(intent)) =
        (capture, runtime.as_deref_mut(), intent.as_deref_mut())
    else {
        return;
    };
    if capture.mode != HexWfcCaptureMode::Traversal {
        return;
    }
    let tick = runtime.match_state.tick;
    if tick == 420 {
        stage_local_player(
            runtime,
            observed_facility::hex_wfc::HexCoord {
                q: 4,
                r: 6,
                level: 3,
            },
            Vec3::new(-6.0, 1.4, 0.0),
            std::f32::consts::FRAC_PI_2,
        );
    } else if tick == 650 {
        stage_local_player(
            runtime,
            observed_facility::hex_wfc::HexCoord {
                q: 2,
                r: 4,
                level: 2,
            },
            Vec3::new(0.0, 1.4, 0.0),
            0.0,
        );
    }
    intent.intent = if tick >= 650 {
        player_input::PlayerIntent {
            interact_held: true,
            ..Default::default()
        }
    } else {
        runtime.match_state.bot_command(runtime.local_player)
    };
}

fn stage_local_player(
    runtime: &mut sim::HexWfcRuntime,
    cell: observed_facility::hex_wfc::HexCoord,
    local_offset: Vec3,
    yaw: f32,
) {
    let origin = Vec3::from_array(observed_hex::hex_origin(cell));
    let player = runtime
        .match_state
        .players
        .get_mut(&runtime.local_player)
        .expect("the capture's local player exists");
    player.cell = cell;
    player.position = origin + local_offset;
    player.yaw = yaw;
    player.pitch = -0.18;
    player.climb_target = None;
    player.transit_target = None;
    player.escaped = false;
}

/// First-person flat-shaded collider surfaces can fill the frame as one colour at close
/// range. The traversal evidence uses the Phase 92 lab's proven curated ramp/shaft
/// vantages so slopes, ledges, and the depth of the well remain readable.
fn sync_traversal_capture_camera(
    capture: Option<Res<HexWfcCapture>>,
    runtime: Option<Res<sim::HexWfcRuntime>>,
    mut camera: Query<&mut Transform, With<crate::view::components::GameCam>>,
) {
    let (Some(capture), Some(runtime)) = (capture, runtime) else {
        return;
    };
    if capture.mode != HexWfcCaptureMode::Traversal {
        return;
    }
    let tick = runtime.match_state.tick;
    let (eye, target) = if tick < 400 {
        ramp_capture_vantage(
            observed_facility::hex_wfc::HexCoord {
                q: 0,
                r: 3,
                level: 0,
            },
            Vec3::new(7.0, 0.0, 12.0).normalize(),
        )
    } else if tick < 550 {
        ramp_capture_vantage(
            observed_facility::hex_wfc::HexCoord {
                q: 4,
                r: 6,
                level: 3,
            },
            Vec3::X,
        )
    } else {
        let top = observed_facility::hex_wfc::HexCoord {
            q: 2,
            r: 4,
            level: 2,
        };
        let bottom = observed_facility::hex_wfc::HexCoord { level: 1, ..top };
        let door = Vec3::new(7.0, 0.0, 12.0).normalize();
        (
            Vec3::from_array(observed_hex::hex_origin(top)) + door * 4.2 + Vec3::Y * 1.65,
            Vec3::from_array(observed_hex::hex_origin(bottom)) + Vec3::Y * 0.25,
        )
    };
    if let Ok(mut transform) = camera.single_mut() {
        *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}

fn ramp_capture_vantage(coord: observed_facility::hex_wfc::HexCoord, along: Vec3) -> (Vec3, Vec3) {
    let origin = Vec3::from_array(observed_hex::hex_origin(coord));
    let side = Vec3::new(-along.z, 0.0, along.x);
    (
        origin - along * 3.0 + side * 2.5 + Vec3::Y * 4.4,
        origin + along * 5.5 + Vec3::Y * 7.8,
    )
}
