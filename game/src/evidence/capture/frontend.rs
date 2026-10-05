//! Arc Q Phase 123: the 1280×800 frontend sweep.
//!
//! The gate asks whether every reachable screen survives the baseline viewport without
//! clipping, overlap, or unreadable text. That is a judgement a human makes by looking,
//! but the *looking* needs pictures taken at the stated size, from the assembled game
//! rather than a lab. This driver forces the primary window to the baseline, walks the
//! frontend states in hierarchy order, and photographs each one.
//!
//! It stages the career, launched-session description, and replay tape the late screens
//! read, so Results and Replay show synthetic populated fixtures. Menu mode enters
//! each role through the production launch/Loading handoff; the historical sweep uses
//! direct state entry. Neither mode establishes a completed human session.

use std::path::PathBuf;

#[path = "completion.rs"]
mod completion;
#[path = "guidance.rs"]
mod guidance;

use crate::screens::widgets::WidgetId;
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::ui_widgets::Activate;
use bevy::window::PrimaryWindow;

use crate::GameState;
use crate::hex_wfc::overlay::{MatchOverlayState, PausePage};
use crate::play_setup::{PlayPreset, PlayRules, PlaySeat, PlaySetupDraft};
use crate::screens::onboarding::OnboardingAction;
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
    setup: Option<PlaySetupDraft>,
    help_action: Option<OnboardingAction>,
    production_launch: bool,
    guidance: Option<crate::hex_wfc::GuidanceCaptureCase>,
    completion: Option<usize>,
}

