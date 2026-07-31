//! Click a hex, read what the solver and the tile compiler actually decided.
//!
//! The schematic answers "what shape is the facility". This answers "why is
//! *this* cell that shape", which is the question that comes next every time.

use bevy::prelude::*;
use observed_facility::hex_wfc::{HexArchetype, HexSpace, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, PortClass, hex_origin};
use observed_traversal::ColliderShape;

use crate::draw::{is_volatile, state_layer};
use crate::{LabState, Layer};

/// How near the cursor has to land, in pixels, to count as picking a cell.
const PICK_RADIUS: f32 = 26.0;

/// The cell nearest the cursor among those currently drawn.
///
/// The view is orthographic and every cell is a known point, so this projects
/// candidates forward to the screen rather than casting a ray backward into
/// meshes — cheaper, and it cannot miss a cell whose geometry happens to be
/// hollow where the cursor is.
#[must_use]
pub fn pick(
    world: &HexWfcWorld,
    layer: Layer,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor: Vec2,
) -> Option<HexCoord> {
    let level = state_layer(layer);
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
        // Ties go to the cell nearer the camera, so clicking a stack picks the
        // one you can actually see.
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

fn face_name(face: HexFace) -> &'static str {
    match face {
        HexFace::East => "E",
        HexFace::SouthEast => "SE",
        HexFace::SouthWest => "SW",
        HexFace::West => "W",
        HexFace::NorthWest => "NW",
        HexFace::NorthEast => "NE",
        HexFace::Up => "up",
        HexFace::Down => "down",
    }
}

fn port_name(class: PortClass) -> &'static str {
    match class {
        PortClass::Sealed => "sealed",
        PortClass::Door => "door",
        PortClass::RampOpen => "ramp",
        PortClass::ShaftOpen => "shaft",
    }
}

fn archetype_name(archetype: HexArchetype) -> &'static str {
    match archetype {
        HexArchetype::Void => "void",
        HexArchetype::Straight => "straight",
        HexArchetype::Corner => "corner",
        HexArchetype::Junction => "junction",
        HexArchetype::Room => "room",
        HexArchetype::RampUp => "ramp up",
        HexArchetype::RampHead => "ramp head",
        HexArchetype::Shaft => "shaft",
        HexArchetype::Expanse => "expanse",
    }
}

fn space_name(space: HexSpace) -> &'static str {
    match space {
        HexSpace::Void => "void",
        HexSpace::Room => "room",
        HexSpace::Hall => "hall",
    }
}

/// The diagnostics panel for the selected cell, or the prompt when none is.
#[must_use]
pub fn diagnostics(state: &LabState) -> String {
    let Some(coord) = state.selected else {
        return "no cell selected   ( click a hex )".to_string();
    };
    let world = &state.world;
    let Some(placement) = world.placements.get(&coord) else {
        return format!("{coord:?} holds no placement");
    };

    let open_faces = HexFace::LATERAL
        .into_iter()
        .filter(|face| placement.is_open(*face))
        .map(face_name)
        .collect::<Vec<_>>();
    let lateral = if open_faces.is_empty() {
        "none".to_string()
    } else {
        open_faces.join(" ")
    };

    let register = world
        .architecture
        .get(&coord)
        .map_or("none", |register| register.slug());

    let room = world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.cells.contains(&coord))
        .map(|blueprint| {
            format!(
                "{} (anchor {},{},L{}; {} cells)",
                blueprint.role.label(),
                blueprint.anchor.q,
                blueprint.anchor.r,
                blueprint.anchor.level,
                blueprint.cells.len()
            )
        });

    // The projector already resolved which authored tile this cell renders as,
    // so read its answer rather than re-running selection here and risking a
    // different one.
    let mut tiles: Vec<String> = Vec::new();
    let mut hulls = 0usize;
    let mut vertices = 0usize;
    if let Some(snapshot) = state.geometry.as_ref() {
        for piece in snapshot
            .pieces
            .iter()
            .filter(|piece| piece.source_cell == coord)
        {
            hulls += 1;
            if let ColliderShape::ConvexHull { points } = &piece.shape {
                vertices += points.len();
            }
            if let Some(tile) = piece.tile.as_ref() {
                let name = format!("{}/{} v{}", tile.archetype, tile.register, tile.variant);
                if !tiles.contains(&name) {
                    tiles.push(name);
                }
            }
        }
    }
    let tile_line = if tiles.is_empty() {
        "none (this cell emits no prefab)".to_string()
    } else {
        tiles.join(", ")
    };

    let revision = world.cell_revisions.get(&coord).copied().unwrap_or(0);
    let stability = if is_volatile(placement.archetype) {
        "CAN REWIRE: the solver may replace this during a relayout"
    } else {
        "will not rewire: structural, survives every relayout"
    };

    format!(
        "SELECTED  q{} r{} L{}\n\
         tile        {tile_line}\n\
         geometry    {hulls} hulls / {vertices} vertices\n\
         archetype   {} ({} space)\n\
         doors       {lateral}      up {}   down {}\n\
         district    {register}\n\
         room        {}\n\
         revision    {revision}\n\
         {stability}",
        coord.q,
        coord.r,
        coord.level,
        archetype_name(placement.archetype),
        space_name(placement.space),
        port_name(placement.up),
        port_name(placement.down),
        room.unwrap_or_else(|| "none".to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_selected_prompts_rather_than_showing_an_empty_panel() {
        let mut state = LabState::new(0);
        state.selected = None;
        assert!(diagnostics(&state).contains("click a hex"));
    }

    #[test]
    fn a_selected_cell_reports_its_solver_and_tile_decisions() {
        let mut state = LabState::new(0);
        let coord = *state
            .world
            .placements
            .iter()
            .find(|(_, placement)| placement.archetype != HexArchetype::Void)
            .expect("a solved facility places something")
            .0;
        state.selected = Some(coord);
        let text = diagnostics(&state);
        for field in [
            "SELECTED",
            "tile",
            "geometry",
            "archetype",
            "doors",
            "district",
            "room",
            "revision",
        ] {
            assert!(text.contains(field), "the panel must report {field}");
        }
        assert!(
            text.contains("rewire"),
            "the panel must say whether the solver can move this cell"
        );
    }

    #[test]
    fn a_shaft_reads_as_structural_and_a_corridor_does_not() {
        let mut state = LabState::new(0);
        let shaft = state
            .world
            .placements
            .iter()
            .find(|(_, placement)| placement.archetype == HexArchetype::Shaft)
            .map(|(coord, _)| *coord);
        if let Some(coord) = shaft {
            state.selected = Some(coord);
            assert!(diagnostics(&state).contains("will not rewire"));
        }
        let corridor = state
            .world
            .placements
            .iter()
            .find(|(_, placement)| placement.archetype == HexArchetype::Straight)
            .map(|(coord, _)| *coord);
        if let Some(coord) = corridor {
            state.selected = Some(coord);
            assert!(diagnostics(&state).contains("CAN REWIRE"));
        }
    }

    #[test]
    fn every_lateral_face_and_port_class_has_a_name() {
        for face in HexFace::ALL {
            assert!(!face_name(face).is_empty());
        }
        for class in [
            PortClass::Sealed,
            PortClass::Door,
            PortClass::RampOpen,
            PortClass::ShaftOpen,
        ] {
            assert!(!port_name(class).is_empty());
        }
    }
}
