use bevy::{app::AppExit, input_focus::InputFocus, prelude::*};

use crate::{
    AppState, LabSettings,
    session::{GameplaySession, SessionResetRequested},
};

const NORMAL_BUTTON: Color = Color::srgb(0.08, 0.13, 0.20);
const HOVERED_BUTTON: Color = Color::srgb(0.12, 0.28, 0.40);
const PRESSED_BUTTON: Color = Color::srgb(0.15, 0.62, 0.74);
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

#[derive(Component)]
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

type ButtonVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    (Changed<Interaction>, With<Button>),
>;

type ButtonActionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static MenuAction),
    (Changed<Interaction>, With<Button>),
>;

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
                .spawn(panel_node(px(620), px(590)))
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
                    spawn_button(panel, "Start lifecycle run", MenuAction::Start);
                    spawn_button(panel, "Settings", MenuAction::Settings);
                    spawn_button(panel, "Controls", MenuAction::Controls);
                    spawn_button(panel, "Quit", MenuAction::Quit);
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
                .spawn(panel_node(px(650), px(520)))
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
                        SettingsLabel::HighContrast,
                        format!("High contrast: {}", on_off(settings.high_contrast)),
                        MenuAction::ToggleHighContrast,
                    );
                    spawn_settings_button(
                        panel,
                        SettingsLabel::Diagnostics,
                        format!(
                            "Lifecycle monitor: {}",
                            on_off(settings.diagnostics_visible)
                        ),
                        MenuAction::ToggleDiagnostics,
                    );
                    spawn_button(panel, "Back", MenuAction::BackToMain);
                });
        });
}

pub(crate) fn setup_controls(mut commands: Commands) {
    commands
        .spawn(screen_root(AppState::Controls))
        .with_children(|parent| {
            parent
                .spawn(panel_node(px(700), px(560)))
                .with_children(|panel| {
                    spawn_text(panel, "MENU LAB", 16.0, ACCENT);
                    spawn_title(panel, "CONTROLS");
                    spawn_spacer(panel, 12.0);
                    for line in [
                        "Mouse       Activate menu buttons",
                        "Escape      Back / pause / resume",
                        "Enter       Skip boot or loading",
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
                    spawn_button(panel, "Back", MenuAction::BackToMain);
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
                    spawn_button(panel, "Pause", MenuAction::Pause);
                    spawn_button(panel, "Reset session", MenuAction::ResetSession);
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
                .spawn(panel_node(px(570), px(500)))
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
                    spawn_button(panel, "Resume", MenuAction::Resume);
                    spawn_button(panel, "Reset and resume", MenuAction::ResetAndResume);
                    spawn_button(panel, "Return to main menu", MenuAction::ReturnToMain);
                });
        });
}

pub(crate) fn update_settings_labels(
    settings: Res<LabSettings>,
    mut labels: Query<(&SettingsLabel, &mut Text)>,
) {
    if !settings.is_changed() {
        return;
    }

    for (label, mut text) in &mut labels {
        **text = match label {
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
    }
}

pub(crate) fn button_visuals(
    settings: Res<LabSettings>,
    mut focus: ResMut<InputFocus>,
    mut buttons: ButtonVisualQuery,
) {
    for (entity, interaction, mut background, mut border) in &mut buttons {
        let (normal, hovered, pressed) = if settings.high_contrast {
            (
                Color::BLACK,
                Color::srgb(0.12, 0.25, 0.85),
                Color::srgb(0.0, 0.8, 1.0),
            )
        } else {
            (NORMAL_BUTTON, HOVERED_BUTTON, PRESSED_BUTTON)
        };

        match interaction {
            Interaction::Pressed => {
                focus.set(entity);
                *background = pressed.into();
                *border = BorderColor::all(Color::WHITE);
            }
            Interaction::Hovered => {
                focus.set(entity);
                *background = hovered.into();
                *border = BorderColor::all(ACCENT);
            }
            Interaction::None => {
                focus.clear();
                *background = normal.into();
                *border = BorderColor::all(Color::srgba(0.25, 0.65, 0.85, 0.5));
            }
        }
    }
}

pub(crate) fn handle_button_actions(
    buttons: ButtonActionQuery,
    mut exit: MessageWriter<AppExit>,
    mut next_state: ResMut<NextState<AppState>>,
    mut settings: ResMut<LabSettings>,
    mut clear_color: ResMut<ClearColor>,
    mut reset: ResMut<SessionResetRequested>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            MenuAction::Start => next_state.set(AppState::Loading),
            MenuAction::Settings => next_state.set(AppState::Settings),
            MenuAction::Controls => next_state.set(AppState::Controls),
            MenuAction::BackToMain | MenuAction::ReturnToMain => {
                next_state.set(AppState::MainMenu);
            }
            MenuAction::ToggleHighContrast => {
                settings.high_contrast = !settings.high_contrast;
                clear_color.0 = if settings.high_contrast {
                    Color::BLACK
                } else {
                    Color::srgb(0.025, 0.035, 0.055)
                };
            }
            MenuAction::ToggleDiagnostics => {
                settings.diagnostics_visible = !settings.diagnostics_visible;
            }
            MenuAction::Pause => next_state.set(AppState::Paused),
            MenuAction::Resume => next_state.set(AppState::Gameplay),
            MenuAction::ResetSession => reset.0 = true,
            MenuAction::ResetAndResume => {
                reset.0 = true;
                next_state.set(AppState::Gameplay);
            }
            MenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

pub(crate) fn handle_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
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

    if keyboard.just_pressed(KeyCode::Enter) {
        match state.get() {
            AppState::Boot => next_state.set(AppState::MainMenu),
            AppState::Loading => next_state.set(AppState::Gameplay),
            _ => {}
        }
    }

    if keyboard.just_pressed(KeyCode::Escape) {
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

fn spawn_button(parent: &mut ChildSpawnerCommands, label: &str, action: MenuAction) {
    parent
        .spawn((
            Button,
            action,
            Node {
                width: px(340),
                height: px(52),
                margin: UiRect::vertical(px(3)),
                border: UiRect::all(px(1)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(NORMAL_BUTTON),
            BorderColor::all(Color::srgba(0.25, 0.65, 0.85, 0.5)),
        ))
        .with_children(|button| {
            spawn_text(button, label, 18.0, TEXT);
        });
}

fn spawn_settings_button(
    parent: &mut ChildSpawnerCommands,
    marker: SettingsLabel,
    label: String,
    action: MenuAction,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                width: px(390),
                height: px(58),
                margin: UiRect::vertical(px(4)),
                border: UiRect::all(px(1)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(NORMAL_BUTTON),
            BorderColor::all(Color::srgba(0.25, 0.65, 0.85, 0.5)),
        ))
        .with_children(|button| {
            button.spawn((
                marker,
                Text::new(label),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT),
            ));
        });
}

fn on_off(value: bool) -> &'static str {
    if value { "ON" } else { "OFF" }
}
