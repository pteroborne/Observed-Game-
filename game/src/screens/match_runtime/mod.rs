pub(crate) mod ambience;
pub(crate) mod input;
pub(crate) mod teleport;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use observed_core::RoomId;
use observed_match::hybrid::{HybridMatch, LocalAction};
use observed_match::maze::TILE_SIZE;
use observed_net::netmatch::LiveNetMatch;
use observed_net::network::NetworkProfile;
use observed_style::{self as style, MarkerRole, SurfaceRole};
use observed_traversal::{FpsBody, FpsConfig};

use super::*;
use crate::GameState;
use crate::flow::{Career, MATCH_SEED, resolve};
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::teleport::Place;

// Re-exports
pub(crate) use ambience::{
    apply_match_atmosphere, apply_place_atmosphere, clear_match_atmosphere, district_for_place,
    flicker_lights, sync_decohere_fx,
};
pub(crate) use input::{grab_match_cursor, release_match_cursor};
pub(crate) use teleport::{
    compute_gap_dests, debug_cross_gap_for_capture, debug_place_into, place_body_at, teleport_sim,
};

#[derive(SystemParam)]
pub(crate) struct MatchPumpInput<'w, 's> {
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    gamepads: Query<'w, 's, &'static Gamepad>,
}

// --- match (first-person 3D, networked) ------------------------------------
pub(crate) fn setup_match(
    mut commands: Commands,
    mut career: ResMut<Career>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
) {
    career.begin_match();
    if !all_planned_assets_present() {
        warn!("one or more planned match assets are absent; procedural fallbacks will be used");
    }
    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);
    let live = LiveNetMatch::new(seed_val, NetworkProfile::Hostile);
    let game = live.host_match();
    let initial_escaped = game.competitive.escaped_count();
    let initial_commits = game.reroute_commits;
    let keys = KeystoneState::new(seed_val);
    let items = ItemsState::single_player();
    let tp_config = FpsConfig::default();
    let start_place = Place::Room(game.local_room());
    let start_geom =
        crate::teleport::geom_for(start_place, &nav_from_brain(seed_val, game, &keys, &items));
    let start_arena = crate::teleport::place_arena(&start_geom, 0.0, WALL_HEIGHT);
    let start_gap_dests =
        compute_gap_dests(seed_val, start_place, &start_geom, game, &keys, &items);
    let spawn = Vec3::new(0.0, tp_config.half_height, 0.0);
    commands.insert_resource(MatchRuntime {
        live,
        wait_timer: Timer::from_seconds(0.45, TimerMode::Repeating),
        done: false,
    });
    commands.insert_resource(MatchPaused(false));
    commands.insert_resource(TacMapState(false));
    commands.insert_resource(MatchIntent::default());
    commands.insert_resource(ItemIntent::default());
    commands.insert_resource(DecohereFx {
        last_commits: initial_commits,
        flash: 0.0,
    });
    commands.insert_resource(MatchAudioState {
        last_position: spawn,
        stride_distance: 0.0,
        last_place: start_place,
        escaped_count: initial_escaped,
    });
    commands.insert_resource(TeleportState {
        place: start_place,
        body: FpsBody::spawned(spawn, 0.0),
        config: tp_config,
        arena: start_arena,
        geom: start_geom,
        prev_xz: Vec2::ZERO,
        crossed_exit: false,
        pending_exit: None,
        arrived_from: None,
        gap_dests: start_gap_dests,
        rendered: None,
    });
    commands.insert_resource(keys);
    commands.insert_resource(items);
    commands.insert_resource(crate::guardian::Guardian::default());
    commands.insert_resource(crate::guardian::ActionLog::default());
    commands.insert_resource(TeleportAnimation::default());
    commands.insert_resource(LastTeleportPad::default());

    let load_texture =
        |path: &'static str| asset_present(path).then(|| asset_server.load::<Image>(path));
    let wall_texture = load_texture(WALL_TEX);
    let floor_texture = load_texture(FLOOR_TEX);
    let ceiling_texture = load_texture(CEILING_TEX);

    let floor_material = materials.add(textured_neon_material(
        &style::surface(SurfaceRole::Plain),
        floor_texture.clone(),
    ));
    let spine_floor_material = materials.add(textured_neon_material(
        &style::surface(SurfaceRole::Spine),
        floor_texture.clone(),
    ));
    let safe_floor_material = materials.add(textured_neon_material(
        &style::surface(SurfaceRole::SafeBypass),
        floor_texture.clone(),
    ));
    let trap_active_material = materials.add(textured_neon_material(
        &style::surface(SurfaceRole::TrapArmed),
        floor_texture.clone(),
    ));
    let trap_idle_material = materials.add(textured_neon_material(
        &style::surface(SurfaceRole::TrapIdle),
        floor_texture,
    ));
    let wall_material = materials.add(textured_neon_material(
        &style::surface(SurfaceRole::Wall),
        wall_texture,
    ));
    let ceiling_material = materials.add(StandardMaterial {
        cull_mode: None,
        double_sided: true,
        ..textured_neon_material(&style::surface(SurfaceRole::Ceiling), ceiling_texture)
    });
    let exit_panel_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: asset_present(EXIT_PANEL_TEX)
            .then(|| asset_server.load(EXIT_PANEL_TEX)),
        emissive: LinearRgba::rgb(0.08, 5.0, 0.35),
        unlit: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let fixture_glow_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.9, 1.0),
        emissive: LinearRgba::rgb(4.0, 7.0, 10.0),
        unlit: true,
        ..default()
    });
    let lamp_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.85, 0.72),
        emissive: LinearRgba::rgb(3.2, 2.7, 1.8),
        unlit: true,
        ..default()
    });
    let district_accent_materials = std::array::from_fn(|i| {
        let accent = style::district(style::District::ALL[i]).accent;
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.03, 0.05),
            emissive: LinearRgba::rgb(accent.red * 10.0, accent.green * 10.0, accent.blue * 10.0),
            unlit: true,
            ..default()
        })
    });
    let placeholder_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.10, 0.14),
        emissive: LinearRgba::rgb(0.10, 0.30, 0.45),
        perceptual_roughness: 0.7,
        ..default()
    });
    let doorframe_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.07, 0.11),
        emissive: LinearRgba::rgb(0.35, 1.9, 2.5),
        perceptual_roughness: 0.5,
        ..default()
    });
    let spine_doorframe_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.08, 0.03),
        emissive: LinearRgba::rgb(2.6, 1.7, 0.5),
        perceptual_roughness: 0.5,
        ..default()
    });
    let door_leaf_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.06, 0.10),
        emissive: LinearRgba::rgb(0.10, 0.32, 0.5),
        perceptual_roughness: 0.55,
        ..default()
    });
    let objective = style::marker(MarkerRole::NextRoom);
    let objective_material = materials.add(StandardMaterial {
        base_color: objective.base_color,
        emissive: objective.emissive,
        unlit: true,
        ..default()
    });
    let rival = style::marker(MarkerRole::Rival);
    let rival_material = materials.add(StandardMaterial {
        base_color: rival.base_color,
        emissive: rival.emissive,
        perceptual_roughness: 0.6,
        ..default()
    });
    let anchor = style::marker(MarkerRole::Control);
    let anchor_torch_material = materials.add(StandardMaterial {
        base_color: anchor.base_color,
        emissive: anchor.emissive,
        unlit: true,
        ..default()
    });
    let pad = style::marker(MarkerRole::You);
    let teleport_pad_material = materials.add(StandardMaterial {
        base_color: pad.base_color,
        emissive: pad.emissive,
        unlit: true,
        ..default()
    });
    let team_materials = TEAM_COLORS.map(|color| {
        materials.add(StandardMaterial {
            base_color: color.with_alpha(0.58),
            emissive: color.to_linear() * 1.5,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })
    });
    let load_scene = |path: &'static str| {
        asset_present(path).then(|| asset_server.load(GltfAssetLabel::Scene(0).from_asset(path)))
    };
    let load_sound =
        |path: &'static str| asset_present(path).then(|| asset_server.load::<AudioSource>(path));

    commands.insert_resource(MatchAssets {
        floor_mesh: meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE)),
        wall_mesh: meshes.add(Cuboid::new(TILE_SIZE, WALL_HEIGHT, TILE_SIZE)),
        ceiling_mesh: meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE)),
        panel_mesh: meshes.add(Rectangle::new(4.4, 2.2)),
        placeholder_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        halo_mesh: meshes.add(Cylinder::new(0.46, 0.025)),
        door_post_mesh: meshes.add(Cuboid::new(DOOR_POST_W, WALL_HEIGHT, DOOR_POST_D)),
        door_lintel_mesh: meshes.add(Cuboid::new(HALL_WIDTH, DOOR_LINTEL_H, DOOR_POST_D)),
        door_leaf_mesh: meshes.add(Cuboid::new(
            HALL_WIDTH - 2.0 * DOOR_POST_W,
            WALL_HEIGHT - DOOR_LINTEL_H,
            DOOR_LEAF_D,
        )),
        objective_beam_mesh: meshes.add(Cylinder::new(0.16, 9.0)),
        rival_body_mesh: meshes.add(Capsule3d::new(0.32, 1.0)),
        floor_material,
        spine_floor_material,
        safe_floor_material,
        trap_active_material,
        trap_idle_material,
        wall_material,
        ceiling_material,
        exit_panel_material,
        fixture_glow_material,
        lamp_material,
        district_accent_materials,
        placeholder_material,
        doorframe_material,
        spine_doorframe_material,
        door_leaf_material,
        objective_material,
        rival_material,
        anchor_torch_material,
        teleport_pad_material,
        team_materials,
        light_fixture: load_scene(LIGHT_FIXTURE_MODEL),
        exit_gate: load_scene(EXIT_GATE_MODEL),
        player: load_scene(PLAYER_MODEL),
        bot: load_scene(BOT_MODEL),
        equipment: load_scene(EQUIPMENT_MODEL),
        hazard: load_scene(HAZARD_MODEL),
        footstep: load_sound(FOOTSTEP_SOUND),
        reroute: load_sound(REROUTE_SOUND),
        escape: load_sound(ESCAPE_SOUND),
        ambience: load_sound(AMBIENCE_SOUND),
        door: load_sound(DOOR_SOUND),
    });

    commands
        .spawn(screen_root(GameState::Match))
        .with_children(|root| {
            root.spawn((
                MatchHud,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(16),
                    left: px(16),
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
                Text::new("Match starting…"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TITLE),
            ));
            root.spawn((
                TeleportOverlay,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0),
                    left: px(0),
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                PausePanel,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0),
                    left: px(0),
                    width: percent(100),
                    height: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                children![(
                    Text::new("PAUSED\n\nEsc / Start  Resume\nQ / Y        Quit to menu"),
                    TextFont {
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(TITLE),
                )],
            ));
            root.spawn((
                TacMapPanel,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(16),
                    right: px(16),
                    width: px(TAC_MAP_SIZE),
                    height: px(TAC_MAP_SIZE),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
                children![(
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(6),
                        left: px(10),
                        ..default()
                    },
                    Text::new("TAC-MAP"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(TITLE),
                )],
            ));
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(16),
                    left: px(16),
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(1)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(3),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
                children![
                    text("LEGEND", 15.0, TITLE),
                    text("exit", 13.0, style::marker(MarkerRole::Exit).base_color),
                    text("keystone — pick up", 13.0, Color::srgb(1.0, 0.82, 0.3)),
                    text(
                        "anchor torch - F drop/pick",
                        13.0,
                        style::marker(MarkerRole::Control).base_color
                    ),
                    text(
                        "teleport pad - C drop/pick, E link",
                        13.0,
                        style::marker(MarkerRole::You).base_color
                    ),
                    text("locked exit (red door)", 13.0, Color::srgb(1.0, 0.32, 0.22)),
                    text(
                        "collapse — threat",
                        13.0,
                        style::marker(MarkerRole::Collapse).base_color
                    ),
                    text(
                        "rival teams",
                        13.0,
                        style::marker(MarkerRole::Rival).base_color
                    ),
                    text(
                        "pressure gate (red when lit)",
                        13.0,
                        Color::srgb(1.0, 0.32, 0.22)
                    ),
                ],
            ));
        });
}

