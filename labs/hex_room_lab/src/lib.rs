//! **hex_room_lab** — how several hex tiles combine into one cohesive room.
//!
//! For a chosen multi-cell room blueprint, this lab selects the per-cell authored
//! tile for each footprint cell (exactly as the geometry projector does), assembles
//! them at their lattice origins, and stages the production district lighting rig over
//! the result. A fixed camera orbits the room so the whole assembly is captured from
//! every side. Pressing `1`–`9` switches architecture register; `Tab` cycles the room
//! blueprint; the camera auto-orbits.
//!
//! `OBSERVED2_CAPTURE=<dir>` walks every register in turn, orbiting each and writing one
//! PNG per orbit step (`<register>_<NNN>.png`), then exits — an orbit turntable per
//! district for lighting/tile review.
//!
//! Findings here are meant to transfer to `game/src/hex_wfc/view/` as parameters, the
//! same discipline `lighting_lab` established for the atmosphere palette.

use std::f32::consts::TAU;
use std::path::{Path, PathBuf};

use bevy::app::AppExit;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use observed_authoring::{CompiledTileCatalog, Manifest, TilePrototype};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{blueprint_cell_archetype, blueprint_for_role};
use observed_facility::map_spec::RoomRole;
use observed_hex::{HexCoord, PortSignature, TILE_LEVEL_HEIGHT, hex_origin};
use observed_style::{self as style};
use rapier3d::prelude::{SharedShape, Vector as RapierVector};

/// Room blueprints worth showcasing — every multi-cell role, so the assembly always
/// combines more than one tile. `Tab` cycles this list.
const SHOWCASE_ROLES: [RoomRole; 5] = [
    RoomRole::DecoherenceFork, // 2×2, the most room-like footprint
    RoomRole::Decision,        // triangular three-cell
    RoomRole::DualStation,     // two-cell lateral
    RoomRole::AnchorCheckpoint,
    RoomRole::GuardianControl, // two-cell vertical (atrium)
];

/// Orbit steps captured per register — a full 360° turntable.
const ORBIT_FRAMES: usize = 24;
/// Restores the production key trim (`full_wfc` proven range); mirrors the game rig so
/// the lab predicts the shipped look.
const KEY_INTENSITY_SCALE: f32 = 0.62;
const TILE_FILL_INTENSITY: f32 = 240_000.0;
const TILE_FILL_RANGE: f32 = 12.0;
const TILE_FILL_HEIGHT: f32 = 5.6;
/// Anchor well inside `u16` space so every footprint offset stays non-negative.
const ANCHOR: HexCoord = HexCoord {
    q: 24,
    r: 24,
    level: 0,
};

/// Everything a rebuild spawns — geometry and lights — so a register/role switch clears
/// exactly this set.
#[derive(Component)]
struct RoomEntity;

#[derive(Component)]
struct OrbitCamera;

#[derive(Component)]
struct OverlayText;

#[derive(Resource)]
struct Corpus(Vec<TilePrototype>);

#[derive(Resource)]
struct LabState {
    register: usize,
    role: usize,
    orbit_angle: f32,
    center: Vec3,
    radius: f32,
    height: f32,
    dirty: bool,
}

impl LabState {
    fn register(&self) -> ArchitectureRegister {
        ArchitectureRegister::ALL[self.register]
    }

    fn role(&self) -> RoomRole {
        SHOWCASE_ROLES[self.role]
    }
}

pub fn run() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Observed 2 — Hex Room Lab (tile composition + district lighting)".to_string(),
            resolution: WindowResolution::new(1440, 900),
            present_mode: PresentMode::AutoVsync,
            resizable: true,
            ..default()
        }),
        ..default()
    }))
    .insert_resource(Corpus(load_corpus()))
    .insert_resource(LabState {
        register: 0,
        role: 0,
        orbit_angle: 0.0,
        center: Vec3::ZERO,
        radius: 40.0,
        height: 18.0,
        dirty: true,
    })
    .add_systems(Startup, setup)
    .add_systems(Update, (input, rebuild, orbit, overlay).chain());

    if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
        let _ = std::fs::create_dir_all(&dir);
        app.insert_resource(CaptureRun {
            dir,
            register: 0,
            frame: 0,
            phase: CapturePhase::Stage,
            next_at: 0.0,
        })
        .add_systems(Update, capture_progress.after(orbit));
    }

    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        OrbitCamera,
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: 0.08,
            ..Bloom::NATURAL
        },
        DistanceFog {
            color: Color::srgb(0.01, 0.015, 0.03),
            falloff: FogFalloff::Linear {
                start: 40.0,
                end: 120.0,
            },
            ..default()
        },
        Msaa::Off,
        Transform::from_xyz(0.0, 18.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Orbit camera"),
    ));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(14),
                top: px(12),
                padding: UiRect::all(px(12)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.9)),
            BorderColor::all(Color::srgba(0.3, 0.9, 1.0, 0.55)),
            GlobalZIndex(20),
            Name::new("Hex Room Lab overlay"),
        ))
        .with_children(|root| {
            root.spawn((
                OverlayText,
                Text::new("…"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.94, 1.0)),
            ));
        });
}

