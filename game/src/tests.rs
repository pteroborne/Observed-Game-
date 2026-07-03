use super::*;
use crate::flow::MATCH_SEED;
use crate::view::theme::ScreenRoot;
use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin, state::app::StatesPlugin};
use observed_match::hybrid::LocalAction;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        AssetPlugin {
            file_path: crate::view::assets::assets_dir()
                .to_string_lossy()
                .into_owned(),
            ..default()
        },
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

fn count_audio_cue(app: &mut App, expected: crate::view::components::MatchAudioCue) -> usize {
    let world = app.world_mut();
    let mut query = world.query::<&crate::view::components::MatchAudioCue>();
    query.iter(world).filter(|cue| **cue == expected).count()
}

fn material_named_has_texture(app: &mut App, expected_name: &str) -> bool {
    let handles: Vec<Handle<StandardMaterial>> = {
        let world = app.world_mut();
        let mut query = world.query::<(&Name, &MeshMaterial3d<StandardMaterial>)>();
        query
            .iter(world)
            .filter(|(name, _)| name.as_str() == expected_name)
            .map(|(_, handle)| handle.0.clone())
            .collect()
    };
    let materials = app.world().resource::<Assets<StandardMaterial>>();
    handles.iter().any(|handle| {
        materials
            .get(handle)
            .is_some_and(|material| material.base_color_texture.is_some())
    })
}

fn single_visibility<T: Component>(app: &mut App) -> Visibility {
    let world = app.world_mut();
    let mut query = world.query_filtered::<&Visibility, With<T>>();
    *query
        .single(world)
        .expect("expected exactly one matching visibility component")
}

fn menu_sun_illuminance(app: &mut App) -> f32 {
    let world = app.world_mut();
    let mut query =
        world.query_filtered::<&DirectionalLight, With<crate::view::components::GameSun>>();
    query
        .single(world)
        .expect("expected exactly one startup sun")
        .illuminance
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
        let mut rt = app
            .world_mut()
            .resource_mut::<crate::sim::director::MatchDirector>();
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
    assert!(
        !app.world()
            .contains_resource::<crate::sim::state::ItemIntent>()
    );
    assert_eq!(
        count::<crate::view::components::DroppedItemVisual>(&mut app),
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
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let keys = app.world().resource::<keystones::KeystoneState>();
        let items = app.world().resource::<items::ItemsState>();
        crate::sim::nav::nav_from_brain(MATCH_SEED, runtime.live.host_match(), keys, items).pins
    };
    assert!(
        !pins.is_empty(),
        "a dropped anchor torch pins the current room's incident edges"
    );
    assert_eq!(
        count::<crate::view::components::DroppedItemVisual>(&mut app),
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
        count::<crate::view::components::DroppedItemVisual>(&mut app),
        0,
        "picking the torch back up removes its place visual"
    );
}

#[test]
fn anchor_torch_tethers_current_thresholds_immediately() {
    use observed_style::{self as style, MarkerRole};

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // build the initial room.

    let (room, target, visible_targets) = {
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let tp = app.world().resource::<crate::sim::state::TeleportState>();
        let game = runtime.live.host_match();
        let mut visible_targets: Vec<_> = tp.geom.gaps.iter().map(|gap| gap.target).collect();
        visible_targets.sort_by_key(|room| room.0);
        visible_targets.dedup();
        (
            game.local_room(),
            game.local_target().expect("spine target"),
            visible_targets,
        )
    };

    tap_update(&mut app, KeyCode::KeyF);
    app.update(); // rebuild place geometry/lights after the anchor changes nav.

    {
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let keys = app.world().resource::<keystones::KeystoneState>();
        let items = app.world().resource::<items::ItemsState>();
        let nav =
            crate::sim::nav::nav_from_brain(MATCH_SEED, runtime.live.host_match(), keys, items);
        let mut pinned_targets: Vec<_> = items.placed[0]
            .pin_edges
            .iter()
            .filter_map(|&(a, b)| (a == room).then_some(b))
            .collect();
        pinned_targets.sort_by_key(|room| room.0);
        pinned_targets.dedup();
        assert_eq!(
            pinned_targets, visible_targets,
            "dropping the anchor freezes every threshold visible in the room"
        );
        assert_eq!(
            nav.connections, visible_targets,
            "a tethered room reads its frozen visible threshold table exactly"
        );
        assert!(
            nav.is_tethered(room, target),
            "dropping the anchor tethers the room's visible threshold relation"
        );
        assert!(
            nav.connections.contains(&target),
            "the tethered target remains present in the room nav"
        );
    }

    let control = style::marker(MarkerRole::Control).base_color.to_srgba();
    let has_control_light = {
        let world = app.world_mut();
        let mut q = world.query::<(&PointLight, &Name)>();
        q.iter(world).any(|(light, name)| {
            let c = light.color.to_srgba();
            name.as_str() == "Doorframe tether light"
                && (c.red - control.red).abs() < 0.01
                && (c.green - control.green).abs() < 0.01
                && (c.blue - control.blue).abs() < 0.01
        })
    };
    assert!(
        has_control_light,
        "a tethered threshold gets the anchor/control-coloured frame light"
    );
}

