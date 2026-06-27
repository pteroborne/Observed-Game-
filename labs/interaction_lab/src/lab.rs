use bevy::{ecs::system::SystemParam, prelude::*};
use observed_core::{PlayerId, PlayerIntent};

use crate::{
    engine::{prompt_for_player, tick_interactions},
    input::InteractionInput,
    model::{
        InteractionId, InteractionKind, InteractionWorld, ItemLocation, PLAYER_COUNT, PLAYERS,
    },
};

const PLAYER_COLORS: [Color; PLAYER_COUNT] = [
    Color::srgb(0.20, 0.85, 1.0),
    Color::srgb(1.0, 0.38, 0.30),
    Color::srgb(0.55, 0.95, 0.28),
    Color::srgb(0.82, 0.42, 1.0),
];

#[derive(Component)]
pub(crate) struct InteractionLabOwned;

#[derive(Component)]
pub(crate) struct InteractionLabUiRoot;

#[derive(Component)]
pub(crate) struct InteractionPlayerVisual;

#[derive(Component)]
struct ObjectVisual(InteractionId);

#[derive(Component)]
struct PlayerPrompt(PlayerId);

#[derive(Component)]
struct ObjectStatus(InteractionId);

#[derive(Component)]
struct DebugText;

type DebugTextSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text,
    (
        With<DebugText>,
        Without<PlayerPrompt>,
        Without<ObjectStatus>,
    ),
>;

#[derive(Resource, Clone, Debug)]
pub struct InteractionLabRuntime {
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub scenario_index: usize,
    pub last_seen_event: u32,
    pub last_event: String,
}

impl Default for InteractionLabRuntime {
    fn default() -> Self {
        Self {
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            scenario_index: 0,
            last_seen_event: 0,
            last_event: "Ready. Walk to a fixture and follow its prompt.".to_string(),
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct PresentationContext<'w, 's> {
    world: Res<'w, InteractionWorld>,
    runtime: Res<'w, InteractionLabRuntime>,
    player_visuals: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static mut Transform,
            &'static mut Sprite,
        ),
        With<InteractionPlayerVisual>,
    >,
    object_visuals: Query<
        'w,
        's,
        (
            &'static ObjectVisual,
            &'static mut Transform,
            &'static mut Sprite,
        ),
        Without<InteractionPlayerVisual>,
    >,
    prompts: Query<'w, 's, (&'static PlayerPrompt, &'static mut Text)>,
    statuses: Query<'w, 's, (&'static ObjectStatus, &'static mut Text), Without<PlayerPrompt>>,
    debug_text: DebugTextSingle<'w, 's>,
}

pub(crate) fn setup_lab(mut commands: Commands, world: Res<InteractionWorld>) {
    spawn_world(&mut commands, &world);
    spawn_ui(&mut commands);
}

fn spawn_world(commands: &mut Commands, world: &InteractionWorld) {
    commands
        .spawn((
            InteractionLabOwned,
            Name::new("Interaction Lab World Root"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Sprite::from_color(Color::srgb(0.025, 0.055, 0.08), Vec2::new(1320.0, 700.0)),
                Transform::from_xyz(0.0, 0.0, -3.0),
            ));
            parent.spawn((
                Sprite::from_color(Color::srgba(0.25, 0.7, 0.9, 0.16), Vec2::new(4.0, 650.0)),
                Transform::from_xyz(0.0, 0.0, -2.0),
            ));

            for object in &world.objects {
                parent.spawn((
                    ObjectVisual(object.id),
                    Name::new(object.name),
                    Sprite::from_color(object_color(&object.kind), object_size(&object.kind)),
                    Transform::from_translation(object.position.extend(0.0)),
                ));
            }

            for (index, player) in world.players.iter().enumerate() {
                parent.spawn((
                    player.id,
                    PlayerIntent::default(),
                    InteractionPlayerVisual,
                    Name::new(format!("{} Interaction Probe", player.id.label())),
                    Sprite::from_color(PLAYER_COLORS[index], Vec2::splat(42.0)),
                    Transform::from_translation(player.position.extend(2.0)),
                ));
            }
        });
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            InteractionLabOwned,
            InteractionLabUiRoot,
            Name::new("Interaction Lab UI Root"),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(14)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(430),
                    padding: UiRect::all(px(12)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(7),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.95)),
                BorderColor::all(Color::srgba(0.25, 0.75, 0.95, 0.65)),
            ))
            .with_children(|panel| {
                panel.spawn(text_bundle(
                    "OBSERVED 2 / INTERACTION LAB",
                    23.0,
                    Color::WHITE,
                ));
                panel.spawn((
                    DebugText,
                    Text::new("Interaction diagnostics starting…"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.68, 0.88, 1.0)),
                ));
                for player in PLAYERS {
                    panel.spawn((
                        PlayerPrompt(player),
                        Text::new(format!("{} prompt pending…", player.label())),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(PLAYER_COLORS[player.index()]),
                    ));
                }
                panel.spawn(text_bundle(
                    "P1: WASD / E / Q   •   P2: Arrows / Num1 / Num2\n\
                     T scenario  •  R reset  •  F1 debug",
                    13.0,
                    Color::srgb(0.55, 0.72, 0.85),
                ));
            });

            root.spawn((
                Node {
                    width: px(390),
                    padding: UiRect::all(px(12)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(7),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.95)),
                BorderColor::all(Color::srgba(0.25, 0.75, 0.95, 0.65)),
            ))
            .with_children(|panel| {
                panel.spawn(text_bundle("FIXTURE STATES", 19.0, Color::WHITE));
                for id in 0..7 {
                    panel.spawn((
                        ObjectStatus(InteractionId(id)),
                        Text::new("fixture pending…"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.75, 0.86, 0.95)),
                    ));
                }
            });
        });
}

