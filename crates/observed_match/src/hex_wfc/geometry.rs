//! Authored hex-tile prefabs projected into one continuous collision arena.

use std::collections::{BTreeMap, BTreeSet};

use glam::{Quat, Vec2, Vec3};
use observed_authoring::{ModuleCellRef, RoomPrototype, TileKey, TileLightKind, TilePrototype};
use observed_facility::hex_wfc::{
    HexArchetype, HexPlacement, HexRelayoutDelta, HexSpace, HexWfcWorld, PortSignature,
    StampedBlueprint, blueprint_cell_archetype, blueprint_for_role, placement_tile_archetype,
};
use observed_facility::map_spec::RoomRole;
use observed_hex::{CORNERS, HexCoord, HexFace, PortClass, TILE_LEVEL_HEIGHT, hex_origin};
use observed_traversal::{
    ArenaSpec, ColliderDelta, ColliderShape, ColliderSpec, StableColliderId,
    rapier_controller::RapierTraversalScene,
};

const COLLIDER_STRIDE: usize = 128;
const SHELL_ID_BASE: u32 = 0xF000_0000;
const SHELL_THICKNESS: f32 = 0.5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexStructureRole {
    Hall,
    Room,
    Ramp,
    Shaft,
    Boundary,
}

/// One render/collision primitive from an authored prefab or the arena shell.
#[derive(Clone, Debug, PartialEq)]
pub struct HexStructurePiece {
    pub id: StableColliderId,
    /// Stable low-cell/blueprint anchor used by debug inspection.
    pub anchor: HexCoord,
    /// The concrete lattice cell whose ID range owns this primitive.
    pub source_cell: HexCoord,
    pub role: HexStructureRole,
    pub tile: Option<TileKey>,
    pub center: Vec3,
    pub rotation: [f32; 4],
    pub shape: ColliderShape,
}

/// One semantic practical selected with its authored prefab. Positions are
/// already transformed into facility world space; presentation supplies the
/// district colour, bounded energy, and shadow budget.
#[derive(Clone, Debug, PartialEq)]
pub struct HexLightSource {
    pub source_cell: HexCoord,
    pub role: HexStructureRole,
    pub tile: TileKey,
    pub kind: TileLightKind,
    pub position: Vec3,
}

