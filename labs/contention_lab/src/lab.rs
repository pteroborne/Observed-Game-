use bevy::prelude::*;
use observed_core::{Direction, RoomId, TeamId};
use observed_observation::contention::{ContentionWorld, PinSource};
use observed_observation::{DoorId, ROOM_COUNT, ROOM_HALF};

const TEAM_COLORS: [Color; 4] = [
    Color::srgb(0.96, 0.28, 0.34), // Team 0: red
    Color::srgb(0.32, 0.62, 1.0),  // Team 1: blue
    Color::srgb(0.72, 0.46, 1.0),  // Team 2: purple
    Color::srgb(1.0, 0.62, 0.20),  // Team 3: orange
];

#[derive(Component)]
pub(crate) struct ContentionOwned;

#[derive(Component)]
pub(crate) struct ContentionUiRoot;

#[derive(Component)]
pub(crate) struct DebugText;

#[derive(Component)]
pub(crate) struct HelpText;

#[derive(Component)]
pub(crate) struct DebugPanel;

#[derive(Component)]
pub(crate) struct MemberDot(pub(crate) usize); // member index

#[derive(Component)]
#[allow(dead_code)]
pub(crate) struct AnchorMarker(pub(crate) TeamId, pub(crate) RoomId);

#[derive(Resource, Clone, Debug)]
pub struct ContentionRuntime {
    pub selected_member: usize, // which member Team 0 is controlling (always 0 for now)
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub knowledge_mode: usize, // 0=off, 1=team 0, 2=team 1, 3=team 2, 4=team 3, then loop
}

impl Default for ContentionRuntime {
    fn default() -> Self {
        Self {
            selected_member: 0,
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            knowledge_mode: 0,
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<ContentionWorld>) {
    // Spawn room backgrounds
    commands
        .spawn((
            ContentionOwned,
            Name::new("Structure Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for index in 0..ROOM_COUNT {
                let room = RoomId(index as u32);
                let center = world.world.room_center(room);
                parent.spawn((
                    Name::new(format!("Room {index}")),
                    Sprite::from_color(
                        Color::srgb(0.07, 0.10, 0.15),
                        Vec2::splat(ROOM_HALF * 2.0 - 6.0),
                    ),
                    Transform::from_translation(center.extend(-2.0)),
                ));
            }
        });

    // Spawn member dots
    commands
        .spawn((
            ContentionOwned,
            Name::new("Members Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, member) in world.members.iter().enumerate() {
                parent.spawn((
                    MemberDot(index),
                    Name::new(format!("Member {} (Team {})", index, member.team.0)),
                    Sprite::from_color(TEAM_COLORS[member.team.0 as usize], Vec2::splat(20.0)),
                    Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
                ));
            }
        });

    // Spawn anchor markers
    commands
        .spawn((
            ContentionOwned,
            Name::new("Anchors Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for anchor in &world.anchors {
                let center = world.world.room_center(anchor.room);
                parent.spawn((
                    AnchorMarker(anchor.team, anchor.room),
                    Name::new(format!(
                        "Anchor Team {} Room {}",
                        anchor.team.0, anchor.room.0
                    )),
                    Sprite::from_color(TEAM_COLORS[anchor.team.0 as usize], Vec2::splat(12.0)),
                    Transform::from_translation(center.extend(3.0)),
                ));
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            ContentionOwned,
            ContentionUiRoot,
            Name::new("Contention Lab UI Root"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(16.0)),
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
                    width: Val::Px(500.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.6)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    DebugText,
                    Text::new("Contention diagnostics starting…"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.80, 0.94, 1.0)),
                ));
            });

