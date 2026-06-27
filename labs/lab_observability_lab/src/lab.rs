//! The ECS layer for Phase A7.
//!
//! It wires the pure [`crate::model`] and the [`crate::config`] schema into a
//! running lab: a deterministic event simulation drives semantic [`ObservedEvent`]
//! messages, a config-gated **lab-local** tracer mirrors them to Bevy's `tracing`
//! log (the capability `bevy_log_events` would have provided, minus its forced
//! egui stack), and a neon-noir overlay makes the otherwise-invisible event flow
//! and the live config inspectable far faster than console-only printing.

use std::collections::VecDeque;
use std::path::PathBuf;

use bevy::{prelude::*, window::WindowFocused};
use bevy_mod_config::{ConfigNode, ReadConfig, ScalarData, manager::Instance};
use observed_style::{
    ColorVisionMode, MarkerRole, SurfaceRole, marker, simulate_color_vision, surface,
};

use crate::config::{self, ConfigManager, ObservabilityConfig};
use crate::model::{self, EventKind, LaunchManifest, TickEvent, TracedEvent};

/// Decay rate (per second) of a room tile's event glow.
const GLOW_DECAY_PER_SEC: f32 = 1.4;
/// How many recent events the trace overlay shows.
const TRACE_VIEW: usize = 12;
/// Live step cadence (seconds per simulation tick) used by `run`.
pub const LIVE_STEP_SECONDS: f32 = 0.35;
/// Horizontal spacing between room tiles.
const TILE_PITCH: f32 = 132.0;
/// Room tile edge length.
const TILE_SIZE: f32 = 112.0;

// ----------------------------------------------------------------------------
// Components
// ----------------------------------------------------------------------------

/// Marks every entity spawned by the lab scene, so reset can despawn cleanly.
/// Config entities are owned by `bevy_mod_config` and deliberately excluded.
#[derive(Component)]
pub struct LabSpawned;

/// The lab's 2D camera.
#[derive(Component)]
pub struct ObsCamera;

/// The root UI node of the overlay.
#[derive(Component)]
pub struct ObsUiRoot;

/// A toggleable overlay panel.
#[derive(Component)]
pub struct OverlayPanel;

/// The diagnostics text element.
#[derive(Component)]
pub struct DiagnosticsText;

/// One map tile for room `index`.
#[derive(Component)]
pub struct RoomTile {
    /// The room this tile represents.
    pub index: u8,
}

/// The translucent atmosphere/fog quad.
#[derive(Component)]
pub struct FogOverlay;

/// The bloom-strength indicator pip.
#[derive(Component)]
pub struct BloomPip;

// ----------------------------------------------------------------------------
// Messages
// ----------------------------------------------------------------------------

/// A semantic event emitted by the simulation on a given tick. This is the
/// message type the lab-local tracer and any consumer subscribe to. It is a
/// projection of deterministic state, never a driver of it.
#[derive(Message, Clone, Copy, Debug)]
pub struct ObservedEvent {
    /// The tick the event fired on.
    pub tick: u32,
    /// The event payload.
    pub event: TracedEvent,
}

// ----------------------------------------------------------------------------
// Resources
// ----------------------------------------------------------------------------

/// The seed currently driving the running simulation. Config edits to the seed
/// are pending until committed here (the lab's `Enter` action).
#[derive(Resource)]
pub struct ActiveManifest(pub LaunchManifest);

impl Default for ActiveManifest {
    fn default() -> Self {
        Self(LaunchManifest::new(model::DEFAULT_SEED))
    }
}

/// The precomputed event stream and its checksum for the active manifest.
#[derive(Resource)]
pub struct ActiveStream {
    /// Events for the current run, ordered by tick.
    pub events: Vec<TickEvent>,
    /// FNV-1a checksum of `events`, used as a determinism invariant.
    pub checksum: u64,
}

impl ActiveStream {
    /// Builds the stream for `manifest` from the pure model.
    pub fn build(manifest: LaunchManifest) -> Self {
        let events = model::simulate(manifest);
        let checksum = model::checksum(&events);
        Self { events, checksum }
    }
}

impl Default for ActiveStream {
    fn default() -> Self {
        Self::build(LaunchManifest::new(model::DEFAULT_SEED))
    }
}

