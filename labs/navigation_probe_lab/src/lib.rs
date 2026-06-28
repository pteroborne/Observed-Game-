//! navigation_probe_lab -- Phase A8 of the Bevy asset-integration roadmap.
//!
//! It answers one question: **can authored geometry produce useful bot / debug
//! navigation without taking ownership of the facility graph?**
//!
//! The lab keeps a hard split:
//!
//! * [`facility`] is the **authoritative** model -- four rooms, a wall cross, and
//!   four doors. Connectivity is decided here, by graph search over open doors.
//! * [`nav`] is a **derived consumer** -- it builds a `vleue_navigator` navmesh
//!   from the facility's walls + closed-door plugs and routes over it with
//!   polyanya. It reads the facility and never writes back.
//!
//! Toggling a door rebuilds the navmesh, so a closed door becomes a solid
//! obstacle and the route detours or fails. An on-screen `[PASS]/[FAIL]` line and
//! exhaustive tests assert the navmesh never disagrees with the authoritative
//! graph: it respects every closed door, and the room graph stays the source of
//! truth.
//!
//! `vleue_navigator` is used as the navmesh builder + path query only
//! (`NavMesh::from_edge_and_obstacles` + `NavMesh::path`); the auto-updater
//! plugin (built for physics-collider obstacle meshes) is not adopted, mirroring
//! the prior labs' "use the data model, not the part that fights the
//! architecture" stance.

mod facility;
mod nav;
mod threshold;

pub use facility::{
    DOOR_COUNT, DoorId, Facility, ROOM_COUNT, Rect, all_doors, all_rooms, door_label, door_rooms,
};
pub use nav::{NavRoute, build_navmesh, query};
pub use threshold::{RoomThresholdView, RuleAudit, SlotId, ThresholdAssignment, ThresholdState};

use bevy::{
    app::AppExit,
    input::InputSystems,
    math::Vec2,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use observed_core::RoomId;
use observed_style::{MarkerRole, SurfaceRole, marker, surface};
use vleue_navigator::NavMesh;

/// Floor-units-to-pixels for the top-down schematic view.
const SCALE: f32 = 17.0;
const BOT_SPEED: f32 = 9.0;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Which rooms the probe routes between. Start is fixed at A; goal is selectable.
#[derive(Resource)]
pub struct Probe {
    pub start: RoomId,
    pub goal: RoomId,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            start: RoomId(0),
            goal: RoomId(3),
        }
    }
}

/// The derived navigation state, rebuilt whenever the facility changes.
#[derive(Resource, Default)]
pub struct NavWorld {
    pub navmesh: Option<NavMesh>,
    pub route: Option<NavRoute>,
    /// Does the navmesh agree with the authoritative graph for the current query?
    pub agreement: bool,
    /// Set when door state or the goal changes; cleared by `rebuild_nav`.
    pub dirty: bool,
}

/// The debug agent walking the current route -- the "bot" the phase asks for. It
/// consumes the derived route (presentation only); it is never authoritative.
#[derive(Resource)]
pub struct Bot {
    pub pos: Vec2,
    pub leg: usize,
    pub arrived: bool,
}

impl Default for Bot {
    fn default() -> Self {
        Self {
            pos: facility::room_center(RoomId(0)),
            leg: 0,
            arrived: false,
        }
    }
}

#[derive(Resource)]
pub struct LabRuntime {
    pub overlay_visible: bool,
    pub reset_count: u32,
    pub last_event: String,
}

impl Default for LabRuntime {
    fn default() -> Self {
        Self {
            overlay_visible: true,
            reset_count: 0,
            last_event: "Authored facility ready. Probe routing A -> D.".to_string(),
        }
    }
}

#[derive(Resource, Default)]
pub struct ResetRequested(pub bool);

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct NavCamera;

#[derive(Component)]
pub struct NavUiRoot;

/// Tags every entity belonging to the projected scene, so a reset can despawn the
/// scene wholesale and rebuild it without leaks. The camera and UI root are *not*
/// tagged: they persist across resets.
#[derive(Component)]
pub struct LabSpawned;

#[derive(Component)]
pub struct WallSprite;

#[derive(Component)]
pub struct FloorSprite;

#[derive(Component)]
pub struct RoomLabel;

