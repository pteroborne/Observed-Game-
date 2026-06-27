use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::{PlayerId, RoomId, TeamId};

use crate::model::{IncentiveWorld, MEMBERS_PER_TEAM, ROOM_COUNT, TEAM_COUNT, TOTAL_MEMBERS};

const TEAM_COLORS: [Color; TEAM_COUNT] =
    [Color::srgb(0.35, 0.78, 1.0), Color::srgb(1.0, 0.55, 0.30)];

const ROOM_SPACING: f32 = 200.0;
const ROOM_HALF_W: f32 = 85.0;
const ROOM_HALF_H: f32 = 110.0;
const BAR_WIDTH: f32 = 150.0;

fn room_center(index: usize) -> Vec2 {
    Vec2::new(
        (index as f32 - (ROOM_COUNT as f32 - 1.0) * 0.5) * ROOM_SPACING,
        0.0,
    )
}

fn member_offset(player: PlayerId) -> Vec2 {
    let team = player.0 as usize / MEMBERS_PER_TEAM;
    let slot = player.0 as usize % MEMBERS_PER_TEAM;
    Vec2::new(
        (slot as f32 - 1.0) * 40.0,
        if team == 0 { 46.0 } else { -46.0 },
    )
}

#[derive(Component)]
pub(crate) struct IncentiveOwned;

#[derive(Component)]
pub(crate) struct IncentiveUiRoot;

#[derive(Component)]
pub(crate) struct ChargeBar(pub RoomId);

#[derive(Component)]
pub(crate) struct MemberDot(pub PlayerId);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct IncentiveRuntime {
    pub selected_team: TeamId,
    pub selected_member: PlayerId,
    pub running: bool,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
}

