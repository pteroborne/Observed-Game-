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

use bevy::camera::Viewport;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::window::{CursorIcon, PresentMode, SystemCursorIcon, WindowResolution};
use observed_content::ArchitectureRegister;
use observed_core::PlayerId;
use observed_hex::HexCoord;
use observed_match::hex_wfc::{HexMatchContent, HexWfcGeometrySnapshot};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use settings::MatchSettings;
use sim::action::TacticsAction;
use sim::unit::PLAYER_TEAM;
use sim::{MovePreview, TacticsGame, TurnPhase, TurnResolution};
use view::board::DrawReport;
use view::camera::BoardCamera;
use view::hud::HudButton;
use view::setup::{SetupRequest, SetupRoot};
use view::{BoardVisual, HudRoot, ViewMode};

const WINDOW_WIDTH: f32 = 1600.0;
const WINDOW_HEIGHT: f32 = 1000.0;
const MIN_ZOOM: f32 = 0.15;
const MAX_ZOOM: f32 = 3.0;
const COMMAND_DOCK_WIDTH: f32 = 380.0;

/// Which screen is up.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum AppState {
    /// Configuring the match.
    #[default]
    Setup,
    /// Visible hand-off between configuration and the potentially expensive solve.
    Loading,
    /// Playing it.
    Play,
}

/// The settings the setup screen is editing. Separate from [`LabState`] so the
/// screen can run before any match exists.
#[derive(Resource, Default)]
pub struct LabSettings(pub MatchSettings);

#[derive(Resource, Default)]
pub struct LabMessage(pub Option<String>);

#[derive(Component)]
struct LoadingRoot;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum OverlayMode {
    #[default]
    None,
    Pause,
    Help,
    Results,
}

#[derive(Resource, Default)]
struct PlayOverlay(OverlayMode);

#[derive(Component)]
struct OverlayRoot;

#[derive(Component, Clone, Copy)]
enum OverlayAction {
    Resume,
    Help,
    Setup,
}

/// Committed authored content used only to project the board. A load failure is
/// retained so the setup screen can report it rather than silently drawing a
/// different facility.
#[derive(Resource)]
pub struct AuthoredBoardContent(pub Result<Arc<HexMatchContent>, String>);

impl Default for AuthoredBoardContent {
    fn default() -> Self {
        let cwd = PathBuf::from("assets/tiles");
        let base = if cwd.exists() {
            cwd
        } else {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
        };
        let registers = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
        Self(HexMatchContent::load(&base, &registers).map(Arc::new))
    }
}

/// Presentation-owned authored geometry kept in generation lockstep with the
/// pure tactics world.
pub struct BoardGeometry {
    pub content: Arc<HexMatchContent>,
    pub snapshot: HexWfcGeometrySnapshot,
}

pub struct PendingMove {
    unit: PlayerId,
    steps: VecDeque<sim::MoveStep>,
    timer: Timer,
}

impl BoardGeometry {
    fn new(game: &TacticsGame, content: Arc<HexMatchContent>) -> Result<Self, String> {
        let snapshot = HexWfcGeometrySnapshot::project_with_rooms(
            &game.world,
            content.cells(),
            content.rooms(),
        )
        .map_err(|error| format!("authored board projection failed: {error:?}"))?;
        Ok(Self { content, snapshot })
    }

