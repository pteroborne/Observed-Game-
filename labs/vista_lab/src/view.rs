//! The first-person view: draw what the pure core says, walk it with the production
//! controller, and let the eye leave the body to see the whole thing from outside.
//!
//! Controls: WASD / mouse walk, Shift run, Space jump; F fly (Space / Ctrl rise and
//! sink); 1–7 authored vantages; T walks the tour; R back to the Bastion; Backspace rebuilds the lab;
//! F1 HUD and legend; F3 exposure survey overlay; Esc frees the cursor, click grabs it.
mod hud;
mod sky;

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::Hdr;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow, WindowResolution};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexCoord, HexFace};
use observed_hex::{FLOOR_SLAB_TOP, TILE_LEVEL_HEIGHT};
use observed_style::open_air::{
    SkyRole, SurveyRole, moon, open_air, sky as sky_color, survey, toward_moon,
};
use observed_style::{
    ArchitectureSurfaceRole, MarkerRole, SurfaceRole, Treatment, architecture,
    architecture_practical_fixture, architecture_surface, architecture_tactical, hex_shell_look,
    marker, surface,
};
use observed_traversal::ConvexRenderMesh;
use player_input::PlayerIntent;

use crate::composition::{UNSAFE_FROM_LEVEL, Vantage, Vista};
use crate::geometry::{Build, Look, Shape, build, face_mid, origin, yaw_toward};
use crate::walk::{STEP, Walker};
use observed_facility::hex_wfc::exposure::{Exposure, Form, Overhang, survey as survey_cells};

/// Everything the lab spawns carries this, so a rebuild can prove it removed it all.
#[derive(Component)]
pub struct VistaEntity;

