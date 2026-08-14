//! The authored deck: what the facility will actually be built as.
//!
//! Re-exports the shared cutaway rendering under the studio's original module
//! path so its established callers keep the same interface, and owns the
//! studio's emission pass over it.
//!
//! This used to be an on-demand extra behind `F`, with a wireframe schematic as
//! the default view. That had the tool showing a diagram of a building rather
//! than the building, so the two swapped places: the solid is the view, and
//! `draw.rs`'s line work is an overlay you ask for when you want to compare the
//! *solver's* topology against the *authored* geometry.
//!
//! The signals the schematic used to carry in its wall bands — pins, the
//! baseline compare, the replay — come with it, as treatments on the solid.
//! That is `tactics_lab`'s rule (`board_treatment`) rather than a new one: a
//! cell wears its district accent when it has nothing else to say, and a named
//! signal treatment when it does.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexArchetype;
use observed_hex::HexCoord;
use observed_style::{SchematicRole, Treatment, schematic};

pub use observed_cutaway::{
    DetailMode, DetailReport, HullBatch, TileMeshCache, WallMode, build, build_low_walls,
    build_walls, focus_set, measure, mesh_from, partial_wall_height, survives,
};

use crate::StudioState;
use crate::draw::{KEY_ILLUMINANCE, KEY_LIGHT_OFFSET, StudioVisual};

/// What one cell means right now, in the order the viewport must resolve.
///
/// Precedence is deliberate and is the same order the variants are declared:
/// selection outranks everything because a cell you are working on must stay
/// findable, and the two replay states outrank authoring state because while a
/// replay runs it owns the colour.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CellSignal {
    Selected,
    /// Mid-replay: propagation emptied this cell's domain.
    Contradicted,
    /// Mid-replay: the solver has not observed this cell yet, so it has no
    /// authored geometry to draw. Not a class the solid pass renders.
    Unobserved,
    /// Compare mode: this profile placed something the baseline did not.
    Changed,
    Pinned,
    Ordinary,
}

impl CellSignal {
    /// The named treatment, or `None` for a cell that should wear its district.
    #[must_use]
    fn treatment(self) -> Option<Treatment> {
        match self {
            Self::Selected => Some(schematic(SchematicRole::Selected)),
            Self::Contradicted | Self::Changed => Some(schematic(SchematicRole::Volatile)),
            Self::Pinned => Some(schematic(SchematicRole::Pinned)),
            Self::Unobserved | Self::Ordinary => None,
        }
    }
}

/// What every drawn cell means, resolved once so the solid and the schematic
/// overlay cannot disagree about the same cell.
#[must_use]
pub fn classify(state: &StudioState) -> BTreeMap<HexCoord, CellSignal> {
    let mut signals = BTreeMap::new();
    let Some(solved) = state.solved.as_ref() else {
        return signals;
    };
    let pinned = crate::brush::pinned_coords(&state.profile);
    let replay = crate::timeline::replay_cells(state);
    // The compare overlay asks "what did my tuning move", against a finished
    // baseline world. Mid-replay that question has no answer, and its colour
    // would sit in the same viewport as the replay's meaning something else.
    let compare = state.show_baseline_compare && replay.is_none();

    for (coord, placement) in &solved.world.placements {
        if placement.archetype == HexArchetype::Void || !state.layer.draws(coord.level) {
            continue;
        }
        let signal = if state.selected == Some(*coord) {
            CellSignal::Selected
        } else if let Some(cells) = replay.as_ref() {
            let cell = cells.get(coord).copied().unwrap_or_default();
            if cell.contradicted {
                CellSignal::Contradicted
            } else if cell.resolved.is_none() {
                CellSignal::Unobserved
            } else {
                CellSignal::Ordinary
            }
        } else if compare
            && state
                .baseline_world
                .as_ref()
                .is_some_and(|world| world.placements.get(coord) != Some(placement))
        {
            CellSignal::Changed
        } else if pinned.contains(coord) {
            CellSignal::Pinned
        } else {
            CellSignal::Ordinary
        };
        signals.insert(*coord, signal);
    }
    signals
}

/// Which cells the solid pass draws, before signals are applied.
fn scope(state: &StudioState, signals: &BTreeMap<HexCoord, CellSignal>) -> BTreeSet<HexCoord> {
    let Some(solved) = state.solved.as_ref() else {
        return BTreeSet::new();
    };
    let drawn: BTreeSet<HexCoord> = signals
        .iter()
        // Context floors above and below stay line work, and a cell the replay
        // has not reached has nothing authored to draw.
        .filter(|(coord, signal)| {
            state.layer.is_focus(coord.level) && **signal != CellSignal::Unobserved
        })
        .map(|(coord, _)| *coord)
        .collect();

    match state.detail_mode {
        // The schematic-only escape hatch: the view this tool used to open on.
        DetailMode::Off => BTreeSet::new(),
        DetailMode::Layer => drawn,
        DetailMode::Focus => focus_set(&solved.world, state.selected)
            .intersection(&drawn)
            .copied()
            .collect(),
        // Only the centre is solid here. Its ring is drawn as wireframe by
        // `neighbors::emit`, which is the whole point of the mode: solid is what
        // is settled, and everything touching it is still a question.
        DetailMode::Neighborhood => state.selected.into_iter().collect(),
    }
}