#[test]
fn nav_keeps_a_tethered_relation_even_if_the_live_graph_drops_it() {
    use crate::flow::MATCH_SEED;
    use crate::teleport::Place;
    use observed_core::RoomId;
    use observed_match::hybrid::HybridMatch;

    let game = HybridMatch::authored(MATCH_SEED);
    let room = game.local_room();
    let live = crate::sim::nav::connections_for(&game, room);
    let absent = (0..9)
        .map(RoomId)
        .find(|candidate| *candidate != room && !live.contains(candidate))
        .expect("the authored graph has at least one non-neighbour");

    let mut items = items::ItemsState::single_player();
    assert!(items.drop_anchor_torch(
        Place::Room(room),
        Vec2::ZERO,
        game.reroute_commits,
        &[absent],
    ));
    let keys = keystones::KeystoneState::new(MATCH_SEED);
    let nav = crate::sim::nav::nav_for_room(MATCH_SEED, &game, &keys, &items, room);

    assert!(
        nav.connections.contains(&absent),
        "a tethered relation is added back to room nav even when absent from the live graph"
    );
    assert!(
        nav.is_tethered(room, absent),
        "the added relation is marked tethered for frame lights and hallway variation"
    );
}

#[test]
fn room_anchor_locks_exact_threshold_set_and_rejects_new_live_edges() {
    use crate::flow::MATCH_SEED;
    use crate::teleport::Place;
    use observed_core::RoomId;
    use observed_match::hybrid::HybridMatch;

    let mut game = HybridMatch::authored(MATCH_SEED);
    let room = game.local_room();
    let locked = crate::sim::nav::connections_for(&game, room);
    assert!(
        !locked.is_empty(),
        "the authored start room has thresholds to freeze"
    );
    let absent = (0..9)
        .map(RoomId)
        .find(|candidate| *candidate != room && !locked.contains(candidate))
        .expect("the authored graph has at least one non-neighbour");

    let mut items = items::ItemsState::single_player();
    assert!(items.drop_anchor_torch(Place::Room(room), Vec2::ZERO, game.reroute_commits, &locked,));

    let mut injected = game
        .rendered
        .first()
        .cloned()
        .expect("authored match has at least one route");
    injected.rooms = (room, absent);
    game.rendered
        .retain(|route| route.rooms.0 != room && route.rooms.1 != room);
    game.rendered.push(injected);

    let keys = keystones::KeystoneState::new(MATCH_SEED);
    let nav = crate::sim::nav::nav_for_room(MATCH_SEED, &game, &keys, &items, room);

    assert_eq!(
        nav.connections, locked,
        "a room anchor uses the stored threshold table, not live graph drift"
    );
    assert!(
        !nav.connections.contains(&absent),
        "new live relations cannot appear as thresholds while the room is anchored"
    );

    let other_nav = crate::sim::nav::nav_for_room(MATCH_SEED, &game, &keys, &items, absent);
    assert!(
        !other_nav.connections.contains(&room),
        "an unanchored room cannot grow a new inbound threshold into a locked room"
    );
}

#[test]
fn teleport_pads_link_the_local_body_between_placed_pads() {
    use bevy::math::Vec2;
    let mut app = test_app();
    go(&mut app, GameState::Match);
    let place = app
        .world()
        .resource::<crate::sim::state::TeleportState>()
        .place;
    {
        let mut items = app.world_mut().resource_mut::<items::ItemsState>();
        assert!(items.drop(items::ItemKind::TeleportPad, place, Vec2::new(0.0, 0.0), 0));
        assert!(items.drop(items::ItemKind::TeleportPad, place, Vec2::new(2.0, 0.0), 0));
    }
    {
        let mut tp = app
            .world_mut()
            .resource_mut::<crate::sim::state::TeleportState>();
        let half_height = tp.config.half_height;
        tp.body.position = Vec3::new(0.0, half_height, 0.0);
        tp.rendered = None;
    }

    tap_update(&mut app, KeyCode::KeyE);

    let tp = app.world().resource::<crate::sim::state::TeleportState>();
    assert_eq!(tp.place, place);
    assert!(
        (tp.body.position.x - 2.0).abs() < 0.01 && tp.body.position.z.abs() < 0.01,
        "activating a pad moves the local body to its paired pad"
    );
    assert_eq!(
        count::<crate::view::components::DroppedItemVisual>(&mut app),
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
        count::<crate::view::components::RivalAvatar>(&mut app),
        TEAM_COUNT - 1,
        "the rivals you share the entrance with are rendered as walking figures"
    );
    // Leaving the Match despawns them with the rest of the state-scoped entities.
    finish_match(&mut app);
    assert_eq!(
        count::<crate::view::components::RivalAvatar>(&mut app),
        0,
        "rival avatars never leak past the Match"
    );
}

#[test]
fn passage_thresholds_are_open_and_any_leaves_are_sealed_and_cleaned_up() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // let rebuild_place build the start room's geometry
    // Passage thresholds are always-open (transparent): no hiding leaf. Any door leaf
    // present is a sealed side door / the locked exit — never an openable one.
    let openable = {
        let world = app.world_mut();
        let mut q = world.query::<&crate::view::components::DoorLeaf>();
        q.iter(world).filter(|leaf| leaf.openable).count()
    };
    assert_eq!(openable, 0, "passage thresholds carry no hiding leaf");
    // The forward doorway still reveals the place beyond (a transparent preview).
    assert!(
        count::<crate::view::components::PassagePreview>(&mut app) > 0,
        "an open threshold shows the neighbour you'll cross into"
    );
    // Leaving the Match despawns every leaf with the rest of the place geometry.
    finish_match(&mut app);
    assert_eq!(
        count::<crate::view::components::DoorLeaf>(&mut app),
        0,
        "door leaves never leak past the Match"
    );
}

