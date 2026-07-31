//! Arc Q Phase 123: the 1280×800 frontend sweep.
//!
//! The gate asks whether every reachable screen survives the baseline viewport without
//! clipping, overlap, or unreadable text. That is a judgement a human makes by looking,
//! but the *looking* needs pictures taken at the stated size, from the assembled game
//! rather than a lab. This driver forces the primary window to the baseline, walks the
//! frontend states in hierarchy order, and photographs each one.
//!
//! It stages the career, launched-session description, and replay tape the late screens
//! read, so Results and Replay describe a real run rather than an empty default. Live
//! overlays (onboarding, pause) and the first playable frame sit inside a prepared match
//! and remain part of the hands-on traversal.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::PrimaryWindow;

use crate::GameState;
use crate::hex_wfc::overlay::{MatchOverlayState, PausePage};
use crate::screens::settings::SettingsPageAction;

/// The Phase 123 baseline viewport.
const BASELINE: (f32, f32) = (1280.0, 800.0);
/// Seconds between entering a screen and photographing it. Layout settles in one frame,
/// but text/atlas uploads and the focus pass want a little slack on a cold adapter.
const SETTLE: f32 = 0.9;

/// One photograph in the sweep. `page_action` fires an in-screen action first, which is
/// how the Controls page is reached without leaving `GameState::Settings`.
struct Shot {
    label: &'static str,
    state: GameState,
    page_action: Option<SettingsPageAction>,
    /// Modal state to install before the shot. The in-match overlays are not screens, so
    /// they are reached by setting the match's own modal state rather than a `GameState`.
    overlay: Option<MatchOverlayState>,
    /// Extra settle time. A hex match has to prepare and then admit its first cells.
    extra_settle: f32,
}

const fn shot(label: &'static str, state: GameState) -> Shot {
    Shot {
        label,
        state,
        page_action: None,
        overlay: None,
        extra_settle: 0.0,
    }
}

/// The player-facing hierarchy from the Arc Q contract, in the order a player meets it.
fn sweep() -> Vec<Shot> {
    vec![
        shot("00_main_menu", GameState::MainMenu),
        shot("01_play_hub", GameState::Play),
        shot("02_play_advanced", GameState::PlayAdvanced),
        shot("03_settings_preferences", GameState::Settings),
        Shot {
            label: "04_settings_controls",
            state: GameState::Settings,
            page_action: Some(SettingsPageAction::OpenBindings),
            ..shot("", GameState::Settings)
        },
        shot("05_loadout", GameState::Loadout),
        shot("06_lan_browser", GameState::LanBrowser),
        shot("07_lobby", GameState::Lobby),
        shot("08_loading", GameState::Loading),
        shot("09_results", GameState::Results),
        shot("10_replay", GameState::Replay),
        // The in-match views. These are the ones a state sweep cannot reach by naming a
        // screen, and the ones where an invisible UI camera hid the pause overlay.
        Shot {
            extra_settle: 6.0,
            ..shot("11_match_first_frame", GameState::HexWfc)
        },
        Shot {
            overlay: Some(MatchOverlayState::Pause(PausePage::Root)),
            ..shot("12_pause_root", GameState::HexWfc)
        },
        Shot {
            overlay: Some(MatchOverlayState::Pause(PausePage::ConfirmLeave)),
            ..shot("13_pause_confirm_leave", GameState::HexWfc)
        },
        Shot {
            overlay: Some(MatchOverlayState::SurvivorMap),
            ..shot("14_survivor_map", GameState::HexWfc)
        },
    ]
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    /// Stage the run facts the late screens read; done once, before the first shot.
    Stage,
    /// Ask for the shot's state and hold the request until it takes. The splash timer
    /// can clobber a first-frame transition, so this keeps asserting rather than
    /// assuming (see the module note in `capture/mod.rs`).
    Enter,
    /// Fire the shot's in-screen action, if it has one.
    Act,
    Settle,
    Shoot,
    Done,
}

#[derive(Resource)]
pub(super) struct FrontendCaptureRequest {
    dir: PathBuf,
    shots: Vec<Shot>,
    index: usize,
    phase: Phase,
    next_at: f32,
    resized: bool,
}

