//! Game-owned spatial layout constants for the teleport place model.
//!
//! These values began life as `observed_match::maze`'s tile grid (the abandoned
//! walkable-maze arena), but the teleport game owns its world scale — the maze module
//! no longer dictates it. The one remaining legitimate `observed_match::maze` import
//! is `capture/tour.rs`, which interprets the *brain's* room coordinates in the
//! brain's own grid space.

/// World size (metres) of one place tile — the unit the place renderer's shared
/// floor/wall/ceiling meshes are built at.
pub(crate) const PLACE_TILE: f32 = 2.2;

/// Every hallway is carved three tiles wide, and doorway frames span the full hall
/// width so doorways and hallways line up by design.
pub(crate) const HALL_WIDTH: f32 = 3.0 * PLACE_TILE;

/// Taller than head height so the facility reads as a vast, breathable structure rather
/// than a tight corridor. Everything (frames, leaves, walls, ceiling) scales off this.
pub(crate) const WALL_HEIGHT: f32 = 4.6;
