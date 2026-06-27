use bevy::{ecs::system::SystemParam, prelude::*};
use observation_lab::model::{DOOR_COUNT, DoorId, ROOM_COUNT, ROOM_HALF, Side};
use observed_core::RoomId;

use crate::model::{EXIT_ROOM, MutableFacility, TEAM_SIZE};

const MEMBER_COLOR: Color = Color::srgb(0.35, 0.80, 1.0);
const GOLD: Color = Color::srgb(1.0, 0.80, 0.28);
const GREEN: Color = Color::srgb(0.40, 1.0, 0.60);
const CYAN: Color = Color::srgb(0.30, 0.80, 1.0);

const MEMBER_OFFSETS: [Vec2; TEAM_SIZE] = [
    Vec2::new(-24.0, -24.0),
    Vec2::new(24.0, -24.0),
    Vec2::new(-24.0, 24.0),
    Vec2::new(24.0, 24.0),
];

#[derive(Component)]
pub(crate) struct FacOwned;

#[derive(Component)]
pub(crate) struct FacUiRoot;

#[derive(Component)]
pub(crate) struct TeamDot(pub usize);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Clone, Debug)]
pub struct FacRuntime {
    pub selected_member: usize,
    pub running: bool,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
}

impl Default for FacRuntime {
    fn default() -> Self {
        Self {
            selected_member: 0,
            running: true,
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
        }
    }
}

#[derive(Resource)]
pub struct AdvanceTimer(pub Timer);

impl Default for AdvanceTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.2, TimerMode::Repeating))
    }
}

#[derive(Resource)]
pub struct DecohereTimer(pub Timer);

impl Default for DecohereTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(2.0, TimerMode::Repeating))
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands
        .spawn((
            FacOwned,
            Name::new("Facility Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for index in 0..TEAM_SIZE {
                parent.spawn((
                    TeamDot(index),
                    Name::new(format!("Member {}", index + 1)),
                    Sprite::from_color(MEMBER_COLOR, Vec2::splat(26.0)),
                    Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
                ));
            }
        });

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            FacOwned,
            FacUiRoot,
            Name::new("Mutable Facility UI Root"),
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
                    width: px(450),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.8, 0.4, 0.6)),
                children![(
                    DebugText,
                    Text::new("Facility diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.95, 0.82)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.8, 0.4, 0.6)),
                children![(
                    Text::new(
                        "MUTABLE FACILITY (integration)\n\
                         Space   Pause / resume the run\n\
                         Enter   Advance the team one spine step\n\
                         Z       Decohere the structure now\n\
                         WASD    Move the selected member through a door\n\
                         1–4     Select a member · R reset · F1 debug\n\n\
                         The team carries the cell along the gold spine to the exit\n\
                         while unobserved rooms rewire (cyan). The spine and the\n\
                         room they stand in (green) stay frozen, so the exit is\n\
                         always reachable — the objective completes despite the churn.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.92, 0.95)),
                )],
            ));
        });
}

pub(crate) fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<FacRuntime>,
    mut facility: ResMut<MutableFacility>,
) {
    for (key, member) in [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected_member = member;
        }
    }
    if keyboard.just_pressed(KeyCode::Space) {
        runtime.running = !runtime.running;
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        facility.advance();
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        facility.decohere();
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }

    for (keys, side) in [
        ([KeyCode::KeyW, KeyCode::ArrowUp], Side::North),
        ([KeyCode::KeyD, KeyCode::ArrowRight], Side::East),
        ([KeyCode::KeyS, KeyCode::ArrowDown], Side::South),
        ([KeyCode::KeyA, KeyCode::ArrowLeft], Side::West),
    ] {
        if keys.iter().any(|key| keyboard.just_pressed(*key)) {
            facility.step_member(runtime.selected_member, side);
        }
    }
}

