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
pub mod coverage;
pub mod detail;
pub mod draw;
pub mod field_widgets;
pub mod input;
pub mod layer;
pub mod module;
pub mod neighbors;
pub mod panels;
pub mod persist;
pub mod pick;
pub mod script;
pub mod solve;
pub mod state;
pub mod timeline;
pub mod tunables;
pub mod viewport;
pub mod viewport_input;

/// Pin and coverage gates, split out so neither test file outgrows the
/// 600-line review budget the rest of the WFC path lives under.
#[cfg(test)]
pub mod authoring_tests;
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
pub use state::{
    CatalogHash, Corpus, MAX_WORKING_LEVELS, ProfileOrigin, SolveResult, StudioState, corpus,
};
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

pub struct StudioPlugin;

impl Plugin for StudioPlugin {
    fn build(&self, app: &mut App) {
        // Without a slider plugin the sliders spawn, lay out, and draw exactly as
        // they should while ignoring every drag.
        //
        // `SliderPlugin` alone, not the whole `UiWidgetsPlugins` group. The
        // group's menu plugin runs `Update` systems against `InputFocus`, a
        // resource owned by a plugin this tool does not install - which turns
        // every headless test into a panic. Taking only the widget in use also
        // keeps the focus story straight: this tool routes the keyboard through
        // `KeyboardOwner`, and never through Bevy's focus resource.
        //
        // Guarded because 0.19's `DefaultPlugins` ships `UiWidgetsPlugins` (0.18's
        // did not), so the windowed studio already has it and adding it again is a
        // panic. Headless tests build on `MinimalPlugins` and still need it here.
        if !app.is_plugin_added::<bevy::ui_widgets::SliderPlugin>() {
            app.add_plugins(bevy::ui_widgets::SliderPlugin);
        }
        app.insert_resource(ClearColor(schematic_screen()))
            .init_resource::<StudioState>()
            .init_resource::<detail::TileMeshCache>()
            .init_resource::<LabMenuState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseScroll>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
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
                    field_widgets::sync_field_rows.after(update_studio_solve),
                    viewport_input::handle_viewport_painting,
                    viewport_input::update_hover_and_cursor,
                    viewport::sync_camera_viewport,
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

fn setup_studio(mut commands: Commands) {
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

    chrome::setup_chrome(commands);
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
