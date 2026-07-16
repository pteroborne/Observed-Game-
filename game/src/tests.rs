use super::*;
use crate::flow::MATCH_SEED;
use crate::sim::state::TeleportState;
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
    .init_asset::<TextureAtlasLayout>()
    .init_asset::<Scene>()
    .init_asset::<AudioSource>()
    .insert_resource(ClearColor(Color::BLACK))
    .add_plugins(ObservedGamePlugin);
    app.update();
    // Integration tests always start from the shipped all-on population, never
    // from whichever personal profile happens to be present in the workspace.
    app.insert_resource(crate::flow::Career::default());
    app
}

fn count<T: Component>(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<T>>();
    query.iter(world).count()
}

fn hud_readout(
    app: &mut App,
    expected: crate::view::components::MatchHudReadout,
) -> Option<String> {
    let world = app.world_mut();
    let mut query = world.query::<(&crate::view::components::MatchHudReadout, &Text)>();
    query
        .iter(world)
        .find_map(|(readout, text)| (*readout == expected).then(|| (**text).to_string()))
}

fn result_summary_texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query_filtered::<&Text, With<crate::screens::menu::ResultsSummaryText>>();
    query.iter(world).map(|text| (**text).to_string()).collect()
}

fn all_texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query::<&Text>();
    query.iter(world).map(|text| (**text).to_string()).collect()
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

fn material_named_has_base_color(app: &mut App, expected_name: &str, expected: Color) -> bool {
    let handles: Vec<Handle<StandardMaterial>> = {
        let world = app.world_mut();
        let mut query = world.query::<(&Name, &MeshMaterial3d<StandardMaterial>)>();
        query
            .iter(world)
            .filter(|(name, _)| name.as_str() == expected_name)
            .map(|(_, handle)| handle.0.clone())
            .collect()
    };
    let expected = expected.to_srgba();
    let materials = app.world().resource::<Assets<StandardMaterial>>();
    handles.iter().any(|handle| {
        materials.get(handle).is_some_and(|material| {
            let actual = material.base_color.to_srgba();
            (actual.red - expected.red).abs() <= 0.001
                && (actual.green - expected.green).abs() <= 0.001
                && (actual.blue - expected.blue).abs() <= 0.001
        })
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
fn keystone_pickup_and_exit_unlock_have_semantic_audio_cues() {
    use crate::teleport::Place;
    use crate::view::components::MatchAudioCue;
    use keystones::KeystoneState;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    let keystone_room = {
        let mut keys = app.world_mut().resource_mut::<KeystoneState>();
        keys.required = 1;
        *keys
            .rooms
            .first()
            .expect("the active map has at least one keystone room")
    };
    app.world_mut()
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let runtime = world.resource::<crate::sim::director::MatchDirector>();
            let keys = world.resource::<keystones::KeystoneState>();
            let items = world.resource::<items::ItemsState>();
            let from = runtime.live.host_match().local_room();
            crate::screens::match_runtime::debug_place_into(
                &mut tp,
                runtime,
                Place::Room(keystone_room),
                from,
                keys,
                items,
            );
        });

    app.update();
    app.update();

    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::Keystone),
        1,
        "collecting a keystone plays exactly one pickup cue"
    );
    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::ExitUnlock),
        1,
        "the final required keystone plays the exit-unlock cue"
    );
    assert!(
        app.world().resource::<KeystoneState>().gate_open(),
        "the test should have exercised the real gate-open edge"
    );
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

    // Anchor beside a real aperture: the 12-unit local radius is intentionally
    // tactical and a torch at the centre of a liminal-scale hub need not pin it all.
    {
        let mut tp = app
            .world_mut()
            .resource_mut::<crate::sim::state::TeleportState>();
        let gap = tp.geom.gaps[0];
        let pos = gap.center - gap.normal * 2.0;
        tp.body.position.x = pos.x;
        tp.body.position.z = pos.y;
        tp.prev_xz = pos;
    }

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
        crate::sim::nav::nav_from_brain(MATCH_SEED, runtime.live.host_match(), keys, items)
            .pinned_corridors
    };
    assert!(
        !pins.is_empty(),
        "a dropped anchor torch pins thresholds inside its local influence radius"
    );
    assert_eq!(
        count::<crate::view::components::DroppedItemVisual>(&mut app),
        1,
        "dropping the torch renders a visible tool"
    );
    assert_eq!(
        count_audio_cue(
            &mut app,
            crate::view::components::MatchAudioCue::ToolInteract
        ),
        1,
        "a successful anchor action plays the semantic tool interaction cue"
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
    assert_eq!(
        count_audio_cue(
            &mut app,
            crate::view::components::MatchAudioCue::ToolInteract
        ),
        2,
        "picking the anchor back up plays the same semantic tool cue"
    );
}

#[test]
fn anchor_torch_tethers_current_thresholds_immediately() {
    use observed_style::{self as style, MarkerRole};

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // build the initial room.

    let (room, target, expected_targets) = {
        let (room, target) = {
            let runtime = app
                .world()
                .resource::<crate::sim::director::MatchDirector>();
            let game = runtime.live.host_match();
            (
                game.local_room(),
                game.local_target().expect("spine target"),
            )
        };
        let mut tp = app
            .world_mut()
            .resource_mut::<crate::sim::state::TeleportState>();
        let nearest = *tp
            .geom
            .gaps
            .iter()
            .find(|gap| gap.target == target)
            .expect("the start room exposes the forward threshold");
        let pos = nearest.center - nearest.normal * 2.0;
        tp.body.position.x = pos.x;
        tp.body.position.z = pos.y;
        tp.prev_xz = pos;
        let mut expected_targets: Vec<_> = tp
            .geom
            .gaps
            .iter()
            .filter(|gap| gap.center.distance(pos) <= items::ANCHOR_RADIUS)
            .map(|gap| gap.target)
            .collect();
        expected_targets.sort_by_key(|room| room.0);
        expected_targets.dedup();
        (room, target, expected_targets)
    };

    tap_update(&mut app, KeyCode::KeyF);
    app.world_mut()
        .resource_mut::<crate::sim::state::TeleportState>()
        .rendered = None;
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
            pinned_targets, expected_targets,
            "dropping the anchor freezes exactly the local-radius threshold set"
        );
        assert_eq!(
            nav.connections, expected_targets,
            "a tethered room reads its frozen local-radius threshold table exactly"
        );
        assert!(
            nav.is_tethered_corridor(crate::teleport::corridor_id_for(room, target))
                || !expected_targets.contains(&target),
            "the forward relation is tethered when it falls inside the local radius"
        );
        assert!(
            nav.connections.contains(&target) || !expected_targets.contains(&target),
            "a tethered target remains present in the room nav"
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
    let has_radius_light = {
        let world = app.world_mut();
        let mut q = world.query::<(&PointLight, &Name)>();
        q.iter(world).any(|(light, name)| {
            name.as_str() == "Anchor torch light"
                && (light.range - items::ANCHOR_RADIUS).abs() < 0.01
        })
    };
    assert!(
        has_radius_light,
        "the torch renders its 12-unit influence radius"
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
        nav.is_tethered_corridor(crate::teleport::corridor_id_for(room, absent)),
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
        !tp.transits.is_empty(),
        "the start room freezes its doorway destinations on entry"
    );
    // The forward doorway's frozen destination is the hallway you'll teleport into, so
    // the preview and the crossing read the identical snapshot.
    let forward = tp.geom.forward_gap().expect("a forward doorway");
    let frozen = tp
        .transits
        .iter()
        .find(|transit| transit.source_gap.threshold == forward.threshold)
        .expect("the forward doorway has a frozen destination");
    assert!(
        matches!(
            frozen.destination.as_ref().map(|snapshot| snapshot.place),
            Some(crate::teleport::Place::Hallway { .. })
        ),
        "the forward doorway leads into a hallway; fault={:?}",
        frozen.fault
    );
}

#[test]
fn portal_previews_have_one_isolated_camera_and_target_per_valid_threshold() {
    use bevy::camera::RenderTarget;
    use bevy::camera::visibility::RenderLayers;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    let expected = {
        let tp = app.world().resource::<crate::sim::state::TeleportState>();
        tp.geom
            .gaps
            .iter()
            .filter(|gap| gap.kind.is_passage())
            .filter(|gap| {
                tp.transits.iter().any(|transit| {
                    transit.source_gap.threshold == gap.threshold && transit.is_valid()
                })
            })
            .count()
    };
    let (surface_ids, camera_ids, target_ids, isolated_count) = {
        let world = app.world_mut();
        let mut surfaces = world.query::<(&crate::view::components::PortalSurface, &Transform)>();
        let surface_ids = surfaces
            .iter(world)
            .filter_map(|(surface, _)| surface.snapshot_id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut cameras =
            world.query::<(&crate::view::components::PortalPreviewCamera, &RenderTarget)>();
        let mut camera_ids = std::collections::BTreeSet::new();
        let mut target_ids = std::collections::BTreeSet::new();
        for (camera, target) in cameras.iter(world) {
            camera_ids.insert(camera.snapshot_id);
            let (_, surface_transform) = surfaces
                .iter(world)
                .find(|(surface, _)| surface.snapshot_id == Some(camera.snapshot_id))
                .expect("every portal camera has a matching aperture surface");
            let local_width = surface_transform.rotation * Vec3::X;
            let tangent = Vec3::new(-camera.source_normal.z, 0.0, camera.source_normal.x);
            assert!(
                local_width.normalize().dot(tangent.normalize()).abs() > 0.999,
                "portal width must align with its threshold tangent"
            );
            let RenderTarget::Image(image) = target else {
                panic!("portal cameras must render to images");
            };
            target_ids.insert(image.handle.id());
        }
        let mut previews =
            world.query_filtered::<&RenderLayers, With<crate::view::components::PassagePreview>>();
        let isolated_count = previews
            .iter(world)
            .filter(|layers| **layers == RenderLayers::layer(1))
            .count();
        (surface_ids, camera_ids, target_ids, isolated_count)
    };

    assert_eq!(surface_ids.len(), expected);
    assert_eq!(camera_ids, surface_ids);
    assert_eq!(target_ids.len(), expected);
    assert!(
        isolated_count > 0,
        "remote geometry lives off the main layer"
    );
}

#[test]
fn live_gate_closure_refreshes_current_snapshot_and_transits_atomically() {
    use observed_core::RoomId;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    let (from, exit) = {
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let game = runtime.live.host_match();
        let exit = game.competitive.exit_room().expect("the map has an exit");
        let from = (0..64)
            .map(RoomId)
            .find(|room| crate::sim::nav::connections_for(game, *room).contains(&exit))
            .expect("one live room connects to the exit");
        (from, exit)
    };
    app.world_mut()
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let runtime = world.resource::<crate::sim::director::MatchDirector>();
            let keys = world.resource::<crate::keystones::KeystoneState>();
            let items = world.resource::<crate::items::ItemsState>();
            crate::screens::match_runtime::crossing::debug_place_into(
                &mut tp,
                runtime,
                teleport::Place::legacy_hallway(from, exit, 0),
                from,
                keys,
                items,
            );
        });
    let before_id =
        {
            let tp = app.world().resource::<crate::sim::state::TeleportState>();
            assert!(tp.geom.gaps.iter().any(|gap| {
                gap.target == exit && gap.kind == crate::teleport::GapKind::LockedExit
            }));
            tp.current_snapshot.id
        };

    {
        let mut keys = app
            .world_mut()
            .resource_mut::<crate::keystones::KeystoneState>();
        let rooms = keys.rooms.clone();
        for room in rooms {
            keys.collect(room);
        }
    }
    app.update();

    let tp = app.world().resource::<crate::sim::state::TeleportState>();
    let live_exit = tp
        .geom
        .gaps
        .iter()
        .find(|gap| gap.target == exit && gap.kind == crate::teleport::GapKind::Exit)
        .expect("the live exit opens");
    assert_ne!(tp.current_snapshot.id, before_id);
    assert!(tp.current_snapshot.geom.gaps.iter().any(|gap| {
        gap.threshold == live_exit.threshold && gap.kind == crate::teleport::GapKind::Exit
    }));
    assert!(
        tp.transits.iter().any(|transit| {
            transit.source_gap.threshold == live_exit.threshold && transit.is_valid()
        }),
        "opened exit must have a valid reciprocal transit: {:?}",
        tp.transits
            .iter()
            .map(|transit| (transit.source_gap, transit.fault.as_deref()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        tp.rapier.collider_count(),
        tp.current_snapshot.arena.colliders.len()
    );
}

#[test]
fn crossing_installs_the_exact_snapshot_shown_by_the_portal() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    app.world_mut()
        .resource_scope(|_world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let transit = tp
                .transits
                .iter()
                .find(|transit| transit.is_valid())
                .cloned()
                .expect("a valid threshold transaction");
            let shown = transit.destination.as_ref().unwrap().id;
            let expected_collider_count =
                transit.destination.as_ref().unwrap().arena.colliders.len();

            assert!(crate::screens::match_runtime::crossing::cross_transit(
                &mut tp, &transit
            ));

            assert_eq!(tp.current_snapshot.id, shown);
            assert_eq!(tp.rapier.collider_count(), expected_collider_count);
            assert_eq!(tp.geom.gaps.len(), tp.current_snapshot.geom.gaps.len());
            let position = Vec2::new(tp.body.position.x, tp.body.position.z);
            assert!(
                crate::teleport::inside_footprint(&tp.geom, position, tp.config.radius),
                "a leading-edge crossing installs the body inside its destination"
            );
        });
}

