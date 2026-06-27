//! Typed source for the authored LDtk schematic.
//!
//! The generated JSON is a real LDtk project: one level, one IntGrid symbol
//! layer, and one entity layer. Keeping the source typed lets tests parse the
//! exact artifact the runtime writes without maintaining a large hand-edited JSON
//! blob.

use bevy::{math::IVec2, prelude::*};
use bevy_ecs_ldtk::ldtk::{
    Definitions, EntityDefinition, EntityInstance, FieldInstance, FieldValue,
    IntGridValueDefinition, LayerDefinition, LayerInstance, LdtkJson, Type, WorldLayout,
};

pub const CELL_SIZE: i32 = 32;
pub const GRID_W: i32 = 12;
pub const GRID_H: i32 = 5;
pub const LEVEL_PX_W: i32 = GRID_W * CELL_SIZE;
pub const LEVEL_PX_H: i32 = GRID_H * CELL_SIZE;

pub const SCHEMATIC_LAYER: &str = "SchematicCells";
pub const ENTITY_LAYER: &str = "SchematicEntities";

pub const ROOM_ENTITY: &str = "Room";
pub const PORT_ENTITY: &str = "Port";

pub const CODE_EMPTY: i32 = 0;
pub const CODE_ROOM: i32 = 1;
pub const CODE_CORRIDOR: i32 = 2;
pub const CODE_DOOR: i32 = 3;
pub const CODE_SPAWN: i32 = 4;
pub const CODE_OBJECTIVE: i32 = 5;

fn field_int(identifier: &str, value: i32, def_uid: i32) -> FieldInstance {
    FieldInstance {
        identifier: identifier.to_string(),
        tile: None,
        field_instance_type: "Int".to_string(),
        value: FieldValue::Int(Some(value)),
        def_uid,
        real_editor_values: Vec::new(),
    }
}

fn field_string(identifier: &str, value: &str, def_uid: i32) -> FieldInstance {
    FieldInstance {
        identifier: identifier.to_string(),
        tile: None,
        field_instance_type: "String".to_string(),
        value: FieldValue::String(Some(value.to_string())),
        def_uid,
        real_editor_values: Vec::new(),
    }
}

fn entity(
    identifier: &str,
    iid: &str,
    def_uid: i32,
    grid: IVec2,
    size_cells: IVec2,
    fields: Vec<FieldInstance>,
    color: Color,
) -> EntityInstance {
    EntityInstance {
        identifier: identifier.to_string(),
        def_uid,
        iid: iid.to_string(),
        grid,
        px: grid * CELL_SIZE,
        width: size_cells.x * CELL_SIZE,
        height: size_cells.y * CELL_SIZE,
        pivot: Vec2::ZERO,
        smart_color: color,
        field_instances: fields,
        ..Default::default()
    }
}

fn room_entity(
    iid: &str,
    id: i32,
    label: &str,
    kind: &str,
    grid: IVec2,
    size: IVec2,
) -> EntityInstance {
    entity(
        ROOM_ENTITY,
        iid,
        1,
        grid,
        size,
        vec![
            field_int("id", id, 1),
            field_string("label", label, 2),
            field_string("kind", kind, 3),
        ],
        if kind == "corridor" {
            Color::srgb(1.0, 0.78, 0.3)
        } else {
            Color::srgb(0.2, 0.55, 1.0)
        },
    )
}

fn port_entity(iid: &str, id: i32, a: i32, b: i32, grid: IVec2) -> EntityInstance {
    entity(
        PORT_ENTITY,
        iid,
        2,
        grid,
        IVec2::ONE,
        vec![
            field_int("id", id, 4),
            field_int("room_a", a, 5),
            field_int("room_b", b, 6),
            field_string("socket", "door", 7),
        ],
        Color::srgb(0.6, 0.3, 1.0),
    )
}

fn int_grid_csv() -> Vec<i32> {
    let mut cells = vec![CODE_EMPTY; (GRID_W * GRID_H) as usize];
    let mut set = |x: i32, y: i32, value: i32| {
        cells[(y * GRID_W + x) as usize] = value;
    };

    for y in 0..GRID_H {
        for x in 0..4 {
            set(x, y, CODE_ROOM);
        }
        for x in 8..12 {
            set(x, y, CODE_ROOM);
        }
    }

    set(1, 2, CODE_SPAWN);
    set(10, 2, CODE_OBJECTIVE);
    set(4, 2, CODE_DOOR);
    set(5, 2, CODE_CORRIDOR);
    set(6, 2, CODE_CORRIDOR);
    set(7, 2, CODE_DOOR);

    cells
}

fn entity_layer(entities: Vec<EntityInstance>) -> LayerInstance {
    LayerInstance {
        identifier: ENTITY_LAYER.to_string(),
        iid: "layer-entities".to_string(),
        layer_instance_type: Type::Entities,
        layer_def_uid: 2,
        level_id: 1,
        c_wid: GRID_W,
        c_hei: GRID_H,
        grid_size: CELL_SIZE,
        entity_instances: entities,
        visible: true,
        ..Default::default()
    }
}

fn int_grid_layer() -> LayerInstance {
    LayerInstance {
        identifier: SCHEMATIC_LAYER.to_string(),
        iid: "layer-cells".to_string(),
        layer_instance_type: Type::IntGrid,
        layer_def_uid: 1,
        level_id: 1,
        c_wid: GRID_W,
        c_hei: GRID_H,
        grid_size: CELL_SIZE,
        int_grid_csv: int_grid_csv(),
        visible: true,
        ..Default::default()
    }
}