/// The simulation clock. `next` is the next tick to process; `last` is the most
/// recently processed tick (for display).
#[derive(Resource, Default)]
pub struct SimClock {
    /// Next tick to process.
    pub next: u32,
    /// Last processed tick.
    pub last: u32,
    /// Total processed ticks (monotonic across loops).
    pub steps: u64,
}

impl SimClock {
    fn reset(&mut self) {
        self.next = 0;
        self.last = 0;
        self.steps = 0;
    }
}

/// Throttles simulation stepping. `interval <= 0.0` steps once every frame
/// (used by headless tests); a positive interval paces live runs.
#[derive(Resource)]
pub struct StepGate {
    /// Seconds between steps (`<= 0.0` means every frame).
    pub interval: f32,
    /// Accumulated time since the last step.
    pub accum: f32,
}

impl Default for StepGate {
    fn default() -> Self {
        Self {
            interval: 0.0,
            accum: 0.0,
        }
    }
}

/// A bounded ring buffer of the most recent events, shown in the overlay.
#[derive(Resource, Default)]
pub struct TraceLog {
    /// Recent events, newest at the back.
    pub entries: VecDeque<TickEvent>,
}

impl TraceLog {
    fn push(&mut self, entry: TickEvent) {
        if self.entries.len() >= TRACE_VIEW {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Per-room event glow, decayed by [`present_scene`]. `None` is unlit.
#[derive(Resource, Default)]
pub struct TileHighlights {
    /// Glow `(kind, intensity)` per room, indexed by room id.
    pub glow: [Option<(EventKind, f32)>; model::ROOM_COUNT as usize],
}

impl TileHighlights {
    fn light(&mut self, room: u8, kind: EventKind) {
        if let Some(slot) = self.glow.get_mut(room as usize) {
            *slot = Some((kind, 1.0));
        }
    }

    fn clear(&mut self) {
        self.glow = Default::default();
    }
}

/// Transient lab state that is not part of any deterministic manifest.
#[derive(Resource)]
pub struct RuntimeState {
    /// Whether the window is focused.
    pub focused: bool,
    /// Number of scene resets performed.
    pub reset_count: u32,
    /// Number of successful config saves.
    pub save_count: u32,
    /// Number of successful config loads.
    pub load_count: u32,
    /// Total trace lines logged (across the session).
    pub logged: u64,
    /// Latest status line for the overlay.
    pub notice: String,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            focused: true,
            reset_count: 0,
            save_count: 0,
            load_count: 0,
            logged: 0,
            notice: "Observability lab ready.".to_string(),
        }
    }
}

/// Set when the user requests a scene reset.
#[derive(Resource, Default)]
pub struct ResetRequested(pub bool);

/// Set when the user commits the config seed into the active manifest.
#[derive(Resource, Default)]
pub struct CommitRequested(pub bool);

/// Pending persistence actions.
#[derive(Resource, Default, Clone, Copy)]
pub struct PersistRequest {
    /// Save the config to disk.
    pub save: bool,
    /// Load the config from disk.
    pub load: bool,
}

impl PersistRequest {
    fn pending(self) -> bool {
        self.save || self.load
    }
}

/// Where the JSON config is persisted. Defaults to a temp file (overridable
/// with the `OBSERVED2_CONFIG` env var) so the lab never litters the repo.
#[derive(Resource)]
pub struct ConfigPersistPath(pub PathBuf);

impl Default for ConfigPersistPath {
    fn default() -> Self {
        let path = std::env::var("OBSERVED2_CONFIG").map_or_else(
            |_| std::env::temp_dir().join("observed2_lab_observability_config.json"),
            PathBuf::from,
        );
        Self(path)
    }
}

// ----------------------------------------------------------------------------
// System sets
// ----------------------------------------------------------------------------

/// Ordered phases so config edits land before reads, and the sim before its
/// presentation.
#[derive(SystemSet, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ObsSet {
    /// Read input, edit config knobs, set request flags.
    Input,
    /// Apply commit / reset / persist requests.
    Apply,
    /// Step the simulation and emit events.
    Simulate,
    /// Mirror events to the trace log.
    Trace,
    /// Project state to sprites and the overlay.
    Present,
}

// ----------------------------------------------------------------------------
// Startup
// ----------------------------------------------------------------------------

