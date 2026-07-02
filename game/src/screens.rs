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

mod audio;
mod hud;
mod input;
mod loadout;
mod lobby;
mod match_runtime;
mod menu;
mod place;

pub(crate) use audio::*;
pub(crate) use hud::*;
pub(crate) use input::*;
pub(crate) use loadout::*;
pub(crate) use lobby::*;
pub(crate) use match_runtime::*;
pub(crate) use menu::*;
pub(crate) use place::*;

// --- TEMPORARY bridges (Arc G2, stage B removes them) ------------------------
// The shared items extracted from this module are re-exported here so the submodules'
// `use super::*` and external `screens::X` paths keep resolving while consumers are
// converted to explicit imports, group by group.
pub(crate) use crate::layout::WALL_HEIGHT;
pub(crate) use crate::sim::state::*;
pub(crate) use crate::view::assets::*;
pub(crate) use crate::view::components::*;
pub(crate) use crate::view::theme::*;

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
            .add_systems(OnEnter(GameState::Splash), setup_splash)
            .add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
            .add_systems(OnEnter(GameState::Loadout), setup_loadout)
            .add_systems(OnEnter(GameState::Lobby), setup_lobby)
            .add_systems(OnEnter(GameState::Results), setup_results)
            .add_systems(
                Update,
                (
                    // Menu navigation is shared across every menu-like screen and is
                    // inert where there are no buttons (Splash/Match).
                    menu_navigate.after(InputSystems),
                    menu_highlight,
                    menu_activate,
                    menu_escape,
                    splash_advance.run_if(in_state(GameState::Splash)),
                    main_menu_banner.run_if(in_state(GameState::MainMenu)),
                    loadout_header.run_if(in_state(GameState::Loadout)),
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
        let in_match = || in_state(GameState::Match).and(resource_exists::<MatchRuntime>);
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .init_resource::<place::LastRenderedSignature>()
            .add_systems(
                OnEnter(GameState::Match),
                (
                    setup_match,
                    spawn_match_setpieces,
                    grab_match_cursor,
                    apply_match_atmosphere,
                )
                    .chain(),
            )
            .add_systems(Update, place::update_carried_torch_light.run_if(in_match()))
            .add_systems(
                OnExit(GameState::Match),
                (
                    release_match_cursor,
                    cleanup_match_resources,
                    clear_match_atmosphere,
                ),
            )
            .add_systems(
                FixedUpdate,
                drive_spectator_bot
                    .run_if(in_match().and(resource_exists::<SpectatorBot>))
                    .before(teleport_sim),
            )
            .add_systems(FixedUpdate, teleport_sim.run_if(in_match()))
            .add_systems(
                Update,
                (
                    match_input.after(InputSystems),
                    toggle_tac_map,
                    match_pump,
                    item_actions,
                    crate::guardian::update_guardian_in_match,
                    rebuild_place,
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
                    apply_place_atmosphere,
                    sync_decohere_fx,
                    flicker_lights,
                    keystone_pickup,
                    sync_rival_avatars,
                    sync_match_audio,
                    animate_doors,
                    hud::update_teleport_animation,
                    place::animate_teleport_pad_glow,
                    present_match_camera,
                    match_draw,
                    draw_tac_map,
                )
                    .chain()
                    .run_if(in_match()),
            );
    }
}
