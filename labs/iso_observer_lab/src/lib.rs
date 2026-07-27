//! Isometric facility observer — the Arc O Phase 104 instrument.
//!
//! Arc O changes what the hex WFC composes: contiguous districts, per-district
//! composition profiles, an `Expanse` archetype, authored vertical kits. None of
//! that is falsifiable from a first-person camera inside a corridor, so this lab
//! exists to make a whole solved facility legible at a glance. A tile is a
//! pixel: on its own it says almost nothing, so the lab draws what the tiles
//! *compose*, on channels that do not compete:
//!
//! - **Colour is the district.** Every cell is tinted by its district accent
//!   from [`observed_style::architecture`]. The lab never invents a colour.
//! - **Height is the archetype**, and **width is the composition** — both from
//!   [`observed_style::hex_sketch`], the same table the in-game survivor map
//!   reads, so the two views cannot drift. Rooms fill their hex and meet with no
//!   seam, so a multi-cell room reads as one space; corridors are ribbons.
//! - **Bars are connections.** One per open port pair, so a corridor run reads
//!   as a route and a riser shows two floors are joined.
//!
//! Reading them together answers the questions Arc O is accountable for: are
//! districts contiguous, does one register compose differently from its
//! neighbour, and is verticality clustered or sprayed uniformly across the map.
//!
//! This is the lab, not the game: it renders the world directly and shows ground
//! truth. The in-game map promoted from it in Phase 105 reads player knowledge
//! instead and must never see a cell the team has not discovered.

mod capture;
mod prism;

use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::FRAC_PI_4;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexArchetype, HexSpace, HexWfcConfig, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, PortClass, TILE_LEVEL_HEIGHT, hex_origin};
use observed_style::{HexSketch, HexSketchRole, MarkerRole, hex_link, hex_sketch, marker};

/// The pinned baseline corpus. Every Arc O phase re-captures these same five
/// seeds so before/after comparisons are like-for-like; changing this list
/// invalidates the comparison, so treat it as part of the evidence contract.
pub const PRESET_SEEDS: [u64; 5] = [
    0xa11c_e3d0_0000_0008,
    0x0000_0000_000c_0ffe,
    0x0000_0000_0000_0b0b,
    0x0000_0000_000d_00d0,
    0x5eed_0000_0000_0001,
];

/// True isometric pitch: `atan(1 / sqrt(2))`, the angle at which the three axes
/// of a cube project to equal screen lengths.
const ISO_PITCH: f32 = -0.615_479_7;
const WINDOW_WIDTH: f32 = 1600.0;
const WINDOW_HEIGHT: f32 = 1000.0;
/// Connection bars ride just above the lower of the two cells they join, so they
/// read as a bridge over the composition rather than a rod through it.
const LINK_CLEARANCE: f32 = 0.25;
const LINK_THICKNESS: f32 = 1.7;
/// A riser is pushed past the footprint, because a bar drawn up the middle of a
/// shaft column is inside the column and invisible.
const RISER_OFFSET: f32 = 6.6;
/// Only the three forward lateral faces are walked, so each undirected edge is
/// drawn once rather than twice from opposite ends.
const FORWARD_FACES: [HexFace; 3] = [HexFace::East, HexFace::SouthEast, HexFace::SouthWest];

#[derive(Component)]
struct LabVisual;

#[derive(Component)]
struct LabStatus;

#[derive(Component)]
struct LabCamera;

/// Which levels are drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewMode {
    /// Every level, stacked — the composition read.
    #[default]
    Stack,
    /// One level alone, for counting a floor plan without occlusion.
    Slice,
}

#[derive(Resource)]
pub struct LabState {
    pub world: HexWfcWorld,
    pub seed_index: usize,
    pub mode: ViewMode,
    pub focus_level: u8,
    pub dirty: bool,
    pub reset_count: u32,
    status: String,
}

impl LabState {
    /// Solve the preset at `seed_index` at production facility scale.
    #[must_use]
    pub fn new(seed_index: usize) -> Self {
        let index = seed_index % PRESET_SEEDS.len();
        let seed = PRESET_SEEDS[index];
        let config = HexWfcConfig::arc_default();
        let world = HexWfcWorld::generate(seed, config)
            .expect("the production hex config must solve at every preset seed");
        Self {
            world,
            seed_index: index,
            mode: ViewMode::Stack,
            focus_level: 0,
            dirty: true,
            reset_count: 0,
            status: String::new(),
        }
    }