#[derive(Component)]
pub struct VistaCamera;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    /// A body on the production controller.
    Walk,
    /// A free eye, for seeing the vista from outside.
    Fly,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyPose {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

/// The lab: the authored vista, what the exposure survey said about it, what was
/// built from that, and the body walking it.
#[derive(Resource)]
pub struct Lab {
    pub vista: Vista,
    pub exposures: Vec<Exposure>,
    pub build: Build,
    pub walker: Walker,
    pub mode: ViewMode,
    pub fly: FlyPose,
    pub look_delta: Vec2,
    pub hud: bool,
    pub survey_overlay: bool,
    pub falls: u32,
    /// A transient line for the HUD, and how long it has left.
    pub note: Option<(String, f32)>,
    /// The floor height the body last stood on, to notice a fall that lands.
    pub last_ground: f32,
    /// Walk the tour instead of reading the keyboard.
    pub touring: bool,
    /// The capture owns the clock: the fixed-step body does not advance on its own.
    pub manual_clock: bool,
    pub rebuild: bool,
}

impl Lab {
    #[must_use]
    pub fn new() -> Self {
        let vista = Vista::authored();
        let exposures = survey_cells(&vista.world, UNSAFE_FROM_LEVEL);
        let build = build(&vista, &exposures);
        let walker = Walker::touring(&vista, &build);
        let last_ground = walker.feet().y;
        Self {
            fly: FlyPose {
                position: walker.body.position,
                yaw: walker.body.yaw,
                pitch: 0.0,
            },
            vista,
            exposures,
            build,
            walker,
            mode: ViewMode::Walk,
            look_delta: Vec2::ZERO,
            hud: true,
            survey_overlay: false,
            falls: 0,
            note: None,
            last_ground,
            touring: false,
            manual_clock: false,
            rebuild: false,
        }
    }

    /// Back to the Bastion terrace, on foot.
    pub fn respawn(&mut self) {
        self.walker = Walker::touring(&self.vista, &self.build);
        self.walker.waypoints.clear();
        self.last_ground = self.walker.feet().y;
        self.mode = ViewMode::Walk;
    }

    /// Start the tour walk from the Bastion.
    pub fn start_tour(&mut self) {
        self.walker = Walker::touring(&self.vista, &self.build);
        self.walker.body.pitch = -0.12;
        self.last_ground = self.walker.feet().y;
        self.mode = ViewMode::Walk;
        self.touring = true;
    }

    /// Put the eye at an authored vantage: on foot if it can be stood at, flying if not.
    pub fn take(&mut self, vantage: &Vantage) {
        let eye = Vec3::from_array(vantage.eye);
        let look = (Vec3::from_array(vantage.look_at) - eye).normalize_or_zero();
        let yaw = yaw_toward(Vec2::new(look.x, look.z));
        let pitch = look.y.clamp(-1.0, 1.0).asin();
        self.touring = false;
        if vantage.standing {
            let config = self.walker.config;
            let spawn = (self.walker.body.spawn, self.walker.body.spawn_yaw);
            self.walker.body = observed_traversal::FpsBody::spawned(
                eye - Vec3::Y * (config.eye_height - config.half_height),
                yaw,
            );
            self.walker.body.pitch = pitch;
            (self.walker.body.spawn, self.walker.body.spawn_yaw) = spawn;
            self.walker.waypoints.clear();
            self.last_ground = self.walker.feet().y;
            self.mode = ViewMode::Walk;
        } else {
            self.fly = FlyPose {
                position: eye,
                yaw,
                pitch,
            };
            self.mode = ViewMode::Fly;
        }
    }

    /// Where the eye is and which way it looks.
    #[must_use]
    pub fn eye(&self) -> (Vec3, Quat) {
        match self.mode {
            ViewMode::Walk => {
                let config = &self.walker.config;
                let body = &self.walker.body;
                (
                    body.position + Vec3::Y * (config.eye_height - config.half_height),
                    look_rotation(body.yaw, body.pitch),
                )
            }
            ViewMode::Fly => (
                self.fly.position,
                look_rotation(self.fly.yaw, self.fly.pitch),
            ),
        }
    }

    pub fn say(&mut self, line: impl Into<String>) {
        self.note = Some((line.into(), 4.0));
    }
}

impl Default for Lab {
    fn default() -> Self {
        Self::new()
    }
}

fn look_rotation(yaw: f32, pitch: f32) -> Quat {
    Quat::from_rotation_y(-yaw) * Quat::from_rotation_x(pitch)
}

/// The built cell a point is inside, if any.
#[must_use]
pub fn cell_at(vista: &Vista, point: Vec3) -> Option<HexCoord> {
    #[allow(clippy::cast_possible_truncation)]
    let level = (point.y / TILE_LEVEL_HEIGHT).floor() as i32;
    #[allow(clippy::cast_possible_truncation)]
    let r0 = (point.z / 12.0).round() as i32;
    let mut best: Option<(f32, HexCoord)> = None;
    for r in r0 - 1..=r0 + 1 {
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        let q0 = ((point.x - 7.0 * r as f32) / 14.0).round() as i32;
        for q in q0 - 1..=q0 + 1 {
            let (Ok(q), Ok(r), Ok(level)) =
                (u16::try_from(q), u16::try_from(r), u8::try_from(level))
            else {
                continue;
            };
            let at = HexCoord { q, r, level };
            let o = origin(at);
            let distance = Vec2::new(o.x - point.x, o.z - point.z).length();
            if best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, at));
            }
        }
    }
    best.map(|(_, at)| at).filter(|at| {
        vista
            .world
            .placements
            .get(at)
            .is_some_and(|p| p.space.built())
    })
}

pub fn run() {
    let mut app = App::new();
    let capture = crate::capture::Capture::from_env();
    let (width, height) = capture
        .as_ref()
        .map_or((1600, 900), crate::capture::Capture::window);
    app.insert_resource(ClearColor(sky_color(SkyRole::Horizon)))
        .insert_resource(Time::<Fixed>::from_hz(f64::from(1.0 / STEP)))
        .init_resource::<Lab>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Vista Lab".to_string(),
                resolution: WindowResolution::new(width, height),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, (setup, spawn_scene, hud::spawn))
        .add_systems(FixedUpdate, step_body)
        .add_systems(
            Update,
            (
                read_input,
                rebuild_scene,
                fly,
                sync_camera,
                sky::follow_camera,
                sky::drift_clouds,
                draw_survey,
                hud::update,
            )
                .chain(),
        );
    if let Some(capture) = capture {
        app.insert_resource(capture)
            .add_systems(Update, crate::capture::progress.before(sync_camera));
    }
    app.run();
}

