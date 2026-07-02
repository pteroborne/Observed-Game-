pub(crate) mod ambience;
pub(crate) mod crossing;
pub(crate) mod input;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use observed_core::RoomId;
use observed_facility::map_spec::{RoomRole, sector_relay_v1};
use observed_match::elimination::{EliminationSeries, MAX_AUTOPLAY_TICKS};
use observed_match::hybrid::{HybridMatch, LocalAction};
use observed_match::teamplay::TeamplayMatch;
use observed_net::netmatch::LiveNetMatch;
use observed_net::network::NetworkProfile;
use observed_style::{self as style, MarkerRole};
use observed_traversal::{FpsBody, FpsConfig};
use player_input::PlayerIntent;

use super::input::{gamepad_pause_pressed, gamepad_quit_pressed};
use crate::layout::WALL_HEIGHT;
use crate::sim::state::{
    ItemIntent, LastTeleportPad, MatchIntent, MatchPaused, MatchRuntime, SeriesRuntime,
    SpectatorBot, TeleportState,
};
use crate::view::assets::{MatchAssets, all_planned_assets_present};
use crate::view::components::{
    DecohereFx, KeystoneItem, MatchAudioState, MatchHud, PausePanel, TacMapPanel, TacMapState,
    TeleportAnimation, TeleportOverlay,
};
use crate::view::theme::{BORDER, PANEL, TAC_MAP_SIZE, TITLE, screen_root, text};

use crate::GameState;
use crate::flow::{Career, LOCAL_TEAM, MATCH_SEED, resolve};
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::teleport::Place;

const SPECTATOR_BOT_WAYPOINT_RADIUS: f32 = 0.9;
const SPECTATOR_BOT_CROSS_RADIUS: f32 = 1.2;
pub(crate) const SPECTATOR_TEAMPLAY_STEP_FRAMES: u8 = 30;

// Re-exports
pub(crate) use ambience::district_for_place;
pub(crate) use crossing::{
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
    spectator_bot: Option<ResMut<SpectatorBot>>,
) {
    career.begin_match();
    if !all_planned_assets_present() {
        warn!("one or more planned match assets are absent; procedural fallbacks will be used");
    }
    let seed_val = seed.map(|s| s.0).unwrap_or(MATCH_SEED);
    if let Some(mut spectator) = spectator_bot
        && spectator.seed != seed_val
    {
        *spectator = SpectatorBot::for_seed(seed_val);
    }
    let map_spec = sector_relay_v1();
    let live = LiveNetMatch::new_for_map_spec(seed_val, NetworkProfile::Hostile, map_spec.clone());
    let game = live.host_match();
    let initial_escaped = game.competitive.escaped_count();
    let initial_commits = game.reroute_commits;
    let keys = KeystoneState::for_map(seed_val, &map_spec);
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
    commands.insert_resource(SeriesRuntime {
        series: EliminationSeries::new(seed_val),
        autoplay_timer: Timer::from_seconds(0.45, TimerMode::Repeating),
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
    let guardian_room = map_spec
        .role_room(RoomRole::GuardianControl)
        .unwrap_or(RoomId(8));
    commands.insert_resource(crate::guardian::Guardian {
        room: guardian_room,
        ..default()
    });
    commands.insert_resource(crate::guardian::ActionLog::default());
    commands.insert_resource(TeleportAnimation::default());
    commands.insert_resource(LastTeleportPad::default());

    commands.insert_resource(MatchAssets::load(
        &asset_server,
        &mut meshes,
        &mut materials,
    ));

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
                    text("mystery corridors", 13.0, Color::srgb(1.0, 0.32, 0.22)),
                ],
            ));
        });
}