impl FrontendCaptureRequest {
    pub(super) fn new(dir: String) -> Self {
        Self {
            dir: PathBuf::from(dir),
            shots: sweep(),
            index: 0,
            phase: Phase::Stage,
            next_at: 0.0,
            resized: false,
        }
    }
}

/// Hold the window at the baseline, reasserting every frame until the surface reports
/// it. A logical `set` alone is not enough: the window can open maximized, in which case
/// the request is ignored, and on a high-DPI display logical pixels are not the pixels
/// the screenshot records. Pin the scale factor so the saved PNG really is 1280×800.
fn hold_baseline(request: &mut FrontendCaptureRequest, window: &mut Window) {
    let (width, height) = BASELINE;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the baseline is a small positive integer pair"
    )]
    let (physical_width, physical_height) = (width as u32, height as u32);
    request.resized = window.resolution.physical_width() == physical_width
        && window.resolution.physical_height() == physical_height;
    if request.resized {
        return;
    }
    window.set_maximized(false);
    window.resolution.set_scale_factor_override(Some(1.0));
    window
        .resolution
        .set_physical_resolution(physical_width, physical_height);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_frontend_progress(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut request: ResMut<FrontendCaptureRequest>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    page_actions: Query<(Entity, &SettingsPageAction)>,
    mut career: ResMut<crate::flow::Career>,
    mut next: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    hold_baseline(&mut request, &mut window);
    let elapsed = time.elapsed_secs();

    match request.phase {
        // Nothing is photographed until the surface actually reports the baseline,
        // so a shot can never silently record some other viewport.
        Phase::Stage if request.resized => {
            let (_, result, solo, tape) = super::scenarios::staged_results_case(0);
            *career = crate::flow::Career::default();
            career.bot_rival_teams = !solo;
            career.record(result);
            let _ = career.award();
            career.last_unlocks.clear();
            commands.insert_resource(tape);
            commands.insert_resource(crate::play_setup::ActivePlaySession {
                kind: crate::play_setup::ActivePlayKind::TeamRace,
                teams: 4,
                members_per_team: 1,
                networked: false,
            });
            request.phase = Phase::Enter;
        }
        Phase::Enter => {
            let target = request.shots[request.index].state;
            if *state.get() == target {
                request.phase = Phase::Act;
            } else {
                // Entering the canonical match without going through Loading is the
                // private harness path, and it requires a direct driver. It also gives
                // the shot a body that walks, so the first frame is a real vantage
                // rather than a spawn-point stare.
                if target == GameState::HexWfc {
                    commands.insert_resource(crate::sim::state::SpectatorBot::for_seed(
                        crate::flow::MATCH_SEED,
                    ));
                }
                next.set(target);
            }
        }
        Phase::Act => {
            let shot = &request.shots[request.index];
            if let Some(wanted) = shot.page_action
                && let Some((entity, _)) =
                    page_actions.iter().find(|(_, action)| **action == wanted)
            {
                commands.trigger(Activate { entity });
            }
            if let Some(overlay) = shot.overlay {
                commands.insert_resource(overlay);
            }
            request.next_at = elapsed + SETTLE + shot.extra_settle;
            request.phase = Phase::Settle;
        }
        Phase::Settle if elapsed >= request.next_at => {
            request.phase = Phase::Shoot;
        }
        Phase::Shoot => {
            let shot = &request.shots[request.index];
            let path = request.dir.join(format!("{}.png", shot.label));
            info!(
                "FRONTEND_CAPTURE shot={} state={:?} viewport={}x{}",
                shot.label,
                state.get(),
                window.resolution.physical_width(),
                window.resolution.physical_height()
            );
            crate::evidence::driver::screenshot_to(
                &mut commands,
                path.to_string_lossy().into_owned(),
            );
            request.index += 1;
            if request.index >= request.shots.len() {
                request.next_at = elapsed + 1.0;
                request.phase = Phase::Done;
            } else {
                request.phase = Phase::Enter;
            }
        }
        Phase::Done if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
