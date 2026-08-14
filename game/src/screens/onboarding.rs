//! Versioned first-run help for the canonical continuous hex facility.
//!
//! The overlay is deliberately a small presentation adapter: control copy comes from
//! persisted bindings, semantic widgets own keyboard/pointer/controller activation,
//! and completion is persisted in [`crate::settings::UserPreferences`]. It never
//! reads physical input and it refuses to spawn outside [`GameState::HexWfc`].

use bevy::{prelude::*, ui::InteractionDisabled, ui_widgets::Activate};

use super::widgets::{
    self, FocusScope, FocusScopeId, UiInputCapture, WidgetId, WidgetLabel, WidgetSpec,
    activation_enabled,
};
use crate::GameState;
use crate::hex_wfc::HexOnboardingGate;
use crate::play_setup::{PlayPreset, PlaySetupDraft};
use crate::settings::{Settings, key_name, save_settings};
use crate::view::theme::{ACCENT, BORDER, DIM, PANEL, TITLE, WARNING, text};

const SCOPE: FocusScopeId = FocusScopeId("hex_onboarding");
const NEXT: WidgetId = WidgetId::named("hex_onboarding.next");
const SKIP: WidgetId = WidgetId::named("hex_onboarding.skip");
const OVERLAY_PRIORITY: i16 = 300;
const CAPTURE_OWNER: &str = "hex_onboarding";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnboardingBeat {
    pub(crate) title: &'static str,
    pub(crate) body: String,
}

/// Build the exact first-run copy from the active keyboard bindings. Controller
/// labels describe the fixed mapping in `screens::input::read_gamepad_match`.
pub(crate) fn onboarding_beats(settings: &Settings) -> Vec<OnboardingBeat> {
    let keys = &settings.bindings;
    vec![
        OnboardingBeat {
            title: "READ THE FACILITY",
            body: format!(
                "{}/{}/{}/{} move; mouse or {}/{}/{}/{} looks. On controller, use the left and right sticks. Open thresholds are physical crossings: rooms let you decide, while corridors commit you to traversal risk.",
                key_name(keys.move_forward),
                key_name(keys.move_left),
                key_name(keys.move_back),
                key_name(keys.move_right),
                key_name(keys.look_up),
                key_name(keys.look_left),
                key_name(keys.look_down),
                key_name(keys.look_right),
            ),
        },
        OnboardingBeat {
            title: "TRAVERSE",
            body: format!(
                "{} jumps and {} sprints. On controller, the south face button jumps; left trigger or left-stick click sprints. A fall reroutes you instead of ending the run.",
                key_name(keys.jump),
                key_name(keys.sprint),
            ),
        },
        OnboardingBeat {
            title: "COOPERATE AND ANCHOR",
            body: format!(
                "{} uses mechanisms and collects objectives; the west face button does the same on controller. {} or left bumper deploys an anchor lantern at the threshold you are looking at. {} or the east face button recovers it.",
                key_name(keys.interact),
                key_name(keys.torch),
                key_name(keys.recover_lantern),
            ),
        },
        OnboardingBeat {
            title: "FIND A WAY OUT",
            body: format!(
                "Collect the keystones shown on the HUD, synchronize a team station, then regroup at the exit. {} opens the survivor map; controller right trigger or Select does too. {} or Start opens the match menu.",
                key_name(keys.tac_map),
                key_name(keys.pause),
            ),
        },
    ]
}

#[derive(Component)]
pub(crate) struct OnboardingPanel;

#[derive(Component)]
pub(crate) struct OnboardingProgress;

#[derive(Component)]
pub(crate) struct OnboardingTitle;

#[derive(Component)]
pub(crate) struct OnboardingBody;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OnboardingAction {
    Next,
    Skip,
}

/// Presence is the canonical input adapter's modal seam: while this resource exists,
/// gameplay intent is neutral and the cursor belongs to the semantic overlay.
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OnboardingState {
    pub(crate) step: usize,
}

type OnboardingBodyQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<OnboardingBody>,
        Without<OnboardingProgress>,
        Without<OnboardingTitle>,
    ),
>;

/// Spawn only for a fresh/revised onboarding in the canonical production match.
/// Keeping the state check here as well as in plugin scheduling prevents an accidental
/// future registration from reviving onboarding in the deprecated Match fixture.
pub(crate) fn spawn(
    mut commands: Commands,
    current: Res<State<GameState>>,
    settings: Res<Settings>,
    play_setup: Res<PlaySetupDraft>,
    lan: Res<crate::lan::LanRuntime>,
    mut gate: ResMut<HexOnboardingGate>,
    mut capture: ResMut<UiInputCapture>,
) {
    if *current.get() != GameState::HexWfc
        || !settings.needs_onboarding()
        || play_setup.preset == PlayPreset::Spectate
        || evidence_capture_active()
    {
        return;
    }

    let beats = onboarding_beats(&settings);
    let first = &beats[0];
    let networked = lan.client.is_some();
    gate.active = true;
    capture.capture(CAPTURE_OWNER);
    commands.insert_resource(OnboardingState::default());
    commands
        .spawn((
            OnboardingPanel,
            DespawnOnExit(GameState::HexWfc),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(24)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.005, 0.008, 0.015, 0.82)),
            GlobalZIndex(200),
            Name::new("Canonical first-run onboarding"),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(720),
                    max_width: percent(94),
                    padding: UiRect::axes(px(42), px(34)),
                    border: UiRect::all(px(2)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(14),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
                widgets::focus_scope(FocusScope::overlay(
                    SCOPE,
                    NEXT,
                    SKIP,
                    OVERLAY_PRIORITY,
                )),
            ))
            .with_children(|panel| {
                panel.spawn((
                    OnboardingProgress,
                    text(progress_label(0, beats.len()), 14.0, ACCENT),
                ));
                panel.spawn((OnboardingTitle, text(first.title, 34.0, TITLE)));
                panel.spawn((
                    OnboardingBody,
                    text(first.body.clone(), 18.0, TITLE),
                    TextLayout::justify(Justify::Center),
                    Node {
                        max_width: px(620),
                        ..default()
                    },
                ));
                panel.spawn(text(
                    "Connections can refactor only when unobserved and unprotected. The frame light reports an anchor lock-not whether someone is looking.",
                    14.0,
                    DIM,
                ));
                if networked {
                    panel.spawn(text(
                        "LAN MATCH CONTINUES - the runner sends neutral input while help is open",
                        14.0,
                        WARNING,
                    ));
                }
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(NEXT, SCOPE, 0, next_label(0, beats.len()))
                        .with_size(520.0, 50.0),
                    OnboardingAction::Next,
                );
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(SKIP, SCOPE, 1, "Skip onboarding")
                        .with_size(520.0, 46.0),
                    OnboardingAction::Skip,
                );
                panel.spawn(text(
                    "Enter / A continues | Esc / B skips | Controls remain available from the match menu",
                    13.0,
                    ACCENT,
                ));
            });
        });
}

/// Screen-local semantic activation observer. It advances one explicit step or records
/// the current revision as complete when the player finishes or skips.
#[allow(clippy::too_many_arguments)]
pub(crate) fn activate(
    activation: On<Activate>,
    actions: Query<&OnboardingAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut state: Option<ResMut<OnboardingState>>,
    mut gate: ResMut<HexOnboardingGate>,
    mut settings: ResMut<Settings>,
    panels: Query<Entity, With<OnboardingPanel>>,
    mut progress: Query<&mut Text, With<OnboardingProgress>>,
    mut titles: Query<&mut Text, (With<OnboardingTitle>, Without<OnboardingProgress>)>,
    mut bodies: OnboardingBodyQuery,
    mut next_labels: Query<(&OnboardingAction, &mut WidgetLabel)>,
    mut commands: Commands,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    let beats = onboarding_beats(&settings);

    if *action == OnboardingAction::Skip || state.step + 1 >= beats.len() {
        finish(&mut commands, &mut gate, &mut settings, &panels);
        return;
    }

    state.step += 1;
    let beat = &beats[state.step];
    if let Ok(mut label) = progress.single_mut() {
        **label = progress_label(state.step, beats.len());
    }
    if let Ok(mut label) = titles.single_mut() {
        **label = beat.title.to_string();
    }
    if let Ok(mut label) = bodies.single_mut() {
        **label = beat.body.clone();
    }
    for (action, mut label) in &mut next_labels {
        if *action == OnboardingAction::Next {
            label.0 = next_label(state.step, beats.len()).to_string();
        }
    }
}