#[derive(Component)]
pub struct DoorMarker(pub DoorId);

#[derive(Component)]
struct OverlayText;

#[derive(Component)]
struct OverlayPanel;

#[derive(Component)]
struct HelpPanel;

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct NavigationProbeLabPlugin;

impl Plugin for NavigationProbeLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Facility>()
            .init_resource::<Probe>()
            .init_resource::<Bot>()
            .init_resource::<ThresholdState>()
            .init_resource::<LabRuntime>()
            .init_resource::<ResetRequested>()
            .insert_resource(NavWorld {
                dirty: true,
                ..default()
            })
            .add_systems(Startup, (setup_camera, setup_scene, setup_ui))
            .add_systems(
                Update,
                (
                    handle_input.after(InputSystems),
                    prepare_reset,
                    rebuild_scene_after_reset,
                    rebuild_nav,
                    advance_bot,
                    update_door_colors,
                    draw_nav,
                    update_overlay,
                )
                    .chain(),
            );
    }
}

/// Builds and runs the windowed lab.
pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(surface(SurfaceRole::Ceiling).base_color))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Navigation Probe Lab (Phase A8)".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(NavigationProbeLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        // Show both probes in the captured frame: close AB so A->D detours through C,
        // then anchor room A so the threshold table visibly collapses to AC.
        app.world_mut()
            .resource_mut::<Facility>()
            .set_open(DoorId(0), false);
        let facility = app.world().resource::<Facility>().clone();
        app.world_mut()
            .resource_mut::<ThresholdState>()
            .lock_room(&facility, RoomId(0));
        app.world_mut().resource_mut::<LabRuntime>().last_event =
            "Capture demo: AB closed, room A anchored to AC, A -> D reroutes through C."
                .to_string();
        app.insert_resource(CaptureRequest { path, frame: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

// ---------------------------------------------------------------------------
// Coordinate helpers
// ---------------------------------------------------------------------------

fn world(p: Vec2, z: f32) -> Vec3 {
    Vec3::new(
        (p.x - facility::FACILITY_W * 0.5) * SCALE,
        (facility::FACILITY_H * 0.5 - p.y) * SCALE,
        z,
    )
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        NavCamera,
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Name::new("Navigation Probe Camera"),
    ));
}

/// (Re)build the static projected scene: floor, wall fills, door markers, and room
/// labels. Despawns any prior `LabSpawned` entities first so resets never leak.
fn setup_scene(mut commands: Commands, existing: Query<Entity, With<LabSpawned>>) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    // Floor backdrop.
    let floor = surface(SurfaceRole::Plain).base_color;
    commands.spawn((
        LabSpawned,
        FloorSprite,
        Sprite::from_color(
            floor,
            Vec2::new(
                facility::FACILITY_W * SCALE + 24.0,
                facility::FACILITY_H * SCALE + 24.0,
            ),
        ),
        Transform::from_translation(world(Vec2::new(15.0, 15.0), 1.0)),
        Name::new("Facility Floor"),
    ));

    // Wall fills (clamped to the facility for display; the navmesh uses the
    // extended rects).
    let wall_color = surface(SurfaceRole::Wall).edge.unwrap_or(Color::WHITE);
    for (i, raw) in facility::base_walls().into_iter().enumerate() {
        let rect = raw.clamped();
        commands.spawn((
            LabSpawned,
            WallSprite,
            Sprite::from_color(wall_color, rect.size() * SCALE),
            Transform::from_translation(world(rect.center(), 2.0)),
            Name::new(format!("Wall {i}")),
        ));
    }

    // Door markers (colour set each frame from facility state).
    for door in all_doors() {
        let gap = facility::door_gap(door);
        commands.spawn((
            LabSpawned,
            DoorMarker(door),
            Sprite::from_color(marker(MarkerRole::Exit).base_color, gap.size() * SCALE),
            Transform::from_translation(world(gap.center(), 3.0)),
            Name::new(format!("Door {}", door_label(door))),
        ));
    }

    // Room labels.
    for room in all_rooms() {
        commands.spawn((
            LabSpawned,
            RoomLabel,
            Text2d::new(facility::room_label(room).to_string()),
            TextFont {
                font_size: 30.0,
                ..default()
            },
            TextColor(Color::srgb(0.75, 0.86, 1.0)),
            Transform::from_translation(world(facility::room_center(room), 20.0)),
            Name::new(format!("Room {} label", facility::room_label(room))),
        ));
    }
}

