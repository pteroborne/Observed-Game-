//! `composition_studio` — author the WFC composition profile and watch the
//! facility answer.
//!
//! Tiles say what the solver *may* build; the composition profile says what it
//! *tends* to build. Slice 0 made that profile authorable content and folded it
//! into the simulation content hash. This tool is where a person actually turns
//! the knobs and sees the layout move.
//!
//! Two rules this tool inherits and must not soften:
//!
//! - **Never display a hash it did not compute.** The status line shows the
//!   folded simulation hash because editing a profile locks out LAN peers who
//!   have not taken the same edit. A placeholder there would be worse than a
//!   blank, so an unreadable catalog reports *unavailable* rather than a
//!   plausible-looking wrong value.
//! - **Never silently fall back to the baseline profile.** Slice 0 made a
//!   missing profile a hard error precisely because a quiet fallback is what the
//!   content hash exists to catch. If the corpus profile cannot be read, the
//!   tool says so, keeps saying so, and refuses to promote over it.

pub mod actionbar;
pub mod brush;
pub mod capture;
pub mod chrome;
pub mod chrome_layout;
pub mod coverage;
pub mod detail;
pub mod draw;
pub mod field_widgets;
pub mod input;
pub mod layer;
pub mod load;
pub mod module;
pub mod neighbors;
pub mod panels;
pub mod persist;
pub mod pick;
pub mod regions;
pub mod script;
pub mod solve;
pub mod state;
pub mod tabs;
pub mod theme;
pub mod timeline;
pub mod touch_bar;
pub mod tunables;
pub mod typography;
pub mod viewport;
pub mod viewport_input;

/// Pin and coverage gates, split out so neither test file outgrows the
/// 600-line review budget the rest of the WFC path lives under.
#[cfg(test)]
pub mod authoring_tests;
#[cfg(test)]
pub mod browser_tests;
#[cfg(test)]
pub mod neighbor_tests;
#[cfg(test)]
pub mod tests;
#[cfg(test)]
pub mod timeline_tests;
#[cfg(test)]
pub mod view_tests;
#[cfg(test)]
pub mod widget_tests;

use bevy::prelude::*;
use observed_style::schematic_screen;

pub use chrome::{LabMenuState, StudioTab};
pub use draw::DrawReport;
pub use layer::Layer;
pub use load::{Corpus, corpus};
pub use state::{CatalogHash, MAX_WORKING_LEVELS, ProfileOrigin, SolveResult, StudioState};
pub use viewport::StudioCamera;

/// The same five seeds `iso_observer_lab` pins, so a studio capture and an Arc O
/// capture are comparing the same facilities. Treat as an evidence contract.
pub const PRESET_SEEDS: [u64; 5] = [
    0xa11c_e3d0_0000_0008,
    0x0000_0000_000c_0ffe,
    0x0000_0000_0000_0b0b,
    0x0000_0000_000d_00d0,
    0x5eed_0000_0000_0001,
];

/// How long after the last edit the solver waits before re-solving. Dragging a
/// value through ten steps should cost one solve, not ten.
pub const SOLVE_DEBOUNCE_SECONDS: f32 = 0.25;

/// Width of the docked panel, in logical pixels. The 3D camera's viewport is
/// inset by this, so the panel sits *beside* the facility rather than on top of
/// it — nothing is ever hidden behind chrome.
pub const PANEL_WIDTH: f32 = 560.0;

/// Below this window width the panel cannot sit beside the facility.
///
/// [`PANEL_WIDTH`] plus enough viewport to be worth looking at. A handset is
/// 360-430 logical pixels wide, so it is never close; the threshold exists for
/// the sizes in between, where a narrow desktop window should behave like a
/// phone rather than squeeze the facility into a strip.
pub const COMPACT_WIDTH_LIMIT: f32 = 960.0;

/// How much of a compact window's height the panel takes when open.
///
/// Slightly over half. The facility keeps the larger share of what remains
/// legible at a glance, while the panel still shows several controls without
/// scrolling.
pub const COMPACT_PANEL_FRACTION: f32 = 0.52;

/// How the panel and the facility share the window.
///
/// The studio's premise is that you change a value and watch the facility
/// answer, so both must be on screen at once. A phone cannot do that side by
/// side - [`PANEL_WIDTH`] alone is wider than the whole display - so the split
/// turns to run the other way rather than the panel becoming a screen you have
/// to leave to see the result.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LayoutMode {
    /// Panel beside the facility, full height.
    #[default]
    Wide,
    /// Panel below the facility, full width.
    Compact,
}

impl LayoutMode {
    #[must_use]
    pub fn for_window_width(width: f32) -> Self {
        if width < COMPACT_WIDTH_LIMIT {
            Self::Compact
        } else {
            Self::Wide
        }
    }

    #[must_use]
    pub fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }
}

