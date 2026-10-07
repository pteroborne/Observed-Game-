//! One-way enclosure roofs for Observers. Rays, platforms and Guardian movement
//! still use the complete scene. The immutable spatial index follows generation.
use std::collections::{BTreeMap, BTreeSet};

use glam::{Quat, Vec3};
use observed_authoring::HullSurface;
use observed_core::PlayerId;
use observed_traversal::{ColliderShape, FpsBody, StableColliderId};

use crate::hex_wfc::{HexStructurePiece, HexWfcGeometrySnapshot};

impl super::HexWfcMatch {
    /// Read-only evidence pose on an enclosure roof above a clear interior point.
    pub fn roof_landing_point(&self, cell: observed_hex::HexCoord) -> Option<Vec3> {
        let floor = self.standing_point(cell)?;
        self.geometry
            .pieces
            .iter()
            .filter(|p| p.source_cell == cell)
            .filter_map(roof)
            .filter(|r| {
                floor.x >= r.min.x && floor.x <= r.max.x && floor.z >= r.min.z && floor.z <= r.max.z
            })
            .max_by(|a, b| a.max.y.total_cmp(&b.max.y))
            .map(|r| Vec3::new(floor.x, r.max.y, floor.z))
    }
}

#[derive(Clone, Debug)]
struct Roof {
    id: StableColliderId,
    min: Vec3,
    max: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex_wfc::{HexPiecePart, HexStructureRole};
    use observed_hex::HexCoord;
    use observed_traversal::{
        ArenaSpec, ColliderSpec, FpsConfig, RapierKinematicSettings,
        rapier_controller::{RapierTraversalScene, step_character_with_filter},
    };
    use player_input::PlayerIntent;

    fn cap(bottom: f32, surface: HullSurface) -> HexStructurePiece {
        let at = HexCoord {
            q: 0,
            r: 0,
            level: 0,
        };
        HexStructurePiece {
            id: StableColliderId(42),
            anchor: at,
            source_cell: at,
            role: HexStructureRole::Hall,
            part: HexPiecePart::Authored,
            surface: Some(surface),
            tile: None,
            center: Vec3::new(0.0, bottom + 0.1, 0.0),
            rotation: Quat::IDENTITY.to_array(),
            shape: ColliderShape::Cuboid {
                half: Vec3::new(6.0, 0.1, 6.0),
            },
        }
    }

    #[test]
    fn low_and_tall_roofs_pass_descenders_but_stop_an_upward_capsule() {
        let config = FpsConfig::deliberate_rapier();
        for bottom in [3.5, 7.5] {
            let piece = cap(bottom, HullSurface::Ceiling);
            let scene = RapierTraversalScene::from_arena_spec(&ArenaSpec {
                floor_y: 0.5,
                safety_center: Vec3::new(0.0, 5.0, 0.0),
                safety_half: Vec3::new(20.0, 30.0, 20.0),
                colliders: vec![
                    ColliderSpec {
                        id: StableColliderId(1),
                        center: Vec3::new(0.0, 0.4, 0.0),
                        rotation: Quat::IDENTITY.to_array(),
                        shape: ColliderShape::Cuboid {
                            half: Vec3::new(10.0, 0.1, 10.0),
                        },
                        friction: 0.9,
                    },
                    ColliderSpec {
                        id: piece.id,
                        center: piece.center,
                        rotation: piece.rotation,
                        shape: piece.shape.clone(),
                        friction: 0.9,
                    },
                ],
            });
            let mut passages = RoofPassages::default();
            passages.refresh(0, &[piece]);
            let mut falling =
                FpsBody::spawned(Vec3::new(0.0, bottom + 1.5 + config.half_height, 0.0), 0.0);
            let mut rising =
                FpsBody::spawned(Vec3::new(2.0, bottom - config.half_height - 0.2, 0.0), 0.0);
            rising.velocity.y = 8.0;
            let mut maximum_head = rising.position.y + config.half_height;
            for _ in 0..240 {
                for (id, body) in [(PlayerId(0), &mut falling), (PlayerId(1), &mut rising)] {
                    let ignored = passages.for_body(id, body, config.half_height, config.radius);
                    step_character_with_filter(
                        &scene,
                        body,
                        PlayerIntent::default(),
                        &config,
                        RapierKinematicSettings::shipped(&config),
                        1.0 / 60.0,
                        &|id| !ignored.contains(&id),
                    );
                }
                maximum_head = maximum_head.max(rising.position.y + config.half_height);
            }
            assert!(
                (falling.position.y - config.half_height - 0.5).abs() < 0.1,
                "falling body: {falling:?}"
            );
            assert!(
                maximum_head <= bottom + 0.08,
                "upward body crossed roof: {maximum_head}"
            );
            assert!(
                scene
                    .ray_distance(Vec3::new(0.0, bottom - 1.0, 0.0), Vec3::Y, 2.0)
                    .is_some(),
                "movement filtering must not alter structural rays"
            );
        }
    }

