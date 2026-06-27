//! Bevy presentation for the networked hybrid match: a top-down map/spectator of
//! the two peers' converged match, plus the live transport statistics and a
//! `[PASS]`/`[FAIL]` health line. The map is exactly the role CLAUDE.md gives the
//! 2D view — the schematic spectator for the first-person match — now driven by the
//! lockstep-replicated state instead of a single local sim.

use bevy::prelude::*;

use observed_core::RoomId;
use observed_match::facility::{EXIT_ROOM, TEAM_COUNT};
use observed_match::maze::{GRID_H, GRID_W, RoomRect, TILE_SIZE, Tile};
use observed_net::network::NetworkProfile;

use crate::netmatch::NetMatch;

const SEED: u64 = 1;
/// Transport ticks advanced per frame while auto-running.
const TICKS_PER_FRAME: u32 = 3;
const MAP_SCALE: f32 = 9.0;

const TEAM_COLORS: [Color; TEAM_COUNT] = [
    Color::srgb(0.96, 0.28, 0.34),
    Color::srgb(0.32, 0.62, 1.0),
    Color::srgb(0.72, 0.46, 1.0),
    Color::srgb(1.0, 0.62, 0.20),
];

#[derive(Component)]
pub struct NetCam;

#[derive(Component)]
pub struct NetUiRoot;

#[derive(Component)]
pub struct DebugText;

#[derive(Resource)]
pub struct NetRuntime {
    pub net: NetMatch,
    pub profile: NetworkProfile,
    pub auto: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
}

impl Default for NetRuntime {
    fn default() -> Self {
        Self {
            net: NetMatch::authored(SEED, NetworkProfile::Hostile),
            profile: NetworkProfile::Hostile,
            auto: true,
            reset_requested: false,
            reset_count: 0,
        }
    }
}

pub fn setup_lab(mut commands: Commands) {
    commands.spawn((Camera2d, NetCam, Name::new("Net Match Camera")));
    commands.spawn((
        NetUiRoot,
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.04, 0.07, 0.86)),
        children![(
            DebugText,
            Text::new("Networked hybrid match"),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.95, 1.0)),
        )],
    ));
}

pub fn handle_input(keyboard: Res<ButtonInput<KeyCode>>, mut runtime: ResMut<NetRuntime>) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        let total = runtime.net.total;
        if runtime.net.peers[0].committed_round < total {
            runtime.net.advance_transport_tick();
        }
    }
    if keyboard.just_pressed(KeyCode::KeyA) {
        runtime.auto = !runtime.auto;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        runtime.profile = match runtime.profile {
            NetworkProfile::Hostile => NetworkProfile::Clean,
            NetworkProfile::Clean => NetworkProfile::Hostile,
        };
        runtime.reset_requested = true;
    }
}

pub fn advance(mut runtime: ResMut<NetRuntime>) {
    if !runtime.auto || runtime.reset_requested {
        return;
    }
    for _ in 0..TICKS_PER_FRAME {
        if runtime.net.synchronized() {
            break;
        }
        runtime.net.advance_transport_tick();
    }
}

pub fn perform_reset(mut runtime: ResMut<NetRuntime>) {
    if !runtime.reset_requested {
        return;
    }
    let profile = runtime.profile;
    runtime.net.reset(profile);
    runtime.reset_requested = false;
    runtime.reset_count += 1;
}

fn tile_screen(x: usize, y: usize) -> Vec2 {
    let wx = (x as f32 - GRID_W as f32 * 0.5 + 0.5) * TILE_SIZE;
    let wy = (y as f32 - GRID_H as f32 * 0.5 + 0.5) * TILE_SIZE;
    Vec2::new(wx * MAP_SCALE, -wy * MAP_SCALE)
}

fn world_screen(wx: f32, wz: f32) -> Vec2 {
    Vec2::new(wx * MAP_SCALE, -wz * MAP_SCALE)
}

fn room_screen(room: RoomId, rooms: &[RoomRect]) -> Vec2 {
    let (x, y) = rooms[room.0 as usize].center_tile();
    tile_screen(x, y)
}