/// Loads any persisted config, then commits its seed into the launch manifest.
/// Exclusive because `bevy_mod_config`'s persistence needs `&mut World`.
pub fn startup_launch(world: &mut World) {
    let path = world.resource::<ConfigPersistPath>().0.clone();
    if path.exists() {
        let manager = world.resource::<Instance<ConfigManager>>().instance.clone();
        match std::fs::File::open(&path) {
            Ok(file) => {
                if let Err(err) = manager.from_reader(world, file) {
                    warn!(target: "lab_observability", "ignoring unreadable config {path:?}: {err}");
                }
            }
            Err(err) => {
                warn!(target: "lab_observability", "could not open config {path:?}: {err}");
            }
        }
    }

    let seed = read_seed(world);
    let manifest = LaunchManifest::new(seed);
    world.resource_mut::<ActiveManifest>().0 = manifest;
    world.insert_resource(ActiveStream::build(manifest));
}

/// Reads the current `seed` config scalar directly from the world.
fn read_seed(world: &mut World) -> u32 {
    let mut query = world.query::<(&ConfigNode, &ScalarData<u32>)>();
    for (node, data) in query.iter(world) {
        if config::is_field(node, config::field::SEED) {
            return data.0;
        }
    }
    model::DEFAULT_SEED
}

/// Spawns the camera, room tiles, fog, bloom pip, and overlay UI.
pub fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        ObsCamera,
        LabSpawned,
        Name::new("Observability Lab Camera"),
    ));

    let dim = surface(SurfaceRole::Wall).base_color;
    let first = -(TILE_PITCH * (model::ROOM_COUNT as f32 - 1.0)) / 2.0;
    for room in 0..model::ROOM_COUNT {
        let x = first + room as f32 * TILE_PITCH;
        commands.spawn((
            LabSpawned,
            RoomTile { index: room },
            Sprite {
                color: dim,
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(x, -40.0, 0.0),
            Name::new(format!("Room tile {room}")),
        ));
    }

    // Atmosphere quad over the map; alpha is driven live by the fog knob.
    commands.spawn((
        LabSpawned,
        FogOverlay,
        Sprite {
            color: surface(SurfaceRole::Ceiling).base_color.with_alpha(0.45),
            custom_size: Some(Vec2::new(
                TILE_PITCH * model::ROOM_COUNT as f32 + 60.0,
                320.0,
            )),
            ..default()
        },
        Transform::from_xyz(0.0, -40.0, 1.0),
        Name::new("Fog overlay"),
    ));

    // Bloom indicator pip; its width tracks the bloom knob.
    commands.spawn((
        LabSpawned,
        BloomPip,
        Sprite {
            color: Color::from(marker(MarkerRole::Exit).emissive),
            custom_size: Some(Vec2::new(60.0, 18.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 170.0, 2.0),
        Name::new("Bloom pip"),
    ));

    spawn_overlay(&mut commands);
}

fn spawn_overlay(commands: &mut Commands) {
    commands
        .spawn((
            LabSpawned,
            ObsUiRoot,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
            Name::new("Observability Lab UI Root"),
        ))
        .with_children(|root| {
            root.spawn((
                OverlayPanel,
                Node {
                    width: px(560),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(surface(SurfaceRole::Wall).base_color.with_alpha(0.94)),
                BorderColor::all(marker(MarkerRole::NextRoom).base_color.with_alpha(0.6)),
                children![(
                    DiagnosticsText,
                    Text::new("observability diagnostics starting..."),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.95, 1.0)),
                )],
            ));
            root.spawn((
                OverlayPanel,
                Node {
                    width: px(360),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(surface(SurfaceRole::Wall).base_color.with_alpha(0.94)),
                BorderColor::all(marker(MarkerRole::Control).base_color.with_alpha(0.6)),
                children![(
                    Text::new(help_text()),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.93, 1.0)),
                )],
            ));
        });
}

fn help_text() -> String {
    "OBSERVABILITY LAB (Phase A7)\n\
     bevy_mod_config knobs + lab-local trace\n\n\
     [ ] fog     ; ' bloom\n\
     V color-vision   T trace verbosity\n\
     - = seed (candidate)\n\
     Enter commit seed -> manifest\n\
     F1 overlay   F2 save   F3 load\n\
     R reset scene\n\n\
     Debug knobs never touch the sim;\n\
     only a committed seed changes it."
        .to_string()
}