fn text_bundle(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

pub(crate) fn move_players(
    time: Res<Time>,
    intents: Query<(&PlayerId, &PlayerIntent)>,
    mut world: ResMut<InteractionWorld>,
) {
    for (player, intent) in &intents {
        let speed = if intent.sprint_held { 245.0 } else { 165.0 };
        if let Some(actor) = world.player_mut(*player) {
            actor.position += intent.movement * speed * time.delta_secs();
            actor.position.x = actor.position.x.clamp(-630.0, 630.0);
            actor.position.y = actor.position.y.clamp(-320.0, 320.0);
        }
    }
}

pub(crate) fn simulate_interactions(
    time: Res<Time>,
    intents: Query<(&PlayerId, &PlayerIntent)>,
    mut world: ResMut<InteractionWorld>,
) {
    let frames = intents
        .iter()
        .map(|(player, intent)| (*player, *intent))
        .collect::<Vec<_>>();
    tick_interactions(&mut world, &frames, time.delta_secs().min(0.05));
}

pub(crate) fn handle_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<InteractionLabRuntime>,
    mut world: ResMut<InteractionWorld>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        runtime.scenario_index = (runtime.scenario_index + 1) % 6;
        place_scenario(&mut world, runtime.scenario_index);
        runtime.last_event = format!("Scenario {} positioned P1 and P2.", runtime.scenario_index);
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<InteractionLabRuntime>,
    mut world: ResMut<InteractionWorld>,
    mut input: ResMut<InteractionInput>,
    mut intents: Query<&mut PlayerIntent>,
) {
    if !runtime.reset_requested {
        return;
    }

    runtime.reset_requested = false;
    runtime.reset_count += 1;
    runtime.scenario_index = 0;
    runtime.last_seen_event = 0;
    runtime.last_event = format!("Reset {} restored every fixture.", runtime.reset_count);
    world.reset();
    *input = InteractionInput::default();
    for mut intent in &mut intents {
        *intent = PlayerIntent::default();
    }
}

fn place_scenario(world: &mut InteractionWorld, scenario: usize) {
    let positions = match scenario {
        0 => [Vec2::new(-520.0, -250.0), Vec2::new(-450.0, -250.0)],
        1 => [Vec2::new(-520.0, -80.0), Vec2::new(-250.0, -80.0)],
        2 => [Vec2::new(-25.0, -80.0), Vec2::new(65.0, -80.0)],
        3 => [Vec2::new(275.0, -80.0), Vec2::new(365.0, -80.0)],
        4 => [Vec2::new(-380.0, 190.0), Vec2::new(-80.0, 190.0)],
        _ => [Vec2::new(235.0, 190.0), Vec2::new(315.0, 190.0)],
    };
    for (index, player) in [PlayerId(0), PlayerId(1)].into_iter().enumerate() {
        if let Some(actor) = world.player_mut(player) {
            actor.position = positions[index];
            actor.active_target = None;
        }
    }
    for object in &mut world.objects {
        object.active_users.remove(&PlayerId(0));
        object.active_users.remove(&PlayerId(1));
        match &mut object.kind {
            InteractionKind::TimedControl { progress, .. }
            | InteractionKind::TwoPlayerControl { progress, .. } => *progress = 0.0,
            _ => {}
        }
    }
}

pub(crate) fn update_last_event(
    mut runtime: ResMut<InteractionLabRuntime>,
    world: Res<InteractionWorld>,
) {
    if world.total_events == runtime.last_seen_event {
        return;
    }
    runtime.last_seen_event = world.total_events;
    if let Some(event) = world.recent_events.last() {
        runtime.last_event = event.label();
    }
}

