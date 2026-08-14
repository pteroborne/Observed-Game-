//! Schematic line work for the studio viewport.
//!
//! The question this view answers is "what did my edit just do to the
//! facility", so it is drawn the way the survivor map and `iso_observer_lab`
//! draw it: cells meet edge to edge at their true quantized-hexagon corners,
//! composition is carried by which faces are walled, and colour comes from
//! [`observed_style`] rather than from anything invented here.
//!
//! Everything batches into one mesh per colour. At production scale a single
//! layer is roughly twelve thousand cells; as entities that is hopeless, and as
//! three line meshes it is nothing.
//!
//! # The legend (Legibility Contract — every state means something)
//!
//! Two modes, because one colour cannot answer two questions at once. The
//! status bar always says which is active.
//!
//! **Pin mode** (default) — *what have I fixed?*
//! - **dim green outline** — the cell floor plan, i.e. the lattice itself.
//! - **red walls** — the solver is free to rewire this cell.
//! - **green walls** — an authored pin holds this cell; the solver may not
//!   change it. Painting turns red into green, which is the whole feedback
//!   loop of the tool.
//!
//! **Compare mode** (`B`) — *what did my last edit change?*
//! - **green walls** — identical to the same seed solved at the baseline.
//! - **red walls** — this cell moved.
//!
//! In both modes an **amber ring** is the selected cell.
//!
//! The roles come from [`observed_style::SchematicRole`], whose documented
//! meanings are already exactly "the solver will not rewire this" (`Pinned`)
//! and "it can" (`Volatile`). Nothing here invents a palette.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_facility::hex_wfc::{HexArchetype, HexPlacement, HexSpace, HexWfcWorld};
use observed_hex::{CORNERS, HexCoord, HexFace, PortClass, hex_origin};
use observed_style::{HexSketch, HexSketchRole, SchematicRole, Treatment, hex_sketch, schematic};

use crate::{Layer, StudioState};

/// Walls draw at a fraction of the cell's height. A plan is read from above;
/// full-height cages stack into an unreadable thicket, while a low band says
/// "you cannot pass here" without occluding the cell behind it.
const WALL_FRACTION: f32 = 1.0 / 3.0;

/// Cells meet edge to edge. Composition is carried by which faces are walled —
/// a room's internal faces are open, so a room emerges as one enclosed area —
/// rather than by shrinking a hex away from its neighbours.
const CELL_EXTENT: f32 = 1.0;

/// Re-exported: the lighting of an isometric cutaway is shared with the
/// game's spectator overview, which looks at the same building the same way.
pub use observed_style::iso::light::KEY_OFFSET as KEY_LIGHT_OFFSET;

pub use observed_style::iso::light::KEY_ILLUMINANCE;

pub use observed_style::iso::light::AMBIENT_BRIGHTNESS;

/// Component tag for the geometry pass: schematic lines and detail hulls.
#[derive(Component)]
pub struct StudioVisual;

/// Component tag for the overlay pass: selection and hover rings only.
///
/// Separate from [`StudioVisual`] so hover can be redrawn without touching
/// geometry. Hover fires on every mouse move to a new cell; re-triangulating a
/// facility for that is affordable at a hundred cells and ruinous at five
/// thousand.
#[derive(Component)]
pub struct StudioOverlay;

/// Redraw just the selection and hover rings.
pub fn rebuild_overlay(
    mut commands: Commands,
    mut state: ResMut<StudioState>,
    existing: Query<Entity, With<StudioOverlay>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !state.overlay_dirty {
        return;
    }
    state.overlay_dirty = false;

    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let Some(solved) = state.solved.as_ref() else {
        return;
    };

    let lift = if state.detail_mode == crate::detail::DetailMode::Off {
        0.0
    } else {
        0.6
    };
    let mut rings = LineBatch::default();
    for coord in [state.selected, state.hovered].into_iter().flatten() {
        if !solved.world.placements.contains_key(&coord) {
            continue;
        }
        let origin = Vec3::from_array(hex_origin(coord)) + Vec3::Y * lift;
        // Hover rides higher and tighter than selection, so when they land on
        // the same cell you can still tell which is which.
        let (inset, height) = if state.selected == Some(coord) {
            (CELL_EXTENT, 0.2)
        } else {
            (CELL_EXTENT * 0.86, 0.45)
        };
        for (a, b) in floor_ring(inset) {
            rings.segment(origin + a + Vec3::Y * height, origin + b + Vec3::Y * height);
        }
    }

    if let Some(mesh) = rings.into_mesh() {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(unlit(schematic(SchematicRole::Selected)))),
            StudioOverlay,
        ));
    }
}

