//! State and wiring for `module_studio`.

use std::path::PathBuf;

use bevy::prelude::*;
use observed_style::schematic_screen;

use crate::module::diagnose::{Diagnosis, diagnose_file};
use crate::module::watch::{ModuleWatch, default_dir, poll_watch};
use crate::viewport::{ISO_PITCH, StudioCamera, detent_yaw};

/// Everything on screen, rebuilt whenever the watcher reports a change.
#[derive(Resource)]
pub struct ModuleState {
    /// Every module in the corpus, in scan order.
    pub diagnoses: Vec<Diagnosis>,
    /// Which one is on screen.
    pub selected: usize,
    /// Redraw the geometry.
    pub dirty: bool,
    pub zoom: f32,
    pub detent: usize,
    /// The generation the current diagnoses were built from, so a poll that
    /// found nothing does not cause a re-parse of the whole corpus.
    pub built_from: u64,
    pub status: String,
    /// Which recipe step is held, and which of its parameters.
    ///
    /// Kept on the state rather than derived from the diagnosis, because
    /// the diagnosis is rebuilt on every file change and the author's
    /// selection has to survive their own edit.
    pub step: usize,
    pub param: usize,
}

impl Default for ModuleState {
    fn default() -> Self {
        Self {
            diagnoses: Vec::new(),
            selected: 0,
            dirty: true,
            zoom: 1.0,
            detent: 0,
            built_from: 0,
            status: String::from("scanning"),
            step: 0,
            param: 0,
        }
    }
}

impl ModuleState {
    /// The module on screen, if the corpus is not empty.
    #[must_use]
    pub fn current(&self) -> Option<&Diagnosis> {
        self.diagnoses.get(self.selected)
    }

    /// How many modules currently fail.
    #[must_use]
    pub fn failing(&self) -> usize {
        self.diagnoses
            .iter()
            .filter(|diagnosis| !diagnosis.is_clean())
            .count()
    }

    /// Move to the next module that fails, wrapping.
    ///
    /// The point of a corpus-wide validator is to walk the failures, not to
    /// page through 58 clean modules looking for them.
    pub fn select_next_failing(&mut self) {
        let count = self.diagnoses.len();
        if count == 0 {
            return;
        }
        for step in 1..=count {
            let index = (self.selected + step) % count;
            if !self.diagnoses[index].is_clean() {
                self.selected = index;
                self.dirty = true;
                return;
            }
        }
        self.status = String::from("every module validates; nothing to jump to");
    }

    /// Clamp the step/parameter cursor to what the current recipe has.
    ///
    /// Called after any change to either, so an edit that removes the last step
    /// cannot leave the cursor pointing past the end and silently stop
    /// responding to arrow keys.
    pub fn clamp_cursor(&mut self) {
        // The counts are read out before anything is written back, so the
        // immutable borrow of the diagnosis ends before the cursor moves.
        let shape: Option<(usize, Vec<usize>)> = self
            .current()
            .and_then(|d| d.recipe.as_ref())
            .map(|recipe| {
                (
                    recipe.steps.len(),
                    recipe.steps.iter().map(|s| s.params().len()).collect(),
                )
            });
        let Some((steps, param_counts)) = shape else {
            self.step = 0;
            self.param = 0;
            return;
        };
        if steps == 0 {
            self.step = 0;
            self.param = 0;
            return;
        }
        self.step = self.step.min(steps - 1);
        let count = param_counts[self.step];
        self.param = if count == 0 {
            0
        } else {
            self.param.min(count - 1)
        };
    }

