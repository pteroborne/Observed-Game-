//! The sky outside the facility: a dome darkest straight down, a sun or a large moon
//! and a field of stars above, and a cloud sea in two layers below the lattice. Seen
//! only where the building opens onto it.
//!
//! Each floor has its own ([`observed_style::open_air::sky_mood`], by the floor's
//! district): the Backrooms' dreamcore dusk, Zen's sunset, the Reactor's moonlit night,
//! golden hour over the sky floor. Climbing crossfades from one floor's sky to the
//! next's ([`sync_mood`]).
//!
//! The colours and the cloud texture are `observed_style::open_air`'s, the same ones
//! the Architect's cutaway and `labs/vista_lab` draw, so all three views of the same
//! air agree. Two layers at different depths, drifting at different speeds, are the
//! parallax that makes the drop read as a drop.
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::math::Affine2;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use observed_content::ArchitectureRegister;
use observed_style::open_air::{
    CLOUD_TEXTURE_SIZE, MOON_ANGULAR_DIAMETER, MOON_TEXTURE_SIZE, SkyMood, cloud_rgba, halo_rgba,
    moon_rgba, sky_mood, stars, sun_rgba,
};

use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;
use crate::view::components::GameCam;

/// The dome's radius: inside the play camera's far plane, outside everything else.
const DOME_RADIUS: f32 = 900.0;

/// The cloud layers below the lattice: depth, opacity, tiling, drift in tiles/second.
const LAYERS: [(f32, f32, f32, Vec2); 2] = [
    (-24.0, 0.62, 16.0, Vec2::new(0.0022, 0.0009)),
    (-60.0, 0.8, 9.0, Vec2::new(-0.0011, 0.0016)),
];

/// How long one floor's sky takes to become the next's, seconds.
const SKY_FADE_SECONDS: f32 = 2.0;
/// Where the disc and its glow hang inside the unit dome. Far enough in that even the
/// glow's corners never reach it: a quad that pokes through the dome is clipped along
/// the dome's facets.
const DISC_AT: f32 = 0.72;
const GLOW_AT: f32 = 0.7;

#[derive(Component)]
pub(in crate::hex_wfc) struct SkyDome;

/// The sky's light: the one directional light in the facility, from the floor's sun or
/// moon.
#[derive(Component)]
pub(in crate::hex_wfc) struct HexMoon;

/// One of the things hanging in the sky whose look follows the floor's mood.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::hex_wfc) enum SkyBody {
    Moon,
    Sun,
    Glow,
    Stars,
}

/// The sky now, and the crossfade toward the floor the eye is on.
#[derive(Resource, Clone, Debug)]
pub(in crate::hex_wfc) struct HexSky {
    pub(in crate::hex_wfc) now: SkyMood,
    from: SkyMood,
    to: SkyMood,
    progress: f32,
}

impl HexSky {
    fn settled(mood: SkyMood) -> Self {
        Self {
            now: mood,
            from: mood,
            to: mood,
            progress: 1.0,
        }
    }
}

/// The sky over the floor of `cell` in `world`: its district's, by the climb.
pub(in crate::hex_wfc) fn mood_at(
    world: &observed_facility::hex_wfc::HexWfcWorld,
    cell: observed_facility::hex_wfc::HexCoord,
) -> SkyMood {
    sky_mood(ArchitectureRegister::for_floor(
        cell.level,
        world.config.levels,
    ))
}

/// `OBSERVED2_HEX_MOONLIGHT=off` leaves the moon in the sky and its light out of the
/// scene: the switch the moonlight's cost is measured against.
const MOONLIGHT_ENV: &str = "OBSERVED2_HEX_MOONLIGHT";

/// Light from where the floor's sun or moon hangs, shadowed, so that walls and ceilings
/// keep it out:
/// it reaches a loggia's floor and a walkway, and nothing sealed (the vista capture's
/// `sealed_room` still is the same with it and without it). Cascades cover the
/// resident cells and stop there, because the far skin carries the moon's light baked
/// in and casts nothing.
///
/// Measured on the Phase 101 arc gate, uncapped: +140 us median frame, +95 us p95.
pub(super) fn spawn_moonlight(commands: &mut Commands, mood: &SkyMood) {
    if std::env::var(MOONLIGHT_ENV).is_ok_and(|value| value.eq_ignore_ascii_case("off")) {
        return;
    }
    commands.spawn((
        HexMoon,
        DirectionalLight {
            color: observed_style::open_air::sunlight(mood),
            illuminance: mood.lux,
            shadow_maps_enabled: true,
            ..default()
        },
        bevy::light::CascadeShadowConfigBuilder {
            num_cascades: 3,
            minimum_distance: 0.3,
            first_cascade_far_bound: 12.0,
            maximum_distance: 70.0,
            overlap_proportion: 0.2,
        }
        .build(),
        Transform::from_translation(Vec3::from_array(mood.toward)).looking_at(Vec3::ZERO, Vec3::Y),
        DespawnOnExit(GameState::HexWfc),
        Name::new("Moonlight"),
    ));
}

