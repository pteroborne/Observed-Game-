//! The assembled game's state-scoped frontend: Splash → Main Menu → preset-first Play
//! Hub → prepared canonical hex match → Results/Replay, with an optional LAN browser
//! and lobby. Shared semantic widgets provide pointer/keyboard/controller parity,
//! stable focus, accessible disabled states, and one visual language; each screen owns
//! its typed actions.
//!
//! Career/profile state comes from [`crate::flow`], authoritative network state from
//! [`crate::lan`], and played match logic from [`crate::hex_wfc`]. Presentation reads
//! those domains and never reconstructs their rules from widget entities.
//!
//! This module owns the screen composition plugins. Shared state lives at its real layer —
//! simulation resources in [`crate::sim`], presentation components/theme/assets in
//! [`crate::view`], spatial constants in [`crate::layout`].

use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::GameState;
use crate::sim::director::MatchDirector;
use crate::sim::state::SpectatorBot;

pub(crate) mod audio;
pub(crate) mod hud;
pub(crate) mod input;
pub(crate) mod lan;
pub(crate) mod loading;
pub(crate) mod loadout;
pub(crate) mod lobby;
pub(crate) mod main_menu;
pub(crate) mod match_runtime;
pub(crate) mod menu;
pub(crate) mod onboarding;
pub(crate) mod place;
pub(crate) mod play;
pub(crate) mod replay;
pub(crate) mod results;
pub(crate) mod settings;
pub(crate) mod widgets;

#[derive(Resource)]
pub struct SplashTimer(pub Timer);

// --- composition -----------------------------------------------------------
/// The menu flow: every menu-like screen (splash/main-menu/loadout/lobby/results) and
/// the shared keyboard/controller navigation. Inert where there are no buttons.
pub(crate) struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(widgets::FrontendWidgetsPlugin)
            .init_resource::<settings::SettingsRebind>()
            .init_resource::<crate::hex_wfc::loading::HexLaunchRequestSequence>()
            .init_resource::<crate::hex_wfc::loading::HexLoadingState>()
            .add_observer(lan::activate_browser)
            .add_observer(lobby::activate)
            .add_observer(loading::activate)
            .add_observer(onboarding::activate)
            .add_observer(settings::activate)
            .add_observer(loadout::activate)
            .add_observer(results::activate)
            .add_observer(replay::activate)
            .add_systems(OnEnter(GameState::Splash), menu::setup_splash)
            .add_systems(OnExit(GameState::Splash), menu::cleanup_splash)
            .add_observer(main_menu::activate)
            .add_observer(play::activate_hub)
            .add_observer(play::activate_advanced)
            .add_systems(OnEnter(GameState::MainMenu), main_menu::setup)
            .add_systems(OnEnter(GameState::Play), play::setup_hub)
            .add_systems(OnEnter(GameState::PlayAdvanced), play::setup_advanced)
            .add_systems(OnEnter(GameState::LanBrowser), lan::setup_browser)
            .add_systems(
                OnEnter(GameState::Loading),
                (crate::hex_wfc::loading::start_loading, loading::setup).chain(),
            )
            .add_systems(
                OnExit(GameState::Loading),
                crate::hex_wfc::loading::cleanup_loading_worker,
            )
            .add_systems(OnEnter(GameState::Loadout), loadout::setup)
            .add_systems(OnEnter(GameState::Lobby), lobby::setup_lobby)
            .add_systems(OnEnter(GameState::Results), results::setup)
            .add_systems(OnEnter(GameState::Replay), replay::setup_replay)
            .add_systems(OnExit(GameState::Replay), replay::cleanup)
            .add_systems(OnEnter(GameState::Settings), settings::setup)
            .add_systems(OnExit(GameState::Settings), settings::cleanup)
            .add_systems(OnEnter(GameState::HexWfc), onboarding::spawn)
            .add_systems(OnExit(GameState::HexWfc), onboarding::cleanup)
            .add_systems(
                Update,
                (
                    lan::poll_lan,
                    lan::edit_direct_address.run_if(in_state(GameState::LanBrowser)),
                    lan::refresh_browser_text.run_if(in_state(GameState::LanBrowser)),
                    lan::refresh_lobby_text.run_if(in_state(GameState::Lobby)),
                    menu::splash_advance.run_if(in_state(GameState::Splash)),
                    onboarding::release_capture_after_dismissal,
                    main_menu::update_banner.run_if(in_state(GameState::MainMenu)),
                    play::refresh_hub.run_if(in_state(GameState::Play)),
                    play::refresh_advanced.run_if(in_state(GameState::PlayAdvanced)),
                    loadout::refresh.run_if(in_state(GameState::Loadout)),
                    lobby::lobby_update_labels.run_if(in_state(GameState::Lobby)),
                    crate::hex_wfc::loading::poll_loading.run_if(in_state(GameState::Loading)),
                    loading::refresh.run_if(in_state(GameState::Loading)),
                    replay::advance_playback.run_if(in_state(GameState::Replay)),
                    replay::refresh_controls.run_if(in_state(GameState::Replay)),
                    replay::update_replay_info.run_if(in_state(GameState::Replay)),
                    replay::draw_replay_map.run_if(in_state(GameState::Replay)),
                    audio::play_frontend_feedback,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    settings::adjust_focused.after(InputSystems),
                    settings::capture_rebind,
                    settings::refresh_labels,
                )
                    .chain()
                    .run_if(in_state(GameState::Settings)),
            );
    }
}