    fn reload(&mut self, seed_index: usize) {
        let carried = self.reset_count;
        *self = Self::new(seed_index);
        self.reset_count = carried;
    }

    fn drawn(&self, coord: HexCoord) -> bool {
        match self.mode {
            ViewMode::Stack => true,
            ViewMode::Slice => coord.level == self.focus_level,
        }
    }

    /// How many cells of each archetype the current solve placed. This is the
    /// number Arc O's composition phases move, so it is on screen rather than
    /// in a log.
    #[must_use]
    pub fn archetype_census(&self) -> BTreeMap<&'static str, usize> {
        let mut census = BTreeMap::new();
        for placement in self.world.placements.values() {
            if placement.archetype == HexArchetype::Void {
                continue;
            }
            *census
                .entry(archetype_label(placement.archetype))
                .or_insert(0) += 1;
        }
        census
    }

    /// How many cells fall into each composition, and how many connections join
    /// them. These are the numbers Arc O's composition phases are supposed to
    /// move, so they belong on screen rather than in a log.
    #[must_use]
    pub fn composition_census(&self) -> (BTreeMap<&'static str, usize>, usize, usize) {
        let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut lateral = 0;
        let mut vertical = 0;
        let grid = self.world.config.grid();
        for (coord, placement) in &self.world.placements {
            if placement.archetype == HexArchetype::Void {
                continue;
            }
            let role = sketch_role(
                placement.archetype,
                placement.space,
                self.world.room_id_at(*coord).is_some(),
            );
            *by_kind.entry(role.composition().label()).or_insert(0) += 1;
            for face in FORWARD_FACES {
                if placement.is_open(face)
                    && let Some(neighbour) = grid.neighbor(*coord, face)
                    && self
                        .world
                        .placements
                        .get(&neighbour)
                        .is_some_and(|other| other.is_open(face.opposite()))
                {
                    lateral += 1;
                }
            }
            if placement.up != PortClass::Sealed {
                let above = HexCoord {
                    q: coord.q,
                    r: coord.r,
                    level: coord.level.saturating_add(1),
                };
                if self
                    .world
                    .placements
                    .get(&above)
                    .is_some_and(|other| other.down != PortClass::Sealed)
                {
                    vertical += 1;
                }
            }
        }
        (by_kind, lateral, vertical)
    }

    /// How many cells each register owns, and how many disjoint regions those
    /// cells form on the level grid. A district that is a *place* has few, large
    /// regions; today's per-cell lottery produces hundreds of singletons, which
    /// is bug backlog #14 made visible.
    #[must_use]
    pub fn district_census(&self) -> BTreeMap<ArchitectureRegister, (usize, usize)> {
        let mut cells: BTreeMap<ArchitectureRegister, BTreeSet<HexCoord>> = BTreeMap::new();
        for (coord, register) in &self.world.architecture {
            if self
                .world
                .placements
                .get(coord)
                .is_none_or(|p| p.archetype == HexArchetype::Void)
            {
                continue;
            }
            cells.entry(*register).or_default().insert(*coord);
        }
        cells
            .into_iter()
            .map(|(register, owned)| {
                let regions = count_regions(&owned);
                (register, (owned.len(), regions))
            })
            .collect()
    }
}

/// Flood-fill `cells` over lateral hex adjacency within a level, returning the
/// number of connected components.
fn count_regions(cells: &BTreeSet<HexCoord>) -> usize {
    let mut unvisited = cells.clone();
    let mut regions = 0;
    while let Some(&start) = unvisited.iter().next() {
        regions += 1;
        let mut frontier = vec![start];
        unvisited.remove(&start);
        while let Some(coord) = frontier.pop() {
            for (dq, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)] {
                let Some(q) = coord.q.checked_add_signed(dq) else {
                    continue;
                };
                let Some(r) = coord.r.checked_add_signed(dr) else {
                    continue;
                };
                let neighbour = HexCoord {
                    q,
                    r,
                    level: coord.level,
                };
                if unvisited.remove(&neighbour) {
                    frontier.push(neighbour);
                }
            }
        }
    }
    regions
}

