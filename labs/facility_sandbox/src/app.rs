use bevy::{app::AppExit, prelude::*};

use crate::sandbox::{self, SandboxBuilt};

#[derive(States, Clone, Copy, Default, Debug, Eq, Hash, PartialEq)]
pub enum AppState {
    #[default]
    Boot,
    MainMenu,
    Loading,
    Sandbox,
    Paused,
}

pub fn in_sandbox_or_paused(state: Res<State<AppState>>) -> bool {
    matches!(state.get(), AppState::Sandbox | AppState::Paused)
}

#[derive(Component)]
struct BootUi;

#[derive(Component)]
struct MenuUi;

#[derive(Component)]
struct LoadingUi;

#[derive(Component)]
struct PauseUi;

#[derive(Resource)]
struct PhaseTimer(Timer);

pub fn register(app: &mut App) {
    app.init_state::<AppState>()
        .add_systems(OnEnter(AppState::Boot), enter_boot)
        .add_systems(Update, advance_boot.run_if(in_state(AppState::Boot)))
        .add_systems(OnExit(AppState::Boot), despawn_marked::<BootUi>)
        .add_systems(
            OnEnter(AppState::MainMenu),
            (sandbox::teardown_sandbox, enter_menu).chain(),
        )
        .add_systems(Update, menu_input.run_if(in_state(AppState::MainMenu)))
        .add_systems(OnExit(AppState::MainMenu), despawn_marked::<MenuUi>)
        .add_systems(OnEnter(AppState::Loading), enter_loading)
        .add_systems(Update, advance_loading.run_if(in_state(AppState::Loading)))
        .add_systems(OnExit(AppState::Loading), despawn_marked::<LoadingUi>)
        .add_systems(Update, pause_input.run_if(in_state(AppState::Sandbox)))
        .add_systems(OnEnter(AppState::Paused), enter_pause)
        .add_systems(Update, pause_menu_input.run_if(in_state(AppState::Paused)))
        .add_systems(OnExit(AppState::Paused), despawn_marked::<PauseUi>);
}

fn overlay<M: Component>(commands: &mut Commands, marker: M, lines: &str, accent: Color) {
    commands.spawn((
        marker,
        Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        GlobalZIndex(40),
        children![(
            Node {
                padding: UiRect::all(px(24)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.96)),
            BorderColor::all(accent),
            children![(
                Text::new(lines.to_string()),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.93, 1.0)),
            )],
        )],
    ));
}

fn enter_boot(mut commands: Commands) {
    overlay(
        &mut commands,
        BootUi,
        "OBSERVED 2\nFacility Sandbox\n\nbooting…",
        Color::srgba(0.4, 0.8, 1.0, 0.6),
    );
    commands.insert_resource(PhaseTimer(Timer::from_seconds(0.8, TimerMode::Once)));
}

fn advance_boot(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut timer: ResMut<PhaseTimer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if timer.0.tick(time.delta()).is_finished() || keyboard.just_pressed(KeyCode::Enter) {
        next.set(AppState::MainMenu);
    }
}

fn enter_menu(mut commands: Commands) {
    overlay(
        &mut commands,
        MenuUi,
        "FACILITY SANDBOX\n\nMove four players and the power cell to the exit.\n\n\
         ENTER  start\nESC    quit",
        Color::srgba(0.4, 0.8, 1.0, 0.6),
    );
}

fn menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        next.set(AppState::Loading);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn enter_loading(mut commands: Commands, mut built: ResMut<SandboxBuilt>) {
    overlay(
        &mut commands,
        LoadingUi,
        "Loading facility…",
        Color::srgba(0.4, 0.8, 1.0, 0.6),
    );
    sandbox::build_sandbox(&mut commands, &mut built);
    commands.insert_resource(PhaseTimer(Timer::from_seconds(0.6, TimerMode::Once)));
}

fn advance_loading(
    time: Res<Time>,
    mut timer: ResMut<PhaseTimer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if timer.0.tick(time.delta()).is_finished() {
        next.set(AppState::Sandbox);
    }
}

fn pause_input(keyboard: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next.set(AppState::Paused);
    }
}

fn enter_pause(mut commands: Commands) {
    overlay(
        &mut commands,
        PauseUi,
        "PAUSED\n\nESC  resume\nM    return to main menu",
        Color::srgba(1.0, 0.8, 0.3, 0.6),
    );
}

fn pause_menu_input(keyboard: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next.set(AppState::Sandbox);
    }
    if keyboard.just_pressed(KeyCode::KeyM) {
        next.set(AppState::MainMenu);
    }
}

fn despawn_marked<T: Component>(mut commands: Commands, query: Query<Entity, With<T>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
