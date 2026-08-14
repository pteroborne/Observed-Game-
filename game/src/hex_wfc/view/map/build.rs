//! Turning the survivor's knowledge into the isometric sketch.
//!
//! Everything read here is gated on membership in the local team's
//! `HexPlayerMapKnowledge`. The facility is indexed only at cells that knowledge
//! already contains, so an undiscovered placement is never touched.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexWfcWorld;
use observed_facility::map_spec::RoomRole;
use observed_hex::{HexCoord, HexFace, PortClass, TILE_LEVEL_HEIGHT, hex_origin, prism_hull};
use observed_match::hex_wfc::{HexMapDiscovery, HexPlayerMapKnowledge};
use observed_style::{HexComposition, MarkerRole, hex_link};
use std::collections::{BTreeMap, BTreeSet};

use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

use crate::hex_wfc::view::assets::hull_mesh;

use super::cell::{CellState, Stability, archetype_height, composition, marker_key, sketch};
use super::{HexMapCell, HexMapVisual, MAP_RENDER_LAYER};

/// Only the three "forward" lateral faces are walked, so each undirected edge is
/// drawn exactly once instead of twice from opposite ends.
const FORWARD_FACES: [HexFace; 3] = [HexFace::East, HexFace::SouthEast, HexFace::SouthWest];
/// Connection bars ride just above the *lower* of the two cells they join, so
/// they read as a bridge laid over the composition rather than a rod driven
/// through it. Riding at floor height buries every bar inside the prisms.
const LINK_CLEARANCE: f32 = 0.25;
/// A vertical riser is pushed out past a cell's footprint, because a link drawn
/// up the middle of a shaft column is inside the column and invisible.
const RISER_OFFSET: f32 = 6.6;
const LINK_THICKNESS: f32 = 1.7;
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
    pub(super) lateral_links: usize,
    pub(super) vertical_links: usize,
    pub(super) permanent: usize,
    pub(super) held: usize,
    pub(super) mutable: usize,
    pub(super) floors: BTreeSet<u8>,
}

impl MapCensus {
    fn see(&mut self, at: Vec3, extent: Vec3) {
        self.bounds = Some(match self.bounds {
            None => (at - extent, at + extent),
            Some((min, max)) => (min.min(at - extent), max.max(at + extent)),
        });
    }
}

struct Assets<'a> {
    meshes: &'a mut bevy::asset::Assets<Mesh>,
    materials: &'a mut bevy::asset::Assets<StandardMaterial>,
    prisms: BTreeMap<(u32, u32), Handle<Mesh>>,
    bars: BTreeMap<(u32, u32, u32), Handle<Mesh>>,
    tint: BTreeMap<(u8, u8, u8, u8), Handle<StandardMaterial>>,
    signal: BTreeMap<u8, Handle<StandardMaterial>>,
}