/// Remove the modal resource even when the match exits before the player responds.
/// An interrupted first run remains incomplete and is offered again next time.
pub(crate) fn cleanup(
    mut commands: Commands,
    mut gate: ResMut<HexOnboardingGate>,
    mut capture: ResMut<UiInputCapture>,
) {
    commands.remove_resource::<OnboardingState>();
    gate.active = false;
    capture.release(CAPTURE_OWNER);
}

/// Keep the modal input capture through the exact key/button edge that dismissed
/// onboarding. Without this one-frame latch, Escape/East can also open Pause after
/// the observer removes `OnboardingState`, depending on cross-plugin system order.
pub(crate) fn release_capture_after_dismissal(
    onboarding: Option<Res<OnboardingState>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut capture: ResMut<UiInputCapture>,
) {
    if onboarding.is_some() {
        return;
    }
    let dismissal_edge =
        keyboard.any_just_pressed([KeyCode::Escape, KeyCode::Enter, KeyCode::Space])
            || gamepads.iter().any(|gamepad| {
                gamepad.just_pressed(GamepadButton::East)
                    || gamepad.just_pressed(GamepadButton::South)
            });
    if !dismissal_edge {
        capture.release(CAPTURE_OWNER);
    }
}

fn finish(
    commands: &mut Commands,
    gate: &mut HexOnboardingGate,
    settings: &mut Settings,
    panels: &Query<Entity, With<OnboardingPanel>>,
) {
    settings.complete_onboarding();
    save_settings(settings);
    gate.active = false;
    commands.remove_resource::<OnboardingState>();
    for panel in panels {
        commands.entity(panel).despawn();
    }
}

fn evidence_capture_active() -> bool {
    std::env::vars_os().any(|(key, _)| {
        key.to_string_lossy()
            .starts_with("OBSERVED2_CAPTURE_HEX_WFC")
    })
}

fn progress_label(step: usize, total: usize) -> String {
    format!("FIRST RUN  -  {} / {total}", step + 1)
}