fn setup_ui(mut commands: Commands) {
    commands
        .spawn((
            NavUiRoot,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
            Name::new("Navigation Probe UI Root"),
        ))
        .with_children(|root| {
            root.spawn((
                OverlayPanel,
                Node {
                    width: px(440),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.03, 0.94)),
                BorderColor::all(Color::srgba(0.4, 0.7, 1.0, 0.6)),
                children![(
                    OverlayText,
                    Text::new("Building navmesh..."),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.93, 1.0)),
                )],
            ));
            root.spawn((
                HelpPanel,
                Node {
                    width: px(380),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.03, 0.94)),
                BorderColor::all(Color::srgba(0.4, 0.7, 1.0, 0.6)),
                children![(
                    Text::new(
                        "NAVIGATION PROBE LAB (Phase A8)\n\
                         R reset - F1 overlay - T lock room A\n\
                         1-4 goal room A/B/C/D\n\
                         Z/X/C/V toggle door AB/AC/BD/CD\n\n\
                         gold line = navmesh route (vleue_navigator)\n\
                         cyan = probe bot / start, green = goal\n\
                         green door = open, red door = closed\n\
                         purple diamond = locked threshold assignment\n\n\
                         The room graph is authoritative; the navmesh\n\
                         is a derived consumer rebuilt on each change.\n\
                         Threshold slots are fixed; assignments collapse.",
                    ),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.93, 1.0)),
                )],
            ));
        });
}

// ---------------------------------------------------------------------------
// Update systems
// ---------------------------------------------------------------------------

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut facility: ResMut<Facility>,
    mut probe: ResMut<Probe>,
    mut nav: ResMut<NavWorld>,
    mut thresholds: ResMut<ThresholdState>,
    mut runtime: ResMut<LabRuntime>,
    mut reset: ResMut<ResetRequested>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        reset.0 = true;
        return;
    }
    if keys.just_pressed(KeyCode::F1) {
        runtime.overlay_visible = !runtime.overlay_visible;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        let locked = thresholds.toggle_room_lock(&facility, RoomId(0));
        runtime.last_event = if locked {
            "Room A anchored: visible threshold assignment table collapsed.".to_string()
        } else {
            "Room A anchor removed: threshold assignments read the live graph again.".to_string()
        };
    }

    let goal_keys = [
        (KeyCode::Digit1, RoomId(0)),
        (KeyCode::Digit2, RoomId(1)),
        (KeyCode::Digit3, RoomId(2)),
        (KeyCode::Digit4, RoomId(3)),
    ];
    for (key, room) in goal_keys {
        if keys.just_pressed(key) && probe.goal != room {
            probe.goal = room;
            nav.dirty = true;
            runtime.last_event = format!("Goal set to room {}.", facility::room_label(room));
        }
    }

    let door_keys = [
        (KeyCode::KeyZ, DoorId(0)),
        (KeyCode::KeyX, DoorId(1)),
        (KeyCode::KeyC, DoorId(2)),
        (KeyCode::KeyV, DoorId(3)),
    ];
    for (key, door) in door_keys {
        if keys.just_pressed(key) {
            facility.toggle(door);
            nav.dirty = true;
            runtime.last_event = format!(
                "Door {} {}.",
                door_label(door),
                if facility.is_open(door) {
                    "opened"
                } else {
                    "closed"
                }
            );
        }
    }
}

fn prepare_reset(
    reset: Res<ResetRequested>,
    mut facility: ResMut<Facility>,
    mut probe: ResMut<Probe>,
    mut nav: ResMut<NavWorld>,
    mut thresholds: ResMut<ThresholdState>,
    mut runtime: ResMut<LabRuntime>,
) {
    if !reset.0 {
        return;
    }
    *facility = Facility::all_open();
    *probe = Probe::default();
    nav.dirty = true;
    thresholds.clear();
    runtime.reset_count += 1;
    runtime.last_event = format!(
        "Reset #{} -- all doors open, probe A -> D.",
        runtime.reset_count
    );
}

