//! Inert industrial details on the authored collision cages and conveyor decks.
//! Every entity belongs to its resident cell; rewriting or unloading removes it.
use super::{assets::HexWfcVisualAssets, spectate::Cutaway};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use observed_facility::hex_wfc::ChargeworksPart;
use observed_hex::{HexCoord, HexFace, hex_origin};

mod field;
pub(in crate::hex_wfc) fn install(app: &mut App) {
    field::install(app);
}

#[derive(Component)]
struct ConveyorDeck {
    coord: HexCoord,
    length: f32,
}

/// Quantized lattice turn matches forge/grid_turn, including slanted hex joins.
fn turn(mut p: Vec3, heading: HexFace) -> Vec3 {
    for _ in 0..heading.index() {
        p = Vec3::new(0.5 * p.x - 0.875 * p.z, p.y, 6.0 / 7.0 * p.x + 0.5 * p.z);
    }
    p
}

pub(super) fn spawn(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    coord: HexCoord,
    part: ChargeworksPart,
    heading: HexFace,
) -> usize {
    let origin = Vec3::from_array(hex_origin(coord));
    let mut count = 0;
    let mut decks = Vec::new();
    let mut add =
        |position: Vec3, size: Vec3, bearing: f32, material: usize, name: &'static str| {
            let local = turn(position, heading);
            let turned_axis = turn(Vec3::new(bearing.cos(), 0.0, bearing.sin()), heading);
            let angle = turned_axis.z.atan2(turned_axis.x);
            let entity = commands
                .spawn((
                    Mesh3d(assets.detail_box(meshes, size)),
                    MeshMaterial3d(assets.chargeworks_detail(material)),
                    Transform::from_translation(origin + local)
                        .with_rotation(Quat::from_rotation_y(-angle)),
                    ChildOf(parent),
                    NotShadowCaster,
                    crate::hex_wfc::view::NeverShadowCaster,
                    Cutaway {
                        local,
                        min_y: position.y - size.y / 2.0,
                        max_y: position.y + size.y / 2.0,
                        origin_y: origin.y,
                        cell_level: coord.level,
                        climb_wall: false,
                    },
                    Name::new(name),
                ))
                .id();
            count += 1;
            entity
        };
    let mut conveyor = |from: Vec3, to: Vec3| {
        let delta = to - from;
        let length = delta.length();
        let bearing = delta.z.atan2(delta.x);
        let entity = add(
            (from + to) * 0.5 + Vec3::Y * 0.76,
            Vec3::new(length, 0.012, 1.65),
            bearing,
            0,
            "Static conveyor deck finish",
        );
        decks.push((entity, length));
        let tangent = Vec3::new(-delta.z, 0.0, delta.x).normalize();
        for side in [-1.0, 1.0] {
            add(
                (from + to) * 0.5 + tangent * side * 0.79 + Vec3::Y * 0.775,
                Vec3::new(length, 0.014, 0.07),
                bearing,
                2,
                "Amber conveyor edge",
            );
        }
        // Broad slats and arrow halves carry direction without animated cargo.
        let mut at = 0.55;
        while at < length - 0.3 {
            let p = from + delta * (at / length) + Vec3::Y * 0.777;
            add(p, Vec3::new(0.075, 0.014, 1.4), bearing, 1, "Conveyor slat");
            if (at % 3.0) < 1.0 {
                for side in [-1.0, 1.0] {
                    add(
                        p + tangent * side * 0.19,
                        Vec3::new(0.54, 0.017, 0.08),
                        bearing - side * 0.65,
                        2,
                        "Conveyor direction chevron",
                    );
                }
            }
            at += 1.0;
        }
    };
    use ChargeworksPart::*;
    match part {
        Fabricator => conveyor(Vec3::new(-4.375, 0.0, 0.0), Vec3::new(7.0, 0.0, 0.0)),
        Transfer => {
            conveyor(Vec3::new(3.5, 0.0, 6.0), Vec3::ZERO);
            conveyor(Vec3::ZERO, Vec3::new(7.0, 0.0, 0.0));
        }
        Receiver => {
            conveyor(Vec3::new(3.5, 0.0, 6.0), Vec3::ZERO);
            conveyor(Vec3::ZERO, Vec3::new(-4.375, 0.0, 0.0));
        }
    }
    let pods = match part {
        Fabricator => [(-1.5, 4.0), (2.5, 4.0)],
        Transfer => [(2.75, -3.0), (-2.0, 0.0)],
        Receiver => [(2.0, -3.0), (4.5, -2.0)],
    };
    for (x, z) in pods {
        add(
            Vec3::new(x, 1.68, z),
            Vec3::new(0.82, 1.62, 0.82),
            0.0,
            3,
            "Caged sample charge core (inert)",
        );
        for y in [0.83, 2.56] {
            add(
                Vec3::new(x, y, z),
                Vec3::new(1.15, 0.05, 1.15),
                0.0,
                2,
                "Canister hazard rim",
            );
        }
    }
    if part == Receiver {
        for z in [-1.8, 1.8] {
            add(
                Vec3::new(-5.20, 3.95, z),
                Vec3::new(0.025, 5.9, 0.13),
                0.0,
                3,
                "Receiving vault light slit",
            );
        }
        add(
            Vec3::new(-5.20, 6.9, 0.0),
            Vec3::new(0.025, 0.13, 3.7),
            0.0,
            3,
            "Receiving vault crown light",
        );
    }
    if part == Fabricator {
        for z in [-2.0, 2.0] {
            add(
                Vec3::new(-2.49, 3.6, z),
                Vec3::new(0.025, 4.6, 0.12),
                0.0,
                3,
                "Fabricator status inset (inert)",
            );
        }
    }
    for (entity, length) in decks {
        commands
            .entity(entity)
            .insert(ConveyorDeck { coord, length });
    }
    count
}
