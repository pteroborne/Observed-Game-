//! The two ways this lab draws a solved facility.
//!
//! **Schematic** is the default and the point of the lab: line work on a dark
//! screen, hue reserved for one question — will the solver rewire this or not.
//! When a single layer is in view it draws the *real* authored hulls, because at
//! one layer that is affordable and it is the only way to see what the tiles
//! actually compose. Across all layers it falls back to cell shells, since a
//! hundred and twenty thousand hulls is not a diagram.
//!
//! **Solid** is the dev view: the district-coloured massing this lab shipped
//! with, kept because it answers a different question — where the districts are,
//! not what the geometry is.

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexArchetype, HexWfcWorld};
use observed_hex::{HexFace, PortClass, hex_origin};
use observed_style::{MarkerRole, SchematicRole, hex_sketch, marker, schematic};
use observed_traversal::ColliderShape;

use crate::wire::{LineBatch, cell_outline, ramp_glyph, stair_glyph};
use crate::{LabState, LabVisual, Layer, cell_sketch, sketch_role};

/// World width of a schematic line. Constant in world space, so zooming in
/// thickens the stroke the way leaning toward a screen would.
const STROKE: f32 = 0.42;

/// Whether the solver may rewire this cell.
///
/// Mirrors `collapse::placement_is_mutable_topology`, which is private to
/// `observed_facility`: only `Void`, `Straight`, `Corner` and `Junction` are
/// topology-mutable. Everything else — rooms, ramps, shafts — is structural and
/// survives a relayout. This lab has no observer, so unlike the in-game map
/// there is no transient "held" state; the question here is purely what the
/// solver is free to move.
#[must_use]
pub fn is_volatile(archetype: HexArchetype) -> bool {
    matches!(
        archetype,
        HexArchetype::Void | HexArchetype::Straight | HexArchetype::Corner | HexArchetype::Junction
    )
}

/// What one schematic rebuild drew, for the status line and the tests.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawReport {
    pub cells: usize,
    pub segments: usize,
    pub tiles_wired: usize,
    pub real_geometry: bool,
}

fn line_material(
    materials: &mut Assets<StandardMaterial>,
    role: SchematicRole,
) -> Handle<StandardMaterial> {
    let treatment = schematic(role);
    materials.add(StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        // A phosphor line is its own light source; shading it would only muddy
        // the read.
        unlit: true,
        // A ribbon is a flat quad standing in for a line, so it has no
        // meaningful back face. Culling one would make its visibility depend on
        // which way the segment happened to run.
        cull_mode: None,
        double_sided: true,
        ..default()
    })
}

/// Draw the schematic. Returns what it drew.
pub fn schematic_view(
    commands: &mut Commands,
    state: &mut LabState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    view: Vec3,
) -> DrawReport {
    let mut pinned = LineBatch::default();
    let mut volatile = LineBatch::default();
    let mut selected = LineBatch::default();
    let mut report = DrawReport {
        real_geometry: state.detail && state.geometry.is_some(),
        ..DrawReport::default()
    };

    // Read the Copy view state before splitting the borrow, so the edge cache
    // can stay mutable while the world is read.
    let layer = state_layer(state.layer);
    let selected_cell = state.selected;
    let LabState {
        world,
        geometry,
        edges,
        ..
    } = state;

    for (coord, placement) in &world.placements {
        if placement.archetype == HexArchetype::Void {
            continue;
        }
        if let Some(level) = layer
            && coord.level != level
        {
            continue;
        }
        report.cells += 1;
        let origin = Vec3::from_array(hex_origin(*coord));
        let is_selected = selected_cell == Some(*coord);
        let batch = if is_selected {
            &mut selected
        } else if is_volatile(placement.archetype) {
            &mut volatile
        } else {
            &mut pinned
        };

        // Real hulls only when asked for, or for the selected cell — that is
        // the "what is actually in there" the inspector exists to answer. The
        // default is the symbolic outline, which is both far cheaper and far
        // more readable: construction edges are not information.
        let drew_real = if report.real_geometry || is_selected {
            match geometry.as_ref() {
                Some(snapshot) => {
                    let mut any = false;
                    for piece in snapshot
                        .pieces
                        .iter()
                        .filter(|piece| piece.source_cell == *coord)
                    {
                        let ColliderShape::ConvexHull { points } = &piece.shape else {
                            continue;
                        };
                        let key = piece.tile.as_ref().map_or_else(
                            || "untiled".to_string(),
                            |tile| format!("{}/{}/{}", tile.archetype, tile.register, tile.variant),
                        );
                        for (a, b) in edges.edges_for(&key, std::slice::from_ref(points)) {
                            batch.segment(origin + *a, origin + *b);
                        }
                        any = true;
                    }
                    any
                }
                None => false,
            }
        } else {
            false
        };

        if !drew_real {
            let sketch = cell_sketch(world, *coord).unwrap_or_else(|| {
                hex_sketch(sketch_role(placement.archetype, placement.space, false))
            });
            let height = sketch.height.unwrap_or(0.9);
            let mut open = [false; 6];
            for face in HexFace::LATERAL {
                open[face.index()] = placement.is_open(face);
            }
            for (a, b) in cell_outline(height, sketch.inset, open) {
                batch.segment(origin + a, origin + b);
            }
            // A vertical connection gets a symbol, so a floor change is legible
            // without selecting the cell.
            let glyph = match placement.archetype {
                HexArchetype::Shaft if placement.up != PortClass::Sealed => {
                    Some(stair_glyph(height))
                }
                HexArchetype::RampUp => Some(ramp_glyph(height)),
                _ => None,
            };
            if let Some(glyph) = glyph {
                for (a, b) in glyph {
                    batch.segment(origin + a, origin + b);
                }
            }
        }
    }
    report.tiles_wired = edges.len();

    for (batch, role) in [
        (pinned, SchematicRole::Pinned),
        (volatile, SchematicRole::Volatile),
        (selected, SchematicRole::Selected),
    ] {
        report.segments += batch.segments();
        let Some(mesh) = batch.build_ribbons(STROKE, view) else {
            continue;
        };
        let material = line_material(materials, role);
        commands.spawn((
            LabVisual,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            Name::new(role.label()),
        ));
    }
    report
}