#[derive(Component)]
pub(in crate::hex_wfc) struct CloudLayer {
    material: Handle<StandardMaterial>,
    tiles: f32,
    drift: Vec2,
    opacity: f32,
}

/// Colour the dome's vertices for `mood`, darkest straight down.
fn paint_dome(dome: &mut Mesh, mood: &SkyMood) {
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        dome.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        let colors: Vec<[f32; 4]> = positions
            .iter()
            .map(|p| {
                let c = mood.along(p[1]);
                [c.red, c.green, c.blue, 1.0]
            })
            .collect();
        dome.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
}

/// Spawn the dome and the cloud sea, centred on the facility, under `mood`.
pub(super) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    center: Vec3,
    mood: SkyMood,
) {
    commands.insert_resource(HexSky::settled(mood));
    let mut dome = Sphere::new(1.0).mesh().uv(64, 32);
    paint_dome(&mut dome, &mood);
    let dome = commands
        .spawn((
            SkyDome,
            Mesh3d(meshes.add(dome)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                unlit: true,
                fog_enabled: false,
                cull_mode: None,
                ..default()
            })),
            Transform::from_translation(center).with_scale(Vec3::splat(DOME_RADIUS)),
            NotShadowCaster,
            crate::hex_wfc::view::NeverShadowCaster,
            NotShadowReceiver,
            DespawnOnExit(GameState::HexWfc),
            Name::new("Sky dome"),
        ))
        .id();
    // Children of the dome, in its unit-sphere frame: at infinity, and with the eye.
    for (body, kind) in heavens(meshes, materials, images, &mood) {
        commands.spawn((body, kind, ChildOf(dome)));
    }

    let texture = images.add(cloud_image());
    for (depth, opacity, tiles, drift) in LAYERS {
        let material = materials.add(StandardMaterial {
            base_color: mood.cloud.with_alpha(opacity),
            base_color_texture: Some(texture.clone()),
            uv_transform: Affine2::from_scale(Vec2::splat(tiles)),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            CloudLayer {
                material: material.clone(),
                tiles,
                drift,
                opacity,
            },
            Mesh3d(meshes.add(Plane3d::default().mesh().size(4_000.0, 4_000.0))),
            MeshMaterial3d(material),
            Transform::from_xyz(center.x, depth, center.z),
            NotShadowCaster,
            crate::hex_wfc::view::NeverShadowCaster,
            NotShadowReceiver,
            DespawnOnExit(GameState::HexWfc),
            Name::new("Cloud sea"),
        ));
    }
}

/// What hangs in the sky: the moon, a sun, the glow round whichever shows, and the
/// stars, each placed in the dome's unit-sphere frame just inside it, so the dome's
/// scale puts them at the horizon's distance and the dome's position keeps them with
/// the eye. Both discs are always there; the mood says which one shows.
fn heavens(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    mood: &SkyMood,
) -> Vec<(impl Bundle, SkyBody)> {
    let disc_width = DISC_AT * MOON_ANGULAR_DIAMETER;
    let glow_width = GLOW_AT * MOON_ANGULAR_DIAMETER * 2.4;
    let sky_body = |texture: Option<Handle<Image>>, alpha: AlphaMode| StandardMaterial {
        base_color_texture: texture,
        alpha_mode: alpha,
        unlit: true,
        fog_enabled: false,
        cull_mode: None,
        ..default()
    };
    let mut material = |kind: SkyBody, base: StandardMaterial| {
        let mut base = base;
        base.base_color = body_color(kind, mood);
        materials.add(base)
    };
    let glow = material(
        SkyBody::Glow,
        sky_body(
            Some(images.add(square_image(halo_rgba(), MOON_TEXTURE_SIZE))),
            AlphaMode::Add,
        ),
    );
    let moon = material(
        SkyBody::Moon,
        sky_body(
            Some(images.add(square_image(moon_rgba(), MOON_TEXTURE_SIZE))),
            AlphaMode::Blend,
        ),
    );
    let sun = material(
        SkyBody::Sun,
        sky_body(
            Some(images.add(square_image(sun_rgba(), MOON_TEXTURE_SIZE))),
            AlphaMode::Add,
        ),
    );
    let stars = material(SkyBody::Stars, sky_body(None, AlphaMode::Add));
    vec![
        (
            (
                Mesh3d(meshes.add(Rectangle::new(glow_width, glow_width))),
                MeshMaterial3d(glow),
                body_pose(SkyBody::Glow, mood),
                NotShadowCaster,
                crate::hex_wfc::view::NeverShadowCaster,
                Name::new("Sky glow"),
            ),
            SkyBody::Glow,
        ),
        (
            (
                Mesh3d(meshes.add(Rectangle::new(disc_width, disc_width))),
                MeshMaterial3d(moon),
                body_pose(SkyBody::Moon, mood),
                NotShadowCaster,
                crate::hex_wfc::view::NeverShadowCaster,
                Name::new("Moon"),
            ),
            SkyBody::Moon,
        ),
        (
            (
                Mesh3d(meshes.add(Rectangle::new(disc_width, disc_width))),
                MeshMaterial3d(sun),
                body_pose(SkyBody::Sun, mood),
                NotShadowCaster,
                crate::hex_wfc::view::NeverShadowCaster,
                Name::new("Sun"),
            ),
            SkyBody::Sun,
        ),
        (
            (
                Mesh3d(meshes.add(star_field())),
                MeshMaterial3d(stars),
                Transform::IDENTITY,
                NotShadowCaster,
                crate::hex_wfc::view::NeverShadowCaster,
                Name::new("Stars"),
            ),
            SkyBody::Stars,
        ),
    ]
}