impl HexStructurePiece {
    fn collider(&self) -> ColliderSpec {
        ColliderSpec {
            id: self.id,
            center: self.center,
            rotation: self.rotation,
            shape: self.shape.clone(),
            friction: 0.8,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HexWfcGeometrySnapshot {
    pub generation: u32,
    pub pieces: Vec<HexStructurePiece>,
    pub lights: Vec<HexLightSource>,
    pub arena: ArenaSpec,
    /// Ramp heads intentionally emit nothing: their low-cell prefab spans both levels.
    pub ramp_heads: usize,
    /// Each room blueprint is visited once, even when its footprint has many cells.
    pub blueprint_instances: usize,
    piece_indices: BTreeMap<StableColliderId, usize>,
    collider_indices: BTreeMap<StableColliderId, usize>,
}

/// Bounded geometry projection of one accepted logical relayout.
#[derive(Clone, Debug, PartialEq)]
pub struct HexGeometryDelta {
    pub previous_generation: u32,
    pub generation: u32,
    pub changed_cells: BTreeSet<HexCoord>,
    pub removed_piece_ids: BTreeSet<StableColliderId>,
    pub upserted_pieces: Vec<HexStructurePiece>,
    pub upserted_lights: Vec<HexLightSource>,
    pub colliders: ColliderDelta,
    pub ramp_heads: usize,
    pub blueprint_instances: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HexGeometryError {
    MissingArchitecture(HexCoord),
    MissingTile {
        coord: HexCoord,
        archetype: &'static str,
        register: String,
        signature: PortSignature,
    },
    BlueprintCellMissing(HexCoord),
    BlueprintArchetypeMissing {
        role: RoomRole,
        cell_index: usize,
    },
    TooManyHulls {
        coord: HexCoord,
        hulls: usize,
    },
    ColliderIdCapacity {
        cells: u64,
        required_max: u64,
    },
    StaleDelta {
        snapshot_generation: u32,
        delta_generation: u32,
    },
    InvalidArena,
}

impl HexWfcGeometrySnapshot {
    /// Project a solved world using its pure per-cell architecture register map.
    /// Manifest ordering does not affect selection: candidates are sorted by key.
    pub fn project(
        world: &HexWfcWorld,
        prototypes: &[TilePrototype],
    ) -> Result<Self, HexGeometryError> {
        Self::project_with_rooms(world, prototypes, &[])
    }

    /// Project a solved world, preferring one validated whole-room module over
    /// its per-cell fallback kit whenever the room contract matches exactly.
    pub fn project_with_rooms(
        world: &HexWfcWorld,
        prototypes: &[TilePrototype],
        room_prototypes: &[RoomPrototype],
    ) -> Result<Self, HexGeometryError> {
        validate_id_capacity(world)?;
        let catalogue = Catalogue::new(prototypes);
        let room_catalogue = RoomCatalogue::new(room_prototypes);
        let mut pieces = Vec::new();
        let mut lights = Vec::new();
        let mut consumed_rooms = BTreeSet::new();

        // A room footprint is one semantic instance. Its authored component
        // cells are expanded here, once from the blueprint anchor, and skipped
        // by the ordinary per-placement pass below.
        for blueprint in &world.blueprints {
            project_blueprint(
                world,
                blueprint,
                &catalogue,
                &room_catalogue,
                &mut consumed_rooms,
                &mut pieces,
                &mut lights,
            )?;
        }

        let mut ramp_heads = 0;
        for placement in world.placements.values() {
            if placement.space == HexSpace::Void || consumed_rooms.contains(&placement.coord) {
                continue;
            }
            if placement.archetype == HexArchetype::RampHead {
                ramp_heads += 1;
                continue;
            }
            let role = role_for(placement);
            let Some(archetype) = placement_tile_archetype(placement) else {
                continue;
            };
            let tile = tile_for(
                world,
                &catalogue,
                placement.coord,
                archetype,
                placement.ports(),
            )?;
            push_tile(
                world,
                placement.coord,
                placement.coord,
                role,
                tile,
                &mut pieces,
                &mut lights,
            )?;
        }

        push_boundary_shell(world, &mut pieces);
        sort_lights(&mut lights);
        let arena = arena_for(world, &pieces);
        arena
            .validate()
            .map_err(|_| HexGeometryError::InvalidArena)?;
        let piece_indices = stable_indices(&pieces, |piece| piece.id);
        let collider_indices = stable_indices(&arena.colliders, |collider| collider.id);
        Ok(Self {
            generation: world.generation,
            pieces,
            lights,
            arena,
            ramp_heads,
            blueprint_instances: world.blueprints.len(),
            piece_indices,
            collider_indices,
        })
    }

    #[must_use]
    pub fn rapier_scene(&self) -> RapierTraversalScene {
        RapierTraversalScene::from_arena_spec(&self.arena)
    }

    /// Project only cells named by an accepted facility delta. Stable collider
    /// ID ranges make removals independent of hull counts before and after.
    pub fn project_delta(
        &self,
        world: &HexWfcWorld,
        logical: &HexRelayoutDelta,
        prototypes: &[TilePrototype],
    ) -> Result<HexGeometryDelta, HexGeometryError> {
        self.project_delta_with_rooms(world, logical, prototypes, &[])
    }

    /// Incremental counterpart to [`Self::project_with_rooms`]. If any cell of
    /// a whole-room instance changes, its anchor-owned collider range is
    /// replaced once and the complete footprint is exposed to presentation.
    pub fn project_delta_with_rooms(
        &self,
        world: &HexWfcWorld,
        logical: &HexRelayoutDelta,
        prototypes: &[TilePrototype],
        room_prototypes: &[RoomPrototype],
    ) -> Result<HexGeometryDelta, HexGeometryError> {
        if self.generation != logical.previous_generation || world.generation != logical.generation
        {
            return Err(HexGeometryError::StaleDelta {
                snapshot_generation: self.generation,
                delta_generation: logical.generation,
            });
        }
        validate_id_capacity(world)?;
        let catalogue = Catalogue::new(prototypes);
        let room_catalogue = RoomCatalogue::new(room_prototypes);
        let mut changed_cells = logical.changed_cells.clone();
        for stamped in &world.blueprints {
            if stamped
                .cells
                .iter()
                .any(|cell| changed_cells.contains(cell))
                && room_for(world, stamped, &room_catalogue).is_some()
            {
                changed_cells.extend(stamped.cells.iter().copied());
            }
        }
        let mut upserted_pieces = Vec::new();
        let mut upserted_lights = Vec::new();
        let mut projected_rooms = BTreeSet::new();
        for &coord in &changed_cells {
            if let Some(stamped) = world
                .blueprints
                .iter()
                .find(|blueprint| blueprint.cells.contains(&coord))
                && let Some(room) = room_for(world, stamped, &room_catalogue)
            {
                if projected_rooms.insert(stamped.anchor) {
                    push_room(
                        world,
                        stamped,
                        room,
                        &mut upserted_pieces,
                        &mut upserted_lights,
                    )?;
                }
                continue;
            }
            project_cell(
                world,
                coord,
                &catalogue,
                &mut upserted_pieces,
                &mut upserted_lights,
            )?;
        }
        let mut old_colliders = BTreeMap::new();
        for &coord in &changed_cells {
            for id in collider_ids_for_cell(world, coord) {
                if let Some(&index) = self.piece_indices.get(&id) {
                    let piece = &self.pieces[index];
                    debug_assert_eq!(piece.source_cell, coord);
                    old_colliders.insert(id, piece.collider());
                }
            }
        }
        let removed_piece_ids = old_colliders.keys().copied().collect::<BTreeSet<_>>();
        let new_colliders = upserted_pieces
            .iter()
            .map(|piece| (piece.id, piece.collider()))
            .collect::<BTreeMap<_, _>>();
        // Registers often select byte-identical structural hulls. Presentation
        // still receives every changed-cell piece, while Rapier sees only IDs
        // whose actual collision spec changed.
        let colliders = ColliderDelta {
            removed: old_colliders
                .iter()
                .filter_map(|(&id, old)| (new_colliders.get(&id) != Some(old)).then_some(id))
                .collect(),
            upserted: new_colliders
                .into_iter()
                .filter_map(|(id, new)| (old_colliders.get(&id) != Some(&new)).then_some(new))
                .collect(),
        };
        Ok(HexGeometryDelta {
            previous_generation: logical.previous_generation,
            generation: logical.generation,
            changed_cells,
            removed_piece_ids,
            upserted_pieces,
            upserted_lights,
            colliders,
            ramp_heads: self.ramp_heads.saturating_sub(
                logical
                    .previous_placements
                    .values()
                    .filter(|placement| placement.archetype == HexArchetype::RampHead)
                    .count(),
            ) + logical
                .region
                .cells
                .iter()
                .filter(|coord| world.placements[coord].archetype == HexArchetype::RampHead)
                .count(),
            blueprint_instances: world.blueprints.len(),
        })
    }

    /// Apply a previously projected delta to the pure geometry snapshot.
    pub fn apply_delta(&mut self, delta: &HexGeometryDelta) -> Result<(), HexGeometryError> {
        if self.generation != delta.previous_generation {
            return Err(HexGeometryError::StaleDelta {
                snapshot_generation: self.generation,
                delta_generation: delta.generation,
            });
        }
        apply_indexed_delta(
            &mut self.pieces,
            &mut self.piece_indices,
            &delta.removed_piece_ids,
            &delta.upserted_pieces,
            |piece| piece.id,
        );
        apply_indexed_delta(
            &mut self.arena.colliders,
            &mut self.collider_indices,
            &delta.colliders.removed,
            &delta.colliders.upserted,
            |collider| collider.id,
        );
        self.lights
            .retain(|light| !delta.changed_cells.contains(&light.source_cell));
        self.lights.extend(delta.upserted_lights.iter().cloned());
        sort_lights(&mut self.lights);
        self.generation = delta.generation;
        self.ramp_heads = delta.ramp_heads;
        self.blueprint_instances = delta.blueprint_instances;
        Ok(())
    }
}

fn stable_indices<T>(
    values: &[T],
    id: impl Fn(&T) -> StableColliderId,
) -> BTreeMap<StableColliderId, usize> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| (id(value), index))
        .collect()
}

fn sort_lights(lights: &mut [HexLightSource]) {
    lights.sort_by(|a, b| {
        a.source_cell
            .cmp(&b.source_cell)
            .then(a.tile.cmp(&b.tile))
            .then(a.position.x.total_cmp(&b.position.x))
            .then(a.position.y.total_cmp(&b.position.y))
            .then(a.position.z.total_cmp(&b.position.z))
    });
}

fn apply_indexed_delta<T: Clone>(
    values: &mut Vec<T>,
    indices: &mut BTreeMap<StableColliderId, usize>,
    removed: &BTreeSet<StableColliderId>,
    upserted: &[T],
    id: impl Fn(&T) -> StableColliderId,
) {
    let mut replacements = upserted
        .iter()
        .cloned()
        .map(|value| (id(&value), value))
        .collect::<BTreeMap<_, _>>();
    for removed_id in removed {
        if let Some(replacement) = replacements.remove(removed_id) {
            if let Some(&index) = indices.get(removed_id) {
                values[index] = replacement;
            } else {
                let index = values.len();
                values.push(replacement);
                indices.insert(*removed_id, index);
            }
            continue;
        }
        let Some(index) = indices.remove(removed_id) else {
            continue;
        };
        values.swap_remove(index);
        if index < values.len() {
            indices.insert(id(&values[index]), index);
        }
    }
    for (replacement_id, replacement) in replacements {
        if let Some(&index) = indices.get(&replacement_id) {
            values[index] = replacement;
        } else {
            let index = values.len();
            values.push(replacement);
            indices.insert(replacement_id, index);
        }
    }
}

fn collider_ids_for_cell(
    world: &HexWfcWorld,
    coord: HexCoord,
) -> impl Iterator<Item = StableColliderId> {
    let base = u32::try_from(
        u64::try_from(world.config.grid().index(coord)).expect("validated grid index fits u64")
            * COLLIDER_STRIDE as u64
            + 1,
    )
    .expect("validated collider ID capacity");
    (0..COLLIDER_STRIDE as u32).map(move |offset| StableColliderId(base + offset))
}

fn project_cell(
    world: &HexWfcWorld,
    coord: HexCoord,
    catalogue: &Catalogue<'_>,
    pieces: &mut Vec<HexStructurePiece>,
    lights: &mut Vec<HexLightSource>,
) -> Result<(), HexGeometryError> {
    let placement = world
        .placements
        .get(&coord)
        .ok_or(HexGeometryError::BlueprintCellMissing(coord))?;
    if placement.space == HexSpace::Void || placement.archetype == HexArchetype::RampHead {
        return Ok(());
    }
    if placement.space == HexSpace::Room {
        let stamped = world
            .blueprints
            .iter()
            .find(|blueprint| blueprint.cells.contains(&coord))
            .ok_or(HexGeometryError::BlueprintCellMissing(coord))?;
        let index = stamped
            .cells
            .iter()
            .position(|cell| *cell == coord)
            .expect("blueprint was selected by footprint membership");
        let blueprint = blueprint_for_role(stamped.role);
        let archetype = blueprint_cell_archetype(stamped.role, index).ok_or(
            HexGeometryError::BlueprintArchetypeMissing {
                role: stamped.role,
                cell_index: index,
            },
        )?;
        let signature = blueprint.cell_signature(blueprint.cells[index]);
        let tile = tile_for(world, catalogue, coord, archetype, signature)?;
        return push_tile(
            world,
            stamped.anchor,
            coord,
            HexStructureRole::Room,
            tile,
            pieces,
            lights,
        );
    }
    let role = role_for(placement);
    let Some(archetype) = placement_tile_archetype(placement) else {
        return Ok(());
    };
    let tile = tile_for(world, catalogue, coord, archetype, placement.ports())?;
    push_tile(world, coord, coord, role, tile, pieces, lights)
}

fn validate_id_capacity(world: &HexWfcWorld) -> Result<(), HexGeometryError> {
    let cells = u64::from(world.config.cols)
        * u64::from(world.config.rows)
        * u64::from(world.config.levels);
    let required_max = cells.saturating_mul(COLLIDER_STRIDE as u64);
    if required_max >= u64::from(SHELL_ID_BASE) {
        return Err(HexGeometryError::ColliderIdCapacity {
            cells,
            required_max,
        });
    }
    Ok(())
}

struct Catalogue<'a> {
    by_key: BTreeMap<(&'a str, &'a str, PortSignature), Vec<&'a TilePrototype>>,
}

impl<'a> Catalogue<'a> {
    fn new(prototypes: &'a [TilePrototype]) -> Self {
        let mut by_key: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for prototype in prototypes {
            by_key
                .entry((
                    prototype.key.archetype.as_str(),
                    prototype.key.register.as_str(),
                    prototype.signature,
                ))
                .or_default()
                .push(prototype);
        }
        for candidates in by_key.values_mut() {
            candidates.sort_by(|a, b| a.key.cmp(&b.key));
        }
        Self { by_key }
    }

