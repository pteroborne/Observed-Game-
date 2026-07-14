//! Shared match camera/view helpers.
//!
//! The match has several useful "views" of the same place: the player's first-person
//! eye, a doorway preview, a guardian/inspection angle, and future tethered-room views.
//! Keep those transforms here so evidence capture, debug views, and passage previews
//! share one vocabulary instead of re-deriving camera math ad hoc.

use bevy::prelude::*;
use observed_traversal::{FpsBody, FpsConfig};

use crate::teleport::{self, DoorGap, PlaceGeom};

#[derive(Clone, Copy, Debug)]
pub(crate) struct MatchView {
    pub transform: Transform,
}

impl MatchView {
    pub fn apply_to(self, target: &mut Transform) {
        *target = self.transform;
    }
}

/// Build the renderer `Transform` that places a previewed child place per a
/// [`teleport::Align2d`] (translate by its XZ offset, rotate by its yaw about +Y).
/// Passage-preview geometry and any camera looking into that preview use this same
/// alignment, so "what you see" and "what you enter" are mechanically identical.
pub(crate) fn alignment_transform(a: teleport::Align2d) -> Transform {
    Transform::from_xyz(a.offset.x, 0.0, a.offset.y).with_rotation(Quat::from_rotation_y(a.yaw))
}

pub(crate) fn player_view(body: &FpsBody, config: &FpsConfig) -> MatchView {
    MatchView {
        transform: Transform::from_translation(body.eye(config))
            .looking_to(body.look_dir(), Vec3::Y),
    }
}

pub(crate) fn bot_view(body: &FpsBody, config: &FpsConfig) -> MatchView {
    MatchView {
        transform: player_view(body, config).transform,
    }
}

/// A first-person doorway diagnostic: stand just inside a threshold and look through
/// its outward normal. This is used for evidence captures of the previewed place.
pub(crate) fn doorway_preview_view(
    gap: &DoorGap,
    eye_height: f32,
    stand_back: f32,
    pitch: f32,
) -> MatchView {
    let n = gap.normal.normalize_or_zero();
    let stand = gap.center - n * stand_back;
    let (sp, cp) = pitch.sin_cos();
    let dir = Vec3::new(n.x * cp, sp, n.y * cp).normalize_or(Vec3::NEG_Z);
    MatchView {
        transform: Transform::from_xyz(stand.x, eye_height, stand.y).looking_to(dir, Vec3::Y),
    }
}

/// Body pose matching [`doorway_preview_view`], for systems that still drive the shared
/// first-person camera from `TeleportState`.
pub(crate) fn doorway_body_pose(
    gap: &DoorGap,
    body_center_y: f32,
    stand_back: f32,
    pitch: f32,
) -> (Vec3, f32, f32) {
    let n = gap.normal.normalize_or_zero();
    let stand = gap.center - n * stand_back;
    let yaw = n.x.atan2(-n.y);
    (Vec3::new(stand.x, body_center_y, stand.y), yaw, pitch)
}

/// Top-down guardian view of the current place. It is intentionally unframed and purely
/// derived from place geometry, so it can inspect rooms, hallways, and tethered previews.
// Kept as a named debug hook for the camera system; no UI command switches to it yet.
#[allow(dead_code)]
pub(crate) fn guardian_view(geom: &PlaceGeom, wall_height: f32) -> MatchView {
    let radius = geom.half.x.max(geom.half.y);
    MatchView {
        transform: Transform::from_xyz(0.0, (radius * 2.2).max(wall_height * 7.0), 0.1)
            .looking_at(Vec3::ZERO, Vec3::NEG_Z),
    }
}

/// Oblique debug view reserved for inspecting an anchored/tethered room relation. It
/// shares the same place framing as the guardian view but keeps walls readable.
// Kept as a named debug hook for the camera system; the capture flow currently uses the
// first-person bot view, but tethered-room inspection should land here instead of inlining
// another camera transform.
#[allow(dead_code)]
pub(crate) fn tethered_room_view(
    room: observed_core::RoomId,
    geom: &PlaceGeom,
    wall_height: f32,
) -> MatchView {
    let radius = geom.half.x.max(geom.half.y);
    let lane = (room.0 % 3) as f32 - 1.0;
    MatchView {
        transform: Transform::from_xyz(
            lane * 2.0,
            (radius * 1.15).max(wall_height * 2.0),
            radius + 8.0,
        )
        .looking_at(Vec3::new(0.0, wall_height * 0.4, 0.0), Vec3::Y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_core::RoomId;

    fn forward(transform: Transform) -> Vec3 {
        transform.rotation * Vec3::NEG_Z
    }

    fn test_threshold(room: RoomId, target: RoomId) -> teleport::ThresholdLink {
        teleport::ThresholdLink {
            room: teleport::RoomThreshold {
                room,
                slot: teleport::ThresholdSlotId(0),
            },
            hall: teleport::HallThreshold {
                corridor: teleport::corridor_id_for(room, target),
                slot: teleport::ThresholdSlotId(0),
            },
            local_side: teleport::ThresholdLocalSide::Room,
        }
    }

    #[test]
    fn doorway_preview_view_looks_through_the_gap_normal() {
        let gap = DoorGap {
            center: Vec2::new(0.0, 8.0),
            normal: Vec2::Y,
            width: 4.0,
            target: RoomId(1),
            kind: teleport::GapKind::Forward,
            threshold: test_threshold(RoomId(0), RoomId(1)),
            floor_y: 0.0,
        };

        let view = doorway_preview_view(&gap, 1.6, 2.0, -0.1);

        assert!((view.transform.translation.z - 6.0).abs() < 0.001);
        assert!(
            forward(view.transform).dot(Vec3::new(0.0, -0.1_f32.sin(), 1.0).normalize()) > 0.98
        );
    }

    #[test]
    fn alignment_transform_places_child_origin_at_alignment_offset() {
        let align = teleport::Align2d {
            yaw: 0.75,
            offset: Vec2::new(3.0, -2.0),
        };

        let transform = alignment_transform(align);

        assert!((transform.translation - Vec3::new(3.0, 0.0, -2.0)).length() < 0.001);
        assert!(
            (forward(transform) - (Quat::from_rotation_y(0.75) * Vec3::NEG_Z)).length() < 0.001
        );
    }
}