/// What the last rebuild actually emitted. Surfaced in the status line so an
/// empty viewport is always distinguishable from a broken one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawReport {
    pub cells: usize,
    pub changed: usize,
    pub pinned: usize,
    pub meshes: usize,
}

/// One colour's worth of accumulated line work.
///
/// `pub(crate)` because the neighbourhood explorer accumulates its wireframe
/// the same way: one mesh per colour, however many hulls it outlines.
#[derive(Default)]
pub(crate) struct LineBatch {
    points: Vec<[f32; 3]>,
}

impl LineBatch {
    pub(crate) fn segment(&mut self, from: Vec3, to: Vec3) {
        self.points.push(from.to_array());
        self.points.push(to.to_array());
    }

    fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub(crate) fn into_mesh(self) -> Option<Mesh> {
        if self.points.is_empty() {
            return None;
        }
        let count = self.points.len() as u32;
        let mut mesh = Mesh::new(
            PrimitiveTopology::LineList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.points);
        mesh.insert_indices(Indices::U32((0..count).collect()));
        Some(mesh)
    }
}

/// The cell-local corners at `inset`, in [`CORNERS`] order.
///
/// [`CORNERS`] is the **quantized** hexagon — 14 m across flats, 16 m across
/// corners, deliberately 0.8% irregular so every corner lands on an integer
/// TrenchBroom coordinate. Drawing a regular hexagon here instead would put the
/// outline in the wrong place at every seam.
fn ring(y: f32, inset: f32) -> [Vec3; 6] {
    let mut points = [Vec3::ZERO; 6];
    for (slot, &(x, z)) in points.iter_mut().zip(CORNERS.iter()) {
        #[allow(clippy::cast_precision_loss)]
        {
            *slot = Vec3::new(x as f32 * inset, y, z as f32 * inset);
        }
    }
    points
}

/// The floor outline of a cell at full extent, so neighbouring cells meet and
/// the lattice reads as one connected surface rather than as scattered tiles.
fn floor_ring(inset: f32) -> [(Vec3, Vec3); 6] {
    let corners = ring(0.0, inset);
    std::array::from_fn(|index| (corners[index], corners[(index + 1) % 6]))
}

/// A vertical band on every face the cell does **not** open through.
fn wall_edges(height: f32, open: [bool; 6]) -> Vec<(Vec3, Vec3)> {
    let base = ring(0.0, CELL_EXTENT);
    let top = ring(height, CELL_EXTENT);
    let mut edges = Vec::new();
    for face in 0..6 {
        if open[face] {
            continue;
        }
        let next = (face + 1) % 6;
        edges.push((top[face], top[next]));
        edges.push((base[face], top[face]));
        edges.push((base[next], top[next]));
    }
    edges
}

/// A climb symbol, so a floor change is legible without selecting the cell.
fn vertical_glyph(height: f32) -> [(Vec3, Vec3); 2] {
    let top = Vec3::new(0.0, height * 1.6, 0.0);
    [
        (Vec3::new(-2.0, 0.0, 0.0), top),
        (Vec3::new(2.0, 0.0, 0.0), top),
    ]
}

/// Which sketch role a placement draws as. Mirrors `iso_observer_lab` so the
/// studio and the shipped survivor map cannot drift apart.
#[must_use]
pub fn sketch_role(archetype: HexArchetype, space: HexSpace, in_blueprint: bool) -> HexSketchRole {
    if in_blueprint || space == HexSpace::Room {
        return HexSketchRole::Room;
    }
    match archetype {
        HexArchetype::Void => HexSketchRole::Void,
        HexArchetype::Straight | HexArchetype::Corner => HexSketchRole::Corridor,
        HexArchetype::Junction => HexSketchRole::Junction,
        HexArchetype::RampUp => HexSketchRole::Ramp,
        HexArchetype::RampHead => HexSketchRole::RampHead,
        HexArchetype::Shaft => HexSketchRole::Shaft,
        HexArchetype::Expanse => HexSketchRole::Expanse,
        HexArchetype::Room => HexSketchRole::Room,
    }
}

#[must_use]
fn cell_sketch(world: &HexWfcWorld, coord: HexCoord, placement: &HexPlacement) -> HexSketch {
    hex_sketch(sketch_role(
        placement.archetype,
        placement.space,
        world.room_id_at(coord).is_some(),
    ))
}

