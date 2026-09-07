//! The simulation. Deliberately Bevy-free: no entities, no colours, no camera,
//! so the whole turn pipeline is testable headlessly and a match is
//! reproducible from a `ModeSpec`, a seed and an intent log.

pub mod architect;
pub mod board;
pub mod bot;
pub mod mutation;
pub mod objective;
pub mod prng;
pub mod resolution;
pub mod rival;
pub mod rules;
pub mod setback;
pub mod state;
pub mod step;
pub mod threat;
pub mod tiles;
pub mod vision;

#[cfg(test)]
mod tests;