fn rebuild_scene_after_reset(
    mut reset: ResMut<ResetRequested>,
    commands: Commands,
    existing: Query<Entity, With<LabSpawned>>,
) {
    if !reset.0 {
        return;
    }
    reset.0 = false;
    setup_scene(commands, existing);
}

fn rebuild_nav(
    mut nav: ResMut<NavWorld>,
    facility: Res<Facility>,
    probe: Res<Probe>,
    mut bot: ResMut<Bot>,
) {
    if !nav.dirty {
        return;
    }
    nav.dirty = false;

    let navmesh = build_navmesh(&facility);
    let route = query(&navmesh, probe.start, probe.goal);

    let graph_reach = facility.graph_reachable(probe.start, probe.goal);
    let walk_ok = route
        .as_ref()
        .is_none_or(|r| facility.open_walk_valid(&r.rooms, probe.start, probe.goal));
    nav.agreement = (route.is_some() == graph_reach) && walk_ok;

    nav.navmesh = Some(navmesh);
    nav.route = route;

    *bot = Bot {
        pos: facility::room_center(probe.start),
        leg: 0,
        arrived: false,
    };
}

fn advance_bot(time: Res<Time>, nav: Res<NavWorld>, mut bot: ResMut<Bot>) {
    let Some(route) = &nav.route else {
        return;
    };
    if bot.arrived || route.waypoints.len() < 2 {
        bot.arrived = true;
        return;
    }
    let mut budget = BOT_SPEED * time.delta_secs();
    while budget > 0.0 && bot.leg + 1 < route.waypoints.len() {
        let target = route.waypoints[bot.leg + 1];
        let to = target - bot.pos;
        let dist = to.length();
        if dist <= budget {
            bot.pos = target;
            bot.leg += 1;
            budget -= dist;
        } else {
            bot.pos += to / dist * budget;
            budget = 0.0;
        }
    }
    if bot.leg + 1 >= route.waypoints.len() {
        bot.arrived = true;
    }
}

fn update_door_colors(facility: Res<Facility>, mut doors: Query<(&DoorMarker, &mut Sprite)>) {
    let open = marker(MarkerRole::Exit).base_color;
    let closed = marker(MarkerRole::Collapse).base_color;
    for (door, mut sprite) in &mut doors {
        sprite.color = if facility.is_open(door.0) {
            open
        } else {
            closed
        };
    }
}

fn draw_nav(
    nav: Res<NavWorld>,
    facility: Res<Facility>,
    probe: Res<Probe>,
    bot: Res<Bot>,
    thresholds: Res<ThresholdState>,
    mut gizmos: Gizmos,
) {
    // Room outlines.
    let room_edge = surface(SurfaceRole::Plain).edge.unwrap_or(Color::WHITE);
    for room in all_rooms() {
        giz_rect(&mut gizmos, facility::room_rect(room), 10.0, room_edge);
    }

    // Start and goal markers.
    giz_diamond(
        &mut gizmos,
        facility::room_center(probe.start),
        1.4,
        marker(MarkerRole::You).base_color,
    );
    giz_diamond(
        &mut gizmos,
        facility::room_center(probe.goal),
        2.0,
        marker(MarkerRole::Exit).base_color,
    );

    // Navmesh route polyline.
    if let Some(route) = &nav.route {
        let path_color = marker(MarkerRole::NextRoom).base_color;
        for pair in route.waypoints.windows(2) {
            gizmos.line(world(pair[0], 12.0), world(pair[1], 12.0), path_color);
        }
    }

    // Threshold assignment markers: every visible assignment gets a diamond at its
    // slot. Purple means a room lock collapsed that relation; green means live.
    for room in all_rooms() {
        let view = thresholds.room_view(&facility, room);
        for assignment in view.assignments {
            let locked = thresholds.relation_locked(room, assignment.target);
            giz_diamond(
                &mut gizmos,
                threshold::slot_position(assignment.slot),
                if locked { 1.15 } else { 0.75 },
                if locked {
                    marker(MarkerRole::Control).base_color
                } else {
                    marker(MarkerRole::Exit).base_color
                },
            );
        }
    }

    // Probe bot.
    giz_square(
        &mut gizmos,
        bot.pos,
        0.7,
        marker(MarkerRole::You).base_color,
    );
}

