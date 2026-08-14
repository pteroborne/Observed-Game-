//! Screen-space picking for hex cell selection and diagnostics reporting.

use bevy::prelude::*;
use observed_facility::hex_wfc::{HexArchetype, HexWfcWorld};
use observed_hex::{HexCoord, hex_origin};

use crate::{Layer, StudioState};

/// How near the cursor has to land, in pixels, to count as picking a cell.
const PICK_RADIUS: f32 = 26.0;

/// Filter layer helper: `None` for `All`, `Some(level)` for `Single(level)`.
fn state_level(layer: Layer) -> Option<u8> {
    match layer {
        Layer::Single(level) => Some(level),
        Layer::All => None,
    }
}

/// The cell nearest the cursor among those currently drawn.
#[must_use]
pub fn pick(
    world: &HexWfcWorld,
    layer: Layer,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor: Vec2,
) -> Option<HexCoord> {
    let level = state_level(layer);
    let mut best: Option<(f32, HexCoord)> = None;
    for (coord, placement) in &world.placements {
        if placement.archetype == HexArchetype::Void {
            continue;
        }
        if let Some(level) = level
            && coord.level != level
        {
            continue;
        }
        let centre = Vec3::from_array(hex_origin(*coord)) + Vec3::Y * 0.5;
        let Ok(screen) = camera.world_to_viewport(camera_transform, centre) else {
            continue;
        };
        let distance = screen.distance(cursor);
        if distance > PICK_RADIUS {
            continue;
        }
        let depth = -camera_transform
            .affine()
            .inverse()
            .transform_point3(centre)
            .z;
        let score = distance + depth * 0.001;
        if best.is_none_or(|(current, _)| score < current) {
            best = Some((score, *coord));
        }
    }
    best.map(|(_, coord)| coord)
}

/// Diagnostic report for selected hex cell.
pub fn diagnostics(state: &StudioState) -> String {
    let Some(selected) = state.selected else {
        return "No cell selected. Click a hex in viewport to inspect.".to_string();
    };
    let Some(solved) = state.solved.as_ref() else {
        return "No solve available.".to_string();
    };
    let Some(placement) = solved.world.placements.get(&selected) else {
        return format!("Selected cell {selected:?} not in placements.");
    };

    let district = solved
        .world
        .architecture
        .get(&selected)
        .map_or("none", |r| r.slug());

    // What the solver went through to place this, rather than only what it
    // placed. `N` answers "what could stand here" properly, by re-opening the
    // ring; this is the cheap half of that question, and it says so.
    let traced = crate::timeline::describe_cell(state, selected)
        .map_or_else(String::new, |described| format!("\nSolver:    {described}"));

    format!(
        "SELECTED CELL: q={} r={} L={}\n\
         Archetype: {:?}\n\
         Space:     {:?}\n\
         District:  {}\n\
         Up Port:   {:?}\n\
         Down Port: {:?}{}",
        selected.q,
        selected.r,
        selected.level,
        placement.archetype,
        placement.space,
        district,
        placement.up,
        placement.down,
        traced,
    )
}