fn layer_definition(identifier: &str, uid: i32, kind: Type) -> LayerDefinition {
    LayerDefinition {
        identifier: identifier.to_string(),
        uid,
        layer_definition_type: match kind {
            Type::IntGrid => "IntGrid",
            Type::Entities => "Entities",
            Type::Tiles => "Tiles",
            Type::AutoLayer => "AutoLayer",
        }
        .to_string(),
        purple_type: kind,
        grid_size: CELL_SIZE,
        guide_grid_wid: CELL_SIZE,
        guide_grid_hei: CELL_SIZE,
        display_opacity: 1.0,
        inactive_opacity: 1.0,
        render_in_world_view: true,
        int_grid_values: if kind == Type::IntGrid {
            vec![
                int_grid_value(CODE_ROOM, "Room", Color::srgb(0.2, 0.55, 1.0)),
                int_grid_value(CODE_CORRIDOR, "Corridor", Color::srgb(1.0, 0.78, 0.3)),
                int_grid_value(CODE_DOOR, "DoorThreshold", Color::srgb(0.6, 0.3, 1.0)),
                int_grid_value(CODE_SPAWN, "Spawn", Color::srgb(0.6, 0.95, 1.0)),
                int_grid_value(CODE_OBJECTIVE, "Objective", Color::srgb(0.2, 1.0, 0.4)),
            ]
        } else {
            Vec::new()
        },
        ..Default::default()
    }
}

fn int_grid_value(value: i32, name: &str, color: Color) -> IntGridValueDefinition {
    IntGridValueDefinition {
        value,
        identifier: Some(name.to_string()),
        color,
        ..Default::default()
    }
}

fn entity_definition(identifier: &str, uid: i32, color: Color) -> EntityDefinition {
    EntityDefinition {
        identifier: identifier.to_string(),
        uid,
        width: CELL_SIZE,
        height: CELL_SIZE,
        color,
        fill_opacity: 0.35,
        line_opacity: 1.0,
        ..Default::default()
    }
}

pub fn ldtk_project() -> LdtkJson {
    let entities = vec![
        room_entity(
            "room-a",
            1,
            "Room A",
            "room",
            IVec2::new(0, 0),
            IVec2::new(4, 5),
        ),
        room_entity(
            "corridor",
            2,
            "Main corridor",
            "corridor",
            IVec2::new(4, 2),
            IVec2::new(4, 1),
        ),
        room_entity(
            "room-b",
            3,
            "Room B",
            "room",
            IVec2::new(8, 0),
            IVec2::new(4, 5),
        ),
        port_entity("port-a-corridor", 1, 1, 2, IVec2::new(4, 2)),
        port_entity("port-corridor-b", 2, 2, 3, IVec2::new(7, 2)),
    ];

    let level = bevy_ecs_ldtk::ldtk::Level {
        identifier: "TwoRoomSchematic".to_string(),
        iid: "level-two-room-schematic".to_string(),
        uid: 1,
        px_wid: LEVEL_PX_W,
        px_hei: LEVEL_PX_H,
        world_x: 0,
        world_y: 0,
        use_auto_identifier: false,
        layer_instances: Some(vec![entity_layer(entities), int_grid_layer()]),
        ..Default::default()
    };

    LdtkJson {
        app_build_id: 1.5,
        default_entity_width: CELL_SIZE,
        default_entity_height: CELL_SIZE,
        default_grid_size: CELL_SIZE,
        default_level_width: Some(LEVEL_PX_W),
        default_level_height: Some(LEVEL_PX_H),
        defs: Definitions {
            entities: vec![
                entity_definition(ROOM_ENTITY, 1, Color::srgb(0.2, 0.55, 1.0)),
                entity_definition(PORT_ENTITY, 2, Color::srgb(0.6, 0.3, 1.0)),
            ],
            layers: vec![
                layer_definition(SCHEMATIC_LAYER, 1, Type::IntGrid),
                layer_definition(ENTITY_LAYER, 2, Type::Entities),
            ],
            ..Default::default()
        },
        dummy_world_iid: "world-schematic".to_string(),
        external_levels: false,
        iid: "observed2-ldtk-schematic".to_string(),
        json_version: "1.5.3".to_string(),
        level_name_pattern: "Level_%idx".to_string(),
        levels: vec![level],
        world_grid_width: Some(LEVEL_PX_W),
        world_grid_height: Some(LEVEL_PX_H),
        world_layout: Some(WorldLayout::Free),
        ..Default::default()
    }
}

pub fn ldtk_json() -> String {
    serde_json::to_string_pretty(&ldtk_project()).expect("typed LDtk source serializes")
}

pub fn parse_ldtk_json() -> LdtkJson {
    serde_json::from_str(&ldtk_json()).expect("typed LDtk source parses through bevy_ecs_ldtk")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_project_is_valid_for_bevy_ecs_ldtk_schema() {
        let parsed = parse_ldtk_json();
        assert_eq!(parsed.levels.len(), 1);
        let level = &parsed.levels[0];
        assert_eq!(level.px_wid, LEVEL_PX_W);
        assert_eq!(level.px_hei, LEVEL_PX_H);
        assert_eq!(level.layer_instances.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn int_grid_captures_schematic_symbols() {
        let cells = int_grid_csv();
        assert_eq!(cells.len(), (GRID_W * GRID_H) as usize);
        assert_eq!(cells.iter().filter(|v| **v == CODE_DOOR).count(), 2);
        assert_eq!(cells.iter().filter(|v| **v == CODE_SPAWN).count(), 1);
        assert_eq!(cells.iter().filter(|v| **v == CODE_OBJECTIVE).count(), 1);
    }
}