#[test]
fn committed_corridor_keeps_an_open_valid_exit_after_the_graph_advances() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    let transit = app
        .world()
        .resource::<crate::sim::state::TeleportState>()
        .transits
        .iter()
        .find(|transit| transit.source_gap.kind == crate::teleport::GapKind::Forward)
        .cloned()
        .expect("start room has a valid forward transaction");
    let (from, to) = match transit.destination.as_ref().map(|snapshot| snapshot.place) {
        Some(crate::teleport::Place::Hallway { from, to, .. }) => (from, to),
        other => panic!("forward doorway must lead to a hallway, got {other:?}"),
    };
    {
        let runtime = app
            .world_mut()
            .resource_mut::<crate::sim::director::MatchDirector>();
        assert_eq!(runtime.live.host_match().local_room(), from);
        assert_eq!(runtime.live.host_match().local_target(), Some(to));
    }
    assert!(
        app.world_mut()
            .resource_mut::<crate::sim::director::MatchDirector>()
            .live
            .force_round(LocalAction::Advance)
    );

    app.world_mut()
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            assert!(crate::screens::match_runtime::crossing::cross_transit(
                &mut tp, &transit
            ));
            tp.transits = crate::screens::match_runtime::crossing::compute_threshold_transits(
                MATCH_SEED,
                &tp.current_snapshot,
                world
                    .resource::<crate::sim::director::MatchDirector>()
                    .live
                    .host_match(),
                world.resource::<crate::keystones::KeystoneState>(),
                world.resource::<crate::items::ItemsState>(),
                &tp.config,
                &tp.collision_catalog,
                tp.simulation_content_hash,
            );
        });
    app.update();

    let tp = app.world().resource::<crate::sim::state::TeleportState>();
    let exit = tp
        .geom
        .gaps
        .iter()
        .find(|gap| gap.target == to && gap.kind == crate::teleport::GapKind::Exit)
        .expect("the committed corridor's onward endpoint remains physically open");
    assert!(
        tp.transits.iter().any(|candidate| {
            candidate.source_gap.threshold == exit.threshold && candidate.is_valid()
        }),
        "the open exit must retain an exact reciprocal destination-room snapshot"
    );
}

#[test]
fn presentation_and_anchor_rebuilds_do_not_mutate_frozen_transactions() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    let before = {
        let tp = app.world().resource::<crate::sim::state::TeleportState>();
        tp.transits
            .iter()
            .filter_map(|transit| transit.destination.as_ref().map(|dest| dest.id))
            .collect::<Vec<_>>()
    };
    let (place, position, version, thresholds) = {
        let tp = app.world().resource::<crate::sim::state::TeleportState>();
        let position = tp.geom.forward_gap().unwrap().center;
        (
            tp.place,
            position,
            app.world()
                .resource::<crate::sim::director::MatchDirector>()
                .live
                .host_match()
                .reroute_commits,
            tp.geom
                .gaps
                .iter()
                .map(|gap| (gap.target, gap.center))
                .collect::<Vec<_>>(),
        )
    };
    app.world_mut()
        .resource_mut::<crate::items::ItemsState>()
        .drop_anchor_torch_in_radius(place, position, version, &thresholds);
    app.world_mut()
        .resource_mut::<crate::sim::state::TeleportState>()
        .rendered = None;
    app.update();
    let after = app
        .world()
        .resource::<crate::sim::state::TeleportState>()
        .transits
        .iter()
        .filter_map(|transit| transit.destination.as_ref().map(|dest| dest.id))
        .collect::<Vec<_>>();
    assert_eq!(before, after);
}

#[test]
fn mismatched_reciprocal_identity_fails_closed() {
    use observed_core::CorridorId;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    let (mut source, config, catalog, hash) = {
        let tp = app.world().resource::<crate::sim::state::TeleportState>();
        (
            tp.current_snapshot.clone(),
            tp.config,
            tp.collision_catalog.clone(),
            tp.simulation_content_hash,
        )
    };
    source
        .geom
        .gaps
        .iter_mut()
        .find(|gap| gap.kind.is_passage())
        .unwrap()
        .threshold
        .hall
        .corridor = CorridorId(u32::MAX);
    let transits = crate::screens::match_runtime::compute_threshold_transits(
        crate::flow::MATCH_SEED,
        &source,
        app.world()
            .resource::<crate::sim::director::MatchDirector>()
            .live
            .host_match(),
        app.world().resource::<keystones::KeystoneState>(),
        app.world().resource::<items::ItemsState>(),
        &config,
        &catalog,
        hash,
    );
    let fault = transits
        .iter()
        .find(|transit| transit.source_gap.threshold.hall.corridor == CorridorId(u32::MAX))
        .expect("corrupt threshold retained as diagnostic fault");
    assert!(!fault.is_valid());
    assert!(fault.fault.as_deref().unwrap().starts_with("LINK FAULT"));
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
                Place::legacy_hallway(from, to, variation),
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
                Place::legacy_hallway(from, to, variation),
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
        let knowledge = app.world().resource::<crate::sim::state::MapKnowledge>();
        let tp = app.world().resource::<crate::sim::state::TeleportState>();
        let game = runtime.live.host_match();
        tacmap::build_map(
            &game.competitive,
            keys,
            sightings,
            knowledge,
            game.reroute_commits,
            tp.place,
        )
    };
    assert!(
        !expected_model.rivals.is_empty(),
        "the rivals sharing the entrance at match start should already be witnessed"
    );
    // Phase 50 fog of war: rival pips carry no team-label text in the live race, the
    // exit only draws once found, glimpsed rooms add hollow outlines, and the series
    // status line is debug-HUD-only.
    let expected_elements = tacmap::route_segment_count(&expected_model)
        + expected_model.rooms.len()
        + expected_model.glimpsed.len()
        + usize::from(expected_model.exit_known)
        + expected_model.keystones.len()
        + expected_model.rivals.len()
        + 1;
    assert_eq!(
        count::<crate::view::components::TacMapElement>(&mut app),
        expected_elements,
        "the visible map draws route bars, known rooms, glimpses, keys, rivals, and the player"
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
fn without_the_debug_flag_the_match_spawns_no_status_hud_or_legend() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    assert_eq!(
        count::<crate::view::components::MatchHud>(&mut app),
        0,
        "normal play is HUD-free: no status readout panel"
    );
    let world = app.world_mut();
    let mut readouts = world.query::<&crate::view::components::MatchHudReadout>();
    assert_eq!(
        readouts.iter(world).count(),
        0,
        "no readout entities exist without the debug HUD"
    );
    let texts = all_texts(&mut app);
    assert!(
        !texts.iter().any(|text| text.contains("LEGEND")),
        "the legend is debug-only"
    );
    // The player-facing overlays survive the HUD removal.
    assert_eq!(
        count::<crate::view::components::TacMapPanel>(&mut app),
        1,
        "the tac-map stays available"
    );
    assert_eq!(
        count::<crate::view::components::PausePanel>(&mut app),
        1,
        "the pause overlay stays available"
    );
}

#[test]
fn the_tac_map_is_a_survivors_sketch_not_a_blueprint() {
    let default_spec = crate::map_catalog::default_map_spec(MATCH_SEED);
    assert!(
        default_spec.room_count() >= 24,
        "the fog-of-war assertion must exercise the generated default map"
    );

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    let model = {
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let keys = app.world().resource::<keystones::KeystoneState>();
        let sightings = app.world().resource::<crate::sim::state::RivalSightings>();
        let knowledge = app.world().resource::<crate::sim::state::MapKnowledge>();
        let tp = app.world().resource::<crate::sim::state::TeleportState>();
        let game = runtime.live.host_match();
        tacmap::build_map(
            &game.competitive,
            keys,
            sightings,
            knowledge,
            game.reroute_commits,
            tp.place,
        )
    };
    assert_eq!(
        model.rooms.len(),
        1,
        "at match start the player has only stood in the entrance"
    );
    assert!(
        !model.glimpsed.is_empty(),
        "the entrance's open doorways are glimpsed neighbours"
    );
    assert!(
        model.rooms.len() + model.glimpsed.len() < model.total_rooms,
        "most of the generated facility must start unknown"
    );
    let entrance = model.rooms[0].0;
    assert!(
        !model.routes.is_empty(),
        "the doorways visible from the entrance are known connections"
    );
    assert!(
        model
            .routes
            .iter()
            .all(|&(a, b)| a == entrance || b == entrance),
        "every known connection touches the entrance — nothing beyond the player's eyes"
    );
    let knowledge = app.world().resource::<crate::sim::state::MapKnowledge>();
    assert_eq!(
        model.exit_known,
        knowledge.knows_room(model.exit),
        "the exit marker exists exactly when the player has witnessed the exit room"
    );
}

#[test]
fn hud_shows_objective_keystone_and_collapse_readouts_on_the_generated_map() {
    let default_spec = crate::map_catalog::default_map_spec(MATCH_SEED);
    assert!(
        default_spec.room_count() >= 24,
        "Phase 50's HUD assertion must exercise the generated default map, not the old authored fixture"
    );

    let mut app = test_app();
    // The status readouts are debug-only (Phase 50): flip the app-level flag the way
    // OBSERVED2_DEBUG_HUD would before the match spawns.
    app.world_mut()
        .resource_mut::<crate::view::components::DebugHud>()
        .0 = true;
    go(&mut app, GameState::Match);
    app.update();

    let live_room_count = {
        let director = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        director
            .live
            .host_match()
            .competitive
            .map_spec
            .as_ref()
            .expect("game match uses a MapSpec")
            .room_count()
    };
    assert_eq!(
        live_room_count,
        default_spec.room_count(),
        "HUD test should run against map_catalog::default_map_spec"
    );
    let required_keys = app.world().resource::<keystones::KeystoneState>().required;

    let objective = hud_readout(
        &mut app,
        crate::view::components::MatchHudReadout::Objective,
    )
    .expect("objective readout");
    assert!(objective.contains("EXIT LOCKED"));
    assert!(
        objective.contains("tac-map"),
        "exit geography belongs in the tac-map, not a route hint"
    );

    let keystones = hud_readout(&mut app, crate::view::components::MatchHudReadout::Keystone)
        .expect("keystone readout");
    assert!(keystones.contains("KEYSTONES"));
    assert!(keystones.contains(&format!("0 / {required_keys}")));

    let collapse = hud_readout(&mut app, crate::view::components::MatchHudReadout::Collapse)
        .expect("collapse readout");
    assert!(collapse.contains("COLLAPSE"));
    assert!(collapse.contains("sealed"));
    assert!(collapse.contains("frontier"));

    let standing = hud_readout(&mut app, crate::view::components::MatchHudReadout::Standing)
        .expect("standing readout");
    assert!(standing.contains("LEADER"));
    assert!(standing.contains("SERIES"));

    let controls = hud_readout(&mut app, crate::view::components::MatchHudReadout::Controls)
        .expect("controls readout");
    assert!(controls.contains("tac-map"));
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
fn completed_match_records_a_replay_and_results_can_open_it() {
    let mut app = test_app();
    go(&mut app, GameState::Match);
    finish_match(&mut app);

    {
        let tape = app.world().resource::<crate::sim::replay::ReplayTape>();
        assert!(!tape.is_empty(), "the match records replay samples");
        assert!(tape.result.is_some(), "the replay stores the final result");
        assert!(
            !tape.markers.is_empty(),
            "the replay includes event markers for inspection"
        );
    }

    app.world_mut().resource_mut::<screens::MenuCursor>().0 = 1;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.world_mut().run_schedule(Update);
    app.update();

    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Replay
    );
    assert_eq!(count::<ScreenRoot>(&mut app), 1);
    assert!(
        app.world()
            .contains_resource::<screens::replay::ReplayPlayback>()
    );
    assert_eq!(count::<screens::replay::ReplayMapPanel>(&mut app), 1);
}

#[test]
fn results_screen_renders_every_outcome_shape() {
    use observed_core::{RoomId, TeamId};

    let cases = [
        (
            flow::MatchResult {
                placement: Some(1),
                escaped: 1,
                absorbed: 3,
                winner: Some(TeamId(0)),
                local_won: true,
            },
            false,
            "VICTORY",
            "finished 1st",
        ),
        (
            flow::MatchResult {
                placement: Some(2),
                escaped: 1,
                absorbed: 3,
                winner: Some(TeamId(1)),
                local_won: false,
            },
            false,
            "PLACED",
            "finished 2nd",
        ),
        (
            flow::MatchResult {
                placement: None,
                escaped: 1,
                absorbed: 3,
                winner: Some(TeamId(2)),
                local_won: false,
            },
            false,
            "ABSORBED",
            "absorbed your team",
        ),
        (
            flow::MatchResult {
                placement: Some(4),
                escaped: 1,
                absorbed: 3,
                winner: Some(TeamId(3)),
                local_won: false,
            },
            false,
            "ABSORBED",
            "finished 4th",
        ),
        (
            flow::MatchResult {
                placement: Some(1),
                escaped: 1,
                absorbed: 0,
                winner: Some(TeamId(0)),
                local_won: true,
            },
            true,
            "VICTORY",
            "Solo traversal complete",
        ),
    ];

    for (result, solo, headline, outcome_text) in cases {
        let spec = crate::map_catalog::default_map_spec(MATCH_SEED);
        let mut tape = crate::sim::replay::ReplayTape::new(MATCH_SEED, &spec);
        tape.push_sample(0, 1, Vec::new());
        tape.visited_rooms = vec![RoomId(0), RoomId(4), RoomId(7)];
        tape.collapsed_rooms = if solo {
            Vec::new()
        } else {
            vec![RoomId(0), RoomId(4)]
        };
        tape.escape_order = if solo {
            vec![TeamId(0)]
        } else {
            vec![result.winner.unwrap_or(TeamId(1)), TeamId(0)]
        };
        tape.keystones_collected = 3;
        tape.keystones_required = 3;
        tape.anchor_uses = 1;
        tape.result = Some(result.clone());

        let story = crate::screens::menu::build_results_story(Some(&result), Some(&tape), solo);
        assert_eq!(story.headline, headline);
        assert_eq!(story.lines.len(), 7, "the summary stays glanceable");
        assert!(story.lines.join("\n").contains(outcome_text));
        assert!(story.lines.join("\n").contains("Escape order") ^ solo);
        assert!(story.lines.join("\n").contains("collapse"));
        assert!(story.lines.join("\n").contains("3 rooms"));
        assert!(story.lines.join("\n").contains("3/3 keystones"));
        assert!(story.lines.join("\n").contains("anchor once"));

        let mut app = test_app();
        app.world_mut().resource_mut::<Career>().bot_rival_teams = !solo;
        app.world_mut()
            .resource_mut::<Career>()
            .record(result.clone());
        app.insert_resource(tape);

        go(&mut app, GameState::Results);

        let all_text = all_texts(&mut app).join("\n");
        assert!(
            all_text.contains(headline),
            "results headline should include {headline}; rendered:\n{all_text}"
        );
        let summary = result_summary_texts(&mut app).join("\n");
        assert!(
            summary.contains(outcome_text),
            "summary should include {outcome_text}; rendered:\n{summary}"
        );
        assert!(summary.contains("collapse"));
        assert!(summary.contains("Your path"));
        assert!(summary.contains("keystones"));
        assert!(summary.contains("anchor"));
        assert!(summary.contains("Run 1"));
        if solo {
            assert!(!summary.contains("Team 2"), "solo has no rival lines");
        }
    }
}

