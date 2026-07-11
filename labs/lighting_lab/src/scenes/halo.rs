//! Scene 5 — forerunner. Monumental indifference: a tall cool metal volume
//! whose side walls are chained obtuse-angle facets, cyan seam-lines along
//! every junction, and one wide volumetric shaft falling from a high aperture.
//! Architecture at a scale that was not built for you. This scene hosts the
//! volumetrics × bloom capture matrix.

use super::{SceneCtx, SceneSpawned};
use bevy::light::{FogVolume, VolumetricLight};
use bevy::prelude::*;
use observed_style as style;

pub fn spawn(ctx: &mut SceneCtx) {
    let (h, len) = (15.0_f32, 34.0_f32);
    let metal = ctx.metal(Color::srgb(0.10, 0.115, 0.14), 0.35);
    let metal_floor = ctx.metal(Color::srgb(0.08, 0.09, 0.11), 0.5);

    // Seam glow: the Archive district's structural accent, brightened for the
    // monumental read but still a structure color, not a signal color.
    let archive = style::district(style::District::Archive);
    let seam = ctx.glow(archive.accent, 6.0);

    ctx.slab(
        Vec3::new(0.0, -0.2, -len * 0.5),
        Vec3::new(30.0, 0.4, len + 8.0),
        metal_floor,
        "Deck",
    );
    ctx.slab(
        Vec3::new(0.0, h + 0.2, -len * 0.5),
        Vec3::new(30.0, 0.4, len + 8.0),
        metal.clone(),
        "Canopy",
    );
    ctx.slab(
        Vec3::new(0.0, h * 0.5, -len - 0.3),
        Vec3::new(30.0, h, 0.6),
        metal.clone(),
        "End wall",
    );

    // Each side wall: three facet panels chained at obtuse angles (the Halo CE
    // constraint — nothing meets at 90°). Seam strips run up every junction.
    for side in [-1.0_f32, 1.0] {
        let base_x = side * 9.0;
        let angles = [0.0_f32, 0.24, -0.20];
        let seg_len = len / 3.0;
        for (i, a) in angles.iter().enumerate() {
            let zc = -seg_len * (i as f32 + 0.5);
            let yaw = side * *a;
            let t = Transform::from_xyz(base_x + side * (i as f32 * 0.9), h * 0.5, zc)
                .with_rotation(Quat::from_rotation_y(yaw));
            ctx.slab_at(t, Vec3::new(0.7, h, seg_len + 1.2), metal.clone(), "Facet");
            // Junction seam: a vertical glow strip at the panel edge.
            let edge = t * Vec3::new(0.0, 0.0, -seg_len * 0.5);
            ctx.slab(
                Vec3::new(edge.x - side * 0.42, h * 0.5, edge.z),
                Vec3::new(0.08, h - 1.0, 0.08),
                seam.clone(),
                "Seam",
            );
        }
        // A floor-perimeter seam line the full length.
        ctx.slab(
            Vec3::new(base_x - side * 1.1, 0.06, -len * 0.5),
            Vec3::new(0.1, 0.06, len - 2.0),
            seam.clone(),
            "Floor seam",
        );
    }

    let rim_glow = ctx.glow(LinearRgba::rgb(0.7, 0.85, 1.0), 0.8);
    ctx.slab(
        Vec3::new(3.0, h + 0.1, -20.0),
        Vec3::new(4.4, 0.25, 4.4),
        rim_glow,
        "Aperture rim",
    );
    ctx.commands.spawn((
        SceneSpawned,
        SpotLight {
            color: Color::srgb(0.85, 0.93, 1.0),
            intensity: 220_000_000.0,
            range: 60.0,
            radius: 0.3,
            shadows_enabled: true,
            inner_angle: 0.20,
            outer_angle: 0.30,
            ..default()
        },
        VolumetricLight,
        Transform::from_xyz(3.0, h + 6.0, -20.0).looking_at(Vec3::new(1.0, 0.0, -18.0), Vec3::Y),
        Name::new("Monument shaft"),
    ));
    // The air the shaft becomes visible in.
    ctx.commands.spawn((
        SceneSpawned,
        FogVolume {
            fog_color: Color::srgb(0.55, 0.65, 0.8),
            density_factor: 0.06,
            absorption: 0.25,
            scattering: 0.45,
            ..default()
        },
        Transform::from_xyz(0.0, h * 0.5, -len * 0.5).with_scale(Vec3::new(28.0, h + 2.0, len)),
        Name::new("Hall air"),
    ));

    ctx.ambient(Color::srgb(0.4, 0.5, 0.7), 6.0);
    ctx.signal_kit(Vec3::new(-4.5, 0.0, -13.0), 0.5);
    // Low camera, wide view up the hall: the human is small here.
    ctx.camera(
        Transform::from_xyz(-5.5, 1.6, -2.0).looking_at(Vec3::new(2.5, 5.0, -22.0), Vec3::Y),
        Some(SceneCtx::fog(Color::srgb(0.008, 0.012, 0.02), 26.0, 80.0)),
    );
}
