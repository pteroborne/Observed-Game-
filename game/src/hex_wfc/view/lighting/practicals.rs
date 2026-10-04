//! Cell-owned light sources. Reservoir downlights retain their illumination
//! and shadows throughout the composition; they do not enter the moving
//! nearest-point-light shadow budget. A spotlight uses one shadow face.

use bevy::prelude::*;
use observed_hex::HexCoord;
use observed_style::{HexPracticalLight, cistern};

use super::super::HexPractical;

#[derive(Component)]
pub(in crate::hex_wfc) struct FixedReservoirLight;

pub(in crate::hex_wfc::view) fn spawn_practical(
    commands: &mut Commands,
    parent: Entity,
    coord: HexCoord,
    position: Vec3,
    practical: HexPracticalLight,
    reservoir: bool,
) -> usize {
    let mut light = commands.spawn((
        HexPractical(coord),
        Transform::from_translation(position),
        ChildOf(parent),
        Name::new("Authored tile practical"),
    ));
    if reservoir {
        light.insert((
            FixedReservoirLight,
            SpotLight {
                color: cistern::fixture_color(),
                intensity: cistern::FIXTURE_INTENSITY,
                range: cistern::FIXTURE_RANGE,
                radius: cistern::FIXTURE_RADIUS,
                inner_angle: cistern::FIXTURE_INNER_ANGLE,
                outer_angle: cistern::FIXTURE_OUTER_ANGLE,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_translation(position).looking_to(Vec3::NEG_Y, Vec3::Z),
            Name::new("Fixed Cistern fluorescent downlight"),
        ));
        commands.spawn((
            HexPractical(coord),
            FixedReservoirLight,
            PointLight {
                color: cistern::fixture_color(),
                intensity: cistern::FIXTURE_BOUNCE_INTENSITY,
                range: cistern::FIXTURE_BOUNCE_RANGE,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(position - Vec3::Y * 0.5),
            ChildOf(parent),
            Name::new("Fixed Cistern diffuse bounce fill"),
        ));
        2
    } else {
        light.insert(PointLight {
            color: practical.color,
            intensity: practical.intensity,
            range: practical.range,
            radius: practical.radius,
            shadow_maps_enabled: false,
            ..default()
        });
        1
    }
}
