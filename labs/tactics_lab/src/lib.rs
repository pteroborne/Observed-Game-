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

pub mod bot;
#[cfg(not(target_arch = "wasm32"))]
pub mod capture;
pub mod settings;
pub mod sim;
pub mod view;

use bevy::camera::Viewport;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::window::{CursorIcon, PresentMode, SystemCursorIcon, WindowResolution};
use observed_content::ArchitectureRegister;
use observed_core::PlayerId;
use observed_hex::HexCoord;
use observed_match::hex_wfc::{HexMatchContent, HexWfcGeometrySnapshot};
use std::collections::VecDeque;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use settings::MatchSettings;
use sim::action::TacticsAction;
use sim::unit::PLAYER_TEAM;
use sim::{MovePreview, TacticsEvent, TacticsGame, TurnPhase, TurnResolution};
use view::board::DrawReport;
use view::camera::BoardCamera;
use view::hud::{CommandDock, DesktopHudContent, HudButton, MobileHudContent, MobileMorePanel};
use view::setup::{SetupRequest, SetupRoot};
use view::{BoardVisual, HudRoot, ViewMode};

const WINDOW_WIDTH: f32 = 1600.0;
const WINDOW_HEIGHT: f32 = 1000.0;
const MIN_ZOOM: f32 = 0.15;
const MAX_ZOOM: f32 = 3.0;
const CAMERA_NUDGE: f32 = 18.0;
const BOT_BEAT_SECONDS: f32 = 0.45;
const TOUCH_DRAG_THRESHOLD: f32 = 18.0;
const TOUCH_FEEDBACK_SECONDS: f32 = 1.25;

#[derive(Clone, Copy, Debug)]
pub struct CameraPose {
    pub zoom: f32,
    pub pan: Vec2,
}

impl Default for CameraPose {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

#[derive(Resource, Default)]
struct MapPointerCapture {
    panning: bool,
}

/// Presentation-only gesture state. Touch input changes the same camera pose
/// and invokes the same cell command path as mouse input; it never enters the
/// deterministic tactics simulation or replay digest.
#[derive(Resource)]
struct MapTouchGesture {
    active: bool,
    moved: bool,
    max_contacts: usize,
    feedback: Timer,
}

impl Default for MapTouchGesture {
    fn default() -> Self {
        Self {
            active: false,
            moved: false,
            max_contacts: 0,
            feedback: Timer::from_seconds(0.0, TimerMode::Once),
        }
    }
}

#[derive(Resource)]
struct BotSpectator {
    enabled: bool,
    step_requested: bool,
    cadence: Timer,
    last: String,
}

impl Default for BotSpectator {
    fn default() -> Self {
        Self {
            enabled: false,
            step_requested: false,
            cadence: Timer::from_seconds(BOT_BEAT_SECONDS, TimerMode::Repeating),
            last: String::new(),
        }
    }
}

/// Full-window camera for UI. The board camera owns an inset viewport, so it
/// cannot also be allowed to define the HUD's layout coordinate space.
#[derive(Component)]
struct HudCamera;

#[derive(SystemParam)]
struct CameraInput<'w, 's> {
    mouse: Res<'w, ButtonInput<MouseButton>>,
    motion: Res<'w, AccumulatedMouseMotion>,
    scroll: Res<'w, AccumulatedMouseScroll>,
    windows: Query<'w, 's, &'static Window>,
    viewports: Query<'w, 's, &'static Camera, With<BoardCamera>>,
}

#[derive(SystemParam)]
struct TacticsCameras<'w, 's> {
    board: Query<'w, 's, Entity, With<BoardCamera>>,
    hud: Query<'w, 's, Entity, With<HudCamera>>,
}

#[derive(SystemParam)]
struct BoardHoverInput<'w, 's> {
    time: Res<'w, Time>,
    windows: Query<'w, 's, (Entity, &'static Window)>,
    cameras: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<BoardCamera>>,
    controls: Query<'w, 's, (&'static Interaction, Has<InteractionDisabled>), With<Button>>,
    capture: Res<'w, MapPointerCapture>,
    touch_gesture: ResMut<'w, MapTouchGesture>,
}

type HudRootNode = (
    With<HudRoot>,
    Without<CommandDock>,
    Without<DesktopHudContent>,
    Without<MobileHudContent>,
);
type CommandDockNode = (
    With<CommandDock>,
    Without<HudRoot>,
    Without<DesktopHudContent>,
    Without<MobileHudContent>,
);
type DesktopHudNode = (
    With<DesktopHudContent>,
    Without<HudRoot>,
    Without<CommandDock>,
    Without<MobileHudContent>,
);
type MobileHudNode = (
    With<MobileHudContent>,
    Without<HudRoot>,
    Without<CommandDock>,
    Without<DesktopHudContent>,
);
type StatusTextNode = (
    With<view::hud::StatusText>,
    Without<view::hud::SquadText>,
    Without<view::hud::MobileStatusText>,
    Without<view::hud::MobilePromptText>,
);
type SquadTextNode = (
    With<view::hud::SquadText>,
    Without<view::hud::StatusText>,
    Without<view::hud::MobileStatusText>,
    Without<view::hud::MobilePromptText>,
);
type MobileStatusTextNode = (
    With<view::hud::MobileStatusText>,
    Without<view::hud::StatusText>,
    Without<view::hud::SquadText>,
    Without<view::hud::MobilePromptText>,
);
type MobilePromptTextNode = (
    With<view::hud::MobilePromptText>,
    Without<view::hud::StatusText>,
    Without<view::hud::SquadText>,
    Without<view::hud::MobileStatusText>,
);

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
        #[cfg(target_arch = "wasm32")]
        {
            let registers = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
            Self(
                HexMatchContent::from_embedded(
                    include_str!("../../../assets/tiles/compiled_catalog.ron"),
                    include_str!("../../../assets/tiles/compiled_catalog.sha256"),
                    include_str!("../../../assets/tiles/composition_profile.ron"),
                    include_str!("../../../assets/tiles/composition_profile.sha256"),
                    &registers,
                )
                .map(Arc::new),
            )
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
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
    pub deck_camera: CameraPose,
    pub map_camera: CameraPose,
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
    /// A presentation-only pause over an already-resolved pad traversal.
    pub teleport: Option<view::portals::TeleportPresentation>,
    /// The fit-everything framing, recomputed only when the board is rebuilt.
    ///
    /// Framing walks every cell in the lattice, which at full facility scale is
    /// five and a half thousand of them. Recomputing that per frame to service a
    /// scroll wheel is the kind of cost that only shows up on the largest board
    /// setting — where it is least affordable — so zoom and pan modify a cached
    /// framing instead.
    framing: (Transform, f32),
    viewport_size: Vec2,
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
            deck_camera: CameraPose::default(),
            map_camera: CameraPose::default(),
            dirty: true,
            report: DrawReport::default(),
            geometry: None,
            hovered: None,
            preview: None,
            overlay_dirty: true,
            notice: String::new(),
            pending_move: None,
            teleport: None,
            framing: (Transform::IDENTITY, 1.0),
            viewport_size: Vec2::new(WINDOW_WIDTH - view::hud::COMMAND_DOCK_WIDTH, WINDOW_HEIGHT),
        }
    }