    fn select(
        &self,
        archetype: &'static str,
        register: &str,
        signature: PortSignature,
        variation: u64,
    ) -> Option<&'a TilePrototype> {
        let exact = self
            .by_key
            .get(&(archetype, register, signature))
            .or_else(|| self.by_key.get(&(archetype, "generic", signature)));
        if let Some(candidates) = exact {
            return weighted_select(candidates, variation, |candidate| candidate.weight);
        }
        None
    }
}

struct RoomCatalogue<'a> {
    by_role: BTreeMap<(String, &'a str), Vec<&'a RoomPrototype>>,
}

impl<'a> RoomCatalogue<'a> {
    fn new(prototypes: &'a [RoomPrototype]) -> Self {
        let mut by_role: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for prototype in prototypes {
            by_role
                .entry((
                    normalized_role(&prototype.room_role),
                    prototype.key.register.as_str(),
                ))
                .or_default()
                .push(prototype);
        }
        for candidates in by_role.values_mut() {
            candidates.sort_by(|a, b| (&a.key, &a.id).cmp(&(&b.key, &b.id)));
        }
        Self { by_role }
    }

    fn select(&self, role: RoomRole, register: &str, variation: u64) -> Option<&'a RoomPrototype> {
        let blueprint = blueprint_for_role(role);
        let role_name = normalized_role(blueprint.name);
        let candidate_list = self
            .by_role
            .get(&(role_name.clone(), register))
            .or_else(|| self.by_role.get(&(role_name, "generic")))?;
        let candidates = candidate_list
            .iter()
            .copied()
            .filter(|candidate| room_contract_matches(candidate, &blueprint))
            .collect::<Vec<_>>();
        weighted_select(&candidates, variation, |candidate| candidate.weight)
    }
}

