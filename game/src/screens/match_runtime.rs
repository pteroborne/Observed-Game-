//! The match lifecycle and the fixed-step teleport controller: build the live
//! networked match + its assets and HUD on entering ([`setup_match`]), apply/clear the
//! neon-noir atmosphere and cursor grab, step the body and commit spine rounds
//! ([`teleport_sim`]), pump the lockstep transport and resolve the result
//! ([`match_pump`]), and collect keystones ([`keystone_pickup`]).

use bevy::ecs::system::SystemParam;
use bevy::gltf::GltfAssetLabel;
use bevy::input::gamepad::Gamepad;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use observed_core::RoomId;
use observed_match::hybrid::{HybridMatch, LocalAction};
use observed_match::maze::TILE_SIZE;
use observed_net::netmatch::LiveNetMatch;
use observed_net::network::NetworkProfile;
use observed_style::{self as style, MarkerRole, SurfaceRole};
use observed_traversal::{FIXED_DT, FpsBody, FpsConfig, step_body};

use super::*;
use crate::GameState;
use crate::flow::{Career, MATCH_SEED, resolve};
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::teleport::{self, GapKind, Place};

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
) {
    career.begin_match();
    if !all_planned_assets_present() {
        warn!("one or more planned match assets are absent; procedural fallbacks will be used");
    }
    let live = LiveNetMatch::new(MATCH_SEED, NetworkProfile::Hostile);
    let game = live.host_match();
    let initial_escaped = game.competitive.escaped_count();
    let initial_commits = game.reroute_commits;
    // Keystone-gated exit: items to find before the exit unlocks.
    let keys = KeystoneState::new(MATCH_SEED);
    // Single-player droppable items: one anchor torch + two teleport pads.
    let items = ItemsState::single_player();
    // Teleport place state: start in the local team's room, body at its centre.
    let tp_config = FpsConfig::default();
    let start_place = Place::Room(game.local_room());
    let start_geom = teleport::geom_for(start_place, &nav_from_brain(game, &keys, &items));
    let start_arena = teleport::place_arena(&start_geom, 0.0, WALL_HEIGHT);
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
    // Seed the decohere feedback with the live commit count so it doesn't flash on entry.
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
        rendered: None,
    });
    commands.insert_resource(keys);
    commands.insert_resource(items);

    // Surfaces are driven by the shared neon-noir visual language (`observed_style`),
    // not ad-hoc colours or drop-in textures — code-as-art the match can't drift from.
    let floor_material = materials.add(neon_material(&style::surface(SurfaceRole::Plain)));
    // The protected spine glows warm gold so the objective path reads at a glance.
    let spine_floor_material = materials.add(neon_material(&style::surface(SurfaceRole::Spine)));
    let safe_floor_material =
        materials.add(neon_material(&style::surface(SurfaceRole::SafeBypass)));
    let trap_active_material =
        materials.add(neon_material(&style::surface(SurfaceRole::TrapArmed)));
    let trap_idle_material = materials.add(neon_material(&style::surface(SurfaceRole::TrapIdle)));
    let wall_material = materials.add(neon_material(&style::surface(SurfaceRole::Wall)));
    let ceiling_material = materials.add(StandardMaterial {
        // The ceiling is seen from above in the tac-map too, so keep it double-sided.
        cull_mode: None,
        double_sided: true,
        ..neon_material(&style::surface(SurfaceRole::Ceiling))
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
    // Missing GLB models fall back to a quiet steel-blue block (not glaring magenta),
    // so an absent asset reads as "a prop belongs here", not a rendering bug.
    let placeholder_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.10, 0.14),
        emissive: LinearRgba::rgb(0.10, 0.30, 0.45),
        perceptual_roughness: 0.7,
        ..default()
    });
    // Neon doorway frames mark a passage so you can find it; the leaf is a dark blast
    // panel that hides what's beyond until it slides open.
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
    // The objective beacon: a gold beam over your next room, from the shared marker
    // palette so "gold = go here" means the same thing everywhere.
    let objective = style::marker(MarkerRole::NextRoom);
    let objective_material = materials.add(StandardMaterial {
        base_color: objective.base_color,
        emissive: objective.emissive,
        unlit: true,
        ..default()
    });
    // Rival avatars take the shared "rival" marker treatment, so they read as the same
    // colour the legend documents for rival teams. Emissive (but lit) so they glow as a
    // figure under the neon-noir bloom without washing out.
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
                RouteShiftPanel,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0),
                    left: px(0),
                    width: percent(100),
                    height: percent(100),
                    border: UiRect::all(px(10)),
                    align_items: AlignItems::FlexEnd,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::bottom(px(72)),
                    ..default()
                },
                BorderColor::all(Color::srgba(1.0, 0.18, 0.82, 0.85)),
                BackgroundColor(Color::srgba(0.45, 0.02, 0.34, 0.10)),
                children![(
                    Text::new("ROUTE SHIFT — PASSAGES CHANGED BEHIND YOU"),
                    TextFont {
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.70, 0.94)),
                )],
            ));
            // The tac-map overlay panel (top-right). Hidden until Tab; its room/route/
            // marker children are (re)built each frame by `draw_tac_map` while shown.
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
            // Legend: each line's colour is its on-screen colour, so nothing is an
            // unexplained marker (drawn from the shared style palette).
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