    /// Adjust the held parameter by `delta` and write the recipe back to disk.
    ///
    /// Saving on every keystroke is deliberate: the file is the document, the
    /// watcher is what re-previews it, and an in-memory edit that had to be
    /// committed separately would let the viewport and the file disagree about
    /// what the module is. Recipes are small and this is a human-paced loop.
    pub fn nudge_param(&mut self, delta: f64) {
        self.clamp_cursor();
        let (step, param) = (self.step, self.param);
        let Some(diagnosis) = self.diagnoses.get_mut(self.selected) else {
            return;
        };
        let path = diagnosis.path.clone();
        let Some(recipe) = diagnosis.recipe.as_mut() else {
            return;
        };
        let Some(target) = recipe.steps.get_mut(step) else {
            return;
        };
        let Some(&(name, value)) = target.params().get(param) else {
            return;
        };
        if !target.set_param(name, value + delta) {
            return;
        }
        let text = recipe.to_ron();
        let label = format!("{name} -> {:.2}", value + delta);
        match std::fs::write(&path, text.as_bytes()) {
            // The watcher picks the write up and re-diagnoses, so the viewport
            // and the validity verdict both follow within a poll.
            Ok(()) => self.status = label,
            Err(error) => self.status = format!("cannot write {}: {error}", path.display()),
        }
    }

    pub fn select(&mut self, index: usize) {
        if self.diagnoses.is_empty() {
            return;
        }
        self.selected = index % self.diagnoses.len();
        self.step = 0;
        self.param = 0;
        self.dirty = true;
    }
}

/// Re-diagnose the corpus when the watcher says it moved.
///
/// Whole-corpus rather than per-file: 58 modules parse in a few milliseconds,
/// and the alternative is tracking which diagnosis belongs to which mtime and
/// getting it subtly wrong the first time a file is renamed.
pub fn rebuild_diagnoses(watch: Res<ModuleWatch>, mut state: ResMut<ModuleState>) {
    if watch.generation == state.built_from {
        return;
    }
    let previous = state.current().map(Diagnosis::name);

    state.diagnoses = watch
        .scan()
        .iter()
        .map(|path| diagnose_file(path))
        .collect();
    state.built_from = watch.generation;
    state.dirty = true;

    // Follow the module by name, not by index: a file added earlier in the
    // sort order would otherwise silently swap what you are looking at.
    if let Some(name) = previous
        && let Some(index) = state
            .diagnoses
            .iter()
            .position(|diagnosis| diagnosis.name() == name)
    {
        state.selected = index;
    }
    state.selected = state.selected.min(state.diagnoses.len().saturating_sub(1));
    state.clamp_cursor();

    let failing = state.failing();
    let total = state.diagnoses.len();
    state.status = if total == 0 {
        format!("no .map files under {}", watch.dir.display())
    } else if failing == 0 {
        format!("{total} modules, all valid")
    } else {
        format!("{total} modules, {failing} failing")
    };
}

/// Keys: page modules, jump to the next failure, orbit, zoom.
pub fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ModuleState>,
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
) {
    if keyboard.just_pressed(KeyCode::BracketRight) || keyboard.just_pressed(KeyCode::ArrowDown) {
        let next = state.selected + 1;
        state.select(next);
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) || keyboard.just_pressed(KeyCode::ArrowUp) {
        let next = state.selected + state.diagnoses.len().saturating_sub(1);
        state.select(next);
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        state.select_next_failing();
    }
    if keyboard.just_pressed(KeyCode::KeyQ) {
        state.detent = (state.detent + 1) % crate::viewport::AZIMUTH_DETENTS;
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        state.detent = (state.detent + crate::viewport::AZIMUTH_DETENTS - 1)
            % crate::viewport::AZIMUTH_DETENTS;
        state.dirty = true;
    }
    // Recipe editing. Deliberately different keys from the module paging
    // above, so a key never means two things at once - the same rule the
    // sibling tool enforces with ownership.
    if state.current().is_some_and(|d| d.is_parametric()) {
        if keyboard.just_pressed(KeyCode::KeyX) {
            state.step += 1;
            state.param = 0;
            state.clamp_cursor();
        }
        if keyboard.just_pressed(KeyCode::KeyS) && !keyboard.pressed(KeyCode::ControlLeft) {
            state.step = state.step.saturating_sub(1);
            state.param = 0;
            state.clamp_cursor();
        }
        if keyboard.just_pressed(KeyCode::KeyD) {
            state.param += 1;
            state.clamp_cursor();
        }
        if keyboard.just_pressed(KeyCode::KeyA) {
            state.param = state.param.saturating_sub(1);
            state.clamp_cursor();
        }
        let coarse = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        let unit = if coarse { 8.0 } else { 1.0 };
        if keyboard.just_pressed(KeyCode::ArrowRight) {
            state.nudge_param(unit);
        }
        if keyboard.just_pressed(KeyCode::ArrowLeft) {
            state.nudge_param(-unit);
        }
    }

    if scroll.delta.y.abs() > f32::EPSILON {
        state.zoom = (state.zoom * 1.14_f32.powf(-scroll.delta.y)).clamp(0.05, 4.0);
    }
}

