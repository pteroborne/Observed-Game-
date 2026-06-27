//! Pure projection: LDtk project data -> the game's schematic graph model.
//!
//! The LDtk file owns authoring convenience only. This module is the boundary that
//! turns layers/entities into stable domain IDs and schematic symbols. It has no
//! Bevy `Entity` values, rendering, or asset-server dependency.

use bevy::math::{IVec2, Vec2};
use bevy_ecs_ldtk::ldtk::{EntityInstance, FieldValue, LayerInstance, LdtkJson, Type};
use observed_core::{PortId, RoomId};

use crate::ldtk_source::{
    CODE_CORRIDOR, CODE_DOOR, CODE_OBJECTIVE, CODE_ROOM, CODE_SPAWN, ENTITY_LAYER, PORT_ENTITY,
    ROOM_ENTITY, SCHEMATIC_LAYER,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchematicRoomKind {
    Room,
    Corridor,
}

impl SchematicRoomKind {
    pub fn label(self) -> &'static str {
        match self {
            SchematicRoomKind::Room => "room",
            SchematicRoomKind::Corridor => "corridor",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchematicSymbol {
    Room,
    Corridor,
    DoorThreshold,
    Spawn,
    Objective,
}

impl SchematicSymbol {
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            CODE_ROOM => Some(Self::Room),
            CODE_CORRIDOR => Some(Self::Corridor),
            CODE_DOOR => Some(Self::DoorThreshold),
            CODE_SPAWN => Some(Self::Spawn),
            CODE_OBJECTIVE => Some(Self::Objective),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SchematicSymbol::Room => "room fill",
            SchematicSymbol::Corridor => "corridor",
            SchematicSymbol::DoorThreshold => "door threshold",
            SchematicSymbol::Spawn => "spawn",
            SchematicSymbol::Objective => "objective",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicCell {
    pub grid: IVec2,
    pub symbol: SchematicSymbol,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchematicRoom {
    pub id: RoomId,
    pub label: String,
    pub kind: SchematicRoomKind,
    pub grid_min: IVec2,
    pub grid_size: IVec2,
    pub px_min: Vec2,
    pub px_size: Vec2,
}

impl SchematicRoom {
    pub fn center_px(&self) -> Vec2 {
        self.px_min + self.px_size * 0.5
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchematicPort {
    pub id: PortId,
    pub a: RoomId,
    pub b: RoomId,
    pub grid: IVec2,
    pub pos_px: Vec2,
    pub socket: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Schematic {
    pub width_cells: i32,
    pub height_cells: i32,
    pub cell_size: i32,
    pub level_px_size: Vec2,
    pub rooms: Vec<SchematicRoom>,
    pub ports: Vec<SchematicPort>,
    pub cells: Vec<SchematicCell>,
}

impl Schematic {
    pub fn room(&self, id: RoomId) -> Option<&SchematicRoom> {
        self.rooms.iter().find(|room| room.id == id)
    }

    pub fn graph_signature(&self) -> Vec<(PortId, RoomId, RoomId)> {
        self.ports
            .iter()
            .map(|port| (port.id, port.a, port.b))
            .collect()
    }

    pub fn cells_with(&self, symbol: SchematicSymbol) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.symbol == symbol)
            .count()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectError {
    MissingLevel,
    MissingLayers,
    MissingLayer(&'static str),
    WrongLayerType(&'static str),
    InvalidGrid,
    MissingField { entity: String, field: &'static str },
    InvalidRoomKind(String),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::MissingLevel => write!(f, "LDtk project has no levels"),
            ProjectError::MissingLayers => write!(f, "LDtk level has no layer instances"),
            ProjectError::MissingLayer(layer) => write!(f, "missing LDtk layer {layer}"),
            ProjectError::WrongLayerType(layer) => write!(f, "LDtk layer {layer} has wrong type"),
            ProjectError::InvalidGrid => write!(f, "LDtk IntGrid dimensions do not match CSV"),
            ProjectError::MissingField { entity, field } => {
                write!(f, "LDtk entity {entity} is missing field {field}")
            }
            ProjectError::InvalidRoomKind(kind) => write!(f, "invalid room kind {kind}"),
        }
    }
}

impl std::error::Error for ProjectError {}

fn field_int(entity: &EntityInstance, field: &'static str) -> Result<i32, ProjectError> {
    entity
        .field_instances
        .iter()
        .find(|value| value.identifier == field)
        .and_then(|value| match &value.value {
            FieldValue::Int(Some(value)) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| ProjectError::MissingField {
            entity: entity.iid.clone(),
            field,
        })
}

fn field_string(entity: &EntityInstance, field: &'static str) -> Result<String, ProjectError> {
    entity
        .field_instances
        .iter()
        .find(|value| value.identifier == field)
        .and_then(|value| match &value.value {
            FieldValue::String(Some(value)) => Some(value.clone()),
            _ => None,
        })
        .ok_or_else(|| ProjectError::MissingField {
            entity: entity.iid.clone(),
            field,
        })
}

fn layer<'a>(
    layers: &'a [LayerInstance],
    identifier: &'static str,
) -> Result<&'a LayerInstance, ProjectError> {
    layers
        .iter()
        .find(|layer| layer.identifier == identifier)
        .ok_or(ProjectError::MissingLayer(identifier))
}

fn entity_center_px(entity: &EntityInstance) -> Vec2 {
    Vec2::new(
        entity.px.x as f32 + entity.width as f32 * 0.5,
        entity.px.y as f32 + entity.height as f32 * 0.5,
    )
}

fn project_room(entity: &EntityInstance, cell_size: i32) -> Result<SchematicRoom, ProjectError> {
    let kind = match field_string(entity, "kind")?.as_str() {
        "room" => SchematicRoomKind::Room,
        "corridor" => SchematicRoomKind::Corridor,
        other => return Err(ProjectError::InvalidRoomKind(other.to_string())),
    };
    Ok(SchematicRoom {
        id: RoomId(field_int(entity, "id")? as u32),
        label: field_string(entity, "label")?,
        kind,
        grid_min: entity.grid,
        grid_size: IVec2::new(entity.width / cell_size, entity.height / cell_size),
        px_min: Vec2::new(entity.px.x as f32, entity.px.y as f32),
        px_size: Vec2::new(entity.width as f32, entity.height as f32),
    })
}

fn project_port(entity: &EntityInstance) -> Result<SchematicPort, ProjectError> {
    Ok(SchematicPort {
        id: PortId(field_int(entity, "id")? as u32),
        a: RoomId(field_int(entity, "room_a")? as u32),
        b: RoomId(field_int(entity, "room_b")? as u32),
        grid: entity.grid,
        pos_px: entity_center_px(entity),
        socket: field_string(entity, "socket")?,
    })
}

pub fn project(ldtk: &LdtkJson) -> Result<Schematic, ProjectError> {
    let level = ldtk.levels.first().ok_or(ProjectError::MissingLevel)?;
    let layers = level
        .layer_instances
        .as_ref()
        .ok_or(ProjectError::MissingLayers)?;

    let cell_layer = layer(layers, SCHEMATIC_LAYER)?;
    if cell_layer.layer_instance_type != Type::IntGrid {
        return Err(ProjectError::WrongLayerType(SCHEMATIC_LAYER));
    }
    let entity_layer = layer(layers, ENTITY_LAYER)?;
    if entity_layer.layer_instance_type != Type::Entities {
        return Err(ProjectError::WrongLayerType(ENTITY_LAYER));
    }

    if cell_layer.grid_size <= 0
        || cell_layer.int_grid_csv.len() != (cell_layer.c_wid * cell_layer.c_hei) as usize
    {
        return Err(ProjectError::InvalidGrid);
    }

    let mut cells = Vec::new();
    for (i, code) in cell_layer.int_grid_csv.iter().copied().enumerate() {
        if let Some(symbol) = SchematicSymbol::from_code(code) {
            let x = i as i32 % cell_layer.c_wid;
            let y = i as i32 / cell_layer.c_wid;
            cells.push(SchematicCell {
                grid: IVec2::new(x, y),
                symbol,
            });
        }
    }

    let mut rooms = Vec::new();
    let mut ports = Vec::new();
    for entity in &entity_layer.entity_instances {
        match entity.identifier.as_str() {
            ROOM_ENTITY => rooms.push(project_room(entity, cell_layer.grid_size)?),
            PORT_ENTITY => ports.push(project_port(entity)?),
            _ => {}
        }
    }

    rooms.sort_by_key(|room| room.id);
    ports.sort_by_key(|port| port.id);

    Ok(Schematic {
        width_cells: cell_layer.c_wid,
        height_cells: cell_layer.c_hei,
        cell_size: cell_layer.grid_size,
        level_px_size: Vec2::new(level.px_wid as f32, level.px_hei as f32),
        rooms,
        ports,
        cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldtk_source;

    fn schematic() -> Schematic {
        project(&ldtk_source::parse_ldtk_json()).expect("LDtk schematic projects")
    }

    #[test]
    fn round_trips_the_same_topology_expected_from_the_3d_authoring_lab() {
        let schematic = schematic();
        assert_eq!(schematic.rooms.len(), 3, "two rooms plus one corridor");
        assert_eq!(
            schematic.room(RoomId(1)).unwrap().kind,
            SchematicRoomKind::Room
        );
        assert_eq!(
            schematic.room(RoomId(2)).unwrap().kind,
            SchematicRoomKind::Corridor
        );
        assert_eq!(
            schematic.room(RoomId(3)).unwrap().kind,
            SchematicRoomKind::Room
        );
        assert_eq!(
            schematic.graph_signature(),
            vec![
                (PortId(1), RoomId(1), RoomId(2)),
                (PortId(2), RoomId(2), RoomId(3)),
            ]
        );
    }

    #[test]
    fn entities_own_domain_ids_but_not_game_identity() {
        let schematic = schematic();
        let labels: Vec<_> = schematic
            .rooms
            .iter()
            .map(|room| room.label.as_str())
            .collect();
        assert_eq!(labels, vec!["Room A", "Main corridor", "Room B"]);
        assert_eq!(schematic.ports[0].socket, "door");
        assert_eq!(schematic.ports[1].socket, "door");
    }

    #[test]
    fn int_grid_becomes_tactical_symbols() {
        let schematic = schematic();
        assert_eq!(schematic.cells_with(SchematicSymbol::DoorThreshold), 2);
        assert_eq!(schematic.cells_with(SchematicSymbol::Spawn), 1);
        assert_eq!(schematic.cells_with(SchematicSymbol::Objective), 1);
        assert_eq!(schematic.cells_with(SchematicSymbol::Corridor), 2);
    }
}
