//! The stack: every floor at once, isometric, in the desk's panel.
//!
//! The board is one floor, flat and exact, because placing a tile needs a cell under the
//! cursor with nothing in front of it. What that costs is the climb: which floor the team
//! is on, which is breaking, where the summit is. The stack answers those at a glance,
//! the way the survivor map answers them for an Observer - the same isometric reading,
//! from the same knowledge the board draws - and a click on a floor puts the board on it.
//!
//! Floors are pulled apart well past their real height so none hides another. Each is a
//! faint plate of the whole lattice with the team's known cells on it, in their district's
//! colour, the floor in view lit and the rest dim; on them, the team's Observers, the
//! contradictions, the summit and the prison lobby. It is drawn by its own camera into a
//! viewport over a space the panel keeps empty for it.

use std::hash::{Hash, Hasher};

use bevy::asset::RenderAssetUsages;
use bevy::camera::Viewport;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use observed_facility::hex_wfc::HexWfcConfig;
use observed_hex::{HexCoord, hex_origin};
use observed_match::ascent::sim::ObserverState;
use observed_style::{MarkerRole, marker};

use super::ArchitectDesk;
use super::pick::{self, CELL_RADIUS};
use crate::GameState;
use crate::hex_wfc::equipment::hex_prism;
use crate::hex_wfc::sim::HexWfcRuntime;

/// The stack's own render layer and camera order, above the board.
pub(super) const STACK_LAYER: usize = 4;
const STACK_ORDER: isize = 3;
/// Metres between floors in the stack: far more than a storey, so that no floor's
/// footprint, seen at [`PITCH`], reaches the floor above it.
pub(super) const SPACING: f32 = 124.0;
/// How far the stack is looked down on. Head-on rather than diagonal: a floor is wide and
/// shallow, and seen corner-on it collapses to a sliver. North stays up, as on the board.
const PITCH: f32 = -0.52;

/// The space in the panel the stack is drawn into.
#[derive(Component)]
pub(super) struct StackSpace;

/// A floor's number, beside its plate.
#[derive(Component)]
pub(super) struct FloorLabel(u8);

#[derive(Component)]
pub(super) struct StackCamera;

#[derive(Component)]
pub(super) struct StackPiece;

#[derive(Resource, Default)]
pub(super) struct Stack {
    signature: u64,
}

pub(super) fn setup(mut commands: Commands, stack: Option<Res<Stack>>) {
    if stack.is_some() {
        return;
    }
    commands.insert_resource(Stack::default());
    commands.spawn((
        StackCamera,
        DespawnOnExit(GameState::HexWfc),
        Camera3d::default(),
        Camera {
            order: STACK_ORDER,
            // Drawn over the panel's own background, which shows round it.
            clear_color: ClearColorConfig::None,
            viewport: Some(Viewport {
                physical_position: UVec2::ZERO,
                physical_size: UVec2::ONE,
                ..default()
            }),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection::default_3d()),
        RenderLayers::layer(STACK_LAYER),
        Transform::default(),
        Name::new("Architect stack camera"),
    ));
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        DirectionalLight {
            illuminance: 4_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        RenderLayers::layer(STACK_LAYER),
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.9, -0.95, 0.0)),
        Name::new("Architect stack key"),
    ));
}

