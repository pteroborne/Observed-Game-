//! Read-only targeting feedback. Selection never authorizes an action.
use glam::Vec3;
use observed_core::PlayerId;

use super::HexWfcMatch;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimCandidate {
    pub distance: f32,
    pub in_reach: bool,
}

impl HexWfcMatch {
    /// A visible, aimed interaction point. A small target radius also permits
    /// aiming at the visible fixture rather than its exact mathematical centre.
    pub fn aim_candidate(
        &self,
        player: PlayerId,
        point: Vec3,
        reach: f32,
        radius: f32,
    ) -> Option<AimCandidate> {
        let (eye, look) = self.eye_and_look(player)?;
        let offset = point - eye;
        let distance = offset.length();
        let along = offset.dot(look);
        if along <= 0.0 || distance > 30.0 || (offset - look * along).length() > radius {
            return None;
        }
        let ray = offset.normalize_or_zero();
        if self
            .physics
            .ray_distance(eye, ray, (distance - radius).max(0.0))
            .is_some()
        {
            return None;
        }
        let body = self.players.get(&player)?;
        Some(AimCandidate {
            distance,
            in_reach: (body.position - point).with_y(0.0).length() <= reach
                && (body.position.y - point.y).abs() < 2.2,
        })
    }
}
