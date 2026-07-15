//! The nine dioramas. Each scene module owns one liminal register: it spawns
//! its full rig (camera included) tagged [`SceneSpawned`], and the lab tears
//! everything down on a scene switch. Geometry is freestanding (user ruling) —
//! findings transfer to the game as parameters, not code.

// File names retain the references that originally proved each look. Public
// modules and scene identities use the production architecture catalogue.
#[path = "halo.rs"]
pub mod facet_monument;
#[path = "babel.rs"]
pub mod infinite_gallery;
#[path = "severance.rs"]
pub mod institutional;
#[path = "blame.rs"]
pub mod megastructure;
#[path = "brutalist.rs"]
pub mod monolith;
#[path = "backrooms.rs"]
pub mod overlit_grid;
#[path = "japanese.rs"]
pub mod shadow_screen;
#[path = "rudon.rs"]
pub mod thinning;
#[path = "silo.rs"]
pub mod wellshaft;

use bevy::{
    camera::Exposure,
    pbr::{DistanceFog, FogFalloff},
    post_process::bloom::Bloom,
    prelude::*,
    render::view::Hdr,
};
use observed_style as style;

/// Every entity a scene spawns, so switching scenes despawns exactly this set.
#[derive(Component)]
pub struct SceneSpawned;

/// The scene camera (exactly one per scene).
#[derive(Component)]
pub struct SceneCam;

/// The nine registers, in key order 1–9.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Scene {
    ShadowScreen,
    Monolith,
    OverlitGrid,
    Institutional,
    FacetMonument,
    Megastructure,
    Wellshaft,
    InfiniteGallery,
    Thinning,
}

impl Scene {
    pub const ALL: [Scene; 9] = [
        Scene::ShadowScreen,
        Scene::Monolith,
        Scene::OverlitGrid,
        Scene::Institutional,
        Scene::FacetMonument,
        Scene::Megastructure,
        Scene::Wellshaft,
        Scene::InfiniteGallery,
        Scene::Thinning,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Scene::ShadowScreen => "shadow-screen",
            Scene::Monolith => "monolith",
            Scene::OverlitGrid => "overlit-grid",
            Scene::Institutional => "institutional",
            Scene::FacetMonument => "facet-monument",
            Scene::Megastructure => "megastructure",
            Scene::Wellshaft => "wellshaft",
            Scene::InfiniteGallery => "infinite-gallery",
            Scene::Thinning => "thinning",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Scene::ShadowScreen => "Shadow Screen — shadow as material",
            Scene::Monolith => "Monolith — mass and one hard shaft",
            Scene::OverlitGrid => "Overlit Grid — the evenly lit nowhere",
            Scene::Institutional => "Institutional — pristine orthogonal order",
            Scene::FacetMonument => "Facet Monument — monumental indifference",
            Scene::Megastructure => "Megastructure — silhouettes against the void",
            Scene::Wellshaft => "Wellshaft — warm pools in buried dark",
            Scene::InfiniteGallery => "Infinite Gallery — infinity by repetition",
            Scene::Thinning => "Thinning — detail decays toward featurelessness",
        }
    }

    pub fn index(self) -> usize {
        Scene::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// Whether this scene's rig stages volumetric fog (the V toggle and the
    /// capture matrix only apply where a shaft exists to see).
    pub fn volumetric(self) -> bool {
        matches!(self, Scene::FacetMonument)
    }

    pub fn spawn(self, ctx: &mut SceneCtx) {
        match self {
            Scene::ShadowScreen => shadow_screen::spawn(ctx),
            Scene::Monolith => monolith::spawn(ctx),
            Scene::OverlitGrid => overlit_grid::spawn(ctx),
            Scene::Institutional => institutional::spawn(ctx),
            Scene::FacetMonument => facet_monument::spawn(ctx),
            Scene::Megastructure => megastructure::spawn(ctx),
            Scene::Wellshaft => wellshaft::spawn(ctx),
            Scene::InfiniteGallery => infinite_gallery::spawn(ctx),
            Scene::Thinning => thinning::spawn(ctx),
        }
    }
}

/// Everything a scene needs to build itself.
pub struct SceneCtx<'w, 's, 'a> {
    pub commands: &'a mut Commands<'w, 's>,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<StandardMaterial>,
}