fn input(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    capture: Option<Res<CaptureRun>>,
    mut state: ResMut<LabState>,
) {
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (index, key) in DIGITS.iter().enumerate() {
        if keyboard.just_pressed(*key) {
            state.register = index;
            state.dirty = true;
        }
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        state.role = (state.role + 1) % SHOWCASE_ROLES.len();
        state.dirty = true;
    }
    // Auto-orbit only when not being driven by the capture turntable.
    if capture.is_none() {
        state.orbit_angle = (state.orbit_angle + time.delta_secs() * 0.35) % TAU;
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    corpus: Res<Corpus>,
    existing: Query<Entity, With<RoomEntity>>,
    mut state: ResMut<LabState>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let register = state.register();
    let role = state.role();
    let palette = style::architecture(register);
    commands.insert_resource(GlobalAmbientLight {
        color: palette.ambient_color,
        brightness: palette.ambient_brightness,
        ..default()
    });
    commands.insert_resource(ClearColor(palette.fog_color));

    // Neutral structural material: the lab showcases the LIGHTING and the tile seams,
    // so albedo stays out of the way (a mid grey the district lights tint).
    let surface = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.52, 0.55),
        perceptual_roughness: 0.92,
        ..default()
    });

    let blueprint = blueprint_for_role(role);
    let mut origins = Vec::new();
    for (index, &offset) in blueprint.cells.iter().enumerate() {
        let coord = HexCoord {
            q: (i32::from(ANCHOR.q) + offset.0) as u16,
            r: (i32::from(ANCHOR.r) + offset.1) as u16,
            level: (i32::from(ANCHOR.level) + offset.2) as u8,
        };
        let origin = Vec3::from_array(hex_origin(coord));
        origins.push(origin);

        let Some(archetype) = blueprint_cell_archetype(role, index) else {
            continue;
        };
        let signature = blueprint.cell_signature(offset);
        match select_tile(&corpus.0, archetype, register, signature) {
            Some(tile) => {
                for hull in &tile.hulls {
                    // Dollhouse cutaway: drop ceiling slabs so the orbit sees into the
                    // room and the key light reaches the interior — the whole point of
                    // the lab is judging interior tile composition, not the roof.
                    if is_ceiling(hull) {
                        continue;
                    }
                    let Some(mesh) = hull_mesh(hull) else {
                        continue;
                    };
                    commands.spawn((
                        RoomEntity,
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(surface.clone()),
                        Transform::from_translation(origin),
                        Name::new(format!("Room tile {archetype}")),
                    ));
                }
            }
            None => warn!(
                "hex_room_lab: no tile for {archetype} / {} / {signature:?}",
                register.slug()
            ),
        }

        // Tier-2 per-tile fill (mirrors the game rig): a tinted omni so every cell of
        // the room is lit, not just the one under the key.
        commands.spawn((
            RoomEntity,
            PointLight {
                color: palette.light_color,
                intensity: TILE_FILL_INTENSITY,
                range: TILE_FILL_RANGE,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(origin + Vec3::Y * TILE_FILL_HEIGHT),
            Name::new("Tile fill"),
        ));
    }

    // Frame the assembled room.
    let count = origins.len().max(1) as f32;
    let center =
        origins.iter().copied().sum::<Vec3>() / count + Vec3::Y * (TILE_LEVEL_HEIGHT * 0.45);
    let extent = origins
        .iter()
        .map(|origin| (*origin - center).length())
        .fold(0.0_f32, f32::max);
    state.center = center;
    state.radius = extent + 26.0;
    state.height = (extent + 26.0) * 0.7;

    // Tier-1 district key: the one shadow-casting source, staged above the room.
    commands.spawn((
        RoomEntity,
        SpotLight {
            color: palette.key_color,
            intensity: palette.key_intensity * KEY_INTENSITY_SCALE,
            range: palette.key_range,
            radius: palette.key_radius,
            inner_angle: palette.key_inner_angle,
            outer_angle: palette.key_outer_angle,
            shadows_enabled: palette.key_shadows_enabled,
            ..default()
        },
        Transform::from_translation(center + Vec3::new(3.0, 22.0, 3.0)).looking_at(center, Vec3::Z),
        Name::new("District key"),
    ));
}

