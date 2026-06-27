use bevy::{ecs::system::SystemParam, prelude::*};
use climbing_lab::{ClimbBody, ClimbConfig, ClimbMode, step_body};
use observed_core::{PlayerId, PlayerIntent};

use crate::facility::{
    Facility, GOAL_X, ItemKind, LADDER_X, LEDGE_TOP, PIT_MAX, PIT_MIN, PLAYER_COUNT, WORLD_MAX_X,
};

const PLAYER_COLORS: [Color; PLAYER_COUNT] = [
    Color::srgb(0.30, 0.85, 1.0),
    Color::srgb(1.0, 0.42, 0.34),
    Color::srgb(0.62, 1.0, 0.36),
    Color::srgb(0.86, 0.46, 1.0),
];

const SPAWNS: [Vec2; PLAYER_COUNT] = [
    Vec2::new(80.0, 34.0),
    Vec2::new(140.0, 34.0),
    Vec2::new(200.0, 34.0),
    Vec2::new(260.0, 34.0),
];

#[derive(Component)]
pub struct SandboxOwned;

#[derive(Component)]
pub struct PlayerBody(pub PlayerId);

#[derive(Component)]
pub struct ItemSprite(pub ItemKind);

#[derive(Component)]
pub struct HudText;

#[derive(Component)]
pub struct SandboxCamera;

#[derive(Resource)]
pub struct SandboxRuntime {
    pub selected_player: PlayerId,
    pub spectator: bool,
    pub debug_visible: bool,
}

impl Default for SandboxRuntime {
    fn default() -> Self {
        Self {
            selected_player: PlayerId(0),
            spectator: false,
            debug_visible: true,
        }
    }
}

#[derive(Resource, Default)]
pub struct HumanInput {
    pub movement: Vec2,
    pub jump_queued: bool,
}

#[derive(Resource, Default)]
pub struct SandboxBuilt(pub bool);

/// Spawn the players, item sprites, and HUD. Idempotent via `SandboxBuilt`.
pub fn build_sandbox(commands: &mut Commands, built: &mut SandboxBuilt) {
    if built.0 {
        return;
    }
    built.0 = true;

    for (index, spawn) in SPAWNS.into_iter().enumerate() {
        commands.spawn((
            SandboxOwned,
            PlayerBody(PlayerId(index as u16)),
            ClimbBody::new(spawn, ClimbMode::Free { grounded: false }),
            Sprite::from_color(PLAYER_COLORS[index], Vec2::new(28.0, 60.0)),
            Transform::from_translation(spawn.extend(5.0)),
            Name::new(format!("Player {}", index + 1)),
        ));
    }

    for (kind, color) in [
        (ItemKind::Battery, Color::srgb(0.98, 0.84, 0.25)),
        (ItemKind::Jack, Color::srgb(0.75, 0.78, 0.85)),
    ] {
        commands.spawn((
            SandboxOwned,
            ItemSprite(kind),
            Sprite::from_color(color, Vec2::splat(24.0)),
            Transform::from_translation(Vec3::new(0.0, -1000.0, 4.0)),
            Name::new(kind.label()),
        ));
    }

    commands
        .spawn((
            SandboxOwned,
            HudRoot,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(14)),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(720),
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.92)),
                BorderColor::all(Color::srgba(0.4, 0.8, 1.0, 0.6)),
                children![(
                    HudText,
                    Text::new("Facility online."),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.82, 0.92, 1.0)),
                )],
            ));
        });
}

#[derive(Component)]
struct HudRoot;

/// Despawn everything the sandbox owns and reset the model.
pub fn teardown_sandbox(
    mut commands: Commands,
    mut built: ResMut<SandboxBuilt>,
    mut facility: ResMut<Facility>,
    mut runtime: ResMut<SandboxRuntime>,
    mut input: ResMut<HumanInput>,
    owned: Query<Entity, With<SandboxOwned>>,
) {
    for entity in &owned {
        commands.entity(entity).despawn();
    }
    built.0 = false;
    facility.reset();
    *runtime = SandboxRuntime::default();
    *input = HumanInput::default();
}

pub fn sample_input(keyboard: Res<ButtonInput<KeyCode>>, mut input: ResMut<HumanInput>) {
    let x = axis(&keyboard, KeyCode::KeyD, KeyCode::KeyA)
        + axis(&keyboard, KeyCode::ArrowRight, KeyCode::ArrowLeft);
    let y = axis(&keyboard, KeyCode::KeyW, KeyCode::KeyS)
        + axis(&keyboard, KeyCode::ArrowUp, KeyCode::ArrowDown);
    input.movement = Vec2::new(x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0));
    input.jump_queued |= keyboard.just_pressed(KeyCode::Space);
}