/// Which style-owned sketch role a cell draws as.
///
/// The heights and widths live in `observed_style` so this lab and the in-game
/// survivor map cannot drift apart; only the mapping from the solver's
/// vocabulary is local. Blueprint membership wins over archetype, because the
/// blueprint is the authority on what composes a room.
#[must_use]
pub fn sketch_role(archetype: HexArchetype, space: HexSpace, in_blueprint: bool) -> HexSketchRole {
    if in_blueprint || space == HexSpace::Room {
        return HexSketchRole::Room;
    }
    match archetype {
        HexArchetype::Void => HexSketchRole::Void,
        HexArchetype::Straight | HexArchetype::Corner => HexSketchRole::Corridor,
        HexArchetype::Junction => HexSketchRole::Junction,
        HexArchetype::RampUp => HexSketchRole::Ramp,
        HexArchetype::RampHead => HexSketchRole::RampHead,
        HexArchetype::Shaft => HexSketchRole::Shaft,
        HexArchetype::Room => HexSketchRole::Room,
    }
}

/// How a cell of this world is drawn: slab height and footprint fill.
#[must_use]
pub fn cell_sketch(world: &HexWfcWorld, coord: HexCoord) -> Option<HexSketch> {
    let placement = world.placements.get(&coord)?;
    Some(hex_sketch(sketch_role(
        placement.archetype,
        placement.space,
        world.room_id_at(coord).is_some(),
    )))
}

/// Slab height for an archetype alone, for callers that only need to clear it.
#[must_use]
pub fn archetype_height(archetype: HexArchetype) -> Option<f32> {
    hex_sketch(sketch_role(archetype, HexSpace::Hall, false)).height
}

#[must_use]
pub fn archetype_label(archetype: HexArchetype) -> &'static str {
    match archetype {
        HexArchetype::Void => "void",
        HexArchetype::Straight => "straight",
        HexArchetype::Corner => "corner",
        HexArchetype::Junction => "junction",
        HexArchetype::Room => "room",
        HexArchetype::RampUp => "ramp",
        HexArchetype::RampHead => "ramp head",
        HexArchetype::Shaft => "shaft",
    }
}

fn register_label(register: ArchitectureRegister) -> &'static str {
    match register {
        ArchitectureRegister::ShadowScreen => "shadow screen",
        ArchitectureRegister::Monolith => "monolith",
        ArchitectureRegister::OverlitGrid => "overlit grid",
        ArchitectureRegister::Institutional => "institutional",
        ArchitectureRegister::FacetMonument => "facet monument",
        ArchitectureRegister::Megastructure => "megastructure",
        ArchitectureRegister::Wellshaft => "wellshaft",
        ArchitectureRegister::InfiniteGallery => "infinite gallery",
        ArchitectureRegister::Thinning => "thinning",
        ArchitectureRegister::LiminalGrid => "liminal grid",
    }
}

/// The world-space axis-aligned bounds of every drawn cell, padded by one cell.
fn world_bounds(state: &LabState) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for (coord, placement) in &state.world.placements {
        let Some(height) = archetype_height(placement.archetype) else {
            continue;
        };
        let origin = Vec3::from_array(hex_origin(*coord));
        min = min.min(origin - Vec3::new(8.0, 0.0, 8.0));
        max = max.max(origin + Vec3::new(8.0, height, 8.0));
    }
    if min.x > max.x {
        return (Vec3::ZERO, Vec3::ONE);
    }
    (min, max)
}

/// Frame the whole facility in an orthographic isometric view.
///
/// Returned as (transform, scale, far) so the framing maths is testable without
/// a renderer. `scale` is world units per pixel, which is what
/// [`OrthographicProjection::default_3d`] consumes; `far` must be passed
/// explicitly because the 3D default is 1000 m and a production facility's
/// diagonal alone exceeds that — leave it and most of the map is clipped away.
#[must_use]
pub fn frame_camera(min: Vec3, max: Vec3) -> (Transform, f32, f32) {
    let rotation = Quat::from_euler(EulerRot::YXZ, FRAC_PI_4, ISO_PITCH, 0.0);
    let centre = (min + max) * 0.5;
    let inverse = rotation.inverse();

    // Project the eight AABB corners into camera space and take the screen
    // extent. Exact, and independent of which way the isometric angle faces.
    let mut extent = Vec2::ZERO;
    for i in 0..8u8 {
        let corner = Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        );
        let camera_space = inverse * (corner - centre);
        extent = extent.max(camera_space.truncate().abs());
    }

    let scale = (extent.x * 2.0 / WINDOW_WIDTH)
        .max(extent.y * 2.0 / WINDOW_HEIGHT)
        .max(f32::MIN_POSITIVE)
        * 1.08;
    let diagonal = (max - min).length().max(1.0);
    let transform =
        Transform::from_translation(centre + rotation * Vec3::Z * diagonal).with_rotation(rotation);
    (transform, scale, diagonal * 2.0)
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.010, 0.020)))
        .insert_resource(LabState::new(0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Isometric Observer Lab".to_string(),
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                resolution: WindowResolution::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_input, rebuild, update_status).chain());

    if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(capture::CaptureRun::new(dir))
            .add_systems(Update, capture::capture_progress.after(rebuild));
    }
    app.run();
}

