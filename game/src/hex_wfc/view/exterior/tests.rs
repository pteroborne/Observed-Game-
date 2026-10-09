use std::collections::BTreeMap;

use observed_facility::hex_wfc::profile::SpaceMix;
use observed_facility::hex_wfc::{
    HexArchetype, HexPlacement, HexSpace, HexWfcConfig, PortClass, lateral_bit,
};

use super::*;

fn world(cells: &[(HexCoord, HexArchetype, u8)]) -> HexWfcWorld {
    let placements = cells
        .iter()
        .map(|&(coord, archetype, doors)| {
            (
                coord,
                HexPlacement {
                    low_doors: 0,
                    coord,
                    space: HexSpace::Hall,
                    archetype,
                    doors,
                    up: PortClass::Sealed,
                    down: PortClass::Sealed,
                },
            )
        })
        .collect();
    HexWfcWorld {
        seed: 1,
        generation: 0,
        config: HexWfcConfig {
            cols: 8,
            rows: 8,
            levels: 4,
            min_rooms: 0,
            max_rooms: 0,
            retry_budget: 1,
            min_room_distance: 1,
        },
        placements,
        blueprints: Vec::new(),
        architecture: BTreeMap::new(),
        cell_revisions: BTreeMap::new(),
        initial_modules: Default::default(),
        last_attempts: 1,
        authored_pins: Default::default(),
        space_mix: SpaceMix::baseline(),
        route_corridors: false,
        carve_unrouted: false,
        open_air: false,
        sealed: false,
    }
}

fn at(q: u16, r: u16, level: u8) -> HexCoord {
    HexCoord { q, r, level }
}

#[test]
fn an_open_hall_shows_bands_and_a_lip_where_a_walled_one_shows_a_face() {
    let corner = HexArchetype::Corner;
    let doors = lateral_bit(HexFace::East) | lateral_bit(HexFace::SouthWest);
    let lone = world(&[(at(3, 3, 1), corner, doors)]);
    let skin = cell_skin(&lone, at(3, 3, 1)).expect("built");
    // Four open faces: two bands each, and a lip each; the two door faces have
    // nothing beyond them in this world, but a door is never an open edge.
    assert_eq!(skin.walls.indices.len() / 6, 2 * 4 + 2);
    assert_eq!(skin.lips.indices.len() / 6, 4);
    assert!(!skin.caps.is_empty(), "sky above: a roof");
    assert!(!skin.keel.is_empty(), "nothing below: a keel");
}

#[test]
fn a_buried_cell_has_no_skin_to_draw() {
    let hall = HexArchetype::Expanse;
    let mut cells = vec![(at(3, 3, 1), hall, 0)];
    for face in HexFace::ALL {
        let (dq, dr, dl) = face.delta();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        cells.push((
            at((3 + dq) as u16, (3 + dr) as u16, (1 + dl) as u8),
            hall,
            0,
        ));
    }
    let buried = world(&cells);
    let skin = cell_skin(&buried, at(3, 3, 1)).expect("built");
    assert!(skin.walls.is_empty() && skin.caps.is_empty());
    assert!(skin.lips.is_empty() && skin.keel.is_empty());
}

#[test]
fn every_polygon_faces_outward() {
    let lone = world(&[(
        at(3, 3, 1),
        HexArchetype::Corner,
        lateral_bit(HexFace::East),
    )]);
    let skin = cell_skin(&lone, at(3, 3, 1)).expect("built");
    for data in [&skin.walls, &skin.caps, &skin.lips, &skin.keel] {
        for triangle in data.indices.chunks(3) {
            let [a, b, c] =
                [0, 1, 2].map(|k| Vec3::from_array(data.positions[triangle[k] as usize]));
            let normal = Vec3::from_array(data.normals[triangle[0] as usize]);
            let facing = (b - a).cross(c - a);
            assert!(facing.dot(normal) >= -1e-4, "a back-facing triangle");
        }
    }
}
#[test]
fn detailed_rooms_replace_exterior_proxies_for_the_entire_footprint() {
    use super::super::{
        ResidentCell,
        shell::{CellGeometryIndex, HexGeometryCatalog},
    };
    let anchor = at(0, 0, 0);
    let footprint = vec![anchor, at(1, 0, 0), at(0, 0, 1)];
    let catalog = HexGeometryCatalog {
        generation: 0,
        cells: BTreeMap::from([(
            anchor,
            CellGeometryIndex {
                footprint: footprint.clone(),
                piece_ids: Vec::new(),
                lights: Vec::new(),
            },
        )]),
        boundary_piece_ids: Vec::new(),
    };
    let mut resident = BTreeMap::from([(
        anchor,
        ResidentCell {
            shown: true,
            entity: Entity::PLACEHOLDER,
            child_pieces: 1,
        },
    )]);
    assert_eq!(
        detailed_coverage(&catalog, &resident),
        footprint.into_iter().collect()
    );
    resident.get_mut(&anchor).unwrap().shown = false;
    assert!(detailed_coverage(&catalog, &resident).is_empty());
}