fn next_label(step: usize, total: usize) -> &'static str {
    if step + 1 >= total {
        "Start exploring"
    } else {
        "Next"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_app(game_state: GameState, settings: Settings) -> App {
        let mut app = App::new();
        app.insert_resource(State::new(game_state))
            .insert_resource(settings)
            .insert_resource(PlaySetupDraft::default())
            .insert_resource(crate::lan::LanRuntime::new())
            .insert_resource(HexOnboardingGate::default())
            .init_resource::<UiInputCapture>()
            .add_observer(activate)
            .add_systems(Startup, spawn);
        app.update();
        app
    }

    fn action_entity(app: &mut App, wanted: OnboardingAction) -> Entity {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &OnboardingAction)>();
        query
            .iter(world)
            .find_map(|(entity, action)| (*action == wanted).then_some(entity))
            .expect("onboarding action exists")
    }

    #[test]
    fn beats_are_short_and_reflect_rebindings() {
        let mut settings = Settings::default();
        crate::settings::BindingSlot::MoveForward.set(&mut settings.bindings, KeyCode::KeyJ);
        crate::settings::BindingSlot::Jump.set(&mut settings.bindings, KeyCode::KeyK);
        crate::settings::BindingSlot::Interact.set(&mut settings.bindings, KeyCode::KeyI);
        crate::settings::BindingSlot::Torch.set(&mut settings.bindings, KeyCode::KeyG);
        crate::settings::BindingSlot::TacMap.set(&mut settings.bindings, KeyCode::KeyM);
        crate::settings::BindingSlot::Pause.set(&mut settings.bindings, KeyCode::F10);

        let beats = onboarding_beats(&settings);
        assert_eq!(beats.len(), 4);
        assert!(beats[0].body.starts_with("J/"));
        assert!(beats[1].body.contains("K jumps"));
        assert!(beats[2].body.contains("I uses"));
        assert!(beats[2].body.contains("G or left bumper"));
        assert!(beats[3].body.contains("M opens"));
        assert!(beats[3].body.contains("F10 or Start"));
        assert!(beats.iter().all(|beat| beat.body.len() < 360));
    }

    #[test]
    fn spawn_is_hard_gated_to_canonical_hex_state() {
        let mut deprecated = spawn_app(GameState::Match, Settings::default());
        let mut canonical = spawn_app(GameState::HexWfc, Settings::default());
        let mut completed = Settings::default();
        completed.complete_onboarding();
        let mut completed = spawn_app(GameState::HexWfc, completed);

        let deprecated_count = {
            let world = deprecated.world_mut();
            let mut query = world.query::<&OnboardingPanel>();
            query.iter(world).count()
        };
        let canonical_count = {
            let world = canonical.world_mut();
            let mut query = world.query::<&OnboardingPanel>();
            query.iter(world).count()
        };
        let completed_count = {
            let world = completed.world_mut();
            let mut query = world.query::<&OnboardingPanel>();
            query.iter(world).count()
        };
        assert_eq!(deprecated_count, 0);
        assert_eq!(canonical_count, 1);
        assert_eq!(completed_count, 0);
    }

    #[test]
    fn spectator_runs_do_not_receive_participant_onboarding() {
        let mut app = App::new();
        app.insert_resource(State::new(GameState::HexWfc))
            .insert_resource(Settings::default())
            .insert_resource(PlaySetupDraft::for_preset(PlayPreset::Spectate))
            .insert_resource(crate::lan::LanRuntime::new())
            .insert_resource(HexOnboardingGate::default())
            .init_resource::<UiInputCapture>()
            .add_systems(Startup, spawn);
        app.update();

        assert!(!app.world().contains_resource::<OnboardingState>());
        assert!(!app.world().resource::<HexOnboardingGate>().active);
    }

    #[test]
    fn semantic_next_completes_and_persists_the_current_version() {
        let mut app = spawn_app(GameState::HexWfc, Settings::default());
        let next = action_entity(&mut app, OnboardingAction::Next);

        for expected_step in 1..4 {
            app.world_mut().trigger(Activate { entity: next });
            app.update();
            assert_eq!(
                app.world().resource::<OnboardingState>().step,
                expected_step
            );
        }
        app.world_mut().trigger(Activate { entity: next });
        app.update();

        let settings = app.world().resource::<Settings>();
        assert_eq!(
            settings.completed_onboarding_version,
            crate::settings::CURRENT_ONBOARDING_VERSION
        );
        assert!(!settings.needs_onboarding());
        assert!(!app.world().contains_resource::<OnboardingState>());
    }

    #[test]
    fn semantic_skip_completes_immediately() {
        let mut app = spawn_app(GameState::HexWfc, Settings::default());
        let skip = action_entity(&mut app, OnboardingAction::Skip);

        app.world_mut().trigger(Activate { entity: skip });
        app.update();

        assert!(!app.world().resource::<Settings>().needs_onboarding());
        assert!(!app.world().contains_resource::<OnboardingState>());
        assert!(
            app.world().resource::<UiInputCapture>().is_active(),
            "dismissal retains input capture until the triggering edge has passed"
        );
    }
}
