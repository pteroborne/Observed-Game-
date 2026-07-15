use std::collections::HashMap;

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_style::{self as style, MarkerRole, SurfaceRole, ThresholdFrameState};

use crate::content::GameContent;
use crate::view::assets::{ContentScene, neon_material, palette_tinted_neon_material};
use crate::view::environment::{cuboid_mesh, load_content_scene, load_repeating_texture};

#[derive(Clone)]
pub(in crate::full_wfc) struct RegisterMaterials {
    pub room_floor: Handle<StandardMaterial>,
    pub hall_floor: Handle<StandardMaterial>,
    pub wall: Handle<StandardMaterial>,
    pub ceiling: Handle<StandardMaterial>,
    pub accent: Handle<StandardMaterial>,
    pub dark: Handle<StandardMaterial>,
    pub fixture: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(in crate::full_wfc) struct FullWfcVisualAssets {
    mesh_cache: HashMap<[u32; 3], Handle<Mesh>>,
    registers: Vec<RegisterMaterials>,
    threshold: [Handle<StandardMaterial>; 3],
    pub exit: Handle<StandardMaterial>,
    pub threshold_gate: Option<ContentScene>,
    pub cable_bundle: Option<ContentScene>,
    pub vertical_ring: Handle<Mesh>,
}

impl FullWfcVisualAssets {
    pub fn load(
        asset_server: &AssetServer,
        content: &GameContent,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Self {
        let wall_texture = load_repeating_texture(asset_server, observed_assets::WALL.path);
        let floor_texture = load_repeating_texture(asset_server, observed_assets::FLOOR.path);
        let registers = ArchitectureRegister::ALL
            .into_iter()
            .map(|register| {
                let palette = style::architecture(register);
                let room = style::surface(SurfaceRole::Plain);
                let hall = style::surface(SurfaceRole::GantryDeck);
                let wall = style::surface(SurfaceRole::Wall);
                let ceiling = style::surface(SurfaceRole::Ceiling);
                let dark_palette = style::drained(&palette);
                let accent_scale =
                    (1.35 / style::luminance(palette.accent).max(0.01)).clamp(1.0, 5.5);
                let accent = style::Treatment {
                    base_color: palette.light_color,
                    emissive: palette.accent * accent_scale,
                    signal: false,
                    edge: None,
                };
                let fixture = style::Treatment {
                    base_color: palette.light_color,
                    emissive: palette.light_color.to_linear() * 1.45,
                    signal: false,
                    edge: None,
                };
                RegisterMaterials {
                    room_floor: materials.add(palette_tinted_neon_material(
                        &room,
                        &palette,
                        floor_texture.clone(),
                    )),
                    hall_floor: materials.add(palette_tinted_neon_material(
                        &hall,
                        &palette,
                        floor_texture.clone(),
                    )),
                    wall: materials.add(palette_tinted_neon_material(
                        &wall,
                        &palette,
                        wall_texture.clone(),
                    )),
                    ceiling: materials.add(StandardMaterial {
                        cull_mode: None,
                        double_sided: true,
                        ..palette_tinted_neon_material(&ceiling, &palette, None)
                    }),
                    accent: materials.add(neon_material(&accent)),
                    dark: materials.add(palette_tinted_neon_material(
                        &wall,
                        &dark_palette,
                        wall_texture.clone(),
                    )),
                    fixture: materials.add(neon_material(&fixture)),
                }
            })
            .collect();
        let threshold = ThresholdFrameState::ALL.map(|state| {
            let treatment = style::threshold_frame(state);
            materials.add(StandardMaterial {
                unlit: true,
                ..neon_material(&treatment)
            })
        });
        Self {
            mesh_cache: HashMap::new(),
            registers,
            threshold,
            exit: materials.add(StandardMaterial {
                unlit: true,
                ..neon_material(&style::marker(MarkerRole::Exit))
            }),
            threshold_gate: load_content_scene(asset_server, &content.manifest, "kenney_gate"),
            cable_bundle: load_content_scene(asset_server, &content.manifest, "kenney_cables"),
            vertical_ring: meshes.add(Torus::new(1.52, 1.6)),
        }
    }

    pub fn register(&self, register: ArchitectureRegister) -> &RegisterMaterials {
        &self.registers[register.stable_id() as usize]
    }

    pub fn threshold(&self, state: ThresholdFrameState) -> Handle<StandardMaterial> {
        let index = ThresholdFrameState::ALL
            .iter()
            .position(|candidate| *candidate == state)
            .expect("threshold state belongs to the style catalogue");
        self.threshold[index].clone()
    }

    pub fn mesh_for(&mut self, meshes: &mut Assets<Mesh>, size: Vec3) -> Handle<Mesh> {
        let key = [size.x.to_bits(), size.y.to_bits(), size.z.to_bits()];
        self.mesh_cache
            .entry(key)
            .or_insert_with(|| meshes.add(cuboid_mesh(size)))
            .clone()
    }
}