/// Give the Match its neon-noir atmosphere: HDR + bloom so the emissive visual
/// language glows, distance fog over the dark maze, and a low ambient so the neon
/// (not flat fill light) does the talking. The camera and ambient are shared with
/// the menus, so this is applied on entering the Match and removed on exit.
pub(crate) fn apply_match_atmosphere(mut commands: Commands, camera: Query<Entity, With<GameCam>>) {
    if let Ok(camera) = camera.single() {
        commands.entity(camera).insert((
            Hdr,
            // Gentler than NATURAL (0.15): the visual language uses hot HDR emission
            // (an armed trap is 9.0), which at full bloom clips a near surface to a
            // featureless wash — the opposite of the Legibility Contract.
            Bloom {
                intensity: 0.08,
                ..Bloom::NATURAL
            },
            DistanceFog {
                color: Color::srgb(0.01, 0.015, 0.03),
                falloff: FogFalloff::Linear {
                    start: 16.0,
                    end: 72.0,
                },
                ..default()
            },
        ));
    }
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.35, 0.42, 0.6),
        brightness: 110.0,
        ..default()
    });
}

/// Undo [`apply_match_atmosphere`] so the menus keep their bright, fog-free look.
pub(crate) fn clear_match_atmosphere(mut commands: Commands, camera: Query<Entity, With<GameCam>>) {
    if let Ok(camera) = camera.single() {
        commands
            .entity(camera)
            .remove::<Hdr>()
            .remove::<Bloom>()
            .remove::<DistanceFog>();
    }
    // Restore the generous menu ambient set at startup in `setup_camera`.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.74, 0.85),
        brightness: 900.0,
        ..default()
    });
}

fn set_cursor_grab(cursors: &mut Query<&mut CursorOptions, With<PrimaryWindow>>, grab: bool) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = if grab {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
        cursor.visible = !grab;
    }
}

pub(crate) fn grab_match_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    set_cursor_grab(&mut cursors, true);
}

pub(crate) fn release_match_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    set_cursor_grab(&mut cursors, false);
}

/// The rooms connected to `room` in the current rendered graph (its open doorways'
/// partners), deduped and ordered. Used to shape a room's footprint — both the room
/// you're in and a room previewed beyond a hallway doorway.
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

fn room_target(game: &HybridMatch, room: RoomId, connections: &[RoomId]) -> Option<RoomId> {
    if room == game.local_room() {
        return game.local_target();
    }
    if connections.contains(&game.local_room()) {
        Some(game.local_room())
    } else {
        connections.first().copied()
    }
}

/// Build the navigation snapshot for a specific room: that room's rendered
/// connections, the doorway that should remain passable, the live decohere version,
/// the exit lock, and anchor-torch pins.
pub(crate) fn nav_for_room(
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
    room: RoomId,
) -> teleport::Nav {
    let connections = connections_for(game, room);
    let target_room = room_target(game, room, &connections);
    teleport::Nav {
        connections,
        target_room,
        seed: MATCH_SEED,
        version: game.reroute_commits,
        // The exit door stays locked until the player holds the required keystones.
        exit_locked: !keys.gate_open(),
        pins: items.pins(|room| connections_for(game, room)),
    }
}

/// Build the brain's navigation snapshot for the local team's current match room.
pub(crate) fn nav_from_brain(
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
) -> teleport::Nav {
    nav_for_room(game, keys, items, game.local_room())
}

/// Build the snapshot for the currently rendered place. Rooms use their own
/// connection set; hallway geometry uses the room it was entered from.
pub(crate) fn nav_for_place(
    game: &HybridMatch,
    keys: &KeystoneState,
    items: &ItemsState,
    place: Place,
) -> teleport::Nav {
    let room = match place {
        Place::Room(room) => room,
        Place::Hallway { from, .. } => from,
    };
    nav_for_room(game, keys, items, room)
}