impl SceneCtx<'_, '_, '_> {
    /// Matte surface material.
    pub fn matte(&mut self, color: Color, roughness: f32) -> Handle<StandardMaterial> {
        self.materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: roughness,
            ..default()
        })
    }

    /// Metal surface material (the Facet Monument walls).
    pub fn metal(&mut self, color: Color, roughness: f32) -> Handle<StandardMaterial> {
        self.materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: roughness,
            metallic: 0.85,
            ..default()
        })
    }

    /// Emissive material: `strength` scales the color into HDR so bloom reads it.
    pub fn glow(&mut self, color: LinearRgba, strength: f32) -> Handle<StandardMaterial> {
        self.materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: color * strength,
            perceptual_roughness: 1.0,
            ..default()
        })
    }

    /// An axis-aligned box: `center` and full `size`.
    pub fn slab(
        &mut self,
        center: Vec3,
        size: Vec3,
        material: Handle<StandardMaterial>,
        name: &'static str,
    ) {
        let mesh = self.meshes.add(Cuboid::new(size.x, size.y, size.z));
        self.commands.spawn((
            SceneSpawned,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(center),
            Name::new(name),
        ));
    }

    /// A box with an arbitrary transform (rotated facets, struts).
    pub fn slab_at(
        &mut self,
        transform: Transform,
        size: Vec3,
        material: Handle<StandardMaterial>,
        name: &'static str,
    ) {
        let mesh = self.meshes.add(Cuboid::new(size.x, size.y, size.z));
        self.commands.spawn((
            SceneSpawned,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            transform,
            Name::new(name),
        ));
    }

    /// The scene camera. HDR + natural bloom everywhere (the game's look);
    /// per-scene fog is optional; exposure is fixed so captures are comparable.
    pub fn camera(&mut self, transform: Transform, fog: Option<DistanceFog>) {
        let mut cam = self.commands.spawn((
            SceneSpawned,
            SceneCam,
            Camera3d::default(),
            Hdr,
            Bloom::NATURAL,
            Exposure::default(),
            Msaa::Off,
            transform,
            Name::new("Scene camera"),
        ));
        if let Some(fog) = fog {
            cam.insert(fog);
        }
    }

    /// Linear distance fog helper.
    pub fn fog(color: Color, start: f32, end: f32) -> DistanceFog {
        DistanceFog {
            color,
            falloff: FogFalloff::Linear { start, end },
            ..default()
        }
    }

    /// Scene-wide ambient fill.
    pub fn ambient(&mut self, color: Color, brightness: f32) {
        self.commands.insert_resource(GlobalAmbientLight {
            color,
            brightness,
            ..default()
        });
    }

    /// The signal kit, staged identically in every scene: objective, anchor
    /// device, exit door frame, rival. If a register hides any of these, the
    /// register fails the Legibility Contract — that is a finding, not a style.
    /// Treatments come from the production style crate; a small point light
    /// spills each signal's color, matching the game's marker presentation.
    pub fn signal_kit(&mut self, base: Vec3, facing: f32) {
        let rot = Quat::from_rotation_y(facing);
        let place = |offset: Vec3| base + rot * offset;

        // Objective (keystone stand-in): the "next room" marker read.
        let objective = style::marker(style::MarkerRole::NextRoom);
        self.beacon(
            place(Vec3::new(-1.8, 0.45, 0.0)),
            Vec3::new(0.35, 0.9, 0.35),
            &objective,
            "Kit objective",
        );
        // Anchor device: the seizable-control read.
        let anchor = style::marker(style::MarkerRole::Control);
        self.beacon(
            place(Vec3::new(-0.6, 0.3, 0.0)),
            Vec3::new(0.3, 0.6, 0.3),
            &anchor,
            "Kit anchor",
        );
        // Rival: team-colored presence.
        let rival = style::team(1);
        self.beacon(
            place(Vec3::new(0.6, 0.85, 0.0)),
            Vec3::new(0.45, 1.7, 0.45),
            &rival,
            "Kit rival",
        );
        // Exit door frame: two posts and a lintel in the exit read.
        let exit = style::marker(style::MarkerRole::Exit);
        let exit_mat = self.materials.add(StandardMaterial {
            base_color: exit.base_color,
            emissive: exit.emissive,
            perceptual_roughness: 0.8,
            ..default()
        });
        for dx in [-0.65_f32, 0.65] {
            self.slab_at(
                Transform::from_translation(place(Vec3::new(1.9 + dx, 1.1, 0.0)))
                    .with_rotation(rot),
                Vec3::new(0.14, 2.2, 0.14),
                exit_mat.clone(),
                "Kit exit post",
            );
        }
        self.slab_at(
            Transform::from_translation(place(Vec3::new(1.9, 2.27, 0.0))).with_rotation(rot),
            Vec3::new(1.44, 0.14, 0.14),
            exit_mat,
            "Kit exit lintel",
        );
    }

    /// One kit beacon: an emissive block plus its color spill light.
    fn beacon(&mut self, center: Vec3, size: Vec3, t: &style::Treatment, name: &'static str) {
        let mat = self.materials.add(StandardMaterial {
            base_color: t.base_color,
            emissive: t.emissive,
            perceptual_roughness: 0.8,
            ..default()
        });
        self.slab(center, size, mat, name);
        let srgb = t.base_color.to_srgba();
        self.commands.spawn((
            SceneSpawned,
            PointLight {
                color: Color::srgb(srgb.red, srgb.green, srgb.blue),
                intensity: 12_000.0,
                range: 5.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(center + Vec3::Y * (size.y * 0.5 + 0.25)),
            Name::new("Kit signal spill"),
        ));
    }
}
