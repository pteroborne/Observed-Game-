use super::logic::{parse_ascii_map, shuffle_links};
use super::model::{
    HallwayId, HallwayNode, Link, RoomNode, SimpleRng, ThresholdEndpoint, ThresholdSlotId,
};
use bevy::{
    app::AppExit,
    math::Vec3Swizzles,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use observed_core::RoomId;
use std::collections::{HashMap, HashSet};

// ==========================================
// BEVY LAB FEASIBILITY PROTOTYPE
// ==========================================

#[derive(Resource)]
pub struct DebugFlag(pub bool);

#[derive(Resource)]
pub struct LabState {
    pub rooms: HashMap<RoomId, RoomNode>,
    pub hallways: HashMap<HallwayId, HallwayNode>,
    pub links: Vec<Link>,
    pub active_endpoint: ThresholdEndpoint,
    pub player_pos: Vec3, // Local position inside the active node
    pub rng: SimpleRng,
    pub status_msg: String,
    pub last_action_msg: String,
}

#[derive(Component)]
pub struct LabCamera;

#[derive(Component)]
pub struct LabTextUi;

pub struct TopologyLabPlugin;

impl Plugin for TopologyLabPlugin {
    fn build(&self, app: &mut App) {
        // Load the complete bipartite graph ASCII map
        let ascii_map = r#"
            [ROOMS]
            ROOM 0: Spawn, Slots: [0; 1; 2]
            ROOM 1: Key, Slots: [0; 1; 2]
            ROOM 2: VideoDisplay, Slots: [0; 1; 2]
            ROOM 3: Exit, Slots: [0; 1; 2]

            [HALLWAYS]
            HALLWAY 0: T-Junction, Slots: [0; 1; 2; 3]
            HALLWAY 1: Cross-Junction, Slots: [0; 1; 2; 3]
            HALLWAY 2: Junction-C, Slots: [0; 1; 2; 3]

            [CONNECTIONS]
            ROOM 0, SLOT 0 <-> HALLWAY 0, SLOT 0
            ROOM 1, SLOT 0 <-> HALLWAY 0, SLOT 1
            ROOM 2, SLOT 0 <-> HALLWAY 0, SLOT 2
            ROOM 3, SLOT 0 <-> HALLWAY 0, SLOT 3
            ROOM 0, SLOT 1 <-> HALLWAY 1, SLOT 0
            ROOM 1, SLOT 1 <-> HALLWAY 1, SLOT 1
            ROOM 2, SLOT 1 <-> HALLWAY 1, SLOT 2
            ROOM 3, SLOT 1 <-> HALLWAY 1, SLOT 3
            ROOM 0, SLOT 2 <-> HALLWAY 2, SLOT 0
            ROOM 1, SLOT 2 <-> HALLWAY 2, SLOT 1
            ROOM 2, SLOT 2 <-> HALLWAY 2, SLOT 2
            ROOM 3, SLOT 2 <-> HALLWAY 2, SLOT 3
        "#;

        let (rooms, hallways, links) = parse_ascii_map(ascii_map).unwrap();

        let initial_state = LabState {
            rooms,
            hallways,
            links,
            active_endpoint: ThresholdEndpoint::Room(RoomId(0), ThresholdSlotId(0)),
            player_pos: Vec3::ZERO,
            rng: SimpleRng(1337),
            status_msg: "Solvability: Connected (DFS PASS)".to_string(),
            last_action_msg: "Spawned in Room 0 (Spawn)".to_string(),
        };

        let debug_enabled = std::env::args().any(|arg| arg == "--debug")
            || std::env::var("OBSERVED2_DEBUG").is_ok()
            || std::env::var("OBSERVED2_CAPTURE_BOT").is_ok();

        app.insert_resource(initial_state)
            .insert_resource(DebugFlag(debug_enabled))
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(Startup, (setup_lab, setup_ui))
            .add_systems(FixedUpdate, simulate_movement)
            .add_systems(
                Update,
                (handle_decoherence, draw_facility, update_ui_text).chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.01, 0.015, 0.02)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Topology & Decoherence Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(TopologyLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut state: ResMut<LabState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Trigger a shuffle for screenshot demonstration
        let current_node = state.active_endpoint;
        let mut observed = HashSet::new();
        observed.insert(current_node);

        let LabState {
            ref rooms,
            ref hallways,
            ref mut links,
            ref mut rng,
            ..
        } = *state;

        let success = shuffle_links(rooms, hallways, links, &observed, rng);
        if success {
            state.last_action_msg = "Decoherence Shuffle (Screenshot capture)".to_string();
        }
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 1.0 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 2.0 {
        exit.write(AppExit::Success);
    }
}

// Help map local nodes to physical 3D locations for display and teleporting
fn node_center_and_half(endpoint: &ThresholdEndpoint) -> (Vec3, Vec2) {
    match endpoint {
        ThresholdEndpoint::Room(RoomId(0), _) => (Vec3::new(-18.0, 0.0, 8.0), Vec2::new(4.0, 4.0)),
        ThresholdEndpoint::Room(RoomId(1), _) => (Vec3::new(-6.0, 0.0, 8.0), Vec2::new(4.0, 4.0)),
        ThresholdEndpoint::Room(RoomId(2), _) => (Vec3::new(6.0, 0.0, 8.0), Vec2::new(4.0, 4.0)),
        ThresholdEndpoint::Room(RoomId(3), _) => (Vec3::new(18.0, 0.0, 8.0), Vec2::new(4.0, 4.0)),
        ThresholdEndpoint::Hallway(HallwayId(0), _) => {
            (Vec3::new(-10.0, -8.0, -8.0), Vec2::new(3.0, 2.0))
        }
        ThresholdEndpoint::Hallway(HallwayId(1), _) => {
            (Vec3::new(0.0, -8.0, -8.0), Vec2::new(3.0, 2.0))
        }
        ThresholdEndpoint::Hallway(HallwayId(2), _) => {
            (Vec3::new(10.0, -8.0, -8.0), Vec2::new(3.0, 2.0))
        }
        _ => (Vec3::ZERO, Vec2::new(4.0, 4.0)),
    }
}

// Local slot coordinates relative to the node's center
fn slot_local_pos(endpoint: &ThresholdEndpoint) -> Vec2 {
    match endpoint {
        ThresholdEndpoint::Room(_, ThresholdSlotId(0)) => Vec2::new(-1.5, -4.0),
        ThresholdEndpoint::Room(_, ThresholdSlotId(1)) => Vec2::new(0.0, -4.0),
        ThresholdEndpoint::Room(_, ThresholdSlotId(2)) => Vec2::new(1.5, -4.0),
        ThresholdEndpoint::Hallway(_, ThresholdSlotId(0)) => Vec2::new(-2.0, 2.0),
        ThresholdEndpoint::Hallway(_, ThresholdSlotId(1)) => Vec2::new(-0.7, 2.0),
        ThresholdEndpoint::Hallway(_, ThresholdSlotId(2)) => Vec2::new(0.7, 2.0),
        ThresholdEndpoint::Hallway(_, ThresholdSlotId(3)) => Vec2::new(2.0, 2.0),
        _ => Vec2::ZERO,
    }
}

fn setup_lab(mut commands: Commands) {
    // Spawn Camera looking down at the facility, slightly higher and further back
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 32.0, 22.0).looking_at(Vec3::new(0.0, -4.0, 0.0), Vec3::Y),
        LabCamera,
    ));

    // Directional Light
    commands.spawn((
        DirectionalLight {
            illuminance: 1500.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(5.0, 20.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("Loading layout..."),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            left: Val::Px(20.0),
            ..default()
        },
        LabTextUi,
    ));
}

fn simulate_movement(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    let mut move_dir = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        move_dir.z -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        move_dir.z += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        move_dir.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        move_dir.x += 1.0;
    }

    if move_dir.length_squared() > 0.0 {
        let speed = 8.0 * 1.0 / 60.0;
        state.player_pos += move_dir.normalize() * speed;
    }

    // Contain player within the active node's boundaries
    let (_, half) = node_center_and_half(&state.active_endpoint);
    // Slightly smaller boundary to prevent falling out
    let margin = 0.4;
    state.player_pos.x = state.player_pos.x.clamp(-half.x + margin, half.x - margin);
    state.player_pos.z = state.player_pos.z.clamp(-half.y + margin, half.y - margin);

    // Check if player is approaching and crossing the active slot threshold
    let local_slot = slot_local_pos(&state.active_endpoint);
    let dist_to_slot = state.player_pos.xy().distance(local_slot);

    if dist_to_slot < 0.6 {
        // Find the connected endpoint in our links
        let current_ep = state.active_endpoint;
        if let Some(link) = state.links.iter().find(|l| l.contains_node(current_ep)) {
            let next_ep = link.other_endpoint(current_ep).unwrap();

            // Portal Teleportation!
            state.active_endpoint = next_ep;

            // Spawn player slightly inside the new node, away from the door to prevent immediate re-teleport
            let next_slot_local = slot_local_pos(&next_ep);
            let (_, next_half) = node_center_and_half(&next_ep);

            // Determine direction to step inside
            let step_dir = if next_slot_local.y.abs() > next_half.y - 0.5 {
                // Door is on North/South edge
                Vec2::new(0.0, -next_slot_local.y.signum())
            } else {
                // Door is on East/West edge
                Vec2::new(-next_slot_local.x.signum(), 0.0)
            };

            state.player_pos = Vec3::new(
                next_slot_local.x + step_dir.x * 1.0,
                0.0,
                next_slot_local.y + step_dir.y * 1.0,
            );

            let name_str = match next_ep {
                ThresholdEndpoint::Room(RoomId(id), _) => format!("Room {}", id),
                ThresholdEndpoint::Hallway(HallwayId(id), _) => format!("Hallway {}", id),
            };
            state.last_action_msg = format!("Teleported to {}", name_str);
        }
    }
}