    #[must_use]
    fn camera_pose(&self) -> CameraPose {
        match self.mode {
            ViewMode::Deck => self.deck_camera,
            ViewMode::Overview => self.map_camera,
        }
    }

    fn camera_pose_mut(&mut self) -> &mut CameraPose {
        match self.mode {
            ViewMode::Deck => &mut self.deck_camera,
            ViewMode::Overview => &mut self.map_camera,
        }
    }

    fn recenter_camera(&mut self) {
        *self.camera_pose_mut() = CameraPose::default();
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
            if self
                .apply_tactics_action(id, TacticsAction::Move(step.face))
                .is_err()
            {
                return false;
            }
            if self.teleport.is_some() {
                break;
            }
        }
        if let Some(unit) = self.game.units.get(&id) {
            self.level = self
                .teleport
                .map_or(unit.cell.level, |transit| transit.from.level);
        }
        !steps.is_empty()
    }

    /// Apply one ordinary simulation action and retain only the metadata needed
    /// to project a completed portal traversal.
    fn apply_tactics_action(
        &mut self,
        id: PlayerId,
        action: TacticsAction,
    ) -> Result<(), sim::action::Refusal> {
        let first_new_event = self.game.events.len();
        self.game.apply(id, action)?;
        if let Some(TacticsEvent::PadTraversed { unit, from, to }) = self.game.events
            [first_new_event..]
            .iter()
            .rev()
            .copied()
            .find(|event| matches!(event, TacticsEvent::PadTraversed { .. }))
            && unit == id
        {
            self.teleport = Some(view::portals::TeleportPresentation::new(unit, from, to));
            self.level = from.level;
            self.notice = format!(
                "Portal transit: U{} L{} ({},{}) -> L{} ({},{})",
                unit.0, from.level, from.q, from.r, to.level, to.q, to.r
            );
        }
        Ok(())
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
            canvas: Some("#tactics-canvas".to_string()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }))
    .add_plugins(observed_ui::FrontendWidgetsPlugin);
    configure(&mut app);
    #[cfg(not(target_arch = "wasm32"))]
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
        .init_resource::<MapPointerCapture>()
        .init_resource::<MapTouchGesture>()
        .init_resource::<BotSpectator>()
        .init_resource::<AuthoredBoardContent>()
        .init_resource::<observed_cutaway::TileMeshCache>()
        .add_message::<SetupRequested>()
        .add_observer(view::setup::activate)
        .add_observer(view::setup::adjust)
        .add_systems(OnEnter(AppState::Setup), enter_setup)
        .add_systems(OnExit(AppState::Setup), despawn::<SetupRoot>)
        .add_systems(OnEnter(AppState::Loading), enter_loading)
        .add_systems(OnExit(AppState::Loading), despawn::<LoadingRoot>)
        .add_systems(OnEnter(AppState::Play), enter_play)
        .add_systems(
            OnExit(AppState::Play),
            (
                despawn::<BoardVisual>,
                despawn::<view::units::UnitVisual>,
                despawn::<view::portals::PadVisual>,
                despawn::<view::portals::TeleportEffect>,
                despawn::<view::BoardOverlay>,
                despawn::<HudRoot>,
                despawn::<OverlayRoot>,
                despawn::<HudCamera>,
                leave_play,
            ),
        )
        .add_systems(
            Update,
            (
                handle_setup_requests,
                view::setup::sync_sliders,
                view::setup::sync_labels,
                view::setup::focus_hovered,
                scroll_setup,
            )
                .chain()
                .run_if(in_state(AppState::Setup)),
        )
        .add_systems(Update, begin_loading.run_if(in_state(AppState::Loading)))
        .add_systems(Update, sync_screen_cursor)
        .add_systems(
            Update,
            (sync_hud_layout, sync_camera_viewport)
                .chain()
                .run_if(in_state(AppState::Play)),
        )
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
                drive_spectator_bot,
                view::portals::advance,
                scroll_command_dock,
                touch_map_controls,
                update_board_hover,
                board_clicks,
                rebuild_board,
                view::portals::sync_plates,
                view::units::sync,
                view::portals::animate_effects,
                camera_controls,
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
    cameras: TacticsCameras,
) {
    for entity in cameras.board.iter().chain(cameras.hud.iter()) {
        commands.entity(entity).despawn();
    }
    commands.spawn((
        BoardCamera,
        Camera2d,
        bevy::ui::IsDefaultUiCamera,
        Name::new("Setup camera"),
    ));
    view::setup::spawn(&mut commands, &settings.0, message.0.as_deref());
}