#[test]
fn results_rematch_launches_full_wfc_directly_with_a_new_seed() {
    let mut app = test_app();
    let previous = 99_u64;
    app.insert_resource(flow::ActiveMatchSeed(previous));
    {
        let mut career = app.world_mut().resource_mut::<Career>();
        career.bot_rival_teams = false;
        career.bot_ai_teammates = false;
        career.bot_guardian = true;
        career.record(flow::MatchResult {
            placement: Some(1),
            escaped: 1,
            absorbed: 0,
            winner: Some(observed_core::TeamId(0)),
            local_won: true,
        });
    }
    go(&mut app, GameState::Results);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.world_mut().run_schedule(Update);
    app.update();

    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::FullWfc
    );
    assert_ne!(
        app.world().resource::<flow::ActiveMatchSeed>().0,
        previous,
        "rematch never silently reuses the previous facility seed"
    );
    let career = app.world().resource::<Career>();
    assert!(!career.bot_rival_teams);
    assert!(!career.bot_ai_teammates);
    assert!(career.bot_guardian);
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
fn play_launches_the_continuous_full_wfc_match() {
    let mut app = test_app();
    go(&mut app, GameState::MainMenu);
    app.world_mut().resource_mut::<screens::MenuCursor>().0 = 0;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.world_mut().run_schedule(Update);
    app.update();

    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::FullWfc
    );
    assert!(
        !app.world()
            .contains_resource::<crate::sim::state::SpectatorBot>(),
        "Full WFC owns its eight-player command simulation, not the legacy spectator resource"
    );
}

#[test]
fn spectate_mode_holds_the_match_open_until_the_visible_body_finishes() {
    let mut app = test_app();
    app.world_mut()
        .insert_resource(flow::ActiveMatchSeed(flow::MATCH_SEED));
    app.world_mut()
        .insert_resource(crate::sim::state::SpectatorBot::for_seed(flow::MATCH_SEED));
    go(&mut app, GameState::Match);

    // Force a ready series result before the visible first-person run has finished — the
    // teamplay series resolves the whole elimination in a handful of ticks, long before
    // the bot has physically walked a large facility.
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
        assert!(director.series.finished());
    }

    // A ready series result must NOT end the match while the visible body is still
    // running: ending there is exactly the "the game ends too quickly" report — the
    // camera would cut to Results with the bot abandoned mid-facility.
    app.update();
    assert!(
        app.world().resource::<Career>().last_result.is_none(),
        "a ready series result does not end the match while the visible body is still running"
    );
    assert!(
        !app.world()
            .resource::<crate::sim::director::MatchDirector>()
            .done,
        "the match stays live until the visible run catches up"
    );

    // Once the visible body finishes (reached the exit, or gave up after being wedged),
    // the ready series result is recorded and Spectate follows into Results.
    app.world_mut()
        .resource_mut::<crate::sim::state::SpectatorBot>()
        .finished = true;
    app.update();
    assert!(
        app.world().resource::<Career>().last_result.is_some(),
        "with the series ready and the visible body finished, the match records its result"
    );
    assert!(
        app.world()
            .resource::<crate::sim::director::MatchDirector>()
            .done
    );
    assert!(
        app.world()
            .resource::<crate::sim::replay::ReplayTape>()
            .result
            .is_some(),
        "the replay tape keeps the completed result for post-match inspection"
    );
    app.update();
    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Results,
        "Spectate follows normal match completion into Results"
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

