//! Immutable committed content loaded before any match state exists.

use std::{collections::BTreeMap, sync::Arc};

use bevy::prelude::Resource;
use observed_content::{
    BakedModule, ContentManifest, ModulePlacement, PlaceLayoutSnapshot, PresentationContentHash,
    SimulationContentHash, TraversalProfile,
};
use observed_traversal::{ArenaSpec, ColliderShape, ColliderSpec, FpsConfig, StableColliderId};

use crate::hallway::HallwayFlavor;
use crate::teleport::{Place, PlaceGeom};

#[derive(Resource)]
pub(crate) struct GameContent {
    pub manifest: ContentManifest,
    pub simulation_hash: SimulationContentHash,
    pub presentation_hash: PresentationContentHash,
    pub collision_catalog: Arc<ContentCollisionCatalog>,
}

pub(crate) struct ContentCollisionCatalog {
    modules: BTreeMap<String, (observed_content::ArchitectureModule, BakedModule)>,
}

impl GameContent {
    pub fn committed() -> Self {
        let manifest =
            ContentManifest::parse(include_str!("../../assets/content/content_manifest.json"))
                .expect("committed content manifest parses");
        manifest
            .validate()
            .expect("committed content manifest validates");
        let baked_modules: BTreeMap<String, BakedModule> = [
            include_str!("../../assets/content/modules/gantry.json"),
            include_str!("../../assets/content/modules/colonnade.json"),
        ]
        .into_iter()
        .map(|source| {
            let module: BakedModule =
                serde_json::from_str(source).expect("committed baked module parses");
            module.validate().expect("committed baked module validates");
            (module.module_id.clone(), module)
        })
        .collect();
        let collision_catalog = Arc::new(ContentCollisionCatalog {
            modules: manifest
                .modules
                .iter()
                .filter_map(|definition| {
                    let short_id = definition
                        .baked_path
                        .rsplit('/')
                        .next()?
                        .trim_end_matches(".json");
                    baked_modules
                        .get(short_id)
                        .cloned()
                        .map(|baked| (definition.id.clone(), (definition.clone(), baked)))
                })
                .collect(),
        });
        let simulation_hash = manifest.simulation_hash();
        let presentation_hash = manifest.presentation_hash();
        Self {
            manifest,
            simulation_hash,
            presentation_hash,
            collision_catalog,
        }
    }

    pub fn traversal_config(&self) -> FpsConfig {
        config_from_profile(self.manifest.traversal)
    }
}

impl ContentCollisionCatalog {
    pub fn layout_for_place(&self, place: Place, geom: &PlaceGeom) -> Option<PlaceLayoutSnapshot> {
        let Place::Hallway { variation, .. } = place else {
            return None;
        };
        if geom.is_wellshaft() {
            return None;
        }
        let module_id = match crate::hallway::template(variation).flavor {
            HallwayFlavor::Gantry => "corridor.gantry.01",
            HallwayFlavor::Colonnade => "corridor.colonnade.01",
            _ => return None,
        };
        let (definition, _) = self.modules.get(module_id)?;
        let source_width = definition.bounds.max[0] - definition.bounds.min[0];
        let source_length = definition.bounds.max[2] - definition.bounds.min[2];
        Some(PlaceLayoutSnapshot {
            generation: variation as u32,
            placements: vec![ModulePlacement {
                module_id: module_id.into(),
                translation: [0.0, 0.0, geom.half.y],
                yaw_degrees: 0,
                scale: [
                    geom.half.x * 2.0 / source_width,
                    1.0,
                    geom.half.y * 2.0 / source_length,
                ],
                entry_port: "ground_in".into(),
                exit_port: "ground_out".into(),
            }],
        })
    }

    #[cfg(test)]
    fn contains_bake(&self, id: &str) -> bool {
        self.modules.values().any(|(_, bake)| bake.module_id == id)
    }

