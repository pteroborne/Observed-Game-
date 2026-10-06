//! Turning the survivor's knowledge into the isometric sketch.
//!
//! Projection is gated on the local team's `HexPlayerMapKnowledge`. Unknown
//! source cells and incomplete room footprints never emit authored hulls.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use observed_content::ArchitectureRegister;

use observed_hex::{HexCoord, hex_origin, prism_hull};
use observed_match::hex_wfc::HexBodyPlace;
use observed_style::{HexComposition, MarkerRole};
use std::collections::{BTreeMap, BTreeSet};

use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

use crate::hex_wfc::view::assets::hull_mesh;

use super::cell::{CellState, Stability, composition, marker_key, sketch};
use super::overlay::{rooms_present, spawn_orientation_frame, spawn_player_marker};
use super::{HexMapCell, HexMapLandmark, HexMapVisual, MAP_RENDER_LAYER};

/// A stability cap is a thin plate, not a tower — it must not read as height,
/// because height already means archetype.
const CAP_THICKNESS: f32 = 0.35;

/// What the build measured, for the legend and for the tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct MapCensus {
    pub(super) bounds: Option<(Vec3, Vec3)>,
    pub(super) traversed: usize,
    pub(super) glimpsed: usize,
    pub(super) stale: usize,
    pub(super) rooms: BTreeSet<String>,
    pub(super) room_cells: usize,
    pub(super) hall_cells: usize,
    pub(super) vertical_cells: usize,
    pub(super) permanent: usize,
    pub(super) held: usize,
    pub(super) mutable: usize,
    pub(super) floors: BTreeSet<u8>,
}

impl MapCensus {
    pub(super) fn see(&mut self, at: Vec3, extent: Vec3) {
        self.bounds = Some(match self.bounds {
            None => (at - extent, at + extent),
            Some((min, max)) => (min.min(at - extent), max.max(at + extent)),
        });
    }
}

pub(super) struct MapAssets<'a> {
    pub(super) meshes: &'a mut bevy::asset::Assets<Mesh>,
    pub(super) materials: &'a mut bevy::asset::Assets<StandardMaterial>,
    pub(super) prisms: BTreeMap<(u32, u32), Handle<Mesh>>,
    pub(super) bars: BTreeMap<(u32, u32, u32), Handle<Mesh>>,
    pub(super) tint: BTreeMap<(u8, u8, u8, u8), Handle<StandardMaterial>>,
    pub(super) signal: BTreeMap<u8, Handle<StandardMaterial>>,
}

impl MapAssets<'_> {
    pub(super) fn prism(&mut self, height: f32, inset: f32) -> Handle<Mesh> {
        self.prisms
            .entry((height.to_bits(), inset.to_bits()))
            .or_insert_with(|| {
                let hull = prism_hull(height, inset).map(Vec3::from_array);
                self.meshes
                    .add(hull_mesh(&hull).expect("a hex prism is a non-degenerate hull"))
            })
            .clone()
    }

    /// Bars are cached by their dimensions. Hex neighbours sit at only two
    /// distinct pitches, so without this the map allocates one mesh asset per
    /// connection on every rebuild — and it rebuilds whenever knowledge changes.
    pub(super) fn bar(&mut self, length: f32, height: f32, width: f32) -> Handle<Mesh> {
        self.bars
            .entry((length.to_bits(), height.to_bits(), width.to_bits()))
            .or_insert_with(|| self.meshes.add(Cuboid::new(length, height, width)))
            .clone()
    }

    pub(super) fn tint(
        &mut self,
        register: ArchitectureRegister,
        state: CellState,
        focused: bool,
    ) -> Handle<StandardMaterial> {
        self.tint
            .entry((register as u8, state.key(), u8::from(focused), 0))
            .or_insert_with(|| {
                let (base_color, emissive) = state.treatment(register, focused);
                self.materials.add(StandardMaterial {
                    base_color,
                    emissive,
                    perceptual_roughness: 0.92,
                    ..default()
                })
            })
            .clone()
    }

    pub(super) fn signal(&mut self, role: MarkerRole) -> Handle<StandardMaterial> {
        self.signal
            .entry(marker_key(role))
            .or_insert_with(|| {
                let treatment = observed_style::marker(role);
                self.materials.add(StandardMaterial {
                    base_color: treatment.base_color,
                    emissive: treatment.emissive,
                    perceptual_roughness: 0.35,
                    ..default()
                })
            })
            .clone()
    }
}