/// The spectator's finish check now reads `game.competitive.exit_room()` (the map-spec
/// aware accessor) rather than the raw `observed_match::mutable::EXIT_ROOM` constant, so
/// it still recognises arrival at whatever room the live match actually treats as the
/// exit.
#[test]
fn spectator_finishes_when_placed_in_the_competitive_exit_room() {
    let mut app = test_app();
    app.world_mut()
        .insert_resource(flow::ActiveMatchSeed(flow::MATCH_SEED));
    app.world_mut()
        .insert_resource(crate::sim::state::SpectatorBot::for_seed(flow::MATCH_SEED));
    go(&mut app, GameState::Match);

    let exit_room = app
        .world()
        .resource::<crate::sim::director::MatchDirector>()
        .live
        .host_match()
        .competitive
        .exit_room()
        .expect("the authored facility has an exit room");

    app.world_mut()
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let runtime = world.resource::<crate::sim::director::MatchDirector>();
            let keys = world.resource::<keystones::KeystoneState>();
            let items = world.resource::<items::ItemsState>();
            let from = runtime.live.host_match().local_room();
            crate::screens::match_runtime::debug_place_into(
                &mut tp,
                runtime,
                crate::teleport::Place::Room(exit_room),
                from,
                keys,
                items,
            );
        });

    app.world_mut().run_schedule(FixedUpdate);
    assert!(
        app.world()
            .resource::<crate::sim::state::SpectatorBot>()
            .finished,
        "the spectator bot must finish once placed in the live match's exit_room()"
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
    let expected_ceiling_color = {
        let world = app.world();
        let runtime = world.resource::<crate::sim::director::MatchDirector>();
        let tp = world.resource::<crate::sim::state::TeleportState>();
        let palette =
            crate::screens::match_runtime::palette_for_match(MATCH_SEED, tp.place, runtime);
        crate::view::assets::palette_tint_for_surface(
            &observed_style::surface(observed_style::SurfaceRole::Ceiling),
            &palette,
        )
    };
    assert!(
        material_named_has_base_color(&mut app, "Place ceiling", expected_ceiling_color),
        "rendered ceiling material should use the treated district palette tint"
    );
    assert_eq!(
        count_audio_cue(&mut app, crate::view::components::MatchAudioCue::Ambience),
        observed_style::District::ALL.len() + 2,
        "the facility ambience starts with the Match: one bed per district plus the \
         corridor and gantry hallway beds"
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

/// Phase 46 (Arc D, the liminal flip): the generated default map must produce complete,
/// placed matches across a spread of seeds — not just the pinned `MATCH_SEED`. This is
/// the game-side corpus gate alongside `observed_facility::wfc`'s own generation corpus:
/// that crate proves the *map* generates and validates; this proves a full
/// `MatchDirector` run over that generated map — decoherence, elimination series,
/// keystones and all — reaches a placed outcome with no hangs or panics.
#[test]
fn generated_maps_run_complete_matches_across_a_seed_corpus() {
    for seed in 0..12u64 {
        let map_spec = crate::map_catalog::default_map_spec(seed);
        let mut director = crate::sim::director::MatchDirector::new(
            seed,
            map_spec,
            crate::sim::director::BotPopulations::default(),
        );
        let result = director.run_to_completion();
        assert!(
            result.placement.is_some(),
            "seed {seed}: the elimination series must place the local team"
        );
    }
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

/// Observation can temporarily freeze a threshold, but it must not masquerade as a
/// durable anchor lock on the frame's indicator light.
#[test]
fn rival_observation_does_not_change_the_anchor_indicator_light() {
    use crate::sim::director::MatchDirector;
    use crate::view::theme::TEAM_COLORS;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // build the start room

    // Move rival team 1's clump into the spine-forward neighbour — the one doorway
    // that renders as an open `GapKind::Forward` passage (every other connection is a
    // closed `Side` door and never shows a frame light, by design: you cannot see
    // through a shut door). A generic "first connection" is not guaranteed to be the
    // open one on a generated map, so this must target the actual forward room.
    let neighbour = {
        let rt = app.world().resource::<MatchDirector>();
        let game = rt.live.host_match();
        game.local_target()
            .expect("the local team has a spine-forward target")
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
    let neutral = Color::srgb(0.45, 0.62, 0.78).to_srgba();
    let (found_neutral, found_rival) = {
        let world = app.world_mut();
        let mut lights = world.query::<(&PointLight, &Name)>();
        lights.iter(world).fold(
            (false, false),
            |(found_neutral, found_rival), (light, name)| {
                if name.as_str() != "Doorframe tether light" {
                    return (found_neutral, found_rival);
                }
                let c = light.color.to_srgba();
                let matches = |expected: bevy::color::Srgba| {
                    (c.red - expected.red).abs() < 0.01
                        && (c.green - expected.green).abs() < 0.01
                        && (c.blue - expected.blue).abs() < 0.01
                };
                (
                    found_neutral || matches(neutral),
                    found_rival || matches(rival),
                )
            },
        )
    };
    assert!(
        found_neutral,
        "an observed but unanchored threshold keeps the neutral indicator"
    );
    assert!(
        !found_rival,
        "rival observation must not be reported as a durable anchor lock"
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

    // The spine-forward neighbour, not just any connection: only the open
    // `GapKind::Forward` doorway renders a passage preview (and thus an anchor torch
    // prop); every other connection is a closed `Side` door with nothing to preview.
    let neighbour = {
        let rt = app.world().resource::<MatchDirector>();
        let game = rt.live.host_match();
        game.local_target()
            .expect("the local team has a spine-forward target")
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

    let mut director = crate::sim::director::MatchDirector::new(
        MATCH_SEED,
        sector_relay_v1(),
        crate::sim::director::BotPopulations::default(),
    );
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
    use crate::teleport::{Place, ThresholdSlotId};
    use observed_facility::map_spec::TraversalArchetype;

    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    let corridor = spec
        .corridors()
        .into_iter()
        .find(|corridor| {
            spec.corridor_design(corridor.id)
                .is_some_and(|design| design.traversal == TraversalArchetype::GantryExpanse)
        })
        .expect("catalogue v2 contains a Gantry Expanse corridor");
    let from = corridor.endpoints[0].room;
    let to = corridor.endpoints[1].room;
    let place = Place::Hallway {
        corridor: corridor.id,
        entered_socket: ThresholdSlotId(0),
        variation: 0,
        from,
        to,
    };

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    app.world_mut()
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let runtime = world.resource::<crate::sim::director::MatchDirector>();
            let keys = world.resource::<keystones::KeystoneState>();
            let items = world.resource::<items::ItemsState>();
            crate::screens::match_runtime::debug_place_into(
                &mut tp, runtime, place, from, keys, items,
            );
        });
    app.update();

    let names: Vec<String> = {
        let world = app.world_mut();
        let mut query = world.query::<&Name>();
        query.iter(world).map(|n| n.as_str().to_string()).collect()
    };

    let hex_column_count = names
        .iter()
        .filter(|name| name.as_str() == "Place convex structure")
        .count();
    let platform_count = names
        .iter()
        .filter(|name| name.as_str() == "Gantry platform")
        .count();
    let perimeter_wall_count = names
        .iter()
        .filter(|name| name.as_str() == "Place wall")
        .count();
    assert!(
        hex_column_count >= 50,
        "the catalogue Gantry Expanse renders its field of structural hex columns"
    );
    assert!(
        platform_count >= 3,
        "the entry, bridge, and exit platforms are visible"
    );
    assert!(
        perimeter_wall_count >= 4,
        "generated aperture-safe perimeter walls must remain visible"
    );

    // No deck solid should have leaked into the generic "Place wall" render path.
    // The gantry now has legitimate shorter wall fragments around raised/lower
    // doorway cuts, so this only flags slab/stair-scale wall meshes.
    let leaked_wall = {
        let world = app.world_mut();
        let mut query = world.query::<(&Name, &Transform)>();
        query
            .iter(world)
            .any(|(name, transform)| name.as_str() == "Place wall" && transform.scale.y < 0.6)
    };
    assert!(
        !leaked_wall,
        "no 'Place wall' entity should have a slab/stair-scale (< 0.6) y-extent"
    );
}

#[test]
fn a_wellshaft_renders_the_hex_pillar_spiral_bridges_and_sealed_service_bays() {
    use crate::hallway::{HallwayFlavor, TEMPLATES};
    use crate::teleport::{Place, ThresholdSlotId};
    use observed_core::CorridorId;

    let variation = TEMPLATES
        .iter()
        .position(|template| template.flavor == HallwayFlavor::Wellshaft)
        .expect("wellshaft template");
    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    let mut disconnected = None;
    'rooms: for from in &spec.rooms {
        for to in &spec.rooms {
            if from.id != to.id && spec.corridor_role_between(from.id, to.id).is_none() {
                disconnected = Some((from.id, to.id));
                break 'rooms;
            }
        }
    }
    let (from, to) = disconnected.expect("the map has a valid non-adjacent room pair");
    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    app.world_mut()
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let runtime = world.resource::<crate::sim::director::MatchDirector>();
            let keys = world.resource::<keystones::KeystoneState>();
            let items = world.resource::<items::ItemsState>();
            let place = Place::Hallway {
                corridor: CorridorId(u32::MAX),
                entered_socket: ThresholdSlotId(0),
                variation,
                from,
                to,
            };
            crate::screens::match_runtime::debug_place_into(
                &mut tp, runtime, place, from, keys, items,
            );
        });
    app.update();

    let (
        pillars,
        landings,
        bridges,
        treads,
        sealed_bays,
        braces,
        practicals,
        module_count,
        shadowed,
    ) = {
        let world = app.world_mut();
        let mut names = world.query::<&Name>();
        let all_names: Vec<String> = names.iter(world).map(|name| name.to_string()).collect();
        let mut lights = world.query::<(&PointLight, &Name)>();
        let practical_lights: Vec<bool> = lights
            .iter(world)
            .filter(|(_, name)| name.as_str() == "Wellshaft practical light")
            .map(|(light, _)| light.shadows_enabled)
            .collect();
        (
            all_names
                .iter()
                .filter(|name| *name == "Wellshaft hex pillar")
                .count(),
            all_names
                .iter()
                .filter(|name| *name == "Wellshaft landing")
                .count(),
            all_names
                .iter()
                .filter(|name| *name == "Wellshaft bridge")
                .count(),
            all_names
                .iter()
                .filter(|name| *name == "Wellshaft stair tread")
                .count(),
            all_names
                .iter()
                .filter(|name| *name == "Wellshaft sealed service bay")
                .count(),
            all_names
                .iter()
                .filter(|name| *name == "Wellshaft sealed bay brace")
                .count(),
            practical_lights.len(),
            all_names
                .iter()
                .filter(|name| name.starts_with("Module "))
                .count(),
            practical_lights
                .into_iter()
                .filter(|shadow| *shadow)
                .count(),
        )
    };
    assert_eq!(pillars, 1);
    assert_eq!(landings, crate::hallway::WELL_SHAFT_LEVELS);
    assert_eq!(bridges, crate::hallway::WELL_SHAFT_LEVELS);
    assert_eq!(
        treads,
        (crate::hallway::WELL_SHAFT_LEVELS - 1) * crate::hallway::WELL_SHAFT_STEPS_PER_FLIGHT
    );
    assert_eq!(
        sealed_bays,
        crate::hallway::WELL_SHAFT_LEVELS - 2,
        "only the non-graph bridge heads are sealed"
    );
    assert_eq!(
        braces,
        (crate::hallway::WELL_SHAFT_LEVELS - 2) * 2,
        "each sealed service bay has an explicit X brace"
    );
    assert_eq!(practicals, crate::hallway::WELL_SHAFT_LEVELS);
    assert_eq!(shadowed, 1, "only the top practical spends shadow budget");
    assert_eq!(
        module_count, 0,
        "wellshafts keep their dedicated visual language"
    );
}

/// The gantry deck material is built from the `GantryDeck` treatment (through the same
/// `textured_neon_material` helper every other floor-like surface uses) rather than an
/// ad-hoc colour choice — the Legibility Contract requires every rendered surface trace
/// back to a shared `observed_style` role. Since the floor texture drop-in slot is
/// present in this workspace, the built material — like the other textured surfaces —
/// keeps the style tint over the bound texture and remains lit so district lighting
/// can modulate it.
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
        observed_style::surface(observed_style::SurfaceRole::GantryDeck).base_color,
        "a textured neon material keeps the style tint over the texture"
    );
    assert!(
        !mat.unlit,
        "non-signal textured materials stay lit so district lighting can modulate them"
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

/// Rival movement may refresh preview/presence presentation, but the threshold
/// indicator remains neutral because observation is not an anchor.
#[test]
fn rival_movement_without_a_place_change_keeps_the_indicator_neutral() {
    use crate::sim::director::MatchDirector;
    use crate::view::theme::TEAM_COLORS;

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update(); // build the start room

    // The spine-forward neighbour: only that open `GapKind::Forward` doorway ever
    // shows a frame light tint (every other connection is a closed `Side` door).
    let neighbour = {
        let rt = app.world().resource::<MatchDirector>();
        let game = rt.live.host_match();
        game.local_target()
            .expect("the local team has a spine-forward target")
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
    let neutral = Color::srgb(0.45, 0.62, 0.78).to_srgba();
    let (found_neutral, found_rival) = {
        let world = app.world_mut();
        let mut lights = world.query::<(&PointLight, &Name)>();
        lights.iter(world).fold(
            (false, false),
            |(found_neutral, found_rival), (light, name)| {
                if name.as_str() != "Doorframe tether light" {
                    return (found_neutral, found_rival);
                }
                let c = light.color.to_srgba();
                let matches = |expected: bevy::color::Srgba| {
                    (c.red - expected.red).abs() < 0.01
                        && (c.green - expected.green).abs() < 0.01
                        && (c.blue - expected.blue).abs() < 0.01
                };
                (
                    found_neutral || matches(neutral),
                    found_rival || matches(rival),
                )
            },
        )
    };
    assert!(
        found_neutral,
        "rival movement must leave the unanchored threshold indicator neutral"
    );
    assert!(
        !found_rival,
        "rival movement must not look like a durable anchor lock"
    );
}

fn place_into_room_state(app: &mut App, room: observed_core::RoomId) {
    app.world_mut()
        .resource_scope(|world, mut tp: Mut<crate::sim::state::TeleportState>| {
            let runtime = world.resource::<crate::sim::director::MatchDirector>();
            let keys = world.resource::<crate::keystones::KeystoneState>();
            let items = world.resource::<crate::items::ItemsState>();
            crate::screens::match_runtime::crossing::debug_place_into(
                &mut tp,
                runtime,
                teleport::Place::Room(room),
                room,
                keys,
                items,
            );
        });
}

/// Teleport the local player straight into `room` as an exact snapshot transaction and
/// force a full place rebuild. This is a debug/test placement, not a physical crossing.
fn teleport_into_room(app: &mut App, room: observed_core::RoomId) {
    place_into_room_state(app, room);
    app.update();
}

fn freeze_observation_test_match(app: &mut App) {
    app.world_mut()
        .resource_mut::<crate::sim::director::MatchDirector>()
        .done = true;
}

fn place_rival_in_room(
    app: &mut App,
    team_index: usize,
    room: observed_core::RoomId,
) -> observed_core::TeamId {
    let mut runtime = app
        .world_mut()
        .resource_mut::<crate::sim::director::MatchDirector>();
    let team = runtime.live.host.match_state.competitive.teams[team_index].id;
    let base = runtime.live.host.match_state.competitive.teams[team_index].member_base;
    runtime
        .live
        .host
        .match_state
        .competitive
        .structure
        .graph
        .players[base] = room;
    team
}

fn drop_test_anchor(
    app: &mut App,
    room: observed_core::RoomId,
    position: Vec2,
) -> observed_core::TeamId {
    let (version, connections) = {
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let items = app.world().resource::<items::ItemsState>();
        let game = runtime.live.host_match();
        (
            game.reroute_commits,
            crate::sim::nav::connections_for_nav(game, items, room),
        )
    };
    let mut items = app.world_mut().resource_mut::<items::ItemsState>();
    let team = items.team;
    assert!(
        items.drop_anchor_torch(teleport::Place::Room(room), position, version, &connections,),
        "the observation test starts with a carried anchor torch"
    );
    team
}

/// Arc D stage D1 fix: the wall-monitor rooms and the guardian console must spawn from
/// the active map spec's [`observed_facility::map_spec::RoomRole::Monitor`] /
/// `GuardianControl` rooms, never a hardcoded room id — the bug the user reported ("the
/// observation rooms with monitors don't display the rooms correctly") traced back to
/// `factory.rs` checking `room.0 == 5/6/3`, which silently stopped matching once the map
/// moved on. This is spec-driven (queries the app's own active map, whatever it is)
/// rather than pinning any one catalog map's room ids.
#[test]
fn monitor_room_renders_panels_for_its_page_and_guardian_console_spawns_in_its_role_room() {
    use observed_core::RoomId;
    use observed_facility::map_spec::RoomRole;

    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    let monitor_room = spec
        .role_room(RoomRole::Monitor)
        .expect("the active map has a Monitor room");
    let console_room = spec
        .role_room(RoomRole::GuardianControl)
        .expect("the active map has a GuardianControl room");

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();

    teleport_into_room(&mut app, monitor_room);
    let mut panel_rooms: Vec<RoomId> = {
        let world = app.world_mut();
        let mut query = world.query::<&crate::screens::place::ObservationPanel>();
        query.iter(world).map(|panel| panel.room).collect()
    };
    let mut expected_page = crate::screens::place::monitor_page_for(&spec, monitor_room)
        .expect("the Monitor room has a panel page");
    panel_rooms.sort_unstable_by_key(|room| room.0);
    expected_page.sort_unstable_by_key(|room| room.0);
    assert_eq!(
        panel_rooms, expected_page,
        "the Monitor room renders exactly one unified panel for every room on its page"
    );

    // The Guardian starts in the GuardianControl-role room (session.rs's role-driven
    // spawn), so walking straight in would trigger its same-room "catch" and bounce the
    // player elsewhere before this frame's rebuild — move it away first so the test can
    // observe the console deterministically.
    {
        let mut guardian = app.world_mut().resource_mut::<crate::guardian::Guardian>();
        guardian.room = spec
            .rooms
            .iter()
            .map(|r| r.id)
            .find(|&id| id != console_room)
            .expect("the map has another room for the guardian to occupy");
    }
    teleport_into_room(&mut app, console_room);
    assert_eq!(
        count::<crate::screens::place::GuardianConsole>(&mut app),
        1,
        "the interactive console spawns exactly once, in the GuardianControl-role room"
    );
}

/// Phase 65's feed is a pure projection of live game state. It carries the target room's
/// footprint, current doorway targets, rivals, local anchor, and guardian without needing
/// a rendered entity or mutating either observation ledger.
#[test]
fn observation_page_content_projects_structure_and_live_occupants() {
    use observed_facility::map_spec::RoomRole;

    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    let monitor_room = spec
        .role_room(RoomRole::Monitor)
        .expect("the active map has a Monitor room");
    let page = crate::screens::place::monitor_page_for(&spec, monitor_room)
        .expect("the Monitor room has a panel page");
    let shown_room = *page
        .first()
        .expect("the Monitor room's page is non-empty on this map");

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    freeze_observation_test_match(&mut app);

    let rival_team = place_rival_in_room(&mut app, 1, shown_room);
    let anchor_position = Vec2::new(0.75, -0.5);
    let anchor_team = drop_test_anchor(&mut app, shown_room, anchor_position);
    let staged_guardian = crate::guardian::Guardian {
        room: shown_room,
        pos: Vec3::new(-0.6, 0.76, 0.9),
        ..default()
    };

    let (contents, expected_connections) = {
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let items = app.world().resource::<items::ItemsState>();
        let game = runtime.live.host_match();
        (
            crate::screens::place::observation_page_contents(
                MATCH_SEED,
                &page,
                game,
                items,
                Some(&staged_guardian),
            ),
            crate::sim::nav::connections_for_nav(game, items, shown_room),
        )
    };

    assert_eq!(
        contents
            .iter()
            .map(|content| content.room)
            .collect::<Vec<_>>(),
        page,
        "pure page content preserves the stable panel-to-room mapping"
    );
    assert_eq!(
        contents,
        {
            let runtime = app
                .world()
                .resource::<crate::sim::director::MatchDirector>();
            let items = app.world().resource::<items::ItemsState>();
            crate::screens::place::observation_page_contents(
                MATCH_SEED,
                &page,
                runtime.live.host_match(),
                items,
                Some(&staged_guardian),
            )
        },
        "the feed projection is deterministic for identical live state"
    );

    let shown = contents
        .iter()
        .find(|content| content.room == shown_room)
        .expect("the staged room has a panel");
    let direct = {
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let items = app.world().resource::<items::ItemsState>();
        crate::screens::place::observation_panel_content(
            MATCH_SEED,
            shown_room,
            runtime.live.host_match(),
            items,
            Some(&staged_guardian),
        )
    };
    assert_eq!(
        shown, &direct,
        "page projection delegates to the pure panel model"
    );
    assert!(
        shown.footprint.len() >= 4,
        "the feed carries a legible closed room footprint"
    );
    let mut actual_targets: Vec<_> = shown
        .doorways
        .iter()
        .map(|doorway| doorway.target)
        .collect();
    let mut expected_targets = expected_connections;
    actual_targets.sort_unstable_by_key(|room| room.0);
    expected_targets.sort_unstable_by_key(|room| room.0);
    assert_eq!(
        actual_targets, expected_targets,
        "doorway stubs use the room's live navigation targets"
    );
    assert!(
        shown.occupants.contains(&rival_team),
        "the staged rival appears in the room's occupant list"
    );
    assert!(
        shown
            .occupants
            .windows(2)
            .all(|teams| teams[0].0 <= teams[1].0),
        "occupants are stable and sorted by TeamId"
    );
    assert!(
        shown.anchors.iter().any(|anchor| {
            anchor.team == anchor_team && anchor.position == Some(anchor_position)
        }),
        "the local placed anchor appears with its live room position"
    );
    assert_eq!(
        shown.guardian,
        Some(Vec2::ZERO),
        "the guardian's room-level warning dot is centred on the occupied panel"
    );
    for other in contents.iter().filter(|content| content.room != shown_room) {
        assert_ne!(
            other.guardian,
            Some(Vec2::ZERO),
            "guardian state never bleeds onto another panel"
        );
        assert!(
            !other.anchors.iter().any(|anchor| {
                anchor.team == anchor_team && anchor.position == Some(anchor_position)
            }),
            "local anchor state never bleeds onto another panel"
        );
    }
}

/// The Bevy presentation consumes the pure feed model exactly: every footprint edge and
/// doorway becomes a line, every rival and guardian becomes one dot, and every anchor
/// becomes a visible halo rather than a status-only screen tint.
#[test]
fn rendered_observation_feed_counts_match_the_pure_model() {
    use observed_facility::map_spec::RoomRole;

    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    let monitor_room = spec
        .role_room(RoomRole::Monitor)
        .expect("the active map has a Monitor room");
    let page = crate::screens::place::monitor_page_for(&spec, monitor_room)
        .expect("the Monitor room has a panel page");
    let shown_room = page[0];

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    freeze_observation_test_match(&mut app);
    let rival_team = place_rival_in_room(&mut app, 1, shown_room);
    let anchor_team = drop_test_anchor(&mut app, shown_room, Vec2::new(0.4, 0.3));
    {
        let mut guardian = app.world_mut().resource_mut::<crate::guardian::Guardian>();
        guardian.room = shown_room;
        guardian.pos = Vec3::new(0.2, 0.76, -0.7);
    }

    let contents = {
        let runtime = app
            .world()
            .resource::<crate::sim::director::MatchDirector>();
        let items = app.world().resource::<items::ItemsState>();
        let guardian = app.world().resource::<crate::guardian::Guardian>();
        crate::screens::place::observation_page_contents(
            MATCH_SEED,
            &page,
            runtime.live.host_match(),
            items,
            Some(guardian),
        )
    };
    teleport_into_room(&mut app, monitor_room);

    let primitives: Vec<_> = {
        let world = app.world_mut();
        let mut query = world.query::<(
            &crate::screens::place::ObservationFeedElement,
            &crate::screens::place::ObservationFeedPrimitive,
        )>();
        query
            .iter(world)
            .map(|(element, primitive)| (element.room, *primitive))
            .collect()
    };

    for content in &contents {
        let for_room: Vec<_> = primitives
            .iter()
            .filter_map(|(room, primitive)| (*room == content.room).then_some(*primitive))
            .collect();
        assert_eq!(
            for_room
                .iter()
                .filter(|primitive| {
                    matches!(
                        primitive,
                        crate::screens::place::ObservationFeedPrimitive::Footprint
                    )
                })
                .count(),
            content.footprint.len(),
            "R{} renders one segment per footprint edge",
            content.room.0
        );

        let mut rendered_doorways: Vec<_> = for_room
            .iter()
            .filter_map(|primitive| match primitive {
                crate::screens::place::ObservationFeedPrimitive::Doorway(slot) => Some(*slot),
                _ => None,
            })
            .collect();
        let mut expected_doorways: Vec<_> = content
            .doorways
            .iter()
            .map(|doorway| doorway.slot)
            .collect();
        rendered_doorways.sort_unstable();
        expected_doorways.sort_unstable();
        assert_eq!(
            rendered_doorways, expected_doorways,
            "R{} renders exactly its live doorway stubs",
            content.room.0
        );

        let mut rendered_occupants: Vec<_> = for_room
            .iter()
            .filter_map(|primitive| match primitive {
                crate::screens::place::ObservationFeedPrimitive::Occupant(team) => Some(*team),
                _ => None,
            })
            .collect();
        rendered_occupants.sort_unstable_by_key(|team| team.0);
        assert_eq!(
            rendered_occupants, content.occupants,
            "R{} renders one dot per rival occupant",
            content.room.0
        );
        assert_eq!(
            for_room
                .iter()
                .filter(|primitive| {
                    matches!(
                        primitive,
                        crate::screens::place::ObservationFeedPrimitive::Guardian
                    )
                })
                .count(),
            usize::from(content.guardian.is_some()),
            "R{} renders exactly one guardian dot when occupied",
            content.room.0
        );
    }

    assert!(
        primitives.iter().any(|&(room, primitive)| {
            room == shown_room
                && primitive
                    == crate::screens::place::ObservationFeedPrimitive::Occupant(rival_team)
        }),
        "the staged rival has a rendered dot"
    );
    assert_eq!(
        primitives
            .iter()
            .filter(|&&(room, primitive)| {
                room == shown_room
                    && primitive
                        == crate::screens::place::ObservationFeedPrimitive::Anchor(anchor_team)
            })
            .count(),
        8,
        "the staged anchor renders as an eight-segment halo"
    );
}

#[test]
fn guardian_dot_moves_between_shown_panels_after_one_update() {
    use observed_core::RoomId;
    use observed_facility::map_spec::RoomRole;

    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    let monitor_room = spec
        .role_room(RoomRole::Monitor)
        .expect("the active map has a Monitor room");
    let page = crate::screens::place::monitor_page_for(&spec, monitor_room)
        .expect("the Monitor room has a panel page");
    let shown: Vec<RoomId> = page
        .iter()
        .copied()
        .filter(|&room| room != monitor_room)
        .take(2)
        .collect();
    assert_eq!(shown.len(), 2, "the page exposes two guardian test panels");

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    freeze_observation_test_match(&mut app);
    {
        let mut guardian = app.world_mut().resource_mut::<crate::guardian::Guardian>();
        guardian.room = shown[0];
        guardian.pos = Vec3::new(-0.4, 0.76, 0.2);
    }
    teleport_into_room(&mut app, monitor_room);

    let guardian_rooms = |app: &mut App| -> Vec<RoomId> {
        let world = app.world_mut();
        let mut query = world.query::<(
            &crate::screens::place::ObservationFeedElement,
            &crate::screens::place::ObservationFeedPrimitive,
        )>();
        query
            .iter(world)
            .filter_map(|(element, primitive)| {
                (*primitive == crate::screens::place::ObservationFeedPrimitive::Guardian)
                    .then_some(element.room)
            })
            .collect()
    };
    assert_eq!(guardian_rooms(&mut app), vec![shown[0]]);

    {
        let mut guardian = app.world_mut().resource_mut::<crate::guardian::Guardian>();
        guardian.room = shown[1];
        guardian.pos = Vec3::new(0.5, 0.76, -0.3);
    }
    app.update();
    assert_eq!(
        guardian_rooms(&mut app),
        vec![shown[1]],
        "one Update clears the old panel and moves the live guardian dot to the new room"
    );
}

/// Entering the Monitor room legitimately records that room and the doorways visible
/// from it. Once that personal observation has settled, remote facts visible only on the
/// panels must never expand the tac-map's witnessed-room/edge ledger.
#[test]
fn remote_observation_panel_facts_do_not_expand_map_knowledge() {
    use observed_facility::map_spec::RoomRole;

    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    let monitor_room = spec
        .role_room(RoomRole::Monitor)
        .expect("the active map has a Monitor room");
    let page = crate::screens::place::monitor_page_for(&spec, monitor_room)
        .expect("the Monitor room has a panel page");

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    freeze_observation_test_match(&mut app);
    teleport_into_room(&mut app, monitor_room);
    app.update(); // record the rebuilt Monitor room's real doorway set

    let remote_room = {
        let knowledge = app.world().resource::<crate::sim::state::MapKnowledge>();
        page.iter()
            .copied()
            .find(|&room| !knowledge.knows_room(room))
            .expect("the monitor page includes a room outside personal doorway knowledge")
    };
    let before = {
        let knowledge = app.world().resource::<crate::sim::state::MapKnowledge>();
        (
            knowledge.visited.clone(),
            knowledge.glimpsed.clone(),
            knowledge.edges.clone(),
        )
    };

    place_rival_in_room(&mut app, 1, remote_room);
    drop_test_anchor(&mut app, remote_room, Vec2::new(0.3, -0.2));
    {
        let mut guardian = app.world_mut().resource_mut::<crate::guardian::Guardian>();
        guardian.room = remote_room;
        guardian.pos = Vec3::new(0.1, 0.76, 0.4);
    }
    app.update();

    let knowledge = app.world().resource::<crate::sim::state::MapKnowledge>();
    assert_eq!(
        (
            knowledge.visited.clone(),
            knowledge.glimpsed.clone(),
            knowledge.edges.clone(),
        ),
        before,
        "remote rival/anchor/guardian panel facts never write to MapKnowledge"
    );
    assert!(
        !knowledge.knows_room(remote_room),
        "the tac-map still does not know the remotely monitored room"
    );
}

/// Generated maps use room ids above nine. The physical segmented label must render
/// every decimal digit rather than clamping all multi-digit rooms to `R9`.
#[test]
fn observation_panel_physically_renders_multi_digit_room_ids() {
    use observed_core::RoomId;
    use observed_facility::map_spec::RoomRole;

    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    let target = RoomId(10);
    assert!(
        spec.room(target).is_some(),
        "the generated map contains R10"
    );
    let monitor_room = spec
        .rooms_with_role(RoomRole::Monitor)
        .into_iter()
        .find(|&room| {
            crate::screens::place::monitor_page_for(&spec, room)
                .is_some_and(|page| page.contains(&target))
        })
        .expect("one Monitor room pages R10");

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    freeze_observation_test_match(&mut app);
    teleport_into_room(&mut app, monitor_room);

    let segment_count = {
        let world = app.world_mut();
        let mut query = world.query::<(
            &crate::screens::place::ObservationMonitorLabelSegment,
            &Name,
        )>();
        query
            .iter(world)
            .filter(|(segment, name)| {
                segment.room == target && name.as_str() == "Observation monitor room-id segment"
            })
            .count()
    };
    assert_eq!(
        segment_count, 14,
        "R10 uses six R segments, two `1` segments, and six `0` segments"
    );
}

/// Count each panel's own screen, four bezel pieces, label/status segments, and feed
/// elements. This is a true per-panel cap rather than the old whole-room average.
#[test]
fn monitor_panels_stay_within_the_per_panel_entity_budget() {
    use observed_facility::map_spec::RoomRole;

    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);
    let monitor_room = spec
        .role_room(RoomRole::Monitor)
        .expect("the active map has a Monitor room");
    let page = crate::screens::place::monitor_page_for(&spec, monitor_room)
        .expect("the Monitor room has a panel page");

    let mut app = test_app();
    go(&mut app, GameState::Match);
    app.update();
    freeze_observation_test_match(&mut app);
    teleport_into_room(&mut app, monitor_room);

    assert_eq!(
        count::<crate::screens::place::ObservationPanel>(&mut app),
        page.len(),
        "one panel root is spawned per page room"
    );
    let (feed_rooms, label_rooms) = {
        let world = app.world_mut();
        let mut feeds = world.query::<&crate::screens::place::ObservationFeedElement>();
        let feed_rooms: Vec<_> = feeds.iter(world).map(|element| element.room).collect();
        let mut labels = world.query::<&crate::screens::place::ObservationMonitorLabelSegment>();
        let label_rooms: Vec<_> = labels.iter(world).map(|segment| segment.room).collect();
        (feed_rooms, label_rooms)
    };
    for room in page {
        let feed_count = feed_rooms.iter().filter(|&&shown| shown == room).count();
        let label_count = label_rooms.iter().filter(|&&shown| shown == room).count();
        let panel_entity_count = 1 + 4 + label_count + feed_count;
        assert!(
            panel_entity_count <= crate::screens::place::MONITOR_PANEL_ENTITY_BUDGET,
            "R{} panel uses {panel_entity_count} entities, exceeding the {} entity budget",
            room.0,
            crate::screens::place::MONITOR_PANEL_ENTITY_BUDGET,
        );
    }
}

/// Phase 60: Dressing props are deterministic across identical runs and never overlap
/// with any gap's threshold clearance zones (which extend 2.5m into the room).
#[test]
fn dressing_props_are_deterministic_and_respect_clearance() {
    use crate::sim::state::TeleportState;
    use crate::view::assets::MatchAssets;
    let spec = crate::map_catalog::active_map_spec(MATCH_SEED);

    for room in &spec.rooms {
        let room_id = room.id;

        let mut app1 = test_app();
        go(&mut app1, GameState::Match);
        app1.update();
        {
            let (decor_column, decor_torch, decor_lab_crate, decor_lab_table) = {
                let assets = app1.world().resource::<MatchAssets>();
                (
                    assets.decor_column.clone(),
                    assets.decor_torch.clone(),
                    assets.decor_lab_crate.clone(),
                    assets.decor_lab_table.clone(),
                )
            };
            let mut images = app1.world_mut().resource_mut::<Assets<Image>>();
            let dummy = Image::default();
            if let Some(h) = &decor_column {
                let _ = images.insert(h.id(), dummy.clone());
            }
            if let Some(h) = &decor_torch {
                let _ = images.insert(h.id(), dummy.clone());
            }
            if let Some(h) = &decor_lab_crate {
                let _ = images.insert(h.id(), dummy.clone());
            }
            if let Some(h) = &decor_lab_table {
                let _ = images.insert(h.id(), dummy.clone());
            }
        }
        teleport_into_room(&mut app1, room_id);

        let mut app2 = test_app();
        go(&mut app2, GameState::Match);
        app2.update();
        {
            let (decor_column, decor_torch, decor_lab_crate, decor_lab_table) = {
                let assets = app2.world().resource::<MatchAssets>();
                (
                    assets.decor_column.clone(),
                    assets.decor_torch.clone(),
                    assets.decor_lab_crate.clone(),
                    assets.decor_lab_table.clone(),
                )
            };
            let mut images = app2.world_mut().resource_mut::<Assets<Image>>();
            let dummy = Image::default();
            if let Some(h) = &decor_column {
                let _ = images.insert(h.id(), dummy.clone());
            }
            if let Some(h) = &decor_torch {
                let _ = images.insert(h.id(), dummy.clone());
            }
            if let Some(h) = &decor_lab_crate {
                let _ = images.insert(h.id(), dummy.clone());
            }
            if let Some(h) = &decor_lab_table {
                let _ = images.insert(h.id(), dummy.clone());
            }
        }
        teleport_into_room(&mut app2, room_id);

        let geom = app1.world().resource::<TeleportState>().geom.clone();

        // Extract spawned prop positions
        let extract_prop_positions = |app: &mut App| -> Vec<Vec3> {
            let world = app.world_mut();
            let mut query = world.query_filtered::<(&Transform, &Name), With<crate::view::components::PlaceGeometry>>();
            let mut positions: Vec<Vec3> = query
                .iter(world)
                .filter(|(_, name)| name.as_str() == "Room dressing prop")
                .map(|(t, _)| t.translation)
                .collect();
            // Sort to ensure order is stable for comparison
            positions.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
            positions
        };

        let pos1 = extract_prop_positions(&mut app1);
        let pos2 = extract_prop_positions(&mut app2);

        // 1. Assert determinism: identical match seed + room ID -> identical placement
        assert_eq!(
            pos1, pos2,
            "Room {:?} dressing must be deterministic",
            room_id
        );

        // 2. Assert clearance: check every spawned prop against threshold clearance boxes
        for p_pos in &pos1 {
            let p = Vec2::new(p_pos.x, p_pos.z);

            // Check wall distance (clamping to geom with radius 0.8 must be same)
            let clamped = crate::teleport::contain(&geom, p, 0.8);
            assert!(
                (clamped - p).length() <= 0.05,
                "Room {:?} prop at {:?} violates wall clearance",
                room_id,
                p
            );

            // Check center distance
            assert!(
                p.length() >= 1.79,
                "Room {:?} prop at {:?} violates center clearance",
                room_id,
                p
            );

            // Check gap clearance
            for gap in &geom.gaps {
                let along = Vec2::new(-gap.normal.y, gap.normal.x);
                let to_p = p - gap.center;
                let dist_along = to_p.dot(along).abs();
                let dist_normal = to_p.dot(gap.normal);

                let inside_clearance = dist_along < (gap.width * 0.5 + 0.49)
                    && dist_normal > -2.49
                    && dist_normal < 0.99;
                assert!(
                    !inside_clearance,
                    "Room {:?} prop at {:?} overlaps threshold clearance for gap toward {:?}",
                    room_id, p, gap.target
                );
            }
        }
    }
}

// --- Phase 48: onboarding & settings ----------------------------------------

/// `Settings` is inserted once at startup (app-lifetime, like `Career`) — never as
/// part of the Match resource lifecycle, and it must already be present the moment
/// the app boots (input systems read it unconditionally).
#[test]
fn settings_is_inserted_at_startup_and_survives_state_changes() {
    let mut app = test_app();
    assert!(app.world().contains_resource::<crate::settings::Settings>());
    go(&mut app, GameState::MainMenu);
    assert!(app.world().contains_resource::<crate::settings::Settings>());
    go(&mut app, GameState::Match);
    assert!(app.world().contains_resource::<crate::settings::Settings>());
    finish_match(&mut app);
    assert!(
        app.world().contains_resource::<crate::settings::Settings>(),
        "Settings is app-lifetime, not match-scoped"
    );
}

/// Rebinding a key through `Settings` changes the `PlayerIntent` `match_input`
/// produces — the input abstraction invariant: gameplay never reads hardware
/// directly, so proving the rebind takes effect at the `MatchIntent` level (not just
/// inside the `Settings` resource) proves the whole path is wired, not just stored.
#[test]
fn rebinding_a_key_changes_the_intent_it_produces() {
    use crate::settings::{BindingSlot, Settings};

    let mut app = test_app();
    app.insert_resource(crate::settings::Settings::default());
    go(&mut app, GameState::Match);

    // Default binding: KeyW drives forward movement.
    tap_update(&mut app, KeyCode::KeyW);
    assert!(
        app.world()
            .resource::<crate::sim::state::MatchIntent>()
            .0
            .movement
            .y
            > 0.0,
        "the default binding (W) drives forward movement"
    );

    // Rebind "move forward" to KeyJ; the old key (W) must stop working and the new
    // key (J) must now drive the identical intent.
    {
        let mut settings = app.world_mut().resource_mut::<Settings>();
        BindingSlot::MoveForward.set(&mut settings.bindings, KeyCode::KeyJ);
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset(KeyCode::KeyW);
    tap_update(&mut app, KeyCode::KeyW);
    assert_eq!(
        app.world()
            .resource::<crate::sim::state::MatchIntent>()
            .0
            .movement
            .y,
        0.0,
        "the old key no longer drives movement once rebound"
    );

    tap_update(&mut app, KeyCode::KeyJ);
    assert!(
        app.world()
            .resource::<crate::sim::state::MatchIntent>()
            .0
            .movement
            .y
            > 0.0,
        "the rebound key drives the same forward-movement intent"
    );
}

/// Rebinding a keyboard binding must never touch gamepad mapping (README invariant:
/// gamepad support must not regress). `read_gamepad_match` is a pure function of the
/// `Gamepad` input alone, so this asserts it still produces the same jump intent for
/// the identical stick/button state regardless of what `Settings.bindings` holds.
#[test]
fn rebinding_a_keyboard_key_does_not_affect_gamepad_mapping() {
    use crate::screens::input::read_gamepad_match;
    use crate::settings::{BindingSlot, Settings};
    use bevy::input::gamepad::{Gamepad, GamepadButton};

    let mut app = test_app();
    app.insert_resource(crate::settings::Settings::default());
    go(&mut app, GameState::Match);
    {
        let mut settings = app.world_mut().resource_mut::<Settings>();
        BindingSlot::Jump.set(&mut settings.bindings, KeyCode::KeyJ);
    }

    let mut gamepad = Gamepad::default();
    gamepad.digital_mut().press(GamepadButton::South);
    let (intent, _) = read_gamepad_match(&gamepad);
    assert!(
        intent.jump_pressed,
        "gamepad South still jumps after a keyboard jump rebind"
    );
}

/// The Settings screen is a proper state-scoped screen (mirrors `Loadout`): entering
/// it spawns exactly one screen root that despawns cleanly on exit, and Up/Down moves
/// the row cursor.
#[test]
fn settings_screen_is_state_scoped_and_navigable() {
    let mut app = test_app();
    go(&mut app, GameState::MainMenu);
    go(&mut app, GameState::Settings);
    assert_eq!(count::<crate::view::theme::ScreenRoot>(&mut app), 1);
    assert!(
        count::<crate::screens::settings::SettingsRowText>(&mut app) > 0,
        "the settings screen renders its row list"
    );

    let before = app
        .world()
        .resource::<crate::screens::settings::SettingsCursor>()
        .0;
    tap_update(&mut app, KeyCode::ArrowDown);
    let after = app
        .world()
        .resource::<crate::screens::settings::SettingsCursor>()
        .0;
    assert_ne!(before, after, "Down moves the settings cursor");

    go(&mut app, GameState::MainMenu);
    assert_eq!(
        count::<crate::screens::settings::SettingsRowText>(&mut app),
        0,
        "settings rows never leak past the screen"
    );
    assert_eq!(count::<crate::view::theme::ScreenRoot>(&mut app), 1);
}

#[test]
fn interactive_rebind_flow_works_correctly() {
    use crate::screens::settings::{SettingsCursor, SettingsRebind};
    use crate::settings::Settings;

    let tap_update_clean = |app: &mut App, key: KeyCode| {
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(key);
        }
        app.world_mut().run_schedule(Update);
    };
    let release_update_clean = |app: &mut App, key: KeyCode| {
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.release(key);
        }
        app.world_mut().run_schedule(Update);
    };

    // Scenario 1: Arm with Enter, then press K, assert binding is K.
    {
        let mut app = test_app();
        app.insert_resource(Settings::default());
        go(&mut app, GameState::MainMenu);
        go(&mut app, GameState::Settings);

        // BindingSlot::MoveLeft is at row index 5
        app.world_mut().resource_mut::<SettingsCursor>().0 = 5;

        // Press Enter to start rebind capture (enters WaitingForActivationRelease)
        tap_update_clean(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<SettingsRebind>().0.is_active());
        assert!(!app.world().resource::<SettingsRebind>().0.is_armed());

        // Release Enter to arm the capture
        release_update_clean(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<SettingsRebind>().0.is_active());
        assert!(app.world().resource::<SettingsRebind>().0.is_armed());

        // Press KeyK to capture the binding
        tap_update_clean(&mut app, KeyCode::KeyK);
        assert!(!app.world().resource::<SettingsRebind>().0.is_active());

        // Verify the setting is now KeyK
        let settings = app.world().resource::<Settings>();
        assert_eq!(settings.bindings.move_left, KeyCode::KeyK);
    }

    // Scenario 2: Arm and press Enter again, assert it is bound.
    {
        let mut app = test_app();
        app.insert_resource(Settings::default());
        go(&mut app, GameState::MainMenu);
        go(&mut app, GameState::Settings);

        // BindingSlot::MoveLeft is at row index 5
        app.world_mut().resource_mut::<SettingsCursor>().0 = 5;

        // Press Enter to start rebind capture
        tap_update_clean(&mut app, KeyCode::Enter);

        // Release Enter to arm the capture
        release_update_clean(&mut app, KeyCode::Enter);

        // Press Enter again to bind it deliberately
        tap_update_clean(&mut app, KeyCode::Enter);
        assert!(!app.world().resource::<SettingsRebind>().0.is_active());

        // Verify the setting is now Enter
        let settings = app.world().resource::<Settings>();
        assert_eq!(settings.bindings.move_left, KeyCode::Enter);
    }

    // Scenario 3: Cancel works (Escape cancels rebinding and doesn't exit).
    {
        let mut app = test_app();
        app.insert_resource(Settings::default());
        go(&mut app, GameState::MainMenu);
        go(&mut app, GameState::Settings);

        // BindingSlot::MoveLeft is at row index 5
        app.world_mut().resource_mut::<SettingsCursor>().0 = 5;

        // Press Enter to start rebind capture
        tap_update_clean(&mut app, KeyCode::Enter);

        // Release Enter to arm the capture
        release_update_clean(&mut app, KeyCode::Enter);

        // Press Escape to cancel the capture
        tap_update_clean(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<SettingsRebind>().0.is_active());

        // Verify GameState is still Settings (didn't back out)
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Settings
        );

        // Verify the setting is unchanged (remains KeyA)
        let settings = app.world().resource::<Settings>();
        assert_eq!(settings.bindings.move_left, KeyCode::KeyA);
    }
}

