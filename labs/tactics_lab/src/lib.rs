//! Turn-based tactical variant of the Observed facility.
//!
//! A squad of units moves cell to cell across a solved hex facility, spending
//! action points; when the turn ends, unobserved structure re-collapses through
//! the same observation-safe relayout the shipped game uses. See [`sim`] for the
//! rules and what is reused rather than rewritten, and [`settings`] for what a
//! player configures before a match.
//!
//! The lab opens on a match setup screen and returns to it on `R`, which is the
//! reset contract every lab in this workspace keeps: no leaked entities, no
//! restart needed.

pub mod capture;
pub mod settings;
pub mod sim;
pub mod view;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use observed_core::PlayerId;
use observed_hex::HexCoord;

use settings::MatchSettings;
use sim::action::TacticsAction;
use sim::unit::PLAYER_TEAM;
use sim::{TacticsGame, TurnPhase};
use view::board::DrawReport;
use view::camera::BoardCamera;
use view::hud::HudButton;
use view::setup::{SetupRequest, SetupRoot};
use view::{BoardVisual, HudRoot, ViewMode};

const WINDOW_WIDTH: f32 = 1600.0;
const WINDOW_HEIGHT: f32 = 1000.0;
const MIN_ZOOM: f32 = 0.15;
const MAX_ZOOM: f32 = 3.0;

/// Which screen is up.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum AppState {
    /// Configuring the match.
    #[default]
    Setup,
    /// Playing it.
    Play,
}

/// The settings the setup screen is editing. Separate from [`LabState`] so the
/// screen can run before any match exists.
#[derive(Resource, Default)]
pub struct LabSettings(pub MatchSettings);

/// A setup control was activated.
#[derive(Message)]
pub struct SetupRequested(pub SetupRequest);

/// The live match and everything presentation needs that is not part of it.
#[derive(Resource)]
pub struct LabState {
    pub game: TacticsGame,
    pub mode: ViewMode,
    /// Level the flat view is showing.
    pub level: u8,
    pub selected: Option<PlayerId>,
    pub zoom: f32,
    pub pan: Vec2,
    /// Set whenever the board needs redrawing. Presentation never mutates the
    /// match, so this is the only channel between the two.
    pub dirty: bool,
    pub report: DrawReport,
    /// The fit-everything framing, recomputed only when the board is rebuilt.
    ///
    /// Framing walks every cell in the lattice, which at full facility scale is
    /// five and a half thousand of them. Recomputing that per frame to service a
    /// scroll wheel is the kind of cost that only shows up on the largest board
    /// setting — where it is least affordable — so zoom and pan modify a cached
    /// framing instead.
    framing: (Transform, f32),
}

impl LabState {
    #[must_use]
    pub fn new(game: TacticsGame) -> Self {
        let selected = game
            .units
            .values()
            .find(|unit| unit.team == PLAYER_TEAM)
            .map(|unit| unit.id);
        Self {
            game,
            mode: ViewMode::default(),
            level: 0,
            selected,
            zoom: 1.0,
            pan: Vec2::ZERO,
            dirty: true,
            report: DrawReport::default(),
            framing: (Transform::IDENTITY, 1.0),
        }
    }

    /// The current camera framing.
    #[must_use]
    pub fn framing(&self) -> (Transform, f32) {
        self.framing
    }

    /// Move the selection to the next player unit that can still act, so a
    /// player is never left cycling through spent units.
    pub fn select_next(&mut self) {
        let candidates: Vec<PlayerId> = self
            .game
            .units
            .values()
            .filter(|unit| unit.team == PLAYER_TEAM && !unit.escaped)
            .map(|unit| unit.id)
            .collect();
        if candidates.is_empty() {
            self.selected = None;
            return;
        }
        let start = self
            .selected
            .and_then(|id| candidates.iter().position(|&candidate| candidate == id))
            .map_or(0, |index| index + 1);
        let with_points = candidates
            .iter()
            .cycle()
            .skip(start)
            .take(candidates.len())
            .find(|&&id| !self.game.legal_actions(id).is_empty())
            .copied();
        self.selected = with_points.or_else(|| candidates.get(start % candidates.len()).copied());
    }