/// The dev view: district-coloured solid massing, as this lab shipped.
pub fn solid_view(
    commands: &mut Commands,
    state: &LabState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> DrawReport {
    let mut report = DrawReport::default();
    let mut mesh_cache: std::collections::BTreeMap<(u32, u32), Handle<Mesh>> =
        std::collections::BTreeMap::new();
    let mut material_cache: std::collections::BTreeMap<u8, Handle<StandardMaterial>> =
        std::collections::BTreeMap::new();
    let layer = state_layer(state.layer);

    for coord in state.world.placements.keys() {
        if let Some(level) = layer
            && coord.level != level
        {
            continue;
        }
        let Some(sketch) = cell_sketch(&state.world, *coord) else {
            continue;
        };
        let Some(height) = sketch.height else {
            continue;
        };
        report.cells += 1;
        let register = state
            .world
            .architecture
            .get(coord)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        let mesh = mesh_cache
            .entry((height.to_bits(), sketch.inset.to_bits()))
            .or_insert_with(|| meshes.add(crate::prism::hex_prism(height, sketch.inset)))
            .clone();
        let material = material_cache
            .entry(register as u8)
            .or_insert_with(|| {
                let accent = observed_style::architecture(register).accent;
                materials.add(StandardMaterial {
                    base_color: Color::LinearRgba(accent),
                    emissive: accent * 0.22,
                    perceptual_roughness: 0.92,
                    ..default()
                })
            })
            .clone();
        commands.spawn((
            LabVisual,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(Vec3::from_array(hex_origin(*coord))),
            Name::new("solid cell"),
        ));
    }

    // The two fixed endpoints stay signal-tier in either view.
    for (coord, role) in [
        (state.world.config.spawn(), MarkerRole::You),
        (state.world.config.exit(), MarkerRole::Exit),
    ] {
        if let Some(level) = layer
            && coord.level != level
        {
            continue;
        }
        let treatment = marker(role);
        commands.spawn((
            LabVisual,
            Mesh3d(meshes.add(crate::prism::hex_prism(0.5, 0.55))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: treatment.base_color,
                emissive: treatment.emissive,
                perceptual_roughness: 0.4,
                ..default()
            })),
            Transform::from_translation(Vec3::from_array(hex_origin(coord)) + Vec3::Y * 9.0),
            Name::new("endpoint marker"),
        ));
    }
    report
}

/// `None` means every level.
#[must_use]
pub fn state_layer(layer: Layer) -> Option<u8> {
    match layer {
        Layer::Single(level) => Some(level),
        Layer::All => None,
    }
}

/// Bounds of the drawn cells, so the camera frames what is actually on screen
/// rather than the whole lattice.
#[must_use]
pub fn drawn_bounds(world: &HexWfcWorld, layer: Layer) -> Option<(Vec3, Vec3)> {
    let level = state_layer(layer);
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for (coord, placement) in &world.placements {
        if placement.archetype == HexArchetype::Void {
            continue;
        }
        if let Some(level) = level
            && coord.level != level
        {
            continue;
        }
        let height = cell_sketch(world, *coord)
            .and_then(|sketch| sketch.height)
            .unwrap_or(0.9);
        let origin = Vec3::from_array(hex_origin(*coord));
        let low = origin - Vec3::new(8.0, 0.0, 8.0);
        let high = origin + Vec3::new(8.0, height, 8.0);
        bounds = Some(match bounds {
            None => (low, high),
            Some((min, max)) => (min.min(low), max.max(high)),
        });
    }
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_topology_mutable_archetypes_read_as_volatile() {
        for archetype in [
            HexArchetype::Straight,
            HexArchetype::Corner,
            HexArchetype::Junction,
            HexArchetype::Void,
        ] {
            assert!(is_volatile(archetype), "{archetype:?} can be rewired");
        }
        for archetype in [
            HexArchetype::Room,
            HexArchetype::RampUp,
            HexArchetype::RampHead,
            HexArchetype::Shaft,
        ] {
            assert!(!is_volatile(archetype), "{archetype:?} is structural");
        }
    }

    #[test]
    fn a_single_layer_frames_tighter_than_the_whole_stack() {
        let state = LabState::new(0);
        let all = drawn_bounds(&state.world, Layer::All).expect("the stack draws");
        let one = drawn_bounds(&state.world, Layer::Single(0)).expect("level 0 draws");
        assert!(
            one.1.y - one.0.y < all.1.y - all.0.y,
            "one layer is shorter than ten"
        );
    }

    #[test]
    fn the_layer_cycle_maps_onto_a_level_filter() {
        assert_eq!(state_layer(Layer::Single(3)), Some(3));
        assert_eq!(state_layer(Layer::All), None);
    }
}