pub(crate) fn cleanup_match_resources(mut commands: Commands) {
    commands.remove_resource::<MatchRuntime>();
    commands.remove_resource::<MatchIntent>();
    commands.remove_resource::<ItemIntent>();
    commands.remove_resource::<MatchPaused>();
    commands.remove_resource::<TacMapState>();
    commands.remove_resource::<MatchAssets>();
    commands.remove_resource::<MatchAudioState>();
    commands.remove_resource::<TeleportState>();
    commands.remove_resource::<KeystoneState>();
    commands.remove_resource::<ItemsState>();
    commands.remove_resource::<DecohereFx>();
}

pub(crate) fn connections_for(game: &HybridMatch, room: RoomId) -> Vec<RoomId> {
    let mut connections: Vec<RoomId> = game
        .rendered
        .iter()
        .filter_map(|route| {
            if route.rooms.0 == room {
                Some(route.rooms.1)
            } else if route.rooms.1 == room {
                Some(route.rooms.0)
            } else {
                None
            }
        })
        .collect();
    connections.sort_unstable_by_key(|r| r.0);
    connections.dedup();
    connections
}

pub(crate) fn connections_for_nav(
    game: &HybridMatch,
    items: &ItemsState,
    room: RoomId,
) -> Vec<RoomId> {
    if let Some(connections) = items.locked_room_connections(room) {
        return connections;
    }
    let mut connections: Vec<RoomId> = connections_for(game, room)
        .into_iter()
        .filter(|&other| items.relation_allowed_by_room_locks(room, other))
        .collect();
    connections.extend(
        items
            .pinned_connections(room)
            .into_iter()
            .filter(|&other| items.relation_allowed_by_room_locks(room, other)),
    );
    connections.sort_by_key(|room| room.0);
    connections.dedup();
    connections
}