fn orbit(state: Res<LabState>, mut camera: Query<&mut Transform, With<OrbitCamera>>) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    let position = state.center
        + Vec3::new(
            state.radius * state.orbit_angle.cos(),
            state.height,
            state.radius * state.orbit_angle.sin(),
        );
    *transform = Transform::from_translation(position).looking_at(state.center, Vec3::Y);
}

fn overlay(state: Res<LabState>, mut text: Query<&mut Text, With<OverlayText>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    **text = format!(
        "HEX ROOM LAB\nregister {}/9  [{}]\nroom  [{:?}]  ({} tiles)\n\n1–9 register · Tab room · auto-orbit",
        state.register + 1,
        state.register().slug(),
        state.role(),
        blueprint_for_role(state.role()).cells.len(),
    );
}

// --- tile selection + meshing (mirrors the geometry projector) ----------------

/// Deterministically pick the authored tile for `(archetype, register, signature)` — the
/// lowest variant, matching the projector's sorted candidate order.
fn select_tile<'a>(
    corpus: &'a [TilePrototype],
    archetype: &str,
    register: ArchitectureRegister,
    signature: PortSignature,
) -> Option<&'a TilePrototype> {
    corpus
        .iter()
        .filter(|tile| {
            tile.key.archetype == archetype
                && tile.key.register == register.slug()
                && tile.signature == signature
        })
        .min_by_key(|tile| tile.key.variant)
}

/// A hull sitting entirely in the top slab of the cell — the ceiling. Dropped for the
/// dollhouse cutaway. Walls (which reach the floor) and floors keep a low point, so only
/// true ceiling/roof slabs match.
fn is_ceiling(hull: &[Vec3]) -> bool {
    hull.iter().all(|point| point.y >= TILE_LEVEL_HEIGHT - 1.5)
}

fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let points: Vec<_> = hull
        .iter()
        .map(|point| RapierVector::new(point.x, point.y, point.z))
        .collect();
    let shape = SharedShape::convex_hull(&points)?;
    let (vertices, indices) = shape.as_convex_polyhedron()?.to_trimesh();
    let positions: Vec<[f32; 3]> = vertices
        .iter()
        .map(|point| [point.x, point.y, point.z])
        .collect();
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices.into_iter().flatten().collect()))
        .with_duplicated_vertices()
        .with_computed_flat_normals(),
    )
}

// --- corpus (mirrors the game's authoring loader) -----------------------------

fn tile_dir() -> PathBuf {
    let cwd = PathBuf::from("assets/tiles");
    if cwd.exists() {
        return cwd;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
}

fn load_corpus() -> Vec<TilePrototype> {
    let base = tile_dir();
    let mut cells = Manifest::load(&base.join("manifest.ron"))
        .expect("hex tile manifest loads")
        .load_tiles(&base)
        .expect("hex tile prototypes validate");
    if let Ok(text) = std::fs::read_to_string(base.join("compiled_catalog.ron"))
        && let Ok(compiled) = CompiledTileCatalog::from_ron(&text)
    {
        let slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
        if let Ok(strict) = compiled.runtime_catalog(&slugs) {
            cells.extend(strict.cells);
        }
    }
    cells
}

// --- capture turntable --------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum CapturePhase {
    Stage,
    Settle,
    Shoot,
    Done,
}

#[derive(Resource)]
struct CaptureRun {
    dir: String,
    register: usize,
    frame: usize,
    phase: CapturePhase,
    next_at: f32,
}

impl CaptureRun {
    const SETTLE: f32 = 0.6;
    const SHOT: f32 = 0.12;
}

fn capture_progress(
    time: Res<Time>,
    mut run: ResMut<CaptureRun>,
    mut state: ResMut<LabState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    match run.phase {
        CapturePhase::Stage => {
            state.register = run.register;
            state.dirty = true;
            state.orbit_angle = 0.0;
            run.frame = 0;
            run.next_at = now + CaptureRun::SETTLE;
            run.phase = CapturePhase::Settle;
        }
        CapturePhase::Settle if now >= run.next_at => {
            state.orbit_angle = 0.0;
            run.next_at = now + CaptureRun::SHOT;
            run.phase = CapturePhase::Shoot;
        }
        CapturePhase::Shoot if now >= run.next_at => {
            let slug = ArchitectureRegister::ALL[run.register].slug();
            let file = format!("{}/{slug}_{:03}.png", run.dir, run.frame);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(file));
            run.frame += 1;
            if run.frame < ORBIT_FRAMES {
                state.orbit_angle = run.frame as f32 / ORBIT_FRAMES as f32 * TAU;
                run.next_at = now + CaptureRun::SHOT;
            } else if run.register + 1 < ArchitectureRegister::ALL.len() {
                run.register += 1;
                run.phase = CapturePhase::Stage;
            } else {
                run.next_at = now + 1.0;
                run.phase = CapturePhase::Done;
            }
        }
        CapturePhase::Done if now >= run.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