/// How one of the sky's bodies is coloured under `mood`. The moon fades by its alpha,
/// the additive sun, glow and stars by their colour.
fn body_color(kind: SkyBody, mood: &SkyMood) -> Color {
    match kind {
        SkyBody::Moon => Color::LinearRgba(mood.disc.with_alpha(mood.moon)),
        SkyBody::Sun => Color::LinearRgba(mood.disc * (1.0 - mood.moon)),
        SkyBody::Glow => Color::LinearRgba(mood.glow),
        SkyBody::Stars => Color::LinearRgba(LinearRgba::WHITE * mood.stars),
    }
}

/// Where one of the sky's bodies hangs under `mood`, facing the eye.
fn body_pose(kind: SkyBody, mood: &SkyMood) -> Transform {
    let toward = Vec3::from_array(mood.toward);
    let at = match kind {
        SkyBody::Moon | SkyBody::Sun => DISC_AT,
        SkyBody::Glow => GLOW_AT,
        SkyBody::Stars => return Transform::IDENTITY,
    };
    Transform::from_translation(toward * at).looking_at(Vec3::ZERO, Vec3::Y)
}

/// Every star as a tiny quad facing the eye, brightness in its vertex colour.
fn star_field() -> Mesh {
    let field = stars(1_800);
    let mut positions = Vec::with_capacity(field.len() * 4);
    let mut colors = Vec::with_capacity(field.len() * 4);
    let mut indices = Vec::with_capacity(field.len() * 6);
    for star in field {
        let direction = Vec3::from_array(star.direction);
        let right = Vec3::Y.cross(direction).normalize_or(Vec3::X);
        let up = direction.cross(right);
        let at = direction * 0.97;
        let half = 0.97 * star.size * 0.5;
        let base = u32::try_from(positions.len()).expect("star count fits");
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push((at + (right * x + up * y) * half).to_array());
            colors.push([
                star.brightness,
                star.brightness,
                star.brightness * 1.08,
                1.0,
            ]);
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}

fn square_image(data: Vec<u8>, size: u32) -> Image {
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn cloud_image() -> Image {
    let size = CLOUD_TEXTURE_SIZE;
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        cloud_rgba(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// The dome is centred on the eye, so the horizon is always at eye height.
pub(in crate::hex_wfc) fn follow_camera(
    camera: Query<&Transform, With<GameCam>>,
    mut dome: Query<&mut Transform, (With<SkyDome>, Without<GameCam>)>,
) {
    let (Ok(eye), Ok(mut dome)) = (camera.single(), dome.single_mut()) else {
        return;
    };
    dome.translation = eye.translation;
}

/// Settle the sky at once on the floor the eye is on, rather than crossfading: for a
/// capture that places the runner on another floor and takes a still, where a fade
/// would be caught half done. Consumed by [`sync_mood`].
#[derive(Resource)]
pub(in crate::hex_wfc) struct SettleSky;

/// The sky's directional light, apart from the bodies hanging in the sky.
type SkyLight<'w, 's> = Query<
    'w,
    's,
    (&'static mut DirectionalLight, &'static mut Transform),
    (With<HexMoon>, Without<SkyBody>),
>;

/// Crossfade the sky toward the floor the eye is on: the dome, the clouds, the sun or
/// moon and the stars, and the light they give. Nothing is touched once it has settled.
#[allow(clippy::too_many_arguments)]
pub(in crate::hex_wfc) fn sync_mood(
    mut commands: Commands,
    settle: Option<Res<SettleSky>>,
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    sky: Option<ResMut<HexSky>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    dome: Query<&Mesh3d, With<SkyDome>>,
    mut bodies: Query<(&SkyBody, &MeshMaterial3d<StandardMaterial>, &mut Transform)>,
    clouds: Query<&CloudLayer>,
    mut light: SkyLight,
) {
    let Some(mut sky) = sky else {
        return;
    };
    let target = mood_at(&runtime.match_state.facility, runtime.viewed().cell);
    if target != sky.to {
        sky.from = sky.now;
        sky.to = target;
        sky.progress = 0.0;
    }
    if settle.is_some() {
        commands.remove_resource::<SettleSky>();
        sky.from = target;
        sky.progress = 0.0;
    }
    if sky.progress >= 1.0 {
        return;
    }
    sky.progress = (sky.progress + time.delta_secs() / SKY_FADE_SECONDS).min(1.0);
    let t = sky.progress * sky.progress * (3.0 - 2.0 * sky.progress);
    let mood = sky.from.toward_mood(&sky.to, t);
    sky.now = mood;

    if let Ok(handle) = dome.single()
        && let Some(mut mesh) = meshes.get_mut(&handle.0)
    {
        paint_dome(&mut mesh, &mood);
    }
    for (&kind, material, mut transform) in &mut bodies {
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.base_color = body_color(kind, &mood);
        }
        *transform = body_pose(kind, &mood);
    }
    for layer in &clouds {
        if let Some(mut material) = materials.get_mut(&layer.material) {
            material.base_color = mood.cloud.with_alpha(layer.opacity);
        }
    }
    if let Ok((mut light, mut transform)) = light.single_mut() {
        light.color = observed_style::open_air::sunlight(&mood);
        light.illuminance = mood.lux;
        *transform = Transform::from_translation(Vec3::from_array(mood.toward))
            .looking_at(Vec3::ZERO, Vec3::Y);
    }
}

pub(in crate::hex_wfc) fn drift_clouds(
    time: Res<Time>,
    layers: Query<&CloudLayer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    for layer in &layers {
        if let Some(mut material) = materials.get_mut(&layer.material) {
            material.uv_transform = Affine2::from_scale_angle_translation(
                Vec2::splat(layer.tiles),
                0.0,
                (layer.drift * t).fract(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;

    use super::*;

    /// The light comes from where the disc hangs. They are placed by the same
    /// constant, but a light points along its forward axis and a quad faces back at
    /// the eye, so one sign error would put the shadows on the moon's side.
    #[test]
    fn the_moonlight_shines_from_the_moon_and_casts_shadows() {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let night = observed_style::open_air::night();
        spawn_moonlight(&mut Commands::new(&mut queue, &world), &night);
        queue.apply(&mut world);
        let mut lights = world.query::<(&DirectionalLight, &Transform)>();
        let (light, transform) = lights.single(&world).expect("one moonlight");
        assert!(
            light.shadow_maps_enabled,
            "unshadowed, it lights every room"
        );
        let travels = transform.forward().as_vec3();
        let from_moon = -Vec3::from_array(night.toward);
        assert!(
            travels.dot(from_moon) > 0.999_9,
            "light travels {travels}, but the moon shines along {from_moon}"
        );
    }

    /// Under a sun the moon is gone and the sun shows; under the night, the reverse. The
    /// stars show only where the mood has them.
    #[test]
    fn a_floor_shows_its_own_sun_or_moon() {
        use observed_content::ArchitectureRegister as R;
        let zen = sky_mood(R::ShadowScreen);
        let night = observed_style::open_air::night();
        let alpha = |kind, mood: &SkyMood| body_color(kind, mood).to_linear().alpha;
        let lit =
            |kind, mood: &SkyMood| observed_style::luminance(body_color(kind, mood).to_linear());
        assert_eq!(alpha(SkyBody::Moon, &zen), 0.0);
        assert!(lit(SkyBody::Sun, &zen) > 1.0);
        assert_eq!(lit(SkyBody::Stars, &zen), 0.0);
        assert_eq!(alpha(SkyBody::Moon, &night), 1.0);
        assert_eq!(lit(SkyBody::Sun, &night), 0.0);
        assert!(lit(SkyBody::Stars, &night) > 0.9);
        // The disc hangs where the light comes from.
        let pose = body_pose(SkyBody::Sun, &zen);
        assert!(
            pose.translation
                .normalize()
                .dot(Vec3::from_array(zen.toward))
                > 0.999
        );
    }
}
