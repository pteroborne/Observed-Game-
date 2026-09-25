//! Two screens onto one match.

pub mod animate;
pub mod art;
pub mod board;
pub mod hud;
pub mod input;

use bevy::prelude::*;
use observed_hex::coords::HexCoord;
use observed_mechanics::state::MatchState;

pub const HEX_RADIUS: f32 = 1.0;
const SQRT3: f32 = 1.732_050_8;

#[must_use]
pub fn world_of(state: &MatchState, coord: HexCoord) -> Vec2 {
    let centre = state.board.centre();
    let dq = f32::from(coord.q) - f32::from(centre.q);
    let dr = f32::from(coord.r) - f32::from(centre.r);
    Vec2::new(HEX_RADIUS * SQRT3 * (dq + dr * 0.5), -HEX_RADIUS * 1.5 * dr)
}

#[must_use]
pub fn cell_at(state: &MatchState, point: Vec2) -> Option<HexCoord> {
    state
        .board
        .cells()
        .map(|cell| (cell, world_of(state, cell).distance_squared(point)))
        .filter(|&(_, d)| d <= HEX_RADIUS * HEX_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(cell, _)| cell)
}
