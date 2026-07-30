use bevy::{
    app::AppExit, ecs::system::SystemParam, input::gamepad::GamepadButton, input_focus::InputFocus,
    prelude::*, ui::InteractionDisabled, ui_widgets::Activate,
};

use crate::{
    AppState, LabSettings,
    session::{GameplaySession, SessionResetRequested},
    widgets::{
        self, FocusMemory, FocusScope, FocusScopeId, WidgetId, WidgetLabel, WidgetSpec, WidgetText,
    },
};

const TEXT: Color = Color::srgb(0.88, 0.94, 1.0);
const MUTED_TEXT: Color = Color::srgb(0.55, 0.67, 0.78);
const ACCENT: Color = Color::srgb(0.25, 0.85, 1.0);
const PANEL: Color = Color::srgba(0.025, 0.055, 0.09, 0.96);

#[derive(Component)]
pub(crate) struct ScreenRoot;

#[derive(Component, Clone, Copy, Debug)]
pub(crate) enum MenuAction {
    Start,
    Settings,
    Controls,
    BackToMain,
    ToggleHighContrast,
    ToggleDiagnostics,
    Pause,
    Resume,
    ResetSession,
    ResetAndResume,
    ReturnToMain,
    Quit,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum SettingsLabel {
    HighContrast,
    Diagnostics,
}

#[derive(Resource, Deref, DerefMut)]
pub(crate) struct BootTimer(Timer);

#[derive(Resource, Deref, DerefMut)]
pub(crate) struct LoadingTimer(Timer);

#[derive(Component)]
pub(crate) struct LoadingStatus;

pub(crate) fn setup_boot(mut commands: Commands) {
    commands.insert_resource(BootTimer(Timer::from_seconds(0.9, TimerMode::Once)));
    commands
        .spawn(screen_root(AppState::Boot))
        .with_children(|parent| {
            parent
                .spawn(panel_node(px(640), px(300)))
                .with_children(|panel| {
                    spawn_title(panel, "OBSERVED 2");
                    spawn_text(panel, "TECHNICAL FOUNDATION", 22.0, ACCENT);
                    spawn_text(
                        panel,
                        "Initializing application states and lifecycle monitor…",
                        17.0,
                        MUTED_TEXT,
                    );
                    spawn_text(panel, "Press Enter to continue", 15.0, MUTED_TEXT);
                });
        });
}

pub(crate) fn remove_boot_timer(mut commands: Commands) {
    commands.remove_resource::<BootTimer>();
}

pub(crate) fn boot_countdown(
    time: Res<Time>,
    timer: Option<ResMut<BootTimer>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Some(mut timer) = timer else {
        return;
    };
    if timer.tick(time.delta()).is_finished() {
        next_state.set(AppState::MainMenu);
    }
}

pub(crate) fn setup_main_menu(mut commands: Commands) {
    commands
        .spawn(screen_root(AppState::MainMenu))
        .with_children(|parent| {
            parent
                .spawn((
                    panel_node(px(620), px(650)),
                    widgets::focus_scope(FocusScope::new(
                        FocusScopeId::MainMenu,
                        WidgetId::MainStart,
                    )),
                ))
                .with_children(|panel| {
                    spawn_text(panel, "OBSERVED 2", 18.0, ACCENT);
                    spawn_title(panel, "MENU LAB");
                    spawn_text(
                        panel,
                        "A state-transition and cleanup prototype",
                        17.0,
                        MUTED_TEXT,
                    );
                    spawn_spacer(panel, 18.0);
                    widgets::spawn_button(
                        panel,
                        WidgetSpec::disabled(
                            WidgetId::MainContinueUnavailable,
                            FocusScopeId::MainMenu,
                            0,
                            "Continue — no saved session",
                        ),
                        MenuAction::Start,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::MainStart,
                        FocusScopeId::MainMenu,
                        1,
                        "Start lifecycle run",
                        MenuAction::Start,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::MainSettings,
                        FocusScopeId::MainMenu,
                        2,
                        "Settings",
                        MenuAction::Settings,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::MainControls,
                        FocusScopeId::MainMenu,
                        3,
                        "Controls",
                        MenuAction::Controls,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::MainQuit,
                        FocusScopeId::MainMenu,
                        4,
                        "Quit",
                        MenuAction::Quit,
                    );
                    spawn_spacer(panel, 12.0);
                    spawn_text(
                        panel,
                        "Success: repeated runs return with zero gameplay-owned entities.",
                        14.0,
                        MUTED_TEXT,
                    );
                });
        });
}

pub(crate) fn setup_settings(mut commands: Commands, settings: Res<LabSettings>) {
    commands
        .spawn(screen_root(AppState::Settings))
        .with_children(|parent| {
            parent
                .spawn((
                    panel_node(px(650), px(520)),
                    widgets::focus_scope(FocusScope::new(
                        FocusScopeId::Settings,
                        WidgetId::SettingsHighContrast,
                    )),
                ))
                .with_children(|panel| {
                    spawn_text(panel, "MENU LAB", 16.0, ACCENT);
                    spawn_title(panel, "SETTINGS");
                    spawn_text(
                        panel,
                        "These settings persist across gameplay sessions.",
                        16.0,
                        MUTED_TEXT,
                    );
                    spawn_spacer(panel, 18.0);
                    spawn_settings_button(
                        panel,
                        WidgetId::SettingsHighContrast,
                        0,
                        SettingsLabel::HighContrast,
                        format!("High contrast: {}", on_off(settings.high_contrast)),
                        MenuAction::ToggleHighContrast,
                    );
                    spawn_settings_button(
                        panel,
                        WidgetId::SettingsDiagnostics,
                        1,
                        SettingsLabel::Diagnostics,
                        format!(
                            "Lifecycle monitor: {}",
                            on_off(settings.diagnostics_visible)
                        ),
                        MenuAction::ToggleDiagnostics,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::SettingsBack,
                        FocusScopeId::Settings,
                        2,
                        "Back",
                        MenuAction::BackToMain,
                    );
                });
        });
}

pub(crate) fn setup_controls(mut commands: Commands) {
    commands
        .spawn(screen_root(AppState::Controls))
        .with_children(|parent| {
            parent
                .spawn((
                    panel_node(px(700), px(590)),
                    widgets::focus_scope(FocusScope::new(
                        FocusScopeId::Controls,
                        WidgetId::ControlsBack,
                    )),
                ))
                .with_children(|panel| {
                    spawn_text(panel, "MENU LAB", 16.0, ACCENT);
                    spawn_title(panel, "CONTROLS");
                    spawn_spacer(panel, 12.0);
                    for line in [
                        "Mouse       Point, focus, and activate",
                        "Arrow / Tab Navigate ordered controls",
                        "Enter/Space Activate the focused control",
                        "D-pad/Stick Navigate • South activate • East back",
                        "Escape      Back / pause / resume",
                        "R           Reset the active gameplay session",
                        "F1          Toggle lifecycle monitor",
                    ] {
                        spawn_text(panel, line, 19.0, TEXT);
                    }
                    spawn_spacer(panel, 22.0);
                    spawn_text(
                        panel,
                        "Gameplay input is intentionally absent: this lab tests application lifecycle only.",
                        15.0,
                        MUTED_TEXT,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::ControlsBack,
                        FocusScopeId::Controls,
                        0,
                        "Back",
                        MenuAction::BackToMain,
                    );
                });
        });
}

pub(crate) fn setup_loading(mut commands: Commands) {
    commands.insert_resource(LoadingTimer(Timer::from_seconds(1.4, TimerMode::Once)));
    commands
        .spawn(screen_root(AppState::Loading))
        .with_children(|parent| {
            parent
                .spawn(panel_node(px(660), px(330)))
                .with_children(|panel| {
                    spawn_text(panel, "SESSION TRANSITION", 16.0, ACCENT);
                    spawn_title(panel, "LOADING GAMEPLAY");
                    panel.spawn((
                        LoadingStatus,
                        Text::new("Validating owned-entity boundary… 0%"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(MUTED_TEXT),
                        Node {
                            margin: UiRect::vertical(px(16)),
                            ..default()
                        },
                    ));
                    spawn_text(
                        panel,
                        "Press Enter to complete immediately",
                        14.0,
                        MUTED_TEXT,
                    );
                });
        });
}

pub(crate) fn remove_loading_timer(mut commands: Commands) {
    commands.remove_resource::<LoadingTimer>();
}

pub(crate) fn loading_countdown(
    time: Res<Time>,
    timer: Option<ResMut<LoadingTimer>>,
    status: Single<&mut Text, With<LoadingStatus>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Some(mut timer) = timer else {
        return;
    };

    timer.tick(time.delta());
    let percent = (timer.fraction() * 100.0).round() as u32;
    let mut status = status.into_inner();
    **status = format!("Validating owned-entity boundary… {percent}%");

    if timer.is_finished() {
        next_state.set(AppState::Gameplay);
    }
}

pub(crate) fn setup_gameplay_hud(mut commands: Commands, session: Res<GameplaySession>) {
    commands
        .spawn((
            ScreenRoot,
            DespawnOnExit(AppState::Gameplay),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(24)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    widgets::focus_scope(FocusScope::new(
                        FocusScopeId::Gameplay,
                        WidgetId::GameplayPause,
                    )),
                    Node {
                        width: px(390),
                        padding: UiRect::all(px(16)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(8),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.015, 0.03, 0.05, 0.90)),
                ))
                .with_children(|panel| {
                    spawn_text(panel, "GAMEPLAY SESSION", 15.0, ACCENT);
                    spawn_text(
                        panel,
                        format!("Generation {}", session.generation),
                        28.0,
                        TEXT,
                    );
                    spawn_text(
                        panel,
                        format!("{} owned entities", session.owned_entities),
                        16.0,
                        MUTED_TEXT,
                    );
                    spawn_text(
                        panel,
                        "Pause preserves this world. Reset replaces it.",
                        14.0,
                        MUTED_TEXT,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::GameplayPause,
                        FocusScopeId::Gameplay,
                        0,
                        "Pause",
                        MenuAction::Pause,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::GameplayReset,
                        FocusScopeId::Gameplay,
                        1,
                        "Reset session",
                        MenuAction::ResetSession,
                    );
                });
        });
}