/// Bounding box of the cells currently drawn, in world coordinates.
#[must_use]
pub fn drawn_bounds(world: &HexWfcWorld, layer: Layer) -> Option<(Vec3, Vec3)> {
    let level = layer.level();
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    for (coord, placement) in &world.placements {
        if placement.archetype == HexArchetype::Void {
            continue;
        }
        if level.is_some_and(|level| coord.level != level) {
            continue;
        }
        let origin = Vec3::from_array(hex_origin(*coord));
        // Pad by the real corner extent, or the outermost cells clip.
        min = min.min(origin - Vec3::new(8.0, 0.0, 8.0));
        max = max.max(origin + Vec3::new(8.0, 8.0, 8.0));
        found = true;
    }
    found.then_some((min, max))
}

pub(crate) fn unlit(treatment: Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        unlit: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    }
}

/// Rebuild the schematic. Despawns every [`StudioVisual`] and re-emits at most
/// four meshes, regardless of how large the facility is.
pub fn rebuild_visuals(
    mut commands: Commands,
    mut state: ResMut<StudioState>,
    mut cache: ResMut<crate::detail::TileMeshCache>,
    existing: Query<Entity, With<StudioVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !state.geometry_dirty {
        return;
    }
    state.geometry_dirty = false;

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    // Reframe before borrowing `solved` for the draw loop.
    let framed = state
        .solved
        .as_ref()
        .and_then(|solved| drawn_bounds(&solved.world, state.layer));
    if let Some((min, max)) = framed {
        state.base_frame = crate::viewport::frame_camera_at(min, max, state.detent);
    }

    // Real authored geometry first, so the schematic overlays it. That order is
    // the point: the schematic's walls are the *solver's* truth and these hulls
    // are the *authored* truth, and a disagreement between them is a real bug
    // worth seeing rather than hiding behind whichever drew last.
    let detail_report = crate::detail::emit_detail(
        &mut commands,
        &state,
        &mut meshes,
        &mut materials,
        &mut cache,
    );
    state.detail_report = detail_report;
    let neighbor_report =
        crate::neighbors::emit(&mut commands, &state, &mut meshes, &mut materials);
    state.neighbors.report = neighbor_report;
    let pinned = crate::brush::pinned_coords(&state.profile);
    // What the solver knew at the replay cursor, or `None` when the replay is
    // settled and the finished world is what to draw.
    let replay = crate::timeline::replay_cells(&state);
    let Some(solved) = state.solved.as_ref() else {
        state.report = DrawReport::default();
        return;
    };
    // The compare overlay asks "what did my tuning move", against a finished
    // baseline world. Mid-replay that question has no answer, and its red would
    // sit in the same viewport as the replay's red meaning something else — the
    // Legibility Contract's exact failure case. The replay owns the colour while
    // it is running, exactly as the neighbourhood explorer does.
    let compare = state.show_baseline_compare && replay.is_none();
    let baseline = state.baseline_world.as_ref();

    // The authored deck is the view; these lines are the *solver's* topology,
    // which is a different question. It is asked with `G`, and answered
    // unasked in three cases where the view would otherwise be wrong:
    //
    // - no projected catalog, so there is no solid at all and a black viewport
    //   would read as a crash;
    // - mid-replay, where an unobserved cell has no authored geometry and
    //   without the lattice the facility grows out of nothing;
    // - the neighbourhood explorer, whose subject is one ring *in a lattice*.
    let solid_drawing = solved.geometry.is_some()
        && state.detail_mode != crate::detail::DetailMode::Off
        && state.detail_report.hulls_drawn > 0;
    let forced = if solved.geometry.is_none() {
        Some("no authored geometry: showing the solver's schematic")
    } else if replay.is_some() {
        Some("replaying: the lattice is drawn for cells not yet observed")
    } else if state.detail_mode == crate::detail::DetailMode::Neighborhood {
        Some("neighbourhood: the lattice is drawn around the ring")
    } else {
        None
    };
    let overlay_on = state.show_schematic
        || forced.is_some()
        || state.detail_mode == crate::detail::DetailMode::Off;
    let forced_reason = forced.map(String::from);
    // With the solid drawing, pins and compare are already said in its
    // treatments, so the line work drops to one neutral colour and stops
    // answering a second question in the same green and red. Without it, the
    // overlay is the only surface there is and carries them as it always did.
    let overlay_carries_signals = !solid_drawing;

    // Just clear of `FLOOR_SLAB_TOP` (0.5 m) so the outline reads on top of a
    // solid floor rather than z-fighting inside it.
    let schematic_lift = if solid_drawing { 0.6 } else { 0.0 };

    let mut grid = LineBatch::default();
    // `walls` is the green batch, `changed_walls` the red one. Which question
    // they answer depends on the mode; see the module legend.
    let mut walls = LineBatch::default();
    let mut changed_walls = LineBatch::default();
    let mut report = DrawReport::default();

    for (coord, placement) in &solved.world.placements {
        if placement.archetype == HexArchetype::Void {
            continue;
        }
        if !state.layer.draws(coord.level) {
            continue;
        }
        // Floors above and below are context: dim outline only, no walls, no
        // solid. They answer "what is under me" without competing with the
        // floor you are actually working on.
        let is_focus_floor = state.layer.is_focus(coord.level);
        report.cells += 1;

        // With detail on, the schematic must ride just above the authored floor
        // slab or the solid buries it. Keeping both visible is the point: these
        // lines are the *solver's* topology and the solid is the *authored*
        // geometry, and a disagreement between them is a bug worth seeing.
        let origin = Vec3::from_array(hex_origin(*coord)) + Vec3::Y * schematic_lift;
        let sketch = cell_sketch(&solved.world, *coord, placement);
        let height = sketch.height.unwrap_or(0.9) * WALL_FRACTION * 8.0;

        // Mid-replay a cell the solver has not observed yet keeps its floor
        // plan and nothing else — the lattice is there, the cell is not. A
        // contradicted cell takes the red ring, because propagation emptying a
        // domain is the event a profile author most needs to see and it has no
        // geometry of its own to draw.
        let replayed = replay.as_ref().map(|cells| {
            cells
                .get(coord)
                .copied()
                .unwrap_or_else(observed_facility::hex_wfc::CellTrace::default)
        });
        if overlay_on {
            let ring_target = if replayed.is_some_and(|cell| cell.contradicted) {
                &mut changed_walls
            } else {
                &mut grid
            };
            for (a, b) in floor_ring(CELL_EXTENT) {
                ring_target.segment(origin + a, origin + b);
            }
        }

        // Compare mode: a cell counts as changed when this profile placed
        // something the baseline did not. Absent baseline (still solving, or
        // the baseline itself failed) means nothing is claimed to have changed.
        let differs =
            compare && baseline.is_some_and(|world| world.placements.get(coord) != Some(placement));
        if differs {
            report.changed += 1;
        }
        let is_pinned = pinned.contains(coord);
        if is_pinned {
            report.pinned += 1;
        }
        // Green means "the solver will not change this": in pin mode that is an
        // authored pin, in compare mode it is agreement with the baseline.
        let hold = if compare { !differs } else { is_pinned };

        // Context floors stop at the outline: no walls, no glyphs, no solid.
        // They say "something is under you" without competing with the floor
        // you are working on. A cell the replay has not reached stops there too,
        // for the same reason: its floor plan is real and its walls are not yet
        // decided. Pin and compare accounting happens above this, so a replay
        // never makes the panel understate how many cells are pinned.
        if !overlay_on || !is_focus_floor || replayed.is_some_and(|cell| cell.resolved.is_none()) {
            continue;
        }

        let mut open = [false; 6];
        for face in HexFace::LATERAL {
            open[face.index()] = placement.is_open(face);
        }
        // One question at a time. When the solid is drawing, pins and compare
        // are already said there, so these lines stay the neutral `Grid` and
        // mean only "this is what the solver placed". Two unrelated questions
        // in one pair of colours is the Legibility Contract's exact failure
        // case — and it is why the neighbourhood explorer, whose own green and
        // red answer "is this cage showing an alternative", has always stepped
        // the schematic back to `Grid`.
        let target = if !overlay_carries_signals
            || state.detail_mode == crate::detail::DetailMode::Neighborhood
        {
            &mut grid
        } else if hold {
            &mut walls
        } else {
            &mut changed_walls
        };
        for (a, b) in wall_edges(height, open) {
            target.segment(origin + a, origin + b);
        }

        let climbs = matches!(placement.archetype, HexArchetype::RampUp)
            || (placement.archetype == HexArchetype::Shaft && placement.up != PortClass::Sealed);
        if climbs {
            for (a, b) in vertical_glyph(height) {
                target.segment(origin + a, origin + b);
            }
        }

        // Selection and hover rings are NOT emitted here — they live in
        // `rebuild_overlay`, so moving the mouse never re-triangulates hulls.
    }

    for (batch, role) in [
        (grid, SchematicRole::Grid),
        (walls, SchematicRole::Pinned),
        (changed_walls, SchematicRole::Volatile),
    ] {
        if batch.is_empty() {
            continue;
        }
        let Some(mesh) = batch.into_mesh() else {
            continue;
        };
        report.meshes += 1;
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(unlit(schematic(role)))),
            StudioVisual,
        ));
    }

    state.report = report;
    state.schematic_forced = forced_reason;
}