#[test]
fn doorway_destinations_are_frozen_at_place_entry() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    let tp = app.world().resource::<crate::sim::state::TeleportState>();
    assert!(
        !tp.gap_dests.is_empty(),
        "the start room freezes its doorway destinations on entry"
    );
    // The forward doorway's frozen destination is the hallway you'll teleport into, so
    // the preview and the crossing read the identical snapshot.
    let forward = tp.geom.forward_gap().expect("a forward doorway");
    let frozen = tp
        .gap_dests
        .iter()
        .find(|d| (d.gap_center - forward.center).length() < 0.05)
        .expect("the forward doorway has a frozen destination");
    assert!(
        matches!(frozen.place, crate::teleport::Place::Hallway { .. }),
        "the forward doorway leads into a hallway"
    );
}

#[test]
fn a_room_doorway_previews_the_hallway_beyond_and_is_cleaned_up() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // let rebuild_place build the start room + its forward preview
    assert!(
        count::<crate::view::components::PassagePreview>(&mut app) > 0,
        "the forward doorway previews the actual hallway you'll enter"
    );
    finish_match(&mut app);
    assert_eq!(
        count::<crate::view::components::PassagePreview>(&mut app),
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
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let runtime = world.resource::<crate::sim::director::MatchDirector>();
            let keys = world.resource::<keystones::KeystoneState>();
            let items = world.resource::<items::ItemsState>();
            let game = runtime.live.host_match();
            let from = game.local_room();
            let to = game.local_target().unwrap_or(RoomId(from.0 + 1));
            let variation = hallway::variation_for(from, to, MATCH_SEED, game.reroute_commits);
            crate::screens::match_runtime::debug_place_into(
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
        count::<crate::view::components::PassagePreview>(&mut app) > 0,
        "a hallway's doorway previews the room you'll enter"
    );
}

