//! Canonical hex-facility match adapter (Arc L Phase 95). Pure authoritative simulation
//! lives in `observed_match::hex_wfc`; this module owns only Bevy input, presentation,
//! feedback, replay, and screen lifecycle integration. It parallels `full_wfc/` in
//! structure and fidelity, driven by [`observed_match::hex_wfc::HexWfcMatch`].

mod architect;
mod ascent;
mod ascent_capture;
mod ask;
mod audio;
mod capture;
pub(in crate::hex_wfc) use capture::{HexWfcCapture, HexWfcCaptureMode};
mod cues;
mod entities;
mod equipment;
mod feedback;
mod guardian;
mod hud;
mod input;
mod kinetic;
mod lantern;
pub(crate) mod launch;
pub(crate) mod loading;
mod net;
mod objective_models;
pub(crate) mod overlay;
mod pad;
mod perf;
mod prison_gate;
pub(crate) use perf::GPU_PROFILE_ENV;
pub mod sim;
pub(crate) mod view;
mod vista_capture;

use bevy::prelude::*;

use crate::GameState;

/// Simulation-facing tutorial gate. The onboarding presentation toggles this resource,
/// while fixed-step logic depends only on this hex-domain seam rather than importing a
/// screen module.
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HexOnboardingGate {
    pub(crate) active: bool,
}

pub(crate) struct HexWfcPlugin;

impl Plugin for HexWfcPlugin {
    fn build(&self, app: &mut App) {
        perf::configure(app);
        // Evidence for the spectator overview needs it *up*, and a capture run
        // has no keyboard. `OBSERVED2_SPECTATE_OVERVIEW=<detent>` opens it at
        // that bearing, so a still of the iso view is reproducible rather than
        // dependent on someone holding a key at the right moment.
        let overview_detent = std::env::var("OBSERVED2_SPECTATE_OVERVIEW")
            .ok()
            .map(|value| value.trim().parse::<usize>().unwrap_or(0));
        app.init_resource::<overlay::MatchOverlayState>()
            .insert_resource(view::spectate::SpectatorOverview {
                active: overview_detent.is_some(),
                detent: overview_detent.unwrap_or(0),
                tile_radius: view::spectate::DEFAULT_TILE_RADIUS,
                ..Default::default()
            })
            .init_resource::<HexOnboardingGate>()
            .init_resource::<view::camera::OverviewFrame>()
            .init_resource::<view::PrisonView>()
            .add_observer(overlay::activate)
            .add_plugins(ask::AskPlugin)
            .add_systems(
                OnEnter(GameState::HexWfc),
                (
                    sim::setup_runtime,
                    overlay::reset,
                    view::setup_view,
                    hud::setup,
                    hud::play::setup,
                    view::map::setup,
                    feedback::setup,
                    audio::setup,
                    entities::setup,
                    equipment::setup,
                    lantern::setup,
                    guardian::setup,
                    pad::setup,
                    kinetic::setup,
                    input::grab_cursor,
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                (
                    perf::begin_fixed,
                    capture::drive_traversal_capture,
                    hud::capture::drive,
                    ascent_capture::drive,
                    sim::step_runtime,
                    perf::end_fixed,
                )
                    .chain()
                    .run_if(in_state(GameState::HexWfc)),
            )
            .add_systems(
                Update,
                (architect::systems(), architect::capture::capture)
                    .chain()
                    // After the match's hotkeys, which read whether the desk holds a card.
                    .after(input::mode_hotkeys)
                    .run_if(in_state(GameState::HexWfc)),
            )
            .add_systems(
                Update,
                (
                    (
                        input::mode_hotkeys,
                        input::release_overlay_transition_capture,
                        input::map_input,
                        overlay::sync_runtime_map,
                        overlay::rebuild,
                        input::sync_cursor,
                        overlay::adjust_focused,
                        overlay::refresh_setting_labels,
                    )
                        .chain(),
                    // Grouped: the facility's outside follows its detailed geometry.
                    (
                        view::exterior::rebuild_changed,
                        view::sync_changed_geometry,
                        view::sync_streamed_cells,
                        view::sync_prison_view,
                        prison_gate::sync_lobby_gate,
                        view::exterior::sync_visibility,
                    )
                        .chain(),
                    view::sync_practical_shadow_budget,
                    // Grouped: the spectator overview is one concern, and the
                    // flat tuple had reached Bevy's 21-system limit.
                    (
                        view::spectate::hotkeys,
                        view::camera::sync_frame,
                        view::spectate::sync_detail_window,
                        view::spectate::sync_cutaway,
                        view::spectate::sync_practicals,
                        view::spectate::trace_cutaway,
                        view::spectate::sync_key_light,
                    )
                        .chain(),
                    (
                        view::sync_camera,
                        view::sky::follow_camera,
                        view::sky::drift_clouds,
                    )
                        .chain(),
                    view::sync_projection,
                    view::sync_lighting_and_atmosphere,
                    (hud::sync, hud::play::sync, kinetic::sync_reticle).chain(),
                    view::map::sync,
                    feedback::sync,
                    feedback::animate,
                    audio::sync,
                    entities::sync,
                    // Grouped: hand equipment, posed after the hands have swayed.
                    (
                        equipment::sway,
                        lantern::sync_projection,
                        lantern::sync_dynamic,
                        lantern::sync_core_glow,
                        lantern::sync_anchor_ghost,
                        pad::sync_projection,
                        pad::sync_dynamic,
                        kinetic::sync_held,
                        kinetic::read_shots,
                        kinetic::pose_held,
                        equipment::spin,
                    )
                        .chain(),
                    guardian::systems(),
                    sim::finish_runtime,
                )
                    .chain()
                    .run_if(in_state(GameState::HexWfc)),
            )
            .add_systems(
                OnExit(GameState::HexWfc),
                (
                    input::release_cursor,
                    view::clear_view,
                    view::spectate::clear,
                    view::map::cleanup,
                    feedback::cleanup,
                    audio::cleanup,
                    entities::cleanup,
                    lantern::cleanup,
                    guardian::cleanup,
                    pad::cleanup,
                    kinetic::cleanup,
                    equipment::cleanup,
                    hud::play::cleanup,
                    sim::cleanup_runtime,
                )
                    .chain(),
            );
        capture::configure(app);
    }
}