/// [DEPRECATED] Legacy place-based first-person match plugin.
/// Sunsetted in favor of `hex_wfc::HexWfcPlugin`. Kept only as a regression testing fixture.
#[deprecated(
    since = "2.0.0",
    note = "Legacy isolated-place match. Use FullWfcPlugin instead."
)]
pub(crate) struct MatchPlugin;

// Suppresses this crate's own `#[deprecated]` on `MatchPlugin` above - the legacy
// match is kept deliberately as a regression fixture, so naming it here is intended.
// It is not a licence to ignore engine deprecations inside this impl.
#[allow(deprecated)]
impl Plugin for MatchPlugin {
    fn build(&self, app: &mut App) {
        let in_match = || in_state(GameState::Match).and_then(resource_exists::<MatchDirector>);
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .init_resource::<place::LastRenderedSignature>()
            .init_resource::<match_runtime::pause_settings::PauseSettingsOpen>()
            .init_resource::<match_runtime::pause_settings::PauseSettingsCursor>()
            .init_resource::<match_runtime::pause_settings::PauseSettingsRebind>()
            .add_systems(
                OnEnter(GameState::Match),
                (
                    match_runtime::setup_match,
                    audio::spawn_match_setpieces,
                    match_runtime::input::grab_match_cursor,
                    match_runtime::ambience::apply_match_atmosphere,
                )
                    .chain(),
            )
            .add_systems(Update, place::update_carried_torch_light.run_if(in_match()))
            .add_systems(
                OnExit(GameState::Match),
                (
                    match_runtime::input::release_match_cursor,
                    match_runtime::cleanup_match_resources,
                    match_runtime::ambience::clear_match_atmosphere,
                ),
            )
            .add_systems(
                FixedUpdate,
                match_runtime::drive_spectator_bot
                    .run_if(in_match().and_then(resource_exists::<SpectatorBot>))
                    .before(match_runtime::crossing::teleport_sim),
            )
            .add_systems(
                FixedUpdate,
                match_runtime::crossing::teleport_sim.run_if(in_match()),
            )
            .add_systems(
                Update,
                (
                    input::match_input.after(InputSystems),
                    hud::toggle_tac_map,
                    match_runtime::match_pump,
                    match_runtime::item_actions,
                    crate::guardian::update_guardian_in_match,
                    match_runtime::sync_threshold_closures,
                    crate::sim::sightings::record_rival_sightings,
                    crate::sim::knowledge::record_map_knowledge,
                    place::rebuild_place,
                    place::normalize_imported_threshold_materials,
                    place::update_tether_monitors,
                    place::update_guardian_monitors,
                    place::sync_observation_monitors,
                    place::interact_guardian_console,
                )
                    .chain()
                    .run_if(in_match()),
            )
            .add_systems(
                Update,
                place::preview::isolate_passage_previews
                    .after(place::rebuild_place)
                    .before(place::normalize_imported_threshold_materials)
                    .run_if(in_match()),
            )
            .add_systems(
                Update,
                (
                    match_runtime::pause_settings::toggle_pause_settings,
                    match_runtime::pause_settings::pause_settings_navigate,
                    match_runtime::pause_settings::pause_settings_adjust,
                    match_runtime::pause_settings::pause_settings_activate,
                    match_runtime::pause_settings::pause_settings_capture_rebind,
                    match_runtime::pause_settings::draw_pause_settings,
                )
                    .chain()
                    // After match_pump: on the frame Escape cancels an in-flight
                    // rebind, match_pump must still see the capture active so the
                    // same press cannot also unpause (or quit via Q).
                    .after(match_runtime::match_pump)
                    .run_if(in_match()),
            )
            .add_systems(
                Update,
                (
                    audio::update_audio_director,
                    match_runtime::ambience::apply_place_atmosphere,
                    match_runtime::ambience::sync_decohere_fx,
                    match_runtime::ambience::flicker_lights,
                    match_runtime::keystone_pickup,
                    place::sync_rival_avatars,
                    audio::sync_match_audio,
                    audio::bleed_rival_sound,
                    audio::fade_ambience_beds,
                    audio::sync_match_stings,
                    place::animate_doors,
                    hud::update_teleport_animation,
                    place::animate_teleport_pad_glow,
                    place::present_match_camera,
                    place::preview::sync_portal_cameras,
                    crate::view::sprites::face_billboard_sprites,
                    hud::match_draw,
                    hud::update_interaction_reticle,
                    hud::draw_tac_map,
                    crate::sim::replay::record_match_replay_sample,
                )
                    .chain()
                    .run_if(in_match()),
            );
    }
}