impl Default for IncentiveRuntime {
    fn default() -> Self {
        Self {
            selected_team: TeamId(0),
            selected_member: PlayerId(0),
            running: true,
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands
        .spawn((
            IncentiveOwned,
            Name::new("Rooms Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for index in 0..ROOM_COUNT {
                let center = room_center(index);
                parent.spawn((
                    Name::new(format!("Room {index}")),
                    Sprite::from_color(
                        Color::srgb(0.07, 0.10, 0.15),
                        Vec2::new(ROOM_HALF_W * 2.0, ROOM_HALF_H * 2.0),
                    ),
                    Transform::from_translation(center.extend(-2.0)),
                ));
                parent.spawn((
                    ChargeBar(RoomId(index as u32)),
                    Name::new(format!("Charge {index}")),
                    Sprite::from_color(Color::srgb(0.3, 0.9, 0.5), Vec2::new(BAR_WIDTH, 4.0)),
                    Transform::from_translation(center.extend(-1.0)),
                ));
            }

            for i in 0..TOTAL_MEMBERS {
                let player = PlayerId(i as u16);
                let team = i / MEMBERS_PER_TEAM;
                parent.spawn((
                    MemberDot(player),
                    Name::new(format!("Member {i}")),
                    Sprite::from_color(TEAM_COLORS[team % TEAM_COUNT], Vec2::splat(26.0)),
                    Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
                ));
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            IncentiveOwned,
            IncentiveUiRoot,
            Name::new("Incentive Lab UI Root"),
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
                    width: px(440),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.85, 0.7, 0.6)),
                children![(
                    DebugText,
                    Text::new("Incentive diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.97, 0.9)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(420),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.85, 0.7, 0.6)),
                children![(
                    Text::new(
                        "INCENTIVE LAB\n\
                         A / D     Move selected member between rooms\n\
                         Tab       Select next member of your team\n\
                         1 / 2     Select your team\n\
                         Space     Pause / resume · R reset · F1 debug\n\n\
                         Rooms pay score while occupied and drain; empty rooms\n\
                         regrow. A team's harvest is multiplied by how many rooms\n\
                         it spreads across — so splitting and revisiting regrown\n\
                         rooms beats clumping or camping one path.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.93, 0.92)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<IncentiveRuntime>,
    mut world: ResMut<IncentiveWorld>,
) {
    for (key, team) in [(KeyCode::Digit1, TeamId(0)), (KeyCode::Digit2, TeamId(1))] {
        if keyboard.just_pressed(key) {
            runtime.selected_team = team;
            runtime.selected_member = PlayerId((team.0 as u16) * MEMBERS_PER_TEAM as u16);
        }
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        let base = runtime.selected_team.0 as u16 * MEMBERS_PER_TEAM as u16;
        let slot = (runtime.selected_member.0 + 1 - base) % MEMBERS_PER_TEAM as u16;
        runtime.selected_member = PlayerId(base + slot);
    }
    if keyboard.just_pressed(KeyCode::Space) {
        runtime.running = !runtime.running;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }

    let step = if keyboard.just_pressed(KeyCode::KeyD) || keyboard.just_pressed(KeyCode::ArrowRight)
    {
        1i32
    } else if keyboard.just_pressed(KeyCode::KeyA) || keyboard.just_pressed(KeyCode::ArrowLeft) {
        -1
    } else {
        0
    };
    if step != 0
        && let Some(member) = world.member(runtime.selected_member)
    {
        let next = (member.room.0 as i32 + step).clamp(0, ROOM_COUNT as i32 - 1) as u32;
        world.move_member(runtime.selected_member, RoomId(next));
    }
}

pub(crate) fn simulate(
    time: Res<Time>,
    runtime: Res<IncentiveRuntime>,
    mut world: ResMut<IncentiveWorld>,
) {
    if runtime.running {
        world.tick(time.delta_secs());
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<IncentiveRuntime>,
    mut world: ResMut<IncentiveWorld>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.reset_count += 1;
    world.reset();
}

pub(crate) fn present_rooms(
    world: Res<IncentiveWorld>,
    mut bars: Query<(&ChargeBar, &mut Transform, &mut Sprite)>,
) {
    for (bar, mut transform, mut sprite) in &mut bars {
        if let Some(room) = world.room(bar.0) {
            let height = (room.charge * (ROOM_HALF_H * 2.0 - 12.0)).max(2.0);
            sprite.custom_size = Some(Vec2::new(BAR_WIDTH, height));
            let center = room_center(bar.0.0 as usize);
            transform.translation.x = center.x;
            transform.translation.y = center.y - ROOM_HALF_H + 6.0 + height * 0.5;
            sprite.color = Color::srgb(
                0.2 + (1.0 - room.charge) * 0.8,
                0.35 + room.charge * 0.55,
                0.35,
            );
        }
    }
}

pub(crate) fn present_members(
    runtime: Res<IncentiveRuntime>,
    world: Res<IncentiveWorld>,
    mut dots: Query<(&MemberDot, &mut Transform, &mut Sprite)>,
) {
    for (dot, mut transform, mut sprite) in &mut dots {
        if let Some(member) = world.member(dot.0) {
            let position = room_center(member.room.0 as usize) + member_offset(dot.0);
            transform.translation.x = position.x;
            transform.translation.y = position.y;
            let team = dot.0.0 as usize / MEMBERS_PER_TEAM;
            let base = TEAM_COLORS[team % TEAM_COUNT];
            sprite.color = if dot.0 == runtime.selected_member {
                base.mix(&Color::WHITE, 0.45)
            } else {
                base
            };
        }
    }
}

pub(crate) fn draw_debug(
    runtime: Res<IncentiveRuntime>,
    world: Res<IncentiveWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }
    for index in 0..ROOM_COUNT {
        gizmos.rect_2d(
            room_center(index),
            Vec2::new(ROOM_HALF_W * 2.0, ROOM_HALF_H * 2.0),
            Color::srgb(0.3, 0.4, 0.5),
        );
    }
    if let Some(member) = world.member(runtime.selected_member) {
        let position = room_center(member.room.0 as usize) + member_offset(runtime.selected_member);
        gizmos.circle_2d(position, 20.0, Color::srgb(0.8, 1.0, 0.85));
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, IncentiveRuntime>,
    world: Res<'w, IncentiveWorld>,
    members: Query<'w, 's, (), With<MemberDot>>,
    ui_roots: Query<'w, 's, (), With<IncentiveUiRoot>>,
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

    let world = &*context.world;
    let mut standings = String::new();
    for team in &world.teams {
        standings.push_str(&format!(
            "{:<8} score {:>6.1}   covers {} room(s)  x{:.1}\n",
            team.label(),
            world.score_of(*team),
            world.coverage(*team),
            world.dispersion(*team),
        ));
    }

    let members = context.members.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = members == TOTAL_MEMBERS && ui_roots == 1;

    let mut text = context.text.into_inner();
    **text = format!(
        "INCENTIVE MONITOR  {}\n\
         {}\
         tick {}   {}\n\
         selected  {} of {}\n\
         members {members}  UI {ui_roots}   resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        standings,
        world.tick_count,
        if context.runtime.running {
            "running"
        } else {
            "paused"
        },
        context.runtime.selected_member.label(),
        context.runtime.selected_team.label(),
        context.runtime.reset_count,
        world.last_event,
    );
}