// ----------------------------------------------------------------------------
// Input
// ----------------------------------------------------------------------------

/// Reads the keyboard: edits config knobs in place, and sets request flags.
// The combined config query is one entity-per-scalar-type pass; the Option<&mut>
// shape is intentional (it avoids aliasing ConfigNode across per-type queries).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut reset: ResMut<ResetRequested>,
    mut commit: ResMut<CommitRequested>,
    mut persist: ResMut<PersistRequest>,
    mut runtime: ResMut<RuntimeState>,
    mut nodes: Query<(
        &mut ConfigNode,
        Option<&mut ScalarData<u32>>,
        Option<&mut ScalarData<f32>>,
        Option<&mut ScalarData<bool>>,
    )>,
) {
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        commit.0 = true;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        reset.0 = true;
    }
    if keys.just_pressed(KeyCode::F2) {
        persist.save = true;
    }
    if keys.just_pressed(KeyCode::F3) {
        persist.load = true;
    }

    for (mut node, u32d, f32d, boold) in &mut nodes {
        if config::is_field(&node, config::field::SEED)
            && let Some(mut data) = u32d
        {
            let mut value = data.0;
            if keys.just_pressed(KeyCode::Minus) {
                value = value.wrapping_sub(1);
            }
            if keys.just_pressed(KeyCode::Equal) {
                value = value.wrapping_add(1);
            }
            if value != data.0 {
                config::set_u32(&mut node, &mut data, value);
                runtime.notice = format!("Candidate seed {value} (Enter to commit).");
            }
        } else if config::is_field(&node, config::field::COLOR_VISION)
            && let Some(mut data) = u32d
        {
            if keys.just_pressed(KeyCode::KeyV) {
                let next = (data.0 + 1) % ColorVisionMode::ALL.len() as u32;
                config::set_u32(&mut node, &mut data, next);
                runtime.notice = format!(
                    "Color-vision preview: {}.",
                    ColorVisionMode::ALL[next as usize].label()
                );
            }
        } else if config::is_field(&node, config::field::TRACE_VERBOSITY)
            && let Some(mut data) = u32d
        {
            if keys.just_pressed(KeyCode::KeyT) {
                let next = (data.0 + 1) % (model::MAX_VERBOSITY + 1);
                config::set_u32(&mut node, &mut data, next);
                runtime.notice = format!("Trace verbosity: {next}.");
            }
        } else if config::is_field(&node, config::field::FOG)
            && let Some(mut data) = f32d
        {
            let mut value = data.0;
            if keys.just_pressed(KeyCode::BracketLeft) {
                value -= 0.05;
            }
            if keys.just_pressed(KeyCode::BracketRight) {
                value += 0.05;
            }
            config::set_unit_f32(&mut node, &mut data, value);
        } else if config::is_field(&node, config::field::BLOOM)
            && let Some(mut data) = f32d
        {
            let mut value = data.0;
            if keys.just_pressed(KeyCode::Semicolon) {
                value -= 0.05;
            }
            if keys.just_pressed(KeyCode::Quote) {
                value += 0.05;
            }
            config::set_unit_f32(&mut node, &mut data, value);
        } else if config::is_field(&node, config::field::OVERLAY)
            && let Some(mut data) = boold
            && keys.just_pressed(KeyCode::F1)
        {
            let value = !data.0;
            config::set_bool(&mut node, &mut data, value);
            runtime.notice = format!("Overlay {}.", if value { "shown" } else { "hidden" });
        }
    }
}

/// Neutralizes nothing in the sim, but tracks focus for the overlay health line.
pub fn update_focus(mut events: MessageReader<WindowFocused>, mut runtime: ResMut<RuntimeState>) {
    for event in events.read() {
        runtime.focused = event.focused;
    }
}

// ----------------------------------------------------------------------------
// Apply requests
// ----------------------------------------------------------------------------

