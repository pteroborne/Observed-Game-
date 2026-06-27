//! **door_lab** — doors as the observation gate (feasibility lab).
//!
//! A 2D schematic projection of the pure [`observed_doors`] model: rooms as cells,
//! doors as the links between them. Open a door to *freeze* its connection; close it (a
//! "slam") and the unseen side is free to rewire; the protected spine keeps the exit
//! reachable no matter what. The pure rules — open = frozen, closed = free, rewire
//! only behind closed doors, spine traversability, the reopen-reveals-change loop,
//! and dead-ends that never sever the exit — are unit-tested in [`observed_doors`];
//! this lab is their projection and a place to feel the mechanic.

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use observed_core::RoomId;
use observed_doors::DoorWorld;
use observed_observation::{DOOR_COUNT, DoorId, ROOM_COUNT, Side};

const ROOM_GREY: Color = Color::srgb(0.16, 0.21, 0.30);
const START_COLOR: Color = Color::srgb(0.30, 0.55, 1.0);
const EXIT_COLOR: Color = Color::srgb(0.28, 1.0, 0.52);
const DEAD_END_COLOR: Color = Color::srgb(1.0, 0.34, 0.30);
const SPINE_COLOR: Color = Color::srgb(1.0, 0.80, 0.32);
const OPEN_COLOR: Color = Color::srgb(0.40, 1.0, 0.60);
const CLOSED_COLOR: Color = Color::srgb(0.34, 0.52, 0.80);
const SEALED_COLOR: Color = Color::srgba(0.40, 0.45, 0.52, 0.6);
const PLAYER_COLOR: Color = Color::srgb(0.92, 1.0, 0.96);
const ROOM_SIZE: f32 = 196.0;

#[derive(Component)]
pub(crate) struct DoorCam;

#[derive(Component)]
pub(crate) struct DoorUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct DebugPanel;

#[derive(Component)]
struct HelpPanel;

#[derive(Resource)]
pub struct DoorLab {
    pub world: DoorWorld,
    pub debug_visible: bool,
    pub reset_count: u32,
    pub last_event: String,
}

impl Default for DoorLab {
    fn default() -> Self {
        Self {
            world: DoorWorld::authored(),
            debug_visible: true,
            reset_count: 0,
            last_event: "Open a door (1-4) to freeze it; close it and the unseen side rewires."
                .to_string(),
        }
    }
}

pub struct DoorLabPlugin;

impl Plugin for DoorLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DoorLab>()
            .add_systems(Startup, (setup_camera, setup_ui))
            .add_systems(
                Update,
                (
                    handle_input.after(InputSystems),
                    draw_debug,
                    update_debug_text,
                )
                    .chain(),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        DoorCam,
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 1.5,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Name::new("Door Lab Camera"),
    ));
}

fn setup_ui(mut commands: Commands) {
    commands
        .spawn((
            DoorUiRoot,
            Name::new("Door Lab UI Root"),
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
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Door diagnostics starting…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.80, 0.94, 1.0)),
                )],
            ));
            root.spawn((
                HelpPanel,
                Node {
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.6)),
                children![(
                    Text::new(
                        "DOOR LAB — doors are the observation gate\n\
                         1 2 3 4        Open/close the N E S W door of your room\n\
                         Arrows         Walk through that door (only if open)\n\
                         D              Decohere now (rewire closed, non-spine doors)\n\
                         R              Reset · F1 Toggle debug\n\n\
                         GOLD = protected spine (always frozen, keeps the exit\n\
                         reachable). GREEN = open/observed → frozen. BLUE = closed →\n\
                         free to rewire. Grey ticks = sealed walls. Open a door to lock\n\
                         a route; close it and the far side may change before you\n\
                         reopen it. Blue room = start, green = exit, red = dead-end.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.92, 0.97)),
                )],
            ));
        });
}

fn handle_input(keyboard: Res<ButtonInput<KeyCode>>, mut lab: ResMut<DoorLab>) {
    if keyboard.just_pressed(KeyCode::F1) {
        lab.debug_visible = !lab.debug_visible;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        lab.world.reset();
        lab.reset_count += 1;
        lab.last_event = format!("Reset {} restored the authored facility.", lab.reset_count);
    }
    if keyboard.just_pressed(KeyCode::KeyD) {
        lab.world.decohere();
        let rewires = lab.world.rewires_last;
        lab.last_event = format!("Decohered: {rewires} doors rewired behind closed doors.");
    }

    for (key, side) in [
        (KeyCode::Digit1, Side::North),
        (KeyCode::Digit2, Side::East),
        (KeyCode::Digit3, Side::South),
        (KeyCode::Digit4, Side::West),
    ] {
        if keyboard.just_pressed(key) {
            let door = lab.world.door_id(lab.world.player(), side);
            let was_open = lab.world.is_open(door);
            lab.world.toggle_facing(side);
            lab.last_event = format!(
                "{} door {}: {}.",
                side.label(),
                if was_open {
                    "closed (slam)"
                } else {
                    "opened (frozen)"
                },
                if was_open {
                    "now free to rewire"
                } else {
                    "connection locked"
                },
            );
        }
    }

    for (key, side) in [
        (KeyCode::ArrowUp, Side::North),
        (KeyCode::ArrowRight, Side::East),
        (KeyCode::ArrowDown, Side::South),
        (KeyCode::ArrowLeft, Side::West),
    ] {
        if keyboard.just_pressed(key) {
            if lab.world.traverse(side) {
                let room = lab.world.player().0;
                lab.last_event = format!("Stepped {} into room {room}.", side.label());
            } else {
                lab.last_event = format!("Can't go {} — door closed or sealed.", side.label());
            }
        }
    }
}