pub fn draw_world(mut gizmos: Gizmos, runtime: Res<NetRuntime>) {
    let peer = &runtime.net.peers[0];
    let session = &peer.match_state;
    let cell = TILE_SIZE * MAP_SCALE;

    // The rerouting maze: floor cells the peers agree on.
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let base = match session.maze_tiles[y * GRID_W + x] {
                Tile::Wall => continue,
                Tile::Room(_) => Color::srgb(0.22, 0.30, 0.40),
                Tile::Corridor if session.trap_tiles.contains(&(x, y)) => {
                    Color::srgb(0.95, 0.16, 0.06)
                }
                Tile::Corridor if session.safe_tiles.contains(&(x, y)) => {
                    Color::srgb(0.12, 0.72, 0.76)
                }
                Tile::Corridor => Color::srgb(0.12, 0.17, 0.24),
            };
            let elevation = session.floor_height(x, y) / (observed_match::maze::LEVEL_HEIGHT * 2.0);
            let color = base.mix(&Color::WHITE, elevation * 0.24);
            gizmos.rect_2d(tile_screen(x, y), Vec2::splat(cell * 0.92), color);
        }
    }

    // Exit and collapse rooms.
    gizmos.circle_2d(
        room_screen(RoomId(EXIT_ROOM), &session.rooms),
        cell * 1.2,
        Color::srgb(0.36, 1.0, 0.58),
    );
    for room in session.competitive.collapse_rooms() {
        gizmos.rect_2d(
            room_screen(room, &session.rooms),
            Vec2::splat(cell * 1.6),
            Color::srgb(0.8, 0.2, 0.22),
        );
    }

    // Team markers at their current rooms.
    for (i, teamcolor) in TEAM_COLORS.iter().enumerate() {
        let room = session.competitive.team_room(i);
        let escaped = session.competitive.teams[i].placement.is_some();
        let color = if escaped {
            teamcolor.mix(&Color::WHITE, 0.55)
        } else {
            *teamcolor
        };
        gizmos.circle_2d(room_screen(room, &session.rooms), cell * 0.7, color);
    }

    // The first-person pose, reconstructed identically by both peers: peer 0 is a
    // bright dot with a facing line; peer 1 is a ring drawn over it (they coincide).
    let p0 = &runtime.net.peers[0].match_state.body;
    let p1 = &runtime.net.peers[1].match_state.body;
    let pose0 = world_screen(p0.position.x, p0.position.z);
    let forward = Vec2::new(p0.yaw.sin(), p0.yaw.cos()) * cell * 1.6;
    gizmos.circle_2d(pose0, cell * 0.45, Color::srgb(1.0, 0.95, 0.4));
    gizmos.line_2d(pose0, pose0 + forward, Color::srgb(1.0, 0.95, 0.4));
    gizmos.circle_2d(
        world_screen(p1.position.x, p1.position.z),
        cell * 0.7,
        Color::srgb(0.4, 1.0, 0.9),
    );
}

pub fn update_debug_text(runtime: Res<NetRuntime>, mut query: Query<&mut Text, With<DebugText>>) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let net = &runtime.net;
    let p0 = &net.peers[0];
    let p1 = &net.peers[1];
    let synced = net.synchronized();
    let health = if synced { "[PASS]" } else { "[ ... ]" };
    let dropped = net.network.dropped;
    let duped = net.network.duplicated;
    let reordered = net.network.reordered;

    **text = format!(
        "NETWORKED HYBRID MATCH  ({})\n\
         transport: {} ticks | profile {} | auto {}\n\
         committed: A {}/{}  B {}/{}   waits {}   resent {}\n\
         packets: sent {} | dropped {} | duped {} | reordered {} | rejected {}\n\
         peers agree: {}   matches single-player tape: {}   both finished: {}\n\
         {}  winner {:?}\n\
         R reset | Space step | A auto | P toggle network",
        runtime.profile.label(),
        net.transport_ticks,
        runtime.profile.label(),
        if runtime.auto { "on" } else { "off" },
        p0.committed_round,
        net.total,
        p1.committed_round,
        net.total,
        net.total_waits(),
        net.total_resent(),
        net.network.sent,
        dropped,
        duped,
        reordered,
        p0.rejected_packets + p1.rejected_packets,
        yes_no(net.peers_agree()),
        yes_no(net.matches_reference()),
        yes_no(net.both_finished()),
        health,
        net.peers[0]
            .match_state
            .competitive
            .winner
            .map(|team| team.0),
    );
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