/// Commits the config seed into the active manifest and rebuilds the stream.
/// This is the *only* path by which a config value enters deterministic state.
#[allow(clippy::too_many_arguments)]
pub fn apply_commit(
    mut commit: ResMut<CommitRequested>,
    cfg: ReadConfig<ObservabilityConfig>,
    mut manifest: ResMut<ActiveManifest>,
    mut stream: ResMut<ActiveStream>,
    mut clock: ResMut<SimClock>,
    mut trace: ResMut<TraceLog>,
    mut tiles: ResMut<TileHighlights>,
    mut runtime: ResMut<RuntimeState>,
) {
    if !commit.0 {
        return;
    }
    commit.0 = false;
    let seed = cfg.read().seed;
    manifest.0 = LaunchManifest::new(seed);
    *stream = ActiveStream::build(manifest.0);
    clock.reset();
    trace.clear();
    tiles.clear();
    runtime.notice = format!(
        "Committed launch seed {seed} (checksum {:016x}).",
        stream.checksum
    );
}

/// Despawns and rebuilds the scene without touching the active manifest or any
/// config entity. This is the reset-safety path the lifecycle test exercises.
#[allow(clippy::too_many_arguments)]
pub fn apply_reset(
    mut reset: ResMut<ResetRequested>,
    mut commands: Commands,
    spawned: Query<Entity, With<LabSpawned>>,
    mut clock: ResMut<SimClock>,
    mut trace: ResMut<TraceLog>,
    mut tiles: ResMut<TileHighlights>,
    mut runtime: ResMut<RuntimeState>,
    manifest: Res<ActiveManifest>,
) {
    if !reset.0 {
        return;
    }
    reset.0 = false;
    runtime.reset_count += 1;
    for entity in &spawned {
        commands.entity(entity).despawn();
    }
    clock.reset();
    trace.clear();
    tiles.clear();
    commands.run_system_cached(setup_scene);
    runtime.notice = format!(
        "Reset {} (manifest seed {} unchanged).",
        runtime.reset_count, manifest.0.seed
    );
}

/// Saves or loads the config via `bevy_mod_config`'s JSON manager. Exclusive
/// because the manager operates on `&mut World`.
pub fn apply_persist(world: &mut World) {
    let request = *world.resource::<PersistRequest>();
    if !request.pending() {
        return;
    }
    *world.resource_mut::<PersistRequest>() = PersistRequest::default();

    let path = world.resource::<ConfigPersistPath>().0.clone();
    let manager = world.resource::<Instance<ConfigManager>>().instance.clone();

    if request.save {
        let notice = match manager.to_string(world) {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(()) => {
                    world.resource_mut::<RuntimeState>().save_count += 1;
                    format!("Saved config -> {}", path.display())
                }
                Err(err) => format!("Save failed: {err}"),
            },
            Err(err) => format!("Serialize failed: {err}"),
        };
        world.resource_mut::<RuntimeState>().notice = notice;
    }

    if request.load {
        let notice = match std::fs::File::open(&path) {
            Ok(file) => match manager.from_reader(world, file) {
                Ok(()) => {
                    world.resource_mut::<RuntimeState>().load_count += 1;
                    format!("Loaded config <- {}", path.display())
                }
                Err(err) => format!("Load failed: {err}"),
            },
            Err(_) => format!("No saved config at {}", path.display()),
        };
        world.resource_mut::<RuntimeState>().notice = notice;
    }
}

// ----------------------------------------------------------------------------
// Simulation + trace
// ----------------------------------------------------------------------------

/// Advances the simulation by at most one tick per frame (paced by [`StepGate`])
/// and projects that tick's events as [`ObservedEvent`] messages, trace entries,
/// and tile glows.
pub fn step_simulation(
    time: Res<Time>,
    mut gate: ResMut<StepGate>,
    mut clock: ResMut<SimClock>,
    stream: Res<ActiveStream>,
    mut writer: MessageWriter<ObservedEvent>,
    mut trace: ResMut<TraceLog>,
    mut tiles: ResMut<TileHighlights>,
) {
    if gate.interval > 0.0 {
        gate.accum += time.delta_secs();
        if gate.accum < gate.interval {
            return;
        }
        gate.accum = 0.0;
    }
    emit_tick(&mut clock, &stream, &mut writer, &mut trace, &mut tiles);
}

/// Processes the current tick: writes one message per event, records it, and
/// lights the relevant room tile. Shared by the live stepper and the capture
/// warm-up so both behave identically.
pub fn emit_tick(
    clock: &mut SimClock,
    stream: &ActiveStream,
    writer: &mut MessageWriter<ObservedEvent>,
    trace: &mut TraceLog,
    tiles: &mut TileHighlights,
) {
    let tick = clock.next;
    for entry in stream.events.iter().filter(|e| e.tick == tick) {
        writer.write(ObservedEvent {
            tick,
            event: entry.event,
        });
        trace.push(*entry);
        if let Some(room) = entry.event.room() {
            tiles.light(room, entry.event.kind());
        }
    }
    clock.last = tick;
    clock.next = (tick + 1) % model::TICKS_PER_RUN;
    clock.steps += 1;
}