fn giz_rect(gizmos: &mut Gizmos, rect: facility::Rect, z: f32, color: Color) {
    let r = rect.clamped();
    let a = world(Vec2::new(r.min.x, r.min.y), z);
    let b = world(Vec2::new(r.max.x, r.min.y), z);
    let c = world(Vec2::new(r.max.x, r.max.y), z);
    let d = world(Vec2::new(r.min.x, r.max.y), z);
    gizmos.line(a, b, color);
    gizmos.line(b, c, color);
    gizmos.line(c, d, color);
    gizmos.line(d, a, color);
}

fn giz_diamond(gizmos: &mut Gizmos, center: Vec2, radius: f32, color: Color) {
    let n = world(center + Vec2::new(0.0, radius), 14.0);
    let e = world(center + Vec2::new(radius, 0.0), 14.0);
    let s = world(center + Vec2::new(0.0, -radius), 14.0);
    let w = world(center + Vec2::new(-radius, 0.0), 14.0);
    gizmos.line(n, e, color);
    gizmos.line(e, s, color);
    gizmos.line(s, w, color);
    gizmos.line(w, n, color);
}

fn giz_square(gizmos: &mut Gizmos, center: Vec2, half: f32, color: Color) {
    let rect = facility::Rect::new(
        center.x - half,
        center.y - half,
        center.x + half,
        center.y + half,
    );
    giz_rect(gizmos, rect, 16.0, color);
    // Diagonals so the agent reads as filled even at small size.
    gizmos.line(
        world(Vec2::new(rect.min.x, rect.min.y), 16.0),
        world(Vec2::new(rect.max.x, rect.max.y), 16.0),
        color,
    );
    gizmos.line(
        world(Vec2::new(rect.min.x, rect.max.y), 16.0),
        world(Vec2::new(rect.max.x, rect.min.y), 16.0),
        color,
    );
}

#[allow(clippy::too_many_arguments)]
fn update_overlay(
    nav: Res<NavWorld>,
    facility: Res<Facility>,
    probe: Res<Probe>,
    bot: Res<Bot>,
    thresholds: Res<ThresholdState>,
    runtime: Res<LabRuntime>,
    cameras: Query<(), With<NavCamera>>,
    ui_roots: Query<(), With<NavUiRoot>>,
    walls: Query<(), With<WallSprite>>,
    doors: Query<(), With<DoorMarker>>,
    labels: Query<(), With<RoomLabel>>,
    floors: Query<(), With<FloorSprite>>,
    mut text: Query<&mut Text, With<OverlayText>>,
    mut panel: Query<&mut Visibility, (With<OverlayPanel>, Without<HelpPanel>)>,
    mut help: Query<&mut Visibility, (With<HelpPanel>, Without<OverlayPanel>)>,
) {
    let visibility = if runtime.overlay_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if let Ok(mut p) = panel.single_mut() {
        *p = visibility;
    }
    if let Ok(mut h) = help.single_mut() {
        *h = visibility;
    }

    let entities_ok = cameras.iter().count() == 1
        && ui_roots.iter().count() == 1
        && walls.iter().count() == facility::base_walls().len()
        && doors.iter().count() == DOOR_COUNT
        && labels.iter().count() == ROOM_COUNT
        && floors.iter().count() == 1;
    let threshold_audit = thresholds.audit(&facility);
    let healthy = entities_ok && nav.agreement && threshold_audit.passed();

    let door_line = all_doors()
        .iter()
        .map(|d| {
            format!(
                "{}:{}",
                door_label(*d),
                if facility.is_open(*d) { "open" } else { "shut" }
            )
        })
        .collect::<Vec<_>>()
        .join("  ");

    let nav_rooms = nav
        .route
        .as_ref()
        .map(|r| {
            r.rooms
                .iter()
                .map(|room| facility::room_label(*room).to_string())
                .collect::<Vec<_>>()
                .join(">")
        })
        .unwrap_or_else(|| "unreachable".to_string());
    let nav_len = nav
        .route
        .as_ref()
        .map(|r| format!("{:.1}", r.length))
        .unwrap_or_else(|| "--".to_string());

    let graph_route = facility
        .graph_route(probe.start, probe.goal)
        .map(|route| {
            route
                .iter()
                .map(|room| facility::room_label(*room).to_string())
                .collect::<Vec<_>>()
                .join(">")
        })
        .unwrap_or_else(|| "unreachable".to_string());
    let locked_rooms = thresholds
        .locked_rooms()
        .into_iter()
        .map(|room| facility::room_label(room).to_string())
        .collect::<Vec<_>>()
        .join(",");
    let locked_rooms = if locked_rooms.is_empty() {
        "none".to_string()
    } else {
        locked_rooms
    };
    let threshold_line = all_rooms()
        .into_iter()
        .map(|room| {
            let view = thresholds.room_view(&facility, room);
            let targets = view
                .assigned_targets()
                .into_iter()
                .map(|target| facility::room_label(target).to_string())
                .collect::<Vec<_>>()
                .join("");
            format!(
                "{}:{}{}",
                facility::room_label(room),
                if view.locked { "*" } else { "" },
                if targets.is_empty() {
                    "-".to_string()
                } else {
                    targets
                }
            )
        })
        .collect::<Vec<_>>()
        .join("  ");

    let Ok(mut text) = text.single_mut() else {
        return;
    };
    *text = Text::new(format!(
        "NAVIGATION PROBE  {}\n\
         probe           {} -> {}\n\
         doors           {door_line}\n\
         doors open      {}/{}\n\
         navmesh route   {nav_rooms}  (len {nav_len})\n\
         graph route     {graph_route}\n\
         nav==graph      {}\n\
         thresholds      {}\n\
         locked rooms    {locked_rooms}\n\
         threshold view  {threshold_line}\n\
         bot             {}\n\
         walls {}  doors {}  labels {}\n\
         resets          {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        facility::room_label(probe.start),
        facility::room_label(probe.goal),
        facility.open_count(),
        DOOR_COUNT,
        if nav.agreement { "agree" } else { "DISAGREE" },
        threshold_audit.summary(),
        if bot.arrived { "arrived" } else { "walking" },
        walls.iter().count(),
        doors.iter().count(),
        labels.iter().count(),
        runtime.reset_count,
        runtime.last_event,
    ));
}