/// Draw the authored deck.
pub(crate) fn emit_detail(
    commands: &mut Commands,
    state: &StudioState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut TileMeshCache,
) -> DetailReport {
    let Some(solved) = state.solved.as_ref() else {
        return DetailReport::default();
    };
    // Without a projected catalog there is nothing authored to draw. `draw.rs`
    // notices the same thing and forces its schematic back on, so the viewport
    // reports a missing catalog rather than going black.
    let Some(snapshot) = solved.geometry.as_ref() else {
        return DetailReport::default();
    };
    let signals = classify(state);
    let cells = scope(state, &signals);
    if cells.is_empty() {
        return DetailReport::default();
    }

    // One pass per signal class. Deliberately not a change to
    // `observed_cutaway`'s bucketing API, which `tactics_lab` and
    // `iso_observer_lab` also depend on; the extra passes are over an already
    // built snapshot, on a rebuild that only happens when geometry changes.
    let mut by_signal: BTreeMap<CellSignal, BTreeSet<HexCoord>> = BTreeMap::new();
    for coord in &cells {
        let signal = signals.get(coord).copied().unwrap_or(CellSignal::Ordinary);
        by_signal.entry(signal).or_default().insert(*coord);
    }

    let bearing = crate::viewport::detent_bearing(state.detent);
    let mut report = DetailReport::default();
    let mut batches: Vec<(Mesh, Treatment)> = Vec::new();

    for (signal, class) in by_signal {
        let (_, context, pass) = build_walls(
            &solved.world,
            snapshot,
            &class,
            None,
            state.walls,
            bearing,
            cache,
        );
        report.cells += pass.cells;
        report.hulls_drawn += pass.hulls_drawn;
        report.hulls_cut += pass.hulls_cut;
        report.distinct_hulls_cached = report.distinct_hulls_cached.max(pass.distinct_hulls_cached);
        for (register, batch) in context {
            let Some(mesh) = batch.into_mesh() else {
                continue;
            };
            // A cell wears its district unless it has something to say.
            let treatment = signal
                .treatment()
                .unwrap_or_else(|| observed_style::architecture_tactical(register));
            batches.push((mesh, treatment));
        }
    }

    // A key light aimed off the current view bearing. Deriving it from the
    // detent means contrast stays equivalent at all six stops instead of one
    // stop happening to look flat; offsetting it from the view axis is what
    // separates a wall face from the floor it stands on, which head-on lighting
    // cannot do.
    let bearing3 = Vec3::new(bearing.x, 0.0, bearing.y);
    let key_from = Quat::from_rotation_y(KEY_LIGHT_OFFSET) * bearing3 + Vec3::Y * 1.15;
    commands.spawn((
        DirectionalLight {
            illuminance: KEY_ILLUMINANCE,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(key_from).looking_at(Vec3::ZERO, Vec3::Y),
        StudioVisual,
    ));
    spawn_district_lights(commands, state, &cells);

    for (mesh, treatment) in batches {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: treatment.base_color,
                emissive: treatment.emissive,
                // Lit, unlike the schematic: this is surface, and without
                // shading every face of a hull is the same flat colour, so
                // interior geometry silhouettes but never reads.
                unlit: false,
                perceptual_roughness: 0.82,
                metallic: 0.04,
                cull_mode: None,
                double_sided: true,
                ..default()
            })),
            StudioVisual,
        ));
    }
    report
}

/// One local practical pool per visible architecture register.
///
/// Ported from `tactics_lab`, for its reason: a view may contain several
/// districts at once, so a single global palette would necessarily lie about
/// all but one of them.
fn spawn_district_lights(commands: &mut Commands, state: &StudioState, cells: &BTreeSet<HexCoord>) {
    let Some(solved) = state.solved.as_ref() else {
        return;
    };
    let mut centres: BTreeMap<ArchitectureRegister, (Vec3, u32)> = BTreeMap::new();
    for coord in cells {
        let register = solved
            .world
            .architecture
            .get(coord)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        let entry = centres.entry(register).or_insert((Vec3::ZERO, 0));
        entry.0 += Vec3::from_array(observed_hex::hex_origin(*coord));
        entry.1 += 1;
    }
    for (register, (sum, count)) in centres {
        #[allow(clippy::cast_precision_loss)]
        let centre = sum / count.max(1) as f32;
        let palette = observed_style::architecture(register);
        commands.spawn((
            PointLight {
                color: palette.light_color,
                intensity: observed_style::iso::light::PRACTICAL_INTENSITY,
                range: observed_style::iso::light::PRACTICAL_RANGE,
                radius: observed_style::iso::light::PRACTICAL_RADIUS,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(
                centre + Vec3::Y * observed_style::iso::light::PRACTICAL_LIFT,
            ),
            StudioVisual,
        ));
    }
}