impl Assets<'_> {
    fn prism(&mut self, height: f32, inset: f32) -> Handle<Mesh> {
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
    fn bar(&mut self, length: f32, height: f32, width: f32) -> Handle<Mesh> {
        self.bars
            .entry((length.to_bits(), height.to_bits(), width.to_bits()))
            .or_insert_with(|| self.meshes.add(Cuboid::new(length, height, width)))
            .clone()
    }

    fn tint(
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

    fn signal(&mut self, role: MarkerRole) -> Handle<StandardMaterial> {
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

    /// Links are structure, not district: they read as the route between two
    /// places, so they take the spine treatment and dim with the weaker of the
    /// two endpoints rather than borrowing either endpoint's colour.
    fn link(&mut self, confident: bool) -> Handle<StandardMaterial> {
        self.signal
            .entry(if confident { 200 } else { 201 })
            .or_insert_with(|| {
                let treatment = hex_link(confident);
                self.materials.add(StandardMaterial {
                    base_color: treatment.base_color,
                    emissive: treatment.emissive,
                    perceptual_roughness: 0.6,
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
        .filter(|player| player.team == local_team && !player.escaped)
        .map(|player| player.cell)
        .collect::<BTreeSet<_>>();

    let mut assets = Assets {
        meshes,
        materials,
        prisms: BTreeMap::new(),
        bars: BTreeMap::new(),
        tint: BTreeMap::new(),
        signal: BTreeMap::new(),
    };

    for (&cell, known) in &knowledge.cells {
        let Some(placement) = world.placements.get(&cell) else {
            continue;
        };
        let in_blueprint = world.room_id_at(cell).is_some();
        let drawn = sketch(placement.archetype, placement.space, in_blueprint);
        let Some(height) = drawn.height else {
            continue;
        };
        let composition = composition(placement.archetype, placement.space, in_blueprint);
        let stability = Stability::of(
            placement.archetype,
            known.anchored,
            team_cells.contains(&cell),
        );
        let state = CellState::of(known, world, cell);
        let focused = cell.level == focus;
        let register = world
            .architecture
            .get(&cell)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);

        census.floors.insert(cell.level);
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

        // Signal tier overrides the district. The Legibility Contract requires
        // gameplay-critical cues to punch through atmosphere, and "where am I"
        // and "where is the way out" are exactly that. Recolouring the cell
        // rather than floating a token above it is deliberate: a token is
        // occluded by whatever sits on the level above, a recoloured cell never
        // is. This is the 3D equivalent of the corner sketch's `@` and `X`.
        let signal = if cell == you_cell {
            Some(MarkerRole::You)
        } else if cell == exit_cell {
            Some(MarkerRole::Exit)
        } else {
            None
        };

        let origin = Vec3::from_array(hex_origin(cell));
        census.see(origin + Vec3::Y * height * 0.5, Vec3::new(8.0, height, 8.0));

        let mesh = assets.prism(height, drawn.inset);
        let material = match signal {
            Some(role) => assets.signal(role),
            None => assets.tint(register, state, focused),
        };
        commands.spawn((
            HexMapVisual,
            HexMapCell,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::from_translation(origin),
            Name::new(composition.label()),
        ));

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
    }

    links(commands, knowledge, world, &mut assets, &mut census);
    rooms_present(commands, world, knowledge, &mut census);
    census
}

/// Draw a bar for every connection the survivor has actually seen from both
/// sides. This is what turns a field of tiles into a graph: corridor runs become
/// visible lines, and a vertical link shows a floor is reachable from here.
fn links(
    commands: &mut Commands,
    knowledge: &HexPlayerMapKnowledge,
    world: &HexWfcWorld,
    assets: &mut Assets,
    census: &mut MapCensus,
) {
    let grid = world.config.grid();
    for (&cell, known) in &knowledge.cells {
        let origin = Vec3::from_array(hex_origin(cell));
        let here_height = world
            .placements
            .get(&cell)
            .and_then(|p| archetype_height(p.archetype))
            .unwrap_or(0.9);

        for face in FORWARD_FACES {
            if known.known_ports.port(face) == PortClass::Sealed {
                continue;
            }
            let Some(neighbour) = grid.neighbor(cell, face) else {
                continue;
            };
            let Some(neighbour_known) = knowledge.cells.get(&neighbour) else {
                continue;
            };
            if neighbour_known.known_ports.port(face.opposite()) == PortClass::Sealed {
                continue;
            }
            let target = Vec3::from_array(hex_origin(neighbour));
            let span = target - origin;
            // Ride over the shorter of the two, so the bar clears both masses.
            let deck = world
                .placements
                .get(&neighbour)
                .and_then(|p| archetype_height(p.archetype))
                .map_or(here_height, |other| here_height.min(other))
                + LINK_CLEARANCE;
            let length = span.length();
            if length <= f32::EPSILON {
                continue;
            }
            let confident = !known.is_stale(world, cell)
                && !neighbour_known.is_stale(world, neighbour)
                && known.discovery == HexMapDiscovery::Traversed
                && neighbour_known.discovery == HexMapDiscovery::Traversed;
            // Quantize so near-identical spans share one cached mesh.
            let length = (length * 16.0).round() / 16.0;
            let mesh = assets.bar(length, LINK_THICKNESS * 0.4, LINK_THICKNESS);
            let material = assets.link(confident);
            commands.spawn((
                HexMapVisual,
                DespawnOnExit(GameState::HexWfc),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                RenderLayers::layer(MAP_RENDER_LAYER),
                Transform::from_translation(origin + span * 0.5 + Vec3::Y * deck)
                    .with_rotation(Quat::from_rotation_y((-span.z).atan2(span.x))),
                Name::new("Hex map link"),
            ));
            census.lateral_links += 1;
        }

        // Vertical links are walked upward only, for the same reason the lateral
        // pass takes three faces: one bar per connection, not two.
        if known.known_ports.port(HexFace::Up) == PortClass::Sealed {
            continue;
        }
        let above = HexCoord {
            q: cell.q,
            r: cell.r,
            level: cell.level.saturating_add(1),
        };
        let Some(above_known) = knowledge.cells.get(&above) else {
            continue;
        };
        if above_known.known_ports.port(HexFace::Down) == PortClass::Sealed {
            continue;
        }
        let mesh = assets.bar(
            LINK_THICKNESS * 0.8,
            TILE_LEVEL_HEIGHT,
            LINK_THICKNESS * 0.8,
        );
        let material = assets.link(true);
        commands.spawn((
            HexMapVisual,
            DespawnOnExit(GameState::HexWfc),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::from_translation(
                origin + Vec3::Y * (TILE_LEVEL_HEIGHT * 0.5) - Vec3::Z * RISER_OFFSET,
            ),
            Name::new("Hex map vertical link"),
        ));
        census.vertical_links += 1;
    }
}

/// Name the rooms the survivor has actually set foot in or seen part of. A room
/// is one decision beat, so knowing *which* rooms are on your map is worth more
/// than knowing how many cells they occupy.
fn rooms_present(
    commands: &mut Commands,
    world: &HexWfcWorld,
    knowledge: &HexPlayerMapKnowledge,
    census: &mut MapCensus,
) {
    for blueprint in &world.blueprints {
        let known_cell = blueprint
            .cells
            .iter()
            .filter_map(|cell| {
                knowledge
                    .cells
                    .get(cell)
                    .filter(|known| known.room_role == Some(blueprint.role))
                    .map(|_| *cell)
            })
            .min();
        let Some(cell) = known_cell else { continue };
        census.rooms.insert(blueprint.role.label().to_string());
        commands.spawn((
            HexMapVisual,
            DespawnOnExit(GameState::HexWfc),
            Text2d::new(room_label(blueprint.role)),
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(observed_style::marker(MarkerRole::NextRoom).base_color),
            RenderLayers::layer(MAP_RENDER_LAYER),
            Transform::from_translation(Vec3::from_array(hex_origin(cell)) + Vec3::Y * 8.0)
                .with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    std::f32::consts::FRAC_PI_4,
                    -0.615_479_7,
                    0.0,
                )),
            Name::new(format!("known {} room label", blueprint.role.label())),
        ));
    }
}

fn room_label(role: RoomRole) -> &'static str {
    match role {
        RoomRole::Start => "START",
        RoomRole::Decision => "DECIDE",
        RoomRole::DecoherenceFork => "FORK",
        RoomRole::Keystone => "KEY",
        RoomRole::DualStation => "SYNC",
        RoomRole::Monitor => "SURVEY",
        RoomRole::AnchorCheckpoint => "ANCHOR",
        RoomRole::GuardianControl => "GUARD",
        RoomRole::Recovery => "RECOVER",
        RoomRole::Exit => "EXIT",
        RoomRole::TeleportRelay => "RELAY",
    }
}