#[test]
fn hallway_room_threshold_does_not_draw_wall_trim_across_opening() {
    use crate::flow::MATCH_SEED;
    use crate::teleport::{GapKind, Place};
    use observed_core::RoomId;

    fn trim_crosses_gap(transform: &Transform, gap: &teleport::DoorGap) -> bool {
        let center = Vec2::new(transform.translation.x, transform.translation.z);
        let normal_dist = (center - gap.center).dot(gap.normal).abs();
        if normal_dist > 0.35 {
            return false;
        }

        let tangent = Vec2::new(-gap.normal.y, gap.normal.x);
        let x_axis = transform.rotation * Vec3::X;
        let z_axis = transform.rotation * Vec3::Z;
        let (axis, half_len) = if transform.scale.x >= transform.scale.z {
            (
                Vec2::new(x_axis.x, x_axis.z).normalize_or_zero(),
                transform.scale.x * 0.5,
            )
        } else {
            (
                Vec2::new(z_axis.x, z_axis.z).normalize_or_zero(),
                transform.scale.z * 0.5,
            )
        };
        if axis.dot(tangent).abs() < 0.92 {
            return false;
        }

        let center_along = (center - gap.center).dot(tangent);
        let lo = center_along - half_len;
        let hi = center_along + half_len;
        lo < gap.width * 0.5 - 0.02 && hi > -gap.width * 0.5 + 0.02
    }

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    app.world_mut()
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let runtime = world.resource::<crate::sim::director::MatchDirector>();
            let keys = world.resource::<keystones::KeystoneState>();
            let items = world.resource::<items::ItemsState>();
            let game = runtime.live.host_match();
            let from = game.local_room();
            let to = game.local_target().unwrap_or(RoomId(from.0 + 1));
            let variation = hallway::variation_for(from, to, MATCH_SEED, game.reroute_commits);
            crate::screens::match_runtime::debug_place_into(
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
    app.update();

    let open_gaps: Vec<_> = app
        .world()
        .resource::<crate::sim::state::TeleportState>()
        .geom
        .gaps
        .iter()
        .copied()
        .filter(|gap| matches!(gap.kind, GapKind::Entry | GapKind::Exit))
        .collect();
    let blocking_trims: Vec<_> = {
        let world = app.world_mut();
        let mut trims = world.query::<(&Name, &Transform)>();
        trims
            .iter(world)
            .filter(|(name, transform)| {
                name.as_str() == "Wall trim"
                    && open_gaps.iter().any(|gap| trim_crosses_gap(transform, gap))
            })
            .map(|(_, transform)| (transform.translation, transform.scale))
            .collect()
    };

    assert_eq!(
        blocking_trims.len(),
        0,
        "wall trim must be split around open hallway-room thresholds: {blocking_trims:?}"
    );
}

#[test]
fn tab_toggles_the_tac_map_and_draws_the_live_schematic() {
    let mut app = test_app();
    go(&mut app, GameState::Match);

    assert_eq!(
        single_visibility::<crate::view::components::TacMapPanel>(&mut app),
        Visibility::Hidden,
        "the tac-map starts hidden"
    );
    assert_eq!(
        count::<crate::view::components::TacMapElement>(&mut app),
        0,
        "hidden map has no dynamic room/marker nodes"
    );

    // Press Tab and run only Update so the just-pressed state is visible to the
    // match systems before PreUpdate clears it.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Tab);
    app.world_mut().run_schedule(Update);

    assert!(
        app.world()
            .resource::<crate::view::components::TacMapState>()
            .0
    );
    assert_eq!(
        single_visibility::<crate::view::components::TacMapPanel>(&mut app),
        Visibility::Visible,
        "Tab shows the tac-map panel"
    );
    let expected_model = {
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let keys = app.world().resource::<keystones::KeystoneState>();
        let sightings = app.world().resource::<crate::sim::state::RivalSightings>();
        let tp = app.world().resource::<crate::sim::state::TeleportState>();
        let game = runtime.live.host_match();
        tacmap::build_map(
            &game.competitive,
            keys,
            sightings,
            game.reroute_commits,
            tp.place,
        )
    };
    assert!(
        !expected_model.rivals.is_empty(),
        "the rivals sharing the entrance at match start should already be witnessed"
    );
    // Each rival pip also draws a team-label text node (Phase 42c), so the dynamic
    // element count carries one extra tagged node per pip beyond the box/outline itself.
    let expected_elements = expected_model.routes.len()
        + expected_model.rooms.len()
        + 1
        + expected_model.keystones.len()
        + expected_model.rivals.len() * 2
        + 1
        + 1;
    assert_eq!(
        count::<crate::view::components::TacMapElement>(&mut app),
        expected_elements,
        "the visible map draws route bars, rooms, exit, keys, rivals, player, and series status"
    );

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset(KeyCode::Tab);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Tab);
    app.world_mut().run_schedule(Update);

    assert!(
        !app.world()
            .resource::<crate::view::components::TacMapState>()
            .0
    );
    assert_eq!(
        single_visibility::<crate::view::components::TacMapPanel>(&mut app),
        Visibility::Hidden,
        "a second Tab hides the tac-map panel"
    );
    assert_eq!(
        count::<crate::view::components::TacMapElement>(&mut app),
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
    assert!(
        app.world()
            .contains_resource::<crate::view::components::TacMapState>()
    );
    assert!(count::<crate::view::components::TacMapElement>(&mut app) > 0);

    finish_match(&mut app);

    assert!(
        !app.world()
            .contains_resource::<crate::view::components::TacMapState>()
    );
    assert_eq!(
        count::<crate::view::components::TacMapElement>(&mut app),
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
    assert!(
        app.world()
            .contains_resource::<crate::sim::state::LobbyRuntime>()
    );

    go(&mut app, GameState::Match);
    assert!(
        app.world()
            .contains_resource::<crate::sim::director::MatchDirector>()
    );

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
fn spectate_ai_menu_option_launches_a_bot_driven_match() {
    let mut app = test_app();
    go(&mut app, GameState::MainMenu);
    app.world_mut().resource_mut::<screens::MenuCursor>().0 = 1;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.world_mut().run_schedule(Update);
    app.update();

    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Match
    );
    assert!(
        app.world()
            .contains_resource::<crate::sim::state::SpectatorBot>()
    );
    assert_eq!(
        app.world().resource::<flow::ActiveMatchSeed>().0,
        flow::MATCH_SEED,
        "spectator mode uses the fixed demo seed so bot crossings stay deterministic"
    );
    {
        let bot = app.world().resource::<crate::sim::state::SpectatorBot>();
        assert_eq!(bot.seed, flow::MATCH_SEED);
        assert!(
            bot.teamplay.plan.solvable(),
            "spectator mode owns a valid procedural co-op seed plan"
        );
        assert_eq!(
            bot.teamplay
                .team(flow::LOCAL_TEAM)
                .expect("local bot team exists")
                .members
                .len(),
            observed_match::facility::MEMBERS_PER_TEAM,
            "spectator mode drives a two-member team, not a single abstract runner"
        );
    }

    let before_teamplay_tick = app
        .world()
        .resource::<crate::sim::state::SpectatorBot>()
        .teamplay
        .tick;
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(
        app.world()
            .resource::<crate::sim::state::SpectatorBot>()
            .teamplay
            .tick,
        before_teamplay_tick,
        "the procedural co-op brain should not fast-forward on a single fixed frame"
    );
    for _ in 1..crate::screens::match_runtime::spectator::SPECTATOR_TEAMPLAY_STEP_FRAMES {
        app.world_mut().run_schedule(FixedUpdate);
    }
    assert!(
        app.world()
            .resource::<crate::sim::state::SpectatorBot>()
            .teamplay
            .tick
            > before_teamplay_tick,
        "the spectator bot should advance the procedural co-op brain"
    );
    assert!(
        app.world()
            .resource::<crate::sim::state::MatchIntent>()
            .0
            .movement
            .y
            > 0.0
            || app
                .world()
                .resource::<crate::sim::state::SpectatorBot>()
                .route_place
                .is_some(),
        "the spectator bot should take over the player body"
    );
}

#[test]
fn spectate_mode_waits_for_the_visible_run_when_the_series_result_is_ready() {
    let mut app = test_app();
    app.world_mut()
        .insert_resource(flow::ActiveMatchSeed(flow::MATCH_SEED));
    app.world_mut()
        .insert_resource(crate::sim::state::SpectatorBot::for_seed(flow::MATCH_SEED));
    go(&mut app, GameState::Match);

    {
        let mut director = app
            .world_mut()
            .resource_mut::<crate::sim::director::MatchDirector>();
        assert!(
            !director.live.finished(),
            "the visible first-person run starts unfinished"
        );
        director
            .series
            .run_to_winner(observed_match::elimination::MAX_AUTOPLAY_TICKS);
        assert!(
            director.series.finished(),
            "the regression forces a ready series result before the rendered run finishes"
        );
    }

    app.world_mut()
        .resource_mut::<crate::sim::state::SpectatorBot>()
        .teamplay_frame_accum =
        crate::screens::match_runtime::spectator::SPECTATOR_TEAMPLAY_STEP_FRAMES - 1;
    app.world_mut().run_schedule(FixedUpdate);
    assert!(
        !app.world()
            .resource::<crate::sim::state::SpectatorBot>()
            .finished,
        "a ready series result must not mark the visible spectator run as finished"
    );

    app.update();
    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Match,
        "Spectate must not jump to Results before the rendered run catches up"
    );
    assert!(
        app.world().resource::<Career>().last_result.is_none(),
        "no result should be recorded while the visible run is still in progress"
    );
    assert!(
        !app.world()
            .resource::<crate::sim::director::MatchDirector>()
            .done
    );
}

#[test]
fn paused_spectate_mode_does_not_advance_the_teamplay_brain() {
    let mut app = test_app();
    app.world_mut()
        .insert_resource(flow::ActiveMatchSeed(flow::MATCH_SEED));
    app.world_mut()
        .insert_resource(crate::sim::state::SpectatorBot::for_seed(flow::MATCH_SEED));
    go(&mut app, GameState::Match);
    app.world_mut()
        .resource_mut::<crate::sim::state::MatchPaused>()
        .0 = true;

    let before = app
        .world()
        .resource::<crate::sim::state::SpectatorBot>()
        .teamplay
        .tick;
    for _ in 0..crate::screens::match_runtime::spectator::SPECTATOR_TEAMPLAY_STEP_FRAMES {
        app.world_mut().run_schedule(FixedUpdate);
    }

    assert_eq!(
        app.world()
            .resource::<crate::sim::state::SpectatorBot>()
            .teamplay
            .tick,
        before,
        "paused Spectate mode should not let simulation progress outrun the frozen view"
    );
}

#[test]
fn the_match_renders_the_current_place_and_starts_on_the_spine() {
    let mut app = test_app();
    assert!(
        crate::view::assets::PLANNED_ASSET_PATHS
            .iter()
            .all(|path| crate::view::assets::assets_dir().join(path).is_file()),
        "every planned drop-in file must be present"
    );

    go(&mut app, GameState::Match);
    // The current place renders as neon-noir geometry (floor/walls/ceiling/frames).
    assert!(
        count::<crate::view::components::PlaceGeometry>(&mut app) > 0,
        "the current place is rendered"
    );
    assert!(
        material_named_has_texture(&mut app, "Place floor"),
        "rendered floor material should use the floor texture slot"
    );
    assert!(
        material_named_has_texture(&mut app, "Room wall"),
        "rendered wall material should use the wall texture slot"
    );
    assert!(
        material_named_has_texture(&mut app, "Place ceiling"),
        "rendered ceiling material should use the ceiling texture slot"
    );
    assert_eq!(
        count_audio_cue(&mut app, crate::view::components::MatchAudioCue::Ambience),
        1,
        "the facility ambience starts with the Match"
    );

    // The player begins in the local team's spine room.
    let start_room = {
        let rt = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        rt.live.host_match().local_room()
    };
    assert_eq!(
        app.world()
            .resource::<crate::sim::state::TeleportState>()
            .place,
        teleport::Place::Room(start_room),
        "the teleport state starts in the local team's room"
    );
}

#[test]
fn match_atmosphere_disables_the_startup_sun_and_restores_it_on_exit() {
    let mut app = test_app();
    assert_eq!(
        menu_sun_illuminance(&mut app),
        crate::view::components::MENU_SUN_ILLUMINANCE
    );

    go(&mut app, GameState::Match);
    assert_eq!(
        menu_sun_illuminance(&mut app),
        0.0,
        "the match relies on place-local lights, not the menu sun"
    );

    finish_match(&mut app);
    assert_eq!(
        menu_sun_illuminance(&mut app),
        crate::view::components::MENU_SUN_ILLUMINANCE,
        "leaving the match restores the menu sun"
    );
}

#[test]
fn driving_spine_rounds_advances_the_brain_with_the_place_renderer_live() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    let before = {
        let rt = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        rt.live.host_match().competitive.round
    };
    // Commit a few spine rounds (as the player's crossings would); the place
    // geometry stays live across the round/teleport churn.
    {
        let mut rt = app
            .world_mut()
            .resource_mut::<crate::sim::director::MatchDirector>();
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
        let rt = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        rt.live.host_match().competitive.round
    };
    assert!(after > before, "spine rounds advanced the match brain");
    assert!(
        count::<crate::view::components::PlaceGeometry>(&mut app) > 0,
        "the place renderer keeps geometry live across rounds"
    );
}