fn weighted_select<'a, T>(
    candidates: &[&'a T],
    variation: u64,
    weight: impl Fn(&T) -> u16,
) -> Option<&'a T> {
    let total = candidates
        .iter()
        .map(|candidate| u64::from(weight(candidate)))
        .sum::<u64>();
    if total == 0 {
        return None;
    }
    let mut slot = variation % total;
    for &candidate in candidates {
        let candidate_weight = u64::from(weight(candidate));
        if slot < candidate_weight {
            return Some(candidate);
        }
        slot -= candidate_weight;
    }
    None
}

fn normalized_role(role: &str) -> String {
    role.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn cell_ref(offset: (i32, i32, i32)) -> Option<ModuleCellRef> {
    Some(ModuleCellRef {
        q: i16::try_from(offset.0).ok()?,
        r: i16::try_from(offset.1).ok()?,
        level: i8::try_from(offset.2).ok()?,
    })
}

fn room_contract_matches(
    candidate: &RoomPrototype,
    blueprint: &observed_facility::hex_wfc::RoomBlueprint,
) -> bool {
    let expected_cells = blueprint
        .cells
        .iter()
        .copied()
        .map(cell_ref)
        .collect::<Option<BTreeSet<_>>>();
    let candidate_cells = candidate.footprint.iter().copied().collect::<BTreeSet<_>>();
    let Some(expected_cells) = expected_cells else {
        return false;
    };
    if candidate_cells != expected_cells {
        return false;
    }
    let authored_ports = candidate
        .ports
        .iter()
        .map(|port| ((port.cell, port.face), port.class))
        .collect::<BTreeMap<_, _>>();
    for &offset in &blueprint.cells {
        let Some(cell) = cell_ref(offset) else {
            return false;
        };
        let signature = blueprint.cell_signature(offset);
        for face in HexFace::ALL {
            let (dq, dr, dl) = face.delta();
            let neighbor = cell_ref((offset.0 + dq, offset.1 + dr, offset.2 + dl));
            if neighbor.is_some_and(|neighbor| expected_cells.contains(&neighbor)) {
                continue;
            }
            let authored = authored_ports
                .get(&(cell, face))
                .copied()
                .unwrap_or(PortClass::Sealed);
            if authored != signature.port(face) {
                return false;
            }
        }
    }
    blueprint.named_ports.iter().all(|&(name, offset, face)| {
        cell_ref(offset).is_some_and(|cell| {
            candidate.ports.iter().any(|port| {
                port.cell == cell
                    && port.face == face
                    && port.class == PortClass::Door
                    && normalized_role(&port.name) == normalized_role(name)
            })
        })
    })
}

#[cfg(test)]
fn variation_index(key: u64, candidate_count: usize) -> usize {
    (key % candidate_count as u64) as usize
}

fn tile_for<'a>(
    world: &HexWfcWorld,
    catalogue: &Catalogue<'a>,
    coord: HexCoord,
    archetype: &'static str,
    signature: PortSignature,
) -> Result<&'a TilePrototype, HexGeometryError> {
    let register = world
        .architecture
        .get(&coord)
        .ok_or(HexGeometryError::MissingArchitecture(coord))?
        .slug();
    catalogue
        .select(
            archetype,
            register,
            signature,
            world.tile_variation_key(coord),
        )
        .ok_or_else(|| HexGeometryError::MissingTile {
            coord,
            archetype,
            register: register.to_string(),
            signature,
        })
}