fn setup(
    mut commands: Commands,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    capture: Option<Res<crate::capture::Capture>>,
    lab: Res<Lab>,
) {
    let palette = open_air(architecture(ArchitectureRegister::Monolith));
    let (eye, rotation) = lab.eye();
    commands.spawn((
        VistaCamera,
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: 0.12,
            ..Bloom::NATURAL
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 72f32.to_radians(),
            far: 1_400.0,
            ..default()
        }),
        DistanceFog {
            color: palette.fog_color,
            falloff: FogFalloff::Linear {
                start: palette.fog_start,
                end: palette.fog_end,
            },
            ..default()
        },
        Transform::from_translation(eye).with_rotation(rotation),
        Name::new("Vista eye"),
    ));
    // The moon: low in the west-south-west, so sheer faces rake into light and shadow.
    commands.spawn((
        DirectionalLight {
            color: moon(),
            illuminance: 3_400.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: 0.3,
            first_cascade_far_bound: 28.0,
            maximum_distance: 320.0,
            overlap_proportion: 0.2,
        }
        .build(),
        // From where the moon hangs in the sky (`open_air::toward_moon`).
        Transform::from_translation(Vec3::from_array(toward_moon()))
            .looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Moon"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: palette.ambient_color,
        brightness: palette.ambient_brightness,
        ..default()
    });
    if capture.is_none()
        && let Ok(mut cursor) = cursors.single_mut()
    {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

/// A gameplay signal at its full treatment. Its glow is HDR emission well above
/// anything the moon or a room can add, so nothing dims it; it is not drawn `unlit`,
/// because Bevy's unlit path discards emission and the fall edge's albedo is dark on
/// purpose — the light is the signal.
fn signal_material(treatment: Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        perceptual_roughness: 0.4,
        ..default()
    }
}

/// Structure: the facility shell's own look for `register`, which is lit, never a
/// light source. Feeding a structural treatment's raw glow straight into a material
/// is what made the first captures read as pale plastic.
fn shell_material(treatment: Treatment, register: ArchitectureRegister) -> StandardMaterial {
    let look = hex_shell_look(&treatment, register);
    StandardMaterial {
        base_color: look.base_color,
        emissive: look.emissive,
        perceptual_roughness: architecture(register).surface_roughness,
        ..default()
    }
}

/// Plain mass with no district: cliffs and the hanging stone under them.
fn mass_material(role: SkyRole) -> StandardMaterial {
    StandardMaterial {
        base_color: sky_color(role),
        perceptual_roughness: 0.95,
        ..default()
    }
}

/// Walkways are the facility's connective tissue, drawn in the connective register.
const WALKWAY_REGISTER: ArchitectureRegister = ArchitectureRegister::Megastructure;

fn material_for(look: Look) -> Option<StandardMaterial> {
    Some(match look {
        Look::Floor(register) => shell_material(
            architecture_surface(register, ArchitectureSurfaceRole::Floor),
            register,
        ),
        Look::Wall(register) => shell_material(
            architecture_surface(register, ArchitectureSurfaceRole::Wall),
            register,
        ),
        Look::Roof(register) => shell_material(
            architecture_surface(register, ArchitectureSurfaceRole::Ceiling),
            register,
        ),
        Look::SheerFace => mass_material(SkyRole::SheerFace),
        Look::Underside | Look::Truss => mass_material(SkyRole::Underside),
        // Practical housings are light sources by definition; their glow is the point.
        Look::Window(register) => {
            let fixture = architecture_practical_fixture(register);
            StandardMaterial {
                base_color: fixture.base_color,
                emissive: fixture.emissive,
                ..default()
            }
        }
        Look::Band(register) => shell_material(architecture_tactical(register), register),
        Look::FallEdge => signal_material(surface(SurfaceRole::GantryEdge)),
        Look::WalkwayDeck => shell_material(surface(SurfaceRole::GantryDeck), WALKWAY_REGISTER),
        Look::Rail => shell_material(surface(SurfaceRole::Wall), WALKWAY_REGISTER),
        Look::Beacon => signal_material(marker(MarkerRole::Exit)),
        Look::Guard => return None,
    })
}