// ---------------------------------------------------------------------------
// Screenshot capture
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    frame: u32,
}

fn capture_progress(
    mut request: ResMut<CaptureRequest>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    request.frame += 1;
    if request.frame == 45 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
    } else if request.frame >= 70 {
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
        .add_plugins(NavigationProbeLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_ui_scene_and_a_built_route() {
        let mut app = test_app();
        assert_eq!(count::<NavCamera>(&mut app), 1);
        assert_eq!(count::<NavUiRoot>(&mut app), 1);
        assert_eq!(count::<WallSprite>(&mut app), facility::base_walls().len());
        assert_eq!(count::<DoorMarker>(&mut app), DOOR_COUNT);
        assert_eq!(count::<RoomLabel>(&mut app), ROOM_COUNT);
        assert_eq!(count::<FloorSprite>(&mut app), 1);

        // The first update built the navmesh and a valid A->D route that agrees
        // with the authoritative graph.
        let nav = app.world().resource::<NavWorld>();
        assert!(nav.navmesh.is_some());
        assert!(nav.agreement);
        let route = nav.route.as_ref().expect("A->D route exists");
        assert_eq!(route.rooms.first(), Some(&RoomId(0)));
        assert_eq!(route.rooms.last(), Some(&RoomId(3)));
    }

    #[test]
    fn closing_a_door_in_app_reroutes_and_stays_in_agreement() {
        let mut app = test_app();
        // Close AB (the demo door) and rebuild.
        app.world_mut()
            .resource_mut::<Facility>()
            .set_open(DoorId(0), false);
        app.world_mut().resource_mut::<NavWorld>().dirty = true;
        app.update();

        let nav = app.world().resource::<NavWorld>();
        assert!(
            nav.agreement,
            "navmesh still agrees with graph after reroute"
        );
        let route = nav.route.as_ref().expect("A->D still reachable via C");
        assert_eq!(route.rooms, vec![RoomId(0), RoomId(2), RoomId(3)]);
    }

    #[test]
    fn isolating_the_start_makes_the_route_unreachable_in_agreement() {
        let mut app = test_app();
        {
            let mut facility = app.world_mut().resource_mut::<Facility>();
            facility.set_open(DoorId(0), false); // AB
            facility.set_open(DoorId(1), false); // AC
        }
        app.world_mut().resource_mut::<NavWorld>().dirty = true;
        app.update();

        let nav = app.world().resource::<NavWorld>();
        assert!(nav.route.is_none(), "A is isolated -> no navmesh route");
        assert!(
            nav.agreement,
            "unreachable on both sides is still agreement"
        );
    }

    #[test]
    fn threshold_lock_blocks_new_outbound_and_inbound_assignments_in_app() {
        let mut app = test_app();
        {
            let mut facility = app.world_mut().resource_mut::<Facility>();
            facility.set_open(DoorId(1), false); // hide AC before A collapses
        }
        {
            let facility = app.world().resource::<Facility>().clone();
            app.world_mut()
                .resource_mut::<ThresholdState>()
                .lock_room(&facility, RoomId(0));
        }

        {
            let mut facility = app.world_mut().resource_mut::<Facility>();
            facility.set_open(DoorId(0), false); // AB live edge disappears
            facility.set_open(DoorId(1), true); // AC live edge appears
        }
        app.update();

        let facility = app.world().resource::<Facility>();
        let thresholds = app.world().resource::<ThresholdState>();
        let a = thresholds.room_view(facility, RoomId(0));
        let b = thresholds.room_view(facility, RoomId(1));
        let c = thresholds.room_view(facility, RoomId(2));

        assert_eq!(
            a.assigned_targets(),
            vec![RoomId(1)],
            "room A remains collapsed to the threshold it had when locked"
        );
        assert!(
            b.assigned_targets().contains(&RoomId(0)),
            "B keeps the reciprocal pinned assignment back to locked A"
        );
        assert!(
            !c.assigned_targets().contains(&RoomId(0)),
            "C cannot grow a new inbound assignment into locked A"
        );
        assert!(thresholds.audit(facility).passed());
    }

    #[test]
    fn reset_rebuilds_the_scene_without_leaking_entities() {
        let mut app = test_app();
        // Perturb: close doors, change goal, then reset.
        {
            let mut facility = app.world_mut().resource_mut::<Facility>();
            facility.set_open(DoorId(0), false);
            facility.set_open(DoorId(2), false);
        }
        app.world_mut().resource_mut::<Probe>().goal = RoomId(1);
        {
            let facility = app.world().resource::<Facility>().clone();
            app.world_mut()
                .resource_mut::<ThresholdState>()
                .lock_room(&facility, RoomId(0));
        }
        app.world_mut().resource_mut::<NavWorld>().dirty = true;
        app.update();

        let baseline_walls = count::<WallSprite>(&mut app);
        for expected in 1..=3 {
            app.world_mut().resource_mut::<ResetRequested>().0 = true;
            app.update();
            assert_eq!(count::<WallSprite>(&mut app), baseline_walls);
            assert_eq!(count::<DoorMarker>(&mut app), DOOR_COUNT);
            assert_eq!(count::<RoomLabel>(&mut app), ROOM_COUNT);
            assert_eq!(count::<FloorSprite>(&mut app), 1);
            assert_eq!(count::<NavCamera>(&mut app), 1);
            assert_eq!(count::<NavUiRoot>(&mut app), 1);

            let world = app.world();
            assert_eq!(world.resource::<LabRuntime>().reset_count, expected);
            // Reset restored all-open doors and the default A->D probe.
            assert_eq!(world.resource::<Facility>(), &Facility::all_open());
            assert_eq!(world.resource::<Probe>().goal, RoomId(3));
            assert!(world.resource::<ThresholdState>().locked_rooms().is_empty());
            assert!(world.resource::<NavWorld>().agreement);
        }
    }

    #[test]
    fn the_bot_walks_the_route_to_the_goal() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = test_app();
        // Advance time by a fixed step each frame so the bot moves deterministically
        // (real wall-clock deltas under MinimalPlugins are near zero).
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            16,
        )));
        for _ in 0..600 {
            app.update();
            if app.world().resource::<Bot>().arrived {
                break;
            }
        }
        let bot = app.world().resource::<Bot>();
        assert!(bot.arrived, "bot should reach the goal along the route");
        let goal = facility::room_center(RoomId(3));
        assert!(
            bot.pos.distance(goal) < 0.5,
            "bot ends at the goal room centre: {:?}",
            bot.pos
        );
    }
}
