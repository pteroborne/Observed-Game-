use std::collections::BTreeMap;

use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    model::{
        CollisionRect, PortId, PortRef, QuarterTurn, RoomId, RoomRegistry, RoomTemplate, WorldPort,
    },
    world::{ConnectionError, RoomWorld},
};

#[derive(Component)]
pub(crate) struct RoomLabOwned;

#[derive(Component)]
pub(crate) struct RoomLabUiRoot;

#[derive(Component)]
pub(crate) struct RoomVisualRoot(pub RoomId);

#[derive(Component)]
pub(crate) struct RoomOwned(pub RoomId);

#[derive(Component)]
pub(crate) struct RoomVisualRevision(u32);

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct LibraryText;

#[derive(Resource, Clone, Debug)]
pub struct RoomLabRuntime {
    pub selected_room: Option<RoomId>,
    pub selected_port_cursor: usize,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub last_event: String,
}

impl Default for RoomLabRuntime {
    fn default() -> Self {
        Self {
            selected_room: Some(RoomId(0)),
            selected_port_cursor: 0,
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            last_event: "Eight authored templates loaded into a connected facility.".to_string(),
        }
    }
}

type DebugSingle<'w, 's> =
    Single<'w, 's, &'static mut Text, (With<DebugText>, Without<LibraryText>)>;

type LibrarySingle<'w, 's> =
    Single<'w, 's, &'static mut Text, (With<LibraryText>, Without<DebugText>)>;

#[derive(SystemParam)]
pub(crate) struct UiContext<'w, 's> {
    registry: Res<'w, RoomRegistry>,
    world: Res<'w, RoomWorld>,
    runtime: Res<'w, RoomLabRuntime>,
    roots: Query<'w, 's, (), With<RoomVisualRoot>>,
    owned: Query<'w, 's, &'static RoomOwned>,
    ui_roots: Query<'w, 's, (), With<RoomLabUiRoot>>,
    debug: DebugSingle<'w, 's>,
    library: LibrarySingle<'w, 's>,
}

pub(crate) fn setup_ui(mut commands: Commands) {
    commands
        .spawn((
            RoomLabOwned,
            RoomLabUiRoot,
            Name::new("Room Lab UI Root"),
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
                    width: px(420),
                    padding: UiRect::all(px(13)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.95)),
                BorderColor::all(Color::srgba(0.25, 0.75, 0.95, 0.65)),
                children![(
                    DebugText,
                    Text::new("Room diagnostics starting…"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.72, 0.90, 1.0)),
                )],
            ));
            root.spawn((
                Node {
                    width: px(430),
                    padding: UiRect::all(px(13)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.95)),
                BorderColor::all(Color::srgba(0.25, 0.75, 0.95, 0.65)),
                children![(
                    LibraryText,
                    Text::new("Room library loading…"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.80, 0.90, 1.0)),
                )],
            ));
        });
}

pub(crate) fn handle_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    registry: Res<RoomRegistry>,
    mut world: ResMut<RoomWorld>,
    mut runtime: ResMut<RoomLabRuntime>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        select_next_room(&world, &mut runtime);
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        runtime.selected_port_cursor = runtime.selected_port_cursor.wrapping_add(1);
    }

    let Some(selected) = runtime.selected_room else {
        return;
    };
    if keyboard.just_pressed(KeyCode::KeyQ) {
        rotate_selected(&mut world, &mut runtime, selected, QuarterTurn::previous);
    }
    if keyboard.just_pressed(KeyCode::KeyE) {
        rotate_selected(&mut world, &mut runtime, selected, QuarterTurn::next);
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        replace_selected(&registry, &mut world, &mut runtime, selected);
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        attach_to_selected(&registry, &mut world, &mut runtime, selected);
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        despawn_selected(&mut world, &mut runtime, selected);
    }
}

fn select_next_room(world: &RoomWorld, runtime: &mut RoomLabRuntime) {
    let ids = world.rooms.keys().copied().collect::<Vec<_>>();
    if ids.is_empty() {
        runtime.selected_room = None;
        return;
    }
    let next = runtime
        .selected_room
        .and_then(|selected| ids.iter().position(|id| *id == selected))
        .map_or(0, |index| (index + 1) % ids.len());
    runtime.selected_room = Some(ids[next]);
    runtime.selected_port_cursor = 0;
    runtime.last_event = format!("Selected room {}.", ids[next].0);
}

fn rotate_selected(
    world: &mut RoomWorld,
    runtime: &mut RoomLabRuntime,
    selected: RoomId,
    rotate: impl FnOnce(QuarterTurn) -> QuarterTurn,
) {
    let Some(room) = world.room(selected).copied() else {
        return;
    };
    match world.rotate_room(selected, rotate(room.transform.rotation)) {
        Ok(disconnected) => {
            runtime.last_event = format!(
                "Rotated room {} and invalidated {disconnected} connection(s).",
                selected.0
            );
        }
        Err(error) => runtime.last_event = error_label(error),
    }
}