/// Arc G3 characterization: the match's outcome authority. Today the interactive
/// match ends when either the live hybrid match or the elimination series finishes,
/// fast-forwards the series if the live match won the race, and resolves the result
/// from the series. The headless career match (`flow::play_match`) must produce the
/// exact same result for the same seed — the HUD, the Results screen, and the career
/// all describe one match, not three.
#[test]
fn headless_and_interactive_matches_agree_on_the_result() {
    let headless = crate::flow::play_match();

    let mut app = test_app();
    go(&mut app, GameState::Match);
    finish_match(&mut app);
    let interactive = app
        .world()
        .resource::<Career>()
        .last_result
        .clone()
        .expect("finishing the interactive match records a result");

    assert_eq!(
        interactive, headless,
        "the interactive match and the headless career run must resolve identically"
    );
    assert!(
        interactive.placement.is_some(),
        "the elimination series always places the local team"
    );
}

/// Arc G4: the Match session's resources are enumerated once
/// (`for_each_match_resource` in `screens::match_runtime::session`) and every one of
/// them must be gone after the Match exits. Before this consolidation, `Guardian`,
/// `ActionLog`, `TeleportAnimation`, and `LastTeleportPad` leaked across matches.
#[test]
fn every_match_resource_is_removed_when_the_match_ends() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    finish_match(&mut app);
    macro_rules! assert_removed {
        ($ty:ty) => {
            assert!(
                !app.world().contains_resource::<$ty>(),
                concat!(stringify!($ty), " leaked past the Match")
            );
        };
    }
    crate::screens::match_runtime::session::for_each_match_resource!(assert_removed);
}