fn setup(mut commands: Commands, state: Res<LabState>) {
    let (min, max) = world_bounds(&state);
    let (transform, scale, far) = frame_camera(min, max);
    commands.spawn((
        LabCamera,
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale,
            near: 0.1,
            far,
            ..OrthographicProjection::default_3d()
        }),
        transform,
        Name::new("Isometric observer camera"),
    ));

    // Studio lighting, not district atmosphere. This lab shows ten registers in
    // one frame, so no single district palette applies; the fill is deliberately
    // neutral and even so the register tints carry the whole colour signal. Same
    // reasoning as `hex_tile_lab`'s `clay` render mode.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 220.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 2_600.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.9, -0.95, 0.0)),
        Name::new("Isometric observer key"),
    ));

    commands.spawn((
        LabStatus,
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.94, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
}

fn handle_input(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        let index = state.seed_index;
        let count = state.reset_count + 1;
        state.reload(index);
        state.reset_count = count;
        return;
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        let next = (state.seed_index + 1) % PRESET_SEEDS.len();
        state.reload(next);
        return;
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        let next = (state.seed_index + PRESET_SEEDS.len() - 1) % PRESET_SEEDS.len();
        state.reload(next);
        return;
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        state.mode = match state.mode {
            ViewMode::Stack => ViewMode::Slice,
            ViewMode::Slice => ViewMode::Stack,
        };
        state.dirty = true;
        return;
    }
    let levels = state.world.config.levels;
    if keyboard.just_pressed(KeyCode::PageUp) && state.focus_level + 1 < levels {
        state.focus_level += 1;
        state.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::PageDown) && state.focus_level > 0 {
        state.focus_level -= 1;
        state.dirty = true;
    }
}