pub(crate) fn cleanup_match_resources(mut commands: Commands) {
    commands.remove_resource::<MatchRuntime>();
    commands.remove_resource::<SeriesRuntime>();
    commands.remove_resource::<SpectatorBot>();
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn drive_spectator_bot(
    paused: Res<MatchPaused>,
    mut spectator: ResMut<SpectatorBot>,
    mut runtime: ResMut<MatchRuntime>,
    mut series_runtime: ResMut<SeriesRuntime>,
    mut tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    guardian: Res<crate::guardian::Guardian>,
    mut intent: ResMut<MatchIntent>,
    mut item_intent: ResMut<ItemIntent>,
) {
    spectator.teamplay_frame_accum = spectator.teamplay_frame_accum.saturating_add(1);
    if spectator.teamplay_frame_accum >= SPECTATOR_TEAMPLAY_STEP_FRAMES {
        spectator.teamplay_frame_accum = 0;
        pump_spectator_teamplay(&mut spectator, &mut series_runtime.series);
    }
    if paused.0 || runtime.done {
        intent.0 = PlayerIntent::default();
        return;
    }
    if spectator.finished {
        intent.0 = PlayerIntent::default();
        return;
    }

    let here = body_xz(&tp);
    let in_same_room = matches!(tp.place, Place::Room(room) if room == guardian.room);
    if in_same_room && items.carried(ItemKind::AnchorTorch) > 0 {
        item_intent.torch_action = true;
    }

    if matches!(tp.place, Place::Room(room) if room.0 == observed_match::mutable::EXIT_ROOM) {
        spectator.finished = true;
        spectator.clear_route();
        intent.0 = PlayerIntent::default();
        return;
    }

    if let Some(gap) = crate::bot::target_gap_for_place(tp.place, &tp.geom, here) {
        let rel = here - gap.center;
        let tangent = Vec2::new(-gap.normal.y, gap.normal.x);
        let at_aperture =
            rel.dot(gap.normal) > -0.45 && rel.dot(tangent).abs() <= gap.width * 0.5 + 0.35;
        if here.distance(gap.center) <= SPECTATOR_BOT_CROSS_RADIUS || at_aperture {
            debug_cross_gap_for_capture(&mut tp, &mut runtime, gap, &keys, &items);
            spectator.clear_route();
            intent.0 = PlayerIntent::default();
            return;
        }
    }

    if spectator.route_place != Some(tp.place)
        || spectator.waypoint >= spectator.route.len()
        || spectator.route.is_empty()
    {
        let Some(gap) = crate::bot::target_gap_for_place(tp.place, &tp.geom, here) else {
            spectator.blocked_ticks += 1;
            spectator.finished =
                runtime.live.host_match().local_target().is_none() || spectator.blocked_ticks > 90;
            intent.0 = PlayerIntent::default();
            return;
        };
        if let Some(path) = crate::bot::route_to_gap(&tp.geom, &tp.arena, &tp.config, here, &gap) {
            spectator.route_place = Some(tp.place);
            spectator.route = path.waypoints;
            spectator.waypoint = 0;
            spectator.blocked_ticks = 0;
        } else {
            spectator.blocked_ticks += 1;
            if spectator.blocked_ticks > 90 {
                spectator.finished = true;
            }
            intent.0 = PlayerIntent::default();
            return;
        }
    }

    while spectator.waypoint + 1 < spectator.route.len()
        && here.distance(spectator.route[spectator.waypoint]) <= SPECTATOR_BOT_WAYPOINT_RADIUS
    {
        spectator.waypoint += 1;
    }

    let target = spectator.route[spectator.waypoint];
    let to = target - here;
    if to.length_squared() < 0.04 {
        intent.0 = PlayerIntent::default();
        return;
    }

    let mut avoidance = Vec2::ZERO;
    let safety_dist = tp.config.radius + 0.05;
    let cy = tp.body.position.y;
    let hy = tp.config.half_height;
    for solid in &tp.arena.solids {
        if cy - hy < solid.max.y && cy + hy > solid.min.y {
            let closest_x = here.x.clamp(solid.min.x, solid.max.x);
            let closest_z = here.y.clamp(solid.min.z, solid.max.z);
            let closest = Vec2::new(closest_x, closest_z);
            let diff = here - closest;
            let dist = diff.length();
            if dist > 0.0 && dist < safety_dist {
                let weight = (safety_dist - dist) / safety_dist;
                avoidance += diff.normalize() * weight * 1.8;
            }
        }
    }

    let mut dir = to.normalize_or_zero();
    if avoidance.length_squared() > 1e-4 {
        dir = (dir + avoidance).normalize_or_zero();
    }
    let forward_dir = Vec2::new(tp.body.forward().x, tp.body.forward().z).normalize_or_zero();
    let is_sharp_turn = forward_dir.dot(dir) < 0.65;
    tp.body.yaw = dir.x.atan2(-dir.y);
    tp.body.pitch = -0.22;

    intent.0 = PlayerIntent {
        movement: Vec2::new(0.0, 1.0),
        sprint_held: !is_sharp_turn,
        ..default()
    };
}

fn pump_spectator_teamplay(spectator: &mut SpectatorBot, series: &mut EliminationSeries) {
    if series.finished() {
        spectator.finished = true;
        return;
    }

    if !spectator.teamplay.finished {
        let events = spectator.teamplay.advance_bot_tick();
        if let Some(event) = events.last() {
            spectator.last_teamplay_event = event.summary();
        }
    }

    let Some(outcome) = spectator.teamplay.round_outcome() else {
        return;
    };

    series.apply_teamplay_round(outcome);
    if series.finished() {
        spectator.finished = true;
        spectator.last_teamplay_event = series.last_event.clone();
        return;
    }

    let next_seed = spectator
        .seed
        .wrapping_add(u64::from(series.current.index).wrapping_mul(0xA11_C0D3));
    spectator.teamplay = TeamplayMatch::for_round(
        next_seed,
        series.current.index,
        series.alive_teams.clone(),
        series.adversary_strength(),
    );
    spectator.teamplay_frame_accum = 0;
    spectator.focused_team = series
        .alive_teams
        .iter()
        .copied()
        .find(|team| *team == LOCAL_TEAM)
        .unwrap_or_else(|| series.alive_teams[0]);
    spectator.focused_member = 0;
    spectator.last_teamplay_event = series.last_event.clone();
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
        exit_room: keys.exit_room,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn match_pump(
    time: Res<Time>,
    input: MatchPumpInput,
    mut runtime: ResMut<MatchRuntime>,
    mut series_runtime: ResMut<SeriesRuntime>,
    spectator_bot: Option<Res<SpectatorBot>>,
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

    let spectator_driven_series = spectator_bot.is_some();

    if !spectator_driven_series
        && !series_runtime.series.finished()
        && series_runtime
            .autoplay_timer
            .tick(time.delta())
            .just_finished()
    {
        series_runtime.series.advance_autoplay_tick();
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
    if !spectator_driven_series && runtime.live.finished() && !series_runtime.series.finished() {
        series_runtime.series.run_to_winner(MAX_AUTOPLAY_TICKS);
    }

    if (runtime.live.finished() || series_runtime.series.finished()) && !runtime.done {
        for _ in 0..64 {
            if runtime.live.in_sync() {
                break;
            }
            runtime.live.pump();
        }
        runtime.done = true;
        let result = if series_runtime.series.finished() {
            crate::flow::resolve_series(&series_runtime.series)
        } else {
            resolve(&runtime.live.host_match().competitive)
        };
        career.record(result);
        next.set(GameState::Results);
    }
}

pub(crate) fn keystone_pickup(
    tp: Res<TeleportState>,
    mut keys: ResMut<KeystoneState>,
    mut series: ResMut<SeriesRuntime>,
    items: Query<(Entity, &KeystoneItem, &GlobalTransform)>,
    mut commands: Commands,
) {
    const PICKUP_RADIUS: f32 = 2.2;
    let body = Vec2::new(tp.body.position.x, tp.body.position.z);
    for (entity, item, transform) in &items {
        let here = Vec2::new(transform.translation().x, transform.translation().z);
        if body.distance(here) <= PICKUP_RADIUS && keys.collect(item.0) {
            if let Some(team) = series.series.current.team_mut(LOCAL_TEAM)
                && !team.collected_keystones.contains(&item.0)
            {
                team.collected_keystones.push(item.0);
                team.collected_keystones.sort_by_key(|room| room.0);
            }
            commands.entity(entity).despawn();
        }
    }
}