fn project_blueprint(
    world: &HexWfcWorld,
    stamped: &StampedBlueprint,
    catalogue: &Catalogue<'_>,
    room_catalogue: &RoomCatalogue<'_>,
    consumed: &mut BTreeSet<HexCoord>,
    pieces: &mut Vec<HexStructurePiece>,
    lights: &mut Vec<HexLightSource>,
) -> Result<(), HexGeometryError> {
    let blueprint = blueprint_for_role(stamped.role);
    if let Some(room) = room_for(world, stamped, room_catalogue) {
        push_room(world, stamped, room, pieces, lights)?;
        consumed.extend(stamped.cells.iter().copied());
        return Ok(());
    }
    for (index, &coord) in stamped.cells.iter().enumerate() {
        world
            .placements
            .get(&coord)
            .ok_or(HexGeometryError::BlueprintCellMissing(coord))?;
        let archetype = blueprint_cell_archetype(stamped.role, index).ok_or(
            HexGeometryError::BlueprintArchetypeMissing {
                role: stamped.role,
                cell_index: index,
            },
        )?;
        // Boundary stamping seals ports that leave the lattice in simulation.
        // Authored room prefabs retain the blueprint's full exterior signature;
        // the arena shell closes those out-of-grid apertures physically.
        let signature = blueprint.cell_signature(blueprint.cells[index]);
        let tile = tile_for(world, catalogue, coord, archetype, signature)?;
        push_tile(
            world,
            stamped.anchor,
            coord,
            HexStructureRole::Room,
            tile,
            pieces,
            lights,
        )?;
        consumed.insert(coord);
    }
    debug_assert_eq!(blueprint.cells.len(), stamped.cells.len());
    Ok(())
}

