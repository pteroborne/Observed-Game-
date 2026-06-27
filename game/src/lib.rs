mod capture;
pub mod flow;
pub mod hallway;
pub mod items;
pub mod keystones;
pub mod maze;
pub mod rivals;
mod screens;
pub mod tacmap;
pub mod teleport;

use bevy::{
    asset::AssetPlugin,
    prelude::*,
    window::{PresentMode, WindowResolution},
};

pub use flow::{Career, MatchResult};

/// The assembled game's top-level flow. Each variant is a self-contained screen
/// whose entities are state-scoped (despawned on exit), so the UX never leaks.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum GameState {
    #[default]
    Splash,
    MainMenu,
    Loadout,
    Lobby,
    Match,
    Results,
}

pub struct ObservedGamePlugin;

impl Plugin for ObservedGamePlugin {
    fn build(&self, app: &mut App) {
        // Top-level composition: register the state machine + persistent career, the
        // shared camera, and the two screen/match plugins. Each plugin owns the systems
        // for its responsibility (see `screens`); evidence capture is wired in `run`.
        app.init_state::<GameState>()
            .init_resource::<Career>()
            .add_systems(Startup, setup_camera)
            .add_plugins((screens::ScreensPlugin, screens::MatchPlugin));
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        screens::GameCam,
        Transform::from_xyz(0.0, 2.0, 0.0),
        Name::new("Observed 2 Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 6_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.85, -0.6, 0.0)),
        Name::new("Observed 2 Sun"),
    ));
    // Generous ambient so the vertical walls (which catch little directional light
    // indoors) read clearly.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.74, 0.85),
        brightness: 900.0,
        ..default()
    });
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.02, 0.03, 0.05)))
        .add_plugins(
            DefaultPlugins
                // Resolve drop-in assets against the workspace `assets/` directory
                // (Bevy otherwise reads `assets/` relative to the crate dir under
                // `cargo run`, missing files dropped at the repo root).
                .set(AssetPlugin {
                    file_path: screens::assets_dir().to_string_lossy().into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2".to_string(),
                        resolution: WindowResolution::new(1440, 900),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(ObservedGamePlugin);

    // Opt-in evidence capture (no-op in normal play).
    capture::configure(&mut app);

    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin, state::app::StatesPlugin,
    };
    use observed_match::hybrid::LocalAction;
    use screens::ScreenRoot;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        // The Match builds meshes/materials and loads drop-in textures/models, so the
        // headless test app needs those asset types registered (no render plugins).
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .init_asset::<Scene>()
        .init_asset::<AudioSource>()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(ObservedGamePlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn count_audio_cue(app: &mut App, expected: screens::MatchAudioCue) -> usize {
        let world = app.world_mut();
        let mut query = world.query::<&screens::MatchAudioCue>();
        query.iter(world).filter(|cue| **cue == expected).count()
    }

    fn single_visibility<T: Component>(app: &mut App) -> Visibility {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Visibility, With<T>>();
        *query
            .single(world)
            .expect("expected exactly one matching visibility component")
    }

    fn go(app: &mut App, state: GameState) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(state);
        app.update();
        assert_eq!(*app.world().resource::<State<GameState>>().get(), state);
    }

    fn tap_update(app: &mut App, key: KeyCode) {
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset(key);
            keys.press(key);
        }
        app.world_mut().run_schedule(Update);
    }

    /// Play the in-Match live networked match to completion (headless), then let the
    /// systems resolve it.
    fn finish_match(app: &mut App) {
        {
            let mut rt = app.world_mut().resource_mut::<screens::MatchRuntime>();
            rt.live.run_to_completion_headless(100_000);
        }
        app.update(); // match_pump records the result and requests Results
        app.update(); // OnEnter(Results) awards it
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Results
        );
    }

    #[test]
    fn boots_into_the_splash_screen() {
        let mut app = test_app();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Splash
        );
        assert_eq!(count::<ScreenRoot>(&mut app), 1);
    }

    #[test]
    fn the_keystone_gate_starts_locked_opens_on_pickup_and_is_cleaned_up() {
        use keystones::KeystoneState;
        let mut app = test_app();
        go(&mut app, GameState::Match);
        // The gate starts locked with keystones to find.
        {
            let keys = app.world().resource::<KeystoneState>();
            assert!(keys.required > 0);
            assert!(!keys.gate_open(), "the exit starts locked");
        }
        // Picking up the keystones opens the gate.
        {
            let mut keys = app.world_mut().resource_mut::<KeystoneState>();
            let rooms = keys.rooms.clone();
            for room in rooms {
                keys.collect(room);
            }
            assert!(
                keys.gate_open(),
                "collecting every keystone unlocks the exit"
            );
        }
        // Leaving the Match removes the keystone state (no leak).
        finish_match(&mut app);
        assert!(!app.world().contains_resource::<KeystoneState>());
    }

    #[test]
    fn droppable_items_start_in_inventory_and_are_cleaned_up() {
        let mut app = test_app();
        go(&mut app, GameState::Match);
        {
            let items = app.world().resource::<items::ItemsState>();
            assert_eq!(items.carried(items::ItemKind::AnchorTorch), 1);
            assert_eq!(items.carried(items::ItemKind::TeleportPad), 2);
            assert!(items.placed.is_empty());
        }

        finish_match(&mut app);

        assert!(!app.world().contains_resource::<items::ItemsState>());
        assert!(!app.world().contains_resource::<screens::ItemIntent>());
        assert_eq!(
            count::<screens::DroppedItemVisual>(&mut app),
            0,
            "dropped item visuals never leak past the Match"
        );
    }

    #[test]
    fn anchor_torch_can_be_dropped_pins_edges_and_can_be_picked_back_up() {
        let mut app = test_app();
        go(&mut app, GameState::Match);

        tap_update(&mut app, KeyCode::KeyF);

        {
            let items = app.world().resource::<items::ItemsState>();
            assert_eq!(items.carried(items::ItemKind::AnchorTorch), 0);
            assert_eq!(items.placed.len(), 1);
            assert_eq!(items.placed[0].kind, items::ItemKind::AnchorTorch);
        }
        let pins = {
            let runtime = app.world().resource::<screens::MatchRuntime>();
            let keys = app.world().resource::<keystones::KeystoneState>();
            let items = app.world().resource::<items::ItemsState>();
            screens::nav_from_brain(runtime.live.host_match(), keys, items).pins
        };
        assert!(
            !pins.is_empty(),
            "a dropped anchor torch pins the current room's incident edges"
        );
        assert_eq!(
            count::<screens::DroppedItemVisual>(&mut app),
            1,
            "dropping the torch renders a visible tool"
        );

        tap_update(&mut app, KeyCode::KeyF);

        {
            let items = app.world().resource::<items::ItemsState>();
            assert_eq!(items.carried(items::ItemKind::AnchorTorch), 1);
            assert!(items.placed.is_empty());
        }
        assert_eq!(
            count::<screens::DroppedItemVisual>(&mut app),
            0,
            "picking the torch back up removes its place visual"
        );
    }

    #[test]
    fn teleport_pads_link_the_local_body_between_placed_pads() {
        use bevy::math::Vec2;
        let mut app = test_app();
        go(&mut app, GameState::Match);
        let place = app.world().resource::<screens::TeleportState>().place;
        {
            let mut items = app.world_mut().resource_mut::<items::ItemsState>();
            assert!(items.drop(items::ItemKind::TeleportPad, place, Vec2::new(0.0, 0.0), 0));
            assert!(items.drop(items::ItemKind::TeleportPad, place, Vec2::new(2.0, 0.0), 0));
        }
        {
            let mut tp = app.world_mut().resource_mut::<screens::TeleportState>();
            let half_height = tp.config.half_height;
            tp.body.position = Vec3::new(0.0, half_height, 0.0);
            tp.rendered = None;
        }

        tap_update(&mut app, KeyCode::KeyE);

        let tp = app.world().resource::<screens::TeleportState>();
        assert_eq!(tp.place, place);
        assert!(
            (tp.body.position.x - 2.0).abs() < 0.01 && tp.body.position.z.abs() < 0.01,
            "activating a pad moves the local body to its paired pad"
        );
        assert_eq!(
            count::<screens::DroppedItemVisual>(&mut app),
            2,
            "both placed pads stay visible after using the link"
        );
    }

    #[test]
    fn rival_avatars_walk_the_room_you_share_and_are_cleaned_up() {
        use observed_match::facility::TEAM_COUNT;
        let mut app = test_app();
        go(&mut app, GameState::Match);
        app.update(); // let sync_rival_avatars reconcile the start room
        // Every team starts clumped at the entrance, so the three rivals appear there.
        assert_eq!(
            count::<screens::RivalAvatar>(&mut app),
            TEAM_COUNT - 1,
            "the rivals you share the entrance with are rendered as walking figures"
        );
        // Leaving the Match despawns them with the rest of the state-scoped entities.
        finish_match(&mut app);
        assert_eq!(
            count::<screens::RivalAvatar>(&mut app),
            0,
            "rival avatars never leak past the Match"
        );
    }

    #[test]
    fn passage_doorways_spawn_openable_door_leaves_that_are_cleaned_up() {
        let mut app = test_app();
        go(&mut app, GameState::Match);
        app.update(); // let rebuild_place build the start room's geometry
        // The current place's passage doorways each carry an openable leaf for
        // `animate_doors` to slide; sealed side doors carry non-openable leaves.
        let openable = {
            let world = app.world_mut();
            let mut q = world.query::<&screens::DoorLeaf>();
            q.iter(world).filter(|leaf| leaf.openable).count()
        };
        assert!(
            openable > 0,
            "the spine forward doorway spawns an openable door leaf"
        );
        // Leaving the Match despawns every leaf with the rest of the place geometry.
        finish_match(&mut app);
        assert_eq!(
            count::<screens::DoorLeaf>(&mut app),
            0,
            "door leaves never leak past the Match"
        );
    }

    #[test]
    fn a_room_doorway_previews_the_hallway_beyond_and_is_cleaned_up() {
        let mut app = test_app();
        go(&mut app, GameState::Match);
        app.update(); // let rebuild_place build the start room + its forward preview
        assert!(
            count::<screens::PassagePreview>(&mut app) > 0,
            "the forward doorway previews the actual hallway you'll enter"
        );
        finish_match(&mut app);
        assert_eq!(
            count::<screens::PassagePreview>(&mut app),
            0,
            "passage preview geometry never leaks past the Match"
        );
    }

    #[test]
    fn a_hallway_doorway_previews_the_room_beyond() {
        use crate::flow::MATCH_SEED;
        use crate::teleport::Place;
        use observed_core::RoomId;
        let mut app = test_app();
        go(&mut app, GameState::Match);
        app.update();
        // Drop into a spine hallway, then rebuild: its exit (and entry) should preview the
        // destination room — the symmetric case to a room previewing its hallway.
        app.world_mut()
            .resource_scope(|world, mut tp: Mut<screens::TeleportState>| {
                let runtime = world.resource::<screens::MatchRuntime>();
                let keys = world.resource::<keystones::KeystoneState>();
                let items = world.resource::<items::ItemsState>();
                let game = runtime.live.host_match();
                let from = game.local_room();
                let to = game.local_target().unwrap_or(RoomId(from.0 + 1));
                let variation = hallway::variation_for(from, to, MATCH_SEED, game.reroute_commits);
                screens::debug_place_into(
                    &mut tp,
                    runtime,
                    Place::Hallway {
                        from,
                        to,
                        variation,
                    },
                    from,
                    keys,
                    items,
                );
            });
        app.update(); // rebuild_place builds the hallway + previews the rooms at its ends
        assert!(
            count::<screens::PassagePreview>(&mut app) > 0,
            "a hallway's doorway previews the room you'll enter"
        );
    }

    #[test]
    fn tab_toggles_the_tac_map_and_draws_the_live_schematic() {
        let mut app = test_app();
        go(&mut app, GameState::Match);

        assert_eq!(
            single_visibility::<screens::TacMapPanel>(&mut app),
            Visibility::Hidden,
            "the tac-map starts hidden"
        );
        assert_eq!(
            count::<screens::TacMapElement>(&mut app),
            0,
            "hidden map has no dynamic room/marker nodes"
        );

        // Press Tab and run only Update so the just-pressed state is visible to the
        // match systems before PreUpdate clears it.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.world_mut().run_schedule(Update);

        assert!(app.world().resource::<screens::TacMapState>().0);
        assert_eq!(
            single_visibility::<screens::TacMapPanel>(&mut app),
            Visibility::Visible,
            "Tab shows the tac-map panel"
        );
        let expected_elements = (tacmap::spine().len() - 1)
            + 9
            + 1
            + keystones::REQUIRED
            + (observed_match::facility::TEAM_COUNT - 1)
            + 1;
        assert_eq!(
            count::<screens::TacMapElement>(&mut app),
            expected_elements,
            "the visible map draws route bars, nine rooms, exit, keys, rivals, and player"
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(KeyCode::Tab);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.world_mut().run_schedule(Update);

        assert!(!app.world().resource::<screens::TacMapState>().0);
        assert_eq!(
            single_visibility::<screens::TacMapPanel>(&mut app),
            Visibility::Hidden,
            "a second Tab hides the tac-map panel"
        );
        assert_eq!(
            count::<screens::TacMapElement>(&mut app),
            0,
            "hiding the map removes its dynamic nodes"
        );
    }

    #[test]
    fn tac_map_state_and_elements_are_cleaned_up_after_match() {
        let mut app = test_app();
        go(&mut app, GameState::Match);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.world_mut().run_schedule(Update);
        assert!(app.world().contains_resource::<screens::TacMapState>());
        assert!(count::<screens::TacMapElement>(&mut app) > 0);

        finish_match(&mut app);

        assert!(!app.world().contains_resource::<screens::TacMapState>());
        assert_eq!(
            count::<screens::TacMapElement>(&mut app),
            0,
            "tac-map UI never leaks past the Match state"
        );
    }

    #[test]
    fn the_full_career_loop_runs_and_grows_the_persistent_profile() {
        let mut app = test_app();
        go(&mut app, GameState::MainMenu);
        assert_eq!(count::<ScreenRoot>(&mut app), 1);

        go(&mut app, GameState::Lobby);
        assert!(app.world().contains_resource::<screens::LobbyRuntime>());

        go(&mut app, GameState::Match);
        assert!(app.world().contains_resource::<screens::MatchRuntime>());

        finish_match(&mut app);
        // The result was awarded into the persistent career.
        assert_eq!(app.world().resource::<Career>().matches_completed, 1);
        assert!(app.world().resource::<Career>().profile.xp > 0);

        go(&mut app, GameState::MainMenu);
        assert_eq!(count::<ScreenRoot>(&mut app), 1);
    }

    #[test]
    fn screens_are_state_scoped_and_never_leak_across_the_loop() {
        let mut app = test_app();
        // Enter the menu once; the loop then only ever transitions between distinct
        // states (the real flow never self-transitions), so each screen's entities
        // are despawned on exit and exactly one screen is alive at any time.
        go(&mut app, GameState::MainMenu);
        for cycle in 1..=5 {
            assert_eq!(
                count::<ScreenRoot>(&mut app),
                1,
                "exactly one screen at a time"
            );
            go(&mut app, GameState::Loadout);
            assert_eq!(count::<ScreenRoot>(&mut app), 1);
            go(&mut app, GameState::MainMenu);
            go(&mut app, GameState::Lobby);
            assert_eq!(count::<ScreenRoot>(&mut app), 1);
            go(&mut app, GameState::Match);
            assert_eq!(count::<ScreenRoot>(&mut app), 1);
            finish_match(&mut app);
            assert_eq!(count::<ScreenRoot>(&mut app), 1);
            go(&mut app, GameState::MainMenu);
            assert_eq!(count::<ScreenRoot>(&mut app), 1);
            assert_eq!(app.world().resource::<Career>().matches_completed, cycle);
        }
    }

    #[test]
    fn equipping_a_cosmetic_from_the_loadout_persists_in_the_career() {
        use observed_progression::progression::catalog;
        let mut app = test_app();
        // Win a few matches so cosmetics unlock.
        {
            let mut career = app.world_mut().resource_mut::<Career>();
            for _ in 0..4 {
                career.record(flow::play_match());
                career.award();
            }
        }
        // Pick an unlocked cosmetic and put the menu cursor on its row.
        let (index, id) = {
            let career = app.world().resource::<Career>();
            catalog()
                .iter()
                .enumerate()
                .find(|(_, c)| career.profile.is_unlocked(c.id))
                .map(|(i, c)| (i, c.id))
                .expect("at least one cosmetic is unlocked after a few wins")
        };
        go(&mut app, GameState::Loadout);
        app.world_mut().resource_mut::<screens::MenuCursor>().0 = index;
        // Press Enter and run only the Update schedule, so the input-clear in
        // PreUpdate does not wipe the press before `menu_activate` reads it.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.world_mut().run_schedule(Update);
        assert!(
            app.world().resource::<Career>().profile.is_equipped(id),
            "equipping from the loadout updates the persistent profile"
        );
    }

    #[test]
    fn the_match_renders_the_current_place_and_starts_on_the_spine() {
        let mut app = test_app();
        assert!(
            screens::PLANNED_ASSET_PATHS
                .iter()
                .all(|path| screens::assets_dir().join(path).is_file()),
            "every planned drop-in file must be present"
        );

        go(&mut app, GameState::Match);
        // The current place renders as neon-noir geometry (floor/walls/ceiling/frames).
        assert!(
            count::<screens::PlaceGeometry>(&mut app) > 0,
            "the current place is rendered"
        );
        assert_eq!(
            count_audio_cue(&mut app, screens::MatchAudioCue::Ambience),
            1,
            "the facility ambience starts with the Match"
        );

        // The player begins in the local team's spine room.
        let start_room = {
            let rt = app.world().resource::<screens::MatchRuntime>();
            rt.live.host_match().local_room()
        };
        assert_eq!(
            app.world().resource::<screens::TeleportState>().place,
            teleport::Place::Room(start_room),
            "the teleport state starts in the local team's room"
        );
    }

    #[test]
    fn driving_spine_rounds_advances_the_brain_with_the_place_renderer_live() {
        let mut app = test_app();
        go(&mut app, GameState::Match);
        let before = {
            let rt = app.world().resource::<screens::MatchRuntime>();
            rt.live.host_match().competitive.round
        };
        // Commit a few spine rounds (as the player's crossings would); the place
        // geometry stays live across the round/teleport churn.
        {
            let mut rt = app.world_mut().resource_mut::<screens::MatchRuntime>();
            for _ in 0..4 {
                if rt.live.finished() {
                    break;
                }
                let action = if rt.live.local_active() {
                    LocalAction::Advance
                } else {
                    LocalAction::Wait
                };
                rt.live.force_round(action);
            }
        }
        app.update();
        let after = {
            let rt = app.world().resource::<screens::MatchRuntime>();
            rt.live.host_match().competitive.round
        };
        assert!(after > before, "spine rounds advanced the match brain");
        assert!(
            count::<screens::PlaceGeometry>(&mut app) > 0,
            "the place renderer keeps geometry live across rounds"
        );
    }
}