/// Phase 42c (a): the sighting ledger's writer (`sim::sightings::record_rival_sightings`)
/// turns a rival clump moving into a witnessed neighbour into a recorded `Seen` sighting
/// — the tac-map's fog-of-war input, not the live facility position.
#[test]
fn placing_a_rival_in_a_neighbour_and_rebuilding_records_a_seen_sighting() {
    use crate::sim::director::MatchDirector;
    use crate::sim::state::{RivalSightings, SightingKind};
    use observed_core::TeamId;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // build the start room

    let neighbour = {
        let rt = app.world().resource::<MatchDirector>();
        let game = rt.live.host_match();
        let conns = crate::sim::nav::connections_for(game, game.local_room());
        *conns.first().expect("the start room has a neighbour")
    };
    {
        let mut rt = app.world_mut().resource_mut::<MatchDirector>();
        let base = rt.live.host.match_state.competitive.teams[1].member_base;
        rt.live.host.match_state.competitive.structure.graph.players[base] = neighbour;
    }
    app.update();

    let sightings = app.world().resource::<RivalSightings>();
    let sighting = sightings.get(TeamId(1), neighbour);
    assert!(
        sighting.is_some_and(|s| s.kind == SightingKind::Seen),
        "a rival standing in a witnessed neighbour must be recorded as Seen"
    );
}

/// Phase 38 (contested observation): a rival team standing in the room beyond a
/// threshold pins that connection for everyone — and the doorframe light takes the
/// rival team's colour so the player can read whose grip holds the door.
#[test]
fn a_rival_in_the_neighbouring_room_tints_the_threshold_light_with_its_colour() {
    use crate::sim::director::MatchDirector;
    use crate::sim::state::TeleportState;
    use crate::view::theme::TEAM_COLORS;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // build the start room

    // Move rival team 1's clump into the first room visible through a doorway.
    let neighbour = {
        let rt = app.world().resource::<MatchDirector>();
        let game = rt.live.host_match();
        let conns = crate::sim::nav::connections_for(game, game.local_room());
        *conns.first().expect("the start room has a neighbour")
    };
    {
        let mut rt = app.world_mut().resource_mut::<MatchDirector>();
        let base = rt.live.host.match_state.competitive.teams[1].member_base;
        rt.live.host.match_state.competitive.structure.graph.players[base] = neighbour;
    }
    // Force a rebuild of the place presentation.
    app.world_mut().resource_mut::<TeleportState>().rendered = None;
    app.update();

    let rival = TEAM_COLORS[1].to_srgba();
    let found = {
        let world = app.world_mut();
        let mut lights = world.query::<(&PointLight, &Name)>();
        lights.iter(world).any(|(light, name)| {
            let c = light.color.to_srgba();
            name.as_str() == "Doorframe tether light"
                && (c.red - rival.red).abs() < 0.01
                && (c.green - rival.green).abs() < 0.01
                && (c.blue - rival.blue).abs() < 0.01
        })
    };
    assert!(
        found,
        "a threshold into a rival-occupied room carries that team's frame light"
    );
}

/// Phase 42c (c): a rival team's presence appearing in a neighbouring room plays
/// exactly one `RivalBleed` cue (not one per frame while it stays there), and it is
/// distinguishable from the player's own `Footstep` cue.
#[test]
fn a_rival_appearing_in_a_neighbour_bleeds_exactly_one_sound_cue() {
    use crate::sim::director::MatchDirector;
    use crate::sim::state::{RivalSightings, SightingKind};
    use crate::view::components::MatchAudioCue;
    use observed_core::TeamId;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // build the start room

    let neighbour = {
        let rt = app.world().resource::<MatchDirector>();
        let game = rt.live.host_match();
        let conns = crate::sim::nav::connections_for(game, game.local_room());
        *conns.first().expect("the start room has a neighbour")
    };
    {
        let mut rt = app.world_mut().resource_mut::<MatchDirector>();
        let base = rt.live.host.match_state.competitive.teams[1].member_base;
        rt.live.host.match_state.competitive.structure.graph.players[base] = neighbour;
    }
    // Several frames with the rival unmoved: still exactly one bleed cue total.
    app.update();
    app.update();
    app.update();

    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::RivalBleed),
        1,
        "a rival holding steady in a witnessed neighbour bleeds sound exactly once"
    );
    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::Footstep),
        0,
        "a stationary player takes no footsteps of their own here"
    );

    // The neighbour is *also* a `rival_signals` presence hit, so the witnessing writer
    // (`record_rival_sightings`) records `Seen` for it in the same frame the bleed
    // system records `Heard` — and `Seen` outranks `Heard`, so the ledger correctly
    // keeps the higher-information kind. This still proves the bleed system only ever
    // *attempts* a `Heard` write (never a `Seen`/`AnchorSpotted` one): see the sibling
    // sighting-ledger unit tests for `Heard` sticking when nothing outranks it.
    let sightings = app.world().resource::<RivalSightings>();
    assert_eq!(
        sightings.get(TeamId(1), neighbour).map(|s| s.kind),
        Some(SightingKind::Seen),
        "a witnessed presence outranks the bleed system's Heard write for the same room"
    );
}

/// Phase 42c (d): a doorway preview into a room carrying a rival anchor also renders
/// that rival's anchor torch prop — "preview == arrival honesty" — not just once you
/// physically cross into the room.
#[test]
fn a_doorway_preview_into_an_anchored_room_renders_the_rival_anchor_torch() {
    use crate::sim::director::MatchDirector;
    use crate::sim::state::TeleportState;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // build the start room

    let neighbour = {
        let rt = app.world().resource::<MatchDirector>();
        let game = rt.live.host_match();
        let conns = crate::sim::nav::connections_for(game, game.local_room());
        *conns.first().expect("the start room has a neighbour")
    };
    {
        let mut rt = app.world_mut().resource_mut::<MatchDirector>();
        rt.live
            .host
            .match_state
            .competitive
            .place_team_anchor(observed_core::TeamId(1), neighbour);
    }
    app.world_mut().resource_mut::<TeleportState>().rendered = None;
    app.update();

    let found = count::<crate::view::components::PassagePreview>(&mut app) > 0 && {
        let world = app.world_mut();
        let mut query = world.query::<&Name>();
        query
            .iter(world)
            .any(|name| name.as_str() == "Rival anchor torch")
    };
    assert!(
        found,
        "a previewed room carrying a rival anchor must render its anchor torch prop \
         ahead of arrival"
    );
}