/// Fit the camera's viewport to the panel's space for it, and the whole stack in it, and
/// put each floor's number beside its plate.
#[allow(clippy::type_complexity)]
pub(super) fn frame(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    desk: Res<ArchitectDesk>,
    space: Query<(Entity, &ComputedNode, &UiGlobalTransform), With<StackSpace>>,
    mut camera: Query<
        (
            &mut Camera,
            &mut Transform,
            &GlobalTransform,
            &mut Projection,
        ),
        With<StackCamera>,
    >,
    mut labels: Query<(&FloorLabel, &mut Node, &mut TextColor)>,
) {
    let (Ok((space, node, placed)), Ok((mut camera, mut transform, seen_from, mut projection))) =
        (space.single(), camera.single_mut())
    else {
        return;
    };
    let config = runtime.match_state.facility.config;
    if labels.is_empty() {
        commands.entity(space).with_children(|space| {
            for level in 0..config.levels {
                space.spawn((
                    FloorLabel(level),
                    Text::new(format!("{}", level + 1)),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(crate::view::theme::DIM),
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                ));
            }
        });
    }
    let (min, max) = plate_bounds(config);
    for (label, mut place, mut color) in &mut labels {
        let beside = Vec3::new(min.x - 14.0, floor_height(label.0), (min.y + max.y) * 0.5);
        // The camera answers in window space; a label is placed inside the stack's space.
        if let (Ok(at), Some(viewport)) = (
            camera.world_to_viewport(seen_from, beside),
            camera.logical_viewport_rect(),
        ) {
            let at = at - viewport.min;
            place.left = px(at.x - 12.0);
            place.top = px(at.y - 8.0);
        }
        color.0 = if label.0 == desk.floor {
            crate::view::theme::ACCENT
        } else {
            crate::view::theme::DIM
        };
    }
    let size = node.size();
    if size.x < 2.0 || size.y < 2.0 {
        return;
    }
    let corner = (placed.translation - size * 0.5).max(Vec2::ZERO);
    camera.viewport = Some(Viewport {
        physical_position: corner.as_uvec2(),
        physical_size: size.as_uvec2(),
        ..default()
    });
    let (framed, scale) = frame_stack(config, size);
    *transform = framed;
    *projection = Projection::Orthographic(OrthographicProjection {
        scale,
        near: 0.1,
        far: 4_000.0,
        ..OrthographicProjection::default_3d()
    });
}

/// Rebuild the stack when what the team knows, or the floor in view, has changed.
pub(super) fn draw(
    mut commands: Commands,
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    mut stack: ResMut<Stack>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    drawn: Query<Entity, With<StackPiece>>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let rules = ascent.rules();
    let Some(knowledge) = rules.team_knowledge.get(&desk.team) else {
        return;
    };
    let mut hasher = std::hash::DefaultHasher::new();
    desk.floor.hash(&mut hasher);
    for (cell, known) in &knowledge.cells {
        (cell, known.placement.space.built()).hash(&mut hasher);
    }
    rules.contradictions.hash(&mut hasher);
    for observer in rules.observers.values() {
        (observer.cell, observer.state == ObserverState::Active).hash(&mut hasher);
    }
    let requests = super::requests::team_requests(ascent.session(), desk.team);
    for request in &requests {
        (request.target, request.kind as u8).hash(&mut hasher);
    }
    let signature = hasher.finish();
    if signature == stack.signature {
        return;
    }
    stack.signature = signature;
    for entity in &drawn {
        commands.entity(entity).despawn();
    }
    let config = rules.world.config;
    let layer = RenderLayers::layer(STACK_LAYER);
    // Tall, so a known cell stands up off its floor at this distance.
    let slab = meshes.add(hex_prism(CELL_RADIUS - 0.6, CELL_RADIUS - 0.6, 0.0, 12.0));
    let pin = meshes.add(hex_prism(5.0, 5.0, 0.0, 44.0));
    let plate = meshes.add(plate_mesh(config));
    let mut paint = |color: Color, emissive: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: color,
            emissive,
            ..default()
        })
    };
    let spawn = |commands: &mut Commands, mesh: &Handle<Mesh>, material, at: Vec3| {
        commands.spawn((
            StackPiece,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(at),
            layer.clone(),
        ));
    };
    for level in 0..config.levels {
        let focused = level == desk.floor;
        let tone = if focused { 0.09 } else { 0.03 };
        let plate_paint = paint(
            Color::srgba(0.4, 0.92, 1.0, 1.0).with_luminance(tone),
            LinearRgba::rgb(0.1, 0.35, 0.45) * if focused { 1.0 } else { 0.25 },
        );
        spawn(
            &mut commands,
            &plate,
            plate_paint,
            Vec3::Y * floor_height(level),
        );
    }
    for (&cell, known) in &knowledge.cells {
        if !known.placement.space.built() {
            continue;
        }
        let register = rules
            .world
            .architecture
            .get(&cell)
            .copied()
            .unwrap_or(observed_content::ArchitectureRegister::ALL[0]);
        let accent = observed_style::architecture(register).accent;
        let gain = if cell.level == desk.floor { 1.0 } else { 0.45 };
        let material = paint(
            Color::LinearRgba(accent * (0.5 * gain)),
            accent * (0.35 * gain),
        );
        spawn(&mut commands, &slab, material, at(cell));
    }
    let mut pin_at = |commands: &mut Commands, role: MarkerRole, cell: HexCoord| {
        let t = marker(role);
        let material = paint(t.base_color, t.emissive);
        spawn(commands, &pin, material, at(cell) + Vec3::Y * 2.0);
    };
    for &cell in &rules.contradictions {
        if knowledge.cells.contains_key(&cell) {
            pin_at(&mut commands, MarkerRole::Collapse, cell);
        }
    }
    for observer in rules.observers.values() {
        if observer.team == desk.team && observer.state == ObserverState::Active {
            pin_at(&mut commands, MarkerRole::Teammate, observer.cell);
        }
    }
    let summit = config.exit();
    if knowledge.cells.contains_key(&summit) {
        pin_at(&mut commands, MarkerRole::Exit, summit);
    }
    if let Some(&lobby) = rules.prison.cells.iter().next() {
        pin_at(&mut commands, MarkerRole::Prison, lobby);
    }
    // The team's requests, taller than any other pin, so a floor that asks shows it.
    let beacon = meshes.add(hex_prism(3.5, 3.5, 0.0, 80.0));
    for request in &requests {
        let tint = observed_style::architect::color(super::requests::role(request.kind));
        let material = paint(tint, tint.to_linear() * 0.8);
        spawn(
            &mut commands,
            &beacon,
            material,
            at(request.target) + Vec3::Y * 2.0,
        );
    }
}