fn rendered_slot_for(
    game: &HybridMatch,
    room: RoomId,
    target: RoomId,
) -> Option<crate::teleport::ThresholdSlotId> {
    game.rendered
        .iter()
        .find(|route| {
            (route.rooms.0 == room && route.rooms.1 == target)
                || (route.rooms.0 == target && route.rooms.1 == room)
        })
        .and_then(|route| {
            [route.key.0, route.key.1]
                .into_iter()
                .find(|door| (door.0 as u32 / 4) == room.0)
                .map(|door| crate::teleport::ThresholdSlotId((door.0 % 4) as u8))
        })
}

pub(crate) fn slot_for_connection(
    game: &HybridMatch,
    items: &ItemsState,
    room: RoomId,
    target: RoomId,
) -> Option<crate::teleport::ThresholdSlotId> {
    rendered_slot_for(game, room, target).or_else(|| {
        connections_for_nav(game, items, room)
            .into_iter()
            .position(|candidate| candidate == target)
            .map(|slot| crate::teleport::ThresholdSlotId(slot as u8))
    })
}

pub(crate) fn room_connection_slots(
    game: &HybridMatch,
    items: &ItemsState,
    room: RoomId,
    connections: &[RoomId],
) -> Vec<crate::teleport::RoomConnectionSlot> {
    connections
        .iter()
        .enumerate()
        .map(|(fallback, &target)| crate::teleport::RoomConnectionSlot {
            target,
            slot: slot_for_connection(game, items, room, target)
                .unwrap_or(crate::teleport::ThresholdSlotId(fallback as u8)),
        })
        .collect()
}