fn handle_decoherence(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    if keyboard.just_pressed(KeyCode::KeyD) {
        // Active node and its immediate neighbors are observed (frozen)
        let current_node = state.active_endpoint;
        let mut observed = HashSet::new();
        observed.insert(current_node);

        // Also freeze immediately linked endpoints
        for link in state.links.iter() {
            if link.contains_node(current_node)
                && let Some(other) = link.other_endpoint(current_node)
            {
                observed.insert(other);
            }
        }

        let LabState {
            ref rooms,
            ref hallways,
            ref mut links,
            ref mut rng,
            ..
        } = *state;

        let success = shuffle_links(rooms, hallways, links, &observed, rng);

        if success {
            state.last_action_msg = "Decoherence Swapped Unobserved Connections!".to_string();
            state.status_msg = "Solvability: Connected (DFS PASS)".to_string();
        } else {
            state.last_action_msg =
                "Shuffle failed (Could not find valid solvable pairing)".to_string();
        }
    }
}

fn draw_cuboid_gizmo(gizmos: &mut Gizmos, center: Vec3, size: Vec3, color: Color) {
    let half = size * 0.5;
    let min = center - half;
    let max = center + half;
    let c = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    for (a, d) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
        gizmos.line(c[a], c[d], color);
        gizmos.line(c[a + 4], c[d + 4], color);
        gizmos.line(c[a], c[a + 4], color);
    }
}

