//! Three bounded weather batches per sector; no particles, cameras or simulation writes.
use super::turn;
use crate::{
    GameState,
    hex_wfc::{sim::HexWfcRuntime, view::spectate::Cutaway},
};
use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use observed_hex::{HexCoord, HexFace};
use std::collections::BTreeMap;
#[derive(Component)]
struct WeatherPatch {
    coord: HexCoord,
    heading: HexFace,
    kind: u8,
    outline: Vec<Vec3>,
}
#[derive(Clone, ShaderType)]
struct Settings {
    tint: Vec4,
    haze: Vec4,
    unpowered: Vec4,
    params: Vec4,
}
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct WeatherMaterial {
    #[uniform(0)]
    settings: Settings,
}
impl Material for WeatherMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/rain_court.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
#[derive(Resource, Default)]
struct WeatherAssets {
    materials: BTreeMap<(u8, u8), Handle<WeatherMaterial>>,
    meshes: BTreeMap<(usize, u8), Handle<Mesh>>,
}
pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<WeatherMaterial>::default())
        .add_systems(OnEnter(GameState::HexWfc), setup)
        .add_systems(
            PostUpdate,
            (spawn, sync_power)
                .chain()
                .before(bevy::transform::TransformSystems::Propagate)
                .run_if(in_state(GameState::HexWfc)),
        )
        .add_systems(OnExit(GameState::HexWfc), cleanup);
}
fn setup(mut commands: Commands) {
    commands.init_resource::<WeatherAssets>();
}
fn cleanup(mut commands: Commands) {
    commands.remove_resource::<WeatherAssets>();
}
fn patch(
    commands: &mut Commands,
    parent: Entity,
    coord: HexCoord,
    heading: HexFace,
    origin: Vec3,
    kind: u8,
    points: Vec<Vec3>,
) {
    commands.spawn((
        WeatherPatch {
            coord,
            heading,
            kind,
            outline: points,
        },
        Transform::from_translation(origin),
        Visibility::default(),
        ChildOf(parent),
    ));
}
pub(super) fn marker(
    commands: &mut Commands,
    parent: Entity,
    coord: HexCoord,
    heading: HexFace,
    origin: Vec3,
    aperture: &[(f32, f32)],
    garden: &[(f32, f32)],
) {
    for (kind, points, y) in [(0, aperture, 0.53), (1, garden, 0.523), (2, aperture, 7.64)] {
        patch(
            commands,
            parent,
            coord,
            heading,
            origin,
            kind,
            points
                .iter()
                .map(|&(x, z)| turn(Vec3::new(x, y, z), heading))
                .collect(),
        );
    }
}
pub(super) fn lantern(
    commands: &mut Commands,
    parent: Entity,
    coord: HexCoord,
    heading: HexFace,
    origin: Vec3,
    at: Vec3,
) {
    let corners = [
        Vec3::new(-0.32, 0.0, -0.32),
        Vec3::new(0.32, 0.0, -0.32),
        Vec3::new(0.32, 0.0, 0.32),
        Vec3::new(-0.32, 0.0, 0.32),
    ];
    patch(
        commands,
        parent,
        coord,
        heading,
        origin,
        3,
        corners.map(|p| turn(at + p, heading)).to_vec(),
    );
}
fn material(kind: u8) -> WeatherMaterial {
    let palette =
        observed_style::architecture(observed_content::ArchitectureRegister::ShadowScreen);
    let haze = LinearRgba::from(palette.fog_color);
    let unpowered = LinearRgba::from(observed_style::rain_court::unpowered_panel_color());
    let c = LinearRgba::from(if kind == 3 {
        observed_style::rain_court::fixture_color()
    } else {
        observed_style::rain_court::weather_colors()[usize::from(kind.min(2))]
    });
    WeatherMaterial {
        settings: Settings {
            tint: Vec4::new(c.red, c.green, c.blue, 1.0),
            haze: Vec4::new(haze.red, haze.green, haze.blue, 1.0),
            unpowered: Vec4::new(unpowered.red, unpowered.green, unpowered.blue, 1.0),
            params: Vec4::new(1.0, f32::from(kind), palette.fog_start, palette.fog_end),
        },
    }
}
fn contains(p: Vec2, outline: &[Vec3]) -> bool {
    let mut sign = 0.0f32;
    for i in 0..outline.len() {
        let a = Vec2::new(outline[i].x, outline[i].z);
        let b = Vec2::new(
            outline[(i + 1) % outline.len()].x,
            outline[(i + 1) % outline.len()].z,
        );
        let cross = (b - a).perp_dot(p - a);
        if cross.abs() < 0.0001 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if sign * cross < 0.0 {
            return false;
        }
    }
    true
}
fn mesh(patch: &WeatherPatch) -> Mesh {
    let mut points = Vec::new();
    let mut uv = Vec::new();
    let mut indices = Vec::new();
    let mut quad = |p: [Vec3; 4], tex: [[f32; 2]; 4]| {
        let start = points.len() as u32;
        points.extend(p.map(|p| p.to_array()));
        uv.extend(tex);
        indices.extend([
            start,
            start + 1,
            start + 2,
            start,
            start + 2,
            start + 3,
            start + 2,
            start + 1,
            start,
            start + 3,
            start + 2,
            start,
        ]);
    };
    if patch.kind == 0 {
        // Deterministic distribution stays inside the aperture at every exact heading.
        for i in 0..120 {
            let x = 1.8 + (i * 37 % 101) as f32 / 101.0 * 5.2;
            let z = -0.2 + (i * 61 % 103) as f32 / 103.0 * 6.0;
            let p = turn(Vec3::new(x, 0.53, z), patch.heading);
            if !contains(Vec2::new(p.x, p.z), &patch.outline) {
                continue;
            }
            for axis in [Vec3::X, Vec3::Z] {
                let w = axis * 0.006;
                quad(
                    [p - w, p + w, p + w + Vec3::Y * 6.6, p - w + Vec3::Y * 6.6],
                    [
                        [i as f32, 0.0],
                        [i as f32, 0.0],
                        [i as f32, 6.6],
                        [i as f32, 6.6],
                    ],
                );
            }
        }
    } else {
        for i in 1..patch.outline.len() - 1 {
            let a = patch.outline[0];
            let b = patch.outline[i];
            let c = patch.outline[i + 1];
            quad(
                [a, b, c, c],
                [[a.x, a.z], [b.x, b.z], [c.x, c.z], [c.x, c.z]],
            );
        }
    }
    let n = points.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, points)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(indices))
}
fn spawn(
    mut commands: Commands,
    patches: Query<(Entity, &WeatherPatch, &Transform), Added<WeatherPatch>>,
    mut assets: ResMut<WeatherAssets>,
    mut materials: ResMut<Assets<WeatherMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (parent, patch, pose) in &patches {
        let material = assets
            .materials
            .entry((patch.coord.level, patch.kind))
            .or_insert_with(|| materials.add(material(patch.kind)))
            .clone();
        // Lanterns have two different local positions, so their tiny mesh stays local.
        let mesh = if patch.kind == 3 {
            meshes.add(mesh(patch))
        } else {
            assets
                .meshes
                .entry((patch.heading.index(), patch.kind))
                .or_insert_with(|| meshes.add(mesh(patch)))
                .clone()
        };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            ChildOf(parent),
            NotShadowCaster,
            crate::hex_wfc::view::NeverShadowCaster,
            NotShadowReceiver,
            Cutaway {
                local: Vec3::ZERO,
                min_y: 0.5,
                max_y: 7.7,
                origin_y: pose.translation.y,
                cell_level: patch.coord.level,
                climb_wall: false,
            },
            Name::new("Rain Court local weather or fixed aperture"),
        ));
    }
}
fn sync_power(
    runtime: Res<HexWfcRuntime>,
    assets: Res<WeatherAssets>,
    mut materials: ResMut<Assets<WeatherMaterial>>,
) {
    for (&(level, _), handle) in &assets.materials {
        let power = if runtime
            .ascent
            .as_ref()
            .is_none_or(|a| a.rules().economy.is_powered(level))
        {
            1.0
        } else {
            0.0
        };
        if let Some(mut m) = materials.get_mut(handle)
            && m.settings.params.x != power
        {
            m.settings.params.x = power;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weather_is_batched_once_and_owned_by_the_resident_cell() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<WeatherMaterial>>()
            .init_resource::<WeatherAssets>()
            .add_systems(Update, spawn);
        let cell = app
            .world_mut()
            .spawn((Transform::IDENTITY, Visibility::default()))
            .id();
        let patch = app
            .world_mut()
            .spawn((
                WeatherPatch {
                    coord: HexCoord {
                        q: 0,
                        r: 0,
                        level: 3,
                    },
                    heading: HexFace::East,
                    kind: 0,
                    outline: super::super::GARDEN
                        .map(|(x, z)| Vec3::new(x, 0.53, z))
                        .to_vec(),
                },
                Transform::IDENTITY,
                ChildOf(cell),
            ))
            .id();
        app.update();
        app.update();
        assert_eq!(app.world().get::<Children>(patch).unwrap().len(), 1);
        assert_eq!(
            app.world_mut().query::<&Mesh3d>().iter(app.world()).count(),
            1
        );
        app.world_mut().despawn(cell);
        app.update();
        assert_eq!(
            app.world_mut().query::<&Mesh3d>().iter(app.world()).count(),
            0
        );
    }
}