fn room_for<'a>(
    world: &HexWfcWorld,
    stamped: &StampedBlueprint,
    catalogue: &RoomCatalogue<'a>,
) -> Option<&'a RoomPrototype> {
    let register = world.architecture.get(&stamped.anchor)?.slug();
    catalogue.select(
        stamped.role,
        register,
        world.tile_variation_key(stamped.anchor),
    )
}

fn role_for(placement: &HexPlacement) -> HexStructureRole {
    match placement.archetype {
        HexArchetype::RampUp => HexStructureRole::Ramp,
        HexArchetype::Shaft => HexStructureRole::Shaft,
        _ => HexStructureRole::Hall,
    }
}

fn push_tile(
    world: &HexWfcWorld,
    anchor: HexCoord,
    source_cell: HexCoord,
    role: HexStructureRole,
    tile: &TilePrototype,
    pieces: &mut Vec<HexStructurePiece>,
    lights: &mut Vec<HexLightSource>,
) -> Result<(), HexGeometryError> {
    if tile.hulls.len() > COLLIDER_STRIDE {
        return Err(HexGeometryError::TooManyHulls {
            coord: source_cell,
            hulls: tile.hulls.len(),
        });
    }
    let base = u64::try_from(world.config.grid().index(source_cell))
        .expect("validated grid index fits u64")
        * COLLIDER_STRIDE as u64
        + 1;
    let center = Vec3::from_array(hex_origin(source_cell));
    lights.extend(tile.lights.iter().map(|light| HexLightSource {
        source_cell,
        role,
        tile: tile.key.clone(),
        kind: light.kind,
        position: center + light.position,
    }));
    for (index, hull) in tile.hulls.iter().enumerate() {
        pieces.push(HexStructurePiece {
            id: StableColliderId(
                u32::try_from(base + index as u64).expect("validated collider ID capacity"),
            ),
            anchor,
            source_cell,
            role,
            tile: Some(tile.key.clone()),
            center,
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: ColliderShape::ConvexHull {
                points: hull.clone(),
            },
        });
    }
    Ok(())
}