/// Which region the keyboard is talking to.
///
/// The studio used to be modal: opening the panel froze the viewport. That is
/// the right trade in a lab where a stray keypress moves your character, and
/// the wrong one in a design tool, where the whole loop is *change a value and
/// watch the facility answer*. You cannot watch something you have frozen.
///
/// So the rule "a key never means two things at once" is kept, but enforced by
/// **ownership** rather than by blocking the world: whichever region you last
/// clicked receives keys, both stay live, and the owner is drawn with a border
/// so it is never a guess.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum KeyboardOwner {
    #[default]
    Panel,
    Viewport,
}

impl KeyboardOwner {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Viewport => "viewport",
        }
    }
}

/// Whether the renderer supplies the asset collections, or this app must.
///
/// "Is `Assets<A>` here yet" is the wrong question. Native render plugins
/// insert their collections during `build`, so asking at `build` time happens
/// to work there - but the browser acquires its adapter and device
/// asynchronously, and the collections arrive after every `build` has run.
/// The presence check therefore answers "no" in the browser for types the
/// renderer does own, a fresh empty `Assets<Shader>` would be installed over
/// the top, and `feathers`' embedded shaders would end up somewhere nothing
/// reads.
///
/// So ask about the plugin instead. It is added synchronously, and if it is
/// present it owns these types whenever it gets round to inserting them.
fn renderer_owns_assets(app: &App) -> bool {
    app.is_plugin_added::<bevy::render::RenderPlugin>()
}

/// Register an asset type only if nothing else already has.
///
/// `App::init_asset` is **not** idempotent: it builds a fresh `Assets<A>` and
/// inserts it, replacing whatever was there, while the `AssetServer` keeps
/// handing out indices from the original allocator. Calling it for a type the
/// render plugins already own throws away a populated collection, and the
/// symptom arrives much later as `index out of bounds` deep inside
/// `handle_internal_asset_events` - windowed only, because headless has no
/// render plugin to collide with.
fn init_asset_once<A: Asset>(app: &mut App) {
    if !renderer_owns_assets(app) && !app.world().contains_resource::<Assets<A>>() {
        app.init_asset::<A>();
    }
}

pub struct StudioPlugin;

impl Plugin for StudioPlugin {
    fn build(&self, app: &mut App) {
        // Without a slider plugin the sliders spawn, lay out, and draw exactly as
        // they should while ignoring every drag.
        //
        // `SliderPlugin` alone, not the whole `UiWidgetsPlugins` group: taking
        // only the widget in use keeps the focus story straight, because this
        // tool routes the keyboard through `KeyboardOwner` and never through
        // Bevy's focus resource.
        //
        // Guarded because 0.19's `DefaultPlugins` ships `UiWidgetsPlugins` (0.18's
        // did not), so the windowed studio already has it and adding it again is a
        // panic. Headless tests build on `MinimalPlugins` and still need it here.
        if !app.is_plugin_added::<bevy::ui_widgets::SliderPlugin>() {
            app.add_plugins(bevy::ui_widgets::SliderPlugin);
        }
        // Before `FeathersPlugins`, not after: its `build` registers embedded
        // shader assets immediately, so the asset type has to exist by then.
        // Headless has no render plugin to provide it, and the failure is a
        // runtime panic rather than a compile error.
        init_asset_once::<bevy::shader::Shader>(app);
        init_asset_once::<Mesh>(app);
        init_asset_once::<StandardMaterial>(app);
        // The type scale loads the fonts `feathers` embeds, and only `TextPlugin`
        // registers `Assets<Font>` - which the headless harness does not have.
        init_asset_once::<bevy::text::Font>(app);
        // `feathers`' menu systems read `InputFocus` and `InputFocusVisible`, and
        // 0.19 turns a missing resource into a hard error rather than skipping
        // the system. `InputFocusPlugin` owns both.
        //
        // Deliberately *not* `InputDispatchPlugin`: that one dispatches keyboard
        // events to the focused entity, which is exactly the job `KeyboardOwner`
        // does here. Taking the resources without the dispatcher keeps this tool's
        // "which region owns the keyboard" model authoritative.
        if !app.is_plugin_added::<bevy::input_focus::InputFocusPlugin>() {
            app.add_plugins(bevy::input_focus::InputFocusPlugin);
        }
        // Bevy's editor widget set, themed onto this repo's chrome palette in
        // `theme::apply_studio_theme`. Guarded on the core plugin because
        // `FeathersPlugins` is a group and has no `is_plugin_added` of its own.
        if !app.is_plugin_added::<bevy::feathers::FeathersCorePlugin>() {
            app.add_plugins(bevy::feathers::FeathersPlugins);
        }
        app.add_systems(Startup, theme::apply_studio_theme)
            .insert_resource(ClearColor(schematic_screen()))
            .init_resource::<StudioState>()
            .init_resource::<detail::TileMeshCache>()
            .init_resource::<LabMenuState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<viewport::ViewportTouch>()
            // `Touches` belongs to `InputPlugin`, which the headless harness
            // does not build - the same reason the mouse resources above are
            // initialised by hand. Without it every system in this schedule
            // fails parameter validation, not just the one that reads it.
            .init_resource::<bevy::input::touch::Touches>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseScroll>()
            .add_systems(Startup, setup_studio)
            .add_systems(
                Update,
                (
                    input::handle_chrome_input,
                    update_studio_solve.after(input::handle_chrome_input),
                    timeline::advance_timeline.after(update_studio_solve),
                    draw::rebuild_visuals.after(timeline::advance_timeline),
                    draw::rebuild_overlay.after(draw::rebuild_visuals),
                    viewport::sync_camera.after(draw::rebuild_visuals),
                    chrome::update_chrome_ui.after(update_studio_solve),
                    tabs::update_tab_row,
                    field_widgets::sync_field_rows.after(update_studio_solve),
                    viewport_input::handle_viewport_painting,
                    viewport_input::update_hover_and_cursor,
                    viewport::sync_layout_mode.before(chrome_layout::sync_chrome_layout),
                    chrome_layout::sync_chrome_layout,
                    viewport::sync_camera_viewport.after(viewport::sync_layout_mode),
                    viewport::sync_camera_touch.before(viewport::sync_camera),
                ),
            )
            .add_observer(field_widgets::apply_slider_change);

        if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
            app.insert_resource(capture::CaptureState {
                dir,
                name: String::from("studio_capture"),
                timer: 0.0,
                step: 0,
            })
            .add_systems(Update, capture::capture_system);
        }