    /// Walk the selected unit one step toward `cell`, spending what it can.
    ///
    /// Clicking a distant cell is a *route request*, not a teleport: the unit
    /// walks as far as its remaining points allow and stops. Anything else would
    /// make action points invisible, which is the resource the whole turn is
    /// about.
    pub fn move_selected_toward(&mut self, cell: HexCoord) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let mut moved = false;
        loop {
            let unit = self.game.units[&id];
            if unit.cell == cell {
                break;
            }
            let Some(next) = sim::adversary::advance_toward(&self.game.world, unit.cell, cell)
            else {
                break;
            };
            let Some(face) = sim::vision::exits(&self.game.world, unit.cell)
                .into_iter()
                .find(|&(_, destination)| destination == next)
                .map(|(face, _)| face)
            else {
                break;
            };
            if self.game.apply(id, TacticsAction::Move(face)).is_err() {
                break;
            }
            moved = true;
        }
        moved
    }
}

/// Launch the lab.
pub fn run() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Observed - Tactical Lab".to_string(),
            resolution: WindowResolution::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
            present_mode: PresentMode::AutoVsync,
            ..default()
        }),
        ..default()
    }))
    .add_plugins(observed_ui::FrontendWidgetsPlugin);
    configure(&mut app);
    capture::configure(&mut app);
    app.run();
}

/// Everything but the window, so tests and the capture harness can build the
/// same app without a renderer.
pub fn configure(app: &mut App) {
    app.init_state::<AppState>()
        .init_resource::<LabSettings>()
        .add_message::<SetupRequested>()
        .add_observer(view::setup::activate)
        .add_systems(OnEnter(AppState::Setup), enter_setup)
        .add_systems(OnExit(AppState::Setup), despawn::<SetupRoot>)
        .add_systems(OnEnter(AppState::Play), enter_play)
        .add_systems(
            OnExit(AppState::Play),
            (despawn::<BoardVisual>, despawn::<HudRoot>, leave_play),
        )
        .add_systems(
            Update,
            handle_setup_requests.run_if(in_state(AppState::Setup)),
        )
        .add_systems(
            Update,
            (
                keyboard_commands,
                hud_clicks,
                board_clicks,
                camera_controls,
                rebuild_board,
                refresh_hud,
            )
                .chain()
                .run_if(in_state(AppState::Play)),
        );
}

fn enter_setup(
    mut commands: Commands,
    settings: Res<LabSettings>,
    cameras: Query<Entity, With<BoardCamera>>,
) {
    for entity in cameras.iter() {
        commands.entity(entity).despawn();
    }
    commands.spawn((BoardCamera, Camera2d, Name::new("Setup camera")));
    view::setup::spawn(&mut commands, &settings.0);
}

fn handle_setup_requests(
    mut commands: Commands,
    mut requests: MessageReader<SetupRequested>,
    settings: Res<LabSettings>,
    roots: Query<Entity, With<SetupRoot>>,
    mut next: ResMut<NextState<AppState>>,
) {
    for request in requests.read() {
        match request.0 {
            SetupRequest::Changed => {
                // Rebuild rather than patch: every label is derived from the
                // settings, so there is no second copy to keep in step.
                for entity in roots.iter() {
                    commands.entity(entity).despawn();
                }
                view::setup::spawn(&mut commands, &settings.0);
            }
            SetupRequest::Start => next.set(AppState::Play),
        }
    }
}

fn enter_play(
    mut commands: Commands,
    settings: Res<LabSettings>,
    cameras: Query<Entity, With<BoardCamera>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Ok(game) = TacticsGame::new(settings.0) else {
        // A configuration the solver cannot satisfy sends the player back rather
        // than dropping them into a broken match.
        warn!("these settings did not solve; returning to setup");
        next.set(AppState::Setup);
        return;
    };
    for entity in cameras.iter() {
        commands.entity(entity).despawn();
    }
    commands.spawn((
        BoardCamera,
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection::default_3d()),
        Transform::default(),
        Name::new("Board camera"),
    ));
    commands.insert_resource(LabState::new(game));
    view::hud::spawn(&mut commands);
}

fn leave_play(mut commands: Commands) {
    commands.remove_resource::<LabState>();
}

fn despawn<T: Component>(mut commands: Commands, entities: Query<Entity, With<T>>) {
    for entity in entities.iter() {
        commands.entity(entity).despawn();
    }
}

/// Keyboard accelerators. Every one of these has a pointer equivalent — see the
/// note in [`view::hud`].
fn keyboard_commands(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<LabState>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        next.set(AppState::Setup);
        return;
    }
    let mut command = None;
    if keyboard.just_pressed(KeyCode::Space) {
        command = Some(HudButton::EndTurn);
    } else if keyboard.just_pressed(KeyCode::KeyV) {
        command = Some(HudButton::ToggleView);
    } else if keyboard.just_pressed(KeyCode::Tab) {
        command = Some(HudButton::NextUnit);
    } else if keyboard.just_pressed(KeyCode::BracketRight) {
        command = Some(HudButton::LevelUp);
    } else if keyboard.just_pressed(KeyCode::BracketLeft) {
        command = Some(HudButton::LevelDown);
    }
    if let Some(command) = command {
        apply_command(&mut state, command, &mut next);
    }
    // Number keys select a unit directly, which is the accelerator a tactics
    // player expects and costs nothing to offer.
    for (index, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ]
    .into_iter()
    .enumerate()
    {
        if keyboard.just_pressed(key) {
            let id = PlayerId(index as u16);
            if state.game.units.contains_key(&id) {
                state.selected = Some(id);
                state.dirty = true;
            }
        }
    }
}

