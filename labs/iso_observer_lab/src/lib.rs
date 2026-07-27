//! Isometric facility observer — the Arc O Phase 104 instrument.
//!
//! Arc O changes what the hex WFC composes: contiguous districts, per-district
//! composition profiles, an `Expanse` archetype, authored vertical kits. None of
//! that is falsifiable from a first-person camera inside a corridor, so this lab
//! exists to make a whole solved facility legible at a glance, on two orthogonal
//! channels:
//!
//! - **Colour is the district.** Every cell is tinted by
//!   [`observed_style::architecture_surface`] for its
//!   [`ArchitectureRegister`]. The lab never invents a colour.
//! - **Height is the archetype.** Corridors are thin slabs, junctions a little
//!   thicker, rooms thicker still, ramps a wedge-height block, shafts a column
//!   reaching for the level above.
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
use observed_facility::hex_wfc::{HexArchetype, HexWfcConfig, HexWfcWorld};
use observed_hex::{HexCoord, TILE_LEVEL_HEIGHT, hex_origin};
use observed_style::{ArchitectureSurfaceRole, MarkerRole, architecture_surface, marker};

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
/// Slight per-cell shrink so neighbours read as countable tiles. Kept close to
/// 1.0 deliberately — widen it and genuinely open regions stop reading as one
/// continuous space, which is the exact signal Arc O needs to see.
const CELL_INSET: f32 = 0.94;

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

/// Height in metres for an archetype's slab. The ordering *is* the legend:
/// flat circulation is low, decision space is mid, vertical circulation is tall.
#[must_use]
pub fn archetype_height(archetype: HexArchetype) -> Option<f32> {
    match archetype {
        HexArchetype::Void => None,
        HexArchetype::Straight | HexArchetype::Corner => Some(0.9),
        HexArchetype::Junction => Some(1.5),
        HexArchetype::RampHead => Some(0.6),
        HexArchetype::Room => Some(2.6),
        HexArchetype::RampUp => Some(4.5),
        HexArchetype::Shaft => Some(TILE_LEVEL_HEIGHT * 0.85),
    }
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
/// Returned as (transform, scale) so the framing maths is testable without a
/// renderer: `scale` is world units per pixel, which is what
/// [`OrthographicProjection::default_3d`] consumes.
#[must_use]
pub fn frame_camera(min: Vec3, max: Vec3) -> (Transform, f32) {
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
    let distance = (max - min).length().max(1.0) * 2.0;
    let transform =
        Transform::from_translation(centre + rotation * Vec3::Z * distance).with_rotation(rotation);
    (transform, scale)
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
    let (transform, scale) = frame_camera(min, max);
    commands.spawn((
        LabCamera,
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale,
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
    let mut mesh_cache: BTreeMap<u32, Handle<Mesh>> = BTreeMap::new();
    let mut material_cache: BTreeMap<u8, Handle<StandardMaterial>> = BTreeMap::new();

    let spawn = state.world.config.spawn();
    let exit = state.world.config.exit();

    for (coord, placement) in &state.world.placements {
        if !state.drawn(*coord) {
            continue;
        }
        let Some(height) = archetype_height(placement.archetype) else {
            continue;
        };
        let register = state
            .world
            .architecture
            .get(coord)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);

        let mesh = mesh_cache
            .entry(height.to_bits())
            .or_insert_with(|| meshes.add(prism::hex_prism(height, CELL_INSET)))
            .clone();
        let material = material_cache
            .entry(register as u8)
            .or_insert_with(|| {
                let treatment = architecture_surface(register, ArchitectureSurfaceRole::Floor);
                materials.add(StandardMaterial {
                    base_color: treatment.base_color,
                    emissive: treatment.emissive,
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

    state.status = format!(
        "seed {:#018x}  ({} of {})   view: {view}   {total} placed cells   attempts {}\n\
         colour = district register    height = archetype\n\
         {archetypes}\n\
         districts (cells / disjoint regions — a district that is a place has few, large regions):\n\
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
        let (transform, scale) = frame_camera(min, max);
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