pub(crate) fn present(mut context: PresentationContext) {
    for (player, mut transform, mut sprite) in &mut context.player_visuals {
        if let Some(actor) = context.world.player(*player) {
            transform.translation.x = actor.position.x;
            transform.translation.y = actor.position.y;
            transform.scale = if actor.active_target.is_some() {
                Vec3::splat(1.18)
            } else {
                Vec3::ONE
            };
            sprite.color = if actor.carrying.is_some() {
                Color::srgb(1.0, 0.82, 0.25)
            } else {
                PLAYER_COLORS[player.index()]
            };
        }
    }

    for (visual, mut transform, mut sprite) in &mut context.object_visuals {
        let Some(object) = context.world.object(visual.0) else {
            continue;
        };
        let position = object_position(&context.world, object.id);
        transform.translation.x = position.x;
        transform.translation.y = position.y;
        transform.scale = match &object.kind {
            InteractionKind::Door { open: true, .. } => Vec3::new(0.25, 1.0, 1.0),
            _ => Vec3::ONE,
        };
        sprite.color = object_color(&object.kind);
    }

    for (prompt, mut text) in &mut context.prompts {
        **text = format!(
            "{}: {}",
            prompt.0.label(),
            prompt_for_player(&context.world, prompt.0).text
        );
    }
    for (status, mut text) in &mut context.statuses {
        if let Some(object) = context.world.object(status.0) {
            let users = object
                .active_users
                .iter()
                .map(|player| player.label())
                .collect::<Vec<_>>()
                .join("+");
            **text = format!(
                "{} • {} • users [{}]",
                object.name,
                object.kind.state_label(),
                if users.is_empty() { "—" } else { &users },
            );
        }
    }

    let player_count = context.world.players.len();
    let object_count = context.world.objects.len();
    let healthy = player_count == PLAYER_COUNT && object_count == 7;
    let mut text = context.debug_text.into_inner();
    **text = format!(
        "FRAMEWORK {}  •  reset {}\n\
         players:{player_count} objects:{object_count} events:{}\n\
         scenario:{} debug:{}\n\
         last: {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.runtime.reset_count,
        context.world.total_events,
        context.runtime.scenario_index,
        if context.runtime.debug_visible {
            "ON"
        } else {
            "OFF"
        },
        context.runtime.last_event,
    );
}

pub(crate) fn draw_debug(
    runtime: Res<InteractionLabRuntime>,
    world: Res<InteractionWorld>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    for object in &world.objects {
        gizmos.circle_2d(
            object.position,
            object.radius,
            Color::srgba(0.25, 0.78, 1.0, 0.55),
        );
    }
    for player in &world.players {
        let prompt = prompt_for_player(&world, player.id);
        if let Some(target) = prompt.target
            && let Some(object) = world.object(target)
        {
            gizmos.line_2d(
                player.position,
                object.position,
                Color::srgba(1.0, 0.92, 0.25, 0.7),
            );
        }
        if player.carrying.is_some() {
            gizmos.circle_2d(
                player.position + Vec2::new(0.0, 34.0),
                15.0,
                Color::srgb(1.0, 0.75, 0.18),
            );
        }
    }
}

fn object_position(world: &InteractionWorld, id: InteractionId) -> Vec2 {
    let Some(object) = world.object(id) else {
        return Vec2::ZERO;
    };
    if let InteractionKind::Carryable { location, .. } = object.kind {
        match location {
            ItemLocation::Ground(position) => position,
            ItemLocation::Carried(player) => {
                world.player(player).map_or(object.position, |actor| {
                    actor.position + Vec2::new(0.0, 34.0)
                })
            }
            ItemLocation::Socketed(socket_id) => world
                .objects
                .iter()
                .find(|candidate| {
                    matches!(
                        candidate.kind,
                        InteractionKind::EquipmentSocket {
                            socket,
                            ..
                        } if socket == socket_id
                    )
                })
                .map_or(object.position, |socket| socket.position),
        }
    } else {
        object.position
    }
}

fn object_size(kind: &InteractionKind) -> Vec2 {
    match kind {
        InteractionKind::Lever { .. } => Vec2::new(34.0, 72.0),
        InteractionKind::Door { .. } => Vec2::new(78.0, 130.0),
        InteractionKind::TimedControl { .. } => Vec2::new(82.0, 82.0),
        InteractionKind::TwoPlayerControl { .. } => Vec2::new(150.0, 76.0),
        InteractionKind::Carryable { .. } => Vec2::splat(34.0),
        InteractionKind::EquipmentSocket { .. } => Vec2::new(86.0, 58.0),
        InteractionKind::ClimbPoint { .. } => Vec2::new(52.0, 118.0),
    }
}

fn object_color(kind: &InteractionKind) -> Color {
    match kind {
        InteractionKind::Lever { active: true } => Color::srgb(0.35, 1.0, 0.45),
        InteractionKind::Lever { active: false } => Color::srgb(0.42, 0.48, 0.55),
        InteractionKind::Door { open: true, .. } => Color::srgb(0.25, 0.82, 1.0),
        InteractionKind::Door { open: false, .. } => Color::srgb(0.18, 0.38, 0.55),
        InteractionKind::TimedControl { progress, .. } => {
            Color::srgb(0.25 + *progress * 0.25, 0.55, 0.92)
        }
        InteractionKind::TwoPlayerControl { progress, .. } => {
            Color::srgb(0.55, 0.28 + *progress * 0.25, 0.95)
        }
        InteractionKind::Carryable { .. } => Color::srgb(1.0, 0.72, 0.16),
        InteractionKind::EquipmentSocket {
            inserted: Some(_), ..
        } => Color::srgb(0.25, 1.0, 0.62),
        InteractionKind::EquipmentSocket { inserted: None, .. } => Color::srgb(0.12, 0.44, 0.38),
        InteractionKind::ClimbPoint { .. } => Color::srgb(0.85, 0.55, 0.20),
    }
}