pub fn step_players(
    time: Res<Time<Fixed>>,
    facility: Res<Facility>,
    runtime: Res<SandboxRuntime>,
    mut input: ResMut<HumanInput>,
    mut bodies: Query<(&PlayerBody, &mut ClimbBody)>,
) {
    let world = facility.build_world();
    let config = ClimbConfig::default();
    let dt = time.delta_secs();

    // The human drives the selected player; the rest mirror its locomotion so the
    // squad moves together. Equipment actions act only on the selected player.
    let human = PlayerIntent {
        movement: input.movement,
        jump_pressed: std::mem::take(&mut input.jump_queued),
        ..default()
    };

    for (body, mut climb) in &mut bodies {
        let intent = if body.0 == runtime.selected_player {
            human
        } else {
            // Escort: follow the selected player horizontally, share the jump/climb.
            PlayerIntent {
                movement: human.movement,
                jump_pressed: human.jump_pressed,
                ..default()
            }
        };
        step_body(&mut climb, intent, &world, config, dt);
    }
}

pub fn gameplay_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<SandboxRuntime>,
    mut facility: ResMut<Facility>,
    bodies: Query<(&PlayerBody, &ClimbBody)>,
) {
    for (key, player) in [
        (KeyCode::Digit1, PlayerId(0)),
        (KeyCode::Digit2, PlayerId(1)),
        (KeyCode::Digit3, PlayerId(2)),
        (KeyCode::Digit4, PlayerId(3)),
    ] {
        if keyboard.just_pressed(key) {
            runtime.selected_player = player;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        runtime.spectator = !runtime.spectator;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }

    let Some(position) = selected_position(&bodies, runtime.selected_player) else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyE) {
        facility.pick_up(runtime.selected_player, position);
    }
    if keyboard.just_pressed(KeyCode::KeyG) {
        facility.drop_item(runtime.selected_player, position);
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        facility.place(runtime.selected_player, position);
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        facility.recover(runtime.selected_player, position);
    }
    if keyboard.just_pressed(KeyCode::KeyT)
        && let Some(room) = facility.room_of(position.x)
    {
        facility.replace_room(room);
    }
}

pub fn update_objective(mut facility: ResMut<Facility>, bodies: Query<(&PlayerBody, &ClimbBody)>) {
    let positions = player_positions(&bodies);
    facility.update_objective(&positions);
}

pub fn present_players(
    runtime: Res<SandboxRuntime>,
    mut bodies: Query<(&PlayerBody, &ClimbBody, &mut Transform, &mut Sprite)>,
) {
    for (body, climb, mut transform, mut sprite) in &mut bodies {
        transform.translation.x = climb.position.x;
        transform.translation.y = climb.position.y;
        let base = PLAYER_COLORS[body.0.index() % PLAYER_COUNT];
        sprite.color = if body.0 == runtime.selected_player {
            base.mix(&Color::WHITE, 0.35)
        } else {
            base
        };
    }
}

pub fn present_items(
    facility: Res<Facility>,
    bodies: Query<(&PlayerBody, &ClimbBody)>,
    mut sprites: Query<(&ItemSprite, &mut Transform)>,
) {
    let positions = player_positions(&bodies);
    for (sprite, mut transform) in &mut sprites {
        let item = match sprite.0 {
            ItemKind::Battery => &facility.battery,
            ItemKind::Jack => &facility.jack,
        };
        let position = facility.item_position(item, &positions);
        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }
}

pub fn follow_camera(
    runtime: Res<SandboxRuntime>,
    bodies: Query<(&PlayerBody, &ClimbBody)>,
    mut camera: Query<(&mut Transform, &mut Projection), With<SandboxCamera>>,
) {
    let Ok((mut transform, mut projection)) = camera.single_mut() else {
        return;
    };
    let (target, scale) = if runtime.spectator {
        (Vec2::new(WORLD_MAX_X * 0.5, 140.0), 3.0)
    } else {
        let position =
            selected_position(&bodies, runtime.selected_player).unwrap_or(Vec2::new(300.0, 120.0));
        (position + Vec2::new(0.0, 70.0), 1.05)
    };
    transform.translation.x = lerp(transform.translation.x, target.x, 0.12);
    transform.translation.y = lerp(transform.translation.y, target.y, 0.12);
    if let Projection::Orthographic(ortho) = &mut *projection {
        ortho.scale = lerp(ortho.scale, scale, 0.12);
    }
}