/// A click on the stack puts the board on the floor clicked.
pub(super) fn click(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    runtime: Res<HexWfcRuntime>,
    camera: Query<(&Camera, &GlobalTransform), With<StackCamera>>,
    mut desk: ResMut<ArchitectDesk>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let (Ok(window), Ok((camera, placed))) = (windows.single(), camera.single()) else {
        return;
    };
    let (Some(cursor), Some(viewport)) = (window.cursor_position(), camera.logical_viewport_rect())
    else {
        return;
    };
    if !viewport.contains(cursor) {
        return;
    }
    let Ok(ray) = camera.viewport_to_world(placed, cursor - viewport.min) else {
        return;
    };
    let config = runtime.match_state.facility.config;
    if let Some(level) = floor_under(config, ray) {
        desk.look_at(level);
    }
}

/// Where a floor sits in the stack.
#[must_use]
pub(super) fn floor_height(level: u8) -> f32 {
    f32::from(level) * SPACING
}

/// Where a cell sits in the stack.
fn at(cell: HexCoord) -> Vec3 {
    let origin = hex_origin(cell);
    Vec3::new(origin[0], floor_height(cell.level), origin[2])
}

/// The topmost floor whose lattice a ray from the camera passes through.
#[must_use]
pub(super) fn floor_under(config: HexWfcConfig, ray: Ray3d) -> Option<u8> {
    (0..config.levels).rev().find(|&level| {
        let height = floor_height(level);
        let Some(distance) = ray.intersect_plane(Vec3::Y * height, InfinitePlane3d::new(Vec3::Y))
        else {
            return false;
        };
        let hit = ray.get_point(distance);
        pick::cell_at(config, Vec2::new(hit.x, hit.z), level).is_some()
    })
}

