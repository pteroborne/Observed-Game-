use super::*;
use bevy::mesh::VertexAttributeValues;
use observed_authoring::forge::initial_halls::{InitialHallKind, kind_for_key};
use observed_match::hex_wfc::{HexMatchConfig, HexMatchContent, HexWfcMatch};
use std::sync::Arc;

fn physical() -> HexWfcMatch {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/tiles");
    let catalog = observed_authoring::RuntimeHexCatalog::load(
        &root,
        observed_authoring::tile_source::REGISTERS,
    )
    .expect("production content");
    HexWfcMatch::new_with_content(
        1,
        HexMatchConfig {
            guardian: false,
            teams: 1,
            members_per_team: 1,
            wfc: observed_facility::hex_wfc::HexWfcConfig::arc_default(),
        },
        Arc::new(HexMatchContent::from_runtime_catalog(catalog)),
    )
    .expect("match")
}

fn cell(game: &HexWfcMatch, level: u8, kind: InitialHallKind) -> HexCoord {
    game.facility
        .initial_modules
        .keys()
        .copied()
        .find(|&at| {
            at.level == level
                && game.geometry.pieces_in_cell(at).any(|p| {
                    p.tile
                        .as_ref()
                        .is_some_and(|key| kind_for_key(key) == Some(kind))
                })
        })
        .expect("initial module")
}

fn hit(game: &HexWfcMatch, at: HexCoord, from: Vec3, to: Vec3) -> bool {
    hulls(&game.geometry, at).values().any(|hulls| {
        let refs = hulls.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mesh = crate::hex_wfc::view::assets::build_merged_mesh_facing(&refs, None)
            .expect("proxy mesh");
        let VertexAttributeValues::Float32x3(vertices) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).expect("positions")
        else {
            panic!("position format")
        };
        let indices = mesh
            .indices()
            .expect("triangles")
            .iter()
            .collect::<Vec<_>>();
        indices.chunks_exact(3).any(|t| {
            let [a, b, c] = [0, 1, 2].map(|i| Vec3::from_array(vertices[t[i]]));
            let dir = to - from;
            let (e1, e2) = (b - a, c - a);
            let p = dir.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-6 {
                return false;
            }
            let s = from - a;
            let u = s.dot(p) / det;
            let q = s.cross(e1);
            let v = dir.dot(q) / det;
            let distance = e2.dot(q) / det;
            (0.0..=1.0).contains(&u) && v >= 0.0 && u + v <= 1.0 && (0.0..=1.0).contains(&distance)
        })
    })
}

#[test]
fn distant_initial_geometry_keeps_walls_and_doors_and_does_not_cap_terrace_courts() {
    let game = physical();
    let gallery = cell(&game, 4, InitialHallKind::Gallery);
    let placement = game.facility.placements[&gallery];
    let from = Vec3::Y * 2.0;
    for face in observed_hex::HexFace::LATERAL {
        let [(ax, az), (bx, bz)] = observed_hex::face_edge(face);
        let direction = Vec3::new((ax + bx) as f32, 0.0, (az + bz) as f32).normalize();
        assert_eq!(
            hit(&game, gallery, from, from + direction * 10.0),
            !placement.is_open(face),
            "the distant silhouette must preserve the actual doorway on {face:?}"
        );
    }
    let interior = cell(&game, 4, InitialHallKind::Court);
    let terrace = cell(&game, 5, InitialHallKind::Court);
    assert!(hit(&game, interior, Vec3::Y * 7.0, Vec3::Y * 9.0));
    assert!(
        !hit(&game, terrace, Vec3::Y * 7.0, Vec3::Y * 9.0),
        "a generic distant roof must not fill the authored terrace opening"
    );
}