            root.spawn((
                HelpText,
                Node {
                    width: Val::Px(480.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.6)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(
                        "CONTENTION LAB\n\
                         Arrows    Move Team 0's member (traverse)\n\
                         D         Decohere (rewire unpinned doors)\n\
                         A         Toggle Team 0's anchor in current room\n\
                         K         Cycle knowledge view (OFF→T0→T1→T2→T3→OFF)\n\
                         R         Reset · F1 Toggle help\n\n\
                         Shared observation: any team presence or anchor freezes\n\
                         a room for all teams. Private knowledge: each team's\n\
                         ledger shows only what it has personally observed.",
                    ),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.92, 0.97)),
                ));
            });
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<ContentionRuntime>,
    mut world: ResMut<ContentionWorld>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }

    if keyboard.just_pressed(KeyCode::KeyA) {
        // Toggle Team 0's anchor in Team 0's current room
        if !world.members.is_empty() {
            let room = world.members[0].room;
            if world
                .anchors
                .iter()
                .any(|a| a.team == TeamId(0) && a.room == room)
            {
                world.remove_anchor(TeamId(0), room);
            } else {
                world.place_anchor(TeamId(0), room);
            }
        }
    }

    if keyboard.just_pressed(KeyCode::KeyK) {
        runtime.knowledge_mode = (runtime.knowledge_mode + 1) % 5;
    }

    if keyboard.just_pressed(KeyCode::KeyD) {
        world.record_observations();
        world.decohere();
    }

    // Arrow key movement for Team 0's member
    let move_dir = if keyboard.just_pressed(KeyCode::ArrowUp) {
        Some(Direction::North)
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        Some(Direction::South)
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        Some(Direction::West)
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        Some(Direction::East)
    } else {
        None
    };

    if let Some(direction) = move_dir {
        world.traverse(runtime.selected_member, direction);
        world.record_observations();
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<ContentionRuntime>,
    mut world: ResMut<ContentionWorld>,
    mut commands: Commands,
    query: Query<Entity, With<ContentionOwned>>,
) {
    if !runtime.reset_requested {
        return;
    }

    // Despawn all owned entities
    for entity in &query {
        commands.entity(entity).despawn();
    }

    // Rebuild the world from scratch
    let edges = crate::make_authored_edges();
    *world = ContentionWorld::new(
        observed_observation::ROOM_COUNT,
        &edges,
        &[
            (TeamId(0), RoomId(0)),
            (TeamId(1), RoomId(2)),
            (TeamId(2), RoomId(6)),
            (TeamId(3), RoomId(4)),
        ],
        RoomId(8),
        0x00C0_FFEE_DEAD_BEEF,
    );
    world.record_observations();

    runtime.reset_requested = false;
    runtime.reset_count += 1;

    // Re-setup lab UI and entities by reconstructing setup_lab call
    setup_lab_static(&mut commands, &world);
}

fn setup_lab_static(commands: &mut Commands, world: &ContentionWorld) {
    // Spawn room backgrounds
    commands
        .spawn((
            ContentionOwned,
            Name::new("Structure Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for index in 0..ROOM_COUNT {
                let room = RoomId(index as u32);
                let center = world.world.room_center(room);
                parent.spawn((
                    Name::new(format!("Room {index}")),
                    Sprite::from_color(
                        Color::srgb(0.07, 0.10, 0.15),
                        Vec2::splat(ROOM_HALF * 2.0 - 6.0),
                    ),
                    Transform::from_translation(center.extend(-2.0)),
                ));
            }
        });

    // Spawn member dots
    commands
        .spawn((
            ContentionOwned,
            Name::new("Members Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (index, member) in world.members.iter().enumerate() {
                parent.spawn((
                    MemberDot(index),
                    Name::new(format!("Member {} (Team {})", index, member.team.0)),
                    Sprite::from_color(TEAM_COLORS[member.team.0 as usize], Vec2::splat(20.0)),
                    Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
                ));
            }
        });

    // Spawn anchor markers
    commands
        .spawn((
            ContentionOwned,
            Name::new("Anchors Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for anchor in &world.anchors {
                let center = world.world.room_center(anchor.room);
                parent.spawn((
                    AnchorMarker(anchor.team, anchor.room),
                    Name::new(format!(
                        "Anchor Team {} Room {}",
                        anchor.team.0, anchor.room.0
                    )),
                    Sprite::from_color(TEAM_COLORS[anchor.team.0 as usize], Vec2::splat(12.0)),
                    Transform::from_translation(center.extend(3.0)),
                ));
            }
        });

    spawn_ui(commands);
}

pub(crate) fn present_members(
    world: Res<ContentionWorld>,
    mut query: Query<(&MemberDot, &mut Transform)>,
) {
    for (dot, mut transform) in &mut query {
        if let Some(member) = world.members.get(dot.0) {
            let center = world.world.room_center(member.room);
            transform.translation = center.extend(4.0);
        }
    }
}