/// The camera's pose and scale that fit every floor of `config` in `size` pixels.
#[must_use]
pub(super) fn frame_stack(config: HexWfcConfig, size: Vec2) -> (Transform, f32) {
    let rotation = Quat::from_euler(EulerRot::YXZ, 0.0, PITCH, 0.0);
    let (min, max) = plate_bounds(config);
    let top = floor_height(config.levels.saturating_sub(1));
    let low = Vec3::new(min.x, 0.0, min.y);
    let high = Vec3::new(max.x, top + 20.0, max.y);
    let centre = (low + high) * 0.5;
    let inverse = rotation.inverse();
    let mut extent = Vec2::ZERO;
    for i in 0..8u8 {
        let corner = Vec3::new(
            if i & 1 == 0 { low.x } else { high.x },
            if i & 2 == 0 { low.y } else { high.y },
            if i & 4 == 0 { low.z } else { high.z },
        );
        extent = extent.max((inverse * (corner - centre)).truncate().abs());
    }
    // Room on the left for the floor numbers.
    let scale = (extent.x * 2.0 / (size.x - 36.0).max(1.0)).max(extent.y * 2.0 / size.y) * 1.06;
    let back = (high - low).length();
    let transform =
        Transform::from_translation(centre + rotation * Vec3::Z * back).with_rotation(rotation);
    (transform, scale)
}

/// The plan corners of a floor's lattice, padded to whole hexes.
fn plate_bounds(config: HexWfcConfig) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for corner in plate_corners(config) {
        min = min.min(corner);
        max = max.max(corner);
    }
    (min, max)
}

/// The four corners of a floor's lattice: axial coordinates make it a parallelogram.
fn plate_corners(config: HexWfcConfig) -> [Vec2; 4] {
    let at = |q: u16, r: u16| {
        let origin = hex_origin(HexCoord { q, r, level: 0 });
        Vec2::new(origin[0], origin[2])
    };
    let (q, r) = (config.cols - 1, config.rows - 1);
    let pad = CELL_RADIUS;
    [
        at(0, 0) + Vec2::new(-pad, -pad),
        at(q, 0) + Vec2::new(pad, -pad),
        at(q, r) + Vec2::new(pad, pad),
        at(0, r) + Vec2::new(-pad, pad),
    ]
}

/// A thin plate the shape of a floor's lattice.
fn plate_mesh(config: HexWfcConfig) -> Mesh {
    let corners = plate_corners(config);
    let positions: Vec<[f32; 3]> = corners.iter().map(|c| [c.x, -0.5, c.y]).collect();
    let normals = vec![[0.0, 1.0, 0.0]; 4];
    let uvs = vec![[0.0, 0.0]; 4];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> HexWfcConfig {
        HexWfcConfig::arc_default()
    }

    /// A ray down the stack camera's line of sight through a point on a floor.
    fn sighting(config: HexWfcConfig, cell: HexCoord) -> Ray3d {
        let (camera, _) = frame_stack(config, Vec2::new(270.0, 300.0));
        let forward = camera.rotation * Vec3::NEG_Z;
        Ray3d::new(
            at(cell) - forward * 2_000.0,
            Dir3::new(forward).expect("unit"),
        )
    }

    #[test]
    fn a_click_on_a_floor_picks_that_floor() {
        let config = config();
        for level in 0..config.levels {
            // The near corner of each floor: nothing above can be in front of it there.
            let cell = HexCoord {
                q: config.cols - 1,
                r: config.rows - 1,
                level,
            };
            assert_eq!(floor_under(config, sighting(config, cell)), Some(level));
        }
    }

    #[test]
    fn a_click_beside_the_stack_picks_nothing() {
        let config = config();
        let (camera, _) = frame_stack(config, Vec2::new(270.0, 300.0));
        let forward = camera.rotation * Vec3::NEG_Z;
        let ray = Ray3d::new(
            Vec3::new(-2_000.0, 500.0, -2_000.0) - forward * 2_000.0,
            Dir3::new(forward).expect("unit"),
        );
        assert_eq!(floor_under(config, ray), None);
    }

    #[test]
    fn no_floor_s_footprint_reaches_the_floor_above() {
        // Seen at PITCH, a floor's depth rises on screen by depth x sin; the next floor
        // starts spacing x cos higher.
        let (min, max) = plate_bounds(config());
        let depth = max.y - min.y;
        assert!(SPACING * PITCH.cos() > depth * (-PITCH).sin());
    }
}