fn draw_number_gizmo(gizmos: &mut Gizmos, pos: Vec3, val: u32, color: Color) {
    let size = 0.25;
    let c = color;
    let p = |dx: f32, dz: f32| pos + Vec3::new(dx * size, 0.0, dz * size);

    // Standard 7-segment layout positions
    let top_left = p(-1.0, -1.0);
    let top_right = p(1.0, -1.0);
    let bot_left = p(-1.0, 1.0);
    let bot_right = p(1.0, 1.0);
    let mid_left = p(-1.0, 0.0);
    let mid_right = p(1.0, 0.0);

    match val {
        0 => {
            gizmos.line(top_left, top_right, c);
            gizmos.line(top_right, bot_right, c);
            gizmos.line(bot_right, bot_left, c);
            gizmos.line(bot_left, top_left, c);
        }
        1 => {
            gizmos.line(top_right, bot_right, c);
        }
        2 => {
            gizmos.line(top_left, top_right, c);
            gizmos.line(top_right, mid_right, c);
            gizmos.line(mid_right, mid_left, c);
            gizmos.line(mid_left, bot_left, c);
            gizmos.line(bot_left, bot_right, c);
        }
        3 => {
            gizmos.line(top_left, top_right, c);
            gizmos.line(top_right, bot_right, c);
            gizmos.line(bot_right, bot_left, c);
            gizmos.line(mid_left, mid_right, c);
        }
        4 => {
            gizmos.line(top_left, mid_left, c);
            gizmos.line(mid_left, mid_right, c);
            gizmos.line(top_right, bot_right, c);
        }
        5 => {
            gizmos.line(top_right, top_left, c);
            gizmos.line(top_left, mid_left, c);
            gizmos.line(mid_left, mid_right, c);
            gizmos.line(mid_right, bot_right, c);
            gizmos.line(bot_right, bot_left, c);
        }
        6 => {
            gizmos.line(top_right, top_left, c);
            gizmos.line(top_left, bot_left, c);
            gizmos.line(bot_left, bot_right, c);
            gizmos.line(bot_right, mid_right, c);
            gizmos.line(mid_right, mid_left, c);
        }
        7 => {
            gizmos.line(top_left, top_right, c);
            gizmos.line(top_right, bot_right, c);
        }
        8 => {
            gizmos.line(top_left, top_right, c);
            gizmos.line(top_right, bot_right, c);
            gizmos.line(bot_right, bot_left, c);
            gizmos.line(bot_left, top_left, c);
            gizmos.line(mid_left, mid_right, c);
        }
        9 => {
            gizmos.line(top_left, top_right, c);
            gizmos.line(top_right, bot_right, c);
            gizmos.line(bot_right, bot_left, c);
            gizmos.line(mid_left, mid_right, c);
            gizmos.line(top_left, mid_left, c);
        }
        _ => {
            gizmos.line(top_left, top_right, c);
            gizmos.line(top_right, mid_right, c);
            gizmos.line(mid_right, mid_left, c);
            gizmos.line(mid_left, bot_left, c);
        }
    }
}

