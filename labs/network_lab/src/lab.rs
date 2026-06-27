use bevy::{ecs::system::SystemParam, prelude::*};

use crate::model::{LockstepDemo, NetworkProfile, TARGET_FRAMES};
use crate::protocol::{PEER_COUNT, PeerId};

const PANEL_CENTERS: [Vec2; PEER_COUNT] = [Vec2::new(-350.0, 15.0), Vec2::new(350.0, 15.0)];
const BODY_COLORS: [Color; PEER_COUNT] =
    [Color::srgb(0.25, 0.85, 1.0), Color::srgb(1.0, 0.62, 0.25)];
const WORLD_SCALE: f32 = 8.0;

#[derive(Component)]
pub(crate) struct NetworkOwned;

#[derive(Component)]
pub(crate) struct NetworkUiRoot;

#[derive(Component)]
pub(crate) struct PeerBodyDot {
    pub peer: PeerId,
    pub body: PeerId,
}

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct NetworkRuntime {
    pub running: bool,
    pub step_requested: bool,
    pub reset_requested: bool,
    pub inject_desync_requested: bool,
    pub profile: NetworkProfile,
    pub debug_visible: bool,
    pub ticks_per_update: u32,
    pub reset_count: u32,
}

impl Default for NetworkRuntime {
    fn default() -> Self {
        Self {
            running: true,
            step_requested: false,
            reset_requested: false,
            inject_desync_requested: false,
            profile: NetworkProfile::Hostile,
            debug_visible: true,
            ticks_per_update: 6,
            reset_count: 0,
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands
        .spawn((
            NetworkOwned,
            Name::new("Network Lab World"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (peer, center) in PANEL_CENTERS.iter().enumerate() {
                for (body, color) in BODY_COLORS.iter().enumerate() {
                    parent.spawn((
                        PeerBodyDot {
                            peer: PeerId(peer as u8),
                            body: PeerId(body as u8),
                        },
                        Name::new(format!("Peer {} body {}", peer + 1, body + 1)),
                        Sprite::from_color(*color, Vec2::splat(20.0)),
                        Transform::from_xyz(center.x, center.y, 5.0),
                    ));
                }
            }
        });
    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            NetworkOwned,
            NetworkUiRoot,
            Name::new("Network Lab UI Root"),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                DebugPanel,
                Node {
                    width: px(475),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.96)),
                BorderColor::all(Color::srgba(0.4, 0.82, 1.0, 0.65)),
                children![(
                    DebugText,
                    Text::new("Lockstep diagnostics starting..."),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.84, 0.94, 1.0)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(455),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.96)),
                BorderColor::all(Color::srgba(1.0, 0.62, 0.28, 0.65)),
                children![(
                    Text::new(
                        "PHASE 16 - DETERMINISTIC LOCKSTEP\n\
                         Space   Run / pause transport\n\
                         N       Single network tick\n\
                         L       Toggle CLEAN / HOSTILE link\n\
                         D       Inject a deliberate desync\n\
                         R       Reset session / F1 Toggle debug\n\n\
                         Both peers simulate both first-person bodies. A frame\n\
                         commits only after both quantized inputs arrive. The\n\
                         hostile link drops, delays, duplicates, and reorders\n\
                         datagrams; resend + cumulative ACK repairs the stream.\n\
                         Per-frame hashes expose divergence, and committed frames\n\
                         are the exact replay tape.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.92, 0.88)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<NetworkRuntime>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        runtime.running = !runtime.running;
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        runtime.running = false;
        runtime.step_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyL) {
        runtime.profile = match runtime.profile {
            NetworkProfile::Clean => NetworkProfile::Hostile,
            NetworkProfile::Hostile => NetworkProfile::Clean,
        };
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyD) {
        runtime.inject_desync_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

pub(crate) fn perform_reset(mut runtime: ResMut<NetworkRuntime>, mut demo: ResMut<LockstepDemo>) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.step_requested = false;
    runtime.inject_desync_requested = false;
    runtime.running = false;
    runtime.reset_count += 1;
    demo.reset(runtime.profile);
}

pub(crate) fn simulate(mut runtime: ResMut<NetworkRuntime>, mut demo: ResMut<LockstepDemo>) {
    if runtime.inject_desync_requested {
        runtime.inject_desync_requested = false;
        demo.peers[0].inject_divergence();
    }

    let ticks = if runtime.step_requested {
        runtime.step_requested = false;
        1
    } else if runtime.running {
        runtime.ticks_per_update
    } else {
        0
    };
    for _ in 0..ticks {
        demo.advance_transport_tick();
        if demo.has_desync() || demo.synchronized() {
            runtime.running = false;
            break;
        }
    }
}

pub(crate) fn present_bodies(
    demo: Res<LockstepDemo>,
    mut dots: Query<(&PeerBodyDot, &mut Transform, &mut Sprite)>,
) {
    for (dot, mut transform, mut sprite) in &mut dots {
        let body = demo.peers[dot.peer.index()].world.bodies[dot.body.index()];
        let center = PANEL_CENTERS[dot.peer.index()];
        transform.translation.x = center.x + body.position.x * WORLD_SCALE;
        transform.translation.y = center.y - body.position.z * WORLD_SCALE;
        sprite.color = if demo.peers[dot.peer.index()].desync.is_some() {
            Color::srgb(1.0, 0.15, 0.15)
        } else {
            let base = BODY_COLORS[dot.body.index()];
            if dot.peer == dot.body {
                base.mix(&Color::WHITE, 0.2)
            } else {
                base.mix(&Color::BLACK, 0.25)
            }
        };
    }
}

pub(crate) fn draw_debug(
    runtime: Res<NetworkRuntime>,
    demo: Res<LockstepDemo>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    for (peer_index, center) in PANEL_CENTERS.iter().enumerate() {
        let peer = &demo.peers[peer_index];
        let frame_ratio = peer.next_frame as f32 / demo.target_frames as f32;
        let border = if peer.desync.is_some() {
            Color::srgb(1.0, 0.18, 0.18)
        } else if demo.synchronized() {
            Color::srgb(0.25, 1.0, 0.62)
        } else {
            Color::srgb(0.35, 0.65, 0.85)
        };
        gizmos.rect_2d(*center, Vec2::new(330.0, 330.0), border);
        gizmos.line_2d(
            Vec2::new(center.x - 150.0, center.y - 185.0),
            Vec2::new(center.x + 150.0, center.y - 185.0),
            Color::srgba(0.4, 0.48, 0.55, 0.5),
        );
        gizmos.line_2d(
            Vec2::new(center.x - 150.0, center.y - 185.0),
            Vec2::new(center.x - 150.0 + 300.0 * frame_ratio, center.y - 185.0),
            border,
        );

        let path = &peer.path;
        for body_index in 0..PEER_COUNT {
            let color = BODY_COLORS[body_index].with_alpha(0.55);
            let mut previous = None;
            for positions in path.iter().step_by(4) {
                let point = Vec2::new(
                    center.x + positions[body_index].x * WORLD_SCALE,
                    center.y - positions[body_index].z * WORLD_SCALE,
                );
                if let Some(last) = previous {
                    gizmos.line_2d(last, point, color);
                }
                previous = Some(point);
            }
        }
    }

    let link_color = if demo.has_desync() {
        Color::srgb(1.0, 0.15, 0.15)
    } else if demo.hashes_match() {
        Color::srgb(0.35, 1.0, 0.68)
    } else {
        Color::srgb(1.0, 0.72, 0.28)
    };
    gizmos.line_2d(Vec2::new(-180.0, 0.0), Vec2::new(180.0, 0.0), link_color);
    let shown = demo.network.in_flight().min(12);
    for index in 0..shown {
        let x = -150.0 + index as f32 * 300.0 / shown.max(1) as f32;
        let y = if index % 2 == 0 { 12.0 } else { -12.0 };
        gizmos.circle_2d(Vec2::new(x, y), 5.0, Color::srgb(1.0, 0.82, 0.35));
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, NetworkRuntime>,
    demo: Res<'w, LockstepDemo>,
    body_dots: Query<'w, 's, (), With<PeerBodyDot>>,
    ui_roots: Query<'w, 's, (), With<NetworkUiRoot>>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub(crate) fn update_debug_text(mut context: DebugContext) {
    let visibility = if context.runtime.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = visibility;
    **context.help = visibility;

    let demo = &*context.demo;
    let body_dots = context.body_dots.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let entity_healthy = body_dots == PEER_COUNT * PEER_COUNT && ui_roots == 1;
    let protocol_healthy = !demo.has_desync()
        && (!demo.synchronized()
            || (demo.frames_match()
                && demo.hashes_match()
                && demo.tapes_match()
                && demo.replay_matches()));
    let healthy = entity_healthy && protocol_healthy;

    let mut peers = String::new();
    for peer in &demo.peers {
        let hash = peer.world.state_hash();
        let remote = peer
            .remote_hash(peer.next_frame)
            .map(|value| format!("{:08x}", value as u32))
            .unwrap_or_else(|| "waiting".to_string());
        let desync = peer
            .desync
            .map(|value| format!("DESYNC@{}", value.frame))
            .unwrap_or_else(|| "OK".to_string());
        peers.push_str(&format!(
            "{} frame {:>3}/{:<3} hash {:08x} remote {:<8} {}\n\
             waits {:<4} resend {:<4} dup {:<4} outbox {}\n",
            peer.id.label(),
            peer.next_frame,
            demo.target_frames,
            hash as u32,
            remote,
            desync,
            peer.wait_ticks,
            peer.resent_packets,
            peer.duplicate_inputs,
            peer.outbox_len(),
        ));
    }

    let status = if demo.has_desync() {
        "DESYNC DETECTED"
    } else if demo.synchronized() {
        "SYNCHRONIZED"
    } else if context.runtime.running {
        "RUNNING"
    } else {
        "PAUSED"
    };

    let mut text = context.text.into_inner();
    **text = format!(
        "LOCKSTEP MONITOR  {}\n\
         status       {}\n\
         link         {}\n\
         transport    {} ticks\n\
         in flight    {}\n\
         sent/drop    {} / {}\n\
         delivered    {}\n\
         duplicate    {}\n\
         reordered    {}\n\n\
         {}\
         tapes        {}\n\
         replay       {}\n\
         body dots    {}/{}  UI {}\n\
         resets       {}\n\n\
         Complete frames are the replay tape; no prediction or rollback.",
        if healthy { "[PASS]" } else { "[FAIL]" },
        status,
        demo.network.profile.label(),
        demo.transport_ticks,
        demo.network.in_flight(),
        demo.network.sent,
        demo.network.dropped,
        demo.network.delivered,
        demo.network.duplicated,
        demo.network.reordered,
        peers,
        if demo.tapes_match() {
            "MATCH"
        } else {
            "MISMATCH"
        },
        if demo.replay_matches() {
            "MATCH"
        } else {
            "WAITING"
        },
        body_dots,
        PEER_COUNT * PEER_COUNT,
        ui_roots,
        context.runtime.reset_count,
    );
}

pub(crate) fn capture_ready(demo: &LockstepDemo) -> bool {
    demo.synchronized()
        && demo.network.profile == NetworkProfile::Hostile
        && demo.network.dropped > 0
        && demo.network.duplicated > 0
        && demo.network.reordered > 0
        && demo
            .peers
            .iter()
            .all(|peer| peer.next_frame == TARGET_FRAMES)
}