fn hull_mesh(points: &[Vec3]) -> Option<(Mesh, Vec3)> {
    #[allow(clippy::cast_precision_loss)]
    let centroid = points.iter().copied().sum::<Vec3>() / points.len() as f32;
    let local: Vec<Vec3> = points.iter().map(|p| *p - centroid).collect();
    let render = ConvexRenderMesh::from_convex_hull(&local)?;
    if render.indices.is_empty() {
        return None;
    }
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, render.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, render.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, render.uvs)
    .with_inserted_indices(Indices::U32(render.indices));
    Some((mesh, centroid))
}

fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    lab: Res<Lab>,
) {
    let cube = meshes.add(Cuboid::default());
    let mut cache: BTreeMap<Look, Option<Handle<StandardMaterial>>> = BTreeMap::new();
    for piece in &lab.build.pieces {
        let Some(material) = cache
            .entry(piece.look)
            .or_insert_with(|| material_for(piece.look).map(|m| materials.add(m)))
            .clone()
        else {
            continue;
        };
        let (mesh, transform) = match &piece.shape {
            Shape::Block {
                center,
                rotation,
                half,
            } => (
                cube.clone(),
                Transform::from_translation(*center)
                    .with_rotation(*rotation)
                    .with_scale(*half * 2.0),
            ),
            Shape::Hull(points) => {
                let Some((mesh, centroid)) = hull_mesh(points) else {
                    continue;
                };
                (meshes.add(mesh), Transform::from_translation(centroid))
            }
        };
        let mut entity = commands.spawn((
            VistaEntity,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            transform,
        ));
        // Lit signal strips and the beacon cast nothing: they are light, not mass.
        if matches!(piece.look, Look::FallEdge | Look::Beacon | Look::Window(_)) {
            entity.insert(NotShadowCaster);
        }
    }
    for practical in &lab.build.practicals {
        commands.spawn((
            VistaEntity,
            PointLight {
                color: architecture(practical.register).light_color,
                intensity: 350_000.0,
                range: 16.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(practical.position),
        ));
    }
    commands.spawn((
        VistaEntity,
        PointLight {
            color: marker(MarkerRole::Exit).base_color,
            intensity: 3_000_000.0,
            range: 48.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(lab.build.beacon + Vec3::Y * 4.0),
    ));
    sky::spawn(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut images,
        &lab,
    );
}

fn rebuild_scene(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    existing: Query<Entity, With<VistaEntity>>,
) {
    if !lab.rebuild {
        return;
    }
    let before = existing.iter().count();
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let fresh = Lab {
        hud: lab.hud,
        survey_overlay: lab.survey_overlay,
        ..Lab::new()
    };
    *lab = fresh;
    lab.say(format!(
        "Rebuilt: removed {before} lab entities; the vista respawns from its source"
    ));
    commands.queue(|world: &mut World| {
        let _ = world.run_system_cached(spawn_scene);
    });
}

fn read_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    capture: Option<Res<crate::capture::Capture>>,
    mut lab: ResMut<Lab>,
) {
    if capture.is_some() {
        return;
    }
    let grabbed = cursors
        .single()
        .is_ok_and(|cursor| cursor.grab_mode != CursorGrabMode::None);
    if let Ok(mut cursor) = cursors.single_mut() {
        if keyboard.just_pressed(KeyCode::Escape) {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        } else if mouse_buttons.just_pressed(MouseButton::Left) {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
    }
    if grabbed {
        lab.look_delta += motion.delta;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        lab.hud = !lab.hud;
    }
    if keyboard.just_pressed(KeyCode::F3) {
        lab.survey_overlay = !lab.survey_overlay;
    }
    if keyboard.just_pressed(KeyCode::KeyF) {
        let (eye, _) = lab.eye();
        match lab.mode {
            ViewMode::Walk => {
                let (yaw, pitch) = (lab.walker.body.yaw, lab.walker.body.pitch);
                lab.fly = FlyPose {
                    position: eye,
                    yaw,
                    pitch,
                };
                lab.mode = ViewMode::Fly;
            }
            ViewMode::Fly => lab.mode = ViewMode::Walk,
        }
        lab.touring = false;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        lab.touring = false;
        lab.respawn();
        lab.say("Back on the Bastion terrace");
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        lab.start_tour();
        lab.say("Walking the tour: span, isle, flight, the unrailed span, the Needle");
    }
    if keyboard.just_pressed(KeyCode::Backspace) {
        lab.rebuild = true;
    }
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
    ];
    let vantages = Vista::vantages();
    for (key, vantage) in digits.iter().zip(&vantages) {
        if keyboard.just_pressed(*key) {
            lab.take(vantage);
            lab.say(vantage.title);
        }
    }
}

fn walk_intent(keyboard: &ButtonInput<KeyCode>, look: Vec2) -> PlayerIntent {
    let mut movement = Vec2::ZERO;
    for (key, step) in [
        (KeyCode::KeyW, Vec2::Y),
        (KeyCode::KeyS, Vec2::NEG_Y),
        (KeyCode::KeyD, Vec2::X),
        (KeyCode::KeyA, Vec2::NEG_X),
    ] {
        if keyboard.pressed(key) {
            movement += step;
        }
    }
    PlayerIntent {
        movement,
        look,
        jump_pressed: keyboard.just_pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft),
        ..PlayerIntent::default()
    }
}