fn draw_facility(state: Res<LabState>, debug_flag: Option<Res<DebugFlag>>, mut gizmos: Gizmos) {
    let debug_enabled = debug_flag.map(|df| df.0).unwrap_or(false);

    // 1. Draw each node's physical boundaries
    let mut all_endpoints = Vec::new();
    for &room_id in state.rooms.keys() {
        all_endpoints.push(ThresholdEndpoint::Room(room_id, ThresholdSlotId(0)));
    }
    for &hall_id in state.hallways.keys() {
        all_endpoints.push(ThresholdEndpoint::Hallway(hall_id, ThresholdSlotId(0)));
    }

    for ep in all_endpoints {
        let (center, half) = node_center_and_half(&ep);
        let color = if ep.is_room() {
            if let ThresholdEndpoint::Room(r1, _) = state.active_endpoint
                && let ThresholdEndpoint::Room(r2, _) = ep
                && r1 == r2
            {
                Color::srgb(0.0, 0.8, 1.0) // active room is cyan
            } else {
                Color::srgb(0.0, 0.3, 0.7) // inactive rooms are dark blue
            }
        } else {
            if let ThresholdEndpoint::Hallway(h1, _) = state.active_endpoint
                && let ThresholdEndpoint::Hallway(h2, _) = ep
                && h1 == h2
            {
                Color::srgb(1.0, 0.8, 0.0) // active hallway is bright gold
            } else {
                Color::srgb(0.6, 0.4, 0.0) // inactive hallways are bronze
            }
        };

        // Draw node floor box using our wireframe helper
        draw_cuboid_gizmo(
            &mut gizmos,
            center - Vec3::new(0.0, 0.1, 0.0),
            Vec3::new(half.x * 2.0, 0.2, half.y * 2.0),
            color,
        );

        // Draw room types text-indicator or simple indicators (small boxes) on floor
        let type_color = match ep {
            ThresholdEndpoint::Room(RoomId(0), _) => Color::srgb(0.0, 1.0, 0.0), // Spawn (green)
            ThresholdEndpoint::Room(RoomId(1), _) => Color::srgb(1.0, 0.0, 1.0), // Key (purple)
            ThresholdEndpoint::Room(RoomId(2), _) => Color::srgb(0.0, 0.8, 0.8), // VideoDisplay (teal)
            ThresholdEndpoint::Room(RoomId(3), _) => Color::srgb(1.0, 0.0, 0.0), // Exit (red)
            _ => Color::WHITE,
        };
        if ep.is_room() {
            draw_cuboid_gizmo(
                &mut gizmos,
                center + Vec3::new(0.0, 0.1, 0.0),
                Vec3::new(1.0, 0.3, 1.0),
                type_color,
            );
        }
    }

    // 2. Draw threshold links as green connecting tubes/lines
    for link in state.links.iter() {
        let (center_a, _) = node_center_and_half(&link.a);
        let (center_b, _) = node_center_and_half(&link.b);

        let local_slot_a = slot_local_pos(&link.a);
        let local_slot_b = slot_local_pos(&link.b);

        let global_slot_a = center_a + Vec3::new(local_slot_a.x, 0.2, local_slot_a.y);
        let global_slot_b = center_b + Vec3::new(local_slot_b.x, 0.2, local_slot_b.y);

        // Draw doorway markers
        gizmos.sphere(global_slot_a, 0.4, Color::srgb(0.0, 0.9, 0.2));
        gizmos.sphere(global_slot_b, 0.4, Color::srgb(0.0, 0.9, 0.2));

        // Draw edge line between them
        gizmos.line(global_slot_a, global_slot_b, Color::srgb(0.0, 0.8, 0.1));

        // 3D vector digits debug overlays if enabled
        if debug_enabled {
            if let (ThresholdEndpoint::Room(room_id, _), ThresholdEndpoint::Hallway(hall_id, _)) =
                (link.a, link.b)
            {
                // Room side: draw Hallway ID
                draw_number_gizmo(
                    &mut gizmos,
                    global_slot_a + Vec3::new(0.0, 0.8, 0.0),
                    hall_id.0,
                    Color::srgb(1.0, 0.8, 0.0),
                );
                // Hallway side: draw Room ID
                draw_number_gizmo(
                    &mut gizmos,
                    global_slot_b + Vec3::new(0.0, 0.8, 0.0),
                    room_id.0,
                    Color::srgb(0.0, 0.8, 1.0),
                );
            } else if let (
                ThresholdEndpoint::Hallway(hall_id, _),
                ThresholdEndpoint::Room(room_id, _),
            ) = (link.a, link.b)
            {
                // Room side: draw Hallway ID
                draw_number_gizmo(
                    &mut gizmos,
                    global_slot_b + Vec3::new(0.0, 0.8, 0.0),
                    hall_id.0,
                    Color::srgb(1.0, 0.8, 0.0),
                );
                // Hallway side: draw Room ID
                draw_number_gizmo(
                    &mut gizmos,
                    global_slot_a + Vec3::new(0.0, 0.8, 0.0),
                    room_id.0,
                    Color::srgb(0.0, 0.8, 1.0),
                );
            }
        }
    }

    // 3. Draw Player
    let (active_center, _) = node_center_and_half(&state.active_endpoint);
    let player_global = active_center + Vec3::new(state.player_pos.x, 0.5, state.player_pos.z);
    gizmos.sphere(player_global, 0.6, Color::srgb(0.0, 1.0, 1.0));
}