pub(crate) fn room_target(
    game: &HybridMatch,
    room: RoomId,
    connections: &[RoomId],
) -> Option<RoomId> {
    if room == game.local_room() {
        return game.local_target();
    }
    if connections.contains(&game.local_room()) {
        Some(game.local_room())
    } else {
        connections.first().copied()
    }
}

pub(crate) fn nav_for_room(
    seed: u64,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
    room: RoomId,
) -> crate::teleport::Nav {
    let connections = connections_for_nav(game, items, room);
    let connection_slots = room_connection_slots(game, items, room, &connections);
    let target_room = room_target(game, room, &connections);
    crate::teleport::Nav {
        connections,
        connection_slots,
        hallway_entry_room_slot: None,
        hallway_exit_room_slot: None,
        target_room,
        seed,
        version: game.reroute_commits,
        exit_locked: !keys.gate_open(),
        pins: items.pins(),
    }
}

pub(crate) fn nav_from_brain(
    seed: u64,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
) -> crate::teleport::Nav {
    nav_for_room(seed, game, keys, items, game.local_room())
}

pub(crate) fn nav_for_place(
    seed: u64,
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
    place: Place,
) -> crate::teleport::Nav {
    match place {
        Place::Room(room) => nav_for_room(seed, game, keys, items, room),
        Place::Hallway { from, to, .. } => {
            let mut nav = nav_for_room(seed, game, keys, items, from);
            nav.hallway_entry_room_slot = slot_for_connection(game, items, from, to);
            nav.hallway_exit_room_slot = slot_for_connection(game, items, to, from);
            nav
        }
    }
}

