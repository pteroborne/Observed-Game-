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

// --- Liminal scale dials (Phase 46b, Arc D stage D4) ---------------------------------
//
// The WFC-generated ~30-room default map (Phase 46a) is still dev-scaled: rooms and
// halls read as functional dev geometry, not the liminal, humanoid-scale labyrinth the
// arc calls for (see `docs/liminal_arc_plan.md` D4 and the Phase 41b comfort-pass
// precedent). These constants are the single tuning surface for that scale pass —
// change a value here, not at each call site, to re-tune the facility's proportions.
//
// Scaling a template's *dimensions* does not reshuffle the edge -> variation hash (only
// the template *count* does), so these multipliers are safe to retune without touching
// `hallway::variation_for`'s determinism.

/// Footprint scale for an ordinary room (`RoomRole` anything other than the hub/monitor
/// cases below). Multiplies the seeded apothem/rectangle half-extent in
/// `teleport::geom::room_polygon`, so rooms still vary seed-to-seed but breathe at
/// liminal scale rather than dev scale.
pub(crate) const ROOM_SCALE_STANDARD: f32 = 1.6;

/// Footprint scale for hub-style rooms (`RoomRole::Start`, `RoomRole::Exit`,
/// `RoomRole::Decision`): these are the rooms teams linger in to read doors, call
/// routes, and make decisions, so they get the most generous volume.
pub(crate) const ROOM_SCALE_HUB: f32 = 2.0;

/// Footprint scale for `RoomRole::Monitor` rooms. Smaller than the hub scale because an
/// observation room's nine-panel bank needs long, unbroken wall runs more than raw
/// floor area — `OBSERVATION_ROOM_SIDES`/`OBSERVATION_ROOM_SCALE` in `teleport::geom`
/// already shape it into a many-sided room for those wall runs; this dial scales that
/// shape up to the liminal pass without fighting the panel layout.
pub(crate) const ROOM_SCALE_MONITOR: f32 = 1.8;

/// Length multiplier applied to authored hallway templates in `hallway.rs`. Excludes the
/// Gantry template (dimensions come from `observed_traversal::gantry`'s pure course
/// constants, which stay authored/untouched) and the grid-driven maze/labyrinth
/// templates (their footprint is cell-derived off `grid`; stretching the authored
/// `length`/`width` fields for those would just relabel the same cell-driven geometry,
/// not actually lengthen it, so they are left at their current footprint).
pub(crate) const HALL_STRETCH: f32 = 1.7;

/// Width multiplier applied alongside `HALL_STRETCH` to the same non-grid, non-Gantry
/// hallway templates. Modest — hallway width mostly reads through `HALL_WIDTH` (the
/// doorway span, deliberately unchanged) plus the interior baffles/pillars, which
/// already keep their own proportions relative to the template width.
pub(crate) const HALL_WIDEN: f32 = 1.15;