    pub fn arena_for_layout(
        &self,
        layout: &PlaceLayoutSnapshot,
        geom: &PlaceGeom,
        y_offset: f32,
    ) -> Option<ArenaSpec> {
        let mut colliders =
            crate::teleport::place_boundary_primitives(geom, y_offset, crate::layout::WALL_HEIGHT)
                .into_iter()
                .enumerate()
                .map(|(index, primitive)| {
                    let half_yaw = primitive.yaw * 0.5;
                    ColliderSpec {
                        id: StableColliderId(index as u32 + 1),
                        center: primitive.center,
                        rotation: [0.0, half_yaw.sin(), 0.0, half_yaw.cos()],
                        shape: ColliderShape::Cuboid {
                            half: primitive.half,
                        },
                        friction: 0.8,
                    }
                })
                .collect::<Vec<_>>();
        for placement in &layout.placements {
            let (_, baked) = self.modules.get(&placement.module_id)?;
            let yaw = (placement.yaw_degrees as f32).to_radians();
            let (sin, cos) = yaw.sin_cos();
            for hull in &baked.hulls {
                // The committed TrenchBroom kits reserve 0 for their floor slab and
                // 1/2 for left/right envelope walls. The canonical hybrid projection
                // supplies floor support through Rapier and boundary walls through the
                // aperture plan, retaining only authored interior/traversal hulls here.
                if hull.id <= 2 {
                    continue;
                }
                colliders.push(ColliderSpec {
                    id: StableColliderId(colliders.len() as u32 + 1),
                    center: bevy::math::Vec3::ZERO,
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    shape: ColliderShape::ConvexHull {
                        points: hull
                            .points
                            .iter()
                            .map(|point| {
                                let x = point[0] * placement.scale[0];
                                let z = point[2] * placement.scale[2];
                                bevy::math::Vec3::new(
                                    placement.translation[0] + x * cos + z * sin,
                                    y_offset
                                        + placement.translation[1]
                                        + point[1] * placement.scale[1],
                                    placement.translation[2] - x * sin + z * cos,
                                )
                            })
                            .collect(),
                    },
                    friction: 0.8,
                });
            }
        }
        let spec = ArenaSpec {
            colliders,
            floor_y: y_offset,
            floor_half: geom.half.max_element() + 2.0,
        };
        spec.validate().ok()?;
        Some(spec)
    }
}

fn config_from_profile(profile: TraversalProfile) -> FpsConfig {
    FpsConfig {
        walk_speed: profile.walk_speed,
        run_speed: profile.run_speed,
        ground_accel: profile.ground_accel,
        ground_decel: profile.ground_decel,
        air_accel: profile.air_accel,
        gravity: profile.gravity,
        max_fall: profile.max_fall,
        jump_speed: profile.jump_speed,
        jump_cooldown: profile.jump_cooldown,
        radius: profile.radius,
        half_height: profile.half_height,
        eye_height: profile.eye_height,
        look_step: profile.look_step,
        pitch_limit: profile.pitch_limit,
        substep: FpsConfig::default().substep,
        step_height: profile.step_height,
        controller_offset: profile.controller_offset,
        minimum_step_width: profile.minimum_step_width,
        maximum_slope_degrees: profile.maximum_slope_degrees,
        ground_snap: profile.ground_snap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_catalog_loads_every_declared_bake() {
        let content = GameContent::committed();
        for module in &content.manifest.modules {
            let short_id = module
                .baked_path
                .rsplit('/')
                .next()
                .unwrap()
                .trim_end_matches(".json");
            assert!(content.collision_catalog.contains_bake(short_id));
        }
        assert_eq!(content.traversal_config(), FpsConfig::deliberate_rapier());
    }

    #[test]
    fn proven_hallways_resolve_to_their_baked_convex_sets() {
        use observed_core::RoomId;

        let content = GameContent::committed();
        for (flavor, expected_authored_interiors) in [
            (HallwayFlavor::Gantry, 4usize),
            (HallwayFlavor::Colonnade, 8usize),
        ] {
            let variation = crate::hallway::TEMPLATES
                .iter()
                .position(|template| template.flavor == flavor)
                .unwrap();
            let template = crate::hallway::template(variation);
            let geom = crate::teleport::hallway_geom(RoomId(1), RoomId(2), template, 7, false);
            let place = Place::legacy_hallway(RoomId(1), RoomId(2), variation);
            let layout = content
                .collision_catalog
                .layout_for_place(place, &geom)
                .unwrap();
            let arena = content
                .collision_catalog
                .arena_for_layout(&layout, &geom, -8.0)
                .unwrap();
            let generated_boundary =
                crate::teleport::place_boundary_primitives(&geom, -8.0, crate::layout::WALL_HEIGHT)
                    .len();
            assert_eq!(
                arena.colliders.len(),
                generated_boundary + expected_authored_interiors
            );
            assert!(
                arena
                    .colliders
                    .iter()
                    .take(generated_boundary)
                    .all(|collider| matches!(collider.shape, ColliderShape::Cuboid { .. }))
            );
            assert!(
                arena
                    .colliders
                    .iter()
                    .skip(generated_boundary)
                    .all(|collider| matches!(collider.shape, ColliderShape::ConvexHull { .. }))
            );
        }
    }

    #[test]
    fn committed_cc0_refresh_uses_only_gate_and_cables() {
        let content = GameContent::committed();
        let ids = content
            .manifest
            .assets
            .iter()
            .map(|asset| asset.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["kenney_cables", "kenney_gate"]);
        assert!(
            content
                .manifest
                .assets
                .iter()
                .all(|asset| { asset.author == "Kenney" && asset.license == "CC0-1.0" })
        );
    }
}