/// Frame the module. One cell is 16 m across corners, so the framing is fixed
/// rather than fitted - paging between modules should not make the camera jump.
pub fn sync_camera(
    state: Res<ModuleState>,
    mut camera: Query<(&mut Projection, &mut Transform), With<StudioCamera>>,
) {
    let Ok((mut projection, mut transform)) = camera.single_mut() else {
        return;
    };
    let rotation = Quat::from_euler(EulerRot::YXZ, detent_yaw(state.detent), ISO_PITCH, 0.0);
    let centre = Vec3::new(0.0, 4.0, 0.0);
    *transform =
        Transform::from_translation(centre + rotation * Vec3::Z * 80.0).with_rotation(rotation);
    *projection = Projection::Orthographic(OrthographicProjection {
        scale: 0.030 * state.zoom,
        near: 0.1,
        far: 400.0,
        ..OrthographicProjection::default_3d()
    });
}

pub struct ModuleStudioPlugin {
    pub dir: PathBuf,
}

impl Default for ModuleStudioPlugin {
    fn default() -> Self {
        Self { dir: default_dir() }
    }
}

impl Plugin for ModuleStudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(schematic_screen()))
            .insert_resource(ModuleWatch::new(self.dir.clone()))
            .init_resource::<ModuleState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseScroll>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_systems(Startup, (setup_camera, super::panel::setup_panel))
            .add_systems(
                Update,
                (
                    poll_watch,
                    rebuild_diagnoses.after(poll_watch),
                    handle_input.after(rebuild_diagnoses),
                    super::render::rebuild_module_view.after(handle_input),
                    sync_camera.after(handle_input),
                    super::panel::update_panel.after(rebuild_diagnoses),
                ),
            );

        // Same capture contract as the sibling tool, so the evidence rule
        // applies to this one too: capture the PNG *and look at it*.
        if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
            app.insert_resource(crate::capture::CaptureState {
                dir,
                name: String::from("module_studio"),
                timer: 0.0,
                step: 0,
            })
            .add_systems(Update, crate::capture::capture_system);
        }

        // Which module to open on, by file stem. Capturing a specific failure
        // is the whole point of the evidence path, and paging to it by hand is
        // not something a headless run can do.
        if let Ok(name) = std::env::var("OBSERVED2_MODULE") {
            app.insert_resource(OpenOn(name))
                .add_systems(Update, select_requested_module);
        }
    }
}

/// A module to select once the corpus has been diagnosed.
#[derive(Resource)]
pub struct OpenOn(pub String);

fn select_requested_module(
    open_on: Res<OpenOn>,
    mut state: ResMut<ModuleState>,
    mut done: Local<bool>,
) {
    if *done || state.diagnoses.is_empty() {
        return;
    }
    *done = true;
    match state
        .diagnoses
        .iter()
        .position(|diagnosis| diagnosis.name() == open_on.0)
    {
        Some(index) => state.select(index),
        // Loud rather than silent: a capture of the wrong module that looks
        // plausible is worse than no capture.
        None => state.status = format!("OBSERVED2_MODULE={:?} not found", open_on.0),
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: 0.03,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(Vec3::new(20.0, 30.0, 20.0)),
        AmbientLight {
            color: Color::WHITE,
            brightness: crate::draw::AMBIENT_BRIGHTNESS,
            ..default()
        },
        StudioCamera,
    ));
    // Chrome gets its own full-window camera. The 3D camera here has no inset
    // viewport, but claiming `IsDefaultUiCamera` explicitly is what keeps that
    // true if one is ever added - see the docked-panel bug in the sibling tool.
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        bevy::ui::IsDefaultUiCamera,
    ));
}
