//! The Rogue's sensors, where they hang in the facility.
//!
//! The Ascent rules own every sensor: where the Rogue installed it, whether its floor's
//! power lets it watch, and when a loyal Observer takes it down (`ascent::sim::sensor`).
//! This is where one hangs in the world, so a body can see it and walk up to it: over the
//! floor at its cell's middle, at [`SENSOR_HANG`] above whatever a body would stand on
//! there. The rules hand the whole set over each tick ([`HexWfcMatch::set_sensors`]). A
//! sensor has no collider: it blocks nothing, it only watches.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::PlayerId;
use observed_hex::{FLOOR_SLAB_TOP, HexCoord, TILE_LEVEL_HEIGHT, hex_origin};

use super::HexWfcMatch;

/// How far above the floor under it a sensor hangs, metres: over a body's head, within an
/// arm's reach of it.
pub const SENSOR_HANG: f32 = 2.6;
/// How far from a sensor, across the floor, a body takes it down.
pub const SENSOR_REACH: f32 = 3.0;
/// How far below or above a sensor a body's centre may be and still reach it.
const SENSOR_REACH_HEIGHT: f32 = 3.5;

/// The sensors hanging in a match, and the geometry generation their mounts were found in.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct HexSensors {
    mounts: BTreeMap<HexCoord, Vec3>,
    generation: u32,
}

impl HexWfcMatch {
    /// Every sensor, and where it hangs.
    pub fn sensors(&self) -> impl Iterator<Item = (HexCoord, Vec3)> + '_ {
        self.sensors.mounts.iter().map(|(&cell, &at)| (cell, at))
    }

    /// Hang exactly the sensors on `cells`. A mount is found once, against the colliders
    /// of the moment, and again whenever the facility is rebuilt.
    pub fn set_sensors(&mut self, cells: impl IntoIterator<Item = HexCoord>) {
        let generation = self.geometry.generation;
        let rebuilt = generation != self.sensors.generation;
        let wanted: Vec<HexCoord> = cells.into_iter().collect();
        self.sensors.mounts.retain(|cell, _| wanted.contains(cell));
        for cell in wanted {
            if rebuilt || !self.sensors.mounts.contains_key(&cell) {
                let at = self.sensor_mount(cell);
                self.sensors.mounts.insert(cell, at);
            }
        }
        self.sensors.generation = generation;
    }

    /// Where a sensor on `cell` hangs: over where a body would stand at its middle, or over
    /// whatever surface is under the middle - a ramp's - where no body fits.
    fn sensor_mount(&self, cell: HexCoord) -> Vec3 {
        let middle = Vec3::from_array(hex_origin(cell));
        let floor = self.standing_point(cell).unwrap_or_else(|| {
            let from = middle + Vec3::Y * (TILE_LEVEL_HEIGHT - 1.0);
            self.physics
                .ray_distance(from, Vec3::NEG_Y, TILE_LEVEL_HEIGHT)
                .map_or(middle + Vec3::Y * FLOOR_SLAB_TOP, |drop| {
                    from - Vec3::Y * drop
                })
        });
        floor + Vec3::Y * SENSOR_HANG
    }

    /// The sensor nearest `player`'s body within [`SENSOR_REACH`], if any.
    #[must_use]
    pub fn sensor_in_reach(&self, player: PlayerId) -> Option<HexCoord> {
        let body = self
            .players
            .get(&player)
            .filter(|body| body.in_facility())?;
        self.sensors
            .mounts
            .iter()
            .filter_map(|(&cell, &at)| {
                let across = (body.position - at).with_y(0.0).length();
                ((body.position.y - at.y).abs() <= SENSOR_REACH_HEIGHT && across <= SENSOR_REACH)
                    .then_some((across, cell))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, cell)| cell)
    }
}

#[cfg(test)]
mod tests {
    use crate::hex_wfc::{HexMatchConfig, HexWfcMatch};

    #[test]
    fn a_sensor_hangs_over_its_cell_and_a_body_under_it_reaches_it() {
        let mut game = HexWfcMatch::new_with_content(
            7,
            HexMatchConfig::default(),
            crate::hex_wfc::compatibility_test_content().clone(),
        )
        .expect("the facility solves");
        let (&player, body) = game.players.iter().next().expect("a body");
        let cell = body.cell;
        game.set_sensors([cell]);
        let (hung, at) = game.sensors().next().expect("one sensor");
        assert_eq!(hung, cell);
        let middle = glam::Vec3::from_array(observed_hex::hex_origin(cell));
        assert!(
            (at - middle).with_y(0.0).length() < 4.5,
            "over its own cell"
        );
        game.players.get_mut(&player).unwrap().position = at - glam::Vec3::Y * 1.6;
        assert_eq!(game.sensor_in_reach(player), Some(cell));
        game.players.get_mut(&player).unwrap().position = at + glam::Vec3::X * 20.0;
        assert_eq!(game.sensor_in_reach(player), None);
        game.set_sensors([]);
        assert_eq!(game.sensors().count(), 0);
    }
}