pub(crate) fn simulate(
    time: Res<Time>,
    runtime: Res<FacRuntime>,
    mut advance: ResMut<AdvanceTimer>,
    mut decohere: ResMut<DecohereTimer>,
    mut facility: ResMut<MutableFacility>,
) {
    if !runtime.running || facility.objective_complete {
        return;
    }
    if decohere.0.tick(time.delta()).just_finished() {
        facility.decohere();
    }
    if advance.0.tick(time.delta()).just_finished() {
        facility.advance();
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<FacRuntime>,
    mut facility: ResMut<MutableFacility>,
    mut advance: ResMut<AdvanceTimer>,
    mut decohere: ResMut<DecohereTimer>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.reset_count += 1;
    facility.reset();
    advance.0.reset();
    decohere.0.reset();
}

pub(crate) fn present_team(
    runtime: Res<FacRuntime>,
    facility: Res<MutableFacility>,
    mut dots: Query<(&TeamDot, &mut Transform, &mut Sprite)>,
) {
    for (dot, mut transform, mut sprite) in &mut dots {
        if let Some(room) = facility.team_rooms().get(dot.0) {
            let position =
                facility.structure.graph.room_center(*room) + MEMBER_OFFSETS[dot.0 % TEAM_SIZE];
            transform.translation.x = position.x;
            transform.translation.y = position.y;
            sprite.color = if dot.0 == runtime.selected_member {
                MEMBER_COLOR.mix(&Color::WHITE, 0.45)
            } else {
                MEMBER_COLOR
            };
        }
    }
}

fn connection_color(facility: &MutableFacility, door: DoorId) -> Color {
    if facility.structure.is_protected(door) {
        GOLD
    } else if facility.structure.graph.is_pinned(door) {
        GREEN
    } else {
        CYAN
    }
}

pub(crate) fn draw_debug(
    runtime: Res<FacRuntime>,
    facility: Res<MutableFacility>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }
    let graph = &facility.structure.graph;

    for index in 0..ROOM_COUNT {
        let room = RoomId(index as u32);
        let color = if graph.observed(room) {
            GREEN
        } else {
            Color::srgb(0.22, 0.30, 0.40)
        };
        gizmos.rect_2d(graph.room_center(room), Vec2::splat(ROOM_HALF * 2.0), color);
    }

    // Exit room highlight.
    gizmos.circle_2d(graph.room_center(RoomId(EXIT_ROOM)), ROOM_HALF * 0.7, GREEN);

    for (a, b) in graph.connections() {
        gizmos.line_2d(
            graph.door_position(a),
            graph.door_position(b),
            connection_color(&facility, a),
        );
    }
    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        if graph.is_sealed(door) {
            gizmos.circle_2d(
                graph.door_position(door),
                4.0,
                Color::srgba(0.5, 0.55, 0.6, 0.6),
            );
        }
    }

    // Power cell rides with the lead member.
    let cell = graph.room_center(facility.cell_room()) + Vec2::new(0.0, 54.0);
    gizmos.circle_2d(cell, 11.0, GOLD);
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    runtime: Res<'w, FacRuntime>,
    facility: Res<'w, MutableFacility>,
    dots: Query<'w, 's, (), With<TeamDot>>,
    ui_roots: Query<'w, 's, (), With<FacUiRoot>>,
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

    let facility = &*context.facility;
    let rooms: Vec<u32> = facility.team_rooms().iter().map(|r| r.0).collect();

    let dots = context.dots.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = dots == TEAM_SIZE && ui_roots == 1;

    let mut text = context.text.into_inner();
    **text = format!(
        "MUTABLE FACILITY  {}\n\
         objective       {}\n\
         team at exit    {} / {}\n\
         cell in room    {}\n\
         exit reachable  {}\n\
         spine steps     {}\n\
         decoherences    {}\n\
         team rooms      {:?}\n\
         members {dots}  UI {ui_roots}   resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if facility.objective_complete {
            "COMPLETE"
        } else {
            "in progress"
        },
        facility.at_exit(),
        TEAM_SIZE,
        facility.cell_room().0,
        if facility.connected() { "yes" } else { "NO" },
        facility.steps,
        facility.decohere_count,
        rooms,
        context.runtime.reset_count,
        facility.last_event,
    );
}