const fn shot(label: &'static str, state: GameState) -> Shot {
    Shot {
        label,
        state,
        page_action: None,
        overlay: None,
        extra_settle: 0.0,
        setup: None,
        help_action: None,
        production_launch: false,
        guidance: None,
        completion: None,
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

/// Explicit rules/role/preset variants plus every help beat, using the production
/// semantic actions. This is a layout proof, not a human movement playtest.
fn menu_sweep() -> Vec<Shot> {
    let mut shots = vec![
        shot("00_main_menu", GameState::MainMenu),
        shot("00b_cosmetics", GameState::Loadout),
    ];
    for (rules, seat, names) in [
        (
            PlayRules::Ascent,
            PlaySeat::Observer,
            [
                "01_observer_solo",
                "02_observer_co_op",
                "03_observer_teams",
                "04_observer_spectate",
            ],
        ),
        (
            PlayRules::Ascent,
            PlaySeat::Architect,
            [
                "05_architect_solo",
                "06_architect_co_op",
                "07_architect_teams",
                "08_architect_spectate",
            ],
        ),
        (
            PlayRules::Race,
            PlaySeat::Observer,
            [
                "09_race_solo",
                "10_race_co_op",
                "11_race_teams",
                "12_race_spectate",
            ],
        ),
    ] {
        for (preset, name) in [
            PlayPreset::Solo,
            PlayPreset::CoOp,
            PlayPreset::TeamRace,
            PlayPreset::Spectate,
        ]
        .into_iter()
        .zip(names)
        {
            shots.push(Shot {
                setup: Some(PlaySetupDraft {
                    rules,
                    seat,
                    ..PlaySetupDraft::for_preset(preset)
                }),
                ..shot(name, GameState::Play)
            });
        }
    }
    shots.push(Shot {
        setup: Some(PlaySetupDraft {
            preset: PlayPreset::Custom,
            teams: 4,
            members_per_team: 4,
            fill_empty_seats: true,
            seat: PlaySeat::Architect,
            ..PlaySetupDraft::default()
        }),
        ..shot("13_advanced", GameState::PlayAdvanced)
    });
    for (rules, seat, entry, names) in [
        (
            PlayRules::Ascent,
            PlaySeat::Observer,
            "14_observer_entry",
            [
                "15_observer_help_1",
                "16_observer_help_2",
                "17_observer_help_3",
                "18_observer_help_4",
            ],
        ),
        (
            PlayRules::Ascent,
            PlaySeat::Architect,
            "19_architect_entry",
            [
                "20_architect_help_1",
                "21_architect_help_2",
                "22_architect_help_3",
                "23_architect_help_4",
            ],
        ),
        (
            PlayRules::Race,
            PlaySeat::Observer,
            "24_race_entry",
            [
                "25_race_help_1",
                "26_race_help_2",
                "27_race_help_3",
                "28_race_help_4",
            ],
        ),
    ] {
        shots.push(Shot {
            setup: Some(PlaySetupDraft {
                rules,
                seat,
                ..PlaySetupDraft::for_preset(PlayPreset::Solo)
            }),
            ..shot(entry, GameState::Play)
        });
        for (index, name) in names.into_iter().enumerate() {
            shots.push(Shot {
                help_action: (index > 0).then_some(OnboardingAction::Next),
                production_launch: index == 0,
                extra_settle: if index == 0 { 6.0 } else { 0.0 },
                ..shot(name, GameState::HexWfc)
            });
        }
        if seat == PlaySeat::Architect {
            shots.push(Shot {
                help_action: Some(OnboardingAction::Next),
                ..shot("23b_architect_desk", GameState::HexWfc)
            });
            shots.push(Shot {
                overlay: Some(MatchOverlayState::Pause(PausePage::Root)),
                ..shot("23c_architect_pause", GameState::HexWfc)
            });
        }
    }
    shots
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
    /// Hold the screen until the asynchronous GPU capture reports completion.
    Capturing,
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
    captured: bool,
}

impl FrontendCaptureRequest {
    pub(super) fn new(dir: String) -> Self {
        Self {
            dir: PathBuf::from(dir),
            shots: if std::env::var_os("OBSERVED2_CAPTURE_FRONTEND_COMPLETION").is_some() {
                completion::sweep()
            } else if std::env::var_os("OBSERVED2_CAPTURE_FRONTEND_GUIDANCE").is_some() {
                guidance::sweep()
            } else if std::env::var_os("OBSERVED2_CAPTURE_FRONTEND_MENUS").is_some() {
                menu_sweep()
            } else {
                sweep()
            },
            index: 0,
            phase: Phase::Stage,
            next_at: 0.0,
            resized: false,
            captured: false,
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

type WidgetBounds<'w, 's> = Query<
    'w,
    's,
    (
        &'static WidgetId,
        &'static ComputedNode,
        &'static UiGlobalTransform,
    ),
>;

type TextBounds<'w, 's> = Query<
    'w,
    's,
    (
        &'static Text,
        &'static ComputedNode,
        &'static UiGlobalTransform,
        &'static InheritedVisibility,
    ),
>;

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_frontend_progress(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut request: ResMut<FrontendCaptureRequest>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    page_actions: Query<(Entity, &SettingsPageAction)>,
    mut career: ResMut<crate::flow::Career>,
    mut setup: ResMut<PlaySetupDraft>,
    help_actions: Query<(Entity, &OnboardingAction)>,
    play_actions: Query<(Entity, &crate::screens::play::PlayAction)>,
    widget_bounds: WidgetBounds,
    text_bounds: TextBounds,
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
            if let Some(draft) = &request.shots[request.index].setup {
                *setup = draft.clone();
            }
            let target = request.shots[request.index].state;
            if *state.get() == target {
                request.phase = Phase::Act;
            } else if request.shots[request.index].production_launch {
                // The entry proof uses the same semantic Start and prepared handoff
                // as the player. Leave Loading in charge until it admits the match.
                if *state.get() == GameState::Play {
                    let (entity, _) = play_actions
                        .iter()
                        .find(|(_, action)| **action == crate::screens::play::PlayAction::Launch)
                        .expect("Play exposes the real launch action");
                    commands.trigger(Activate { entity });
                }
            } else {
                if let Some(case) = request.shots[request.index].completion {
                    commands.queue(move |world: &mut World| completion::stage(world, case));
                }
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
            if let Some(wanted) = shot.help_action {
                let (entity, _) = help_actions
                    .iter()
                    .find(|(_, action)| **action == wanted)
                    .expect("role help must be visible for its capture action");
                commands.trigger(Activate { entity });
            }
            if let Some(overlay) = shot.overlay {
                if let Some((entity, _)) = help_actions
                    .iter()
                    .find(|(_, action)| **action == OnboardingAction::Skip)
                {
                    commands.trigger(Activate { entity });
                }
                commands.insert_resource(overlay);
            }
            if let Some(case) = shot.guidance {
                commands.queue(move |world: &mut World| {
                    crate::hex_wfc::stage_guidance_capture(world, case)
                });
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
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            request.captured = false;
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.clone()))
                .observe(
                    |_: On<ScreenshotCaptured>, mut request: ResMut<FrontendCaptureRequest>| {
                        request.captured = true;
                    },
                );
            let bounds: Vec<_> = widget_bounds.iter().filter(|(_, node, _)| node.size().min_element() > 0.0).map(|(id, node, transform)| {
                let rect = Rect::from_center_size(transform.affine().translation, node.size());
                serde_json::json!({ "widget": format!("{id:?}"), "min": [rect.min.x, rect.min.y], "max": [rect.max.x, rect.max.y] })
            }).collect();
            std::fs::write(
                path.with_extension("bounds.json"),
                serde_json::to_vec_pretty(&bounds).expect("widget bounds serialize"),
            )
            .expect("capture bounds are writable");
            let text_rects: Vec<_> = text_bounds.iter()
                .filter(|(_, node, _, visibility)| visibility.get() && node.size().min_element() > 0.0)
                .map(|(text, node, transform, _)| {
                    let rect = Rect::from_center_size(transform.affine().translation, node.size());
                    serde_json::json!({ "text": text.0, "min": [rect.min.x, rect.min.y], "max": [rect.max.x, rect.max.y] })
                }).collect();
            std::fs::write(
                path.with_extension("text-bounds.json"),
                serde_json::to_vec_pretty(&text_rects).expect("text bounds serialize"),
            )
            .expect("capture text bounds are writable");
            request.phase = Phase::Capturing;
        }
        Phase::Capturing if request.captured => {
            request.index += 1;
            if request.index >= request.shots.len() {
                request.next_at = elapsed + 1.0;
                request.phase = Phase::Done;
            } else {
                if request.shots[request.index].completion.is_some() {
                    next.set(GameState::MainMenu);
                }
                request.phase = Phase::Enter;
            }
        }
        Phase::Done if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