fn hud_clicks(
    controls: Query<(&Interaction, &HudButton), Changed<Interaction>>,
    mut state: ResMut<LabState>,
    mut next: ResMut<NextState<AppState>>,
) {
    for (interaction, &button) in controls.iter() {
        if *interaction == Interaction::Pressed {
            apply_command(&mut state, button, &mut next);
        }
    }
}

/// One place every command is carried out, whichever input asked for it.
fn apply_command(state: &mut LabState, command: HudButton, next: &mut ResMut<NextState<AppState>>) {
    match command {
        HudButton::EndTurn => {
            state.game.end_turn();
            state.select_next();
        }
        HudButton::ToggleView => state.mode = state.mode.toggled(),
        HudButton::LevelUp => {
            let top = state.game.world.config.levels.saturating_sub(1);
            state.level = (state.level + 1).min(top);
        }
        HudButton::LevelDown => state.level = state.level.saturating_sub(1),
        HudButton::NextUnit => state.select_next(),
        HudButton::Restart => {
            next.set(AppState::Setup);
            return;
        }
    }
    state.dirty = true;
}

/// Clicking a cell moves the selected unit toward it.
fn board_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
    interactions: Query<&Interaction, With<HudButton>>,
    mut state: ResMut<LabState>,
) {
    if !mouse.just_pressed(MouseButton::Left) || state.game.phase == TurnPhase::Finished {
        return;
    }
    // A click that landed on a HUD control is not a click on the board.
    if interactions
        .iter()
        .any(|interaction| *interaction != Interaction::None)
    {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, transform)) = cameras.single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(transform, cursor) else {
        return;
    };
    let (mode, level) = (state.mode, state.level);
    let Some(cell) = view::board::cell_at_ray(
        &state.game,
        mode,
        level,
        ray.origin,
        ray.direction.as_vec3(),
    ) else {
        return;
    };
    // Clicking a unit selects it; clicking anywhere else asks the selection to
    // walk there.
    if let Some(unit) = state
        .game
        .units
        .values()
        .find(|unit| unit.cell == cell && unit.team == PLAYER_TEAM && !unit.escaped)
        .map(|unit| unit.id)
    {
        state.selected = Some(unit);
        state.dirty = true;
        return;
    }
    if state.move_selected_toward(cell) {
        state.dirty = true;
    }
}

fn camera_controls(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut state: ResMut<LabState>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<BoardCamera>>,
) {
    if scroll.delta.y != 0.0 {
        state.zoom = (state.zoom * (1.0 - scroll.delta.y * 0.1)).clamp(MIN_ZOOM, MAX_ZOOM);
    }
    if mouse.pressed(MouseButton::Right) {
        let scale = state.zoom * 0.6;
        state.pan += Vec2::new(-motion.delta.x, motion.delta.y) * scale;
    }
    let framing = state.framing();
    for (mut transform, mut projection) in &mut cameras {
        view::camera::apply(
            &mut transform,
            &mut projection,
            framing,
            state.zoom,
            state.pan,
        );
    }
}

fn rebuild_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<LabState>,
    existing: Query<Entity, With<BoardVisual>>,
) {
    if !state.dirty {
        return;
    }
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    let (mode, level) = (state.mode, state.level);
    state.report = view::board::build(
        &mut commands,
        &mut meshes,
        &mut materials,
        &state.game,
        mode,
        level,
    );
    // The lattice only changes shape when the board is rebuilt, so this is the
    // one place the framing can go stale.
    state.framing = view::camera::frame(&state.game, mode, level);
    state.dirty = false;
}

fn refresh_hud(
    state: Res<LabState>,
    mut status: Query<&mut Text, (With<view::hud::StatusText>, Without<view::hud::SquadText>)>,
    mut squad: Query<&mut Text, (With<view::hud::SquadText>, Without<view::hud::StatusText>)>,
) {
    for mut text in &mut status {
        **text = view::hud::status_line(&state.game, state.mode, state.level);
    }
    for mut text in &mut squad {
        **text = view::hud::squad_line(&state.game, state.selected);
    }
}