fn handle_setup_requests(
    mut requests: MessageReader<SetupRequested>,
    mut next: ResMut<NextState<AppState>>,
) {
    for request in requests.read() {
        match request.0 {
            SetupRequest::Changed => {
                // The setup sync system derives labels and rails from the live
                // settings without breaking pointer ownership mid-drag.
            }
            SetupRequest::Start => next.set(AppState::Loading),
        }
    }
}

/// Setup can exceed a short phone viewport. Wheel input and a predominantly
/// vertical one-finger drag move only this screen's scroll position; horizontal
/// slider manipulation remains owned by the widget under the finger.
fn scroll_setup(
    scroll: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
    mut roots: Query<(&mut ScrollPosition, &ComputedNode), With<SetupRoot>>,
) {
    let Ok((mut position, computed)) = roots.single_mut() else {
        return;
    };
    let touch_delta = touches
        .iter()
        .filter(|touch| {
            let distance = touch.distance();
            distance.y.abs() > distance.x.abs() && distance.y.abs() >= TOUCH_DRAG_THRESHOLD
        })
        .map(|touch| -touch.delta().y)
        .sum::<f32>();
    let delta = -scroll.delta.y * 32.0 + touch_delta;
    if delta.abs() <= f32::EPSILON {
        return;
    }
    let max_offset = ((computed.content_size().y - computed.size().y)
        * computed.inverse_scale_factor())
    .max(0.0);
    position.y = (position.y + delta).clamp(0.0, max_offset);
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
                             Right-drag pans; wheel zooms; V changes deck/map.\n\
                             B runs/pauses bot spectate; . advances one bot decision.",
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
    cameras: TacticsCameras,
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
    for entity in cameras.board.iter().chain(cameras.hud.iter()) {
        commands.entity(entity).despawn();
    }
    commands.spawn((
        BoardCamera,
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection::default_3d()),
        Transform::default(),
        Name::new("Board camera"),
    ));
    commands.spawn((
        HudCamera,
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        bevy::ui::IsDefaultUiCamera,
        Name::new("Tactics HUD camera"),
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
    commands.insert_resource(BotSpectator::default());
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
    mut spectator: ResMut<BotSpectator>,
    mut next: ResMut<NextState<AppState>>,
) {
    if state.pending_move.is_some() || state.teleport.is_some() {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        next.set(AppState::Setup);
        return;
    }
    let mut command = None;
    if keyboard.just_pressed(KeyCode::KeyB) {
        command = Some(HudButton::ToggleBot);
    } else if keyboard.just_pressed(KeyCode::Period) {
        command = Some(HudButton::StepBot);
    } else if keyboard.just_pressed(KeyCode::Space) {
        command = Some(HudButton::EndTurn);
    } else if keyboard.just_pressed(KeyCode::KeyV) {
        command = Some(HudButton::ToggleView);
    } else if keyboard.just_pressed(KeyCode::Tab) {
        command = Some(HudButton::NextUnit);
    } else if keyboard.just_pressed(KeyCode::BracketRight) {
        command = Some(HudButton::LevelUp);
    } else if keyboard.just_pressed(KeyCode::BracketLeft) {
        command = Some(HudButton::LevelDown);
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        command = Some(HudButton::PanLeft);
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        command = Some(HudButton::PanRight);
    } else if keyboard.just_pressed(KeyCode::ArrowUp) {
        command = Some(HudButton::PanUp);
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        command = Some(HudButton::PanDown);
    } else if keyboard.just_pressed(KeyCode::Equal) {
        command = Some(HudButton::ZoomIn);
    } else if keyboard.just_pressed(KeyCode::Minus) {
        command = Some(HudButton::ZoomOut);
    } else if keyboard.just_pressed(KeyCode::Home) {
        command = Some(HudButton::Recenter);
    }
    if let Some(command) = command {
        apply_command(&mut state, &mut spectator, command, &mut next);
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
        if keyboard.just_pressed(key) && !spectator.enabled {
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
    mut more_panels: Query<&mut Node, With<MobileMorePanel>>,
    mut docks: Query<&mut ScrollPosition, With<CommandDock>>,
    mut state: ResMut<LabState>,
    mut spectator: ResMut<BotSpectator>,
    mut next: ResMut<NextState<AppState>>,
) {
    if state.pending_move.is_some() || state.teleport.is_some() {
        return;
    }
    for (interaction, &button) in controls.iter() {
        if *interaction == Interaction::Pressed {
            if button == HudButton::ToggleMore {
                for mut panel in &mut more_panels {
                    panel.display = if panel.display == Display::None {
                        Display::Flex
                    } else {
                        Display::None
                    };
                }
                for mut position in &mut docks {
                    position.y = 0.0;
                }
                continue;
            }
            apply_command(&mut state, &mut spectator, button, &mut next);
        }
    }
}

/// One place every command is carried out, whichever input asked for it.
fn apply_command(
    state: &mut LabState,
    spectator: &mut BotSpectator,
    command: HudButton,
    next: &mut ResMut<NextState<AppState>>,
) {
    let available_while_spectating = matches!(
        command,
        HudButton::ToggleBot
            | HudButton::StepBot
            | HudButton::ToggleView
            | HudButton::LevelUp
            | HudButton::LevelDown
            | HudButton::PanLeft
            | HudButton::PanRight
            | HudButton::PanUp
            | HudButton::PanDown
            | HudButton::ZoomIn
            | HudButton::ZoomOut
            | HudButton::Recenter
            | HudButton::Restart
            | HudButton::ToggleMore
    );
    if spectator.enabled && !available_while_spectating {
        state.notice = String::from("Pause bot spectate before issuing squad commands");
        return;
    }
    let board_unchanged = matches!(
        command,
        HudButton::PanLeft
            | HudButton::PanRight
            | HudButton::PanUp
            | HudButton::PanDown
            | HudButton::ZoomIn
            | HudButton::ZoomOut
            | HudButton::Recenter
            | HudButton::ToggleBot
            | HudButton::StepBot
            | HudButton::ToggleMore
    );
    match command {
        HudButton::EndTurn => resolve_turn(state),
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
        HudButton::PanLeft => state.camera_pose_mut().pan.x -= CAMERA_NUDGE,
        HudButton::PanRight => state.camera_pose_mut().pan.x += CAMERA_NUDGE,
        HudButton::PanUp => state.camera_pose_mut().pan.y += CAMERA_NUDGE,
        HudButton::PanDown => state.camera_pose_mut().pan.y -= CAMERA_NUDGE,
        HudButton::ZoomIn => {
            let pose = state.camera_pose_mut();
            pose.zoom = (pose.zoom * 0.82).clamp(MIN_ZOOM, MAX_ZOOM);
        }
        HudButton::ZoomOut => {
            let pose = state.camera_pose_mut();
            pose.zoom = (pose.zoom * 1.22).clamp(MIN_ZOOM, MAX_ZOOM);
        }
        HudButton::Recenter => state.recenter_camera(),
        HudButton::ToggleMore => {}
        HudButton::ToggleBot => {
            spectator.enabled = !spectator.enabled;
            spectator.step_requested = spectator.enabled;
            spectator.cadence.reset();
            state.pending_move = None;
            state.hovered = None;
            state.preview = None;
            state.overlay_dirty = true;
            state.notice = if spectator.enabled {
                String::from("Bot spectate running")
            } else {
                String::from("Bot spectate paused")
            };
        }
        HudButton::StepBot => {
            spectator.enabled = false;
            spectator.step_requested = true;
            spectator.cadence.reset();
            state.pending_move = None;
            state.notice = String::from("Bot single-step queued");
        }
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
                    match state.apply_tactics_action(id, action) {
                        Ok(()) => {
                            if state.teleport.is_none() {
                                state.notice =
                                    format!("{action:?} accepted - {} AP", availability.cost);
                            }
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
    if !board_unchanged {
        state.dirty = true;
        state.hovered = None;
        state.preview = None;
        state.overlay_dirty = true;
    }
}

fn resolve_turn(state: &mut LabState) {
    let resolution = state.game.end_turn_detailed();
    if let Some(mut geometry) = state.geometry.take() {
        if let Err(detail) = geometry.apply_resolution(&state.game, &resolution) {
            state.notice = detail;
        } else {
            state.notice = match resolution.outcome {
                sim::relayout::ShiftOutcome::Committed => String::from("Facility shift committed"),
                sim::relayout::ShiftOutcome::Held => String::from("Facility shift held"),
                sim::relayout::ShiftOutcome::NothingToShift => String::from("No facility shift"),
            };
        }
        state.geometry = Some(geometry);
    }
    state.select_next();
}

/// Advance one bot decision at a reviewable cadence. This system is only an
/// adapter: target and action selection stay in the pure [`bot`] module.
fn drive_spectator_bot(
    time: Res<Time>,
    mut spectator: ResMut<BotSpectator>,
    mut state: ResMut<LabState>,
) {
    if state.teleport.is_some() {
        return;
    }
    let due = if spectator.step_requested {
        spectator.step_requested = false;
        true
    } else if spectator.enabled {
        spectator.cadence.tick(time.delta()).just_finished()
    } else {
        false
    };
    if !due {
        return;
    }

    let directive = bot::choose(&state.game);
    spectator.last = bot::describe(directive);
    let old_level = state.level;
    match directive {
        bot::BotDirective::Act { unit, action } => {
            state.selected = Some(unit);
            if let Err(refusal) = state.apply_tactics_action(unit, action) {
                spectator.enabled = false;
                state.notice = format!("Bot paused: {action:?} refused ({refusal:?})");
                return;
            }
            if state.teleport.is_none()
                && let Some(actor) = state.game.units.get(&unit)
            {
                state.level = actor.cell.level;
            }
            if state.teleport.is_none() {
                state.notice = format!("Bot: {}", spectator.last);
            }
        }
        bot::BotDirective::EndTurn => resolve_turn(&mut state),
        bot::BotDirective::Finished => {
            spectator.enabled = false;
            state.notice = String::from("Bot spectate complete");
        }
    }
    if state.level != old_level {
        state.recenter_camera();
    }
    state.dirty = true;
    state.hovered = None;
    state.preview = None;
    state.overlay_dirty = true;
}

fn sync_camera_viewport(
    windows: Query<&Window>,
    mut cameras: Query<(&mut Camera, &mut Transform, &mut Projection), With<BoardCamera>>,
    mut state: ResMut<LabState>,
) {
    let (Ok(window), Ok((mut camera, mut transform, mut projection))) =
        (windows.single(), cameras.single_mut())
    else {
        return;
    };
    let layout = view::hud::DockLayout::for_window(Vec2::new(window.width(), window.height()));
    let size = (layout.viewport_size * window.scale_factor())
        .as_uvec2()
        .max(UVec2::ONE);
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
    let logical = size.as_vec2() / window.scale_factor();
    if logical != state.viewport_size {
        state.viewport_size = logical;
        state.framing = view::camera::frame(&state.game, state.mode, state.level, logical);
    }
    view::camera::apply(
        &mut transform,
        &mut projection,
        state.framing(),
        state.camera_pose().zoom,
        state.camera_pose().pan,
    );
}

fn sync_hud_layout(
    windows: Query<&Window>,
    mut roots: Query<&mut Node, HudRootNode>,
    mut docks: Query<&mut Node, CommandDockNode>,
    mut desktop: Query<&mut Node, DesktopHudNode>,
    mut mobile: Query<&mut Node, MobileHudNode>,
) {
    let (Ok(window), Ok(mut root), Ok(mut dock), Ok(mut desktop), Ok(mut mobile)) = (
        windows.single(),
        roots.single_mut(),
        docks.single_mut(),
        desktop.single_mut(),
        mobile.single_mut(),
    ) else {
        return;
    };
    let layout = view::hud::sync_layout(
        Vec2::new(window.width(), window.height()),
        &mut root,
        &mut dock,
    );
    view::hud::sync_content_visibility(layout.placement, &mut desktop, &mut mobile);
}

/// Mouse wheel over the fixed dock scrolls the dock and never leaks through to
/// map zoom. The same dock width defines the 3D viewport boundary, so pointer
/// ownership cannot drift from the layout under DPI scaling.
fn scroll_command_dock(
    scroll: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    mut docks: Query<(&mut ScrollPosition, &ComputedNode), With<CommandDock>>,
) {
    let (Ok(window), Ok((mut position, computed))) = (windows.single(), docks.single_mut()) else {
        return;
    };
    let layout = view::hud::DockLayout::for_window(Vec2::new(window.width(), window.height()));
    let mouse_over_dock = window
        .cursor_position()
        .is_some_and(|cursor| layout.contains_dock_point(cursor));
    let mouse_delta = if mouse_over_dock {
        -scroll.delta.y * 32.0
    } else {
        0.0
    };
    let touch_delta = touches
        .iter()
        .map(|touch| {
            layout.touch_scroll_delta(
                touch.start_position(),
                touch.distance(),
                touch.delta(),
                TOUCH_DRAG_THRESHOLD,
            )
        })
        .sum::<f32>();
    let delta = mouse_delta + touch_delta;
    if delta.abs() <= f32::EPSILON {
        return;
    }
    let max_offset = ((computed.content_size().y - computed.size().y)
        * computed.inverse_scale_factor())
    .max(0.0);
    position.y = (position.y + delta).clamp(0.0, max_offset);
}

/// One-finger drag pans, two-finger movement pans and pinches, and a short
/// one-finger release activates the cell under the finger. Only contacts that
/// begin inside the camera viewport join the gesture, so touching the dock can
/// never move or zoom the map even if the finger later crosses the boundary.
fn touch_map_controls(
    touches: Res<Touches>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<BoardCamera>>,
    spectator: Res<BotSpectator>,
    mut gesture: ResMut<MapTouchGesture>,
    mut state: ResMut<LabState>,
) {
    let (Ok(window), Ok((camera, transform))) = (windows.single(), cameras.single()) else {
        return;
    };
    let belongs_to_map =
        |position: Vec2| view::camera::cursor_in_viewport(window, camera, position);
    let mut active = touches
        .iter()
        .filter(|touch| belongs_to_map(touch.start_position()))
        .collect::<Vec<_>>();
    active.sort_by_key(|touch| touch.id());
    let started = touches
        .iter_just_pressed()
        .any(|touch| belongs_to_map(touch.start_position()));
    if !gesture.active && (started || !active.is_empty()) {
        gesture.active = true;
        gesture.moved = false;
        gesture.max_contacts = 0;
    }
    if !gesture.active {
        return;
    }

    gesture.max_contacts = gesture.max_contacts.max(active.len());
    if active
        .iter()
        .any(|touch| touch.distance().length() >= TOUCH_DRAG_THRESHOLD)
    {
        gesture.moved = true;
    }

    if !active.is_empty() {
        let count = active.len() as f32;
        let centre = active.iter().map(|touch| touch.position()).sum::<Vec2>() / count;
        let previous_centre = active
            .iter()
            .map(|touch| touch.previous_position())
            .sum::<Vec2>()
            / count;
        let centre_delta = centre - previous_centre;
        if gesture.moved || active.len() >= 2 {
            let scale = state.camera_pose().zoom * 0.6;
            state.camera_pose_mut().pan += Vec2::new(-centre_delta.x, centre_delta.y) * scale;
        }

        if active.len() >= 2 {
            gesture.moved = true;
            let current_span = active[0].position().distance(active[1].position());
            let previous_span = active[0]
                .previous_position()
                .distance(active[1].previous_position());
            if current_span > 1.0 && previous_span > 1.0 {
                let old_zoom = state.camera_pose().zoom;
                let new_zoom = (old_zoom * previous_span / current_span).clamp(MIN_ZOOM, MAX_ZOOM);
                if let Some(anchor) = view::camera::viewport_cursor_offset(window, camera, centre) {
                    let anchor_delta = anchor * state.framing().1 * (old_zoom - new_zoom)
                        / state.viewport_size.y.max(1.0);
                    state.camera_pose_mut().pan += anchor_delta;
                }
                state.camera_pose_mut().zoom = new_zoom;
            }
        }
    }

    let released = touches
        .iter_just_released()
        .find(|touch| belongs_to_map(touch.start_position()));
    let canceled = touches
        .iter_just_canceled()
        .any(|touch| belongs_to_map(touch.start_position()));
    if active.is_empty() && (released.is_some() || canceled) {
        let tapped_cell = if !canceled
            && gesture.max_contacts == 1
            && !gesture.moved
            && let Some(touch) = released
            && touch.distance().length() < TOUCH_DRAG_THRESHOLD
            && let Ok(ray) = camera.viewport_to_world(transform, touch.position())
        {
            view::board::cell_at_ray(
                &state.game,
                state.mode,
                state.level,
                ray.origin,
                ray.direction.as_vec3(),
            )
        } else {
            None
        };
        if let Some(cell) = tapped_cell {
            state.hovered = Some(cell);
            state.preview = state
                .selected
                .map(|selected| state.game.preview_move(selected, cell));
            state.overlay_dirty = true;
            activate_board_cell(&mut state, &spectator, cell);
            gesture.feedback = Timer::from_seconds(TOUCH_FEEDBACK_SECONDS, TimerMode::Once);
        } else if !canceled && gesture.max_contacts == 1 && !gesture.moved {
            state.notice = String::from("Tap directly on a runner or floor tile");
        }
        gesture.active = false;
        gesture.moved = false;
        gesture.max_contacts = 0;
    }
}

/// Keep pointer affordance, hovered cell, and route preview in lockstep without
/// touching authored board geometry.
fn update_board_hover(
    mut input: BoardHoverInput,
    spectator: Res<BotSpectator>,
    mut state: ResMut<LabState>,
    mut commands: Commands,
) {
    let Ok((window_entity, window)) = input.windows.single() else {
        return;
    };
    input.touch_gesture.feedback.tick(input.time.delta());
    if !input.touch_gesture.feedback.is_finished() {
        return;
    }
    let control_hover = input
        .controls
        .iter()
        .find(|(interaction, _)| **interaction != Interaction::None)
        .map(|(_, disabled)| disabled);
    let mut hovered = None;
    let mut preview = None;
    let camera = input.cameras.single().ok();
    let cursor = window.cursor_position();
    let over_map = cursor.is_some_and(|cursor| {
        camera.is_some_and(|(camera, _)| view::camera::cursor_in_viewport(window, camera, cursor))
    });
    let icon =
        if state.pending_move.is_some() || state.teleport.is_some() {
            SystemCursorIcon::Progress
        } else if input.capture.panning {
            SystemCursorIcon::Grabbing
        } else if let Some(disabled) = control_hover {
            if disabled {
                SystemCursorIcon::NotAllowed
            } else {
                SystemCursorIcon::Pointer
            }
        } else if spectator.enabled && over_map {
            SystemCursorIcon::Default
        } else if let (Some(cursor), Some((camera, transform))) = (cursor, camera)
            && over_map
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
    touches: Res<Touches>,
    interactions: Query<&Interaction, With<HudButton>>,
    spectator: Res<BotSpectator>,
    mut state: ResMut<LabState>,
) {
    if !mouse.just_pressed(MouseButton::Left)
        || touches.iter().next().is_some()
        || touches.any_just_pressed()
        || touches.any_just_released()
        || spectator.enabled
        || state.game.phase == TurnPhase::Finished
        || state.pending_move.is_some()
        || state.teleport.is_some()
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
    activate_board_cell(&mut state, &spectator, cell);
}

fn activate_board_cell(state: &mut LabState, spectator: &BotSpectator, cell: HexCoord) {
    if spectator.enabled
        || state.game.phase == TurnPhase::Finished
        || state.pending_move.is_some()
        || state.teleport.is_some()
    {
        return;
    }
    if state.mode == ViewMode::Overview {
        state.mode = ViewMode::Deck;
        state.level = cell.level;
        state.deck_camera = CameraPose::default();
        state.notice = String::from("Deck view opened - tap a runner, then highlighted ground");
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
        state.notice = format!("Unit {} selected - tap highlighted ground to move", unit.0);
        state.dirty = true;
        state.overlay_dirty = true;
        return;
    }
    let Some(id) = state.selected else {
        state.notice = String::from("Select a runner first, then tap its destination");
        return;
    };
    // A touch device has no hover phase. Always derive the route from the cell
    // that was actually activated instead of reusing mouse-only preview state.
    let preview = state.game.preview_move(id, cell);
    state.hovered = Some(cell);
    state.preview = Some(preview.clone());
    state.overlay_dirty = true;
    if preview.can_move() {
        state.notice = format!(
            "Move accepted - {} step(s), {} AP",
            preview.affordable_steps, preview.action_point_cost
        );
        state.pending_move = Some(PendingMove {
            unit: id,
            steps: preview.executable_steps().iter().copied().collect(),
            timer: Timer::new(Duration::from_millis(340), TimerMode::Repeating),
        });
        state.overlay_dirty = true;
    } else {
        state.notice = format!(
            "Move refused: {:?}",
            preview.refusal.unwrap_or(sim::action::Refusal::Blocked)
        );
    }
}

fn advance_pending_move(time: Res<Time>, mut state: ResMut<LabState>) {
    if state.teleport.is_some() {
        return;
    }
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
    match state.apply_tactics_action(pending.unit, TacticsAction::Move(step.face)) {
        Ok(()) => {
            if state.teleport.is_none() {
                state.level = step.to.level;
                state.notice = format!(
                    "Unit {} moved to ({},{},L{})",
                    pending.unit.0, step.to.q, step.to.r, step.to.level
                );
            }
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
    input: CameraInput,
    mut capture: ResMut<MapPointerCapture>,
    mut state: ResMut<LabState>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<BoardCamera>>,
) {
    let cursor_offset = input.windows.single().ok().and_then(|window| {
        window.cursor_position().and_then(|cursor| {
            input
                .viewports
                .single()
                .ok()
                .and_then(|camera| view::camera::viewport_cursor_offset(window, camera, cursor))
        })
    });
    let over_map = cursor_offset.is_some();
    if (input.mouse.just_pressed(MouseButton::Right)
        || input.mouse.just_pressed(MouseButton::Middle))
        && over_map
    {
        capture.panning = true;
    }
    if !input.mouse.pressed(MouseButton::Right) && !input.mouse.pressed(MouseButton::Middle) {
        capture.panning = false;
    }
    if over_map && input.scroll.delta.y.abs() > f32::EPSILON {
        let old_zoom = state.camera_pose().zoom;
        let new_zoom = (old_zoom * 1.14_f32.powf(-input.scroll.delta.y)).clamp(MIN_ZOOM, MAX_ZOOM);
        let anchor_delta =
            cursor_offset.unwrap_or_default() * state.framing().1 * (old_zoom - new_zoom)
                / state.viewport_size.y.max(1.0);
        let pose = state.camera_pose_mut();
        pose.zoom = new_zoom;
        pose.pan += anchor_delta;
    }
    if capture.panning {
        let pose = state.camera_pose_mut();
        let scale = pose.zoom * 0.6;
        pose.pan += Vec2::new(-input.motion.delta.x, input.motion.delta.y) * scale;
    }
    let framing = state.framing();
    let pose = state.camera_pose();
    for (mut transform, mut projection) in &mut cameras {
        view::camera::apply(
            &mut transform,
            &mut projection,
            framing,
            pose.zoom,
            pose.pan,
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
    state.framing = view::camera::frame(&state.game, mode, level, state.viewport_size);
    state.dirty = false;
}

fn refresh_hud(
    state: Res<LabState>,
    spectator: Res<BotSpectator>,
    mut status: Query<&mut Text, StatusTextNode>,
    mut squad: Query<&mut Text, SquadTextNode>,
    mut mobile_status: Query<&mut Text, MobileStatusTextNode>,
    mut mobile_prompt: Query<&mut Text, MobilePromptTextNode>,
) {
    for mut text in &mut status {
        let mut value = view::hud::status_line(&state.game, state.mode, state.level);
        let route = state.preview.as_ref().map_or_else(
            || String::from("Route: hover a cell"),
            |preview| {
                format!(
                    "Route: {} steps / {} AP -> ({},{},L{}){}",
                    preview.affordable_steps,
                    preview.action_point_cost,
                    preview.stopping_cell.q,
                    preview.stopping_cell.r,
                    preview.stopping_cell.level,
                    if preview.reaches_destination
                        && preview.affordable_steps == preview.steps.len()
                    {
                        ""
                    } else {
                        " - continues"
                    }
                )
            },
        );
        value.push('\n');
        value.push_str(&route);
        if let Some(register) = state
            .hovered
            .and_then(|cell| state.game.world.architecture.get(&cell))
        {
            value.push('\n');
            value.push_str("District: ");
            value.push_str(register.label());
        }
        if let Some(pad) = state.hovered.and_then(|cell| {
            state
                .game
                .pads
                .deployed
                .values()
                .find(|pad| pad.cell == cell)
        }) {
            let linked = state.game.pads.link_target(pad.team, pad.id).is_some();
            value.push('\n');
            value.push_str(if linked {
                "Portal plate: LINKED"
            } else {
                "Portal plate: waiting for pair"
            });
        }
        value.push('\n');
        value.push_str(if state.notice.is_empty() {
            "Ready"
        } else {
            &state.notice
        });
        value.push('\n');
        value.push_str(if spectator.enabled {
            "Spectate: RUN"
        } else {
            "Spectate: paused"
        });
        if !spectator.last.is_empty() {
            value.push_str(" | last: ");
            value.push_str(&spectator.last);
        }
        **text = value;
    }
    for mut text in &mut squad {
        **text = view::hud::squad_line(&state.game, state.selected);
    }
    for mut text in &mut mobile_status {
        let view = match state.mode {
            ViewMode::Deck => "DECK",
            ViewMode::Overview => "MAP",
        };
        let unit = state.selected.and_then(|id| state.game.units.get(&id));
        **text = unit.map_or_else(
            || {
                format!(
                    "TURN {}  |  {view} L{}  |  NO RUNNER",
                    state.game.turn, state.level
                )
            },
            |unit| {
                format!(
                    "TURN {}  |  {view} L{}  |  U{}  AP{}",
                    state.game.turn, state.level, unit.id.0, unit.action_points
                )
            },
        );
    }
    for mut text in &mut mobile_prompt {
        **text = if state.pending_move.is_some() {
            String::from("Moving runner...")
        } else if state.teleport.is_some() {
            String::from("Portal transit...")
        } else if spectator.enabled {
            String::from("Bot spectating. Tap Bot run / pause under More to take control.")
        } else if state.selected.is_some() {
            format!(
                "1 Runner selected  |  2 Tap highlighted ground\n{}",
                state.notice
            )
        } else {
            String::from("1 Tap a runner  |  2 Tap highlighted ground")
        };
    }
}

fn refresh_action_buttons(
    mut commands: Commands,
    state: Res<LabState>,
    spectator: Res<BotSpectator>,
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
        let squad_command = matches!(
            button,
            HudButton::EndTurn
                | HudButton::NextUnit
                | HudButton::Interact
                | HudButton::DeployAnchor
                | HudButton::RecoverAnchor
                | HudButton::DeployPad
        );
        let should_disable = ((spectator.enabled || state.teleport.is_some()) && squad_command)
            || action.is_some_and(|action| {
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

#[cfg(test)]
mod interaction_tests {
    use super::*;

    #[test]
    fn activating_a_destination_does_not_depend_on_mouse_hover() {
        let game = TacticsGame::new(MatchSettings::guided()).expect("guided match solves");
        let mut state = LabState::new(game);
        state.mode = ViewMode::Deck;
        let selected = state.selected.expect("guided match has a player runner");
        let destination = state
            .game
            .world
            .placements
            .keys()
            .copied()
            .find(|&cell| state.game.preview_move(selected, cell).can_move())
            .expect("runner has a reachable destination");

        // This is the phone path: the tap supplies a cell without ever having
        // populated the mouse-hover preview.
        state.hovered = None;
        state.preview = None;
        activate_board_cell(&mut state, &BotSpectator::default(), destination);

        assert!(state.pending_move.is_some());
        assert_eq!(state.hovered, Some(destination));
        assert_eq!(
            state.preview.as_ref().map(|preview| preview.requested),
            Some(destination)
        );
    }
}