/// Phase 41 (pressure made flesh): collapse state drives presentation palettes.
#[test]
fn collapse_state_drains_place_palette_and_countdown_triggers_klaxon() {
    use observed_core::{RoomId, TeamId};
    use observed_facility::map_spec::sector_relay_v1;
    use observed_match::elimination::EscapeCountdown;
    use observed_match::hybrid::HybridMatch;

    let mut game = HybridMatch::authored(MATCH_SEED);
    game.competitive.purge_line = 0.0;
    let dying_room = game
        .competitive
        .dying_room()
        .expect("a room ahead of the collapse is dying");

    let intact = crate::screens::match_runtime::palette_for_game(
        MATCH_SEED,
        teleport::Place::Room(RoomId(8)),
        &game,
        false,
    );
    let dying = crate::screens::match_runtime::palette_for_game(
        MATCH_SEED,
        teleport::Place::Room(dying_room),
        &game,
        false,
    );
    assert!(
        dying.ambient_brightness < intact.ambient_brightness,
        "a dying district should drain before it closes"
    );

    let mut director = crate::sim::director::MatchDirector::new(MATCH_SEED, sector_relay_v1());
    let normal = crate::screens::match_runtime::palette_for_match(
        MATCH_SEED,
        teleport::Place::Room(RoomId(0)),
        &director,
    );
    director.series.current.countdown = Some(EscapeCountdown {
        started_by: TeamId(0),
        duration_rounds: 4,
        remaining_rounds: 3,
    });
    assert!(crate::screens::match_runtime::countdown_klaxon_active(
        &director
    ));
    let klaxon = crate::screens::match_runtime::palette_for_match(
        MATCH_SEED,
        teleport::Place::Room(RoomId(0)),
        &director,
    );
    let klaxon_light = klaxon.light_color.to_srgba();
    assert!(
        klaxon.light_color != normal.light_color
            && klaxon_light.red > klaxon_light.green
            && klaxon_light.red > klaxon_light.blue,
        "the first-escape countdown should push the facility into red klaxon lighting"
    );
}

/// Phase 41 (pressure made flesh): collapse-sealed thresholds remain visible as rubble
/// instead of disappearing from the room wall.
#[test]
fn collapse_sealed_thresholds_render_rubble_from_the_style_module() {
    use crate::sim::director::MatchDirector;
    use crate::sim::state::TeleportState;
    use observed_style::{self as style, SurfaceRole};

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    let room = {
        let runtime = app.world().resource::<MatchDirector>();
        runtime.live.host_match().local_room()
    };
    {
        let mut runtime = app.world_mut().resource_mut::<MatchDirector>();
        runtime
            .live
            .host
            .match_state
            .competitive
            .structure
            .seal_room(room);
    }
    app.world_mut().resource_mut::<TeleportState>().rendered = None;
    app.update();

    let expected = style::surface(SurfaceRole::Rubble);
    let rubble_handles: Vec<Handle<StandardMaterial>> = {
        let world = app.world_mut();
        let mut query = world.query::<(&Name, &MeshMaterial3d<StandardMaterial>)>();
        query
            .iter(world)
            .filter(|(name, _)| name.as_str() == "Collapsed rubble")
            .map(|(_, handle)| handle.0.clone())
            .collect()
    };
    let found_rubble = {
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        rubble_handles.iter().any(|handle| {
            materials.get(handle).is_some_and(|material| {
                material.base_color == expected.base_color
                    && material.emissive == expected.emissive
                    && material.unlit
            })
        })
    };
    assert!(
        found_rubble,
        "a collapse-sealed threshold should render a SurfaceRole::Rubble leaf"
    );
    assert_eq!(
        count::<crate::view::components::PassagePreview>(&mut app),
        0,
        "collapsed thresholds are not traversable previews"
    );
}