/// Move the body into `place`, having arrived from room `from`: rebuild the collision
/// arena and spawn the body just inside the doorway it entered through, facing in.
fn place_body(tp: &mut TeleportState, place: Place, from: RoomId, nav: &teleport::Nav) {
    let geom = teleport::geom_for(place, nav);
    let spawn = teleport::entry_spawn(&geom, from);
    let yaw = geom
        .gaps
        .iter()
        .find(|g| g.target == from)
        .map(|g| (-g.normal.x).atan2(g.normal.y))
        .unwrap_or(0.0);
    tp.arena = teleport::place_arena(&geom, 0.0, WALL_HEIGHT);
    tp.geom = geom;
    tp.body = FpsBody::spawned(Vec3::new(spawn.x, tp.config.half_height, spawn.y), yaw);
    tp.place = place;
    tp.prev_xz = spawn;
    tp.crossed_exit = false;
}

/// Move the body directly to a point in `place` without committing a match round.
/// Teleport pads use this: they are local traversal tools, not deterministic match
/// actions replicated through the lockstep brain.
fn place_body_at(tp: &mut TeleportState, place: Place, pos: Vec2, nav: &teleport::Nav) {
    let geom = teleport::geom_for(place, nav);
    let yaw = tp.body.yaw;
    let pitch = tp.body.pitch;
    tp.arena = teleport::place_arena(&geom, 0.0, WALL_HEIGHT);
    tp.geom = geom;
    tp.body = FpsBody::spawned(Vec3::new(pos.x, tp.config.half_height, pos.y), yaw);
    tp.body.pitch = pitch;
    tp.place = place;
    tp.prev_xz = pos;
    tp.crossed_exit = false;
    tp.rendered = None;
}