    fn apply_resolution(
        &mut self,
        game: &TacticsGame,
        resolution: &TurnResolution,
    ) -> Result<(), String> {
        let Some(logical) = resolution.relayout.as_ref() else {
            return Ok(());
        };
        let projected = self.snapshot.project_delta_with_rooms(
            &game.world,
            logical,
            self.content.cells(),
            self.content.rooms(),
        );
        match projected.and_then(|delta| self.snapshot.apply_delta(&delta)) {
            Ok(()) => Ok(()),
            Err(incremental) => {
                self.snapshot = HexWfcGeometrySnapshot::project_with_rooms(
                    &game.world,
                    self.content.cells(),
                    self.content.rooms(),
                )
                .map_err(|full| {
                    format!(
                        "board update failed ({incremental:?}); full projection failed ({full:?})"
                    )
                })?;
                Ok(())
            }
        }
    }
}

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
    pub geometry: Option<BoardGeometry>,
    pub hovered: Option<HexCoord>,
    pub preview: Option<MovePreview>,
    pub overlay_dirty: bool,
    pub notice: String,
    pub pending_move: Option<PendingMove>,
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
            geometry: None,
            hovered: None,
            preview: None,
            overlay_dirty: true,
            notice: String::new(),
            pending_move: None,
            framing: (Transform::IDENTITY, 1.0),
        }
    }

    fn attach_content(&mut self, content: Arc<HexMatchContent>) -> Result<(), String> {
        self.geometry = Some(BoardGeometry::new(&self.game, content)?);
        Ok(())
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
        let preview = self.game.preview_move(id, cell);
        let steps = preview.executable_steps().to_vec();
        for step in &steps {
            if self.game.apply(id, TacticsAction::Move(step.face)).is_err() {
                return false;
            }
        }
        if let Some(unit) = self.game.units.get(&id) {
            self.level = unit.cell.level;
        }
        !steps.is_empty()
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
    app.insert_resource(ClearColor(observed_style::schematic_screen()))
        .init_state::<AppState>()
        .init_resource::<LabSettings>()
        .init_resource::<LabMessage>()
        .init_resource::<PlayOverlay>()
        .init_resource::<AuthoredBoardContent>()
        .init_resource::<observed_cutaway::TileMeshCache>()
        .add_message::<SetupRequested>()
        .add_observer(view::setup::activate)
        .add_systems(OnEnter(AppState::Setup), enter_setup)
        .add_systems(OnExit(AppState::Setup), despawn::<SetupRoot>)
        .add_systems(OnEnter(AppState::Loading), enter_loading)
        .add_systems(OnExit(AppState::Loading), despawn::<LoadingRoot>)
        .add_systems(OnEnter(AppState::Play), enter_play)
        .add_systems(
            OnExit(AppState::Play),
            (
                despawn::<BoardVisual>,
                despawn::<view::BoardOverlay>,
                despawn::<HudRoot>,
                despawn::<OverlayRoot>,
                leave_play,
            ),
        )
        .add_systems(
            Update,
            handle_setup_requests.run_if(in_state(AppState::Setup)),
        )
        .add_systems(Update, begin_loading.run_if(in_state(AppState::Loading)))
        .add_systems(Update, sync_screen_cursor)
        .add_systems(
            Update,
            (toggle_pause, overlay_clicks, show_results)
                .chain()
                .run_if(in_state(AppState::Play)),
        )
        .add_systems(
            Update,
            (
                keyboard_commands,
                hud_clicks,
                advance_pending_move,
                sync_camera_viewport,
                update_board_hover,
                board_clicks,
                camera_controls,
                rebuild_board,
                view::overlay::rebuild,
                refresh_action_buttons,
                refresh_hud,
            )
                .chain()
                .run_if(in_state(AppState::Play))
                .run_if(play_accepts_input),
        );
}

fn enter_setup(
    mut commands: Commands,
    settings: Res<LabSettings>,
    message: Res<LabMessage>,
    cameras: Query<Entity, With<BoardCamera>>,
) {
    for entity in cameras.iter() {
        commands.entity(entity).despawn();
    }
    commands.spawn((BoardCamera, Camera2d, Name::new("Setup camera")));
    view::setup::spawn(&mut commands, &settings.0, message.0.as_deref());
}

fn handle_setup_requests(
    mut commands: Commands,
    mut requests: MessageReader<SetupRequested>,
    settings: Res<LabSettings>,
    message: Res<LabMessage>,
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
                view::setup::spawn(&mut commands, &settings.0, message.0.as_deref());
            }
            SetupRequest::Start => next.set(AppState::Loading),
        }
    }
}