fn replace_selected(
    registry: &RoomRegistry,
    world: &mut RoomWorld,
    runtime: &mut RoomLabRuntime,
    selected: RoomId,
) {
    let Some(room) = world.room(selected).copied() else {
        return;
    };
    let next = room.template.next();
    match world.replace_room(registry, selected, next) {
        Ok(preserved) => {
            runtime.selected_port_cursor = 0;
            runtime.last_event = format!(
                "Replaced room {} with {} and preserved {preserved} connection(s).",
                selected.0,
                next.name()
            );
        }
        Err(error) => runtime.last_event = error_label(error),
    }
}

fn attach_to_selected(
    registry: &RoomRegistry,
    world: &mut RoomWorld,
    runtime: &mut RoomLabRuntime,
    selected: RoomId,
) {
    let Ok(free_ports) = world.free_ports(registry, selected) else {
        return;
    };
    if free_ports.is_empty() {
        runtime.last_event = "Selected room has no free ports.".to_string();
        return;
    }
    let target = free_ports[runtime.selected_port_cursor % free_ports.len()];
    let start = world
        .room(selected)
        .map(|room| room.template.next())
        .unwrap_or(RoomTemplate::StraightCorridor);
    let Some((template, port)) = compatible_template_port(registry, start, target) else {
        runtime.last_event = "No authored template has a compatible port.".to_string();
        return;
    };
    match world.attach_room(registry, target.reference, template, port) {
        Ok(room) => {
            runtime.selected_room = Some(room);
            runtime.selected_port_cursor = 0;
            runtime.last_event = format!(
                "Spawned room {} ({}) aligned to room {}.",
                room.0,
                template.name(),
                selected.0
            );
        }
        Err(error) => runtime.last_event = error_label(error),
    }
}

fn compatible_template_port(
    registry: &RoomRegistry,
    start: RoomTemplate,
    target: WorldPort,
) -> Option<(RoomTemplate, PortId)> {
    let start_index = RoomTemplate::ALL
        .iter()
        .position(|template| *template == start)
        .unwrap_or(0);
    (0..RoomTemplate::ALL.len()).find_map(|offset| {
        let template = RoomTemplate::ALL[(start_index + offset) % RoomTemplate::ALL.len()];
        registry.load(template).and_then(|definition| {
            definition
                .ports
                .iter()
                .find(|port| port.kind == target.kind)
                .map(|port| (template, port.id))
        })
    })
}

fn despawn_selected(world: &mut RoomWorld, runtime: &mut RoomLabRuntime, selected: RoomId) {
    if world.rooms.len() <= 1 {
        runtime.last_event = "Refusing to despawn the final room.".to_string();
        return;
    }
    if world.despawn_room(selected) {
        runtime.selected_room = world.rooms.keys().next().copied();
        runtime.selected_port_cursor = 0;
        runtime.last_event = format!("Despawned room {} and all owned entities.", selected.0);
    }
}

pub(crate) fn perform_reset(
    registry: Res<RoomRegistry>,
    mut world: ResMut<RoomWorld>,
    mut runtime: ResMut<RoomLabRuntime>,
) {
    if !runtime.reset_requested {
        return;
    }
    runtime.reset_requested = false;
    runtime.reset_count += 1;
    runtime.selected_room = Some(RoomId(0));
    runtime.selected_port_cursor = 0;
    runtime.last_event = format!(
        "Reset {} restored the eight-room authored facility.",
        runtime.reset_count
    );
    *world = RoomWorld::authored_facility(&registry);
}

pub(crate) fn sync_room_visuals(
    mut commands: Commands,
    registry: Res<RoomRegistry>,
    world: Res<RoomWorld>,
    roots: Query<(Entity, &RoomVisualRoot, &RoomVisualRevision)>,
) {
    let mut current = BTreeMap::new();
    for (entity, root, revision) in &roots {
        let expected = world.room(root.0).map(|room| room.revision);
        if expected == Some(revision.0) {
            current.insert(root.0, revision.0);
        } else {
            commands.entity(entity).despawn();
        }
    }

    for room in world.rooms.values() {
        if current.get(&room.id) == Some(&room.revision) {
            continue;
        }
        let Some(definition) = registry.load(room.template) else {
            continue;
        };
        let root = commands
            .spawn((
                RoomLabOwned,
                RoomOwned(room.id),
                RoomVisualRoot(room.id),
                RoomVisualRevision(room.revision),
                Name::new(format!("Room {} — {}", room.id.0, room.template.name())),
                Transform {
                    translation: room.transform.translation.extend(0.0),
                    rotation: Quat::from_rotation_z(room.transform.rotation.radians()),
                    ..default()
                },
                Visibility::default(),
            ))
            .id();

        commands.entity(root).with_children(|parent| {
            parent.spawn((
                RoomOwned(room.id),
                Sprite::from_color(room.template.color(), definition.bounds.size),
                Transform::from_xyz(0.0, 0.0, -2.0),
            ));
            for surface in &definition.surfaces {
                parent.spawn((
                    RoomOwned(room.id),
                    Sprite::from_color(Color::srgba(0.025, 0.04, 0.055, 0.92), surface.size),
                    Transform::from_translation(surface.local_center.extend(-1.0)),
                ));
            }
            for port in &definition.ports {
                parent.spawn((
                    RoomOwned(room.id),
                    Sprite::from_color(port.kind.color(), Vec2::splat(13.0)),
                    Transform::from_translation(port.local_position.extend(1.0)),
                ));
            }
        });
    }
}

