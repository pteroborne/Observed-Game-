//! The assembled game's UX: a Bevy state machine that strings the proven systems
//! into one cohesive loop — Splash → Main Menu → Loadout → Lobby → Match → Results
//! → Menu — with a persistent career, a unified visual theme, keyboard/controller navigation,
//! a first-person 3D match with an in-match HUD and pause, and strict state-scoped
//! cleanup (every screen's entities despawn on exit, so transitions never leak).
//!
//! Each screen reuses a proven model: the **career/profile** ([`crate::flow`] +
//! `progression_lab`), the **lobby/matchmaking** (`session_lab`), and the **match** —
//! the live, first-person, networked hybrid match (`net_match_lab`'s `LiveNetMatch`
//! over `fps_hybrid_match_lab`). The state machine and presentation are the only new
//! code; the game logic is the labs.
//!
//! This module owns the screen flow itself: the menu-domain components and the two
//! composition plugins. The shared state it used to host now lives at its real layer —
//! simulation resources in [`crate::sim`], presentation components/theme/assets in
//! [`crate::view`], spatial constants in [`crate::layout`].

use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::GameState;
use crate::sim::director::MatchDirector;
use crate::sim::state::SpectatorBot;
use crate::view::theme::DIM;

pub(crate) mod audio;
pub(crate) mod hud;
pub(crate) mod input;
pub(crate) mod loadout;
pub(crate) mod lobby;
pub(crate) mod match_runtime;
pub(crate) mod menu;
pub(crate) mod place;

// --- menu domain -------------------------------------------------------------
#[derive(Clone, Copy)]
pub(crate) enum MenuAction {
    Goto(GameState),
    StartRun,
    SpectateAi,
    Launch,
    Equip(u16),
    QuitApp,
}

#[derive(Component)]
pub(crate) struct MenuButton {
    index: usize,
    action: MenuAction,
}

#[derive(Component)]
pub(crate) struct MenuBanner;

#[derive(Component)]
pub(crate) struct LoadoutHeader;

#[derive(Resource, Default)]
pub struct MenuCursor(pub usize);

#[derive(Resource)]
pub struct SplashTimer(pub Timer);

fn menu_button(index: usize, action: MenuAction, label: impl Into<String>) -> impl Bundle {
    (
        MenuButton { index, action },
        Text::new(label.into()),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(DIM),
    )
}

// --- composition -----------------------------------------------------------
/// The menu flow: every menu-like screen (splash/main-menu/loadout/lobby/results) and
/// the shared keyboard/controller navigation. Inert where there are no buttons.
pub(crate) struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuCursor>()
            .add_systems(OnEnter(GameState::Splash), menu::setup_splash)
            .add_systems(OnEnter(GameState::MainMenu), menu::setup_main_menu)
            .add_systems(OnEnter(GameState::Loadout), loadout::setup_loadout)
            .add_systems(OnEnter(GameState::Lobby), lobby::setup_lobby)
            .add_systems(OnEnter(GameState::Results), menu::setup_results)
            .add_systems(
                Update,
                (
                    // Menu navigation is shared across every menu-like screen and is
                    // inert where there are no buttons (Splash/Match).
                    menu::menu_navigate.after(InputSystems),
                    menu::menu_highlight,
                    menu::menu_activate,
                    menu::menu_escape,
                    menu::splash_advance.run_if(in_state(GameState::Splash)),
                    menu::main_menu_banner.run_if(in_state(GameState::MainMenu)),
                    loadout::loadout_header.run_if(in_state(GameState::Loadout)),
                )
                    .chain(),
            );
    }
}

/// The first-person match: lifecycle (setup/atmosphere/cursor/cleanup), the fixed-step
/// teleport controller, place rendering, audio, HUD, and the tac-map.
pub(crate) struct MatchPlugin;

impl Plugin for MatchPlugin {
    fn build(&self, app: &mut App) {
        let in_match = || in_state(GameState::Match).and(resource_exists::<MatchDirector>);
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .init_resource::<place::LastRenderedSignature>()
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
                    .run_if(in_match().and(resource_exists::<SpectatorBot>))
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
                    crate::sim::sightings::record_rival_sightings,
                    place::rebuild_place,
                    place::update_tether_monitors,
                    place::update_guardian_monitors,
                    place::interact_guardian_console,
                )
                    .chain()
                    .run_if(in_match()),
            )
            .add_systems(
                Update,
                (
                    match_runtime::ambience::apply_place_atmosphere,
                    match_runtime::ambience::sync_decohere_fx,
                    match_runtime::ambience::flicker_lights,
                    match_runtime::keystone_pickup,
                    place::sync_rival_avatars,
                    audio::sync_match_audio,
                    audio::bleed_rival_sound,
                    place::animate_doors,
                    hud::update_teleport_animation,
                    place::animate_teleport_pad_glow,
                    place::present_match_camera,
                    hud::match_draw,
                    hud::draw_tac_map,
                )
                    .chain()
                    .run_if(in_match()),
            );
    }
}