pub(crate) fn setup_pause_menu(mut commands: Commands, session: Res<GameplaySession>) {
    commands
        .spawn((
            ScreenRoot,
            DespawnOnExit(AppState::Paused),
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.005, 0.008, 0.015, 0.72)),
            GlobalZIndex(50),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    panel_node(px(570), px(500)),
                    widgets::focus_scope(FocusScope::new(
                        FocusScopeId::Pause,
                        WidgetId::PauseResume,
                    )),
                ))
                .with_children(|panel| {
                    spawn_text(panel, "SESSION PRESERVED", 16.0, ACCENT);
                    spawn_title(panel, "PAUSED");
                    spawn_text(
                        panel,
                        format!(
                            "Generation {} • {} owned entities",
                            session.generation, session.owned_entities
                        ),
                        16.0,
                        MUTED_TEXT,
                    );
                    spawn_spacer(panel, 12.0);
                    spawn_menu_button(
                        panel,
                        WidgetId::PauseResume,
                        FocusScopeId::Pause,
                        0,
                        "Resume",
                        MenuAction::Resume,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::PauseResetAndResume,
                        FocusScopeId::Pause,
                        1,
                        "Reset and resume",
                        MenuAction::ResetAndResume,
                    );
                    spawn_menu_button(
                        panel,
                        WidgetId::PauseReturnToMain,
                        FocusScopeId::Pause,
                        2,
                        "Return to main menu",
                        MenuAction::ReturnToMain,
                    );
                });
        });
}