fn push_room(
    world: &HexWfcWorld,
    stamped: &StampedBlueprint,
    room: &RoomPrototype,
    pieces: &mut Vec<HexStructurePiece>,
    lights: &mut Vec<HexLightSource>,
) -> Result<(), HexGeometryError> {
    if room.hulls.len() > COLLIDER_STRIDE {
        return Err(HexGeometryError::TooManyHulls {
            coord: stamped.anchor,
            hulls: room.hulls.len(),
        });
    }
    let base = u64::try_from(world.config.grid().index(stamped.anchor))
        .expect("validated grid index fits u64")
        * COLLIDER_STRIDE as u64
        + 1;
    let center = Vec3::from_array(hex_origin(stamped.anchor));
    lights.extend(room.lights.iter().map(|light| HexLightSource {
        source_cell: stamped.anchor,
        role: HexStructureRole::Room,
        tile: room.key.clone(),
        kind: light.kind,
        position: center + light.position,
    }));
    for (index, hull) in room.hulls.iter().enumerate() {
        pieces.push(HexStructurePiece {
            id: StableColliderId(
                u32::try_from(base + index as u64).expect("validated collider ID capacity"),
            ),
            anchor: stamped.anchor,
            source_cell: stamped.anchor,
            role: HexStructureRole::Room,
            tile: Some(room.key.clone()),
            center,
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: ColliderShape::ConvexHull {
                points: hull.clone(),
            },
        });
    }
    Ok(())
}