/// The lab-local event tracer: mirrors events to Bevy's `tracing` log, gated by
/// the `trace_verbosity` knob. Reading the messages always drains them, so
/// turning verbosity to 0 silences the log **without** changing the simulation
/// or the trace overlay — the "disable logging without behaviour changes" rule.
pub fn trace_events(
    mut reader: MessageReader<ObservedEvent>,
    cfg: ReadConfig<ObservabilityConfig>,
    mut runtime: ResMut<RuntimeState>,
) {
    let verbosity = cfg.read().trace_verbosity;
    for event in reader.read() {
        if model::verbosity_logs(event.event.kind(), verbosity) {
            info!(
                target: "lab_observability",
                "tick {:>2}  {}",
                event.tick,
                event.event.summary()
            );
            runtime.logged += 1;
        }
    }
}

// ----------------------------------------------------------------------------
// Presentation
// ----------------------------------------------------------------------------

/// Maps an event kind to the semantic marker role used for its tile glow.
fn kind_role(kind: EventKind) -> MarkerRole {
    match kind {
        EventKind::Door => MarkerRole::Control,
        EventKind::Observation => MarkerRole::NextRoom,
        EventKind::Interaction => MarkerRole::Teammate,
        EventKind::Round => MarkerRole::Director,
        EventKind::Pickup => MarkerRole::Exit,
        EventKind::Reroute => MarkerRole::Collapse,
    }
}

/// A vivid display colour for an event kind, normalized down from HDR emission.
fn kind_color(kind: EventKind) -> Color {
    let e = marker(kind_role(kind)).emissive;
    let peak = e.red.max(e.green).max(e.blue).max(1.0);
    Color::linear_rgb(e.red / peak, e.green / peak, e.blue / peak)
}

/// Linear-space mix of two colours.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_linear();
    let b = b.to_linear();
    let t = t.clamp(0.0, 1.0);
    Color::linear_rgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    )
}

/// Decays glows and projects fog, bloom, and tile colour from the live config,
/// applying the color-vision preview to every gameplay colour.
// One Sprite query with optional markers avoids `&mut Sprite` aliasing across
// the tile/fog/pip roles.
#[allow(clippy::type_complexity)]
pub fn present_scene(
    time: Res<Time>,
    cfg: ReadConfig<ObservabilityConfig>,
    mut tiles: ResMut<TileHighlights>,
    mut sprites: Query<(
        &mut Sprite,
        Option<&RoomTile>,
        Has<FogOverlay>,
        Has<BloomPip>,
    )>,
) {
    let view = cfg.read();
    let mode = ColorVisionMode::ALL[(view.color_vision as usize) % ColorVisionMode::ALL.len()];
    let bloom = view.bloom.clamp(0.0, 1.0);

    let decay = time.delta_secs() * GLOW_DECAY_PER_SEC;
    for slot in tiles.glow.iter_mut() {
        if let Some((_, intensity)) = slot {
            *intensity -= decay;
            if *intensity <= 0.0 {
                *slot = None;
            }
        }
    }

    let dim = surface(SurfaceRole::Wall).base_color;
    let fog_base = surface(SurfaceRole::Ceiling).base_color;

    for (mut sprite, tile, is_fog, is_pip) in &mut sprites {
        if let Some(tile) = tile {
            let color = match tiles.glow[tile.index as usize] {
                Some((kind, intensity)) => {
                    let lit = mix(dim, kind_color(kind), intensity);
                    brighten(lit, 1.0 + bloom * intensity)
                }
                None => dim,
            };
            sprite.color = Color::from(simulate_color_vision(color, mode));
        } else if is_fog {
            // Fog is atmosphere, not a gameplay signal, so it does not get the
            // color-vision transform; only its strength is config-driven.
            sprite.color = fog_base.with_alpha(view.fog.clamp(0.0, 1.0));
        } else if is_pip {
            let pip = brighten(kind_color(EventKind::Pickup), 0.4 + bloom);
            sprite.color = Color::from(simulate_color_vision(pip, mode));
            sprite.custom_size = Some(Vec2::new(24.0 + bloom * 240.0, 18.0));
        }
    }
}