pub(crate) fn update_settings_labels(
    settings: Res<LabSettings>,
    mut buttons: Query<(&SettingsLabel, &Children, &mut WidgetLabel)>,
    mut texts: Query<&mut Text, With<WidgetText>>,
) {
    if !settings.is_changed() {
        return;
    }

    for (label, children, mut accessibility_label) in &mut buttons {
        let value = match label {
            SettingsLabel::HighContrast => {
                format!("High contrast: {}", on_off(settings.high_contrast))
            }
            SettingsLabel::Diagnostics => {
                format!(
                    "Lifecycle monitor: {}",
                    on_off(settings.diagnostics_visible)
                )
            }
        };
        accessibility_label.0.clone_from(&value);
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = value.clone();
            }
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct MenuActivationContext<'w, 's> {
    buttons: Query<
        'w,
        's,
        (
            &'static MenuAction,
            &'static WidgetId,
            Has<InteractionDisabled>,
        ),
    >,
    state: Res<'w, State<AppState>>,
    focus: ResMut<'w, InputFocus>,
    focus_memory: ResMut<'w, FocusMemory>,
    exit: MessageWriter<'w, AppExit>,
    next_state: ResMut<'w, NextState<AppState>>,
    settings: ResMut<'w, LabSettings>,
    clear_color: ResMut<'w, ClearColor>,
    reset: ResMut<'w, SessionResetRequested>,
}

pub(crate) fn handle_widget_activation(
    activation: On<Activate>,
    mut context: MenuActivationContext,
) {
    let Ok((action, widget, disabled)) = context.buttons.get(activation.entity) else {
        return;
    };
    if disabled {
        return;
    }

    context.focus.set(activation.entity);
    widgets::remember_activated(&mut context.focus_memory, *context.state.get(), *widget);

    match action {
        MenuAction::Start => context.next_state.set(AppState::Loading),
        MenuAction::Settings => context.next_state.set(AppState::Settings),
        MenuAction::Controls => context.next_state.set(AppState::Controls),
        MenuAction::BackToMain | MenuAction::ReturnToMain => {
            context.next_state.set(AppState::MainMenu);
        }
        MenuAction::ToggleHighContrast => {
            context.settings.high_contrast = !context.settings.high_contrast;
            context.clear_color.0 = if context.settings.high_contrast {
                Color::BLACK
            } else {
                Color::srgb(0.025, 0.035, 0.055)
            };
        }
        MenuAction::ToggleDiagnostics => {
            context.settings.diagnostics_visible = !context.settings.diagnostics_visible;
        }
        MenuAction::Pause => context.next_state.set(AppState::Paused),
        MenuAction::Resume => context.next_state.set(AppState::Gameplay),
        MenuAction::ResetSession => context.reset.0 = true,
        MenuAction::ResetAndResume => {
            context.reset.0 = true;
            context.next_state.set(AppState::Gameplay);
        }
        MenuAction::Quit => {
            context.exit.write(AppExit::Success);
        }
    }
}

pub(crate) fn handle_global_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut settings: ResMut<LabSettings>,
    mut reset: ResMut<SessionResetRequested>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        settings.diagnostics_visible = !settings.diagnostics_visible;
    }

    if keyboard.just_pressed(KeyCode::KeyR)
        && matches!(state.get(), AppState::Gameplay | AppState::Paused)
    {
        reset.0 = true;
    }

    let accept = keyboard.just_pressed(KeyCode::Enter)
        || keyboard.just_pressed(KeyCode::Space)
        || gamepads
            .iter()
            .any(|gamepad| gamepad.just_pressed(GamepadButton::South));
    if accept {
        match state.get() {
            AppState::Boot => next_state.set(AppState::MainMenu),
            AppState::Loading => next_state.set(AppState::Gameplay),
            _ => {}
        }
    }

    let back = keyboard.just_pressed(KeyCode::Escape)
        || gamepads
            .iter()
            .any(|gamepad| gamepad.just_pressed(GamepadButton::East));
    if back {
        match state.get() {
            AppState::Settings | AppState::Controls | AppState::Loading => {
                next_state.set(AppState::MainMenu);
            }
            AppState::Gameplay => next_state.set(AppState::Paused),
            AppState::Paused => next_state.set(AppState::Gameplay),
            AppState::Boot => next_state.set(AppState::MainMenu),
            AppState::MainMenu => {}
        }
    }
}