pub(crate) fn draw_debug(
    world: Res<ContentionWorld>,
    runtime: Res<ContentionRuntime>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    // Draw links (or known edges if a knowledge mode is active)
    if runtime.knowledge_mode == 0 {
        // Draw all true links
        for (door_a, door_b) in world.world.connections() {
            draw_link(&mut gizmos, &world, door_a, door_b, runtime.knowledge_mode);
        }
    } else {
        // Draw only known edges for the selected team
        let team_idx = runtime.knowledge_mode - 1;
        let team = TeamId(team_idx as u8);
        let knowledge = world.known_edges(team);

        // Iterate through all doors and draw known edges
        for door_idx in 0..world.world.doors.len() {
            let door_a = DoorId(door_idx as u16);
            if let Some(known_edge) = knowledge.get(door_a) {
                let partner = known_edge.partner;
                if door_a.0 < partner.0 {
                    // Draw with staleness fading
                    let staleness = knowledge.staleness(door_a, world.tick).unwrap_or(0);
                    draw_known_link(&mut gizmos, &world, door_a, partner, staleness, world.tick);
                }
            }
        }
    }

    // Draw room borders (exit room bright, others dimmer)
    for index in 0..ROOM_COUNT {
        let room = RoomId(index as u32);
        let center = world.world.room_center(room);
        let half = ROOM_HALF;
        let border_color = if room == world.exit {
            Color::srgb(1.0, 0.8, 0.0) // yellow for exit
        } else {
            Color::srgb(0.3, 0.4, 0.5)
        };
        let _thickness = if room == world.exit { 2.0 } else { 1.0 };

        // Draw rectangle outline
        let tl = center + Vec2::new(-half, half);
        let tr = center + Vec2::new(half, half);
        let bl = center + Vec2::new(-half, -half);
        let br = center + Vec2::new(half, -half);

        gizmos.line_2d(tl, tr, border_color);
        gizmos.line_2d(tr, br, border_color);
        gizmos.line_2d(br, bl, border_color);
        gizmos.line_2d(bl, tl, border_color);
    }
}

fn draw_link(
    gizmos: &mut Gizmos,
    world: &ContentionWorld,
    door_a: DoorId,
    door_b: DoorId,
    _knowledge_mode: usize,
) {
    let pos_a = world.world.door_position(door_a);
    let pos_b = world.world.door_position(door_b);

    let is_pinned = world.is_pinned(door_a);
    let sources = world.pin_sources(door_a);

    // Determine color: if pinned, tint by first source's team; otherwise cyan (free)
    let (color, _thickness) = if is_pinned {
        let team_color = if let Some(PinSource::Presence(team)) = sources.first() {
            TEAM_COLORS[team.0 as usize]
        } else if let Some(PinSource::Anchor(team)) = sources.first() {
            TEAM_COLORS[team.0 as usize]
        } else {
            Color::srgb(0.5, 0.85, 1.0)
        };
        (team_color, 2.5)
    } else {
        (Color::srgb(0.3, 0.85, 1.0), 1.5)
    };

    gizmos.line_2d(pos_a, pos_b, color);
}

fn draw_known_link(
    gizmos: &mut Gizmos,
    world: &ContentionWorld,
    door_a: DoorId,
    door_b: DoorId,
    staleness: u64,
    tick: u64,
) {
    let pos_a = world.world.door_position(door_a);
    let pos_b = world.world.door_position(door_b);

    // Fade older observations: alpha = 1.0 / (1.0 + staleness_ratio)
    let max_staleness = (tick + 1).max(1);
    let fade = 1.0 / (1.0 + staleness as f32 / max_staleness as f32 * 2.0);
    let color = Color::srgba(0.5, 0.85, 1.0, fade * 0.7);

    gizmos.line_2d(pos_a, pos_b, color);
}

pub(crate) fn update_debug_text(
    world: Res<ContentionWorld>,
    runtime: Res<ContentionRuntime>,
    mut query: Query<&mut Text, With<DebugText>>,
) {
    for mut text in &mut query {
        let anchor_count = world.anchors.len();
        let knowledge_mode_str = match runtime.knowledge_mode {
            0 => "OFF".to_string(),
            1 => "Team 0".to_string(),
            2 => "Team 1".to_string(),
            3 => "Team 2".to_string(),
            4 => "Team 3".to_string(),
            _ => "?".to_string(),
        };

        text.0 = format!(
            "Tick: {}\n\
             Decoherence: {}\n\
             Last attempts: {} {}\n\
             Anchors: {}\n\
             Knowledge: {}\n\
             Exit Room: {}\n\n\
             Member rooms: {}",
            world.tick,
            world.world.decoherence_count,
            world.last_decohere_attempts,
            if world.last_decohere_reverted {
                "(reverted)"
            } else {
                ""
            },
            anchor_count,
            knowledge_mode_str,
            world.exit.0,
            world
                .members
                .iter()
                .map(|m| format!("T{}→R{}", m.team.0, m.room.0))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}
