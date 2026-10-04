//! Cell-owned diffuser meshes and authored practical lights.
use super::{HexPractical, assets::HexWfcVisualAssets};
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_hex::{HexCoord, hex_origin};
use observed_match::hex_wfc::{HexLightSource, HexStructureRole};
const PRACTICAL_HEIGHT: f32 = 5.6;

pub(super) struct PracticalProjection<'a> {
    pub(super) parent: Entity,
    pub(super) coord: HexCoord,
    /// The full set of cells this fixture group is responsible for lighting
    /// — `[coord]` for an ordinary tile, or a room's complete footprint when
    /// `coord` is a whole-room module's anchor (the shell supplies this footprint).
    pub(super) footprint: &'a [HexCoord],
    pub(super) architecture: ArchitectureRegister,
    pub(super) role: HexStructureRole,
    pub(super) composition: observed_style::HexComposition,
    pub(super) authored_lights: &'a [&'a HexLightSource],
    pub(super) wonder: Option<super::lighting::WonderLighting>,
}

/// Cell-owned lighting follows authored sources, with a fallback for unlit cells.
/// Ordinary point lights use the nearby shadow budget; wonders retain fixed downlights.
pub(super) fn spawn_cell_practicals(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    projection: PracticalProjection<'_>,
) -> usize {
    let PracticalProjection {
        parent,
        coord,
        footprint,
        architecture,
        role,
        composition,
        authored_lights,
        wonder,
    } = projection;
    if role == HexStructureRole::Boundary {
        return 0;
    }
    let has_authored_lights = !authored_lights.is_empty();
    let positions: Vec<Vec3> = if !has_authored_lights {
        // Defensive fallback: one fixture per footprint cell, not just the
        // anchor, so a whole-room module with no authored lights still has
        // every part of its floor lit (Legibility Contract). For an
        // ordinary tile `footprint` is exactly `[coord]`, so this produces
        // the same single fixture as before.
        footprint
            .iter()
            .map(|&cell| Vec3::from_array(hex_origin(cell)) + Vec3::Y * PRACTICAL_HEIGHT)
            .collect()
    } else {
        authored_lights
            .iter()
            .map(|source| source.position)
            .collect()
    };
    let practical = observed_style::hex_practical_light(architecture, composition, positions.len());
    let mut child_pieces = 0;
    for position in positions {
        if has_authored_lights
            && !matches!(wonder, Some(super::lighting::WonderLighting::Rain))
            && matches!(
                role,
                HexStructureRole::Room | HexStructureRole::Hall | HexStructureRole::Climb
            )
        {
            // A diffuser is geometry, so it gets the same two treatments the
            // rest of the geometry gets: the storey filter and the cutaway.
            //
            // It used to get neither. The light beside it carried
            // `HexPractical` and the mesh carried nothing, so it drew on every
            // storey at once whatever the cutaway said - and a ceiling-mounted
            // diffuser whose ceiling had been cut away is a thin bright stub
            // hanging in the air. That is what the "floating fixtures" over the
            // overview's floor plan were.
            let origin = Vec3::from_array(hex_origin(coord));
            let at = position + Vec3::Y * 0.18;
            commands.spawn((
                Mesh3d(assets.fixture_mesh(meshes)),
                MeshMaterial3d(assets.register(architecture).fixture()),
                Transform::from_translation(at),
                HexPractical(coord),
                // Measured as a point: a diffuser is small next to the tests
                // being applied to it, and its height is what decides them.
                super::spectate::Cutaway {
                    local: at - origin,
                    min_y: at.y - origin.y,
                    max_y: at.y - origin.y,
                    origin_y: origin.y,
                    cell_level: coord.level,
                    climb_wall: false,
                },
                ChildOf(parent),
                Name::new("Authored fluorescent diffuser"),
            ));
            child_pieces += 1;
        }
        child_pieces +=
            super::lighting::spawn_practical(commands, parent, coord, position, practical, wonder);
    }
    child_pieces
}
