//! Scene 5 — Facet Monument. A genuinely connected chain of obtuse wall facets
//! defines a huge cool-metal volume. The canopy is built around (not through)
//! an aperture, so bounded fog and a shadow-casting spotlight form a visible
//! shaft when volumetrics are enabled.

use super::{SceneCtx, SceneSpawned};
use bevy::light::{FogVolume, VolumetricLight};
use bevy::prelude::*;
use observed_style as style;

pub(crate) const FACET_HEADINGS_DEGREES: [f32; 4] = [0.0, 35.0, -15.0, 30.0];
const FACET_LENGTH: f32 = 9.0;

pub fn spawn(ctx: &mut SceneCtx) {
    let (height, length) = (18.0_f32, 36.0_f32);
    let metal = ctx.metal(Color::srgb(0.15, 0.175, 0.22), 0.35);
    let metal_alt = ctx.metal(Color::srgb(0.21, 0.245, 0.31), 0.38);
    let metal_floor = ctx.metal(Color::srgb(0.11, 0.125, 0.16), 0.5);
    let archive = style::district(style::District::Archive);
    let seam = ctx.glow(archive.accent, 12.0);

    ctx.slab(
        Vec3::new(0.0, -0.2, -length * 0.5),
        Vec3::new(38.0, 0.4, length + 8.0),
        metal_floor,
        "Facet Monument deck",
    );
    ctx.slab(
        Vec3::new(0.0, height * 0.5, -length - 0.3),
        Vec3::new(38.0, height, 0.6),
        metal.clone(),
        "Facet Monument end wall",
    );

    // Adjacent headings differ by 35°, 50°, and 45°: their interior junctions
    // are 145°, 130°, and 135°. Endpoints are shared, so seams mark actual
    // structural junctions rather than decoration on disconnected boxes.
    for side in [-1.0_f32, 1.0] {
        let mut from = Vec2::new(side * 9.0, 2.0);
        for (index, heading) in FACET_HEADINGS_DEGREES.iter().enumerate() {
            // Positive headings fold toward the room centre on both mirrored
            // sides, keeping every facet inside the playable visual field.
            let radians = (heading * -side).to_radians();
            let to = from + Vec2::new(radians.sin() * FACET_LENGTH, -radians.cos() * FACET_LENGTH);
            let panel_material = if index % 2 == 0 {
                metal.clone()
            } else {
                metal_alt.clone()
            };
            wall_between(ctx, from, to, height, panel_material);
            if index > 0 {
                ctx.slab(
                    Vec3::new(from.x - side * 0.46, height * 0.5, from.y),
                    Vec3::new(0.18, height - 1.0, 0.18),
                    seam.clone(),
                    "Facet junction seam",
                );
            }
            from = to;
        }
    }

    // Low, cool grazing fills reveal the facet planes without competing with
    // the single dominant shaft or spending the scene's shadow budget.
    for side in [-1.0_f32, 1.0] {
        ctx.commands.spawn((
            SceneSpawned,
            PointLight {
                color: Color::srgb(0.32, 0.48, 0.72),
                intensity: 1_200_000.0,
                range: 28.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(side * 2.5, 7.0, -15.0),
            Name::new("Facet grazing fill"),
        ));
    }

    // Four canopy slabs leave a real 8×8 opening at (3, -20). The former proof
    // placed a solid roof under its light, making vol-on/off indistinguishable.
    ctx.slab(
        Vec3::new(-8.0, height + 0.2, -17.0),
        Vec3::new(14.0, 0.4, 42.0),
        metal.clone(),
        "Canopy west of aperture",
    );
    ctx.slab(
        Vec3::new(13.0, height + 0.2, -17.0),
        Vec3::new(12.0, 0.4, 42.0),
        metal.clone(),
        "Canopy east of aperture",
    );
    ctx.slab(
        Vec3::new(3.0, height + 0.2, -6.0),
        Vec3::new(8.0, 0.4, 20.0),
        metal.clone(),
        "Canopy before aperture",
    );
    ctx.slab(
        Vec3::new(3.0, height + 0.2, -33.0),
        Vec3::new(8.0, 0.4, 14.0),
        metal,
        "Canopy beyond aperture",
    );
    for (center, size) in [
        (
            Vec3::new(-1.0, height + 0.12, -20.0),
            Vec3::new(0.18, 0.18, 8.0),
        ),
        (
            Vec3::new(7.0, height + 0.12, -20.0),
            Vec3::new(0.18, 0.18, 8.0),
        ),
        (
            Vec3::new(3.0, height + 0.12, -16.0),
            Vec3::new(8.0, 0.18, 0.18),
        ),
        (
            Vec3::new(3.0, height + 0.12, -24.0),
            Vec3::new(8.0, 0.18, 0.18),
        ),
    ] {
        ctx.slab(center, size, seam.clone(), "Open aperture rim");
    }

    // The landing stays dim enough that toggling volumetrics changes the air,
    // while still proving where the shaft lands when volumetrics are disabled.
    let landing = ctx.glow(LinearRgba::rgb(0.44, 0.60, 0.82), 0.42);
    ctx.slab_at(
        Transform::from_xyz(3.0, 0.025, -20.0).with_rotation(Quat::from_rotation_y(-0.18)),
        Vec3::new(7.0, 0.025, 5.0),
        landing,
        "Shaft landing quad",
    );
    ctx.commands.spawn((
        SceneSpawned,
        SpotLight {
            color: Color::srgb(0.85, 0.93, 1.0),
            intensity: 28_000_000.0,
            range: 72.0,
            radius: 0.45,
            shadows_enabled: true,
            inner_angle: 0.11,
            outer_angle: 0.18,
            ..default()
        },
        VolumetricLight,
        Transform::from_xyz(3.0, height + 8.0, -20.0)
            .looking_at(Vec3::new(3.0, 0.0, -20.0), Vec3::Z),
        Name::new("Facet Monument volumetric shaft"),
    ));
    // Keep a volumetric emitter inside the visible bounds. Some render
    // backends cull the off-screen spotlight emitter before the volume pass;
    // this fill preserves the same tall, bounded shaft on those backends.
    ctx.commands.spawn((
        SceneSpawned,
        PointLight {
            color: Color::srgb(0.72, 0.84, 1.0),
            intensity: 180_000.0,
            range: 42.0,
            shadows_enabled: false,
            ..default()
        },
        VolumetricLight,
        Transform::from_xyz(3.0, 12.0, -20.0),
        Name::new("Bounded shaft volume fill"),
    ));
    ctx.commands.spawn((
        SceneSpawned,
        FogVolume {
            fog_color: Color::srgb(0.55, 0.65, 0.8),
            density_factor: 0.08,
            absorption: 0.14,
            scattering: 0.46,
            ..default()
        },
        Transform::from_xyz(3.0, height * 0.5, -20.0).with_scale(Vec3::new(6.0, height, 7.0)),
        Name::new("Bounded shaft air"),
    ));

    ctx.ambient(Color::srgb(0.38, 0.47, 0.66), 72.0);
    ctx.signal_kit(Vec3::new(-4.8, 0.0, -12.0), 0.45);
    ctx.camera(
        Transform::from_xyz(0.0, 1.6, -1.5).looking_at(Vec3::new(0.0, 4.8, -23.0), Vec3::Y),
        Some(SceneCtx::fog(Color::srgb(0.008, 0.012, 0.02), 28.0, 86.0)),
    );
}

fn wall_between(
    ctx: &mut SceneCtx,
    from: Vec2,
    to: Vec2,
    height: f32,
    material: Handle<StandardMaterial>,
) {
    let delta = to - from;
    let midpoint = (from + to) * 0.5;
    let yaw = delta.x.atan2(delta.y);
    ctx.slab_at(
        Transform::from_xyz(midpoint.x, height * 0.5, midpoint.y)
            .with_rotation(Quat::from_rotation_y(yaw)),
        Vec3::new(0.75, height, delta.length() + 0.2),
        material,
        "Connected facet panel",
    );
}