fn step_body(keyboard: Res<ButtonInput<KeyCode>>, mut lab: ResMut<Lab>) {
    if lab.mode != ViewMode::Walk || lab.manual_clock {
        return;
    }
    if lab.touring {
        lab.walker.step_tour();
        if lab.walker.finished() {
            lab.touring = false;
        }
    } else {
        // Mouse look is intent like everything else; the controller owns the turn.
        let look = lab.look_delta * 0.06;
        lab.look_delta = Vec2::ZERO;
        let intent = walk_intent(&keyboard, look);
        lab.walker.step_with(intent);
    }
    if lab.walker.recovered {
        lab.walker.recovered = false;
        lab.falls += 1;
        lab.last_ground = lab.walker.feet().y;
        lab.say("Fell through every deck into true void. Back to the Bastion.");
        return;
    }
    if lab.walker.body.grounded {
        let here = lab.walker.feet().y;
        let fell = lab.last_ground - here;
        if fell > 4.0 {
            let landmark = cell_at(&lab.vista, lab.walker.feet() + Vec3::Y * 0.2)
                .and_then(|at| lab.vista.landmarks.get(&at))
                .map_or("lower structure", |landmark| landmark.label());
            lab.say(format!("Fell {fell:.0} m and landed on {landmark}"));
        }
        lab.last_ground = here;
    }
}

fn fly(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    capture: Option<Res<crate::capture::Capture>>,
    mut lab: ResMut<Lab>,
) {
    if lab.mode != ViewMode::Fly || capture.is_some() {
        return;
    }
    let look = std::mem::take(&mut lab.look_delta) * 0.0025;
    lab.fly.yaw = (lab.fly.yaw + look.x).rem_euclid(std::f32::consts::TAU);
    lab.fly.pitch = (lab.fly.pitch - look.y).clamp(-1.5, 1.5);
    let rotation = look_rotation(lab.fly.yaw, lab.fly.pitch);
    let mut direction = Vec3::ZERO;
    for (key, step) in [
        (KeyCode::KeyW, rotation * Vec3::NEG_Z),
        (KeyCode::KeyS, rotation * Vec3::Z),
        (KeyCode::KeyD, rotation * Vec3::X),
        (KeyCode::KeyA, rotation * Vec3::NEG_X),
        (KeyCode::Space, Vec3::Y),
        (KeyCode::ControlLeft, Vec3::NEG_Y),
    ] {
        if keyboard.pressed(key) {
            direction += step;
        }
    }
    let speed = if keyboard.pressed(KeyCode::ShiftLeft) {
        60.0
    } else {
        18.0
    };
    lab.fly.position += direction.normalize_or_zero() * speed * time.delta_secs();
}