        if let Some(path) = script::script_path() {
            match script::load_script(&path) {
                Ok(script) => {
                    app.insert_resource(script::ScriptRun {
                        script,
                        timer: 0.0,
                        phase: 0,
                    })
                    .add_systems(Update, script::script_system);
                }
                // Refusing to run beats photographing the wrong view under a
                // name that claims to be evidence.
                Err(detail) => panic!("composition studio script: {detail}"),
            }
        }
    }
}

impl Default for StudioPlugin {
    fn default() -> Self {
        Self
    }
}

fn setup_studio(mut commands: Commands, assets: Res<AssetServer>) {
    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        std::f32::consts::FRAC_PI_4,
        viewport::ISO_PITCH,
        0.0,
    );
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: viewport::DEFAULT_ZOOM,
            far: 1000.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(Vec3::new(10.0, 20.0, 10.0)).with_rotation(rotation),
        // Fill for the detail pass, so a cut-open interior is shaded rather
        // than a black hole. Low enough to stay atmosphere: the schematic's
        // emissive lines are the signal and must not be competed with.
        AmbientLight {
            color: Color::WHITE,
            brightness: draw::AMBIENT_BRIGHTNESS,
            ..default()
        },
        StudioCamera,
    ));

    // The chrome gets its own full-window camera, and this is load-bearing.
    //
    // Bevy lays UI out inside its target camera's viewport. The studio camera's
    // viewport is inset by the panel width so the facility renders beside the
    // panel - so if the UI also targeted it, the entire chrome tree would be
    // laid out from the panel's far edge and shifted right by exactly its own
    // width. That presents as "the panel renders in the wrong place", which
    // reads as a text or alignment fault and sends you looking at justification.
    //
    // Splitting the cameras makes the two coordinate spaces independent: the 3D
    // camera's viewport describes where the *world* draws, and never moves the
    // chrome. `IsDefaultUiCamera` is claimed exactly once, here - a second
    // claimant on the same window silently swallows every UI node.
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            // The 3D pass has already drawn; clearing would erase the facility.
            clear_color: ClearColorConfig::None,
            ..default()
        },
        bevy::ui::IsDefaultUiCamera,
    ));

    chrome::setup_chrome(commands, &assets);
}

/// Re-solve once the edit has settled. Debounced so dragging a value through a
/// run of steps costs one production solve rather than one per frame.
fn update_studio_solve(time: Res<Time>, mut state: ResMut<StudioState>) {
    if !state.solve_dirty {
        return;
    }
    let now = time.elapsed_secs();
    if let Some(edited) = state.last_edit
        && now - edited < SOLVE_DEBOUNCE_SECONDS
    {
        return;
    }
    state.last_edit = None;
    solve::run_solve(&mut state);
    // A new solve is a new facility, so a neighbourhood held over from the old
    // one describes cells that have moved. Recomputing it here is what makes
    // the tuning loop work at all: drag a bias, and the *distribution* on the
    // panel moves with the layout rather than lagging a solve behind it.
    if state.detail_mode == detail::DetailMode::Neighborhood {
        neighbors::refresh(&mut state);
    }
    state.geometry_dirty = true;
}