/// Build the whole sketch and return what it measured.
pub(super) fn build(
    commands: &mut Commands,
    runtime: &HexWfcRuntime,
    meshes: &mut bevy::asset::Assets<Mesh>,
    materials: &mut bevy::asset::Assets<StandardMaterial>,
) -> MapCensus {
    let mut census = MapCensus::default();
    let Some(knowledge) = runtime.match_state.player_map(runtime.local_player) else {
        return census;
    };
    let world = &runtime.match_state.facility;
    let focus = runtime.map_level;
    let you_cell = runtime.local().cell;
    let exit_cell = world.config.exit();

    // Teammate occupancy is leak-free — the survivor already knows where their
    // own team is. The global `match_state.observation` frame deliberately is
    // not consulted; it would report cells pinned by a rival and hand over that
    // rival's position.
    let local_team = runtime.local().team;
    let team_cells = runtime
        .match_state
        .players
        .values()
        .filter(|player| {
            player.team == local_team && !player.escaped && player.place == HexBodyPlace::Facility
        })
        .map(|player| player.cell)
        .collect::<BTreeSet<_>>();

    let mut assets = MapAssets {
        meshes,
        materials,
        prisms: BTreeMap::new(),
        bars: BTreeMap::new(),
        tint: BTreeMap::new(),
        signal: BTreeMap::new(),
    };

    let drawn_sources = draw_hulls(commands, runtime, &mut assets, &mut census);

    for (&cell, known) in &knowledge.cells {
        census.floors.insert(cell.level);
        let Some(placement) = world.placements.get(&cell) else {
            continue;
        };
        let in_blueprint = world.room_id_at(cell).is_some();
        let drawn = sketch(placement.archetype, placement.space, in_blueprint);
        let Some(height) = drawn.height else {
            continue;
        };
        let composition = composition(placement.archetype, placement.space, in_blueprint);
        let stability = runtime.ascent.as_ref().map_or_else(
            || {
                Stability::of(
                    placement.archetype,
                    known.anchored,
                    team_cells.contains(&cell),
                )
            },
            |ascent| {
                Stability::in_ascent(
                    matches!(
                        placement.archetype,
                        observed_facility::hex_wfc::HexArchetype::Climb { .. }
                    ) || ascent.rules().prison_core.contains(&cell)
                        || ascent.rules().fixed_structure(cell),
                    known.anchored,
                )
            },
        );
        let state = CellState::of(known, world, cell);
        let focused = cell.level == focus;
        if !focused {
            continue;
        }
        let register = world
            .architecture
            .get(&cell)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);

        match state {
            CellState::Traversed => census.traversed += 1,
            CellState::Glimpsed => census.glimpsed += 1,
            CellState::Stale => census.stale += 1,
        }
        match composition {
            HexComposition::Room => census.room_cells += 1,
            HexComposition::Hall => census.hall_cells += 1,
            HexComposition::Vertical => census.vertical_cells += 1,
        }
        match stability {
            Stability::Permanent => census.permanent += 1,
            Stability::Held => census.held += 1,
            Stability::Mutable => census.mutable += 1,
        }

        // The player beacon and exit pillar keep semantic priority over architecture.
        let signal = if cell == you_cell
            && runtime.local().place == HexBodyPlace::Facility
            && !runtime.local().escaped
        {
            Some(MarkerRole::You)
        } else if cell == exit_cell {
            Some(MarkerRole::Exit)
        } else {
            None
        };

        let origin = Vec3::from_array(hex_origin(cell));
        census.see(origin + Vec3::Y * height * 0.5, Vec3::new(8.0, height, 8.0));

        // The marker owns cell identity for inspection; the hulls themselves may
        // span several cells of a fully discovered room.
        commands.spawn((
            HexMapVisual,
            HexMapCell,
            DespawnOnExit(GameState::HexWfc),
            Transform::from_translation(origin),
        ));
        let drawn_hulls = drawn_sources.contains(&cell);
        let footprint_known = world
            .blueprints
            .iter()
            .find(|room| room.cells.contains(&cell))
            .is_none_or(|room| {
                room.cells.iter().all(|c| {
                    knowledge
                        .cells
                        .get(c)
                        .is_some_and(|k| !k.is_stale(world, *c))
                })
            });
        // Stale or partially discovered rooms remain a flat memory marker. Never
        // consult their current hulls: that would reveal a rewrite or a hidden room.
        if !drawn_hulls && (state == CellState::Stale || !footprint_known) {
            let mesh = assets.prism(0.2, drawn.inset);
            let material = match signal {
                Some(role) => assets.signal(role),
                None => assets.tint(register, state, true),
            };
            commands.spawn((
                HexMapVisual,
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                RenderLayers::layer(MAP_RENDER_LAYER),
                Transform::from_translation(origin),
            ));
        }

        if cell == exit_cell {
            let exit_pillar = assets.bar(1.4, 7.0, 1.4);
            let exit_material = assets.signal(MarkerRole::Exit);
            commands.spawn((
                HexMapVisual,
                HexMapLandmark,
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(exit_pillar),
                MeshMaterial3d(exit_material),
                RenderLayers::layer(MAP_RENDER_LAYER),
                Transform::from_translation(origin + Vec3::Y * (height + 3.5)),
                Name::new("Hex map exit landmark"),
            ));
        }

        if let Some(role) = stability.cap() {
            let cap = assets.prism(CAP_THICKNESS, drawn.inset * 0.62);
            let material = assets.signal(role);
            commands.spawn((
                HexMapVisual,
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(cap),
                MeshMaterial3d(material),
                RenderLayers::layer(MAP_RENDER_LAYER),
                Transform::from_translation(origin + Vec3::Y * (height + 0.05)),
                Name::new(stability.label()),
            ));
        }

        if known.anchored {
            let anchor_pin = assets.bar(0.8, 3.0, 0.8);
            let anchor_mat = assets.signal(MarkerRole::Control);
            commands.spawn((
                HexMapVisual,
                HexMapLandmark,
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(anchor_pin),
                MeshMaterial3d(anchor_mat),
                RenderLayers::layer(MAP_RENDER_LAYER),
                Transform::from_translation(origin + Vec3::Y * (height + 1.5)),
                Name::new("Hex map anchor landmark"),
            ));
        }
    }

    rooms_present(commands, world, knowledge, focus, &mut assets, &mut census);

    if you_cell.level == focus && knowledge.cells.contains_key(&you_cell) {
        let here_height = world.placements.get(&you_cell).map(|_| 0.6).unwrap_or(0.6);
        spawn_player_marker(commands, runtime, here_height, &mut assets, &mut census);
    }
    spawn_orientation_frame(commands, &mut census, focus, &mut assets);

    census
}