pub(crate) fn update_ui(context: UiContext) {
    let selected = context
        .runtime
        .selected_room
        .and_then(|id| context.world.room(id));
    let selected_port = selected.and_then(|room| {
        context
            .world
            .free_ports(&context.registry, room.id)
            .ok()
            .and_then(|ports| {
                (!ports.is_empty())
                    .then(|| ports[context.runtime.selected_port_cursor % ports.len()])
            })
    });
    let visual_roots = context.roots.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let unique_owned_rooms = context
        .owned
        .iter()
        .map(|owned| owned.0)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let collisions = context.world.collisions(&context.registry).len();
    let healthy = context.registry.len() == 8
        && visual_roots == context.world.rooms.len()
        && unique_owned_rooms == context.world.rooms.len()
        && ui_roots == 1;

    let mut debug = context.debug.into_inner();
    **debug = format!(
        "ROOM LIFECYCLE {}  •  reset {}\n\
         definitions     {}\n\
         logical rooms   {}\n\
         visual roots    {}\n\
         owned room IDs  {}\n\
         connections     {}\n\
         collisions      {}\n\
         spawned         {}\n\
         despawned       {}\n\
         replacements    {}\n\n\
         selected        {}\n\
         template        {}\n\
         rotation        {:?}\n\
         free port       {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.runtime.reset_count,
        context.registry.len(),
        context.world.rooms.len(),
        visual_roots,
        unique_owned_rooms,
        context.world.connections.len(),
        collisions,
        context.world.spawn_count,
        context.world.despawn_count,
        context.world.replacement_count,
        selected.map_or_else(|| "none".to_string(), |room| room.id.0.to_string()),
        selected.map_or("—", |room| room.template.name()),
        selected.map_or(QuarterTurn::R0, |room| room.transform.rotation),
        selected_port.map_or_else(
            || "none".to_string(),
            |port| format!("{} / {}", port.reference.port.0, port.kind.label())
        ),
        context.runtime.last_event,
    );

    let mut library = context.library.into_inner();
    **library = format!(
        "ROOM VOCABULARY\n{}\n\n\
         CONTROLS\n\
         Tab    select room\n\
         P      select free port\n\
         Q / E  rotate left / right\n\
         C      connect and spawn room\n\
         T      replace selected template\n\
         X      despawn selected room\n\
         R      reset facility\n\
         F1     toggle debug geometry\n\n\
         PORT RULE\n\
         Connections require equal types,\n\
         coincident positions, and opposite facings.",
        RoomTemplate::ALL
            .into_iter()
            .map(|template| format!("• {}", template.name()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

pub(crate) fn draw_debug(
    registry: Res<RoomRegistry>,
    world: Res<RoomWorld>,
    runtime: Res<RoomLabRuntime>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    for room in world.rooms.values() {
        let Some(definition) = registry.load(room.template) else {
            continue;
        };
        let size = room.transform.rotation.rotate_size(definition.bounds.size);
        let selected = runtime.selected_room == Some(room.id);
        gizmos.rect_2d(
            room.transform.translation,
            size,
            if selected {
                Color::srgb(1.0, 0.95, 0.25)
            } else {
                Color::srgba(0.45, 0.85, 1.0, 0.72)
            },
        );
        for port in &definition.ports {
            let reference = PortRef {
                room: room.id,
                port: port.id,
            };
            let Ok(world_port) = world.port(&registry, reference) else {
                continue;
            };
            gizmos.circle_2d(world_port.position, 9.0, world_port.kind.color());
            gizmos.line_2d(
                world_port.position,
                world_port.position + world_port.facing.vector() * 28.0,
                world_port.kind.color(),
            );
        }
    }

    for connection in &world.connections {
        let Ok(a) = world.port(&registry, connection.a) else {
            continue;
        };
        let Ok(b) = world.port(&registry, connection.b) else {
            continue;
        };
        gizmos.line_2d(a.position, b.position, Color::srgb(0.35, 1.0, 0.55));
        gizmos.circle_2d(a.position, 5.0, Color::WHITE);
    }

    for collision in world.collisions(&registry) {
        draw_collision(collision, &mut gizmos);
    }
}

fn draw_collision(collision: CollisionRect, gizmos: &mut Gizmos) {
    gizmos.rect_2d(
        collision.center,
        collision.size,
        Color::srgba(1.0, 0.28, 0.22, 0.52),
    );
}

fn error_label(error: ConnectionError) -> String {
    format!("Connection operation rejected: {error:?}.")
}