fn push_boundary_shell(world: &HexWfcWorld, pieces: &mut Vec<HexStructurePiece>) {
    let outline = rhombus_outline(world);
    let height = f32::from(world.config.levels) * TILE_LEVEL_HEIGHT;
    let anchor = world.config.spawn();
    for (index, pair) in outline
        .iter()
        .zip(outline.iter().cycle().skip(1))
        .take(outline.len())
        .enumerate()
    {
        let (a, b) = pair;
        let edge = *b - *a;
        let midpoint = (*a + *b) * 0.5;
        let yaw = (-edge.y).atan2(edge.x);
        let rotation = Quat::from_rotation_y(yaw).to_array();
        pieces.push(HexStructurePiece {
            id: StableColliderId(SHELL_ID_BASE + index as u32),
            anchor,
            source_cell: anchor,
            role: HexStructureRole::Boundary,
            tile: None,
            center: Vec3::new(midpoint.x, height * 0.5, midpoint.y),
            rotation,
            shape: ColliderShape::Cuboid {
                half: Vec3::new(edge.length() * 0.5, height * 0.5, SHELL_THICKNESS * 0.5),
            },
        });
    }
}

/// Convex outline of the rhombic axial domain including the canonical hex footprint.
fn rhombus_outline(world: &HexWfcWorld) -> Vec<Vec2> {
    let q = world.config.cols - 1;
    let r = world.config.rows - 1;
    let corners = [
        HexCoord {
            q: 0,
            r: 0,
            level: 0,
        },
        HexCoord { q, r: 0, level: 0 },
        HexCoord { q: 0, r, level: 0 },
        HexCoord { q, r, level: 0 },
    ];
    let mut points = Vec::with_capacity(24);
    for cell in corners {
        let origin = Vec3::from_array(hex_origin(cell));
        points.extend(CORNERS.map(|(x, z)| Vec2::new(origin.x + x as f32, origin.z + z as f32)));
    }
    convex_hull(points)
}

fn convex_hull(mut points: Vec<Vec2>) -> Vec<Vec2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut lower = Vec::new();
    for &point in &points {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], point) <= 0.0
        {
            lower.pop();
        }
        lower.push(point);
    }
    let mut upper = Vec::new();
    for &point in points.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], point) <= 0.0
        {
            upper.pop();
        }
        upper.push(point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn arena_for(world: &HexWfcWorld, pieces: &[HexStructurePiece]) -> ArenaSpec {
    let outline = rhombus_outline(world);
    let min = outline
        .iter()
        .copied()
        .reduce(Vec2::min)
        .unwrap_or(Vec2::ZERO);
    let max = outline
        .iter()
        .copied()
        .reduce(Vec2::max)
        .unwrap_or(Vec2::ZERO);
    let height = f32::from(world.config.levels) * TILE_LEVEL_HEIGHT;
    ArenaSpec {
        colliders: pieces.iter().map(HexStructurePiece::collider).collect(),
        floor_y: 0.0,
        safety_center: Vec3::new((min.x + max.x) * 0.5, height * 0.5, (min.y + max.y) * 0.5),
        safety_half: Vec3::new((max.x - min.x) * 0.55, height, (max.y - min.y) * 0.55),
    }
}

#[cfg(test)]
mod tests;
