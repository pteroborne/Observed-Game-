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
pub(crate) mod cosmetics;
mod cues;
mod doors;
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
mod modal_ui;
mod net;
mod objective_models;
mod observer;
pub(crate) mod overlay;
mod pad;
mod perf;
mod power;
mod prison_gate;
mod sensors;
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
        view::cistern::install(app);
        view::chargeworks::install(app);
        view::rain::install(app);
        view::concourse::install(app);
        view::jade::install(app);
        view::promenade::install(app);
        view::spectate::overview_props::schedule(app);
        // Evidence for the spectator overview needs it *up*, and a capture run
        // has no keyboard. `OBSERVED2_SPECTATE_OVERVIEW=<detent>` opens it at
        // that bearing, so a still of the iso view is reproducible rather than
        // dependent on someone holding a key at the right moment.
        let overview_detent = std::env::var("OBSERVED2_SPECTATE_OVERVIEW")
            .ok()
            .map(|value| value.trim().parse::<usize>().unwrap_or(0));
        app.init_resource::<overlay::MatchOverlayState>()
            .add_systems(Startup, view::configure_clusters)
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
            .add_systems(OnExit(GameState::HexWfc), cosmetics::cleanup)
            .add_systems(
                OnEnter(GameState::HexWfc),
                (
                    sim::setup_runtime,
                    overlay::reset,
                    modal_ui::setup,
                    view::setup_view,
                    hud::setup,
                    hud::play::setup,
                    view::map::setup,
                    feedback::setup,
                    audio::setup,
                    cosmetics::setup,
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
                PostUpdate,
                modal_ui::target_roots
                    .before(bevy::ui::UiSystems::Prepare)
                    .run_if(in_state(GameState::HexWfc)),
            )
            .add_systems(
                FixedUpdate,
                (
                    perf::begin_fixed,
                    capture::drive_traversal_capture,
                    hud::capture::drive,
                    ascent_capture::drive,
                    power::capture::drive,
                    kinetic::capture::drive,
                    doors::capture::drive,
                    sensors::capture::drive,
                    sim::step_runtime,
                    audio::sync,
                    kinetic::read_shots,
                    perf::end_fixed,
                )
                    .chain()
                    .run_if(in_state(GameState::HexWfc)),
            )
            .add_systems(
                Update,
                ascent::join_rogue_board
                    .before(input::mode_hotkeys)
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
                        kinetic::arm_and_fire,
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
                    (
                        view::sync_practical_shadow_budget,
                        view::sync_storey_shadow_casters,
                    ),
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
                        view::sky::sync_mood,
                        view::sky::drift_clouds,
                    )
                        .chain(),
                    view::sync_projection,
                    view::sync_lighting_and_atmosphere,
                    (
                        power::sync_fixtures,
                        power::sync_practicals,
                        power::read_changes,
                        doors::sync,
                        sensors::sync,
                    )
                        .chain(),
                    (hud::sync, hud::play::sync, kinetic::sync_reticle).chain(),
                    view::map::sync,
                    feedback::sync,
                    feedback::animate,
                    (entities::sync, observer::sync, cosmetics::sync).chain(),
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
                    power::cleanup,
                    doors::cleanup,
                    sensors::cleanup,
                    equipment::cleanup,
                    hud::play::cleanup,
                    sim::cleanup_runtime,
                )
                    .chain(),
            );
        capture::configure(app);
    }
}

mod guidance_capture;
pub(crate) use guidance_capture::{Case as GuidanceCaptureCase, stage as stage_guidance_capture};

#[cfg(test)]
#[path = "../tests/ux_guidance.rs"]
mod ux_guidance;

#[cfg(test)]
#[path = "../tests/ux_completion.rs"]
mod ux_completion;
