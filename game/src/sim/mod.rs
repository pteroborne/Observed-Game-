//! Simulation-side modules: Bevy resources that hold the running match's state.
//! No rendering, UI, or asset types — presentation (`crate::view`, the screen systems)
//! reads these; only input/controller/match-pump systems write them.

pub(crate) mod director;
pub(crate) mod nav;
pub(crate) mod state;