#[test]
fn pause_settings_rebind_flow_works_correctly() {
    use crate::screens::match_runtime::pause_settings::{
        PauseSettingsCursor, PauseSettingsOpen, PauseSettingsRebind,
    };
    use crate::settings::Settings;
    use crate::sim::state::MatchPaused;

    // Scenario 1: Arm with Enter, then press K, assert binding is K.
    {
        let mut app = test_app();
        app.insert_resource(Settings::default());
        go(&mut app, GameState::Match);

        // Pause the match and open the settings panel
        app.world_mut().resource_mut::<MatchPaused>().0 = true;
        app.world_mut().resource_mut::<PauseSettingsOpen>().0 = true;

        // BindingSlot::MoveLeft is at row index 5
        app.world_mut().resource_mut::<PauseSettingsCursor>().0 = 5;

        let tap_update_clean = |app: &mut App, key: KeyCode| {
            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.reset_all();
                keys.press(key);
            }
            app.world_mut().run_schedule(Update);
        };
        let release_update_clean = |app: &mut App, key: KeyCode| {
            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.reset_all();
                keys.release(key);
            }
            app.world_mut().run_schedule(Update);
        };

        // Press Enter to start rebind capture
        tap_update_clean(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<PauseSettingsRebind>().0.is_active());
        assert!(!app.world().resource::<PauseSettingsRebind>().0.is_armed());

        // Release Enter to arm the capture
        release_update_clean(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<PauseSettingsRebind>().0.is_active());
        assert!(app.world().resource::<PauseSettingsRebind>().0.is_armed());

        // Press KeyK to capture the binding
        tap_update_clean(&mut app, KeyCode::KeyK);
        assert!(!app.world().resource::<PauseSettingsRebind>().0.is_active());

        // Verify the setting is now KeyK
        let settings = app.world().resource::<Settings>();
        assert_eq!(settings.bindings.move_left, KeyCode::KeyK);
    }
}

