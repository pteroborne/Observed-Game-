//! Cell-owned diffuser meshes and authored practical lights.
use super::{HexPractical, assets::HexWfcVisualAssets};
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_hex::{HexCoord, hex_origin};
use observed_match::hex_wfc::{HexLightSource, HexStructurePiece, HexStructureRole};

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
    pub(super) pieces: &'a [&'a HexStructurePiece],
    pub(super) authored_lights: &'a [&'a HexLightSource],
    pub(super) wonder: Option<super::lighting::WonderLighting>,
    pub(super) fluorescent_field: bool,
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
        pieces,
        wonder,
        fluorescent_field,
    } = projection;
    if role == HexStructureRole::Boundary {
        return 0;
    }
    let has_authored_lights = !authored_lights.is_empty();
    let positions: Vec<(Vec3, Option<observed_authoring::LightAttachment>)> =
        if !has_authored_lights {
            let hulls = pieces
                .iter()
                .map(|p| super::support::points(p, Vec3::ZERO))
                .collect::<Vec<_>>();
            footprint
                .iter()
                .filter_map(|&cell| {
                    let source = Vec3::from_array(hex_origin(cell)) + Vec3::Y * 2.5;
                    let mount = observed_authoring::light_attachment(source, &hulls)?;
                    Some((
                        Vec3::from_array(mount.position) + Vec3::from_array(mount.normal) * 0.3,
                        Some(mount),
                    ))
                })
                .collect()
        } else {
            authored_lights
                .iter()
                .map(|source| (source.position, source.attachment))
                .collect()
        };
    let practical = observed_style::hex_practical_light(architecture, composition, positions.len());
    let mut child_pieces = 0;
    let mut mounted = std::collections::BTreeSet::new();
    for (position, attachment) in positions {
        if let Some(attachment) = attachment
            && !fluorescent_field
            && !matches!(
                wonder,
                Some(
                    super::lighting::WonderLighting::Rain
                        | super::lighting::WonderLighting::Concourse
                        | super::lighting::WonderLighting::Jade
                        | super::lighting::WonderLighting::Promenade
                )
            )
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
            let normal = Vec3::from_array(attachment.normal);
            let support = Vec3::from_array(attachment.position);
            let at = if normal.y > 0.5 {
                position + Vec3::Y * 0.18
            } else {
                support + normal * 0.025
            };
            let key = [at.x, at.y, at.z, normal.x, normal.y, normal.z]
                .map(|v| (v * 1000.0).round() as i32);
            if !mounted.insert(key) {
                // Multi-storey sources can share a roof mount. Draw one fixture
                // while retaining their independently authored illumination.
                child_pieces += super::lighting::spawn_practical(
                    commands, parent, coord, position, practical, wonder,
                );
                continue;
            }
            if normal.y > 0.5 {
                let top = at.y;
                let height = (top - support.y).max(0.02);
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.06, height, 0.06))),
                    MeshMaterial3d(assets.material_for_group(
                        architecture,
                        super::mesh_group::MeshGroupKey::Interior,
                    )),
                    Transform::from_translation(Vec3::new(at.x, support.y + height * 0.5, at.z)),
                    HexPractical(coord),
                    ChildOf(parent),
                    super::spectate::Cutaway {
                        local: at - origin,
                        min_y: support.y - origin.y,
                        max_y: top - origin.y,
                        origin_y: origin.y,
                        cell_level: coord.level,
                        climb_wall: false,
                    },
                    Name::new("Surface-mounted lamp post"),
                    super::NeverShadowCaster,
                    bevy::light::NotShadowCaster,
                ));
                child_pieces += 1;
            }
            if architecture == ArchitectureRegister::ShadowScreen
                && wonder.is_none()
                && normal.y < -0.9
            {
                child_pieces +=
                    super::zen::fixture_frame(commands, assets, meshes, parent, coord, at);
            }
            commands.spawn((
                Mesh3d(assets.fixture_mesh(meshes, architecture)),
                MeshMaterial3d(assets.register(architecture).fixture()),
                Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(
                    Vec3::NEG_Y,
                    if normal.y > 0.5 { Vec3::NEG_Y } else { normal },
                )),
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