fn draw_debug(lab: Res<DoorLab>, mut gizmos: Gizmos) {
    if !lab.debug_visible {
        return;
    }
    let world = &lab.world;

    let dead_ends = world.dead_end_rooms();
    for index in 0..ROOM_COUNT as u32 {
        let room = RoomId(index);
        let color = if room == world.exit() {
            EXIT_COLOR
        } else if room == world.start() {
            START_COLOR
        } else if dead_ends.contains(&room) {
            DEAD_END_COLOR
        } else {
            ROOM_GREY
        };
        gizmos.rect_2d(world.room_center(room), Vec2::splat(ROOM_SIZE), color);
    }

    // Connections (unique pairs, not sealed), coloured by state.
    for index in 0..DOOR_COUNT as u16 {
        let a = DoorId(index);
        let b = world.partner(a);
        if a.0 >= b.0 || world.is_sealed(a) {
            continue;
        }
        let color = connection_color(world, a);
        gizmos.line_2d(world.door_position(a), world.door_position(b), color);
    }

    // Door ticks.
    for index in 0..DOOR_COUNT as u16 {
        let door = DoorId(index);
        let position = world.door_position(door);
        if world.is_sealed(door) {
            gizmos.circle_2d(position, 5.0, SEALED_COLOR);
        } else {
            gizmos.circle_2d(position, 4.0, connection_color(world, door));
        }
    }

    // The player.
    let centre = world.room_center(world.player());
    gizmos.circle_2d(centre, 26.0, PLAYER_COLOR);
    gizmos.circle_2d(centre, 11.0, PLAYER_COLOR);
    // The exit, ringed.
    gizmos.circle_2d(world.room_center(world.exit()), 40.0, EXIT_COLOR);
}

fn connection_color(world: &DoorWorld, door: DoorId) -> Color {
    if world.is_spine(door) {
        SPINE_COLOR
    } else if world.is_open(door) {
        OPEN_COLOR
    } else {
        CLOSED_COLOR
    }
}

fn update_debug_text(
    lab: Res<DoorLab>,
    cams: Query<(), With<DoorCam>>,
    ui_roots: Query<(), With<DoorUiRoot>>,
    mut text: Single<&mut Text, With<DebugText>>,
    mut panel: Single<&mut Visibility, (With<DebugPanel>, Without<HelpPanel>)>,
    mut help: Single<&mut Visibility, (With<HelpPanel>, Without<DebugPanel>)>,
) {
    let visibility = if lab.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **panel = visibility;
    **help = visibility;

    let world = &lab.world;
    let cams = cams.iter().count();
    let ui_roots = ui_roots.iter().count();
    let open_connections = (0..DOOR_COUNT as u16)
        .map(DoorId)
        .filter(|&d| world.is_open(d) && d.0 < world.partner(d).0)
        .count();
    let healthy = cams == 1 && ui_roots == 1 && world.matching_valid() && world.exit_reachable();

    ***text = format!(
        "DOOR MONITOR  {}\n\
         player room        {}\n\
         exit reachable     {}\n\
         matching valid     {}\n\
         open connections   {}\n\
         dead-end rooms     {:?}\n\
         decoherence events {}\n\
         last rewired doors {}\n\
         slams              {}\n\
         reopened-changed   {}\n\
         cameras {cams}  UI {ui_roots}  resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        world.player().0,
        if world.exit_reachable() { "yes" } else { "NO" },
        if world.matching_valid() { "yes" } else { "NO" },
        open_connections,
        world
            .dead_end_rooms()
            .iter()
            .map(|r| r.0)
            .collect::<Vec<_>>(),
        world.decoherence_count,
        world.rewires_last,
        world.slams,
        world.reopened_changed,
        lab.reset_count,
        lab.last_event,
    );
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.010, 0.018, 0.030)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Door Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(DoorLabPlugin);

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
    mut lab: ResMut<DoorLab>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Open a couple of routes (freezing them), then decohere a few times so the
        // contrast — frozen open doors vs. rewired closed ones — is on screen.
        let world = &mut lab.world;
        world.open(world.door_id(RoomId(4), Side::West));
        world.open(world.door_id(RoomId(4), Side::South));
        for _ in 0..5 {
            world.decohere();
        }
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.6 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.4 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DoorLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_one_camera_and_overlay() {
        let mut app = test_app();
        assert_eq!(count::<DoorCam>(&mut app), 1);
        assert_eq!(count::<DoorUiRoot>(&mut app), 1);
    }

    #[test]
    fn reset_restores_the_world_without_leaking() {
        let mut app = test_app();
        for n in 1..=5 {
            {
                let mut lab = app.world_mut().resource_mut::<DoorLab>();
                lab.world.decohere();
                lab.world.decohere();
            }
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyR);
            // Run only Update so PreUpdate's input-clear doesn't wipe the press; then
            // release + clear so the next iteration's press registers as fresh (and
            // the held key can't fire a second reset).
            app.world_mut().run_schedule(Update);
            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.release(KeyCode::KeyR);
                keys.clear();
            }

            assert_eq!(count::<DoorCam>(&mut app), 1);
            assert_eq!(count::<DoorUiRoot>(&mut app), 1);
            let lab = app.world().resource::<DoorLab>();
            assert_eq!(lab.reset_count, n);
            assert_eq!(lab.world.decoherence_count, 0);
            assert!(lab.world.exit_reachable());
        }
    }
}