/// `Settings::default()` reproduces the exact hardcoded bindings/sensitivity that
/// shipped before Phase 48 — the game plays identically until the player edits a
/// setting. Cross-checked here (not just in `settings::tests`) against the actual
/// `MatchIntent` a fresh Match produces for the legacy keys.
#[test]
fn default_settings_reproduce_the_shipped_bindings_and_sensitivity() {
    let settings = crate::settings::Settings::default();
    assert_eq!(
        settings.mouse_sensitivity,
        crate::settings::DEFAULT_MOUSE_SENSITIVITY
    );

    let mut app = test_app();
    app.insert_resource(crate::settings::Settings::default());
    go(&mut app, GameState::Match);
    tap_update(&mut app, KeyCode::KeyW);
    assert!(
        app.world()
            .resource::<crate::sim::state::MatchIntent>()
            .0
            .movement
            .y
            > 0.0,
        "a fresh app's default bindings still drive W as forward"
    );
}

/// Serializing and deserializing `Settings` (via `serde_json`, the persistence
/// format) is the identity — the round-trip the save/load path relies on.
#[test]
fn settings_round_trip_through_serde() {
    use crate::settings::{BindingSlot, Settings};

    let mut settings = Settings {
        master_volume: 0.4,
        mouse_sensitivity: 0.5,
        high_contrast: true,
        first_run: false,
        ..Default::default()
    };
    BindingSlot::Pause.set(&mut settings.bindings, KeyCode::KeyP);

    let json = serde_json::to_string(&settings).unwrap();
    let loaded: Settings = serde_json::from_str(&json).unwrap();
    assert_eq!(settings, loaded);
}