    #[test]
    fn authored_platforms_are_never_roof_passages() {
        assert!(roof(&cap(7.5, HullSurface::Floor)).is_none());
        assert!(roof(&cap(7.5, HullSurface::Wall)).is_none());
        let mut platform = cap(7.5, HullSurface::Floor);
        platform.surface = None;
        platform.role = HexStructureRole::Climb;
        assert!(roof(&platform).is_none());
        platform.role = HexStructureRole::Room;
        assert!(
            roof(&platform).is_none(),
            "untagged elevated galleries are not enclosure roofs"
        );
    }
}

fn roof(piece: &HexStructurePiece) -> Option<Roof> {
    if piece.surface != Some(HullSurface::Ceiling) {
        return None;
    }
    let rotation = Quat::from_array(piece.rotation);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    match &piece.shape {
        ColliderShape::ConvexHull { points } => {
            for point in points {
                let p = piece.center + rotation * *point;
                min = min.min(p);
                max = max.max(p);
            }
        }
        ColliderShape::Cuboid { half } => {
            for x in [-half.x, half.x] {
                for y in [-half.y, half.y] {
                    for z in [-half.z, half.z] {
                        let p = piece.center + rotation * Vec3::new(x, y, z);
                        min = min.min(p);
                        max = max.max(p);
                    }
                }
            }
        }
    }
    Some(Roof {
        id: piece.id,
        min,
        max,
    })
}

#[derive(Clone, Debug, Default)]
pub(super) struct RoofPassages {
    generation: Option<u32>,
    bins: BTreeMap<(i32, i32), Vec<Roof>>,
    passing: BTreeMap<PlayerId, BTreeMap<StableColliderId, f32>>,
}

impl RoofPassages {
    pub(super) fn ignored(
        &mut self,
        geometry: &HexWfcGeometrySnapshot,
        player: PlayerId,
        body: &FpsBody,
        half_height: f32,
        radius: f32,
    ) -> BTreeSet<StableColliderId> {
        self.refresh(geometry.generation, &geometry.pieces);
        self.for_body(player, body, half_height, radius)
    }

    fn refresh(&mut self, generation: u32, pieces: &[HexStructurePiece]) {
        if self.generation != Some(generation) {
            self.bins.clear();

            for roof in pieces.iter().filter_map(roof) {
                for x in ((roof.min.x / 16.0).floor() as i32)..=((roof.max.x / 16.0).floor() as i32)
                {
                    for z in
                        ((roof.min.z / 16.0).floor() as i32)..=((roof.max.z / 16.0).floor() as i32)
                    {
                        self.bins.entry((x, z)).or_default().push(roof.clone());
                    }
                }
            }
            let current: BTreeMap<_, _> = self
                .bins
                .values()
                .flatten()
                .map(|roof| (roof.id, roof.min.y))
                .collect();
            for passing in self.passing.values_mut() {
                passing.retain(|id, bottom| {
                    current.get(id).is_some_and(|height| {
                        *bottom = *height;
                        true
                    })
                });
            }
            self.generation = Some(generation);
        }
    }

    fn for_body(
        &mut self,
        player: PlayerId,
        body: &FpsBody,
        half_height: f32,
        radius: f32,
    ) -> BTreeSet<StableColliderId> {
        let passing = self.passing.entry(player).or_default();
        passing.retain(|_, bottom| body.position.y + half_height >= *bottom - 0.05);
        if body.velocity.y <= 0.0 {
            for x in (((body.position.x - radius) / 16.0).floor() as i32)
                ..=(((body.position.x + radius) / 16.0).floor() as i32)
            {
                for z in (((body.position.z - radius) / 16.0).floor() as i32)
                    ..=(((body.position.z + radius) / 16.0).floor() as i32)
                {
                    for roof in self.bins.get(&(x, z)).into_iter().flatten() {
                        if body.position.y - half_height >= roof.max.y - 0.08
                            && body.position.x + radius >= roof.min.x
                            && body.position.x - radius <= roof.max.x
                            && body.position.z + radius >= roof.min.z
                            && body.position.z - radius <= roof.max.z
                        {
                            passing.insert(roof.id, roof.min.y);
                        }
                    }
                }
            }
        }
        passing.keys().copied().collect()
    }
}