/// Phase 40b (readable irreversibility): placing into a Gantry hallway renders the
/// jump-map decks as standable, lit, legible geometry — not as generic "Place wall"
/// blocks — so the commitment is visible before the jump.
#[test]
fn a_gantry_hallway_renders_readable_decks_and_no_deck_leaks_into_the_wall_path() {
    use crate::hallway::{HallwayFlavor, TEMPLATES};
    use crate::teleport::Place;
    use observed_core::RoomId;

    let gantry_variation = TEMPLATES
        .iter()
        .position(|t| t.flavor == HallwayFlavor::Gantry)
        .expect("a Gantry template exists in the hallway library");

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    app.world_mut()
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let runtime = world.resource::<crate::sim::director::MatchDirector>();
            let keys = world.resource::<keystones::KeystoneState>();
            let items = world.resource::<items::ItemsState>();
            let game = runtime.live.host_match();
            let from = game.local_room();
            let to = game.local_target().unwrap_or(RoomId(from.0 + 1));
            crate::screens::match_runtime::debug_place_into(
                &mut tp,
                runtime,
                Place::Hallway {
                    from,
                    to,
                    variation: gantry_variation,
                },
                from,
                keys,
                items,
            );
        });
    app.update();

    let names: Vec<String> = {
        let world = app.world_mut();
        let mut query = world.query::<&Name>();
        query.iter(world).map(|n| n.as_str().to_string()).collect()
    };

    let deck_count = names.iter().filter(|n| n.as_str() == "Gantry deck").count();
    let edge_count = names
        .iter()
        .filter(|n| n.as_str() == "Gantry deck edge")
        .count();
    let understory_count = names
        .iter()
        .filter(|n| n.as_str() == "Understory landing")
        .count();
    assert!(
        deck_count >= 6,
        "expected >= 6 Gantry deck entities (6 platforms + stair treads), got {deck_count}"
    );
    assert!(
        edge_count >= 4,
        "expected >= 4 Gantry deck edge entities (one platform's worth of rim strips), got {edge_count}"
    );
    assert!(
        understory_count >= 1,
        "expected >= 1 Understory landing marker, got {understory_count}"
    );

    // No deck solid should have leaked into the generic "Place wall" render path: a
    // deck's collision box only ever spans up to its (short) top_y, while every real
    // wall spans the full WALL_HEIGHT, so a wall-named entity with a short y-scale
    // means `spawn_hallway_shell`'s deck-solid exclusion missed a match.
    let leaked_wall = {
        let world = app.world_mut();
        let mut query = world.query::<(&Name, &Transform)>();
        query
            .iter(world)
            .any(|(name, transform)| name.as_str() == "Place wall" && transform.scale.y < 3.0)
    };
    assert!(
        !leaked_wall,
        "no 'Place wall' entity should have a deck-scale (< 3.0) y-extent"
    );
}

/// The gantry deck material is built from the `GantryDeck` treatment (through the same
/// `textured_neon_material` helper every other floor-like surface uses) rather than an
/// ad-hoc colour choice — the Legibility Contract requires every rendered surface trace
/// back to a shared `observed_style` role. Since the floor texture drop-in slot is
/// present in this workspace, the built material — like the other textured surfaces —
/// reads `base_color: WHITE` with the texture bound and `unlit: true` (mirroring
/// `material_named_has_texture`'s existing textured-surface assertions); the
/// entity is also named "Gantry deck" so it is queryable like every other place mesh.
#[test]
fn gantry_deck_material_is_built_from_the_style_treatment() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    let assets = app.world().resource::<crate::view::assets::MatchAssets>();
    let handle = assets.gantry_deck_material.clone();
    let materials = app.world().resource::<Assets<StandardMaterial>>();
    let mat = materials
        .get(&handle)
        .expect("gantry deck material registered in MatchAssets");
    assert!(
        mat.base_color_texture.is_some(),
        "gantry deck material should be built via textured_neon_material like the other floor surfaces"
    );
    assert_eq!(
        mat.base_color,
        Color::WHITE,
        "a textured neon material reads WHITE base_color with the texture carrying colour"
    );
    assert!(
        mat.unlit,
        "a textured neon material goes unlit once a texture is bound"
    );
}

/// Phase 42 (team colours become a style-owned semantic signal): the theme's
/// `TEAM_COLORS`/`team_color` must read exactly the same base colours the shared
/// `observed_style::team` palette defines — this refactor is required to produce zero
/// visual change.
#[test]
fn theme_team_colors_match_style_team_colors() {
    use crate::view::theme::{TEAM_COLORS, team_color};

    for (i, &color) in TEAM_COLORS.iter().enumerate() {
        assert_eq!(
            color,
            observed_style::team(i).base_color,
            "theme TEAM_COLORS[{i}] must match observed_style::team({i})",
        );
        assert_eq!(
            team_color(i),
            observed_style::team(i).base_color,
            "theme::team_color({i}) must match observed_style::team({i})",
        );
    }
}

/// Phase 42 (fix the stale-frame-light defect): before this fix, `LastRenderedSignature`
/// did not track rival presence/anchor state, so a rival moving into a neighbouring room
/// with everything else unchanged (sealed slots, item count, collapse state, klaxon)
/// left the cached signature untouched and the place never rebuilt — the frame light
/// stayed stale. This mirrors `a_rival_in_the_neighbouring_room_tints_the_threshold_light_with_its_colour`
/// but deliberately does NOT clear `tp.rendered`, so only the signature hash — not the
/// "place changed" shortcut — can trigger the rebuild.
#[test]
fn a_rival_moving_into_a_neighbour_refreshes_the_frame_light_without_a_place_change() {
    use crate::sim::director::MatchDirector;
    use crate::view::theme::TEAM_COLORS;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // build the start room

    let neighbour = {
        let rt = app.world().resource::<MatchDirector>();
        let game = rt.live.host_match();
        let conns = crate::sim::nav::connections_for(game, game.local_room());
        *conns.first().expect("the start room has a neighbour")
    };
    {
        let mut rt = app.world_mut().resource_mut::<MatchDirector>();
        let base = rt.live.host.match_state.competitive.teams[1].member_base;
        rt.live.host.match_state.competitive.structure.graph.players[base] = neighbour;
    }
    // Deliberately do NOT reset `tp.rendered` — the only thing that can force a rebuild
    // here is the rival-state hash in `LastRenderedSignature`.
    app.update();

    let rival = TEAM_COLORS[1].to_srgba();
    let found = {
        let world = app.world_mut();
        let mut lights = world.query::<(&PointLight, &Name)>();
        lights.iter(world).any(|(light, name)| {
            let c = light.color.to_srgba();
            name.as_str() == "Doorframe tether light"
                && (c.red - rival.red).abs() < 0.01
                && (c.green - rival.green).abs() < 0.01
                && (c.blue - rival.blue).abs() < 0.01
        })
    };
    assert!(
        found,
        "moving a rival into a neighbour must rebuild the place and refresh the frame \
         light even without a place/teleport change"
    );
}