/// Hull ownership is independent of the displayed floor: a room anchored below
/// can carry upper-storey geometry. Slice the actual hulls as the replay does.
fn draw_hulls(
    commands: &mut Commands,
    runtime: &HexWfcRuntime,
    assets: &mut MapAssets,
    census: &mut MapCensus,
) -> BTreeSet<HexCoord> {
    let game = &runtime.match_state;
    let Some(knowledge) = game.player_map(runtime.local_player) else {
        return BTreeSet::new();
    };
    let allowed: BTreeSet<_> = knowledge
        .cells
        .iter()
        .filter(|(cell, known)| {
            !known.is_stale(&game.facility, **cell)
                && game
                    .facility
                    .blueprints
                    .iter()
                    .find(|room| room.cells.contains(cell))
                    .is_none_or(|room| {
                        room.cells.iter().all(|c| {
                            knowledge
                                .cells
                                .get(c)
                                .is_some_and(|k| !k.is_stale(&game.facility, *c))
                        })
                    })
        })
        .map(|(&cell, _)| cell)
        .collect();
    let mut drawn = BTreeSet::new();
    for piece in &game.geometry.pieces {
        let cell = piece.source_cell;
        if !allowed.contains(&cell) {
            continue;
        }
        let Some(floor) = crate::view::cutaway::surface(
            piece,
            runtime.map_level,
            observed_style::iso::detent_bearing(0),
            false,
        ) else {
            continue;
        };
        let Some(mesh) = crate::view::cutaway::mesh(&piece.shape) else {
            continue;
        };
        let register = game
            .facility
            .architecture
            .get(&cell)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        let role = if floor {
            observed_style::ArchitectureSurfaceRole::Floor
        } else {
            observed_style::ArchitectureSurfaceRole::Wall
        };
        let look = observed_style::hex_shell_surface(register, role);
        let material = assets
            .tint
            .entry((register as u8, 3 + u8::from(floor), 0, 0))
            .or_insert_with(|| {
                assets.materials.add(StandardMaterial {
                    base_color: look.base_color,
                    emissive: look.emissive,
                    perceptual_roughness: 0.9,
                    ..default()
                })
            })
            .clone();
        let mesh = assets.meshes.add(mesh);
        let rotation = Quat::from_array(piece.rotation);
        let points = match &piece.shape {
            observed_traversal::ColliderShape::ConvexHull { points } => points.clone(),
            observed_traversal::ColliderShape::Cuboid { half } => (0..8u8)
                .map(|i| {
                    Vec3::new(
                        if i & 1 == 0 { -half.x } else { half.x },
                        if i & 2 == 0 { -half.y } else { half.y },
                        if i & 4 == 0 { -half.z } else { half.z },
                    )
                })
                .collect(),
        };
        let (min, max) = points
            .into_iter()
            .map(|p| piece.center + rotation * p)
            .fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(min, max), p| (min.min(p), max.max(p)),
            );
        census.see((min + max) * 0.5, (max - min) * 0.5);
        commands.spawn((
            HexMapVisual,
            super::HexMapHull {
                source: (cell.q, cell.r, cell.level),
            },
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::from_translation(piece.center).with_rotation(rotation),
        ));
        drawn.insert(cell);
    }
    drawn
}