/// Capture/diagnostic helper: drop the player straight into `place` (rebuilding the
/// arena + geometry as if they had teleported in from `from`), without any physical
/// crossing. Used by the maze evidence capture in `crate::capture`.
pub(crate) fn debug_place_into(
    tp: &mut TeleportState,
    runtime: &MatchRuntime,
    place: Place,
    from: RoomId,
    keys: &KeystoneState,
    items: &ItemsState,
) {
    let nav = nav_for_place(runtime.live.host_match(), keys, items, place);
    place_body(tp, place, from, &nav);
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

/// Apply local droppable-tool actions sampled by [`match_input`]. These tools are
/// deliberately presentation-local: anchor torches influence the rendered navigation
/// snapshot, and teleport pads move the local body, but neither writes a deterministic
/// match action into the lockstep brain.
pub(crate) fn item_actions(
    runtime: Res<MatchRuntime>,
    keys: Res<KeystoneState>,
    mut tp: ResMut<TeleportState>,
    mut items: ResMut<ItemsState>,
    mut item_intent: ResMut<ItemIntent>,
    paused: Res<MatchPaused>,
) {
    let intent = std::mem::take(&mut *item_intent);
    if paused.0 || runtime.done {
        return;
    }

    let pos = body_xz(&tp);
    let place = tp.place;
    let version = runtime.live.host_match().reroute_commits;
    let mut changed = false;

    if intent.torch_action {
        changed |= pickup_or_drop_item(&mut items, ItemKind::AnchorTorch, place, pos, version);
    }
    if intent.pad_action {
        changed |= pickup_or_drop_item(&mut items, ItemKind::TeleportPad, place, pos, version);
    }
    if intent.activate_pad
        && let Some((target_place, target_pos)) =
            items.pad_link_target(place, pos, PAD_ACTIVATE_RADIUS)
    {
        let nav = nav_for_place(runtime.live.host_match(), &keys, &items, target_place);
        place_body_at(&mut tp, target_place, target_pos, &nav);
        changed = true;
    }

    if changed {
        tp.rendered = None;
    }
}

/// Fixed-step teleport controller: walk the body inside the current place; crossing
/// the forward doorway teleports into the edge's hallway, and reaching the hallway's
/// exit commits the spine `Advance` to the match brain and teleports into the next
/// room. The brain (rounds / networking / replay) is untouched.
pub(crate) fn teleport_sim(
    mut runtime: ResMut<MatchRuntime>,
    tp: ResMut<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    mut intent: ResMut<MatchIntent>,
    paused: Res<MatchPaused>,
) {
    if paused.0 || runtime.done {
        return;
    }
    let tp = tp.into_inner();
    let nav = nav_for_place(runtime.live.host_match(), &keys, &items, tp.place);
    let prev = Vec2::new(tp.body.position.x, tp.body.position.z);
    let config = tp.config;
    let arena = tp.arena.clone();
    step_body(&mut tp.body, intent.0, &arena, &config, FIXED_DT);
    intent.0.interact_pressed = false;
    // A polygon room's angled walls can't be AABB solids, so the controller moves freely
    // and we clamp the body back into the convex room afterward (open at the doorways).
    if tp.geom.poly.is_some() {
        let here = Vec2::new(tp.body.position.x, tp.body.position.z);
        let clamped = teleport::contain(&tp.geom, here, config.radius);
        tp.body.position.x = clamped.x;
        tp.body.position.z = clamped.y;
    }
    let next = Vec2::new(tp.body.position.x, tp.body.position.z);
    tp.prev_xz = next;

    // Crossing tests read the cached place geometry (so a labyrinth isn't regenerated
    // every step); copy out the gaps we need before any teleport replaces it.
    match tp.place {
        Place::Room(room) => {
            if let Some(gap) = tp.geom.forward_gap().copied()
                && teleport::crossed(prev, next, &gap)
            {
                let (place, _) = teleport::apply_crossing(Place::Room(room), &gap, &nav);
                place_body(tp, place, room, &nav);
            }
        }
        Place::Hallway { from, to, .. } => {
            let exit_crossed = tp
                .geom
                .gaps
                .iter()
                .filter(|g| g.kind == GapKind::Exit)
                .any(|g| teleport::crossed(prev, next, g));
            if !tp.crossed_exit && exit_crossed {
                tp.crossed_exit = true;
            }
            if tp.crossed_exit {
                // Commit the spine round only when this is the local team's current
                // protected edge. Pad/backtrack-created traversal remains local.
                let should_commit = {
                    let game = runtime.live.host_match();
                    game.local_room() == from && game.local_target() == Some(to)
                };
                if should_commit && runtime.live.force_round(LocalAction::Advance) {
                    let game = runtime.live.host_match();
                    let arrived = game.local_room();
                    let nav2 = nav_from_brain(game, &keys, &items);
                    place_body(tp, Place::Room(arrived), from, &nav2);
                } else if !should_commit {
                    let nav2 = nav_for_room(runtime.live.host_match(), &keys, &items, to);
                    place_body(tp, Place::Room(to), from, &nav2);
                }
            } else {
                // Backtracking out the entrance returns to the room you came from (no
                // round committed) — so wandering a maze's dead ends back to the mouth
                // never walks the body into the void behind the open doorway.
                let entry_crossed = tp
                    .geom
                    .gaps
                    .iter()
                    .filter(|g| g.kind == GapKind::Entry)
                    .any(|g| teleport::crossed(prev, next, g));
                if entry_crossed {
                    let nav2 = nav_for_room(runtime.live.host_match(), &keys, &items, from);
                    place_body(tp, Place::Room(from), to, &nav2);
                }
            }
        }
    }
}

/// Pump the lockstep transport, keep the match moving when the local team is out,
/// handle pause/quit, and resolve the result when the match ends.
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
        set_cursor_grab(&mut cursors, !paused.0);
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

    // Replicate resolved rounds to the remote over the hostile transport.
    for _ in 0..3 {
        runtime.live.pump();
    }
    // Keep the match advancing once the local team can no longer run.
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

/// Drive the first-person **decoherence** feedback. When the brain's `reroute_commits`
/// advances — the unobserved structure has rewired behind the player — fire a brief
/// route-shift flash overlay, an audio sting (throttled to once per shift), and slam the
/// current place's doors shut so they re-hide what's beyond. The camera jolt is applied
/// separately in `present_match_camera`, which reads the same `flash` timer. Observed
/// rooms still don't change under the player — this is sensory feedback for a graph
/// rewire that affects edges they haven't reached, not a live change to their room.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_decohere_fx(
    time: Res<Time>,
    runtime: Res<MatchRuntime>,
    paused: Res<MatchPaused>,
    assets: Res<MatchAssets>,
    mut fx: ResMut<DecohereFx>,
    mut panel: Query<&mut Visibility, With<RouteShiftPanel>>,
    mut leaves: Query<(&DoorLeaf, &mut Transform)>,
    mut commands: Commands,
) {
    if paused.0 {
        return;
    }
    let commits = runtime.live.host_match().reroute_commits;
    if commits > fx.last_commits {
        let was_idle = fx.flash <= 0.0;
        fx.flash = ROUTE_SHIFT_FLASH_SECS;
        fx.last_commits = commits;
        // Sting only on a fresh shift after a calm spell, so rapid back-to-back reroutes
        // hold the flash without machine-gunning the audio.
        if was_idle {
            play_one_shot(
                &mut commands,
                &assets.reroute,
                MatchAudioCue::Reroute,
                "Route shift",
            );
        }
        // Slam every leaf shut; `animate_doors` reopens the nearby ones over ~0.4 s.
        for (leaf, mut transform) in &mut leaves {
            transform.translation.y = leaf.closed_y;
        }
    }
    if fx.flash > 0.0 {
        fx.flash = (fx.flash - time.delta_secs()).max(0.0);
    }
    let visibility = if fx.flash > 0.0 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if let Ok(mut vis) = panel.single_mut() {
        *vis = visibility;
    }
}

/// Collect a keystone item when the player walks over it (proximity pickup). The body is
/// in the current place's local frame, and each keystone sits at its room's centre.
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