const ITEM_INTERACT_RADIUS: f32 = 1.8;
const PAD_ACTIVATE_RADIUS: f32 = 1.25;

fn body_xz(tp: &TeleportState) -> Vec2 {
    Vec2::new(tp.body.position.x, tp.body.position.z)
}

fn pickup_or_drop_item(
    items: &mut ItemsState,
    kind: ItemKind,
    place: Place,
    pos: Vec2,
    version: u32,
) -> bool {
    if items.pickup(kind, place, pos, ITEM_INTERACT_RADIUS) {
        true
    } else {
        items.drop(kind, place, pos, version)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn item_actions(
    runtime: Res<MatchRuntime>,
    keys: Res<KeystoneState>,
    mut tp: ResMut<TeleportState>,
    mut items: ResMut<ItemsState>,
    mut item_intent: ResMut<ItemIntent>,
    paused: Res<MatchPaused>,
    mut anim: ResMut<TeleportAnimation>,
    mut last_pad: ResMut<LastTeleportPad>,
    mut log: ResMut<crate::guardian::ActionLog>,
    seed: Option<Res<crate::flow::ActiveMatchSeed>>,
) {
    let intent = std::mem::take(&mut *item_intent);
    if paused.0 || runtime.done {
        return;
    }

    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);

    let pos = body_xz(&tp);
    let place = tp.place;
    let version = runtime.live.host_match().reroute_commits;
    let mut changed = false;

    if intent.torch_action {
        changed |= if items.pickup(ItemKind::AnchorTorch, place, pos, ITEM_INTERACT_RADIUS) {
            true
        } else {
            let mut connections = match place {
                Place::Room(_) => tp.geom.gaps.iter().map(|gap| gap.target).collect(),
                Place::Hallway { .. } => Vec::new(),
            };
            connections.sort_by_key(|room| room.0);
            connections.dedup();
            items.drop_anchor_torch(place, pos, version, &connections)
        };
    }
    if intent.pad_action {
        changed |= pickup_or_drop_item(&mut items, ItemKind::TeleportPad, place, pos, version);
    }

    let on_pad_link = items.pad_link_target(place, pos, PAD_ACTIVATE_RADIUS);
    let is_latched = last_pad
        .last_used_pos
        .is_some_and(|(last_place, last_pos)| {
            crate::items::same_place(place, last_place)
                && pos.distance(last_pos) <= PAD_ACTIVATE_RADIUS + 0.3
        });

    if !is_latched {
        if let Some((last_place, last_pos)) = last_pad.last_used_pos
            && (!crate::items::same_place(place, last_place)
                || pos.distance(last_pos) > PAD_ACTIVATE_RADIUS + 0.3)
        {
            last_pad.last_used_pos = None;
        }

        if let Some((target_place, target_pos)) = on_pad_link {
            let nav = nav_for_place(
                seed_val,
                runtime.live.host_match(),
                &keys,
                &items,
                target_place,
            );
            place_body_at(&mut tp, target_place, target_pos, &nav);
            let dests = compute_gap_dests(
                seed_val,
                tp.place,
                &tp.geom,
                runtime.live.host_match(),
                &keys,
                &items,
            );
            tp.gap_dests = dests;
            changed = true;
            last_pad.last_used_pos = Some((target_place, target_pos));
            anim.trigger(2.0, Color::srgba(0.0, 0.8, 1.0, 1.0));
            if let Place::Room(room) = target_place {
                log.add(format!("Teleported via pad to Room {}!", room.0));
            }
        }
    }

    if changed {
        let nav = nav_for_place(seed_val, runtime.live.host_match(), &keys, &items, tp.place);
        let mut geom = crate::teleport::geom_for(tp.place, &nav);
        if matches!(tp.place, Place::Room(_)) {
            crate::teleport::open_entry(&mut geom, tp.arrived_from);
        }
        tp.arena = crate::teleport::place_arena(&geom, 0.0, WALL_HEIGHT);
        if geom.poly.is_some() {
            let clamped = crate::teleport::contain(&geom, body_xz(&tp), tp.config.radius);
            tp.body.position.x = clamped.x;
            tp.body.position.z = clamped.y;
        }
        tp.geom = geom;
        tp.gap_dests = compute_gap_dests(
            seed_val,
            tp.place,
            &tp.geom,
            runtime.live.host_match(),
            &keys,
            &items,
        );
        tp.rendered = None;
    }
}

pub(crate) fn match_pump(
    time: Res<Time>,
    input: MatchPumpInput,
    mut runtime: ResMut<MatchRuntime>,
    mut paused: ResMut<MatchPaused>,
    mut career: ResMut<Career>,
    mut next: ResMut<NextState<GameState>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if input.keyboard.just_pressed(KeyCode::Escape) || gamepad_pause_pressed(&input.gamepads) {
        paused.0 = !paused.0;
        input::set_cursor_grab(&mut cursors, !paused.0);
    }
    if paused.0 {
        if input.keyboard.just_pressed(KeyCode::KeyQ) || gamepad_quit_pressed(&input.gamepads) {
            next.set(GameState::MainMenu);
        }
        return;
    }
    if runtime.done {
        return;
    }

    for _ in 0..3 {
        runtime.live.pump();
    }
    if !runtime.live.finished()
        && !runtime.live.local_active()
        && runtime.wait_timer.tick(time.delta()).just_finished()
    {
        runtime.live.force_round(LocalAction::Wait);
    }
    if runtime.live.finished() && !runtime.done {
        for _ in 0..64 {
            if runtime.live.in_sync() {
                break;
            }
            runtime.live.pump();
        }
        runtime.done = true;
        let result = resolve(&runtime.live.host_match().competitive);
        career.record(result);
        next.set(GameState::Results);
    }
}

pub(crate) fn keystone_pickup(
    tp: Res<TeleportState>,
    mut keys: ResMut<KeystoneState>,
    items: Query<(Entity, &KeystoneItem, &GlobalTransform)>,
    mut commands: Commands,
) {
    const PICKUP_RADIUS: f32 = 2.2;
    let body = Vec2::new(tp.body.position.x, tp.body.position.z);
    for (entity, item, transform) in &items {
        let here = Vec2::new(transform.translation().x, transform.translation().z);
        if body.distance(here) <= PICKUP_RADIUS && keys.collect(item.0) {
            commands.entity(entity).despawn();
        }
    }
}