fn sync_camera(lab: Res<Lab>, mut camera: Query<&mut Transform, With<VistaCamera>>) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    let (eye, rotation) = lab.eye();
    transform.translation = eye;
    transform.rotation = rotation;
}

/// F3: what the exposure survey told the renderer, drawn over the vista.
fn draw_survey(lab: Res<Lab>, mut gizmos: Gizmos) {
    if !lab.survey_overlay {
        return;
    }
    for e in &lab.exposures {
        let o = origin(e.coord);
        let height = match e.form {
            Form::Storey => TILE_LEVEL_HEIGHT,
            Form::Pavilion => crate::geometry::PAVILION_HEIGHT,
            _ => FLOOR_SLAB_TOP,
        };
        for face in HexFace::LATERAL.into_iter().filter(|&f| e.is_sheer(f)) {
            let [a, b] = observed_hex::face_edge(face);
            #[allow(clippy::cast_precision_loss)]
            let (a, b) = (
                o + Vec3::new(a.0 as f32, 0.0, a.1 as f32) * 0.98,
                o + Vec3::new(b.0 as f32, 0.0, b.1 as f32) * 0.98,
            );
            let up = Vec3::Y * height;
            let color = survey(SurveyRole::Sheer);
            gizmos.line(a, b, color);
            gizmos.line(a + up, b + up, color);
            gizmos.line(a, a + up, color);
            gizmos.line(b, b + up, color);
        }
        if let Overhang::Hanging { .. } = e.overhang
            && !matches!(e.form, Form::Span { .. } | Form::Flight { .. })
        {
            let depth = e.overhang.metres().unwrap_or(48.0);
            gizmos.line(o, o - Vec3::Y * depth, survey(SurveyRole::Drop));
        }
        if let Form::Span { axis } = e.form {
            let reach = face_mid(axis);
            let lift = Vec3::Y * (FLOOR_SLAB_TOP + 0.05);
            gizmos.line(o + lift - reach, o + lift + reach, survey(SurveyRole::Span));
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::geometry::floor_point;

    fn headless() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .init_resource::<Lab>();
        app
    }

    fn lab_entities(app: &mut App) -> Vec<Entity> {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<VistaEntity>>()
            .iter(world)
            .collect()
    }

    #[test]
    fn rebuilding_replaces_every_lab_entity_and_leaks_none() {
        let mut app = headless();
        app.world_mut()
            .run_system_once(spawn_scene)
            .expect("scene spawns");
        let first = lab_entities(&mut app);
        assert!(first.len() > 1_000, "{}", first.len());

        app.world_mut().resource_mut::<Lab>().rebuild = true;
        app.world_mut()
            .run_system_once(rebuild_scene)
            .expect("rebuild runs");
        let second = lab_entities(&mut app);
        assert_eq!(second.len(), first.len());
        assert!(
            first
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err()),
            "an entity from before the rebuild survived it"
        );
        assert!(!app.world().resource::<Lab>().rebuild);
    }

    #[test]
    fn only_guards_go_undrawn() {
        let lab = Lab::new();
        for piece in &lab.build.pieces {
            assert_eq!(
                material_for(piece.look).is_none(),
                piece.look == Look::Guard,
                "{:?}",
                piece.look
            );
            if piece.look == Look::Guard {
                assert!(piece.collides, "an invisible piece that stops nothing");
            }
        }
    }

    #[test]
    fn the_hud_finds_the_cell_under_your_feet() {
        let lab = Lab::new();
        for e in lab
            .exposures
            .iter()
            .filter(|e| matches!(e.form, Form::Deck | Form::Span { .. }))
        {
            let feet = floor_point(e.coord) + Vec3::new(1.5, 0.2, -1.0);
            assert_eq!(cell_at(&lab.vista, feet), Some(e.coord));
        }
        let sky = floor_point(crate::composition::c(11, 12, 4)) + Vec3::Y * 0.2;
        assert_eq!(cell_at(&lab.vista, sky), None);
    }
}