fn rebuild(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<LabVisual>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    // One mesh per archetype and one material per (register, archetype-is-room)
    // pair: a few dozen assets for a few thousand cells.
    let mut mesh_cache: BTreeMap<(u32, u32), Handle<Mesh>> = BTreeMap::new();
    let mut material_cache: BTreeMap<u8, Handle<StandardMaterial>> = BTreeMap::new();

    let spawn = state.world.config.spawn();
    let exit = state.world.config.exit();

    for (coord, placement) in &state.world.placements {
        if !state.drawn(*coord) {
            continue;
        }
        let drawn = hex_sketch(sketch_role(
            placement.archetype,
            placement.space,
            state.world.room_id_at(*coord).is_some(),
        ));
        let Some(height) = drawn.height else {
            continue;
        };
        let register = state
            .world
            .architecture
            .get(coord)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);

        let mesh = mesh_cache
            .entry((height.to_bits(), drawn.inset.to_bits()))
            .or_insert_with(|| meshes.add(prism::hex_prism(height, drawn.inset)))
            .clone();
        let material = material_cache
            .entry(register as u8)
            .or_insert_with(|| {
                // `architecture_surface(_, Floor)` is register-blind: every base
                // register falls through to `surface(SurfaceRole::Plain)`, so
                // using it here paints the whole map one grey. The style crate's
                // actual per-neighbourhood structural colour is the district
                // accent — the same channel `PracticalFixture` reads. Note ten
                // registers collapse onto seven accent families, so colour
                // separates districts, not registers; the census below carries
                // the per-register numbers.
                let accent = observed_style::architecture(register).accent;
                materials.add(StandardMaterial {
                    base_color: Color::LinearRgba(accent),
                    emissive: accent * 0.22,
                    perceptual_roughness: 0.92,
                    ..default()
                })
            })
            .clone();

        let origin = Vec3::from_array(hex_origin(*coord));
        commands.spawn((
            LabVisual,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(origin),
            Name::new(archetype_label(placement.archetype)),
        ));
    }

    // Connections. This is what turns a field of tiles into a graph: corridor
    // runs become visible lines and a riser shows two floors are joined. The lab
    // reads ground truth, so both sides come straight from the placements rather
    // than from anyone's knowledge.
    let grid = state.world.config.grid();
    let mut bar_cache: BTreeMap<(u32, u32, u32), Handle<Mesh>> = BTreeMap::new();
    let link_material = {
        let treatment = hex_link(true);
        materials.add(StandardMaterial {
            base_color: treatment.base_color,
            emissive: treatment.emissive,
            perceptual_roughness: 0.6,
            ..default()
        })
    };
    for (coord, placement) in &state.world.placements {
        if !state.drawn(*coord) {
            continue;
        }
        let here = archetype_height(placement.archetype).unwrap_or(0.9);
        let origin = Vec3::from_array(hex_origin(*coord));

        for face in FORWARD_FACES {
            if !placement.is_open(face) {
                continue;
            }
            let Some(neighbour) = grid.neighbor(*coord, face) else {
                continue;
            };
            if !state.drawn(neighbour) {
                continue;
            }
            let Some(other) = state.world.placements.get(&neighbour) else {
                continue;
            };
            if !other.is_open(face.opposite()) {
                continue;
            }
            let target = Vec3::from_array(hex_origin(neighbour));
            let span = target - origin;
            let length = ((span.length() * 16.0).round() / 16.0).max(f32::EPSILON);
            let deck = here.min(archetype_height(other.archetype).unwrap_or(0.9)) + LINK_CLEARANCE;
            let mesh = bar_cache
                .entry((
                    length.to_bits(),
                    (LINK_THICKNESS * 0.4).to_bits(),
                    LINK_THICKNESS.to_bits(),
                ))
                .or_insert_with(|| {
                    meshes.add(Cuboid::new(length, LINK_THICKNESS * 0.4, LINK_THICKNESS))
                })
                .clone();
            commands.spawn((
                LabVisual,
                Mesh3d(mesh),
                MeshMaterial3d(link_material.clone()),
                Transform::from_translation(origin + span * 0.5 + Vec3::Y * deck)
                    .with_rotation(Quat::from_rotation_y((-span.z).atan2(span.x))),
                Name::new("link"),
            ));
        }

        if placement.up == PortClass::Sealed {
            continue;
        }
        let above = HexCoord {
            q: coord.q,
            r: coord.r,
            level: coord.level.saturating_add(1),
        };
        if !state.drawn(above) {
            continue;
        }
        if state
            .world
            .placements
            .get(&above)
            .is_none_or(|other| other.down == PortClass::Sealed)
        {
            continue;
        }
        let mesh = bar_cache
            .entry((
                (LINK_THICKNESS * 0.8).to_bits(),
                TILE_LEVEL_HEIGHT.to_bits(),
                (LINK_THICKNESS * 0.8).to_bits(),
            ))
            .or_insert_with(|| {
                meshes.add(Cuboid::new(
                    LINK_THICKNESS * 0.8,
                    TILE_LEVEL_HEIGHT,
                    LINK_THICKNESS * 0.8,
                ))
            })
            .clone();
        commands.spawn((
            LabVisual,
            Mesh3d(mesh),
            MeshMaterial3d(link_material.clone()),
            Transform::from_translation(
                origin + Vec3::Y * (TILE_LEVEL_HEIGHT * 0.5) - Vec3::Z * RISER_OFFSET,
            ),
            Name::new("riser"),
        ));
    }

    // The two fixed endpoints get real gameplay markers — these are signal-tier
    // and legitimately owned by `observed_style::marker`.
    for (coord, role, label) in [
        (spawn, MarkerRole::You, "spawn marker"),
        (exit, MarkerRole::Exit, "exit marker"),
    ] {
        if !state.drawn(coord) {
            continue;
        }
        let treatment = marker(role);
        let mesh = meshes.add(prism::hex_prism(0.5, 0.55));
        let material = materials.add(StandardMaterial {
            base_color: treatment.base_color,
            emissive: treatment.emissive,
            perceptual_roughness: 0.4,
            ..default()
        });
        let origin = Vec3::from_array(hex_origin(coord));
        commands.spawn((
            LabVisual,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(origin + Vec3::Y * 9.0),
            Name::new(label),
        ));
    }
}

