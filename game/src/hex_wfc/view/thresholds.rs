//! The doorway model stood in each named threshold.
//!
//! Split from `shell` when that file reached the 600-line review budget. The
//! division is real: `shell` instances the authoritative geometry snapshot,
//! and this dresses the one place in it where a room meets a hall.

use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use observed_match::hex_wfc::derive_thresholds;

use super::HexWfcGeometry;
use observed_content::ArchitectureRegister;
use observed_match::hex_wfc::HexTrimKind;

use super::assets::HexWfcVisualAssets;
use crate::hex_wfc::sim::HexWfcRuntime;

/// The authored doorway's clear width, in meters.
///
/// `DOOR_HALF_WIDTH` is 36 in the forge's units and a hex lateral edge of ~129
/// of those reads as ~8 m, so the canonical opening is the 4.5 m the door
/// builder's own docs name.
const THRESHOLD_APERTURE: f32 = 4.5;

/// The gate model's own dimensions, so it can be fitted to an aperture.
const GATE_NATIVE_WIDTH: f32 = 4.2;
const GATE_NATIVE_HEIGHT: f32 = 4.620_71;
const GATE_NATIVE_DEPTH: f32 = 1.4;

/// How far a frame may stand proud of the wall it fills.
///
/// Fitting depth to the same factor as width gives a frame two meters thick,
/// which is most of a hall's walk channel. It is a jamb, not a tunnel.
const GATE_MAX_DEPTH: f32 = 0.5;

/// A doorway model at every named threshold, resident for the whole match.
///
/// Crossing from a hall into a room is a real transition and now the only place
/// a doorway exists at all: halls carry no wall across a connection, so the
/// aperture belongs to the room's authored port. Standing a frame in it is what
/// makes the crossing legible from a distance rather than something you notice
/// only once you are through it.
///
/// Resident rather than streamed with its cell, like the boundary shell. There
/// is one per named room port - tens, not the ~100k structural pieces - and a
/// doorway that popped in as you approached would defeat the point of marking a
/// transition in advance.
pub(in crate::hex_wfc::view) fn spawn_thresholds(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    runtime: &HexWfcRuntime,
) {
    let gate = assets.threshold_gate.clone();
    // Fit the model to the aperture it stands in rather than trusting the
    // asset's authored scale: the hex doorway is the forge's canonical opening,
    // and a frame that does not match it reads as a prop rather than the
    // building.
    // Per axis, and clamped: the aperture sets the width, the storey caps the
    // height, and depth is held to a jamb's thickness.
    let fitted =
        gate.as_ref().map_or(1.0, |gate| gate.scale) * THRESHOLD_APERTURE / GATE_NATIVE_WIDTH;
    let scale = Vec3::new(
        fitted.min(THRESHOLD_APERTURE / GATE_NATIVE_WIDTH),
        fitted.min((observed_hex::TILE_LEVEL_HEIGHT - 0.1) / GATE_NATIVE_HEIGHT),
        fitted.min(GATE_MAX_DEPTH / GATE_NATIVE_DEPTH),
    );
    // The model where there is one, a pair of jambs where there is not.
    // `screens::place` keeps the same fallback, and for the same reason: a
    // threshold that marks itself only while an asset happens to ship is one
    // that silently stops being marked, and being marked is the point.
    let post = assets.trim_mesh(meshes, HexTrimKind::Buttress);
    let material = assets.trim_material(ArchitectureRegister::ALL[0]);
    for piece in derive_thresholds(&runtime.match_state.facility) {
        let rotation = Quat::from_array(piece.rotation);
        let name = format!("Hex threshold {:?} {:?}", piece.cell, piece.face);
        if let Some(gate) = &gate {
            commands.spawn((
                WorldAssetRoot(gate.scene.clone()),
                Transform::from_translation(piece.position)
                    .with_rotation(rotation)
                    .with_scale(scale),
                Name::new(format!("{name} gate")),
                HexWfcGeometry,
                DespawnOnExit(crate::GameState::HexWfc),
            ));
            continue;
        }
        for side in [-1.0_f32, 1.0] {
            let offset = rotation * Vec3::new(side * THRESHOLD_APERTURE * 0.5, 0.0, 0.0);
            commands.spawn((
                Mesh3d(post.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(
                    piece.position + offset + Vec3::Y * observed_hex::TILE_LEVEL_HEIGHT * 0.45,
                )
                .with_rotation(rotation),
                Name::new(format!("{name} jamb")),
                HexWfcGeometry,
                DespawnOnExit(crate::GameState::HexWfc),
            ));
        }
    }
}
