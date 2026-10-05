//! Cell-owned light sources. Reservoir downlights retain their illumination
//! and shadows throughout the composition; they do not enter the moving
//! nearest-point-light shadow budget. A spotlight uses one shadow face.

use bevy::prelude::*;
use observed_hex::HexCoord;
use observed_style::{HexPracticalLight, cistern};

use super::super::HexPractical;

#[derive(Component)]
pub(in crate::hex_wfc) struct FixedPlaceLight;

#[derive(Clone, Copy)]
pub(in crate::hex_wfc) enum WonderLighting {
    Cistern,
    Chargeworks,
    Archive,
    Rain,
    Concourse,
}

pub(in crate::hex_wfc::view) fn spawn_practical(
    commands: &mut Commands,
    parent: Entity,
    coord: HexCoord,
    position: Vec3,
    practical: HexPracticalLight,
    wonder: Option<WonderLighting>,
) -> usize {
    let mut light = commands.spawn((
        HexPractical(coord),
        Transform::from_translation(position),
        ChildOf(parent),
        Name::new("Authored tile practical"),
    ));
    if let Some(theme) = wonder {
        let (color, intensity, bounce) = match theme {
            WonderLighting::Cistern => (
                cistern::fixture_color(),
                cistern::FIXTURE_INTENSITY,
                cistern::FIXTURE_BOUNCE_INTENSITY,
            ),
            WonderLighting::Chargeworks => (
                observed_style::chargeworks::fixture_color(),
                observed_style::chargeworks::FIXTURE_INTENSITY,
                observed_style::chargeworks::FIXTURE_BOUNCE_INTENSITY,
            ),
            WonderLighting::Concourse => (
                observed_style::concourse::fixture_color(),
                observed_style::concourse::FIXTURE_INTENSITY,
                observed_style::concourse::FIXTURE_BOUNCE_INTENSITY,
            ),
            WonderLighting::Rain => (
                observed_style::rain_court::fixture_color(),
                observed_style::rain_court::FIXTURE_INTENSITY,
                observed_style::rain_court::FIXTURE_BOUNCE_INTENSITY,
            ),
            WonderLighting::Archive => (
                observed_style::archive::fixture_color(),
                observed_style::archive::FIXTURE_INTENSITY,
                observed_style::archive::FIXTURE_BOUNCE_INTENSITY,
            ),
        };
        light.insert((
            FixedPlaceLight,
            SpotLight {
                color,
                intensity,
                range: cistern::FIXTURE_RANGE,
                radius: cistern::FIXTURE_RADIUS,
                inner_angle: cistern::FIXTURE_INNER_ANGLE,
                outer_angle: cistern::FIXTURE_OUTER_ANGLE,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_translation(position).looking_to(Vec3::NEG_Y, Vec3::Z),
            Name::new("Fixed wonder work downlight"),
        ));
        commands.spawn((
            HexPractical(coord),
            FixedPlaceLight,
            PointLight {
                color,
                intensity: bounce,
                range: cistern::FIXTURE_BOUNCE_RANGE,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(position - Vec3::Y * 0.5),
            ChildOf(parent),
            Name::new("Fixed wonder diffuse bounce fill"),
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