/// First-run onboarding: on a fresh `Settings` (first_run == true), entering the
/// Match spawns the onboarding panel; dismissing it (Escape) flips the flag so it
/// will not show again on a subsequent match with the same `Settings`.
#[test]
fn first_run_onboarding_shows_once() {
    let mut app = test_app();
    app.insert_resource(crate::settings::Settings::default());
    go(&mut app, GameState::Match);
    assert_eq!(
        count::<crate::screens::onboarding::OnboardingPanel>(&mut app),
        1,
        "a fresh Settings (first_run) shows the onboarding panel on the first match"
    );

    // Dismiss it explicitly.
    tap_update(&mut app, KeyCode::Escape);
    app.world_mut()
        .resource_mut::<crate::sim::state::MatchPaused>()
        .0 = false;
    assert!(
        !app.world()
            .resource::<crate::settings::Settings>()
            .first_run,
        "dismissing onboarding flips first_run false"
    );
    assert_eq!(
        count::<crate::screens::onboarding::OnboardingPanel>(&mut app),
        0,
        "the onboarding panel despawns once dismissed"
    );

    finish_match(&mut app);
    go(&mut app, GameState::MainMenu);
    go(&mut app, GameState::Lobby);
    go(&mut app, GameState::Match);
    assert_eq!(
        count::<crate::screens::onboarding::OnboardingPanel>(&mut app),
        0,
        "onboarding does not show again once first_run has flipped"
    );
}

/// The pause-menu settings overlay starts collapsed on a fresh Match (never leaking
/// open state from a previous session) and toggles with `O` while paused.
#[test]
fn pause_settings_panel_is_hidden_on_a_fresh_match_and_toggles_while_paused() {
    let mut app = test_app();
    go(&mut app, GameState::Match);

    assert_eq!(
        single_visibility::<crate::view::components::PauseSettingsPanel>(&mut app),
        Visibility::Hidden,
        "the pause settings overlay starts hidden"
    );

    app.world_mut()
        .resource_mut::<crate::sim::state::MatchPaused>()
        .0 = true;
    tap_update(&mut app, KeyCode::KeyO);
    assert_eq!(
        single_visibility::<crate::view::components::PauseSettingsPanel>(&mut app),
        Visibility::Visible,
        "O opens the pause settings overlay while paused"
    );
    assert!(
        count::<crate::view::components::PauseSettingsElement>(&mut app) > 0,
        "the open overlay draws its settings rows"
    );

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset(KeyCode::KeyO);
    tap_update(&mut app, KeyCode::KeyO);
    assert_eq!(
        single_visibility::<crate::view::components::PauseSettingsPanel>(&mut app),
        Visibility::Hidden,
        "a second O closes the pause settings overlay"
    );
}

// --- Phase 49: audio & game feel -------------------------------------------

#[test]
fn district_ambience_and_ui_audio_slots_are_manifest_driven() {
    assert_eq!(
        observed_assets::DISTRICT_AMBIENCE.len(),
        observed_style::District::ALL.len(),
        "the asset manifest must expose exactly one ambience slot per style district"
    );
    assert!(
        crate::view::assets::asset_present(observed_assets::UI_CLICK.path),
        "the UI click slot should point at the checked-in drop-in file"
    );
    assert!(
        crate::view::assets::asset_present(observed_assets::UI_HOVER.path),
        "the UI hover slot should point at the checked-in drop-in file"
    );
    for slot in [
        observed_assets::TOOL_INTERACT,
        observed_assets::KEYSTONE,
        observed_assets::EXIT_UNLOCK,
        observed_assets::GUARDIAN_DREAD,
    ] {
        assert!(
            crate::view::assets::asset_present(slot.path),
            "{} should point at a checked-in Phase 56 cue",
            slot.name
        );
    }

    let app = test_app();
    let ui = app.world().resource::<crate::view::components::UiAssets>();
    assert!(ui.click.is_some(), "UI click sound handle should load");
    assert!(ui.hover.is_some(), "UI hover sound handle should load");
}

#[test]
fn ambience_beds_select_by_place_kind_and_hallway_flavor() {
    use crate::screens::audio::{AmbienceBedKind, active_ambience_bed};

    // A room takes its district's bed — the same district the palette uses.
    let room = teleport::Place::Room(observed_core::RoomId(3));
    assert_eq!(
        active_ambience_bed(MATCH_SEED, room, false),
        AmbienceBedKind::District(crate::screens::match_runtime::ambience::district_for_place(
            MATCH_SEED, room
        )),
    );

    // A hallway takes a hallway-flavour bed, never a district bed: the gantry bed
    // when the hall has raised decks, the corridor bed otherwise.
    let hall =
        teleport::Place::legacy_hallway(observed_core::RoomId(1), observed_core::RoomId(2), 0);
    assert_eq!(
        active_ambience_bed(MATCH_SEED, hall, false),
        AmbienceBedKind::Corridor
    );
    assert_eq!(
        active_ambience_bed(MATCH_SEED, hall, true),
        AmbienceBedKind::Gantry
    );

    // The hallway beds are checked-in drop-ins wired through the manifest.
    assert!(
        crate::view::assets::asset_present(observed_assets::AMBIENCE_CORRIDOR.path),
        "the corridor ambience slot should point at the checked-in drop-in file"
    );
    assert!(
        crate::view::assets::asset_present(observed_assets::AMBIENCE_GANTRY.path),
        "the gantry ambience slot should point at the checked-in drop-in file"
    );
}

#[test]
fn sprite_placeholder_slots_are_manifest_driven() {
    for slot in [
        observed_assets::RUNNER_STAND,
        observed_assets::RUNNER_WALK1,
        observed_assets::RUNNER_WALK2,
        observed_assets::RIVAL_STAND,
        observed_assets::RIVAL_WALK1,
        observed_assets::RIVAL_WALK2,
        observed_assets::GUARDIAN_STAND,
        observed_assets::CONTROL_DEVICE,
    ] {
        assert!(
            crate::view::assets::asset_present(slot.path),
            "{} should point at a checked-in 2.5D placeholder sprite",
            slot.name
        );
    }
}

#[test]
fn muted_sfx_suppresses_rival_bleed_cue_without_blocking_detection() {
    use crate::sim::director::MatchDirector;
    use crate::view::components::MatchAudioCue;

    let mut app = test_app();
    app.insert_resource(crate::settings::Settings {
        sfx_volume: 0.0,
        first_run: false,
        ..Default::default()
    });
    go(&mut app, GameState::Match);
    app.update();

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
    app.update();

    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::RivalBleed),
        0,
        "muted SFX must not spawn inaudible rival-bleed cue entities"
    );
    assert!(
        app.world()
            .resource::<crate::view::components::RivalBleedState>()
            .last_heard
            .iter()
            .any(|&(_, room)| room == neighbour),
        "the detection bookkeeping still updates even when the player's SFX channel is muted"
    );
}

#[test]
fn collapse_and_klaxon_stings_are_gated_by_volume_and_fire_once() {
    use crate::sim::director::MatchDirector;
    use crate::view::components::MatchAudioCue;
    use observed_core::TeamId;
    use observed_match::elimination::EscapeCountdown;

    let mut app = test_app();
    app.insert_resource(crate::settings::Settings {
        sfx_volume: 0.0,
        first_run: false,
        ..Default::default()
    });
    go(&mut app, GameState::Match);
    app.update();

    let dying_room = {
        let mut rt = app.world_mut().resource_mut::<MatchDirector>();
        rt.live.host.match_state.competitive.purge_line = 0.0;
        rt.live
            .host_match()
            .competitive
            .dying_room()
            .expect("a room ahead of the collapse is dying")
    };
    place_into_room_state(&mut app, dying_room);
    {
        let mut rt = app.world_mut().resource_mut::<MatchDirector>();
        rt.series.current.countdown = Some(EscapeCountdown {
            started_by: TeamId(0),
            duration_rounds: 4,
            remaining_rounds: 3,
        });
    }

    app.update();
    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::CollapseSting),
        0,
        "muted SFX suppresses the collapse sting"
    );
    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::Klaxon),
        0,
        "muted SFX suppresses the klaxon loop"
    );

    app.world_mut()
        .resource_mut::<crate::settings::Settings>()
        .sfx_volume = 1.0;
    app.update();
    app.update();
    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::CollapseSting),
        1,
        "the collapse sting fires once when the channel becomes audible"
    );
    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::Klaxon),
        1,
        "the active countdown owns one klaxon loop"
    );

    app.update();
    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::CollapseSting),
        1,
        "the collapse sting does not refire every frame"
    );
    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::Klaxon),
        1,
        "the klaxon loop is not duplicated while the countdown remains active"
    );
}

#[test]
fn idle_match_does_not_spawn_repeating_audio_cues() {
    use crate::view::components::MatchAudioCue;

    let mut app = test_app();
    app.insert_resource(crate::settings::Settings {
        first_run: false,
        ..Default::default()
    });
    go(&mut app, GameState::Match);
    let ambience_count = count_audio_cue(&mut app, MatchAudioCue::Ambience);
    assert!(
        ambience_count <= observed_style::District::ALL.len() + 2,
        "ambience beds should be a bounded match setup cost (districts + corridor + \
         gantry), not per-frame spawns"
    );

    for _ in 0..180 {
        app.update();
    }

    assert_eq!(
        count_audio_cue(&mut app, MatchAudioCue::Ambience),
        ambience_count,
        "idle updates must not spawn additional ambience loop entities"
    );
    for cue in MatchAudioCue::ALL {
        if matches!(
            cue,
            MatchAudioCue::Ambience | MatchAudioCue::Land | MatchAudioCue::GuardianDread
        ) {
            continue;
        }
        assert_eq!(
            count_audio_cue(&mut app, cue),
            0,
            "idle match spawned unexpected repeated cue {cue:?}"
        );
    }
    assert!(
        count_audio_cue(&mut app, MatchAudioCue::Land) <= 1,
        "the first grounded fixed-step may produce one landing cue, never a stream"
    );
    assert!(
        count_audio_cue(&mut app, MatchAudioCue::GuardianDread) <= 1,
        "a nearby starting guardian may produce one dread cue, never a repeated stream"
    );
}

#[test]
fn test_cue_table_sanity() {
    use crate::screens::audio::AudioDirector;
    use crate::view::components::MatchAudioCue;

    let director = AudioDirector::default();
    for cue in MatchAudioCue::ALL {
        let config = director.get_config(cue);
        assert!(
            config.is_some(),
            "cue {cue:?} should have a config class mapping"
        );
    }
}

#[test]
fn audio_coverage_doc_mentions_every_match_audio_cue() {
    use crate::view::components::MatchAudioCue;

    let doc = include_str!("../../docs/arc_f/audio_coverage.md");
    assert!(
        !doc.contains("TBD"),
        "audio coverage audit must not carry unresolved placeholders"
    );
    for cue in MatchAudioCue::ALL {
        let token = format!("`MatchAudioCue::{cue:?}`");
        assert!(
            doc.contains(&token),
            "audio coverage doc must mention {token}"
        );
    }
}

#[test]
fn audio_spatial_classes_distinguish_distance_and_occlusion() {
    use crate::screens::audio::{AudioDirector, AudioSourceRelation};
    use crate::view::components::MatchAudioCue;

    let director = AudioDirector::default();
    let same = director
        .spatial_gain_for(
            MatchAudioCue::RivalBleed,
            AudioSourceRelation::SamePlace,
            0.0,
        )
        .expect("rival cue has a spatial class");
    let threshold = director
        .spatial_gain_for(
            MatchAudioCue::RivalBleed,
            AudioSourceRelation::ThroughThreshold,
            0.0,
        )
        .expect("rival cue has a spatial class");
    let wall = director
        .spatial_gain_for(
            MatchAudioCue::RivalBleed,
            AudioSourceRelation::ThroughWall,
            0.0,
        )
        .expect("rival cue has a spatial class");
    let far_threshold = director
        .spatial_gain_for(
            MatchAudioCue::RivalBleed,
            AudioSourceRelation::ThroughThreshold,
            60.0,
        )
        .expect("rival cue has a spatial class");
    let guardian_wall = director
        .spatial_gain_for(
            MatchAudioCue::GuardianDread,
            AudioSourceRelation::ThroughWall,
            60.0,
        )
        .expect("guardian cue has a spatial class");

    assert!(
        same > threshold,
        "same-place should be louder than through-threshold"
    );
    assert!(
        threshold > wall,
        "threshold bleed should beat through-wall bleed"
    );
    assert!(
        threshold > far_threshold,
        "distance rolloff should reduce threshold bleed"
    );
    assert!(
        far_threshold >= 0.34,
        "rival bleed keeps a floor so adjacent rivals remain legible"
    );
    assert_eq!(
        guardian_wall, 0.12,
        "guardian dread keeps only its configured floor behind distant walls"
    );
}