pub fn draw_scene(runtime: Res<SandboxRuntime>, facility: Res<Facility>, mut gizmos: Gizmos) {
    if !runtime.debug_visible {
        return;
    }

    let world = facility.build_world();
    for solid in &world.solids {
        gizmos.rect_2d(
            solid.center,
            solid.half_size * 2.0,
            Color::srgb(0.30, 0.42, 0.55),
        );
    }
    // Ladder.
    gizmos.line_2d(
        Vec2::new(LADDER_X, 0.0),
        Vec2::new(LADDER_X, LEDGE_TOP),
        Color::srgb(0.32, 0.95, 0.60),
    );
    // Room dividers + goal.
    for room in &facility.rooms {
        gizmos.line_2d(
            Vec2::new(room.x_max, -40.0),
            Vec2::new(room.x_max, 320.0),
            Color::srgba(0.4, 0.5, 0.65, 0.4),
        );
    }
    gizmos.line_2d(
        Vec2::new(GOAL_X, -40.0),
        Vec2::new(GOAL_X, 360.0),
        Color::srgb(0.4, 1.0, 0.6),
    );
    // Pit edges.
    for edge in [PIT_MIN, PIT_MAX] {
        gizmos.line_2d(
            Vec2::new(edge, 0.0),
            Vec2::new(edge, -120.0),
            Color::srgba(1.0, 0.4, 0.3, 0.6),
        );
    }
    // Socket marker.
    gizmos.circle_2d(
        crate::facility::SOCKET_POS,
        14.0,
        if facility.door_open() {
            Color::srgb(0.4, 1.0, 0.5)
        } else {
            Color::srgb(1.0, 0.8, 0.3)
        },
    );
}

#[derive(SystemParam)]
pub struct HudContext<'w, 's> {
    facility: Res<'w, Facility>,
    runtime: Res<'w, SandboxRuntime>,
    bodies: Query<'w, 's, (&'static PlayerBody, &'static ClimbBody)>,
    owned: Query<'w, 's, (), With<SandboxOwned>>,
    text: Single<'w, 's, &'static mut Text, With<HudText>>,
}

pub fn update_hud(context: HudContext) {
    let facility = &*context.facility;
    let positions = player_positions(&context.bodies);

    let room_label = |x: f32| {
        facility
            .room_of(x)
            .map(|r| format!("R{}", r.0))
            .unwrap_or_else(|| "--".to_string())
    };
    let player_rooms: Vec<String> = positions.iter().map(|p| room_label(p.x)).collect();
    let battery_room = room_label(facility.item_position(&facility.battery, &positions).x);

    let map = format!(
        "[R0]-[R1{ladder}]-[R2{socket}]{door}[R3{pit}]-[R4{goal}]",
        ladder = "^",
        socket = if facility.door_open() { "*" } else { "o" },
        door = if facility.door_open() { "::" } else { "|" },
        pit = if facility.pit_bridged { "=" } else { "~" },
        goal = "#",
    );

    let objective = if facility.objective_complete {
        "COMPLETE"
    } else {
        "in progress"
    };

    let carried = facility
        .carried_by(context.runtime.selected_player)
        .map(|kind| kind.label())
        .unwrap_or("-");

    let owned = context.owned.iter().count();
    let mut text = context.text.into_inner();
    **text = format!(
        "FACILITY SANDBOX   objective: {objective}\n\
         MAP  {map}\n\
         milestones  door:{}  bridge:{}  squad@goal:{}  cell@goal:{}\n\
         selected {} ({}) carrying:{}   spectator:{}\n\
         players in {}   power cell in {}   replacements {}   owned entities {}\n\
         move WASD | jump Space | climb: hold W on ladder | E pick | C place | X recover | G drop\n\
         1-4 select | V spectator cam | T replace room | F1 debug | Esc pause\n\
         {}",
        yn(facility.door_opened),
        yn(facility.pit_bridged),
        yn(positions.iter().all(|p| p.x >= GOAL_X)),
        yn(facility.item_position(&facility.battery, &positions).x >= GOAL_X),
        context.runtime.selected_player.label(),
        room_label(
            selected_position(&context.bodies, context.runtime.selected_player)
                .unwrap_or_default()
                .x
        ),
        carried,
        if context.runtime.spectator {
            "on"
        } else {
            "off"
        },
        player_rooms.join(" "),
        battery_room,
        facility.replacements,
        owned,
        facility.last_event,
    );
}

// -- helpers --------------------------------------------------------------

fn player_positions(bodies: &Query<(&PlayerBody, &ClimbBody)>) -> Vec<Vec2> {
    let mut positions = vec![Vec2::ZERO; PLAYER_COUNT];
    for (body, climb) in bodies {
        if let Some(slot) = positions.get_mut(body.0.index()) {
            *slot = climb.position;
        }
    }
    positions
}

fn selected_position(bodies: &Query<(&PlayerBody, &ClimbBody)>, player: PlayerId) -> Option<Vec2> {
    bodies
        .iter()
        .find(|(body, _)| body.0 == player)
        .map(|(_, climb)| climb.position)
}

fn axis(keyboard: &ButtonInput<KeyCode>, positive: KeyCode, negative: KeyCode) -> f32 {
    f32::from(u8::from(keyboard.pressed(positive)))
        - f32::from(u8::from(keyboard.pressed(negative)))
}

fn lerp(current: f32, target: f32, t: f32) -> f32 {
    current + (target - current) * t
}

fn yn(value: bool) -> char {
    if value { 'Y' } else { '-' }
}