/// Multiplies a colour's linear RGB by `factor`.
fn brighten(color: Color, factor: f32) -> Color {
    let c = color.to_linear();
    Color::linear_rgb(c.red * factor, c.green * factor, c.blue * factor)
}

/// Rebuilds the diagnostics overlay text and toggles panel visibility.
#[allow(clippy::too_many_arguments)]
pub fn update_overlay(
    cfg: ReadConfig<ObservabilityConfig>,
    manifest: Res<ActiveManifest>,
    stream: Res<ActiveStream>,
    clock: Res<SimClock>,
    trace: Res<TraceLog>,
    runtime: Res<RuntimeState>,
    persist_path: Res<ConfigPersistPath>,
    cameras: Query<(), With<ObsCamera>>,
    ui_roots: Query<(), With<ObsUiRoot>>,
    room_tiles: Query<(), With<RoomTile>>,
    mut panels: Query<&mut Visibility, With<OverlayPanel>>,
    mut texts: Query<&mut Text, With<DiagnosticsText>>,
) {
    let view = cfg.read();

    let visibility = if view.overlay {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut panel in &mut panels {
        *panel = visibility;
    }

    let camera_count = cameras.iter().count();
    let ui_count = ui_roots.iter().count();
    let tile_count = room_tiles.iter().count();
    let expected = model::checksum(&model::simulate(manifest.0));
    let checksum_ok = expected == stream.checksum;
    let healthy = camera_count == 1
        && ui_count == 1
        && tile_count == model::ROOM_COUNT as usize
        && runtime.focused
        && checksum_ok;

    let seed_drift = view.seed != manifest.0.seed;
    let mode = ColorVisionMode::ALL[(view.color_vision as usize) % ColorVisionMode::ALL.len()];

    let mut trace_lines = trace
        .entries
        .iter()
        .rev()
        .map(|entry| format!("    t{:<2} {}", entry.tick, entry.event.summary()))
        .collect::<Vec<_>>();
    if trace_lines.is_empty() {
        trace_lines.push("    (no events yet)".to_string());
    }

    for mut text in &mut texts {
        *text = Text::new(format!(
            "OBSERVABILITY LAB  {pass}\n\
             bevy_mod_config 0.6.2 (egui off) + lab-local trace\n\n\
             MANIFEST (deterministic)\n\
             active seed   {active_seed}\n\
             config seed   {config_seed}{drift}\n\
             stream        {events} events  checksum {checksum:016x}{checksum_flag}\n\
             sim tick      {tick:>2}/{total}   steps {steps}\n\n\
             DEBUG KNOBS (no sim effect)\n\
             fog {fog:.2}   bloom {bloom:.2}   color-vision {vision}\n\
             trace verbosity {verbosity}   logged {logged}\n\
             overlay {overlay}   config @ {path}\n\
             saves {saves}  loads {loads}\n\n\
             RECENT EVENTS (overlay = faster than console)\n\
             {trace}\n\n\
             cam {cam}  ui {ui}  tiles {tiles}  focus {focus}\n\
             {notice}",
            pass = if healthy { "[PASS]" } else { "[FAIL]" },
            active_seed = manifest.0.seed,
            config_seed = view.seed,
            drift = if seed_drift {
                "   <- PENDING (press Enter to relaunch)"
            } else {
                "   (committed)"
            },
            events = stream.events.len(),
            checksum = stream.checksum,
            checksum_flag = if checksum_ok { "" } else { "  !! DRIFT" },
            tick = clock.last,
            total = model::TICKS_PER_RUN,
            steps = clock.steps,
            fog = view.fog,
            bloom = view.bloom,
            vision = mode.label(),
            verbosity = view.trace_verbosity,
            logged = runtime.logged,
            overlay = if view.overlay { "on" } else { "off" },
            path = persist_path.0.display(),
            saves = runtime.save_count,
            loads = runtime.load_count,
            trace = trace_lines.join("\n"),
            cam = camera_count,
            ui = ui_count,
            tiles = tile_count,
            focus = runtime.focused,
            notice = runtime.notice,
        ));
    }
}