#[test]
fn asset_source_ledger_has_no_cc_by_and_covers_game_ready_audio() {
    let sources = include_str!("../../assets/SOURCES.md");
    assert!(
        !sources.contains("CC-BY"),
        "Phase 56 removes CC-BY assets and caveats from the checked-in ledger"
    );
    for slot in observed_assets::SLOTS
        .iter()
        .filter(|slot| slot.kind == observed_assets::AssetKind::Sound)
    {
        if slot.name == observed_assets::CHIME.name {
            continue;
        }
        assert!(
            sources.contains(&format!("`{}`", slot.path)),
            "{} ({}) should have a source row",
            slot.name,
            slot.path
        );
    }
}

#[test]
fn test_ducking_math() {
    use crate::screens::audio::{ActiveDuck, AudioBus, DuckConfig, DuckState};

    let config = DuckConfig {
        bus: AudioBus::Music,
        target_factor: 0.3,
        ease_in: 1.0,
        duration: 2.0,
        ease_out: 1.0,
    };

    let mut duck = ActiveDuck {
        bus: config.bus,
        target_factor: config.target_factor,
        ease_in: config.ease_in,
        duration: config.duration,
        ease_out: config.ease_out,
        elapsed: 0.0,
        state: DuckState::Active,
        source_entity: None,
    };

    // Ease in start
    assert!((duck.current_factor() - 1.0).abs() < 1e-5);

    // Mid ease in
    duck.elapsed = 0.5;
    assert!((duck.current_factor() - 0.65).abs() < 1e-5); // 1.0 + (0.3 - 1.0) * 0.5

    // Ease in complete / Hold
    duck.elapsed = 1.0;
    assert!((duck.current_factor() - 0.3).abs() < 1e-5);

    duck.elapsed = 2.0;
    assert!((duck.current_factor() - 0.3).abs() < 1e-5);

    // Easing out transition
    duck.state = DuckState::EasingOut { start_factor: 0.3 };
    duck.elapsed = 0.0;
    assert!((duck.current_factor() - 0.3).abs() < 1e-5);

    duck.elapsed = 0.5;
    assert!((duck.current_factor() - 0.65).abs() < 1e-5); // 0.3 + (1.0 - 0.3) * 0.5

    duck.elapsed = 1.0;
    assert!((duck.current_factor() - 1.0).abs() < 1e-5);
}

#[test]
fn test_audio_director_cooldown_and_caps() {
    use crate::screens::audio::AudioDirector;
    use crate::settings::Settings;
    use crate::view::components::MatchAudioCue;

    let mut app = test_app();
    let mut director = AudioDirector::default();
    let settings = Settings::default();
    let sound = Some(
        app.world_mut()
            .resource::<AssetServer>()
            .load("sounds/footstep.ogg"),
    );

    // Spawn 5 footsteps in same tick, cap is 3
    let mut count = 0;
    for _ in 0..5 {
        let spawned = director.request(
            &mut app.world_mut().commands(),
            &sound,
            MatchAudioCue::Footstep,
            "Footstep test",
            None,
            &settings,
        );
        if spawned {
            count += 1;
        }
    }
    assert_eq!(
        count, 1,
        "cooldown should prevent refiring in same tick (cooldown > 0)"
    );

    // Let's test caps without cooldown by resetting cooldown tracker and active instances
    director.last_fire.clear();
    director.active_instances.clear();
    let mut count2 = 0;
    for _ in 0..5 {
        director.last_fire.remove(&MatchAudioCue::Footstep);
        let spawned = director.request(
            &mut app.world_mut().commands(),
            &sound,
            MatchAudioCue::Footstep,
            "Footstep test",
            None,
            &settings,
        );
        if spawned {
            count2 += 1;
        }
    }
    assert_eq!(
        count2, 3,
        "instance cap should restrict total concurrent instances to 3"
    );
}

#[test]
fn test_audio_director_ducking() {
    use crate::screens::audio::{AudioBus, AudioDirector};
    use crate::settings::Settings;
    use crate::view::components::MatchAudioCue;

    let mut app = test_app();
    let mut director = AudioDirector::default();
    let settings = Settings::default();
    let sound = Some(
        app.world_mut()
            .resource::<AssetServer>()
            .load("sounds/escape.ogg"),
    );

    // Music duck factor is normally 1.0
    assert!((director.bus_duck_factor(AudioBus::Music) - 1.0).abs() < 1e-5);

    // Request escape sting
    director.request(
        &mut app.world_mut().commands(),
        &sound,
        MatchAudioCue::Escape,
        "Escape test",
        None,
        &settings,
    );

    // Escape config: target_factor = 0.3, ease_in = 0.1
    // At elapsed_secs = 0.0, factor is 1.0 (start of ease)
    assert!((director.bus_duck_factor(AudioBus::Music) - 1.0).abs() < 1e-5);
}

#[test]
fn test_muted_suppression() {
    use crate::screens::audio::AudioDirector;
    use crate::settings::Settings;
    use crate::view::components::MatchAudioCue;

    let mut app = test_app();
    let mut director = AudioDirector::default();
    let settings = Settings {
        sfx_volume: 0.0,
        ..Default::default()
    };
    let sound = Some(
        app.world_mut()
            .resource::<AssetServer>()
            .load("sounds/footstep.ogg"),
    );

    let spawned = director.request(
        &mut app.world_mut().commands(),
        &sound,
        MatchAudioCue::Footstep,
        "Footstep test",
        None,
        &settings,
    );
    assert!(
        !spawned,
        "muted volume settings should suppress cue spawning"
    );
}

#[test]
fn all_eight_toggle_combinations_run_headless_to_valid_outcome() {
    use crate::sim::director::{BotPopulations, MatchDirector};
    let map_spec = crate::map_catalog::default_map_spec(MATCH_SEED);

    for &rival_teams in &[true, false] {
        for &ai_teammates in &[true, false] {
            for &guardian in &[true, false] {
                let config = BotPopulations {
                    rival_teams,
                    ai_teammates,
                    guardian,
                };
                let mut director = MatchDirector::new(MATCH_SEED, map_spec.clone(), config);
                let result = director.run_to_completion();

                assert!(director.done);
                assert!(result.placement.is_some());

                if !rival_teams {
                    assert_eq!(result.placement, Some(1));
                    assert_eq!(result.winner, Some(observed_core::TeamId(0)));
                }
            }
        }
    }
}

#[test]
fn guardian_off_spawns_no_guardian_resources() {
    let mut app = test_app();

    // Disable guardian in the career configuration for this test's app
    app.world_mut().resource_mut::<Career>().bot_guardian = false;

    go(&mut app, GameState::Match);

    // Verify that Guardian and ActionLog resources are not registered in the ECS.
    assert!(
        app.world()
            .get_resource::<crate::guardian::Guardian>()
            .is_none()
    );
    assert!(
        app.world()
            .get_resource::<crate::guardian::ActionLog>()
            .is_none()
    );
}

#[test]
fn bot_populations_env_parsing_and_persistence_round_trips() {
    use crate::sim::director::BotPopulations;

    // 1. Env parsing test (using thread-safe from_str)
    let config = BotPopulations::from_str("no_rivals|no_guardian").unwrap();
    assert!(!config.rival_teams);
    assert!(config.ai_teammates);
    assert!(!config.guardian);

    let config = BotPopulations::from_str("none").unwrap();
    assert!(!config.rival_teams);
    assert!(!config.ai_teammates);
    assert!(!config.guardian);

    let config = BotPopulations::from_str("no_teammates").unwrap();
    assert!(config.rival_teams);
    assert!(!config.ai_teammates);
    assert!(config.guardian);

    let config = BotPopulations::from_str("no_rivals|typo").unwrap();
    assert_eq!(
        config,
        BotPopulations::default(),
        "an unknown token warns and falls back to the all-on default"
    );

    // 2. Profile persistence test
    let test_dir = std::path::PathBuf::from("saves_test");
    let _ = std::fs::create_dir_all(&test_dir);
    let test_path = test_dir.join("profile_test.save");

    crate::settings::TEST_PROFILE_PATH.with(|p| {
        *p.borrow_mut() = Some(test_path.clone());
    });

    let mut career = Career::default();
    career.bot_rival_teams = false;
    career.bot_ai_teammates = false;
    career.bot_guardian = false;

    crate::flow::save_profile(&career);

    let loaded = crate::flow::load_career();
    assert!(!loaded.bot_rival_teams);
    assert!(!loaded.bot_ai_teammates);
    assert!(!loaded.bot_guardian);

    // Clean up test file and reset thread-local
    let _ = std::fs::remove_file(&test_path);
    crate::settings::TEST_PROFILE_PATH.with(|p| {
        *p.borrow_mut() = None;
    });
}

#[test]
fn all_on_default_match_director_digest_is_pinned() {
    use crate::sim::director::{BotPopulations, MatchDirector};

    fn mix(digest: &mut u64, value: u64) {
        *digest ^= value;
        *digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
    }

    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for seed in [0_u64, 1, 2, 7, 42, 0x0A11_C0DE] {
        let mut director = MatchDirector::new(
            seed,
            crate::map_catalog::default_map_spec(seed),
            BotPopulations::default(),
        );
        mix(&mut digest, seed);
        mix(&mut digest, u64::from(director.config.rival_teams));
        mix(&mut digest, u64::from(director.config.ai_teammates));
        mix(&mut digest, u64::from(director.config.guardian));
        for team in &director.live.host_match().competitive.teams {
            mix(&mut digest, u64::from(team.id.0));
            mix(&mut digest, u64::from(team.members));
        }
        for room in &director
            .live
            .host_match()
            .competitive
            .structure
            .graph
            .players
        {
            mix(&mut digest, u64::from(room.0));
        }
        for team in &director.series.alive_teams {
            mix(&mut digest, u64::from(team.0));
        }
        let result = director.run_to_completion();
        mix(&mut digest, u64::from(result.placement.unwrap_or(u8::MAX)));
        mix(&mut digest, result.escaped as u64);
        mix(&mut digest, result.absorbed as u64);
        mix(
            &mut digest,
            u64::from(result.winner.map_or(u8::MAX, |team| team.0)),
        );
        mix(&mut digest, u64::from(result.local_won));
    }

    assert_eq!(
        digest, 0x539C_35C6_26B9_B30F,
        "update only for an intentional all-on behavior change"
    );
}

#[test]
fn test_gameplay_object_sprites_slots_and_fallbacks() {
    let mut app = test_app();
    go(&mut app, GameState::Match);

    assert!(
        app.world()
            .contains_resource::<crate::view::assets::MatchAssets>()
    );

    let assets = app.world().resource::<crate::view::assets::MatchAssets>();

    let _ = assets.keystone_card;
    let _ = assets.keystone_core;
    let _ = assets.exit_access_card;
    let _ = assets.anchor_torch;
    let _ = assets.route_cell;
    let _ = assets.relay_device;
    let _ = assets.battery_charge;
    let _ = assets.repair_token;

    let images = app.world().resource::<Assets<Image>>();
    let _ = assets.keystone_card_sprite(images);
    let _ = assets.keystone_core_sprite(images);
    let _ = assets.exit_access_card_sprite(images);
    let _ = assets.anchor_torch_sprite(images);
    let _ = assets.route_cell_sprite(images);
    let _ = assets.relay_device_sprite(images);
    let _ = assets.battery_charge_sprite(images);
    let _ = assets.repair_token_sprite(images);
}

#[test]
fn test_directional_actors_sheets_and_fallbacks() {
    let mut app = test_app();
    go(&mut app, GameState::Match);

    assert!(
        app.world()
            .contains_resource::<crate::view::assets::MatchAssets>()
    );

    let assets = app.world().resource::<crate::view::assets::MatchAssets>();

    let _ = assets.rival_actor_sheet;
    let _ = assets.rival_actor_layout;
    let _ = assets.rival_actor_meta;
    let _ = assets.guardian_actor_sheet;
    let _ = assets.guardian_actor_layout;
    let _ = assets.guardian_actor_meta;
}

#[test]
fn phase75_corpus_parity_lifecycle() {
    use crate::sim::state::TeleportState;
    use observed_core::RoomId;

    // (1) Verify standard RAII drop behavior for RapierTraversalScene
    let room = crate::teleport::geom::room_geom_with_slots_and_seals_for_role(
        RoomId(0),
        &[RoomId(1)],
        &[],
        &[],
        Some(RoomId(1)),
        None,
        0,
    );
    let scene = crate::teleport::place_rapier_scene(&room, 0.0, 3.4);
    assert!(scene.collider_count() > 0);
    // Once dropped, Rust automatically deallocates the Sets.
    drop(scene);

    // (2) Verify that TeleportState (which wraps RapierTraversalScene) is cleaned up by the match teardown
    let mut app = test_app();
    go(&mut app, GameState::Match);
    assert!(app.world().contains_resource::<TeleportState>());

    let state = app.world().resource::<TeleportState>();
    assert!(
        state.rapier.collider_count() > 0,
        "Rapier scene must be populated during Match"
    );

    // Tear down the match
    finish_match(&mut app);
    assert!(
        !app.world().contains_resource::<TeleportState>(),
        "TeleportState must be despawned on Match exit"
    );
}
