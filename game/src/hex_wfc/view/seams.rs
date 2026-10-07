//! Non-colliding seam decoration.
use super::assets::HexWfcVisualAssets;
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_hex::hex_origin;
use observed_match::hex_wfc::HexTrimPiece;
pub(super) fn spawn_trim(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    piece: &HexTrimPiece,
    architecture: ArchitectureRegister,
    parent: Entity,
) {
    let mesh = assets.trim_mesh(meshes, piece.kind);
    let material = assets.trim_material(architecture);
    // Trim is geometry too, and was the last thing in this view drawing itself
    // on every storey whatever the cutaway said.
    //
    // Seam trim is a lintel or a threshold: it sits at head height on a face,
    // which is precisely where a cutaway removes the wall around it. Untagged,
    // a lintel outlived the doorway it framed and the ceiling above it, and
    // stood in the air over the floor plan.
    let origin = Vec3::from_array(hex_origin(piece.cell));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(piece.position).with_rotation(Quat::from_array(piece.rotation)),
        Name::new(format!("Hex trim {:?} {:?}", piece.kind, piece.face)),
        // Measured as a point, like a diffuser: trim is small next to the
        // tests being applied to it, and its height decides them.
        crate::hex_wfc::view::spectate::Cutaway {
            local: piece.position - origin,
            min_y: piece.position.y - origin.y,
            max_y: piece.position.y - origin.y,
            origin_y: origin.y,
            cell_level: piece.cell.level,
            climb_wall: false,
        },
        ChildOf(parent),
    ));
}