fn enter_loading(mut commands: Commands) {
    commands
        .spawn((
            LoadingRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(observed_style::schematic_screen()),
            Name::new("Tactics loading"),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("SOLVING FACILITY // PROJECTING AUTHORED DECKS"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn begin_loading(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Play);
}

fn play_accepts_input(overlay: Res<PlayOverlay>) -> bool {
    overlay.0 == OverlayMode::None
}

fn sync_screen_cursor(
    app_state: Res<State<AppState>>,
    overlay: Res<PlayOverlay>,
    windows: Query<Entity, With<Window>>,
    controls: Query<(&Interaction, Has<InteractionDisabled>), With<Button>>,
    mut commands: Commands,
) {
    if *app_state.get() == AppState::Play && overlay.0 == OverlayMode::None {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let icon = if *app_state.get() == AppState::Loading {
        SystemCursorIcon::Progress
    } else {
        controls
            .iter()
            .find(|(interaction, _)| **interaction != Interaction::None)
            .map_or(SystemCursorIcon::Default, |(_, disabled)| {
                if disabled {
                    SystemCursorIcon::NotAllowed
                } else {
                    SystemCursorIcon::Pointer
                }
            })
    };
    commands.entity(window).insert(CursorIcon::System(icon));
}

fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    mut overlay: ResMut<PlayOverlay>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) || overlay.0 == OverlayMode::Results {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    overlay.0 = match overlay.0 {
        OverlayMode::None => OverlayMode::Pause,
        OverlayMode::Pause | OverlayMode::Help => OverlayMode::None,
        OverlayMode::Results => OverlayMode::Results,
    };
    if overlay.0 != OverlayMode::None {
        spawn_overlay(&mut commands, overlay.0, None);
    }
}

fn overlay_clicks(
    interactions: Query<(&Interaction, &OverlayAction), Changed<Interaction>>,
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    mut overlay: ResMut<PlayOverlay>,
    mut next: ResMut<NextState<AppState>>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        for root in &roots {
            commands.entity(root).despawn();
        }
        match action {
            OverlayAction::Resume => overlay.0 = OverlayMode::None,
            OverlayAction::Help => {
                overlay.0 = OverlayMode::Help;
                spawn_overlay(&mut commands, OverlayMode::Help, None);
            }
            OverlayAction::Setup => {
                overlay.0 = OverlayMode::None;
                next.set(AppState::Setup);
            }
        }
    }
}

fn show_results(state: Res<LabState>, mut commands: Commands, mut overlay: ResMut<PlayOverlay>) {
    if state.game.status == sim::MatchStatus::Running || overlay.0 != OverlayMode::None {
        return;
    }
    overlay.0 = OverlayMode::Results;
    spawn_overlay(&mut commands, OverlayMode::Results, Some(state.game.status));
}

fn spawn_overlay(commands: &mut Commands, mode: OverlayMode, status: Option<sim::MatchStatus>) {
    let title = match mode {
        OverlayMode::Pause => "MATCH PAUSED",
        OverlayMode::Help => "TACTICAL PROTOCOL",
        OverlayMode::Results => match status {
            Some(sim::MatchStatus::Escaped) => "SQUAD ESCAPED",
            Some(sim::MatchStatus::Outrun) => "RIVAL SQUAD ESCAPED",
            _ => "MATCH COMPLETE",
        },
        OverlayMode::None => return,
    };
    commands
        .spawn((
            OverlayRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            ZIndex(20),
            Name::new(title),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(520.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(28.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(observed_style::schematic_screen()),
                BorderColor::all(
                    observed_style::tactics(observed_style::TacticsRole::DevGrid).base_color,
                ),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(title),
                    TextFont {
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                if mode == OverlayMode::Help {
                    panel.spawn((
                        Text::new(
                            "Hover a deck cell to preview its route and AP limit.\n\
                             Click a unit to select it; click a route to commit.\n\
                             Cyan = reachable, amber = next-turn limit, red = blocked.\n\
                             Right-drag pans; wheel zooms; V changes deck/overview.",
                        ),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                }
                if matches!(mode, OverlayMode::Pause | OverlayMode::Help) {
                    spawn_overlay_button(panel, OverlayAction::Resume, "Resume");
                    if mode == OverlayMode::Pause {
                        spawn_overlay_button(panel, OverlayAction::Help, "Help / legend");
                    }
                }
                spawn_overlay_button(panel, OverlayAction::Setup, "Return to setup");
            });
        });
}

fn spawn_overlay_button(
    parent: &mut ChildSpawnerCommands,
    action: OverlayAction,
    label: &'static str,
) {
    parent
        .spawn((
            action,
            Button,
            Node {
                min_height: Val::Px(48.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.07, 0.10, 0.16)),
            BorderColor::all(
                observed_style::tactics(observed_style::TacticsRole::DevGrid).base_color,
            ),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font_size: 17.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
}

fn enter_play(
    mut commands: Commands,
    settings: Res<LabSettings>,
    content: Res<AuthoredBoardContent>,
    mut message: ResMut<LabMessage>,
    mut overlay: ResMut<PlayOverlay>,
    cameras: Query<Entity, With<BoardCamera>>,
    mut next: ResMut<NextState<AppState>>,
) {
    overlay.0 = OverlayMode::None;
    let Ok(game) = TacticsGame::new(settings.0) else {
        // A configuration the solver cannot satisfy sends the player back rather
        // than dropping them into a broken match.
        let detail = String::from("These settings did not produce a playable facility.");
        warn!("{detail}");
        message.0 = Some(detail);
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
    let mut state = LabState::new(game);
    match &content.0 {
        Ok(content) => {
            if let Err(detail) = state.attach_content(Arc::clone(content)) {
                warn!("{detail}");
                message.0 = Some(detail);
                next.set(AppState::Setup);
                return;
            }
        }
        Err(detail) => {
            warn!("{detail}");
            message.0 = Some(detail.clone());
            next.set(AppState::Setup);
            return;
        }
    }
    message.0 = None;
    state.level = state
        .selected
        .and_then(|id| state.game.units.get(&id))
        .map_or(0, |unit| unit.cell.level);
    commands.insert_resource(state);
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
    if state.pending_move.is_some() {
        return;
    }
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
    if state.pending_move.is_some() {
        return;
    }
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
            let resolution = state.game.end_turn_detailed();
            if let Some(mut geometry) = state.geometry.take() {
                if let Err(detail) = geometry.apply_resolution(&state.game, &resolution) {
                    state.notice = detail;
                } else {
                    state.notice = match resolution.outcome {
                        sim::relayout::ShiftOutcome::Committed => {
                            String::from("Facility shift committed")
                        }
                        sim::relayout::ShiftOutcome::Held => String::from("Facility shift held"),
                        sim::relayout::ShiftOutcome::NothingToShift => {
                            String::from("No facility shift")
                        }
                    };
                }
                state.geometry = Some(geometry);
            }
            state.select_next();
        }
        HudButton::ToggleView => {
            state.mode = state.mode.toggled();
            if state.mode == ViewMode::Deck
                && let Some(id) = state.selected
            {
                state.level = state.game.units[&id].cell.level;
            }
        }
        HudButton::LevelUp => {
            let top = state.game.world.config.levels.saturating_sub(1);
            state.level = (state.level + 1).min(top);
        }
        HudButton::LevelDown => state.level = state.level.saturating_sub(1),
        HudButton::NextUnit => state.select_next(),
        HudButton::Interact
        | HudButton::DeployAnchor
        | HudButton::RecoverAnchor
        | HudButton::DeployPad => {
            let action = match command {
                HudButton::Interact => TacticsAction::Interact,
                HudButton::DeployAnchor => TacticsAction::DeployAnchor,
                HudButton::RecoverAnchor => TacticsAction::RecoverAnchor,
                HudButton::DeployPad => TacticsAction::DeployPad,
                _ => unreachable!("matched contextual actions"),
            };
            if let Some(id) = state.selected {
                let availability = state.game.action_availability(id, action);
                if availability.enabled {
                    match state.game.apply(id, action) {
                        Ok(()) => {
                            state.notice = format!("{action:?} accepted - {} AP", availability.cost)
                        }
                        Err(refusal) => state.notice = format!("Action refused: {refusal:?}"),
                    }
                } else {
                    state.notice = format!("Action unavailable: {:?}", availability.refusal);
                }
            }
        }
        HudButton::Restart => {
            next.set(AppState::Setup);
            return;
        }
    }
    state.dirty = true;
    state.hovered = None;
    state.preview = None;
    state.overlay_dirty = true;
}

fn sync_camera_viewport(
    windows: Query<&Window>,
    mut cameras: Query<&mut Camera, With<BoardCamera>>,
) {
    let (Ok(window), Ok(mut camera)) = (windows.single(), cameras.single_mut()) else {
        return;
    };
    let dock = (COMMAND_DOCK_WIDTH * window.scale_factor()) as u32;
    let width = window.physical_width().saturating_sub(dock).max(1);
    let height = window.physical_height().max(1);
    let size = UVec2::new(width, height);
    if camera
        .viewport
        .as_ref()
        .map(|viewport| viewport.physical_size)
        != Some(size)
    {
        camera.viewport = Some(Viewport {
            physical_position: UVec2::ZERO,
            physical_size: size,
            depth: 0.0..1.0,
        });
    }
}

/// Keep pointer affordance, hovered cell, and route preview in lockstep without
/// touching authored board geometry.
fn update_board_hover(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<(Entity, &Window)>,
    cameras: Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
    controls: Query<(&Interaction, Has<InteractionDisabled>), With<Button>>,
    mut state: ResMut<LabState>,
    mut commands: Commands,
) {
    let Ok((window_entity, window)) = windows.single() else {
        return;
    };
    let control_hover = controls
        .iter()
        .find(|(interaction, _)| **interaction != Interaction::None)
        .map(|(_, disabled)| disabled);
    let mut hovered = None;
    let mut preview = None;
    let icon =
        if state.pending_move.is_some() {
            SystemCursorIcon::Progress
        } else if mouse.pressed(MouseButton::Right) {
            SystemCursorIcon::Grabbing
        } else if let Some(disabled) = control_hover {
            if disabled {
                SystemCursorIcon::NotAllowed
            } else {
                SystemCursorIcon::Pointer
            }
        } else if let (Some(cursor), Ok((camera, transform))) =
            (window.cursor_position(), cameras.single())
        {
            if let Ok(ray) = camera.viewport_to_world(transform, cursor) {
                hovered = view::board::cell_at_ray(
                    &state.game,
                    state.mode,
                    state.level,
                    ray.origin,
                    ray.direction.as_vec3(),
                );
            }
            match hovered {
                None => SystemCursorIcon::Default,
                Some(_) if state.mode == ViewMode::Overview => SystemCursorIcon::Pointer,
                Some(cell)
                    if state.game.units.values().any(|unit| {
                        unit.cell == cell && unit.team == PLAYER_TEAM && !unit.escaped
                    }) =>
                {
                    SystemCursorIcon::Pointer
                }
                Some(cell) => {
                    preview = state
                        .selected
                        .map(|selected| state.game.preview_move(selected, cell));
                    if preview.as_ref().is_some_and(MovePreview::can_move) {
                        SystemCursorIcon::Crosshair
                    } else {
                        SystemCursorIcon::NotAllowed
                    }
                }
            }
        } else {
            SystemCursorIcon::Default
        };
    if hovered != state.hovered || preview != state.preview {
        state.hovered = hovered;
        state.preview = preview;
        state.overlay_dirty = true;
    }
    commands
        .entity(window_entity)
        .insert(CursorIcon::System(icon));
}

/// Clicking a cell moves the selected unit toward it.
fn board_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    interactions: Query<&Interaction, With<HudButton>>,
    mut state: ResMut<LabState>,
) {
    if !mouse.just_pressed(MouseButton::Left)
        || state.game.phase == TurnPhase::Finished
        || state.pending_move.is_some()
    {
        return;
    }
    // A click that landed on a HUD control is not a click on the board.
    if interactions
        .iter()
        .any(|interaction| *interaction != Interaction::None)
    {
        return;
    }
    let Some(cell) = state.hovered else {
        return;
    };
    if state.mode == ViewMode::Overview {
        state.mode = ViewMode::Deck;
        state.level = cell.level;
        state.pan = Vec2::ZERO;
        state.dirty = true;
        state.overlay_dirty = true;
        return;
    }
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
        state.level = cell.level;
        state.notice = format!("Unit {} selected", unit.0);
        state.dirty = true;
        state.overlay_dirty = true;
        return;
    }
    let preview = state.preview.clone();
    if let (Some(id), Some(preview)) = (state.selected, preview.as_ref())
        && preview.can_move()
    {
        state.notice = format!(
            "Move accepted - {} step(s), {} AP",
            preview.affordable_steps, preview.action_point_cost
        );
        state.pending_move = Some(PendingMove {
            unit: id,
            steps: preview.executable_steps().iter().copied().collect(),
            timer: Timer::new(Duration::from_millis(120), TimerMode::Repeating),
        });
        state.overlay_dirty = true;
    } else if let Some(preview) = preview {
        state.notice = format!(
            "Move refused: {:?}",
            preview.refusal.unwrap_or(sim::action::Refusal::Blocked)
        );
    }
}

fn advance_pending_move(time: Res<Time>, mut state: ResMut<LabState>) {
    let Some(mut pending) = state.pending_move.take() else {
        return;
    };
    pending.timer.tick(time.delta());
    if !pending.timer.just_finished() {
        state.pending_move = Some(pending);
        return;
    }
    let Some(step) = pending.steps.pop_front() else {
        return;
    };
    match state
        .game
        .apply(pending.unit, TacticsAction::Move(step.face))
    {
        Ok(()) => {
            state.level = step.to.level;
            state.notice = format!(
                "Unit {} moved to ({},{},L{})",
                pending.unit.0, step.to.q, step.to.r, step.to.level
            );
            state.dirty = true;
            state.overlay_dirty = true;
        }
        Err(refusal) => {
            state.notice = format!("Route interrupted: {refusal:?}");
            return;
        }
    }
    if pending.steps.is_empty() {
        state.hovered = None;
        state.preview = None;
        state.overlay_dirty = true;
    } else {
        state.pending_move = Some(pending);
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
    mut cache: ResMut<observed_cutaway::TileMeshCache>,
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
        state.geometry.as_ref(),
        &mut cache,
        view::board::BoardView {
            selected: state.selected,
            mode,
            level,
        },
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
        let mut value = view::hud::status_line(&state.game, state.mode, state.level);
        if let Some(preview) = state.preview.as_ref() {
            value.push_str(&format!(
                "\nRoute: {} step(s), {} AP, stops at ({},{},L{}){}",
                preview.affordable_steps,
                preview.action_point_cost,
                preview.stopping_cell.q,
                preview.stopping_cell.r,
                preview.stopping_cell.level,
                if preview.reaches_destination && preview.affordable_steps == preview.steps.len() {
                    ""
                } else {
                    " - continues"
                }
            ));
        }
        if !state.notice.is_empty() {
            value.push('\n');
            value.push_str(&state.notice);
        }
        **text = value;
    }
    for mut text in &mut squad {
        **text = view::hud::squad_line(&state.game, state.selected);
    }
}

fn refresh_action_buttons(
    mut commands: Commands,
    state: Res<LabState>,
    buttons: Query<(Entity, &HudButton, Has<InteractionDisabled>)>,
) {
    for (entity, button, disabled) in &buttons {
        let action = match button {
            HudButton::Interact => Some(TacticsAction::Interact),
            HudButton::DeployAnchor => Some(TacticsAction::DeployAnchor),
            HudButton::RecoverAnchor => Some(TacticsAction::RecoverAnchor),
            HudButton::DeployPad => Some(TacticsAction::DeployPad),
            _ => None,
        };
        let should_disable = action.is_some_and(|action| {
            state
                .selected
                .is_none_or(|id| !state.game.action_availability(id, action).enabled)
        });
        if should_disable && !disabled {
            commands.entity(entity).insert(InteractionDisabled);
        } else if !should_disable && disabled {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
    }
}