fn screen_root(state: AppState) -> impl Bundle {
    (
        ScreenRoot,
        DespawnOnExit(state),
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
    )
}

fn panel_node(width: Val, height: Val) -> impl Bundle {
    (
        Node {
            width,
            min_height: height,
            padding: UiRect::all(px(34)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(12),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(Color::srgba(0.25, 0.72, 0.9, 0.55)),
    )
}

fn spawn_title(parent: &mut ChildSpawnerCommands, value: impl Into<String>) {
    spawn_text(parent, value, 48.0, TEXT);
}

fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    value: impl Into<String>,
    size: f32,
    color: Color,
) {
    parent.spawn((
        Text::new(value),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextLayout::new_with_justify(Justify::Center),
    ));
}

fn spawn_spacer(parent: &mut ChildSpawnerCommands, height: f32) {
    parent.spawn(Node {
        width: px(1),
        height: px(height),
        ..default()
    });
}

fn spawn_menu_button(
    parent: &mut ChildSpawnerCommands,
    id: WidgetId,
    scope: FocusScopeId,
    order: u16,
    label: &str,
    action: MenuAction,
) {
    widgets::spawn_button(parent, WidgetSpec::enabled(id, scope, order, label), action);
}

fn spawn_settings_button(
    parent: &mut ChildSpawnerCommands,
    id: WidgetId,
    order: u16,
    marker: SettingsLabel,
    label: String,
    action: MenuAction,
) {
    widgets::spawn_button(
        parent,
        WidgetSpec::enabled(id, FocusScopeId::Settings, order, label).with_size(390.0, 58.0),
        (action, marker),
    );
}

fn on_off(value: bool) -> &'static str {
    if value { "ON" } else { "OFF" }
}