fn update_status(mut state: ResMut<LabState>, mut status: Query<&mut Text, With<LabStatus>>) {
    let Ok(mut text) = status.single_mut() else {
        return;
    };
    let census = state.archetype_census();
    let districts = state.district_census();
    let total: usize = census.values().sum();

    let archetypes = census
        .iter()
        .map(|(label, count)| {
            #[allow(clippy::cast_precision_loss)]
            let share = *count as f32 * 100.0 / total.max(1) as f32;
            format!("{label} {count} ({share:.0}%)")
        })
        .collect::<Vec<_>>()
        .join("  ");

    let mut district_lines = districts
        .iter()
        .map(|(register, (cells, regions))| {
            #[allow(clippy::cast_precision_loss)]
            let mean = *cells as f32 / (*regions).max(1) as f32;
            format!(
                "  {:<17} {cells:>5} cells in {regions:>4} regions (mean {mean:.1})",
                register_label(*register)
            )
        })
        .collect::<Vec<_>>();
    district_lines.sort();

    let view = match state.mode {
        ViewMode::Stack => "stacked".to_string(),
        ViewMode::Slice => format!("level {}", state.focus_level),
    };

    let (by_kind, lateral, vertical) = state.composition_census();
    let composed = by_kind
        .iter()
        .map(|(label, count)| {
            #[allow(clippy::cast_precision_loss)]
            let share = *count as f32 * 100.0 / total.max(1) as f32;
            format!("{label} {count} ({share:.0}%)")
        })
        .collect::<Vec<_>>()
        .join("  ");

    state.status = format!(
        "seed {:#018x}  ({} of {})   view: {view}   {total} placed cells   attempts {}\n\
         colour = district   width = room/hallway/vertical   height = archetype   bars = connections\n\
         archetypes   {archetypes}\n\
         composes     {composed}\n\
         connects     {lateral} lateral | {vertical} vertical\n\
         districts (cells / disjoint regions; a district that is a place has few, large regions):\n\
         {}\n\
         [ ] seed   Tab stack/slice   PageUp/PageDown level   R resolve",
        state.world.seed,
        state.seed_index + 1,
        PRESET_SEEDS.len(),
        state.world.last_attempts,
        district_lines.join("\n"),
    );
    **text = state.status.clone();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(LabState::new(0))
            .add_systems(Update, rebuild);
        app.update();
        app
    }

    fn count_visuals(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<LabVisual>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn every_preset_seed_solves_at_production_scale() {
        for (index, seed) in PRESET_SEEDS.iter().enumerate() {
            let state = LabState::new(index);
            assert_eq!(state.world.seed, *seed);
            assert!(
                !state.world.placements.is_empty(),
                "preset {index} produced an empty facility"
            );
        }
    }

    #[test]
    fn void_is_the_only_archetype_without_a_slab() {
        for archetype in [
            HexArchetype::Void,
            HexArchetype::Room,
            HexArchetype::Straight,
            HexArchetype::Corner,
            HexArchetype::Junction,
            HexArchetype::RampUp,
            HexArchetype::RampHead,
            HexArchetype::Shaft,
        ] {
            let height = archetype_height(archetype);
            if archetype == HexArchetype::Void {
                assert!(height.is_none(), "void draws nothing");
            } else {
                assert!(
                    height.is_some_and(|h| h > 0.0),
                    "{} needs a positive slab",
                    archetype_label(archetype)
                );
            }
        }
    }

    #[test]
    fn vertical_archetypes_stand_taller_than_flat_circulation() {
        let straight = archetype_height(HexArchetype::Straight).expect("straight draws");
        let junction = archetype_height(HexArchetype::Junction).expect("junction draws");
        let room = archetype_height(HexArchetype::Room).expect("room draws");
        let shaft = archetype_height(HexArchetype::Shaft).expect("shaft draws");
        assert!(straight < junction, "junctions read above corridors");
        assert!(junction < room, "rooms read above junctions");
        assert!(room < shaft, "vertical circulation is the tallest read");
    }

    #[test]
    fn the_framing_fits_the_facility_inside_the_viewport() {
        let state = LabState::new(0);
        let (min, max) = world_bounds(&state);
        let (transform, scale, _far) = frame_camera(min, max);
        let inverse = transform.rotation.inverse();
        let centre = (min + max) * 0.5;
        for i in 0..8u8 {
            let corner = Vec3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { min.y } else { max.y },
                if i & 4 == 0 { min.z } else { max.z },
            );
            let projected = (inverse * (corner - centre)).truncate();
            assert!(
                projected.x.abs() <= scale * WINDOW_WIDTH * 0.5 + 1e-3,
                "corner {i} escaped the viewport horizontally"
            );
            assert!(
                projected.y.abs() <= scale * WINDOW_HEIGHT * 0.5 + 1e-3,
                "corner {i} escaped the viewport vertically"
            );
        }
    }

    #[test]
    fn slice_mode_draws_strictly_fewer_cells_than_the_stack() {
        let state = LabState::new(0);
        let stacked = state
            .world
            .placements
            .keys()
            .filter(|c| state.drawn(**c))
            .count();
        let mut sliced = state;
        sliced.mode = ViewMode::Slice;
        sliced.focus_level = 0;
        let one_level = sliced
            .world
            .placements
            .keys()
            .filter(|c| sliced.drawn(**c))
            .count();
        assert!(one_level > 0, "level 0 must draw something");
        assert!(one_level < stacked, "a slice is a subset of the stack");
    }

    #[test]
    fn reset_rebuilds_the_projection_without_leaking_entities() {
        let mut app = test_app();
        let baseline = count_visuals(&mut app);
        assert!(baseline > 0, "the first build must draw the facility");
        for expected in 1..=3 {
            {
                let mut state = app.world_mut().resource_mut::<LabState>();
                let index = state.seed_index;
                let count = state.reset_count + 1;
                state.reload(index);
                state.reset_count = count;
            }
            app.update();
            assert_eq!(
                count_visuals(&mut app),
                baseline,
                "reset {expected} leaked or dropped visuals"
            );
            assert_eq!(app.world().resource::<LabState>().reset_count, expected);
        }
    }

    #[test]
    fn the_district_census_counts_every_drawn_cell_exactly_once() {
        let state = LabState::new(1);
        let drawn = state
            .world
            .placements
            .values()
            .filter(|p| p.archetype != HexArchetype::Void)
            .count();
        let counted: usize = state
            .district_census()
            .values()
            .map(|(cells, _)| cells)
            .sum();
        assert_eq!(
            counted, drawn,
            "every placed cell belongs to exactly one district"
        );
    }

    #[test]
    fn the_composition_census_covers_every_drawn_cell_and_finds_connections() {
        let state = LabState::new(0);
        let drawn = state
            .world
            .placements
            .values()
            .filter(|p| p.archetype != HexArchetype::Void)
            .count();
        let (by_kind, lateral, vertical) = state.composition_census();
        let counted: usize = by_kind.values().sum();
        assert_eq!(
            counted, drawn,
            "every placed cell composes exactly one of room/hallway/vertical"
        );
        assert!(
            lateral > 0 && vertical > 0,
            "a solved facility is connected laterally and vertically; saw \
             {lateral} lateral and {vertical} vertical"
        );
    }

    #[test]
    fn the_lab_and_the_game_share_one_sketch_table() {
        // Both views map their archetypes onto `observed_style`'s roles, so a
        // height or width change lands in both at once. This pins the mapping
        // rather than the numbers, which belong to the style crate's own tests.
        assert_eq!(
            sketch_role(HexArchetype::Shaft, HexSpace::Hall, false),
            HexSketchRole::Shaft
        );
        assert_eq!(
            sketch_role(HexArchetype::Junction, HexSpace::Hall, true),
            HexSketchRole::Room,
            "blueprint membership wins over archetype"
        );
        let room = hex_sketch(HexSketchRole::Room);
        assert!((room.inset - 1.0).abs() < f32::EPSILON, "rooms merge");
        assert!(hex_sketch(HexSketchRole::Corridor).inset < room.inset);
    }

    #[test]
    fn todays_registers_fragment_into_many_regions() {
        // Pins bug backlog #14 as an observation rather than a claim: with the
        // per-cell lottery, base registers shatter into hundreds of tiny
        // regions. Phase 106 replaces the lottery, and this test is the one that
        // must be inverted to prove it — mean region size should climb by orders
        // of magnitude.
        let state = LabState::new(0);
        let census = state.district_census();
        let base = census
            .iter()
            .find(|(register, _)| **register != ArchitectureRegister::LiminalGrid)
            .map(|(_, counts)| *counts)
            .expect("a base register is always assigned");
        #[allow(clippy::cast_precision_loss)]
        let mean = base.0 as f32 / base.1.max(1) as f32;
        assert!(
            mean < 4.0,
            "expected the pre-Phase-106 lottery to fragment registers, saw mean region {mean}"
        );
    }
}
