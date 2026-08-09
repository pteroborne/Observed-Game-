//! State and wiring for `module_studio`.

use std::path::PathBuf;

use bevy::prelude::*;
use observed_style::schematic_screen;

use crate::module::certify::{CertificationState, certify_selected};
use crate::module::diagnose::{Diagnosis, diagnose_file};
use crate::module::rapier_audit::{RapierAuditState, audit_projected_guide, projected_guide};
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
    /// Hide the ceiling and the walls facing the camera.
    ///
    /// **On by default**, unlike the facility view. A module is almost
    /// always a sealed box, and the reason to open one here is to inspect
    /// its interior - headroom, a ramp surface, a socket. Opening on a
    /// solid mass would make the first action of every session the same
    /// keystroke.
    pub cutaway: bool,
    /// How many hulls the last rebuild hid, so the panel can say.
    pub cut_hulls: usize,
    /// The last walk through the selected module, if it has a route.
    pub walk: Option<crate::module::walk::WalkReport>,
    /// Identity-projected compatibility guide for the standalone module.
    pub guide: Option<observed_match::hex_wfc::ProjectedTraversalGuide>,
    /// Explicit state of the optional production follower/Rapier audit.
    pub rapier_audit: RapierAuditState,
    /// Explicit state of the optional authoritative certification run.
    ///
    /// Separate from `rapier_audit` because they answer different questions:
    /// the audit says a projected guide carried a body, certification also
    /// names the profile, guide, and digests that make the result comparable
    /// to what `tilec` proved.
    pub certification: CertificationState,
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
            cutaway: true,
            cut_hulls: 0,
            walk: None,
            guide: None,
            rapier_audit: RapierAuditState::Off,
            certification: CertificationState::Off,
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
                self.refresh_selected_evidence();
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
        self.refresh_selected_evidence();
        self.dirty = true;
    }

    /// Recompute every diagnostic projection owned by the selected module.
    ///
    /// Keeping this as one operation prevents a watcher rebuild or selection
    /// change from showing a guide/audit that belongs to the previous module.
    pub fn refresh_selected_evidence(&mut self) {
        let audit_enabled = self.rapier_audit.is_enabled();
        let (walk, guide, audit) = if let Some(diagnosis) = self.current() {
            let walk = crate::module::route::walk_module(diagnosis);
            let guide = diagnosis.prototype.as_ref().and_then(projected_guide);
            let audit = audit_enabled.then(|| {
                diagnosis.prototype.as_ref().zip(guide.as_ref()).map_or(
                    RapierAuditState::NotApplicable,
                    |(prototype, guide)| {
                        RapierAuditState::Complete(audit_projected_guide(prototype, guide))
                    },
                )
            });
            (walk, guide, audit)
        } else {
            (
                None,
                None,
                audit_enabled.then_some(RapierAuditState::NotApplicable),
            )
        };
        self.walk = walk;
        self.guide = guide;
        if let Some(audit) = audit {
            self.rapier_audit = audit;
        }
        if self.certification.is_enabled() {
            self.certification = self
                .current()
                .map_or(CertificationState::NotApplicable, certify_selected);
        }
    }

    pub fn toggle_certification(&mut self) {
        self.certification = if self.certification.is_enabled() {
            CertificationState::Off
        } else {
            CertificationState::NotApplicable
        };
        self.refresh_selected_evidence();
        self.dirty = true;
    }

    pub fn toggle_rapier_audit(&mut self) {
        self.rapier_audit = if self.rapier_audit.is_enabled() {
            RapierAuditState::Off
        } else {
            RapierAuditState::NotApplicable
        };
        self.refresh_selected_evidence();
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

    state.refresh_selected_evidence();

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

/// Frame the module. One cell is 16 m across corners, so the framing is fixed
/// rather than fitted - paging between modules should not make the camera jump.
pub fn sync_camera(
    state: Res<ModuleState>,
    view: Res<super::neighbor_view::NeighbourView>,
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
        // The ring spans three cells where the plain view spans one, so the
        // mode pulls the camera back. Zoom still multiplies on top.
        scale: 0.030 * state.zoom * super::neighbor_view::ring_scale(&view),
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
        let mut initial_state = ModuleState::default();
        if std::env::var("OBSERVED2_MODULE_RAPIER").is_ok_and(|value| value != "0") {
            initial_state.rapier_audit = RapierAuditState::NotApplicable;
        }
        // Opening straight into the ring, so a headless capture can photograph
        // the mode. Without this there is no way to produce evidence for it.
        let ring_open =
            std::env::var("OBSERVED2_MODULE_NEIGHBOURS").is_ok_and(|value| value != "0");
        if ring_open {
            // The same default `neighbor_view::toggle` applies on the
            // interactive path, so a capture photographs the mode as an author
            // meets it rather than a variant only the env hook can produce.
            initial_state.cutaway = false;
        }
        let neighbours = super::neighbor_view::NeighbourView::opened(ring_open);
        app.insert_resource(ClearColor(schematic_screen()))
            .insert_resource(ModuleWatch::new(self.dir.clone()))
            .insert_resource(initial_state)
            .insert_resource(neighbours)
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
                    super::input::handle_input.after(rebuild_diagnoses),
                    // Between the input and the draw, so a watcher reparse and
                    // a keyboard selection both reach the ring in the same
                    // frame they reach the centre.
                    super::neighbor_view::refresh_ring.after(super::input::handle_input),
                    super::render::rebuild_module_view.after(super::neighbor_view::refresh_ring),
                    sync_camera.after(super::input::handle_input),
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

        // Roll the ring before the shot. Without it every capture photographs
        // the heaviest candidate on every face, which for this corpus is the
        // same module six times - true, and useless as a picture of variety.
        if let Some(rolls) = std::env::var("OBSERVED2_MODULE_ROLL")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
        {
            app.insert_resource(RollOnOpen(rolls)).add_systems(
                Update,
                roll_requested_ring.after(super::neighbor_view::refresh_ring),
            );
        }
    }
}