fn update_ui_text(state: Res<LabState>, mut query: Query<&mut Text, With<LabTextUi>>) {
    if let Ok(mut text) = query.single_mut() {
        let active_name = match state.active_endpoint {
            ThresholdEndpoint::Room(RoomId(id), _) => {
                let node = state.rooms.get(&RoomId(id)).unwrap();
                format!("ROOM {} [{:?}]", id, node.room_type)
            }
            ThresholdEndpoint::Hallway(HallwayId(id), _) => {
                let node = state.hallways.get(&HallwayId(id)).unwrap();
                format!("HALLWAY {} [{}]", id, node.name)
            }
        };

        let mut connections_str = String::new();
        for (i, link) in state.links.iter().enumerate() {
            let desc_ep = |ep: ThresholdEndpoint| -> String {
                match ep {
                    ThresholdEndpoint::Room(RoomId(id), ThresholdSlotId(s)) => {
                        format!("R{} S{}", id, s)
                    }
                    ThresholdEndpoint::Hallway(HallwayId(id), ThresholdSlotId(s)) => {
                        format!("H{} S{}", id, s)
                    }
                }
            };
            connections_str.push_str(&format!(
                "  Link {}: {} <-> {}\n",
                i,
                desc_ep(link.a),
                desc_ep(link.b)
            ));
        }

        text.0 = format!(
            "Observed 2 — Many-to-Many Bipartite Graph & Decoherence Lab\n\n\
            ACTIVE NODE: {}\n\
            LOCAL POS:  {:.2?}\n\n\
            ACTIVE CONNECTIONS:\n{}\n\
            STATUS: {}\n\
            ACTION LOG: {}\n\n\
            CONTROLS:\n\
            - WASD / ARROWS : Move Player\n\
            - D             : Trigger Quantum Decoherence Shuffle (Enforces Connectivity)\n\
            - ESC           : Exit\n",
            active_name, state.player_pos, connections_str, state.status_msg, state.last_action_msg,
        );
    }
}
