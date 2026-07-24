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
                view::sync_practical_shadow_budget,
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
    /// and the showcase (12×9×4) config so the deterministic warning@546 / commit@666
    /// timeline (proven by `observed_relayout_commits_mid_match_deterministically`)
    /// reproduces on the capture path.
    Relayout,
    /// Pinned showcase route through ramps and grounded switchback stairs.
    /// This is evidence-only automation: every frame still advances the authoritative
    /// match through `PlayerIntent` and the production controller.
    Traversal,
}

/// How many screenshots the style montage takes before exiting; matched to a spectated
/// walkthrough long enough to descend several levels.
const STYLE_SHOTS: u16 = 8;
const STYLE_STRIDE: u16 = 45;

/// The pinned seed whose showcase-config relayout commits deterministically at tick 666
/// (generation 0 → 1). Shared with the match-layer determinism test.
const RELAYOUT_CAPTURE_SEED: u64 = 0x3F2B_ECB9_7F4A_7C15;
/// Route gate: two ramp transitions and two stair transitions on a compact
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
    // shows real traversal (ramps, stairs, the exit) rather than a frozen spawn.
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
            // 546 and the commit at 666, and several FixedUpdate steps can share one
            // render frame. At most one screenshot entity is spawned per frame so the
            // async saves never race on the same window.
            let Some(runtime) = runtime.as_deref() else {
                return;
            };
            let tick = runtime.match_state.tick;
            // The three headline stills: quiescent, mid-warning banner, post-commit.
            const STILLS: [(u64, u8, &str); 3] = [
                (526, 0b001, "relayout_1_before"),
                (606, 0b010, "relayout_2_warning"),
                (676, 0b100, "relayout_3_after"),
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
            const GIF_START: u64 = 511;
            const GIF_END: u64 = 706;
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
                (450, "ramp_1_upper"),
                (1_300, "stair_1_approach"),
                (1_600, "stair_1_lower_flight"),
                (5_500, "stair_2_upper_flight"),
                (7_900, "stair_3_second_tower"),
                (14_200, "ramp_2_upper_route"),
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
            if runtime.match_state.players[&runtime.local_player].escaped || tick > 20_000 {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Evidence-only deterministic driver for the pinned traversal montage. Normal play and
/// every other capture mode return immediately. The objective bot walks the complete
/// physical route through ramps and switchback stairs with ordinary `PlayerIntent`.
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
    if runtime.match_state.lanterns.deployed.is_empty() {
        let blueprint = runtime
            .match_state
            .facility
            .blueprints
            .iter()
            .find(|blueprint| blueprint.cells.contains(&runtime.match_state.guardian.cell))
            .expect("capture Guardian belongs to a room blueprint");
        let threshold = observed_facility::hex_wfc::HexThresholdKey {
            room_generation_key: blueprint.generation_key(),
            port: "traversal-capture-pin",
        };
        runtime
            .match_state
            .lanterns
            .deploy(
                runtime.local_player,
                threshold,
                blueprint.anchor,
                Vec3::ZERO,
            )
            .expect("traversal capture starts with one anchor lantern");
    }
    intent.intent = runtime.match_state.bot_command(runtime.local_player);
}