/// How many times to roll the ring once it exists.
#[derive(Resource)]
pub struct RollOnOpen(pub u32);

fn roll_requested_ring(
    rolls: Res<RollOnOpen>,
    open_on: Option<Res<OpenOn>>,
    mut state: ResMut<ModuleState>,
    mut view: ResMut<super::neighbor_view::NeighbourView>,
    mut done: Local<bool>,
) {
    // The ring has to exist first, which takes a diagnose pass and a refresh.
    if *done || !view.on || view.ring.cells.is_empty() {
        return;
    }
    // And it has to be the *requested* module's ring. Rolling the ring of
    // whatever happened to be selected first, before `select_requested_module`
    // moved to the module the capture asked for, hands the roll to a ring that
    // `refresh_ring` is about to discard - which presents as a status line
    // claiming a roll over a picture that plainly did not move.
    if let Some(open_on) = open_on.as_ref()
        && state
            .current()
            .is_none_or(|current| current.name() != open_on.0)
    {
        return;
    }
    *done = true;
    for _ in 0..rolls.0 {
        state.status = view.roll();
    }
    state.dirty = true;
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn diagnosis(stem: &str) -> Diagnosis {
        diagnose_file(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/tiles/authored")
                .join(format!("{stem}.map")),
        )
    }

    #[test]
    fn rapier_toggle_has_explicit_off_and_not_applicable_states() {
        let mut state = ModuleState::default();
        state.toggle_rapier_audit();
        assert_eq!(state.rapier_audit, RapierAuditState::NotApplicable);
        state.toggle_rapier_audit();
        assert_eq!(state.rapier_audit, RapierAuditState::Off);
    }

    #[test]
    fn selection_refreshes_an_enabled_audit_without_inventing_flat_routes() {
        let mut state = ModuleState {
            diagnoses: vec![
                diagnosis("hall_ramp_perimeter_120"),
                diagnosis("hall_straight"),
            ],
            rapier_audit: RapierAuditState::NotApplicable,
            ..ModuleState::default()
        };

        state.refresh_selected_evidence();
        assert!(matches!(
            state.rapier_audit,
            RapierAuditState::Complete(ref report) if report.passed()
        ));
        assert!(state.guide.is_some());

        state.select(1);
        assert_eq!(state.rapier_audit, RapierAuditState::NotApplicable);
        assert!(state.guide.is_none());
        assert!(
            state.walk.is_some(),
            "flat modules retain advisory preflight"
        );
    }
}
