//! Presentation of continuous simulation state. Geometry is projected, never inferred.
use crate::{
    arena::{self, Surface},
    model::{Action, ActorId, Event, Kind, Mode, Outcome, Refusal},
    runtime::Runtime,
};
use bevy::{
    camera::Hdr,
    input::mouse::AccumulatedMouseMotion,
    post_process::bloom::Bloom,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use observed_style::kinetic::{self, Role};
use std::collections::BTreeMap;

#[derive(Component)]
pub struct Scene;
#[derive(Component)]
struct Eye;
#[derive(Component)]
struct Tool;
#[derive(Component)]
enum HudField {
    Summary,
    Status,
    Aim,
    Debug,
}
#[derive(Component)]
struct BridgePart;
#[derive(Component)]
struct PowerPart;
#[derive(Component)]
struct DeviceLabel {
    position: Vec3,
}
#[derive(Component)]
enum Flash {
    Muzzle,
    Impact,
}

#[derive(Component)]
struct ActorVisual(ActorId);
#[derive(Component, Clone, Copy)]
enum UiAction {
    Practice,
    Encounter,
    Reset,
    Pause,
    Debug,
}
#[derive(Resource)]
struct Art {
    cube: Handle<Mesh>,
    materials: Vec<Handle<StandardMaterial>>,
}
const ROLES: [Role; 14] = [
    Role::Floor,
    Role::Wall,
    Role::Catwalk,
    Role::Landing,
    Role::Hazard,
    Role::Minor,
    Role::Prop,
    Role::Target,
    Role::Push,
    Role::Pull,
    Role::Powered,
    Role::Unpowered,
    Role::Text,
    Role::Panel,
];
impl Art {
    fn material(&self, role: Role) -> Handle<StandardMaterial> {
        self.materials[ROLES.iter().position(|r| *r == role).unwrap()].clone()
    }
}
#[derive(Resource, Default)]
struct ViewState {
    generation: Option<u32>,
    actors: BTreeMap<ActorId, Entity>,
    kick: f32,
    pulse: f32,
    impact: Vec3,
    pull: bool,
    message: String,
    message_until: u64,
}
fn color(role: Role) -> Color {
    kinetic::treatment(role).base_color
}
fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ViewState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                crate::evidence::advance,
                buttons,
                input,
                rebuild,
                spawn_actors,
                sync,
                warn_bridge,
                draw,
                events,
                feedback,
                labels,
                hud,
            )
                .chain(),
        );
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let art = Art {
        cube: meshes.add(Cuboid::from_length(1.)),
        materials: ROLES
            .iter()
            .map(|role| {
                let t = kinetic::treatment(*role);
                materials.add(StandardMaterial {
                    base_color: t.base_color,
                    emissive: t.emissive,
                    perceptual_roughness: 0.65,
                    metallic: 0.15,
                    ..default()
                })
            })
            .collect(),
    };
    commands.spawn((
        Eye,
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        Transform::default(),
        children![(
            Tool,
            Mesh3d(art.cube.clone()),
            MeshMaterial3d(art.material(Role::Wall)),
            Transform::from_xyz(0.30, -0.25, -0.65).with_scale(Vec3::new(0.15, 0.15, 0.38)),
            children![(
                Mesh3d(art.cube.clone()),
                MeshMaterial3d(art.material(Role::Powered)),
                Transform::from_xyz(0., 0., -0.55).with_scale(Vec3::new(0.8, 0.35, 0.1)),
            )],
        )],
    ));
    // Camera-local muzzle illumination follows the held tool without moving aim.
    commands.spawn((
        Flash::Muzzle,
        PointLight {
            intensity: 0.,
            range: 3.,
            ..default()
        },
        Transform::default(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 1400.,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-10., 20., 6.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: color(Role::Text),
        brightness: 40.,
        ..default()
    });
    commands.insert_resource(ClearColor(color(Role::Panel)));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            top: px(24),
            ..default()
        },
        children![(
            Text::new("KINETIC / 01\nTHE IMPULSE CHAMBER"),
            font(23.),
            TextColor(color(Role::Text)),
        )],
    ));
    commands.spawn((
        HudField::Summary,
        Text::default(),
        font(16.),
        TextColor(color(Role::Text)),
        Node {
            position_type: PositionType::Absolute,
            right: px(30),
            top: px(24),
            ..default()
        },
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(0),
            top: percent(48),
            width: percent(100),
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![
            (Text::new("+"), font(24.), TextColor(color(Role::Text))),
            (
                HudField::Aim,
                Text::default(),
                font(14.),
                TextColor(color(Role::Text))
            ),
        ],
    ));
    commands.spawn((
        HudField::Status,
        BackgroundColor(color(Role::Panel).with_alpha(0.94)),
        Text::default(),
        font(17.),
        TextColor(color(Role::Text)),
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            bottom: px(112),
            padding: UiRect::all(px(8)),
            ..default()
        },
    ));
    commands.spawn((
        HudField::Debug,
        Text::default(),
        font(13.),
        TextColor(color(Role::Text)),
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            top: px(100),
            ..default()
        },
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            bottom: px(28),
            right: px(30),
            column_gap: px(10),
            align_items: AlignItems::Center,
            ..default()
        },
        children![
            button("1  PRACTICE", UiAction::Practice),
            button("2  ENCOUNTER", UiAction::Encounter),
            button("R  RESET", UiAction::Reset),
            button("P  RESUME / PAUSE", UiAction::Pause),
            button("F3  INSPECT", UiAction::Debug),
        ],
    ));
    commands.spawn((
        Flash::Impact,
        PointLight {
            intensity: 0.,
            range: 5.,
            ..default()
        },
        Transform::default(),
    ));
    for (position, label) in [
        (arena::GENERATOR, "GENERATOR / E"),
        (arena::STATION, "RECHARGE STATION"),
        (arena::PANEL, "BRIDGE CONTROL / E"),
    ] {
        commands.spawn((
            DeviceLabel {
                position: position + Vec3::Y * 1.7,
            },
            Text::new(label),
            font(12.),
            TextColor(color(Role::Text)),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(color(Role::Panel)),
        ));
    }
    commands.insert_resource(art);
}
fn button(label: &str, action: UiAction) -> impl Bundle {
    (
        Button,
        action,
        Node {
            padding: UiRect::axes(px(14), px(12)),
            ..default()
        },
        BackgroundColor(color(Role::Panel)),
        children![(Text::new(label), font(13.), TextColor(color(Role::Text)))],
    )
}
fn apply(action: UiAction, runtime: &mut Runtime) {
    match action {
        UiAction::Practice => runtime.reset(Mode::Practice),
        UiAction::Encounter => runtime.reset(Mode::Encounter),
        UiAction::Reset => {
            let mode = runtime.world.mode;
            let paused = runtime.paused;
            runtime.reset(mode);
            runtime.paused = paused;
        }
        UiAction::Pause => {
            if runtime.paused {
                runtime.paused = false;
            } else {
                runtime.pause();
            }
        }
        UiAction::Debug => runtime.diagnostics = !runtime.diagnostics,
    }
}
fn buttons(
    mut runtime: ResMut<Runtime>,
    mut query: Query<(&Interaction, &UiAction, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, action, mut bg) in &mut query {
        bg.0 = color(if *interaction == Interaction::None {
            Role::Panel
        } else {
            Role::Unpowered
        });
        if *interaction == Interaction::Pressed {
            apply(*action, &mut runtime);
        }
    }
}
fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut runtime: ResMut<Runtime>,
    mut window: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    capture: Option<Res<crate::evidence::Capture>>,
) {
    if capture.is_some() {
        return;
    }
    let Ok((win, mut cursor)) = window.single_mut() else {
        return;
    };
    for (key, action) in [
        (KeyCode::Digit1, UiAction::Practice),
        (KeyCode::Digit2, UiAction::Encounter),
        (KeyCode::KeyR, UiAction::Reset),
        (KeyCode::KeyP, UiAction::Pause),
        (KeyCode::F3, UiAction::Debug),
    ] {
        if keys.just_pressed(key) {
            apply(action, &mut runtime);
        }
    }
    if keys.just_pressed(KeyCode::Escape) || !win.focused {
        runtime.pause();
    }
    if keys.just_pressed(KeyCode::KeyN) && runtime.paused {
        runtime.step_once = true;
    }
    if runtime.world.outcome != Outcome::Playing {
        runtime.pause();
    }
    if runtime.paused {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        return;
    }
    if cursor.grab_mode == CursorGrabMode::None {
        if mouse.just_pressed(MouseButton::Left)
            && win
                .cursor_position()
                .is_some_and(|p| p.y > 100. && p.y < win.height() - 100.)
        {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
        return;
    }
    let axis =
        |positive, negative| f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative));
    runtime.movement.movement = Vec2::new(
        axis(KeyCode::KeyD, KeyCode::KeyA),
        axis(KeyCode::KeyW, KeyCode::KeyS),
    );
    runtime.movement.look += Vec2::new(motion.delta.x, motion.delta.y) * 0.075;
    runtime.movement.sprint_held = keys.pressed(KeyCode::ShiftLeft);
    runtime.movement.jump_pressed |= keys.just_pressed(KeyCode::Space);
    for (pressed, action) in [
        (mouse.just_pressed(MouseButton::Left), Action::Push),
        (mouse.just_pressed(MouseButton::Right), Action::Pull),
        (keys.just_pressed(KeyCode::KeyE), Action::Interact),
    ] {
        if pressed {
            runtime.pending.push_back(action);
        }
    }
}
fn block(commands: &mut Commands, art: &Art, p: Vec3, size: Vec3, role: Role) -> Entity {
    commands
        .spawn((
            Scene,
            Mesh3d(art.cube.clone()),
            MeshMaterial3d(art.material(role)),
            Transform::from_translation(p).with_scale(size),
        ))
        .id()
}
fn rebuild(
    mut commands: Commands,
    runtime: Res<Runtime>,
    art: Res<Art>,
    mut view: ResMut<ViewState>,
    old: Query<Entity, With<Scene>>,
) {
    if view.generation == Some(runtime.generation) {
        return;
    }
    for entity in &old {
        commands.entity(entity).despawn();
    }
    view.actors.clear();
    view.generation = Some(runtime.generation);
    view.kick = 0.;
    view.pulse = 0.;
    view.message.clear();
    view.message_until = 0;
    for solid in &runtime.world.solids {
        let role = match solid.surface {
            Surface::Floor => Role::Floor,
            Surface::Wall => Role::Wall,
            Surface::Catwalk => Role::Catwalk,
            Surface::Landing => Role::Landing,
            Surface::Bridge => Role::Catwalk,
        };
        let entity = block(&mut commands, &art, solid.center, solid.half * 2., role);
        if solid.id == arena::BRIDGE {
            commands.entity(entity).insert(BridgePart);
        }
        if matches!(
            solid.surface,
            Surface::Catwalk | Surface::Landing | Surface::Bridge
        ) {
            for sign in [-1., 1.] {
                let edge = block(
                    &mut commands,
                    &art,
                    solid.center
                        + Vec3::new(sign * (solid.half.x - 0.04), solid.half.y + 0.015, 0.),
                    Vec3::new(0.06, 0.025, solid.half.z * 2.),
                    if solid.surface == Surface::Landing {
                        Role::Landing
                    } else {
                        Role::Hazard
                    },
                );
                if solid.id == arena::BRIDGE {
                    commands.entity(edge).insert(BridgePart);
                }
            }
        }
    }
    // Sparse structural seams and light ribs give metre-scale motion references.
    for x in -7..=0 {
        block(
            &mut commands,
            &art,
            Vec3::new(x as f32 * 2., 0.01, 0.),
            Vec3::new(0.018, 0.015, 24.),
            Role::Wall,
        );
    }
    for z in -5..=5 {
        block(
            &mut commands,
            &art,
            Vec3::new(-7., 0.012, z as f32 * 2.),
            Vec3::new(16., 0.015, 0.018),
            Role::Wall,
        );
    }
    for z in [-10., -2., 6., 10.] {
        block(
            &mut commands,
            &art,
            Vec3::new(-14.8, 2.5, z),
            Vec3::new(0.3, 5., 0.35),
            Role::Catwalk,
        );
        block(
            &mut commands,
            &art,
            Vec3::new(-14.60, 2.4, z),
            Vec3::new(0.06, 2., 0.12),
            Role::Powered,
        );
        commands.spawn((
            Scene,
            PointLight {
                color: color(Role::Powered),
                intensity: 18000.,
                range: 13.,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(-13., 3., z),
        ));
    }
    // Broken edge markings state that an opening is dangerous without hue alone.
    for z in -11..=11 {
        if !(5..=8).contains(&z) {
            block(
                &mut commands,
                &art,
                Vec3::new(0.86, 0.035, z as f32),
                Vec3::new(0.18, 0.035, 0.45),
                Role::Hazard,
            );
        }
    }
    for (p, station) in [
        (arena::GENERATOR, false),
        (arena::STATION, true),
        (arena::PANEL, false),
    ] {
        let entity = block(
            &mut commands,
            &art,
            p + Vec3::new(0., 1.12, 0.),
            Vec3::new(0.85, 0.08, 0.65),
            Role::Powered,
        );
        if p != arena::PANEL {
            commands.entity(entity).insert(PowerPart);
        }
        if station {
            block(
                &mut commands,
                &art,
                p + Vec3::new(0., 0.04, 0.8),
                Vec3::new(1.8, 0.06, 1.8),
                Role::Landing,
            );
        }
    }
}
fn spawn_actors(
    mut commands: Commands,
    runtime: Res<Runtime>,
    art: Res<Art>,
    mut view: ResMut<ViewState>,
) {
    for (&id, actor) in &runtime.world.actors {
        if let std::collections::btree_map::Entry::Vacant(entry) = view.actors.entry(id) {
            let pose = runtime.world.pose(id);
            let size = if actor.kind == Kind::Minor { 1.1 } else { 0.9 };
            let entity = block(
                &mut commands,
                &art,
                pose.position,
                Vec3::splat(size),
                if actor.kind == Kind::Minor {
                    Role::Wall
                } else {
                    Role::Prop
                },
            );
            commands
                .entity(entity)
                .insert(ActorVisual(id))
                .with_children(|parent| {
                    for axis in 0..3 {
                        for u in [-0.51, 0.51] {
                            for v in [-0.51, 0.51] {
                                let mut p = Vec3::ZERO;
                                p[(axis + 1) % 3] = u;
                                p[(axis + 2) % 3] = v;
                                let mut size = Vec3::splat(0.035);
                                size[axis] = 1.0;
                                parent.spawn((
                                    Mesh3d(art.cube.clone()),
                                    MeshMaterial3d(art.material(if actor.kind == Kind::Minor {
                                        Role::Minor
                                    } else {
                                        Role::Unpowered
                                    })),
                                    Transform::from_translation(p).with_scale(size),
                                ));
                            }
                        }
                    }
                    // A dark face with a luminous horizontal pupil distinguishes threats from crates.
                    parent.spawn((
                        Mesh3d(art.cube.clone()),
                        MeshMaterial3d(art.material(Role::Wall)),
                        Transform::from_xyz(0., 0., 0.51).with_scale(Vec3::new(0.65, 0.22, 0.04)),
                    ));
                    if actor.kind == Kind::Minor {
                        parent.spawn((
                            Mesh3d(art.cube.clone()),
                            MeshMaterial3d(art.material(Role::Target)),
                            Transform::from_xyz(0., 0., 0.55)
                                .with_scale(Vec3::new(0.25, 0.06, 0.03)),
                        ));
                    }
                });
            entry.insert(entity);
        }
    }
}
fn sync(
    runtime: Res<Runtime>,
    art: Res<Art>,
    time: Res<Time<Fixed>>,
    mut bodies: Query<(&ActorVisual, &mut Transform, &mut Visibility)>,
    mut camera: Query<&mut Transform, (With<Eye>, Without<ActorVisual>)>,
    mut bridge: Query<&mut Visibility, (With<BridgePart>, Without<ActorVisual>)>,
    mut power: Query<&mut MeshMaterial3d<StandardMaterial>, With<PowerPart>>,
) {
    let alpha = if runtime.paused || runtime.demo_label.is_some() {
        1.
    } else {
        time.overstep_fraction()
    };
    for (actor, mut transform, mut visible) in &mut bodies {
        let Some(a) = runtime.world.actors.get(&actor.0) else {
            continue;
        };
        *visible = if a.alive {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let now = runtime.world.pose(actor.0);
        let old = runtime.previous.get(&actor.0).copied().unwrap_or(now);
        transform.translation = old.position.lerp(now.position, alpha);
        transform.rotation = old.rotation.slerp(now.rotation, alpha);
    }
    for mut transform in &mut camera {
        let w = &runtime.world;
        let mut p = w.player;
        p.position = runtime.previous_player.lerp(p.position, alpha);
        *transform = Transform::from_translation(p.eye(&w.player_config))
            .looking_to(w.player.look_dir(), Vec3::Y);
    }
    for mut visible in &mut bridge {
        *visible = if runtime.world.bridge_present {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut material in &mut power {
        material.0 = art.material(if runtime.world.powered {
            Role::Powered
        } else {
            Role::Unpowered
        });
    }
}
fn warn_bridge(
    runtime: Res<Runtime>,
    art: Res<Art>,
    mut bridge: Query<&mut MeshMaterial3d<StandardMaterial>, With<BridgePart>>,
) {
    if runtime.world.bridge_warning.is_some() {
        let role = if (runtime.world.tick / 10).is_multiple_of(2) {
            Role::Hazard
        } else {
            Role::Catwalk
        };
        for mut material in &mut bridge {
            material.0 = art.material(role);
        }
    }
}
fn draw(runtime: Res<Runtime>, mut gizmos: Gizmos) {
    if let Ok(target) = runtime.world.target() {
        let p = runtime.world.pose(target.id).position;
        gizmos.cube(
            Transform::from_translation(p).with_scale(Vec3::splat(1.24)),
            color(Role::Target),
        );
    }
    if runtime.diagnostics {
        for s in &runtime.world.solids {
            if s.id != arena::BRIDGE || runtime.world.bridge_present {
                gizmos.cube(
                    Transform::from_translation(s.center).with_scale(s.half * 2.),
                    color(Role::Landing),
                );
            }
        }
        let w = &runtime.world;
        gizmos.line(
            w.eye(),
            w.eye() + w.player.look_dir() * w.config.reach,
            color(Role::Target),
        );
        for (&id, a) in &w.actors {
            if a.alive {
                let p = w.pose(id).position;
                gizmos.line(p, p + w.velocity(id) * 0.2, color(Role::Push));
            }
        }
    }
}
fn events(
    mut commands: Commands,
    mut runtime: ResMut<Runtime>,
    mut view: ResMut<ViewState>,
    assets: Res<AssetServer>,
) {
    let tick = runtime.world.tick;
    // Event queue survives FixedUpdate catch-up; no sound/effect is lost between frames.
    for event in runtime.events.drain(..) {
        view.message = match event {
            Event::Fired(action, _, p) => {
                view.kick = 1.;
                view.pulse = 1.;
                view.impact = p;
                view.pull = action == Action::Pull;
                if view.pull {
                    "PULL / impulse applied"
                } else {
                    "PUSH / impulse applied"
                }
                .into()
            }
            Event::Refused(r) => format!("NO IMPULSE / {}", refusal(r)),
            Event::Eliminated(_) => "VOID / minor removed".into(),
            Event::Power(on) => format!("GENERATOR / {}", if on { "ON" } else { "OFF" }),
            Event::Retracting => "BRIDGE / retracting in 2 seconds".into(),
            Event::Retracted => "BRIDGE / support removed".into(),
            Event::Wave(n) => format!("WAVE {n} / incoming"),
            Event::Ended(outcome) => format!("{outcome:?} / reset to try again"),
            Event::Recharge => "TOOL / fully charged".into(),
        };
        let path = match event {
            Event::Fired(Action::Pull, ..) => "sounds/reroute.ogg",
            Event::Fired(..) => "sounds/tool_interact.ogg",
            Event::Eliminated(_) => "sounds/collapse_sting.ogg",
            Event::Refused(_) => "sounds/ui_hover.ogg",
            Event::Ended(_) => "sounds/escape.ogg",
            _ => "sounds/ui_click.ogg",
        };
        commands.spawn((
            Scene,
            AudioPlayer::new(assets.load(path)),
            PlaybackSettings::DESPAWN,
        ));
        view.message_until = tick + 150;
    }
}
fn feedback(
    time: Res<Time>,
    runtime: Res<Runtime>,
    mut view: ResMut<ViewState>,
    mut tool: Query<&mut Transform, (With<Tool>, Without<Flash>)>,
    mut lights: Query<(&Flash, &mut PointLight, &mut Transform), Without<Tool>>,
    mut gizmos: Gizmos,
) {
    let dt = if runtime.demo_label.is_some() {
        observed_traversal::FIXED_DT
    } else {
        time.delta_secs()
    };
    view.kick = (view.kick - dt * 7.).max(0.);
    view.pulse = (view.pulse - dt * 5.).max(0.);
    for mut t in &mut tool {
        t.translation = Vec3::new(
            0.30,
            -0.25,
            -0.65 + view.kick * 0.10 * if view.pull { -1. } else { 1. },
        );
    }
    let signal = color(if view.pull { Role::Pull } else { Role::Push });
    for (flash, mut light, mut transform) in &mut lights {
        light.color = signal;
        light.intensity = view.pulse * 3000.;
        transform.translation = match flash {
            Flash::Muzzle => runtime.world.eye() + runtime.world.player.look_dir() * 0.6,
            Flash::Impact => view.impact,
        };
    }
    if view.pulse > 0. {
        let r = 0.1 + (1. - view.pulse) * 0.3;
        gizmos.sphere(Isometry3d::from_translation(view.impact), r, signal);
    }
}
fn labels(
    runtime: Res<Runtime>,
    camera: Query<(&Camera, &GlobalTransform), With<Eye>>,
    mut labels: Query<(&DeviceLabel, &mut Node)>,
) {
    let Ok((camera, transform)) = camera.single() else {
        return;
    };
    for (label, mut node) in &mut labels {
        if transform.translation().distance(label.position) < 13.
            && runtime
                .world
                .line_clear(transform.translation(), label.position)
            && let Ok(point) = camera.world_to_viewport(transform, label.position)
        {
            node.display = Display::Flex;
            node.left = px(point.x - 65.);
            node.top = px(point.y);
        } else {
            node.display = Display::None;
        }
    }
}

fn refusal(r: Refusal) -> &'static str {
    match r {
        Refusal::Empty => "no target",
        Refusal::Blocked => "structure blocks the ray",
        Refusal::TooFar => "out of reach",
        Refusal::Cooldown => "tool recovering",
        Refusal::EmptyCharge => "recharge at the station",
    }
}
fn hud(runtime: Res<Runtime>, view: Res<ViewState>, mut texts: Query<(&mut Text, &HudField)>) {
    let w = &runtime.world;
    for (mut text, field) in &mut texts {
        if matches!(field, HudField::Summary) {
            text.0 = format!(
                "{}  /  {}\n{}  /  GENERATOR {}\nWAVE {}/3    REMOVED {}",
                if w.mode == Mode::Practice {
                    "PRACTICE"
                } else {
                    "ENCOUNTER"
                },
                if runtime.paused { "PAUSED" } else { "LIVE" },
                if w.mode == Mode::Practice {
                    "CHARGE UNLIMITED".into()
                } else {
                    format!("CHARGE {:03.0}", w.charge)
                },
                if w.powered { "ON" } else { "OFF" },
                w.wave,
                w.kills
            );
        }
        if matches!(field, HudField::Status) {
            text.0 = if runtime.paused {
                format!(
                    "{}\nP to resume, then click the chamber to look.\nWASD move  /  Shift sprint  /  Space jump  /  LMB push  /  RMB pull  /  E operate",
                    if w.outcome == Outcome::Playing {
                        "COMMIT THE IMPULSE. LET THE ROOM DO THE REST."
                    } else {
                        match w.outcome {
                            Outcome::Cleared => "ENCOUNTER CLEARED",
                            Outcome::Captured => "CAPTURED",
                            _ => "LOST TO VOID",
                        }
                    }
                )
            } else {
                format!(
                    "{}\n{}\nSTRIPES: unsafe edge   /   LOWER DECK: survivable landing   /   BRACKETS: selected body",
                    w.interaction()
                        .map(|s| format!("E  {s}"))
                        .unwrap_or_else(|| "LMB PUSH / RMB PULL   /   Esc pause".into()),
                    if let Some(label) = runtime.demo_label {
                        format!(
                            "{label}{}",
                            w.bridge_warning
                                .map(|ticks| format!(" / {:.1}s", ticks as f32 / 60.))
                                .unwrap_or_default()
                        )
                    } else if let Some(left) = w.bridge_warning {
                        format!("BRIDGE RETRACTS IN {:.1}s", left as f32 / 60.)
                    } else if view.message_until > w.tick {
                        view.message.clone()
                    } else if w.mode == Mode::Practice {
                        "Safe practice. Reset restores all targets and the bridge.".into()
                    } else if !w.actors.values().any(|a| a.alive && a.kind == Kind::Minor) {
                        format!("Next wave in {:.1}s", w.wave_delay as f32 / 60.)
                    } else {
                        "Walls stagger. Void eliminates. Station restores charge.".into()
                    }
                )
            };
        }
        if matches!(field, HudField::Aim) {
            text.0 = match w.fire_ready() {
                Ok(t) => format!(
                    "[ {} ]  {:.1} m",
                    if w.actors[&t.id].kind == Kind::Minor {
                        "MINOR"
                    } else {
                        "CRATE"
                    },
                    t.distance
                ),
                Err(r) => refusal(r).into(),
            };
        }
        if matches!(field, HudField::Debug) {
            text.0 = if runtime.diagnostics {
                format!(
                    "SIM 60 Hz / tick {} / digest {:016x}\nvelocity {:?} / grounded {}\n{} rigid bodies / {} colliders\n{}\nN: single tick while paused",
                    w.tick,
                    w.digest(),
                    w.player.velocity,
                    w.player.grounded,
                    w.physics.bodies.len(),
                    w.physics.colliders.len(),
                    w.actors
                        .values()
                        .filter(|a| a.alive)
                        .map(|a| format!(
                            "{} {:?}: {:?} / stagger {}",
                            a.id.0, a.kind, a.behavior, a.stagger
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            } else {
                String::new()
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, input::InputPlugin};
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), InputPlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_resource::<Runtime>()
            .init_resource::<ViewState>()
            .add_systems(Startup, setup)
            .add_systems(Update, (buttons, rebuild, spawn_actors).chain());
        app.update();
        app
    }
    #[test]
    fn ten_ui_resets_rebuild_all_entities_without_leaking_children() {
        let mut app = app();
        let baseline = app.world_mut().query::<Entity>().iter(app.world()).count();
        for _ in 0..10 {
            let mut q = app.world_mut().query::<(&UiAction, &mut Interaction)>();
            for (action, mut interaction) in q.iter_mut(app.world_mut()) {
                if matches!(action, UiAction::Reset) {
                    *interaction = Interaction::Pressed;
                }
            }
            app.update();
            assert_eq!(
                app.world_mut().query::<Entity>().iter(app.world()).count(),
                baseline
            );
            let runtime = app.world().resource::<Runtime>();
            assert_eq!(runtime.world.tick, 0);
            assert!(runtime.pending.is_empty());
            let mut q = app.world_mut().query::<(&UiAction, &mut Interaction)>();
            for (_, mut interaction) in q.iter_mut(app.world_mut()) {
                *interaction = Interaction::None;
            }
            app.update();
        }
    }
    #[test]
    fn all_event_audio_files_exist() {
        for name in [
            "reroute",
            "tool_interact",
            "collapse_sting",
            "ui_hover",
            "escape",
            "ui_click",
        ] {
            assert!(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../../assets/sounds/{name}.ogg"))
                    .is_file()
            );
        }
    }
}
